//! The `files-history` side of the Git dispatch: the history reads, whose continuations name the
//! index the next page starts at, and the writes that move branches, HEAD and the stash, each a
//! [`Mutation::Branching`] under its request identity as the other writes are.

use super::{
    super::{
        blame::BlameQuery,
        branching::{Abort, Branching},
        engine::Mutation,
        history::{ChangesQuery, LogQuery},
        records,
    },
    Service, replying,
};
use crate::{calls::Call, scope::View};
use orchbee_files_protocol::{
    AbortOperation, GitAbort, GitBlame, GitBranchCreate, GitBranchDelete, GitBranchUpstream,
    GitChanges, GitCheckout, GitLog, GitMergeBase, GitName, GitNamed, GitRemoteAdd, GitRemoteRef,
    GitRevision, GitStashApply, GitStashDrop, GitStashPush, GitStashes, GitTagCreate,
    GitUndoCommit, RepositoryMutation,
};

impl Service {
    /// `git.log`.
    pub(crate) fn log(&self, call: Call, request: &GitLog) {
        let Some((git, call)) = self.engine(call) else {
            return;
        };
        let query = LogQuery {
            revisions: request
                .revisions
                .as_ref()
                .map_or_else(Vec::new, |revisions| {
                    revisions.as_slice().iter().map(name).collect()
                }),
            all: request.all,
            path: request.path.as_ref().map(|path| path.as_str().to_owned()),
            follow: request.follow,
            author: request
                .author
                .as_ref()
                .map(|author| author.as_str().to_owned()),
            grep: request.grep.as_ref().map(|grep| grep.as_str().to_owned()),
            statistics: request.statistics,
            // At most 1,000, which every target's `usize` holds.
            limit: usize::try_from(request.limit.get()).unwrap_or(usize::MAX),
            skip: request.cursor.map_or(0, records::page_index),
        };
        let cancel = call.cancellation();
        git.log(
            request.workspace_id,
            request.repository.as_str().to_owned(),
            query,
            cancel,
            replying(call, records::log),
        );
    }

    /// `git.blame`.
    pub(crate) fn blame(&self, call: Call, request: &GitBlame) {
        let Some((git, call)) = self.engine(call) else {
            return;
        };
        let query = BlameQuery {
            path: request.path.as_str().to_owned(),
            commit: request
                .commit
                .as_ref()
                .map(|commit| commit.as_str().to_owned()),
            ignore_whitespace: request.ignore_whitespace,
        };
        let cancel = call.cancellation();
        git.blame(
            request.workspace_id,
            request.repository.as_str().to_owned(),
            query,
            request.cursor.map_or(0, records::page_index),
            cancel,
            replying(call, records::blame),
        );
    }

    /// `git.changes`.
    pub(crate) fn changes(&self, call: Call, request: &GitChanges) {
        let Some((git, call)) = self.engine(call) else {
            return;
        };
        let query = ChangesQuery {
            from: request
                .from
                .as_ref()
                .map(|commit| commit.as_str().to_owned()),
            to: request.to.as_str().to_owned(),
        };
        let cancel = call.cancellation();
        git.changes(
            request.workspace_id,
            request.repository.as_str().to_owned(),
            query,
            request.cursor.map_or(0, records::page_index),
            cancel,
            replying(call, records::changes),
        );
    }

    /// `git.merge-base`.
    pub(crate) fn merge_base(&self, call: Call, request: &GitMergeBase) {
        let Some((git, call)) = self.engine(call) else {
            return;
        };
        let cancel = call.cancellation();
        git.merge_base(
            request.workspace_id,
            request.repository.as_str().to_owned(),
            name(&request.first),
            name(&request.second),
            cancel,
            replying(call, records::merge_base),
        );
    }

    /// `git.stashes`.
    pub(crate) fn stashes(&self, call: Call, request: &GitStashes) {
        let Some((git, call)) = self.engine(call) else {
            return;
        };
        let cancel = call.cancellation();
        git.stashes(
            request.workspace_id,
            request.repository.as_str().to_owned(),
            request.cursor.map_or(0, records::page_index),
            cancel,
            replying(call, records::stashes),
        );
    }

