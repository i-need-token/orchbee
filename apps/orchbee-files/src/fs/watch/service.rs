//! The watching of every Workspace and repository, as a state machine over inputs and the caller's
//! clock that decides which paths the native watcher watches and what each Client receives.
//!
//! - A Workspace with a subscriber watches its present root recursively. A filesystem root, a
//!   drive root or the home directory, and a root whose recursive watch the native watcher refused
//!   for a limit, watch the root's loaded folders one by one instead, at most 512; the refusal and
//!   the bound report `degraded`. Each opened file's folder is watched without recursion.
//! - The watched paths are the fewest that cover every request: a request inside a recursive watch
//!   adds none. A change of the set watches before it stops, so a path two requests share keeps
//!   its watch.
//! - A watch is in effect once the native watcher reports it started. A folder loads before it is
//!   read, so its load reaches the service ahead of the event of any later change, which finds it
//!   loaded while a watch covering it is in effect. A watch that takes effect after the load, as
//!   at the first subscription, for a folder watched one by one or after a restart, asks each
//!   Client to list again the loaded folders it covers that no watch in effect covered, and to
//!   read again its opened files in those folders, since their changes after the read went
//!   unreported, and each repository it overlaps reports a change.
//! - A watch whose path is deleted or missing ends. A missing root is read again every 5,007 ms
//!   and reported by a `root` Update when it changes within one scope; another missing path is
//!   checked as often, and its return, or a change adding it, watches it again and reports it and
//!   the opened files in it as added, as VS Code reports a resumed watch.
//! - A failed watch restarts the watches of each Workspace and repository requesting an
//!   overlapping path after 800 ms; past 5 restarts a Workspace reports `stopped` and watches
//!   nothing until the process restarts, and a repository stays unwatched until it is watched
//!   again.
//! - A repository watches its working tree as a root is watched and the first level of its Git
//!   directory, and reports to the Git engine once per batch of changes.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    time::{Duration, Instant},
};

use orchbee_files_protocol::{
    ChangeKind, ConnectionId, FilesNotice, FsUpdate, Root, WatchState, WorkspaceId,
};

use super::{
    excluded,
    stream::{self, Change, Interest, Stream},
};
use crate::{
    git::ports,
    scope::{RootFact, path::NativePath},
    staging,
};

/// How long the service waits before checking and restarting.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Timing {
    /// Between reads of a missing root or checks of another missing path.
    pub(crate) poll: Duration,
    /// Before a failed watch restarts.
    pub(crate) restart: Duration,
    /// Restarts before a Workspace stops watching.
    pub(crate) restarts: u32,
}

impl Timing {
    /// VS Code's.
    pub(crate) const VS_CODE: Self = Self {
        poll: Duration::from_millis(5_007),
        restart: Duration::from_millis(800),
        restarts: 5,
    };
}

/// Loaded folders a Workspace watches one by one when its root has no recursive watch.
const MAXIMUM_FOLDERS: usize = 512;

/// How a path is watched.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) enum Need {
    /// The path and everything below it.
    Tree,
    /// The path and its entries.
    Folder,
}

type Key = (String, Need);

/// What the native watcher reported.
#[derive(Debug)]
pub(super) enum Native {
    /// An order was carried out: these of its watches started, and these stopped.
    Applied {
        started: Vec<(NativePath, Need)>,
        stopped: Vec<(NativePath, Need)>,
    },
    Changes(Vec<Change>),
    /// Changes at or below the path, or anywhere, were dropped.
    Lost(Option<NativePath>),
    /// A watch did not start, or a recursive one no longer fits its limit.
    Refused {
        path: NativePath,
        need: Need,
        why: Why,
    },
    /// A started watch at the path, or the whole watcher, failed.
    Failed(Option<NativePath>),
}

/// Why a watch did not start.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Why {
    Missing,
    Limit,
    Broken,
}

/// One change of the native watches: start these, then stop those.
#[derive(Debug, Default, Eq, PartialEq)]
pub(super) struct Order {
    pub(super) watch: Vec<(NativePath, Need)>,
    pub(super) unwatch: Vec<(NativePath, Need)>,
}

