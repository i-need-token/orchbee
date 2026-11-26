//! Workspace searches: each window's search of a Workspace runs in the search lane, a separate
//! process of this executable that the Server starts with the first search, and what it finds is
//! held here until that window's search stream reads it a page at a time. A window has one search
//! of a Workspace at once: a new one replaces it, and closing its stream or its connection stops
//! it. A lane that ends ends its searches as unavailable; the next search starts another.
use orchbee_server::{SessionChangeSignal, SessionChangeSource};
use orchbee_server_workspace_files::{
    MAXIMUM_SEARCH_REPORT_LINE_BYTES, SEARCH_FLAG, SearchCommand, SearchReport,
};
use orchbee_workbench_protocol::{
    MAXIMUM_WORKSPACE_FILE_ENTRIES, WorkbenchRequestId, WorkbenchWorkspaceId,
    WorkspaceFilesOutcome as Outcome, WorkspaceSearchFile, WorkspaceSearchQuery,
    WorkspaceSearchState as State,
};
use std::collections::{BTreeMap, VecDeque};
use std::io::{BufRead, BufReader, Read, Write};
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// Searches the Server holds at once, across every window and Workspace; the lane runs fewer.
const MAXIMUM_SEARCHES: usize = 64;
/// Encoded entries one page carries before it stops, unless its first entry alone is larger.
const PAGE_BYTES: usize = 1024 * 1024;

/// One window's search of one Workspace.
struct Search {
    request_id: WorkbenchRequestId,
    /// The search's identity in the lane.
    lane_id: u64,
    /// The lane it runs in.
    incarnation: u64,
    /// What the lane found that no page has carried yet, with each entry's encoded size.
    held: VecDeque<(WorkspaceSearchFile, usize)>,
    /// Entries earlier pages carried.
    delivered: u64,
    /// How the lane said the search ended, once it has.
    ended: Option<State>,
    /// The page last answered, answered again while nothing has changed.
    last: Option<Outcome>,
}

/// The running lane; dropping it ends the process.
struct Lane {
    child: Child,
    stdin: ChildStdin,
    incarnation: u64,
}

impl Drop for Lane {
    fn drop(&mut self) {
        let _killed = self.child.kill();
        let _reaped = self.child.wait();
    }
}

#[derive(Default)]
struct Registry {
    searches: BTreeMap<(NonZeroU64, WorkbenchWorkspaceId), Search>,
    lane: Option<Lane>,
    incarnations: u64,
    next_id: u64,
}

struct Shared {
    executable: Option<PathBuf>,
    registry: Mutex<Registry>,
    changes: SessionChangeSource,
}

/// Every window's searches, and the lane that runs them.
pub(super) struct Searches(Arc<Shared>);

impl Default for Searches {
    fn default() -> Self {
        Self(Arc::new(Shared {
            executable: std::env::current_exe().ok(),
            registry: Mutex::new(Registry::default()),
            changes: SessionChangeSource::default(),
        }))
    }
}

impl Searches {
    /// Wakes `signal` whenever a search finds more or ends.
    pub(super) fn subscribe(&self, signal: &Arc<SessionChangeSignal>) {
        self.0.changes.watch(signal);
    }

    /// The next page of `owner`'s search `request_id` of `workspace`. A search not yet running
    /// starts, beneath `root`, and replaces that owner's earlier search of the Workspace.
    pub(super) fn page(
        &self,
        owner: NonZeroU64,
        workspace: &WorkbenchWorkspaceId,
        request_id: &WorkbenchRequestId,
        root: &str,
        query: &WorkspaceSearchQuery,
    ) -> Outcome {
        let mut registry = self.0.lock();
        let key = (owner, workspace.clone());
        let current = registry
            .searches
            .get(&key)
            .is_some_and(|search| search.request_id == *request_id);
        if !current {
            if let Some(replaced) = registry.searches.remove(&key) {
                cancel(&mut registry, &replaced);
            }
            if registry.searches.len() >= MAXIMUM_SEARCHES {
                return ended(State::Unavailable);
            }
            let search = start(&self.0, &mut registry, request_id, root, query);
            registry.searches.insert(key.clone(), search);
        }
        registry
            .searches
            .get_mut(&key)
            .map_or_else(|| ended(State::Unavailable), next_page)
    }

