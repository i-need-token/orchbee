//! What the Git engine needs from the file engine, which owns this process's file watches and the
//! machine's trash and implements [`Watches`] and [`Trash`] for it. Until it does, [`NoWatches`]
//! and [`NoTrash`] stand in: status then refreshes only when a Client subscribes, after a write,
//! on Agent completion and on an explicit refresh, and a clean to the trash answers
//! `trash-unavailable`, after which the Client may ask for permanent removal.

use orchbee_files_protocol::WorkspaceId;
use std::{path::Path, sync::Arc};

/// Where a reported change happened, which decides whether a repository past the status limit
/// refreshes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Change {
    /// The repository's toplevel itself or its Git directory itself was created, deleted or
    /// renamed. Such a change refreshes even a repository past the status limit.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the file engine's watch adapter reports it")
    )]
    Repository,
    /// Anything else under the working tree or in the first level of the Git directory, and a
    /// rescan after events were dropped.
    Content,
}

/// Where one watched repository's changes go. Clones report to the same place, from any thread;
/// once the repository is no longer watched, its reports are ignored.
#[derive(Clone)]
pub(crate) struct Report(Arc<dyn Fn(Change) + Send + Sync>);

impl Report {
    pub(crate) fn new(report: impl Fn(Change) + Send + Sync + 'static) -> Self {
        Self(Arc::new(report))
    }

    /// Reports a change. The engine waits out a 1,000 ms debounce of its own, so one report per
    /// delivery window is enough: [`Change::Repository`] when any event of the window was one,
    /// else [`Change::Content`].
    #[expect(dead_code, reason = "the file engine's watch adapter calls it")]
    pub(crate) fn changed(&self, change: Change) {
        (self.0)(change);
    }
}

/// The watches of displayed repositories.
///
/// The engine calls both methods on its own thread, so neither may block for long: a walk or a
/// retry runs elsewhere. Each `watch` pairs with one later `unwatch` of the same Workspace and
/// repository; one repository may be watched for several Workspaces at once, each pairing on its
/// own, so watches are counted per path. When the display set changes, every new `watch` comes
/// before the `unwatch` calls it replaces, so a path both share keeps its watch.
pub(crate) trait Watches: Send {
    /// Starts reporting the changes of `repository`, the canonical toplevel in native spelling, for
    /// `workspace`:
    ///
    /// - The working tree, watched as a Workspace root is watched: recursively, with the same
    ///   exclusions, fallbacks and limits, sharing a watch that already covers it. A filesystem
    ///   root, a drive root or the home directory gets no recursive watch. Events inside a `.git`
    ///   directory or file at any depth are not working-tree changes.
    /// - The first level of `git_dir`, which may lie outside the working tree, watched without
    ///   recursion. Events of `index.lock` and of `.watchman-cookie-*` names are dropped: Git's
    ///   own writes produce them and the change that follows reports the write.
    fn watch(&mut self, workspace: WorkspaceId, repository: &str, git_dir: &Path, report: Report);

    /// Stops the watch that `watch` started for the same Workspace and repository.
    fn unwatch(&mut self, workspace: WorkspaceId, repository: &str);
}

/// The watches of a build without a watch adapter: none.
pub(crate) struct NoWatches;

impl Watches for NoWatches {
    fn watch(&mut self, _: WorkspaceId, _: &str, _: &Path, _: Report) {}
    fn unwatch(&mut self, _: WorkspaceId, _: &str) {}
}

/// Why the machine's trash did not take a path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TrashError {
    /// The machine has no trash for the path's volume, which `trash-unavailable` reports.
    Unavailable,
    /// The trash refused or failed the move, which `trash-failed` reports.
    #[expect(dead_code, reason = "the file engine's trash adapter returns it")]
    Failed,
}

/// The trash of the machine this process runs on.
///
/// `git.clean` moves untracked paths of one repository, up to five at once from threads of its
/// own; each path is absolute and native, lies in the repository's working tree, and may be a file,
/// a link, which moves itself and never its target, or a folder, such as an untracked nested
/// repository. The trash never falls back to permanent deletion: only the Client asks for that,
/// after this answers an error.
pub(crate) trait Trash: Send + Sync {
    fn trash(&self, path: &Path) -> Result<(), TrashError>;
}

/// The trash of a build without a trash adapter: every path asks for permanent removal.
pub(crate) struct NoTrash;

impl Trash for NoTrash {
    fn trash(&self, _: &Path) -> Result<(), TrashError> {
        Err(TrashError::Unavailable)
    }
}