    /// `git.checkout`.
    pub(crate) fn checkout(&self, call: Call, view: View, request: &GitCheckout) {
        let mutation = Mutation::Branching(Branching::Checkout {
            revision: name(&request.revision),
            detached: request.detached,
            track: request.track,
        });
        self.mutate(
            call,
            view,
            request.workspace_id,
            request.request_id,
            &request.repository,
            mutation,
        );
    }

    /// `git.branch-create`.
    pub(crate) fn branch_create(&self, call: Call, view: View, request: &GitBranchCreate) {
        let mutation = Mutation::Branching(Branching::BranchCreate {
            name: name(&request.name),
            start: request.start.as_ref().map(name),
            checkout: request.checkout,
        });
        self.mutate(
            call,
            view,
            request.workspace_id,
            request.request_id,
            &request.repository,
            mutation,
        );
    }

    /// `git.branch-delete`.
    pub(crate) fn branch_delete(&self, call: Call, view: View, request: &GitBranchDelete) {
        let mutation = Mutation::Branching(Branching::BranchDelete {
            name: name(&request.name),
            force: request.force,
        });
        self.mutate(
            call,
            view,
            request.workspace_id,
            request.request_id,
            &request.repository,
            mutation,
        );
    }

    /// `git.branch-rename`.
    pub(crate) fn branch_rename(&self, call: Call, view: View, request: &GitNamed) {
        let mutation = Mutation::Branching(Branching::BranchRename {
            name: name(&request.name),
        });
        self.mutate(
            call,
            view,
            request.workspace_id,
            request.request_id,
            &request.repository,
            mutation,
        );
    }

    /// `git.branch-upstream`.
    pub(crate) fn branch_upstream(&self, call: Call, view: View, request: &GitBranchUpstream) {
        let mutation = Mutation::Branching(Branching::BranchUpstream {
            name: name(&request.name),
            upstream: name(&request.upstream),
        });
        self.mutate(
            call,
            view,
            request.workspace_id,
            request.request_id,
            &request.repository,
            mutation,
        );
    }

    /// `git.branch-delete-remote`.
    pub(crate) fn branch_delete_remote(&self, call: Call, view: View, request: &GitRemoteRef) {
        let mutation = Mutation::Branching(Branching::BranchDeleteRemote {
            remote: name(&request.remote),
            name: name(&request.name),
        });
        self.mutate(
            call,
            view,
            request.workspace_id,
            request.request_id,
            &request.repository,
            mutation,
        );
    }

    /// `git.merge`.
    pub(crate) fn merge(&self, call: Call, view: View, request: &GitRevision) {
        let mutation = Mutation::Branching(Branching::Merge {
            revision: name(&request.revision),
        });
        self.mutate(
            call,
            view,
            request.workspace_id,
            request.request_id,
            &request.repository,
            mutation,
        );
    }

    /// `git.rebase`.
    pub(crate) fn rebase(&self, call: Call, view: View, request: &GitRevision) {
        let mutation = Mutation::Branching(Branching::Rebase {
            revision: name(&request.revision),
        });
        self.mutate(
            call,
            view,
            request.workspace_id,
            request.request_id,
            &request.repository,
            mutation,
        );
    }

    /// `git.cherry-pick`.
    pub(crate) fn cherry_pick(&self, call: Call, view: View, request: &GitRevision) {
        let mutation = Mutation::Branching(Branching::CherryPick {
            revision: name(&request.revision),
        });
        self.mutate(
            call,
            view,
            request.workspace_id,
            request.request_id,
            &request.repository,
            mutation,
        );
    }

    /// `git.abort`.
    pub(crate) fn abort(&self, call: Call, view: View, request: &GitAbort) {
        let mutation = Mutation::Branching(Branching::Abort(match request.operation {
            AbortOperation::Merge => Abort::Merge,
            AbortOperation::Rebase => Abort::Rebase,
            AbortOperation::CherryPick => Abort::CherryPick,
        }));
        self.mutate(
            call,
            view,
            request.workspace_id,
            request.request_id,
            &request.repository,
            mutation,
        );
    }

    /// `git.undo-commit`.
    pub(crate) fn undo_commit(&self, call: Call, view: View, request: &GitUndoCommit) {
        let mutation = Mutation::Branching(Branching::UndoCommit {
            commit: request.commit.as_str().to_owned(),
        });
        self.mutate(
            call,
            view,
            request.workspace_id,
            request.request_id,
            &request.repository,
            mutation,
        );
    }

