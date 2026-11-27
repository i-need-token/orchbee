//! Each Workspace's scope as the Server last stated it, the files each Client connection opened in
//! it, and the check a requested path passes when its operation starts. The Server alone decides
//! a Workspace's root, from the Workbench's current path; this process decides what lies inside.
//! A path is readable when it is lexically inside the root, spelled as the Workbench reported it
//! or as its real location, or when it is a file the requesting connection opened in that
//! Workspace; links inside are followed, as VS Code follows them. An operation keeps the view it
//! started under, so a later root change never moves it. Opened files outlive their Workspace's
//! close and end with `fs.ungrant` or their connection; each successful `fs.grant` of a file adds
//! one grant of it, and the file stays opened while one remains.

pub(crate) mod path;

use std::{
    collections::HashMap,
    io,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use orchbee_files_protocol::{
    ConnectionId, ErrorCode, FilePath, FilesError, GrantSource, Operation, Root, ScopeUpdate,
    Token, WorkspaceId, WorkspaceScope,
};

use self::path::NativePath;

/// How many distinct files one connection may hold open in one Workspace. A tab bar holds far
/// fewer; the bound keeps a Client that never ungrants from growing this process without limit.
const MAXIMUM_OPENED: usize = 4096;

/// The scopes of every Workspace the Server stated. A panic ends the process, so a poisoned lock
/// is never observed.
pub(crate) struct Scopes {
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    workspaces: HashMap<WorkspaceId, Workspace>,
    opened: HashMap<(ConnectionId, WorkspaceId), Arc<OpenedFiles>>,
}

struct Workspace {
    /// Counts the Server's changes to this scope, so a fact read under an older one is known stale.
    generation: u64,
    stated: Stated,
}

enum Stated {
    Root(StatedRoot),
    NoRoot,
    Closed,
}

struct StatedRoot {
    /// As the Workbench reported it.
    reported: String,
    /// The same path when its form is one this machine admits; a UNC root admits nothing.
    admitted: Option<NativePath>,
    /// Its real location, once a root fact read it under this statement.
    real: Option<NativePath>,
}

/// One connection's opened files in one Workspace, by [`NativePath::key`], each with the number
/// of grants not yet released.
type OpenedFiles = HashMap<String, (NativePath, u32)>;

/// The scope one operation starts under: what its paths may name until it ends.
pub(crate) struct View {
    workspace: WorkspaceId,
    connection: ConnectionId,
    members: Vec<NativePath>,
    opened: Arc<OpenedFiles>,
}

/// A Workspace's root fact and the statement it was read under.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct RootFact {
    /// [`Scopes::generation`] when the fact was read.
    pub(crate) generation: u64,
    /// The root as `fs.subscribe` reports it.
    pub(crate) root: Root,
}

impl Scopes {
    pub(crate) fn new() -> Self {
        Self {
            state: Mutex::new(State::default()),
        }
    }

    /// Applies the Server's statement of one Workspace's scope; true when it changed the scope.
    pub(crate) fn update(&self, update: ScopeUpdate) -> bool {
        let stated = match update.scope {
            WorkspaceScope::Root(reported) => Stated::Root(StatedRoot {
                admitted: NativePath::parse(&reported).ok(),
                reported,
                real: None,
            }),
            WorkspaceScope::NoRoot => Stated::NoRoot,
            WorkspaceScope::Closed => Stated::Closed,
        };
        let mut state = self.lock();
        let workspace = state
            .workspaces
            .entry(update.workspace)
            .or_insert(Workspace {
                generation: 0,
                stated: Stated::Closed,
            });
        // A root restated as it was keeps the real location already read.
        if workspace.generation > 0 && same_statement(&workspace.stated, &stated) {
            return false;
        }
        workspace.generation += 1;
        workspace.stated = stated;
        true
    }

    /// The number of the Server's changes to `workspace`'s scope so far; 0 before its first.
    pub(crate) fn generation(&self, workspace: WorkspaceId) -> u64 {
        self.lock()
            .workspaces
            .get(&workspace)
            .map_or(0, |workspace| workspace.generation)
    }

    /// Releases every file `connection` opened, in every Workspace.
    pub(crate) fn connection_lost(&self, connection: ConnectionId) {
        self.lock()
            .opened
            .retain(|(holder, _), _| *holder != connection);
    }

    /// The scope a call of `operation` from `connection` starts under.
    ///
    /// # Errors
    ///
    /// `denied` for a Workspace the Server never stated, and for one that closed when the
    /// operation may not name a closed Workspace.
    pub(crate) fn view(
        &self,
        workspace: WorkspaceId,
        connection: ConnectionId,
        operation: Operation,
    ) -> Result<View, FilesError> {
        let state = self.lock();
        let members = match state
            .workspaces
            .get(&workspace)
            .map(|stated| &stated.stated)
        {
            Some(Stated::Root(root)) => root.admitted.iter().chain(&root.real).cloned().collect(),
            Some(Stated::NoRoot) => Vec::new(),
            Some(Stated::Closed) if operation.after_close() => Vec::new(),
            Some(Stated::Closed) | None => return Err(FilesError::new(ErrorCode::Denied)),
        };
        Ok(View {
            workspace,
            connection,
            members,
            opened: state
                .opened
                .get(&(connection, workspace))
                .cloned()
                .unwrap_or_default(),
        })
    }

