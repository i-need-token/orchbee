//! Each Workspace's scope as the Server last stated it, the files each Client connection opened in
//! it, and the check a requested path passes when its operation starts. The Server alone decides
//! a Workspace's root, from the Workbench's current path; this process decides what lies inside.
//! A path is readable when it is lexically inside the root, spelled as the Workbench reported it
//! or as its real location, or inside a member the Git engine establishes: the working tree of a
//! repository consented to in that Workspace, or the file an open editor prompt names. It is also
//! readable when it is a file the requesting connection opened in that Workspace. Links inside are
//! followed, as VS Code follows them. A mutation's path passes [`View::mutable`] when its execution
//! starts: the real location it changes must also lie inside a member's real location, so a link
//! or junction inside cannot carry the change outside. An operation keeps the view it started
//! under, so a later root change never moves it. Opened files outlive their Workspace's close and
//! end with `fs.ungrant` or their connection; each successful `fs.grant` of a file adds one grant
//! of it, and the file stays opened while one remains.

pub(crate) mod path;

use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use orchbee_files_protocol::{
    ConnectionId, ErrorCode, FilePath, FilesError, GrantSource, Operation, Root, ScopeUpdate,
    Token, WorkspaceId, WorkspaceScope,
};

use self::path::NativePath;
use crate::error::from_io;

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
    /// The members the Git engine last stated.
    git: Vec<NativePath>,
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

/// What a mutation changes at its path, which decides whose real location must lie in scope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Change {
    /// The content of the file at the path, written through a link there, and the file created
    /// when nothing is there: saving, Save As and recreating a deleted file. The path's own real
    /// location must lie inside a member's; a file the connection opened is admitted wherever it
    /// lies, since the person chose it.
    Content,
    /// The entry at the path itself, never what a link there names: creating it, renaming or moving
    /// it (from and to), copying onto it, replacing it, deleting it or moving it to the trash. Its
    /// folder's real location must lie inside a member's, so a member itself is never such a
    /// path; an opened file admits nothing more.
    Entry,
}

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
                git: Vec::new(),
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

    /// Replaces the members the Git engine established in `workspace`, a Workspace the Server
    /// stated: its consented repositories' working trees and the files its open editor prompts
    /// name.
    pub(crate) fn set_git_members(&self, workspace: WorkspaceId, members: Vec<NativePath>) {
        if let Some(stated) = self.lock().workspaces.get_mut(&workspace) {
            stated.git = members;
        }
    }

    /// The files each connection holds open in `workspace`.
    pub(crate) fn opened_in(&self, workspace: WorkspaceId) -> Vec<(ConnectionId, NativePath)> {
        let state = self.lock();
        state
            .opened
            .iter()
            .filter(|((_, opened_in), _)| *opened_in == workspace)
            .flat_map(|((connection, _), files)| {
                files.values().map(|(path, _)| (*connection, path.clone()))
            })
            .collect()
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
        let members = match state.workspaces.get(&workspace) {
            Some(Workspace {
                stated: Stated::Root(root),
                git,
                ..
            }) => root
                .admitted
                .iter()
                .chain(&root.real)
                .chain(git)
                .cloned()
                .collect(),
            Some(Workspace {
                stated: Stated::NoRoot,
                git,
                ..
            }) => git.clone(),
            Some(Workspace {
                stated: Stated::Closed,
                ..
            }) if operation.after_close() => Vec::new(),
            Some(_) | None => return Err(FilesError::new(ErrorCode::Denied)),
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
                    ..
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
                    ..
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
    /// Admits `path` for reading: lexically inside the root or a member the Git engine
    /// established, or a file the connection opened.
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

    /// Admits `path` for a grant from `source`. A file picked in a tree or a diff is already in
    /// scope; a Save As destination, a terminal link, a restored tab or the operating system's
    /// open names a file the person chose explicitly, which may lie anywhere.
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
            // A Quick Open pick needs `files-search`, which is not served yet.
            GrantSource::Tree | GrantSource::Diff | GrantSource::QuickOpen => self.read(path),
            GrantSource::SaveAs
            | GrantSource::TerminalLink
            | GrantSource::Restored
            | GrantSource::OsOpen => NativePath::parse(path.as_str()),
        }
    }

    /// Admits `path` for a mutation that changes it as `change` says, checked each time a
    /// mutation of the operation starts. The path, or for [`Change::Entry`] its folder, must lie
    /// lexically inside a member, and its real location inside a member's real location: links
    /// and junctions on the way resolved, and on Windows 8.3 names and case, compared as
    /// [`path::real_within`] compares. A location where nothing is yet is where its nearest
    /// existing folder's real location leads, since nothing below that folder can redirect it. A
    /// file the connection opened is admitted for [`Change::Content`] without either check. It
    /// reads the filesystem and may block, so it runs where the mutation runs.
    ///
    /// # Errors
    ///
    /// - The error of a form this machine does not admit.
    /// - `outside-scope` when the path, or for [`Change::Entry`] its folder, is not lexically
    ///   inside a member; also for [`Change::Entry`] of a filesystem root.
    /// - `link-target-outside` when the real location lies outside every member's, or a link on
    ///   the way names nothing.
    /// - The code of the operating system's failure to read a real location, such as
    ///   `permission-denied` or `not-directory`.
    pub(crate) fn mutable(
        &self,
        path: &FilePath,
        change: Change,
    ) -> Result<NativePath, FilesError> {
        self.mutable_on(&System, path, change)
    }

    fn mutable_on(
        &self,
        filesystem: &impl Filesystem,
        path: &FilePath,
        change: Change,
    ) -> Result<NativePath, FilesError> {
        let path = NativePath::parse(path.as_str())?;
        if change == Change::Content && self.opened.contains_key(&path.key()) {
            return Ok(path);
        }
        let changed = match change {
            Change::Content => Some(path.clone()),
            Change::Entry => path.parent(),
        };
        let Some(changed) =
            changed.filter(|changed| self.members.iter().any(|member| changed.is_within(member)))
        else {
            return Err(FilesError::new(ErrorCode::OutsideScope));
        };
        let real = located(filesystem, &changed)?;
        // A member without a real location now admits nothing by it.
        let inside = self
            .members
            .iter()
            .filter_map(|member| located(filesystem, member).ok())
            .any(|member| path::real_within(&real, &member));
        if inside {
            Ok(path)
        } else {
            Err(FilesError::new(ErrorCode::LinkTargetOutside))
        }
    }
}