/// A record for subscriptions.
#[derive(Debug, Eq, PartialEq)]
pub(super) enum Notice {
    /// For the subscriptions `connection` made in `workspace`.
    To {
        workspace: WorkspaceId,
        connection: ConnectionId,
        notice: FilesNotice,
    },
    /// A `root` Update for every subscription, while scope `generation` is current.
    Root {
        workspace: WorkspaceId,
        generation: u64,
        notice: FilesNotice,
    },
    /// The Workspace's new watch state, for every subscription and every later Snapshot.
    Watch {
        workspace: WorkspaceId,
        state: WatchState,
    },
}

/// What the runner does for the service.
#[derive(Default)]
pub(super) struct Effects {
    pub(super) orders: Vec<Order>,
    /// Workspaces whose root fact the runner reads.
    pub(super) roots: Vec<WorkspaceId>,
    /// Held files whose existence the runner checks.
    pub(super) checks: Vec<(WorkspaceId, NativePath)>,
    /// Missing paths the runner checks, each with the opened files directly inside it.
    pub(super) polls: Vec<(NativePath, Vec<NativePath>)>,
    pub(super) notices: Vec<Notice>,
    pub(super) reports: Vec<(ports::Report, ports::Change)>,
}

/// One repository's watch for one Workspace.
pub(super) struct Repository {
    /// The canonical toplevel.
    pub(super) tree: NativePath,
    pub(super) git_dir: Option<NativePath>,
    pub(super) report: ports::Report,
}

/// The state of every watch.
pub(super) struct Core {
    timing: Timing,
    /// The home directory's spellings.
    homes: Vec<NativePath>,
    workspaces: HashMap<WorkspaceId, Workspace>,
    repositories: HashMap<(WorkspaceId, String), Watched>,
    /// What the native watcher was told to watch.
    watched: BTreeMap<Key, NativePath>,
    /// The watched paths the native watcher reported started and that have not stopped, failed or
    /// ended since.
    effective: BTreeMap<Key, NativePath>,
    /// Requests refused for a limit, until no one makes them.
    refused: HashMap<Key, Why>,
    /// Missing paths, by [`NativePath::key`], until they return or no one requests them.
    suspended: HashMap<String, Suspension>,
    out: Effects,
}

struct Workspace {
    interest: Interest,
    /// The scope generation the interest was read under.
    generation: u64,
    /// The latest root fact.
    fact: Option<RootFact>,
    /// A root fact is being read.
    reading: bool,
    /// The root's watch ended; the next fact decides whether it is present.
    checking: bool,
    /// When a missing root is read again.
    poll: Option<Instant>,
    stream: Stream,
    state: WatchState,
    restarts: Restarts,
}

/// A repository's watch and its restarts.
struct Watched {
    repository: Repository,
    restarts: Restarts,
}

/// The restarts of one requester's watches.
#[derive(Default)]
struct Restarts {
    count: u32,
    /// When the watches start again; the requester watches nothing until then.
    at: Option<Instant>,
}

impl Restarts {
    /// Whether the requester's watches wait for a restart or ended for good under `timing`.
    fn waiting(&self, timing: Timing) -> bool {
        self.at.is_some() || self.count > timing.restarts
    }

    /// Counts a failure at `now`; `true` when it is one past the limit, which ends the watches.
    fn fail(&mut self, timing: Timing, now: Instant) -> bool {
        if self.count > timing.restarts {
            return false;
        }
        self.count += 1;
        if self.count > timing.restarts {
            self.at = None;
            return true;
        }
        self.at = Some(now + timing.restart);
        false
    }

    /// Clears a restart due at `now`; `true` when one was.
    fn due(&mut self, now: Instant) -> bool {
        let due = self.at.is_some_and(|at| at <= now);
        if due {
            self.at = None;
        }
        due
    }
}