    /// `git.stash-push`.
    pub(crate) fn stash_push(&self, call: Call, view: View, request: &GitStashPush) {
        let mutation = Mutation::Branching(Branching::StashPush {
            untracked: request.untracked,
            staged: request.staged,
            message: request
                .message
                .as_ref()
                .map(|message| message.as_str().to_owned()),
        });
        self.mutate(
            call,
            view,
            request.workspace_id,
            request.request_id,
            &request.repository,
            mutation,
        );
    }

    /// `git.stash-apply`.
    pub(crate) fn stash_apply(&self, call: Call, view: View, request: &GitStashApply) {
        self.apply_stash(call, view, request, false);
    }

    /// `git.stash-pop`.
    pub(crate) fn stash_pop(&self, call: Call, view: View, request: &GitStashApply) {
        self.apply_stash(call, view, request, true);
    }

    /// `git.stash-drop`.
    pub(crate) fn stash_drop(&self, call: Call, view: View, request: &GitStashDrop) {
        let mutation = Mutation::Branching(Branching::StashDrop {
            index: request.index.get(),
            commit: request.commit.as_str().to_owned(),
        });
        self.mutate(
            call,
            view,
            request.workspace_id,
            request.request_id,
            &request.repository,
            mutation,
        );
    }

    /// `git.stash-clear`.
    pub(crate) fn stash_clear(&self, call: Call, view: View, request: &RepositoryMutation) {
        self.mutate(
            call,
            view,
            request.workspace_id,
            request.request_id,
            &request.repository,
            Mutation::Branching(Branching::StashClear),
        );
    }

    /// `git.tag-create`.
    pub(crate) fn tag_create(&self, call: Call, view: View, request: &GitTagCreate) {
        let mutation = Mutation::Branching(Branching::TagCreate {
            name: name(&request.name),
            message: request
                .message
                .as_ref()
                .map(|message| message.as_str().to_owned()),
            target: request.target.as_ref().map(name),
        });
        self.mutate(
            call,
            view,
            request.workspace_id,
            request.request_id,
            &request.repository,
            mutation,
        );
    }

    /// `git.tag-delete`.
    pub(crate) fn tag_delete(&self, call: Call, view: View, request: &GitNamed) {
        let mutation = Mutation::Branching(Branching::TagDelete {
            name: name(&request.name),
        });
        self.mutate(
            call,
            view,
            request.workspace_id,
            request.request_id,
            &request.repository,
            mutation,
        );
    }

    /// `git.tag-delete-remote`.
    pub(crate) fn tag_delete_remote(&self, call: Call, view: View, request: &GitRemoteRef) {
        let mutation = Mutation::Branching(Branching::TagDeleteRemote {
            remote: name(&request.remote),
            name: name(&request.name),
        });
        self.mutate(
            call,
            view,
            request.workspace_id,
            request.request_id,
            &request.repository,
            mutation,
        );
    }

    /// `git.remote-add`.
    pub(crate) fn remote_add(&self, call: Call, view: View, request: &GitRemoteAdd) {
        let mutation = Mutation::Branching(Branching::RemoteAdd {
            name: name(&request.name),
            url: name(&request.url),
        });
        self.mutate(
            call,
            view,
            request.workspace_id,
            request.request_id,
            &request.repository,
            mutation,
        );
    }

    /// `git.remote-remove`.
    pub(crate) fn remote_remove(&self, call: Call, view: View, request: &GitNamed) {
        let mutation = Mutation::Branching(Branching::RemoteRemove {
            name: name(&request.name),
        });
        self.mutate(
            call,
            view,
            request.workspace_id,
            request.request_id,
            &request.repository,
            mutation,
        );
    }

    /// `git.stash-apply`, or `git.stash-pop` when `pop` holds.
    fn apply_stash(&self, call: Call, view: View, request: &GitStashApply, pop: bool) {
        let mutation = Mutation::Branching(Branching::StashApply {
            index: request.index.get(),
            commit: request.commit.as_str().to_owned(),
            pop,
            staged: request.staged,
        });
        self.mutate(
            call,
            view,
            request.workspace_id,
            request.request_id,
            &request.repository,
            mutation,
        );
    }
}

fn name(name: &GitName) -> String {
    name.as_str().to_owned()
}