    /// Ends `owner`'s search of `workspace` while it is still `request_id`, or every search of
    /// `owner` for none.
    pub(super) fn forget(
        &self,
        owner: NonZeroU64,
        workspace: Option<(&WorkbenchWorkspaceId, &WorkbenchRequestId)>,
    ) {
        let mut registry = self.0.lock();
        let keys: Vec<_> = registry
            .searches
            .iter()
            .filter(|((holder, searched), search)| {
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

/// A page that ends a search with nothing found.
fn ended(state: State) -> Outcome {
    Outcome::SearchResults {
        from: 0,
        files: Vec::new(),
        more: false,
        state,
    }
}

/// Takes the next page from what `search` holds; the same page as last time while nothing new
/// has come, so a stream woken for another reason sends nothing.
fn next_page(search: &mut Search) -> Outcome {
    let mut files = Vec::new();
    let mut bytes = 0usize;
    while let Some((_, size)) = search.held.front() {
        if !files.is_empty()
            && (bytes.saturating_add(*size) > PAGE_BYTES
                || files.len() >= MAXIMUM_WORKSPACE_FILE_ENTRIES)
        {
            break;
        }
        bytes = bytes.saturating_add(*size);
        if let Some((file, _)) = search.held.pop_front() {
            files.push(file);
        }
    }
    let state = if search.held.is_empty() {
        search.ended.unwrap_or(State::Searching)
    } else {
        State::Searching
    };
    if files.is_empty()
        && let Some(last) = &search.last
        && matches!(last, Outcome::SearchResults { state: answered, .. } if *answered == state)
    {
        return last.clone();
    }
    let page = Outcome::SearchResults {
        from: search.delivered,
        more: !search.held.is_empty(),
        state,
        files,
    };
    if let Outcome::SearchResults { files, .. } = &page {
        search.delivered = search
            .delivered
            .saturating_add(u64::try_from(files.len()).unwrap_or(u64::MAX));
    }
    search.last = Some(page.clone());
    page
}

/// Starts a search in the lane, starting the lane first if none runs. A search the lane cannot
/// take has already ended as unavailable.
fn start(
    shared: &Arc<Shared>,
    registry: &mut Registry,
    request_id: &WorkbenchRequestId,
    root: &str,
    query: &WorkspaceSearchQuery,
) -> Search {
    registry.next_id = registry.next_id.wrapping_add(1);
    let lane_id = registry.next_id;
    let mut search = Search {
        request_id: request_id.clone(),
        lane_id,
        incarnation: 0,
        held: VecDeque::new(),
        delivered: 0,
        ended: None,
        last: None,
    };
    // A lane that exited on its own, after its idle time, is replaced.
    if registry
        .lane
        .as_mut()
        .is_some_and(|lane| !matches!(lane.child.try_wait(), Ok(None)))
    {
        registry.lane = None;
    }
    if registry.lane.is_none() && start_lane(shared, registry).is_err() {
        search.ended = Some(State::Unavailable);
        return search;
    }
    let command = SearchCommand::Start {
        id: lane_id,
        root: root.to_owned(),
        query: query.clone(),
    };
    if let Some(incarnation) = send(registry, &command) {
        search.incarnation = incarnation;
    } else {
        search.ended = Some(State::Unavailable);
    }
    search
}

/// Stops a search the lane may still be running.
fn cancel(registry: &mut Registry, search: &Search) {
    if search.ended.is_none() {
        let _sent = send(registry, &SearchCommand::Cancel { id: search.lane_id });
    }
}

/// Writes one command to the running lane, and names that lane; a lane that cannot take it is
/// ended, and its reader ends its searches.
fn send(registry: &mut Registry, command: &SearchCommand) -> Option<u64> {
    let mut line = serde_json::to_vec(command).ok()?;
    line.push(b'\n');
    let lane = registry.lane.as_mut()?;
    let written = lane
        .stdin
        .write_all(&line)
        .and_then(|()| lane.stdin.flush())
        .is_ok();
    if written {
        Some(lane.incarnation)
    } else {
        registry.lane = None;
        None
    }
}

fn start_lane(shared: &Arc<Shared>, registry: &mut Registry) -> Result<(), ()> {
    let executable = shared.executable.as_ref().ok_or(())?;
    let mut command = Command::new(executable);
    command
        .arg(SEARCH_FLAG)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: the windowless Server's child opens no console.
        command.creation_flags(0x0800_0000);
    }
    let mut child = command.spawn().map_err(|_| ())?;
    let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        let _killed = child.kill();
        let _reaped = child.wait();
        return Err(());
    };
    registry.incarnations = registry.incarnations.wrapping_add(1);
    let incarnation = registry.incarnations;
    let reader = Arc::clone(shared);
    let spawned = std::thread::Builder::new()
        .name("orchbee-server-workspace-files-search".into())
        .spawn(move || read_reports(&reader, stdout, incarnation));
    if spawned.is_err() {
        let _killed = child.kill();
        let _reaped = child.wait();
        return Err(());
    }
    registry.lane = Some(Lane {
        child,
        stdin,
        incarnation,
    });
    Ok(())
}

/// Holds each report of one lane for its search until the lane ends, then ends every search it
/// was still running as unavailable.
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
                SearchReport::Found { id, files } => (id, files, None),
                SearchReport::Ended { id, state } => (id, Vec::new(), Some(state)),
            };
            registry
                .searches
                .values_mut()
                .find(|search| search.lane_id == id && search.incarnation == incarnation)
                .is_some_and(|search| {
                    for file in found {
                        let size =
                            serde_json::to_vec(&file).map_or(usize::MAX, |bytes| bytes.len());
                        search.held.push_back((file, size));
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

#[cfg(test)]
mod tests;