struct Suspension {
    path: NativePath,
    /// When it is checked; absent while a check runs.
    poll: Option<Instant>,
}

impl Core {
    pub(super) fn new(timing: Timing, homes: Vec<NativePath>) -> Self {
        Self {
            timing,
            homes,
            workspaces: HashMap::new(),
            repositories: HashMap::new(),
            watched: BTreeMap::new(),
            effective: BTreeMap::new(),
            refused: HashMap::new(),
            suspended: HashMap::new(),
            out: Effects::default(),
        }
    }

    /// The Workspaces the service knows.
    pub(super) fn workspaces(&self) -> Vec<WorkspaceId> {
        self.workspaces.keys().copied().collect()
    }

    /// The work the runner does now.
    pub(super) fn effects(&mut self) -> Effects {
        std::mem::take(&mut self.out)
    }

    /// Takes `workspace`'s subscribers, loaded folders and opened files as read under scope
    /// `generation`.
    pub(super) fn interest(
        &mut self,
        workspace: WorkspaceId,
        mut interest: Interest,
        generation: u64,
    ) {
        let entry = self
            .workspaces
            .entry(workspace)
            .or_insert_with(|| Workspace {
                interest: Interest::default(),
                generation,
                fact: None,
                reading: false,
                checking: false,
                poll: None,
                stream: Stream::default(),
                state: WatchState::Full,
                restarts: Restarts::default(),
            });
        let was_idle = entry.interest.holders.is_empty();
        interest.root = entry.interest.root.take();
        entry.interest = interest;
        entry.generation = generation;
        entry.stream.retain(&entry.interest);
        if entry.interest.holders.is_empty() {
            // Without a subscriber nothing is watched or read again.
            entry.poll = None;
        } else if was_idle
            || entry
                .fact
                .as_ref()
                .is_none_or(|fact| fact.generation != generation)
        {
            read_root(entry, workspace, &mut self.out);
        }
        root_path(entry);
        self.plan();
    }

    /// Takes `workspace`'s root fact; `None` when it could not be read.
    pub(super) fn root(&mut self, workspace: WorkspaceId, fact: Option<RootFact>, now: Instant) {
        let Some(entry) = self.workspaces.get_mut(&workspace) else {
            return;
        };
        entry.reading = false;
        let Some(fact) = fact else {
            entry.poll = Some(now + self.timing.poll);
            return;
        };
        if let Some(previous) = &entry.fact
            && previous.generation == fact.generation
            && previous.root != fact.root
        {
            self.out.notices.push(Notice::Root {
                workspace,
                generation: fact.generation,
                notice: FilesNotice::FsUpdate(FsUpdate::Root {
                    root: fact.root.clone(),
                }),
            });
        }
        let missing = matches!(fact.root, Root::Missing(_));
        let current = fact.generation == entry.generation;
        entry.fact = Some(fact);
        entry.checking = false;
        entry.poll = None;
        if !entry.interest.holders.is_empty() {
            if !current {
                read_root(entry, workspace, &mut self.out);
            } else if missing {
                entry.poll = Some(now + self.timing.poll);
            }
        }
        root_path(entry);
        self.plan();
    }

