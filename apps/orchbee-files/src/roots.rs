//! `fs.subscribe`'s root facts: the Snapshot each subscription starts with, and the `root` Update
//! every subscription of a Workspace receives after the Server changes that Workspace's scope.
//! The Snapshot reports the Workspace's watch state as `watch` last delivered it. A fact is read
//! under one statement of the scope and delivered only while that statement is current, so a slow
//! read never overwrites a newer root.

use orchbee_files_protocol::{
    FilesNotice, FsSnapshot, FsUpdate, Operation, WatchState, WorkspaceId,
};

use crate::{
    blocking::Helpers,
    calls::Call,
    scope::{RootFact, Scopes, path},
    subscriptions::Subscriptions,
};

/// Opens `call` as a subscription to `workspace`'s root facts and its watch state, which `watch`
/// reads. It reads the filesystem, so it runs on the call's own thread.
pub(crate) fn subscribe(
    scopes: &Scopes,
    helpers: &Helpers,
    subscriptions: &Subscriptions,
    workspace: WorkspaceId,
    mut call: Call,
    watch: impl Fn() -> WatchState,
) {
    loop {
        let RootFact { generation, root } = read(scopes, helpers, workspace);
        let state = watch();
        let snapshot = FilesNotice::FsSnapshot(FsSnapshot {
            root,
            os: path::OPERATING_SYSTEM,
            separator: path::SEPARATOR,
            case_sensitive: path::CASE_SENSITIVE,
            watch: state,
        });
        match subscriptions.join(workspace, call, &snapshot, || {
            scopes.generation(workspace) == generation && watch() == state
        }) {
            Ok(()) => return,
            // The Server changed the scope, or the watch state changed, while the root was read.
            Err(unanswered) => call = unanswered,
        }
    }
}

/// Sends `workspace`'s current root to each of its subscriptions after a change of its scope. It
/// reads the filesystem, so it runs on a thread of its own.
pub(crate) fn refresh(
    scopes: &Scopes,
    helpers: &Helpers,
    subscriptions: &Subscriptions,
    workspace: WorkspaceId,
) {
    let RootFact { generation, root } = read(scopes, helpers, workspace);
    let update = FilesNotice::FsUpdate(FsUpdate::Root { root });
    // A later change has its own refresh, which delivers the newer root.
    subscriptions.deliver(workspace, &update, || {
        scopes.generation(workspace) == generation
    });
}

fn read(scopes: &Scopes, helpers: &Helpers, workspace: WorkspaceId) -> RootFact {
    scopes.root(workspace, |root| {
        helpers.run(Operation::FsSubscribe, move || path::real(&root))
    })
}
