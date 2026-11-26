//! Workspace searches and file listings: each window's search or listing of a Workspace runs in
//! the search lane, a Workspace files service the Server starts with the first of them, and what
//! it finds is held here until that window's stream reads it a page at a time. A window has one
//! search and one listing of a Workspace at once: a new one replaces the one of its kind, and
//! closing its stream or its connection stops it. A lane that ends ends what it ran as
//! unavailable; the next search or listing starts another. A lane starts on a thread of its own:
//! what is asked meanwhile waits there and is handed to it once it runs.
use super::launch::{Connection, Launcher};
use orchbee_capability_node_protocol::WorkspaceFilesService;
use orchbee_server::{SessionChangeSignal, SessionChangeSource};
use orchbee_workbench_protocol::{
    MAXIMUM_WORKSPACE_FILE_ENTRIES, MAXIMUM_WORKSPACE_FILE_LIST_PAGE, WorkbenchRequestId,
    WorkbenchWorkspaceId, WorkspaceFileListQuery, WorkspaceFilesOutcome as Outcome,
    WorkspaceSearchFile, WorkspaceSearchQuery, WorkspaceSearchState as State,
};
use orchbee_workspace_files::{MAXIMUM_SEARCH_REPORT_LINE_BYTES, SearchCommand, SearchReport};
use std::collections::{BTreeMap, VecDeque};
use std::io::{BufRead, BufReader, Read};
use std::num::NonZeroU64;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// Searches and listings the Server holds at once, across every window and Workspace; the lane
/// runs fewer.
const MAXIMUM_SEARCHES: usize = 64;
/// Encoded entries one page carries before it stops, unless its first entry alone is larger.
const PAGE_BYTES: usize = 1024 * 1024;

/// What a window asks the lane to do beneath a Workspace.
#[derive(Clone, Copy)]
pub(super) enum Query<'a> {
    Search(&'a WorkspaceSearchQuery),
    List(&'a WorkspaceFileListQuery),
}

/// The two kinds a window holds one of each per Workspace.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Search,
    List,
}

impl Query<'_> {
    const fn kind(self) -> Kind {
        match self {
            Self::Search(_) => Kind::Search,
            Self::List(_) => Kind::List,
        }
    }
}

/// What the lane reported that no page has carried yet, with each entry's encoded size.
enum Held {
    Files(VecDeque<(WorkspaceSearchFile, usize)>),
    Paths(VecDeque<(String, usize)>),
}

impl Held {
    const fn new(kind: Kind) -> Self {
        match kind {
            Kind::Search => Self::Files(VecDeque::new()),
            Kind::List => Self::Paths(VecDeque::new()),
        }
    }
}

/// One window's search or listing of one Workspace.
struct Search {
    request_id: WorkbenchRequestId,
    /// The search's identity in the lane.
    lane_id: u64,
    /// The lane it runs in; 0 while it waits for one.
    incarnation: u64,
    /// What starts it in the lane, while it waits for one to start.
    waiting: Option<SearchCommand>,
    /// What the lane found that no page has carried yet.
    held: Held,
    /// Entries earlier pages carried.
    delivered: u64,
    /// How the lane said the search ended, once it has.
    ended: Option<State>,
    /// The page last answered, answered again while nothing has changed.
    last: Option<Outcome>,
}

/// The running lane; dropping it ends the service.
struct Lane {
    connection: Connection,
    incarnation: u64,
}

#[derive(Default)]
struct Registry {
    searches: BTreeMap<(NonZeroU64, WorkbenchWorkspaceId, Kind), Search>,
    lane: Option<Lane>,
    /// A lane is starting; no other start runs meanwhile.
    starting: bool,
    incarnations: u64,
    next_id: u64,
}

struct Shared {
    launcher: Arc<Launcher>,
    registry: Mutex<Registry>,
    changes: SessionChangeSource,
}

/// Every window's searches and listings, and the lane that runs them.
pub(super) struct Searches(Arc<Shared>);