    /// Takes what the native watcher reported.
    pub(super) fn native(&mut self, native: Native, now: Instant) {
        match native {
            Native::Applied { started, stopped } => self.applied(started, stopped, now),
            Native::Changes(changes) => self.changes(changes, now),
            Native::Lost(under) => {
                for workspace in self.workspaces.values_mut() {
                    workspace
                        .stream
                        .lost(under.as_ref(), &workspace.interest, now);
                }
                for watched in self.repositories.values() {
                    let repository = &watched.repository;
                    if under
                        .as_ref()
                        .is_none_or(|under| touches(repository, under))
                    {
                        let report = repository.report.clone();
                        self.out.reports.push((report, ports::Change::Content));
                    }
                }
            }
            Native::Refused { path, need, why } => {
                self.effective.remove(&(path.key(), need));
                // A recursive watch past its limit may hold some of its folders.
                if let Some(watched) = self.watched.remove(&(path.key(), need)) {
                    self.unwatch(vec![(watched, need)]);
                }
                match why {
                    Why::Limit => {
                        self.refused.insert((path.key(), need), Why::Limit);
                    }
                    Why::Missing => self.missing(&path, now),
                    Why::Broken => self.fail(Some(&path), now),
                }
                self.plan();
            }
            Native::Failed(Some(path)) => {
                let ended: Vec<Key> = self
                    .watched
                    .keys()
                    .filter(|(key, _)| *key == path.key())
                    .cloned()
                    .collect();
                let mut unwatch = Vec::new();
                for key in ended {
                    self.effective.remove(&key);
                    if let Some(watched) = self.watched.remove(&key) {
                        unwatch.push((watched, key.1));
                    }
                }
                self.unwatch(unwatch);
                self.fail(Some(&path), now);
                self.plan();
            }
            Native::Failed(None) => {
                self.effective.clear();
                let unwatch: Vec<(NativePath, Need)> = std::mem::take(&mut self.watched)
                    .into_iter()
                    .map(|((_, need), path)| (path, need))
                    .collect();
                self.unwatch(unwatch);
                self.fail(None, now);
                self.plan();
            }
        }
    }

    /// Answers the existence checks of held files.
    pub(super) fn resolved(
        &mut self,
        workspace: WorkspaceId,
        files: Vec<(NativePath, Option<bool>)>,
        now: Instant,
    ) {
        if let Some(entry) = self.workspaces.get_mut(&workspace) {
            for (file, exists) in files {
                entry.stream.resolved(&file, exists, &entry.interest, now);
            }
        }
    }

    /// Answers the check of a missing path: whether it is a folder now, and which of the opened
    /// files checked with it exist.
    pub(super) fn polled(
        &mut self,
        path: &NativePath,
        exists: bool,
        files: Vec<NativePath>,
        now: Instant,
    ) {
        let key = path.key();
        if !exists {
            if let Some(suspension) = self.suspended.get_mut(&key) {
                suspension.poll = Some(now + self.timing.poll);
            }
            return;
        }
        if self.suspended.remove(&key).is_none() {
            return;
        }
        let mut added = vec![Change {
            kind: ChangeKind::Added,
            path: path.clone(),
            directory: true,
        }];
        added.extend(files.into_iter().map(|file| Change {
            kind: ChangeKind::Added,
            path: file,
            directory: false,
        }));
        self.deliver(&added, now);
        self.plan();
    }

    /// Starts reporting `repository`'s changes for `workspace`.
    pub(super) fn watch_repository(&mut self, workspace: WorkspaceId, repository: Repository) {
        let watched = Watched {
            repository,
            restarts: Restarts::default(),
        };
        self.repositories
            .insert((workspace, watched.repository.tree.key()), watched);
        self.plan();
    }

    /// Stops reporting the changes of the repository at `tree` for `workspace`.
    pub(super) fn unwatch_repository(&mut self, workspace: WorkspaceId, tree: &NativePath) {
        self.repositories.remove(&(workspace, tree.key()));
        self.plan();
    }

    /// When the next work comes due.
    pub(super) fn due(&self) -> Option<Instant> {
        let workspaces = self.workspaces.values().flat_map(|workspace| {
            [
                workspace.restarts.at,
                workspace.poll,
                workspace.stream.due(),
            ]
            .into_iter()
            .flatten()
        });
        let repositories = self
            .repositories
            .values()
            .filter_map(|watched| watched.restarts.at);
        let suspended = self
            .suspended
            .values()
            .filter_map(|suspension| suspension.poll);
        workspaces.chain(repositories).chain(suspended).min()
    }

