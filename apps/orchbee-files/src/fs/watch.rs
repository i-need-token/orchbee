//! The file engine's watches and the delivery of their changes to `fs.subscribe`: what each Client
//! loaded, each Workspace's watch state, and the service that turns native events into Updates.
//!
//! Listing a folder loads it for the listing connection in that Workspace before reading it, as
//! does each folder of its single-child chain, and `fs.release` unloads it, as the frontend's model
//! backend does; a connection's end unloads everything it loaded. [`service`] decides what is
//! watched and delivered; the service thread `orchbee-files-watch` runs it on the inputs of every
//! other part, the native adapter's thread `orchbee-files-watcher` owns the one native watcher,
//! and filesystem checks run on the process's helpers, so the service never waits on a disk. The
//! service thread starts when the Git engine takes its repository watches or a subscription first
//! joins, the adapter's thread with the first watch, and both end with the drain.

mod coalesce;
mod native;
mod runner;
mod service;
mod stream;
mod throttle;

use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError, mpsc::Sender},
};

use orchbee_files_protocol::{ConnectionId, WatchState, WorkspaceId};

pub(crate) use self::service::Timing;
use self::{runner::Context, service::Native};
use crate::{
    blocking::Helpers,
    git::ports,
    scope::{RootFact, Scopes, path::NativePath},
    subscriptions::Subscriptions,
};

/// Folders one connection may load in one Workspace before every change inside the root reaches
/// it instead. A tree shows far fewer; the bound keeps a Client that never releases from growing
/// this process without limit.
const MAXIMUM_LOADED: usize = 16_384;

/// The watches of every Workspace and what each connection loaded.
pub(crate) struct Watching {
    context: Arc<Context>,
    timing: Timing,
    service: OnceLock<Option<Sender<Input>>>,
}

/// What reaches the service thread.
enum Input {
    /// The subscribers, loaded folders, opened files or scope of one Workspace, or of every one the
    /// service knows, may have changed.
    Changed(Option<WorkspaceId>),
    Native(Native),
    /// A Workspace's root fact; `None` when it could not be read.
    Root {
        workspace: WorkspaceId,
        fact: Option<RootFact>,
    },
    /// Whether each held file exists, as a directory or not.
    Resolved {
        workspace: WorkspaceId,
        files: Vec<(NativePath, Option<bool>)>,
    },
    /// Whether a missing path is a folder again, and which opened files in it exist.
    Polled {
        path: NativePath,
        exists: bool,
        files: Vec<NativePath>,
    },
    WatchRepository {
        workspace: WorkspaceId,
        tree: NativePath,
        git_dir: Option<NativePath>,
        report: ports::Report,
    },
    UnwatchRepository {
        workspace: WorkspaceId,
        tree: NativePath,
    },
    Stop,
}

/// The folders each connection loaded in each Workspace. A panic ends the process, so a poisoned
/// lock is never observed.
#[derive(Default)]
struct Loads {
    state: Mutex<HashMap<(ConnectionId, WorkspaceId), Loaded>>,
}

/// One connection's loaded folders in one Workspace.
#[derive(Clone, Default)]
struct Loaded {
    /// By [`NativePath::key`].
    folders: HashMap<String, NativePath>,
    /// It loaded more than [`MAXIMUM_LOADED`].
    everything: bool,
}

/// Each Workspace's watch state as its subscriptions last received it. Set under the
/// subscriptions' lock, so a Snapshot that reads it under the same lock is never older than an
/// Update its subscription receives.
#[derive(Default)]
struct States {
    state: Mutex<HashMap<WorkspaceId, WatchState>>,
}

impl Watching {
    pub(crate) fn new(
        helpers: Helpers,
        scopes: Arc<Scopes>,
        subscriptions: Arc<Subscriptions>,
        timing: Timing,
    ) -> Self {
        Self {
            context: Arc::new(Context {
                helpers,
                scopes,
                subscriptions,
                loads: Loads::default(),
                states: States::default(),
            }),
            timing,
            service: OnceLock::new(),
        }
    }

    /// Loads `path` for `connection` in `workspace`; true when it was not loaded.
    pub(crate) fn load(
        &self,
        connection: ConnectionId,
        workspace: WorkspaceId,
        path: &NativePath,
    ) -> bool {
        let loaded = self.context.loads.load(connection, workspace, path);
        if loaded {
            self.changed(Some(workspace));
        }
        loaded
    }

    /// Unloads each of `paths` for `connection` in `workspace`.
    pub(crate) fn unload(
        &self,
        connection: ConnectionId,
        workspace: WorkspaceId,
        paths: &[NativePath],
    ) {
        if self.context.loads.unload(connection, workspace, paths) {
            self.changed(Some(workspace));
        }
    }

    /// A subscription joined `workspace`, which starts the service.
    pub(crate) fn subscribed(&self, workspace: WorkspaceId) {
        if let Some(service) = self.service() {
            let _ = service.send(Input::Changed(Some(workspace)));
        }
    }

