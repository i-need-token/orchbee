//! The `files-history` side of the Git dispatch: the history reads, whose continuations name the
//! index the next page starts at; the writes that move branches, HEAD and the stash, each a
//! [`Mutation::Branching`] under its request identity as the other writes are; and a clone, whose
//! folder is named here, in the folder the picker granted.

use super::{
    super::{
        blame::BlameQuery,
        branching::{Abort, Branching},
        engine::Mutation,
        history::{ChangesQuery, LogQuery},
        records,
        remote::Cloning,
    },
    Service, caller, replying,
};
use crate::{
    calls::Call,
    scope::{View, path::NativePath},
};
use orchbee_files_protocol::{
    AbortOperation, GitAbort, GitBlame, GitBranchCreate, GitBranchDelete, GitBranchUpstream,
    GitChanges, GitCheckout, GitClone, GitLog, GitMergeBase, GitName, GitNamed, GitRemoteAdd,
    GitRemoteRef, GitRevision, GitStashApply, GitStashDrop, GitStashPush, GitStashes, GitTagCreate,
    GitUndoCommit, MAXIMUM_PATH_BYTES, RepositoryMutation,
};
use std::{borrow::Cow, fs, path::MAIN_SEPARATOR};

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

    /// `git.clone` into `parent`, which the picker granted. Naming the clone's folder reads which
    /// names `parent` holds, so it runs on a thread of its own.
    pub(crate) fn clone_repository(
        &self,
        call: Call,
        view: View,
        request: &GitClone,
        parent: &NativePath,
    ) {
        let Some((git, call)) = self.engine(call) else {
            return;
        };
        let url = request.url.as_str();
        let cloning = Cloning {
            parent: parent.as_str().to_owned(),
            target: destination(parent, url),
            url: url.to_owned(),
            recursive: request.recursive,
        };
        let caller = caller(&call, view, request.workspace_id, request.request_id);
        git.clone_repository(caller, cloning, replying(call, records::done));
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

/// The folder of `parent` a clone of `url` fills, named as VS Code names it: the URL's last
/// segment, its escapes decoded as `decodeURI` decodes them, without a trailing `.git`, or
/// `repository` when that leaves nothing or a name this machine cannot hold in a folder, where VS
/// Code would fail. While something of that name exists, `-1` up to `-19` is appended, and Git
/// refuses the last of those unless it is an empty folder.
fn destination(parent: &NativePath, url: &str) -> String {
    let decoded = decode_uri(url);
    let last = decoded
        .trim_end_matches('/')
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default();
    let named = last.strip_suffix(".git").unwrap_or(last);
    let holdable = !named.is_empty()
        && !named.chars().any(char::is_control)
        && NativePath::parse(&joined(parent, named))
            .is_ok_and(|path| path.as_str().len() <= MAXIMUM_PATH_BYTES);
    let base = if holdable { named } else { "repository" };
    let mut target = joined(parent, base);
    for count in 1..20 {
        if fs::symlink_metadata(&target).is_err() {
            break;
        }
        target = joined(parent, &format!("{base}-{count}"));
    }
    target
}

/// `name` in `parent`.
fn joined(parent: &NativePath, name: &str) -> String {
    if parent.parent().is_some() {
        format!("{}{MAIN_SEPARATOR}{name}", parent.as_str())
    } else {
        format!("{}{name}", parent.as_str())
    }
}

/// `url` with its escapes decoded as JavaScript's `decodeURI` decodes them, which keeps the escape
/// of each character a URI reserves; a URL whose escapes do not decode to UTF-8 stays as it is,
/// where `decodeURI` throws.
fn decode_uri(url: &str) -> Cow<'_, str> {
    if !url.contains('%') {
        return Cow::Borrowed(url);
    }
    let bytes = url.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while let Some(&byte) = bytes.get(index) {
        let escaped = bytes
            .get(index + 1..index + 3)
            .filter(|digits| byte == b'%' && digits.iter().all(u8::is_ascii_hexdigit))
            .and_then(|digits| std::str::from_utf8(digits).ok())
            .and_then(|digits| u8::from_str_radix(digits, 16).ok())
            .filter(|value| !b";/?:@&=+$,#".contains(value));
        if let Some(value) = escaped {
            decoded.push(value);
            index += 3;
        } else {
            decoded.push(byte);
            index += 1;
        }
    }
    String::from_utf8(decoded).map_or(Cow::Borrowed(url), Cow::Owned)
}