    /// Does the work due at `now`.
    pub(super) fn tick(&mut self, now: Instant) {
        let mut replan = false;
        for watched in self.repositories.values_mut() {
            replan |= watched.restarts.due(now);
        }
        for (id, workspace) in &mut self.workspaces {
            replan |= workspace.restarts.due(now);
            if workspace.poll.is_some_and(|poll| poll <= now) {
                workspace.poll = None;
                read_root(workspace, *id, &mut self.out);
            }
            let due = workspace.stream.tick(now, &workspace.interest);
            self.out
                .checks
                .extend(due.checks.into_iter().map(|file| (*id, file)));
            for delivery in due.deliveries {
                let notices = stream::updates(delivery.changes, delivery.rescan);
                self.out
                    .notices
                    .extend(notices.into_iter().map(|notice| Notice::To {
                        workspace: *id,
                        connection: delivery.connection,
                        notice,
                    }));
            }
        }
        for suspension in self.suspended.values_mut() {
            if suspension.poll.is_some_and(|poll| poll <= now) {
                suspension.poll = None;
                let files: Vec<NativePath> = self
                    .workspaces
                    .values()
                    .flat_map(|workspace| workspace.interest.opened.values())
                    .filter(|file| {
                        file.parent()
                            .is_some_and(|parent| parent.key() == suspension.path.key())
                    })
                    .cloned()
                    .collect();
                self.out.polls.push((suspension.path.clone(), files));
            }
        }
        if replan {
            self.plan();
        }
    }

    /// Takes an order the native watcher carried out: a path it started again stopped before its
    /// starts, any other after them. A watch the service still wants comes into effect.
    fn applied(
        &mut self,
        started: Vec<(NativePath, Need)>,
        stopped: Vec<(NativePath, Need)>,
        now: Instant,
    ) {
        let restarted: HashSet<String> = started.iter().map(|(path, _)| path.key()).collect();
        let (first, later): (Vec<_>, Vec<_>) = stopped
            .into_iter()
            .partition(|(path, _)| restarted.contains(&path.key()));
        for (path, need) in first {
            self.effective.remove(&(path.key(), need));
        }
        // A watch the service stopped wanting meanwhile stops with a later order.
        let fresh: Vec<(NativePath, Need)> = started
            .into_iter()
            .filter(|(path, need)| {
                let key = (path.key(), *need);
                self.watched.contains_key(&key) && !self.effective.contains_key(&key)
            })
            .collect();
        if !fresh.is_empty() {
            let before = Coverage::of(self.effective.iter().map(|((_, need), path)| (path, *need)));
            let after = Coverage::of(fresh.iter().map(|(path, need)| (path, *need)));
            for workspace in self.workspaces.values_mut() {
                workspace.stream.rescan(&workspace.interest, now, |folder| {
                    after.covers(folder) && !before.covers(folder)
                });
            }
            for watched in self.repositories.values() {
                let repository = &watched.repository;
                if fresh.iter().any(|(path, _)| touches(repository, path)) {
                    let report = repository.report.clone();
                    self.out.reports.push((report, ports::Change::Content));
                }
            }
        }
        for (path, need) in fresh {
            self.effective.insert((path.key(), need), path);
        }
        for (path, need) in later {
            self.effective.remove(&(path.key(), need));
        }
    }

    fn changes(&mut self, mut changes: Vec<Change>, now: Instant) {
        // A file an import is still staging never shows; publishing it reports the file.
        changes.retain(|change| !staging::is_staging(change.path.name()));
        let mut replan = false;
        let mut unwatch = Vec::new();
        for change in &changes {
            let key = change.path.key();
            match change.kind {
                ChangeKind::Deleted => {
                    let ended: Vec<Key> = self
                        .watched
                        .keys()
                        .filter(|(watched, _)| *watched == key)
                        .cloned()
                        .collect();
                    if ended.is_empty() {
                        continue;
                    }
                    for ended in ended {
                        self.effective.remove(&ended);
                        if let Some(path) = self.watched.remove(&ended) {
                            unwatch.push((path, ended.1));
                        }
                    }
                    self.missing(&change.path, now);
                    replan = true;
                }
                ChangeKind::Added => replan |= self.suspended.remove(&key).is_some(),
                ChangeKind::Changed => {}
            }
        }
        self.unwatch(unwatch);
        self.deliver(&changes, now);
        let mut reports: HashMap<(WorkspaceId, String), ports::Change> = HashMap::new();
        for (id, watched) in &self.repositories {
            for change in &changes {
                if let Some(kind) = classify(&watched.repository, change) {
                    let reported = reports.entry(id.clone()).or_insert(kind);
                    if kind == ports::Change::Repository {
                        *reported = kind;
                    }
                }
            }
        }
        for (id, change) in reports {
            if let Some(watched) = self.repositories.get(&id) {
                let report = watched.repository.report.clone();
                self.out.reports.push((report, change));
            }
        }
        if replan {
            self.plan();
        }
    }