/// What the mutation check reads of the filesystem.
trait Filesystem {
    /// The real location of `path`, as [`std::fs::canonicalize`] reads it.
    fn real(&self, path: &Path) -> io::Result<PathBuf>;

    /// Whether an entry is at `path` itself, a link counting whatever it names.
    fn exists(&self, path: &Path) -> io::Result<bool>;
}

/// This machine's filesystem.
struct System;

impl Filesystem for System {
    fn real(&self, path: &Path) -> io::Result<PathBuf> {
        std::fs::canonicalize(path)
    }

    fn exists(&self, path: &Path) -> io::Result<bool> {
        match std::fs::symlink_metadata(path) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }
}

/// The real location of `path`; where nothing is at it, the real location of its nearest existing
/// folder with the missing segments below, which no link can redirect while nothing is there.
///
/// # Errors
///
/// `link-target-outside` for a link on the way that names nothing, since where a change through it
/// lands cannot be known, and the operating system's failure otherwise.
fn located(filesystem: &impl Filesystem, path: &NativePath) -> Result<PathBuf, FilesError> {
    let mut missing = Vec::new();
    let mut current = path.clone();
    loop {
        let error = match filesystem.real(current.as_path()) {
            Ok(mut real) => {
                real.extend(missing.iter().rev());
                return Ok(real);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => error,
            Err(error) => return Err(from_io(&error)),
        };
        if filesystem
            .exists(current.as_path())
            .map_err(|error| from_io(&error))?
        {
            return Err(FilesError::new(ErrorCode::LinkTargetOutside));
        }
        // A missing filesystem root leads nowhere.
        let parent = current.parent().ok_or_else(|| from_io(&error))?;
        missing.push(current.name().to_owned());
        current = parent;
    }
}

#[cfg(test)]
mod tests;