impl Searches {
    pub(super) fn new(launcher: Arc<Launcher>) -> Self {
        Self(Arc::new(Shared {
            launcher,
            registry: Mutex::new(Registry::default()),
            changes: SessionChangeSource::default(),
        }))
    }

    /// Wakes `signal` whenever a search or listing finds more or ends.
    pub(super) fn subscribe(&self, signal: &Arc<SessionChangeSignal>) {
        self.0.changes.watch(signal);
    }

    /// The next page of `owner`'s search or listing `request_id` of `workspace`. One not yet
    /// running starts, beneath `root`, and replaces that owner's earlier one of its kind.
    pub(super) fn page(
        &self,
        owner: NonZeroU64,
        workspace: &WorkbenchWorkspaceId,
        request_id: &WorkbenchRequestId,
        root: &str,
        query: Query<'_>,
    ) -> Outcome {
        let mut registry = self.0.lock();
        let kind = query.kind();
        let key = (owner, workspace.clone(), kind);
        let current = registry
            .searches
            .get(&key)
            .is_some_and(|search| search.request_id == *request_id);
        if !current {
            if let Some(replaced) = registry.searches.remove(&key) {
                cancel(&mut registry, &replaced);
            }
            if registry.searches.len() >= MAXIMUM_SEARCHES {
                return ended(kind, State::Unavailable);
            }
            let search = start(&self.0, &mut registry, request_id, root, query);
            registry.searches.insert(key.clone(), search);
        }
        registry
            .searches
            .get_mut(&key)
            .map_or_else(|| ended(kind, State::Unavailable), next_page)
    }