    /// Hands `changes` to every Workspace's stream, without those in a folder its recursive watch
    /// excludes.
    fn deliver(&mut self, changes: &[Change], now: Instant) {
        for workspace in self.workspaces.values_mut() {
            if workspace.interest.holders.is_empty() {
                continue;
            }
            let tree = workspace.interest.root.as_ref();
            for change in changes {
                if tree.is_some_and(|tree| excluded(tree, &change.path)) {
                    continue;
                }
                workspace
                    .stream
                    .observe(change.clone(), &workspace.interest, now);
            }
        }
    }

    /// Ends the watch of `path`, which is missing: a root is read again, another path is checked.
    fn missing(&mut self, path: &NativePath, now: Instant) {
        let mut root = false;
        for (id, workspace) in &mut self.workspaces {
            if workspace
                .interest
                .root
                .as_ref()
                .is_some_and(|present| present.key() == path.key())
            {
                root = true;
                workspace.checking = true;
                workspace.interest.root = None;
                if !workspace.reading {
                    read_root(workspace, *id, &mut self.out);
                }
            }
        }
        if !root {
            self.suspended
                .entry(path.key())
                .or_insert_with(|| Suspension {
                    path: path.clone(),
                    poll: Some(now + self.timing.poll),
                });
        }
    }

    /// Restarts the watches of each Workspace and repository requesting a path overlapping
    /// `path`, or any path when the whole watcher failed.
    fn fail(&mut self, path: Option<&NativePath>, now: Instant) {
        let touches = |requests: &[(NativePath, Need)]| {
            !requests.is_empty()
                && path.is_none_or(|path| {
                    requests
                        .iter()
                        .any(|(requested, _)| overlaps(requested, path))
                })
        };
        for (id, workspace) in &mut self.workspaces {
            if touches(&requests(workspace, &self.refused, &self.homes, self.timing).0)
                && workspace.restarts.fail(self.timing, now)
            {
                workspace.state = WatchState::Stopped;
                self.out.notices.push(Notice::Watch {
                    workspace: *id,
                    state: WatchState::Stopped,
                });
            }
        }
        for watched in self.repositories.values_mut() {
            if touches(&repository_requests(watched, &self.homes, self.timing)) {
                watched.restarts.fail(self.timing, now);
            }
        }
    }

    fn unwatch(&mut self, unwatch: Vec<(NativePath, Need)>) {
        if !unwatch.is_empty() {
            self.out.orders.push(Order {
                watch: Vec::new(),
                unwatch,
            });
        }
    }

