//! Workspace file watches: what each window's right bar follows, joined per Workspace directory
//! and handed to the watch lane, a Workspace files service the Server starts with the first watch.
//! What the lane reports advances each directory's revisions and wakes the watch streams, which
//! reread them; a lane that ends is started again while any watch remains. A lane starts on a
//! thread of its own, so a watch is answered at once, as starting, and never waits for it.
use super::launch::{Connection, Launcher};
use orchbee_capability_node_protocol::WorkspaceFilesService;
use orchbee_server::{SessionChangeSignal, SessionChangeSource};
use orchbee_workbench_protocol::{
    MAXIMUM_WORKSPACE_FILES_WATCHED_DIRECTORIES, MAXIMUM_WORKSPACE_FILES_WATCHED_FILES,
    WorkbenchRevision, WorkbenchWorkspaceId, WorkspaceFilesOutcome as Outcome,
    WorkspaceFilesWatchState as State,
};
use orchbee_workspace_files::{MAXIMUM_WATCHED_ROOTS, WatchEvent, WatchRoot, WatchSet};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader, Read};
use std::num::NonZeroU64;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// Watches the Server holds at once, across every window and Workspace.
const MAXIMUM_WATCHES: usize = 256;
/// One watch-set line's bytes; paths beyond it are left out, deepest Workspace first.
const MAXIMUM_WATCH_SET_BYTES: usize = 4 * 1024 * 1024;
/// One report line's bytes.
const MAXIMUM_REPORT_LINE_BYTES: u64 = 64 * 1024;
/// The first wait before a lane that ended starts again; each quick end doubles it.
const FIRST_RESTART: Duration = Duration::from_secs(1);
const LAST_RESTART: Duration = Duration::from_mins(1);
/// A lane that ran this long ended for some other reason than starting badly.
const STEADY: Duration = Duration::from_mins(1);

/// What one window's watch of one Workspace names.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Watch {
    root: String,
    directories: Vec<String>,
    files: Vec<String>,
}

/// One Workspace directory's revisions and whether the lane follows it.
#[derive(Clone, Copy, Debug)]
struct Followed {
    files: u64,
    repository: u64,
    state: State,
}

/// The running lane; dropping it ends the service.
struct Lane {
    connection: Connection,
    /// Tells this lane's reader apart from a later one's.
    incarnation: u64,
    started: Instant,
}

#[derive(Default)]
struct Registry {
    watches: BTreeMap<(NonZeroU64, WorkbenchWorkspaceId), Watch>,
    followed: BTreeMap<String, Followed>,
    lane: Option<Lane>,
    /// The set the running lane was last given.
    sent: Option<WatchSet>,
    incarnations: u64,
    /// A start, or the wait before one, is under way; no other start runs meanwhile.
    starting: bool,
    /// The last wait before a restart.
    backoff: Option<Duration>,
}

struct Shared {
    launcher: Arc<Launcher>,
    registry: Mutex<Registry>,
    changes: SessionChangeSource,
}

/// Every window's watches, and the lane that follows them.
pub(super) struct Watches(Arc<Shared>);

impl Watches {
    pub(super) fn new(launcher: Arc<Launcher>) -> Self {
        Self(Arc::new(Shared {
            launcher,
            registry: Mutex::new(Registry::default()),
            changes: SessionChangeSource::default(),
        }))
    }

    /// Wakes `signal` whenever a watched directory's revisions or state change.
    pub(super) fn subscribe(&self, signal: &Arc<SessionChangeSignal>) {
        self.0.changes.watch(signal);
    }

    /// Records what `owner`'s watch of `workspace` names, replacing its earlier watch of it, and
    /// answers that directory's revisions as they stand.
    pub(super) fn watch(
        &self,
        owner: NonZeroU64,
        workspace: &WorkbenchWorkspaceId,
        root: &str,
        directories: &[String],
        files: &[String],
    ) -> Outcome {
        let mut registry = self.0.lock();
        let key = (owner, workspace.clone());
        if !registry.watches.contains_key(&key) && registry.watches.len() >= MAXIMUM_WATCHES {
            return watching(None);
        }
        registry.watches.insert(
            key,
            Watch {
                root: root.to_owned(),
                directories: directories.to_vec(),
                files: files.to_vec(),
            },
        );
        sync(&self.0, &mut registry);
        watching(registry.followed.get(root).copied())
    }