    /// The subscribers, opened files or scope of `workspace`, or of any Workspace, may have
    /// changed. Before the first subscription the service has nothing to update.
    pub(crate) fn changed(&self, workspace: Option<WorkspaceId>) {
        if let Some(Some(service)) = self.service.get() {
            let _ = service.send(Input::Changed(workspace));
        }
    }

    /// Unloads everything `connection` loaded.
    pub(crate) fn connection_lost(&self, connection: ConnectionId) {
        self.context.loads.connection_lost(connection);
        self.changed(None);
    }

    /// `workspace`'s watch state for a new subscription's Snapshot.
    pub(crate) fn state(&self, workspace: WorkspaceId) -> WatchState {
        self.context.states.get(workspace)
    }

    /// The watches of displayed repositories, for the Git engine.
    pub(crate) fn repositories(&self) -> RepositoryWatches {
        RepositoryWatches(self.service().cloned())
    }

    /// Stops every watch and the service.
    pub(crate) fn drain(&self) {
        if let Some(Some(service)) = self.service.get() {
            let _ = service.send(Input::Stop);
        }
    }

    fn service(&self) -> Option<&Sender<Input>> {
        self.service
            .get_or_init(|| runner::start(Arc::clone(&self.context), self.timing))
            .as_ref()
    }
}

impl Drop for Watching {
    fn drop(&mut self) {
        self.drain();
    }
}

/// [`ports::Watches`] over the file engine's watches.
pub(crate) struct RepositoryWatches(Option<Sender<Input>>);

impl ports::Watches for RepositoryWatches {
    fn watch(
        &mut self,
        workspace: WorkspaceId,
        repository: &str,
        git_dir: &Path,
        report: ports::Report,
    ) {
        let (Some(service), Ok(tree)) = (&self.0, NativePath::parse(repository)) else {
            return;
        };
        let git_dir = git_dir
            .to_str()
            .and_then(|git_dir| NativePath::parse(git_dir).ok());
        let _ = service.send(Input::WatchRepository {
            workspace,
            tree,
            git_dir,
            report,
        });
    }

    fn unwatch(&mut self, workspace: WorkspaceId, repository: &str) {
        if let (Some(service), Ok(tree)) = (&self.0, NativePath::parse(repository)) {
            let _ = service.send(Input::UnwatchRepository { workspace, tree });
        }
    }
}

impl Loads {
    fn load(&self, connection: ConnectionId, workspace: WorkspaceId, path: &NativePath) -> bool {
        let mut state = self.lock();
        let loaded = state.entry((connection, workspace)).or_default();
        if loaded.everything || loaded.folders.contains_key(&path.key()) {
            return false;
        }
        if loaded.folders.len() < MAXIMUM_LOADED {
            loaded.folders.insert(path.key(), path.clone());
        } else {
            loaded.everything = true;
        }
        true
    }

    fn unload(
        &self,
        connection: ConnectionId,
        workspace: WorkspaceId,
        paths: &[NativePath],
    ) -> bool {
        let mut state = self.lock();
        let Some(loaded) = state.get_mut(&(connection, workspace)) else {
            return false;
        };
        let mut unloaded = false;
        for path in paths {
            unloaded |= loaded.folders.remove(&path.key()).is_some();
        }
        if loaded.folders.is_empty() && !loaded.everything {
            state.remove(&(connection, workspace));
        }
        unloaded
    }

    fn connection_lost(&self, connection: ConnectionId) {
        self.lock().retain(|(holder, _), _| *holder != connection);
    }

    /// What each of `connections` loaded in `workspace`.
    fn of(
        &self,
        workspace: WorkspaceId,
        connections: &[ConnectionId],
    ) -> HashMap<ConnectionId, Loaded> {
        let state = self.lock();
        connections
            .iter()
            .filter_map(|connection| {
                let loaded = state.get(&(*connection, workspace))?;
                Some((*connection, loaded.clone()))
            })
            .collect()
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<(ConnectionId, WorkspaceId), Loaded>> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl States {
    fn get(&self, workspace: WorkspaceId) -> WatchState {
        self.lock()
            .get(&workspace)
            .copied()
            .unwrap_or(WatchState::Full)
    }

    fn set(&self, workspace: WorkspaceId, state: WatchState) {
        self.lock().insert(workspace, state);
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<WorkspaceId, WatchState>> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Whether `path` lies in a folder that a recursive watch of `tree` leaves out, as VS Code's
/// default `files.watcherExclude` does: `.git/objects`, `.git/subtree-cache` and `.hg/store`, at
/// the tree's top and one level down.
pub(super) fn excluded(tree: &NativePath, path: &NativePath) -> bool {
    const EXCLUDED: [[&str; 2]; 3] = [
        [".git", "objects"],
        [".git", "subtree-cache"],
        [".hg", "store"],
    ];
    let Some(below) = path.below(tree) else {
        return false;
    };
    let below: Vec<&str> = below.take(3).collect();
    let starts = |segments: &[&str]| {
        EXCLUDED
            .iter()
            .any(|excluded| segments.get(..2) == Some(excluded.as_slice()))
    };
    starts(&below) || below.get(1..).is_some_and(starts)
}

#[cfg(test)]
mod tests;