    /// Brings the native watches and each Workspace's state up to date with every request.
    fn plan(&mut self) {
        let mut desired: Vec<(NativePath, Need)> = Vec::new();
        let mut degraded: HashMap<WorkspaceId, bool> = HashMap::new();
        for (id, workspace) in &self.workspaces {
            let (requests, limited) = requests(workspace, &self.refused, &self.homes, self.timing);
            desired.extend(requests);
            degraded.insert(*id, limited);
        }
        for watched in self.repositories.values() {
            desired.extend(repository_requests(watched, &self.homes, self.timing));
        }
        let keys: HashSet<Key> = desired
            .iter()
            .map(|(path, need)| (path.key(), *need))
            .collect();
        self.refused.retain(|key, _| keys.contains(key));
        self.suspended
            .retain(|path, _| keys.iter().any(|(key, _)| key == path));
        let target = cover(desired.into_iter().filter(|(path, need)| {
            let key = path.key();
            !self.suspended.contains_key(&key) && !self.refused.contains_key(&(key, *need))
        }));
        // A path watched another way stops before it starts again.
        let mut order = Order::default();
        let mut restarted = Vec::new();
        for (key, path) in &self.watched {
            if !target.contains_key(key) {
                let moved = target.keys().any(|(target, _)| *target == key.0);
                let entry = (path.clone(), key.1);
                if moved {
                    restarted.push(entry);
                } else {
                    order.unwatch.push(entry);
                }
            }
        }
        if !restarted.is_empty() {
            self.out.orders.push(Order {
                watch: Vec::new(),
                unwatch: restarted,
            });
        }
        order.watch = target
            .iter()
            .filter(|(key, _)| !self.watched.contains_key(*key))
            .map(|(key, path)| (path.clone(), key.1))
            .collect();
        self.watched = target;
        if !order.watch.is_empty() || !order.unwatch.is_empty() {
            self.out.orders.push(order);
        }
        for (id, workspace) in &mut self.workspaces {
            // Stopped lasts until the process restarts.
            if workspace.state == WatchState::Stopped {
                continue;
            }
            let state = if degraded.get(id).copied().unwrap_or(false) {
                WatchState::Degraded
            } else {
                WatchState::Full
            };
            if state != workspace.state {
                workspace.state = state;
                self.out.notices.push(Notice::Watch {
                    workspace: *id,
                    state,
                });
            }
        }
    }
}

fn read_root(workspace: &mut Workspace, id: WorkspaceId, out: &mut Effects) {
    if !workspace.reading {
        workspace.reading = true;
        out.roots.push(id);
    }
}

/// Sets the interest's root to the present root its latest fact names. A new scope keeps the
/// old root until its own fact arrives, so the new root's watch starts before the old one stops.
fn root_path(workspace: &mut Workspace) {
    workspace.interest.root = match &workspace.fact {
        Some(RootFact {
            root: Root::Present(path),
            ..
        }) if !workspace.checking => NativePath::parse(path.as_str()).ok(),
        _ => None,
    };
}

/// The watches `workspace` requests, and whether it watches less than its root.
fn requests(
    workspace: &Workspace,
    refused: &HashMap<Key, Why>,
    homes: &[NativePath],
    timing: Timing,
) -> (Vec<(NativePath, Need)>, bool) {
    let interest = &workspace.interest;
    if interest.holders.is_empty() || workspace.restarts.waiting(timing) {
        return (Vec::new(), false);
    }
    let mut requests = Vec::new();
    let mut limited = false;
    if let Some(root) = &interest.root {
        let tree_refused = refused.contains_key(&(root.key(), Need::Tree));
        if special(root, homes) || tree_refused {
            limited = tree_refused;
            let folders: BTreeMap<String, NativePath> = interest
                .holders
                .values()
                .flat_map(|holder| holder.loaded.iter())
                .filter(|(_, folder)| folder.is_within(root))
                .map(|(key, folder)| (key.clone(), folder.clone()))
                .collect();
            limited |= folders.len() > MAXIMUM_FOLDERS;
            for folder in folders.into_values().take(MAXIMUM_FOLDERS) {
                limited |= refused.contains_key(&(folder.key(), Need::Folder));
                requests.push((folder, Need::Folder));
            }
        } else {
            requests.push((root.clone(), Need::Tree));
        }
    }
    for file in interest.opened.values() {
        if let Some(folder) = file.parent() {
            requests.push((folder, Need::Folder));
        }
    }
    (requests, limited)
}

/// The watches a repository requests: its working tree, unless no recursive watch may cover it,
/// and the first level of its Git directory.
fn repository_requests(
    watched: &Watched,
    homes: &[NativePath],
    timing: Timing,
) -> Vec<(NativePath, Need)> {
    let repository = &watched.repository;
    if watched.restarts.waiting(timing) {
        return Vec::new();
    }
    let tree = (!special(&repository.tree, homes)).then(|| (repository.tree.clone(), Need::Tree));
    let git_dir = repository
        .git_dir
        .as_ref()
        .map(|git_dir| (git_dir.clone(), Need::Folder));
    tree.into_iter().chain(git_dir).collect()
}