    /// Records `path`, which `view` admitted with [`View::grantable`], as one more grant of a file
    /// `view`'s connection opened in its Workspace, unless `cancelled` finds the granting call
    /// cancelled, which returns `None`. It decides under the lock that a connection's loss takes
    /// after cancelling the connection's calls, so no grant outlives its connection.
    ///
    /// # Errors
    ///
    /// `unavailable` with reason `too-many-grants` beyond the bound on distinct opened files.
    pub(crate) fn grant(
        &self,
        view: &View,
        path: NativePath,
        cancelled: impl FnOnce() -> bool,
    ) -> Option<Result<(), FilesError>> {
        let mut state = self.lock();
        if cancelled() {
            return None;
        }
        let opened = Arc::make_mut(
            state
                .opened
                .entry((view.connection, view.workspace))
                .or_default(),
        );
        let key = path.key();
        if let Some((_, count)) = opened.get_mut(&key) {
            *count = count.saturating_add(1);
        } else if opened.len() < MAXIMUM_OPENED {
            opened.insert(key, (path, 1));
        } else {
            let reason = Token::new("too-many-grants").expect("the reason is a token");
            return Some(Err(FilesError::unavailable(reason)));
        }
        Some(Ok(()))
    }

    /// Releases one grant of each of `paths` that `view`'s connection holds in its Workspace. A
    /// path it holds no grant of changes nothing.
    pub(crate) fn ungrant(&self, view: &View, paths: &[FilePath]) {
        let holder = (view.connection, view.workspace);
        let mut state = self.lock();
        let Some(opened) = state.opened.get_mut(&holder) else {
            return;
        };
        let opened = Arc::make_mut(opened);
        for path in paths {
            let Ok(path) = NativePath::parse(path.as_str()) else {
                continue;
            };
            let key = path.key();
            if let Some((_, count)) = opened.get_mut(&key) {
                *count -= 1;
                if *count == 0 {
                    opened.remove(&key);
                }
            }
        }
        if opened.is_empty() {
            state.opened.remove(&holder);
        }
    }

    /// `workspace`'s root as `fs.subscribe` reports it, reading the root's real location with
    /// `resolve` outside the lock, since it can block on the filesystem. A real location read
    /// under the current statement becomes a spelling of the root that requests may use.
    pub(crate) fn root(
        &self,
        workspace: WorkspaceId,
        resolve: impl FnOnce(NativePath) -> Result<io::Result<NativePath>, FilesError>,
    ) -> RootFact {
        let (generation, reported, admitted) = {
            let state = self.lock();
            match state.workspaces.get(&workspace) {
                Some(Workspace {
                    generation,
                    stated: Stated::Root(root),
                }) => (*generation, root.reported.clone(), root.admitted.clone()),
                current => {
                    return RootFact {
                        generation: current.map_or(0, |current| current.generation),
                        root: Root::None,
                    };
                }
            }
        };
        let fact = |root| RootFact { generation, root };
        // The Server bounds a root to a valid path; a root this machine cannot spell is reported
        // as stated, and every request under it is refused.
        let Ok(stated) = FilePath::new(reported) else {
            return fact(Root::None);
        };
        let Some(admitted) = admitted else {
            return fact(Root::Present(stated));
        };
        match resolve(admitted) {
            Ok(Ok(real)) => {
                let Ok(present) = FilePath::new(real.as_str()) else {
                    return fact(Root::Present(stated));
                };
                let mut state = self.lock();
                if let Some(Workspace {
                    generation: current,
                    stated: Stated::Root(root),
                }) = state.workspaces.get_mut(&workspace)
                    && *current == generation
                {
                    root.real = Some(real);
                }
                fact(Root::Present(present))
            }
            Ok(Err(error))
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
                ) =>
            {
                fact(Root::Missing(stated))
            }
            // A root that exists but cannot be resolved now, or a filesystem that does not answer,
            // is reported as stated; its operations report their own failures.
            Ok(Err(_)) | Err(_) => fact(Root::Present(stated)),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Whether two statements set the same scope, ignoring the real location read under the first.
fn same_statement(current: &Stated, stated: &Stated) -> bool {
    match (current, stated) {
        (Stated::Root(current), Stated::Root(stated)) => current.reported == stated.reported,
        (Stated::NoRoot, Stated::NoRoot) | (Stated::Closed, Stated::Closed) => true,
        _ => false,
    }
}

impl View {
    /// Admits `path` for reading: lexically inside the root, or a file the connection opened.
    ///
    /// # Errors
    ///
    /// The error of a form this machine does not admit, and `outside-scope` otherwise.
    pub(crate) fn read(&self, path: &FilePath) -> Result<NativePath, FilesError> {
        let path = NativePath::parse(path.as_str())?;
        if self.members.iter().any(|member| path.is_within(member))
            || self.opened.contains_key(&path.key())
        {
            Ok(path)
        } else {
            Err(FilesError::new(ErrorCode::OutsideScope))
        }
    }

    /// Admits `path` for a grant from `source`. A file picked in a tree, a diff or a Save As
    /// dialog is already in scope; a terminal link, a restored tab or the operating system's open
    /// names a file the person chose explicitly, which may lie anywhere.
    ///
    /// # Errors
    ///
    /// As [`Self::read`] for a source that needs the path in scope; otherwise only the error of a
    /// form this machine does not admit.
    pub(crate) fn grantable(
        &self,
        path: &FilePath,
        source: GrantSource,
    ) -> Result<NativePath, FilesError> {
        match source {
            GrantSource::Tree | GrantSource::Diff | GrantSource::SaveAs => self.read(path),
            GrantSource::TerminalLink | GrantSource::Restored | GrantSource::OsOpen => {
                NativePath::parse(path.as_str())
            }
        }
    }
}

#[cfg(test)]
mod tests;