    /// Ends `owner`'s watch of `workspace`, or every watch of `owner` for none.
    pub(super) fn forget(&self, owner: NonZeroU64, workspace: Option<&WorkbenchWorkspaceId>) {
        let mut registry = self.0.lock();
        let before = registry.watches.len();
        registry.watches.retain(|(holder, watched), _| {
            *holder != owner || workspace.is_some_and(|workspace| watched != workspace)
        });
        if registry.watches.len() != before {
            sync(&self.0, &mut registry);
        }
    }
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Registry> {
        self.registry.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn watching(followed: Option<Followed>) -> Outcome {
    let followed = followed.unwrap_or(Followed {
        files: 0,
        repository: 0,
        state: State::Unavailable,
    });
    Outcome::Watching {
        files: WorkbenchRevision::new(followed.files),
        repository: WorkbenchRevision::new(followed.repository),
        state: followed.state,
    }
}

/// The set every watch names, one root per Workspace directory, within the lane's limits.
fn desired(registry: &Registry) -> WatchSet {
    let mut joined: BTreeMap<&str, (BTreeSet<&str>, BTreeSet<&str>)> = BTreeMap::new();
    for watch in registry.watches.values() {
        if joined.len() >= MAXIMUM_WATCHED_ROOTS && !joined.contains_key(watch.root.as_str()) {
            continue;
        }
        let (directories, files) = joined.entry(&watch.root).or_default();
        for directory in &watch.directories {
            if directories.len() < MAXIMUM_WORKSPACE_FILES_WATCHED_DIRECTORIES {
                directories.insert(directory);
            }
        }
        for file in &watch.files {
            if files.len() < MAXIMUM_WORKSPACE_FILES_WATCHED_FILES {
                files.insert(file);
            }
        }
    }
    let mut set = WatchSet {
        roots: joined
            .into_iter()
            .map(|(root, (directories, files))| WatchRoot {
                root: root.to_owned(),
                directories: directories.into_iter().map(str::to_owned).collect(),
                files: files.into_iter().map(str::to_owned).collect(),
            })
            .collect(),
    };
    // Paths past the line's budget are left out, the longest directory first: a deep directory
    // shows the fewest entries, and a recursive platform follows it anyway.
    while serde_json::to_vec(&set).map_or(true, |line| line.len() >= MAXIMUM_WATCH_SET_BYTES) {
        let longest = set
            .roots
            .iter_mut()
            .flat_map(|root| root.directories.iter().chain(&root.files).map(String::len))
            .max();
        let Some(longest) = longest else { break };
        for root in &mut set.roots {
            root.directories.retain(|path| path.len() != longest);
            root.files.retain(|path| path.len() != longest);
        }
    }
    set
}

/// Brings the lane in line with the watches: starts it for the first one, hands it a changed set,
/// and leaves it to exit on its own once nothing is watched. A lane that has to start is handed the
/// set once it runs.
fn sync(shared: &Arc<Shared>, registry: &mut Registry) {
    let set = desired(registry);
    let names: BTreeSet<&str> = set.roots.iter().map(|root| root.root.as_str()).collect();
    registry
        .followed
        .retain(|root, _| names.contains(root.as_str()));
    for root in &set.roots {
        registry
            .followed
            .entry(root.root.clone())
            .or_insert(Followed {
                files: 1,
                repository: 1,
                state: State::Starting,
            });
    }
    if registry.sent.as_ref() == Some(&set) {
        return;
    }
    let Some(lane) = &registry.lane else {
        if !set.roots.is_empty() && !registry.starting {
            registry.starting = true;
            let starter = Arc::clone(shared);
            let spawned = std::thread::Builder::new()
                .name("orchbee-server-workspace-files-watch-start".into())
                .spawn(move || starts(&starter, None));
            if spawned.is_err() {
                registry.starting = false;
                unavailable(registry);
            }
        }
        return;
    };
    let Ok(mut line) = serde_json::to_vec(&set) else {
        return;
    };
    line.push(b'\n');
    if lane.connection.write_line(&line).is_ok() {
        registry.sent = Some(set);
    } else {
        // A lane that cannot take its set is shut down; its reader sees that and starts another.
        registry.sent = None;
        lane.connection.close();
    }
}

fn unavailable(registry: &mut Registry) {
    for followed in registry.followed.values_mut() {
        followed.state = State::Unavailable;
    }
}

/// Starts a lane, after `wait` when given, and starts again after a growing wait while each start
/// fails and any watch remains. The caller has marked the registry as starting.
fn starts(shared: &Arc<Shared>, mut wait: Option<Duration>) {
    loop {
        if let Some(wait) = wait {
            std::thread::sleep(wait);
        }
        {
            let mut registry = shared.lock();
            if registry.watches.is_empty() || registry.lane.is_some() {
                registry.starting = false;
                return;
            }
        }
        // The registry is not held while the service starts, so watches are answered meanwhile.
        let launched = shared.launcher.launch(WorkspaceFilesService::Watch);
        let mut registry = shared.lock();
        let installed = match launched {
            Ok(connection) if !registry.watches.is_empty() => {
                install(shared, &mut registry, connection)
            }
            // Nothing is watched any longer; the service ends with its connection.
            Ok(_unneeded) => {
                registry.starting = false;
                return;
            }
            Err(_) => false,
        };
        if installed {
            registry.starting = false;
            sync(shared, &mut registry);
            wait = None;
        } else {
            unavailable(&mut registry);
            wait = Some(grow(&mut registry));
        }
        drop(registry);
        shared.changes.notify_workspace_files();
        if wait.is_none() {
            return;
        }
    }
}

/// Makes `connection` the running lane, with a thread that reads its reports.
fn install(shared: &Arc<Shared>, registry: &mut Registry, connection: Connection) -> bool {
    let Ok(reports) = connection.reader() else {
        return false;
    };
    registry.incarnations += 1;
    let incarnation = registry.incarnations;
    let reader = Arc::clone(shared);
    let spawned = std::thread::Builder::new()
        .name("orchbee-server-workspace-files-watch".into())
        .spawn(move || read_reports(&reader, reports, incarnation));
    if spawned.is_err() {
        return false;
    }
    for followed in registry.followed.values_mut() {
        followed.state = State::Starting;
    }
    registry.lane = Some(Lane {
        connection,
        incarnation,
        started: Instant::now(),
    });
    registry.sent = None;
    true
}

/// Applies each report of one lane until it ends, then starts another after a wait while any
/// watch remains.
fn read_reports(shared: &Arc<Shared>, stdout: impl Read, incarnation: u64) {
    let mut stdout = BufReader::new(stdout);
    loop {
        let mut line = Vec::new();
        match (&mut stdout)
            .take(MAXIMUM_REPORT_LINE_BYTES)
            .read_until(b'\n', &mut line)
        {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let Ok(report) = serde_json::from_slice::<WatchEvent>(&line) else {
            continue;
        };
        let mut registry = shared.lock();
        if registry.lane.as_ref().map(|lane| lane.incarnation) != Some(incarnation) {
            return;
        }
        let changed = match report {
            WatchEvent::State { root, state } => {
                registry.followed.get_mut(&root).is_some_and(|followed| {
                    let changed = followed.state != state;
                    followed.state = state;
                    changed
                })
            }
            WatchEvent::Changed {
                root,
                files,
                repository,
            } => registry.followed.get_mut(&root).is_some_and(|followed| {
                if files {
                    followed.files = followed.files.wrapping_add(1).max(1);
                }
                if repository {
                    followed.repository = followed.repository.wrapping_add(1).max(1);
                }
                files || repository
            }),
        };
        drop(registry);
        if changed {
            shared.changes.notify_workspace_files();
        }
    }
    ended(shared, incarnation);
}

/// A lane ended: every directory reads unavailable, and while any watch remains another lane
/// starts after a wait that grows while lanes keep ending or failing to start quickly.
fn ended(shared: &Arc<Shared>, incarnation: u64) {
    let wait = {
        let mut registry = shared.lock();
        let Some(lane) = registry
            .lane
            .take_if(|lane| lane.incarnation == incarnation)
        else {
            // The lane was replaced or ended on purpose; whoever did that brought the rest in line.
            return;
        };
        let steady = lane.started.elapsed() >= STEADY;
        drop(lane);
        registry.sent = None;
        unavailable(&mut registry);
        if steady {
            registry.backoff = None;
        }
        if registry.watches.is_empty() || registry.starting {
            None
        } else {
            registry.starting = true;
            Some(grow(&mut registry))
        }
    };
    shared.changes.notify_workspace_files();
    if let Some(wait) = wait {
        starts(shared, Some(wait));
    }
}

/// The next wait before a restart: the first, or twice the last up to the longest.
fn grow(registry: &mut Registry) -> Duration {
    let wait = registry.backoff.map_or(FIRST_RESTART, |last| {
        last.saturating_mul(2).min(LAST_RESTART)
    });
    registry.backoff = Some(wait);
    wait
}