/// A filesystem root, a drive root or the home directory, which no recursive watch covers.
fn special(path: &NativePath, homes: &[NativePath]) -> bool {
    path.name().is_empty() || homes.iter().any(|home| home.key() == path.key())
}

/// The fewest watches that cover `requests`: a request inside a recursive one adds none.
pub(super) fn cover(
    requests: impl Iterator<Item = (NativePath, Need)>,
) -> BTreeMap<Key, NativePath> {
    let mut trees: BTreeMap<String, NativePath> = BTreeMap::new();
    let mut folders: BTreeMap<String, NativePath> = BTreeMap::new();
    for (path, need) in requests {
        match need {
            Need::Tree => trees.entry(path.key()).or_insert(path),
            Need::Folder => folders.entry(path.key()).or_insert(path),
        };
    }
    let mut trees: Vec<NativePath> = trees.into_values().collect();
    trees.sort_by_key(|tree| tree.as_str().len());
    let mut kept: Vec<NativePath> = Vec::new();
    for tree in trees {
        if !kept.iter().any(|outer| tree.is_within(outer)) {
            kept.push(tree);
        }
    }
    let folders: Vec<NativePath> = folders
        .into_values()
        .filter(|folder| !kept.iter().any(|tree| folder.is_within(tree)))
        .collect();
    kept.into_iter()
        .map(|tree| ((tree.key(), Need::Tree), tree))
        .chain(
            folders
                .into_iter()
                .map(|folder| ((folder.key(), Need::Folder), folder)),
        )
        .collect()
}

/// The folders a set of watches covers: those inside its recursive watches and at its folder
/// watches.
struct Coverage<'a> {
    trees: Vec<&'a NativePath>,
    /// By [`NativePath::key`].
    folders: HashSet<String>,
}

impl<'a> Coverage<'a> {
    fn of(watches: impl Iterator<Item = (&'a NativePath, Need)>) -> Self {
        let mut coverage = Self {
            trees: Vec::new(),
            folders: HashSet::new(),
        };
        for (path, need) in watches {
            match need {
                Need::Tree => coverage.trees.push(path),
                Need::Folder => {
                    coverage.folders.insert(path.key());
                }
            }
        }
        coverage
    }

    fn covers(&self, folder: &NativePath) -> bool {
        self.trees.iter().any(|tree| folder.is_within(tree)) || self.folders.contains(&folder.key())
    }
}

fn overlaps(one: &NativePath, other: &NativePath) -> bool {
    one.is_within(other) || other.is_within(one)
}

/// Whether `path` overlaps `repository`'s working tree or Git directory.
fn touches(repository: &Repository, path: &NativePath) -> bool {
    overlaps(&repository.tree, path)
        || repository
            .git_dir
            .as_ref()
            .is_some_and(|git_dir| overlaps(git_dir, path))
}

/// What `change` tells `repository`'s engine, if anything.
fn classify(repository: &Repository, change: &Change) -> Option<ports::Change> {
    let key = change.path.key();
    let git_dir = repository.git_dir.as_ref();
    if key == repository.tree.key() || git_dir.is_some_and(|git_dir| git_dir.key() == key) {
        return Some(ports::Change::Repository);
    }
    if let Some(git_dir) = git_dir
        && change
            .path
            .parent()
            .is_some_and(|parent| parent.key() == git_dir.key())
    {
        let name = change.path.name();
        return (name != "index.lock" && !name.starts_with(".watchman-cookie-"))
            .then_some(ports::Change::Content);
    }
    let mut below = change.path.below(&repository.tree)?;
    (!below.any(|segment| segment == ".git")).then_some(ports::Change::Content)
}

#[cfg(test)]
mod tests;