    /// Ends `owner`'s search or listing of `workspace` while it is still `request_id`, or every
    /// one of `owner` for none.
    pub(super) fn forget(
        &self,
        owner: NonZeroU64,
        workspace: Option<(&WorkbenchWorkspaceId, &WorkbenchRequestId)>,
    ) {
        let mut registry = self.0.lock();
        let keys: Vec<_> = registry
            .searches
            .iter()
            .filter(|((holder, searched, _), search)| {
                *holder == owner
                    && workspace.is_none_or(|(workspace, request_id)| {
                        searched == workspace && search.request_id == *request_id
                    })
            })
            .map(|(key, _)| key.clone())
            .collect();
        for key in keys {
            if let Some(search) = registry.searches.remove(&key) {
                cancel(&mut registry, &search);
            }
        }
    }
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Registry> {
        self.registry.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A page that ends a search or listing with nothing found.
fn ended(kind: Kind, state: State) -> Outcome {
    match kind {
        Kind::Search => Outcome::SearchResults {
            from: 0,
            files: Vec::new(),
            more: false,
            state,
        },
        Kind::List => Outcome::FileList {
            from: 0,
            paths: Vec::new(),
            more: false,
            state,
        },
    }
}

/// Takes the entries of the next page from `held`: at most `entries`, and no more than
/// [`PAGE_BYTES`] once it has one.
fn take<T>(held: &mut VecDeque<(T, usize)>, entries: usize) -> Vec<T> {
    let mut taken = Vec::new();
    let mut bytes = 0usize;
    while let Some((_, size)) = held.front() {
        if !taken.is_empty() && (bytes.saturating_add(*size) > PAGE_BYTES || taken.len() >= entries)
        {
            break;
        }
        bytes = bytes.saturating_add(*size);
        if let Some((entry, _)) = held.pop_front() {
            taken.push(entry);
        }
    }
    taken
}

/// Takes the next page from what `search` holds; the same page as last time while nothing new
/// has come, so a stream woken for another reason sends nothing.
fn next_page(search: &mut Search) -> Outcome {
    let from = search.delivered;
    let ended = search.ended.unwrap_or(State::Searching);
    let (page, count) = match &mut search.held {
        Held::Files(held) => {
            let files = take(held, MAXIMUM_WORKSPACE_FILE_ENTRIES);
            let count = files.len();
            let more = !held.is_empty();
            let state = if more { State::Searching } else { ended };
            (
                Outcome::SearchResults {
                    from,
                    files,
                    more,
                    state,
                },
                count,
            )
        }
        Held::Paths(held) => {
            let paths = take(held, MAXIMUM_WORKSPACE_FILE_LIST_PAGE);
            let count = paths.len();
            let more = !held.is_empty();
            let state = if more { State::Searching } else { ended };
            (
                Outcome::FileList {
                    from,
                    paths,
                    more,
                    state,
                },
                count,
            )
        }
    };
    if count == 0
        && let Some(last) = &search.last
        && state_of(last) == state_of(&page)
    {
        return last.clone();
    }
    search.delivered = from.saturating_add(u64::try_from(count).unwrap_or(u64::MAX));
    search.last = Some(page.clone());
    page
}

/// The state a page answered.
const fn state_of(page: &Outcome) -> Option<State> {
    match page {
        Outcome::SearchResults { state, .. } | Outcome::FileList { state, .. } => Some(*state),
        _ => None,
    }
}

/// Starts a search or listing in the lane, or holds it until a lane starts. One that cannot be
/// started has already ended as unavailable.
fn start(
    shared: &Arc<Shared>,
    registry: &mut Registry,
    request_id: &WorkbenchRequestId,
    root: &str,
    query: Query<'_>,
) -> Search {
    registry.next_id = registry.next_id.wrapping_add(1);
    let lane_id = registry.next_id;
    let command = match query {
        Query::Search(query) => SearchCommand::Start {
            id: lane_id,
            root: root.to_owned(),
            query: query.clone(),
        },
        Query::List(query) => SearchCommand::List {
            id: lane_id,
            root: root.to_owned(),
            query: query.clone(),
        },
    };
    let mut search = Search {
        request_id: request_id.clone(),
        lane_id,
        incarnation: 0,
        waiting: None,
        held: Held::new(query.kind()),
        delivered: 0,
        ended: None,
        last: None,
    };
    if let Some(incarnation) = send(registry, &command) {
        search.incarnation = incarnation;
        return search;
    }
    // No lane runs, or the one that ran could not take it.
    search.waiting = Some(command);
    if !registry.starting {
        registry.starting = true;
        let starter = Arc::clone(shared);
        let spawned = std::thread::Builder::new()
            .name("orchbee-server-workspace-files-search-start".into())
            .spawn(move || start_lane(&starter));
        if spawned.is_err() {
            registry.starting = false;
            search.waiting = None;
            search.ended = Some(State::Unavailable);
        }
    }
    search
}

/// Stops a search or listing the lane may still be running.
fn cancel(registry: &mut Registry, search: &Search) {
    if search.ended.is_none() && search.waiting.is_none() {
        let _sent = send(registry, &SearchCommand::Cancel { id: search.lane_id });
    }
}

/// Writes one command to the running lane, and names that lane; a lane that cannot take it is
/// shut down, and its reader ends its searches.
fn send(registry: &mut Registry, command: &SearchCommand) -> Option<u64> {
    let mut line = serde_json::to_vec(command).ok()?;
    line.push(b'\n');
    let lane = registry.lane.take()?;
    if lane.connection.write_line(&line).is_ok() {
        let incarnation = lane.incarnation;
        registry.lane = Some(lane);
        Some(incarnation)
    } else {
        // Dropping the lane shuts it down; its reader ends what it ran.
        None
    }
}

/// Starts a lane and hands it every search and listing waiting for one; if it cannot start, they
/// end as unavailable.
fn start_lane(shared: &Arc<Shared>) {
    // The registry is not held while the service starts, so pages are answered meanwhile.
    let launched = shared.launcher.launch(WorkspaceFilesService::Search);
    let mut registry = shared.lock();
    registry.starting = false;
    let waiting = registry
        .searches
        .values()
        .any(|search| search.waiting.is_some());
    if waiting {
        if let Ok(connection) = launched {
            install(shared, &mut registry, connection);
        }
        let mut commands = Vec::new();
        for search in registry.searches.values_mut() {
            if let Some(command) = search.waiting.take() {
                commands.push((search.lane_id, command));
            }
        }
        for (lane_id, command) in commands {
            let incarnation = send(&mut registry, &command);
            if let Some(search) = registry
                .searches
                .values_mut()
                .find(|search| search.lane_id == lane_id)
            {
                match incarnation {
                    Some(incarnation) => search.incarnation = incarnation,
                    None => search.ended = Some(State::Unavailable),
                }
            }
        }
    }
    // Without a search or listing waiting, a started service ends with its connection here.
    drop(registry);
    shared.changes.notify_workspace_files();
}

/// Makes `connection` the running lane, with a thread that reads its reports.
fn install(shared: &Arc<Shared>, registry: &mut Registry, connection: Connection) {
    let Ok(reports) = connection.reader() else {
        return;
    };
    registry.incarnations = registry.incarnations.wrapping_add(1).max(1);
    let incarnation = registry.incarnations;
    let reader = Arc::clone(shared);
    let spawned = std::thread::Builder::new()
        .name("orchbee-server-workspace-files-search".into())
        .spawn(move || read_reports(&reader, reports, incarnation));
    if spawned.is_ok() {
        registry.lane = Some(Lane {
            connection,
            incarnation,
        });
    }
}

/// Holds each report of one lane for its search or listing until the lane ends, then ends every
/// one it was still running as unavailable.
fn read_reports(shared: &Arc<Shared>, stdout: impl Read, incarnation: u64) {
    let mut stdout = BufReader::new(stdout);
    let limit = u64::try_from(MAXIMUM_SEARCH_REPORT_LINE_BYTES).unwrap_or(u64::MAX) + 1;
    loop {
        let mut line = Vec::new();
        match (&mut stdout).take(limit).read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        if line.last() != Some(&b'\n') {
            // A line past the lane's own limit: the lane is not one this Server knows.
            break;
        }
        let Ok(report) = serde_json::from_slice::<SearchReport>(&line) else {
            continue;
        };
        let changed = {
            let mut registry = shared.lock();
            let (id, found, state) = match report {
                SearchReport::Found { id, files } => (id, Found::Files(files), None),
                SearchReport::Listed { id, paths } => (id, Found::Paths(paths), None),
                SearchReport::Ended { id, state } => (id, Found::Paths(Vec::new()), Some(state)),
            };
            registry
                .searches
                .values_mut()
                .find(|search| search.lane_id == id && search.incarnation == incarnation)
                .is_some_and(|search| {
                    match (&mut search.held, found) {
                        (Held::Files(held), Found::Files(files)) => hold(held, files),
                        (Held::Paths(held), Found::Paths(paths)) => hold(held, paths),
                        // A report of the other kind is not one this Server asked for.
                        _ => {}
                    }
                    if search.ended.is_none() {
                        search.ended = state;
                    }
                    true
                })
        };
        if changed {
            shared.changes.notify_workspace_files();
        }
    }
    let mut registry = shared.lock();
    if registry
        .lane
        .as_ref()
        .is_some_and(|lane| lane.incarnation == incarnation)
    {
        registry.lane = None;
    }
    let mut changed = false;
    for search in registry.searches.values_mut() {
        if search.incarnation == incarnation && search.ended.is_none() {
            search.ended = Some(State::Unavailable);
            changed = true;
        }
    }
    drop(registry);
    if changed {
        shared.changes.notify_workspace_files();
    }
}

/// What one report found.
enum Found {
    Files(Vec<WorkspaceSearchFile>),
    Paths(Vec<String>),
}

/// Holds `found` after what `held` already holds, with each entry's encoded size.
fn hold<T: serde::Serialize>(held: &mut VecDeque<(T, usize)>, found: Vec<T>) {
    for entry in found {
        let size = serde_json::to_vec(&entry).map_or(usize::MAX, |bytes| bytes.len());
        held.push_back((entry, size));
    }
}

#[cfg(test)]
mod tests;
