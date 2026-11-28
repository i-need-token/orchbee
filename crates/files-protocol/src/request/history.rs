//! The requests of the `files-history` operations: Local History, and the Git reads and writes of
//! VS Code's Source Control Graph, Timeline and blame and of its branch, merge, rebase,
//! cherry-pick, stash, tag and remote commands, each with the arguments VS Code's Git extension
//! passes at ef5ca45. Values are validated here; whether a path lies inside a Workspace's scope,
//! and whether a revision names anything, is decided by `orchbee-files` when the operation starts.

use serde::{Deserialize, Serialize};

use crate::{
    identity::{HistoryEntryId, RequestId, WorkspaceId},
    value::{
        CommitName, FilePath, GitName, Integer, InvalidValue, List, PageCursor, SafeInteger, Text,
        is_false, some,
    },
};

/// How many commits one `git.log` page asks for: 1 to 1,000.
pub type LogLimit = Integer<1, 1000>;

/// A pattern `git.log` matches authors or messages against, at most 1 KiB.
pub type LogPattern = Text<1, 1024>;

/// `git.log`: one page of commits, walked from named revisions as VS Code's Graph walks them, or
/// from HEAD as its Timeline does.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitLog {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The repository.
    pub repository: FilePath,
    /// The revisions to walk from, 1 to 256; absent walks HEAD unless `all` holds.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub revisions: Option<List<GitName, 1, 256>>,
    /// Also walk every branch, remote branch and tag.
    #[serde(default, skip_serializing_if = "is_false")]
    pub all: bool,
    /// List only the commits that change this file or folder of the repository; its toplevel
    /// lists every commit.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub path: Option<FilePath>,
    /// Follow the file at `path` through renames and report its path in each commit.
    #[serde(default, skip_serializing_if = "is_false")]
    pub follow: bool,
    /// List only the commits whose author matches this pattern.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub author: Option<LogPattern>,
    /// List only the commits whose message matches this extended regular expression, ignoring
    /// case.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub grep: Option<LogPattern>,
    /// Report how many files and lines each commit changed.
    #[serde(default, skip_serializing_if = "is_false")]
    pub statistics: bool,
    /// The most commits the page holds.
    pub limit: LogLimit,
    /// The previous page's continuation; absent for the first page.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub cursor: Option<PageCursor>,
}

/// `git.blame`: which commit last changed each line of a file.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitBlame {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The repository.
    pub repository: FilePath,
    /// The file.
    pub path: FilePath,
    /// The commit whose version is blamed; absent blames the working tree.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub commit: Option<CommitName>,
    /// Ignore whitespace changes.
    #[serde(default, skip_serializing_if = "is_false")]
    pub ignore_whitespace: bool,
    /// The previous page's continuation; absent for the first page.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub cursor: Option<PageCursor>,
}

/// `git.changes`: what changed from one commit to another.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitChanges {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The repository.
    pub repository: FilePath,
    /// The earlier commit, whose merge base with `to` the changes start from; absent lists
    /// everything `to` holds.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub from: Option<CommitName>,
    /// The later commit.
    pub to: CommitName,
    /// The previous page's continuation; absent for the first page.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub cursor: Option<PageCursor>,
}

/// `git.merge-base`: the best common ancestor of two revisions.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitMergeBase {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The repository.
    pub repository: FilePath,
    /// One revision.
    pub first: GitName,
    /// The other.
    pub second: GitName,
}

/// `git.stashes`: one page of the stash list.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitStashes {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The repository.
    pub repository: FilePath,
    /// The previous page's continuation; absent for the first page.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub cursor: Option<PageCursor>,
}

/// A mutation of a repository that names nothing else: `git.stash-clear`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepositoryMutation {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation's identity.
    pub request_id: RequestId,
    /// The repository.
    pub repository: FilePath,
}

/// A mutation that names one branch, tag or remote: `git.branch-rename`, which renames the current
/// branch to it, `git.tag-delete` and `git.remote-remove`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitNamed {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation's identity.
    pub request_id: RequestId,
    /// The repository.
    pub repository: FilePath,
    /// The name.
    pub name: GitName,
}

/// A mutation that takes one revision into the current branch: `git.merge`, `git.rebase` and
/// `git.cherry-pick`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitRevision {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation's identity.
    pub request_id: RequestId,
    /// The repository.
    pub repository: FilePath,
    /// The revision.
    pub revision: GitName,
}

/// A mutation that deletes a branch or a tag on a remote: `git.branch-delete-remote` and
/// `git.tag-delete-remote`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitRemoteRef {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation's identity.
    pub request_id: RequestId,
    /// The repository.
    pub repository: FilePath,
    /// The remote.
    pub remote: GitName,
    /// The branch or tag, without its `refs/heads/` or `refs/tags/` prefix.
    pub name: GitName,
}

/// `git.checkout`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitCheckout {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation's identity.
    pub request_id: RequestId,
    /// The repository.
    pub repository: FilePath,
    /// A branch, a remote branch or a commit.
    pub revision: GitName,
    /// Check out its commit with HEAD detached.
    #[serde(default, skip_serializing_if = "is_false")]
    pub detached: bool,
    /// Create a local branch that tracks the remote branch `revision` names.
    #[serde(default, skip_serializing_if = "is_false")]
    pub track: bool,
}

/// `git.branch-create`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitBranchCreate {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation's identity.
    pub request_id: RequestId,
    /// The repository.
    pub repository: FilePath,
    /// The new branch.
    pub name: GitName,
    /// The revision it starts at; absent starts it at HEAD.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub start: Option<GitName>,
    /// Check it out.
    #[serde(default, skip_serializing_if = "is_false")]
    pub checkout: bool,
}

/// `git.branch-delete`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitBranchDelete {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation's identity.
    pub request_id: RequestId,
    /// The repository.
    pub repository: FilePath,
    /// The branch.
    pub name: GitName,
    /// Delete it even when it is not merged.
    #[serde(default, skip_serializing_if = "is_false")]
    pub force: bool,
}

/// `git.branch-upstream`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitBranchUpstream {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation's identity.
    pub request_id: RequestId,
    /// The repository.
    pub repository: FilePath,
    /// The local branch.
    pub name: GitName,
    /// The remote branch it tracks from now on, such as `origin/main`.
    pub upstream: GitName,
}

/// The operation `git.abort` ends.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AbortOperation {
    /// A merge.
    Merge,
    /// A rebase.
    Rebase,
    /// A cherry-pick.
    CherryPick,
}

/// `git.abort`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitAbort {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation's identity.
    pub request_id: RequestId,
    /// The repository.
    pub repository: FilePath,
    /// The operation to end.
    pub operation: AbortOperation,
}

/// `git.undo-commit`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitUndoCommit {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation's identity.
    pub request_id: RequestId,
    /// The repository.
    pub repository: FilePath,
    /// The commit HEAD must still name; another is `stale-revision`.
    pub commit: CommitName,
}

/// A stash or tag message, at most 16 KiB.
pub type GitMessage = Text<1, 16_384>;

/// `git.stash-push`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitStashPush {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation's identity.
    pub request_id: RequestId,
    /// The repository.
    pub repository: FilePath,
    /// The stash's message; absent lets Git describe it.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub message: Option<GitMessage>,
    /// Also stash untracked files.
    #[serde(default, skip_serializing_if = "is_false")]
    pub untracked: bool,
    /// Stash only the staged changes.
    #[serde(default, skip_serializing_if = "is_false")]
    pub staged: bool,
}

/// `git.stash-apply` and `git.stash-pop`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitStashApply {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation's identity.
    pub request_id: RequestId,
    /// The repository.
    pub repository: FilePath,
    /// `n` of `stash@{n}`.
    pub index: SafeInteger,
    /// The commit the entry must still be; another is `stale-revision`.
    pub commit: CommitName,
    /// Restore the staged changes as staged.
    #[serde(default, skip_serializing_if = "is_false")]
    pub staged: bool,
}

/// `git.stash-drop`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitStashDrop {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation's identity.
    pub request_id: RequestId,
    /// The repository.
    pub repository: FilePath,
    /// `n` of `stash@{n}`.
    pub index: SafeInteger,
    /// The commit the entry must still be; another is `stale-revision`.
    pub commit: CommitName,
}

/// `git.tag-create`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitTagCreate {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation's identity.
    pub request_id: RequestId,
    /// The repository.
    pub repository: FilePath,
    /// The new tag.
    pub name: GitName,
    /// The message of an annotated tag; absent creates a lightweight one.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub message: Option<GitMessage>,
    /// The revision it names; absent names HEAD.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub target: Option<GitName>,
}

/// `git.remote-add`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitRemoteAdd {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation's identity.
    pub request_id: RequestId,
    /// The repository.
    pub repository: FilePath,
    /// The new remote.
    pub name: GitName,
    /// Its URL.
    pub url: GitName,
}

/// A Local History entry's name, which `history.rename` gives it: 1 to 1,024 bytes.
pub type HistoryLabel = Text<1, 1024>;

/// `history.restore`: copies an entry's content over its file.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct HistoryRestore {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation's identity.
    pub request_id: RequestId,
    /// The file.
    pub path: FilePath,
    /// The entry.
    pub entry: HistoryEntryId,
}

/// `history.rename`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct HistoryRename {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation's identity.
    pub request_id: RequestId,
    /// The file.
    pub path: FilePath,
    /// The entry.
    pub entry: HistoryEntryId,
    /// Its new name.
    pub label: HistoryLabel,
}

/// What `history.delete` removes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HistoryTarget {
    /// These entries of one file; an entry already gone is skipped.
    Entries {
        /// The file.
        path: FilePath,
        /// 1 to 50 entries.
        entries: List<HistoryEntryId, 1, 50>,
    },
    /// Every entry of every file.
    All,
}

/// `history.delete`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "WireHistoryDelete", into = "WireHistoryDelete")]
pub struct HistoryDelete {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation's identity.
    pub request_id: RequestId,
    /// What it removes.
    pub target: HistoryTarget,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WireHistoryDelete {
    workspace_id: WorkspaceId,
    request_id: RequestId,
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    path: Option<FilePath>,
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    entries: Option<List<HistoryEntryId, 1, 50>>,
    #[serde(default, skip_serializing_if = "is_false")]
    all: bool,
}

impl TryFrom<WireHistoryDelete> for HistoryDelete {
    type Error = InvalidValue;

    fn try_from(wire: WireHistoryDelete) -> Result<Self, InvalidValue> {
        let target = match (wire.path, wire.entries, wire.all) {
            (Some(path), Some(entries), false) => HistoryTarget::Entries { path, entries },
            (None, None, true) => HistoryTarget::All,
            _ => return Err(InvalidValue),
        };
        Ok(Self {
            workspace_id: wire.workspace_id,
            request_id: wire.request_id,
            target,
        })
    }
}

impl From<HistoryDelete> for WireHistoryDelete {
    fn from(delete: HistoryDelete) -> Self {
        let (path, entries, all) = match delete.target {
            HistoryTarget::Entries { path, entries } => (Some(path), Some(entries), false),
            HistoryTarget::All => (None, None, true),
        };
        Self {
            workspace_id: delete.workspace_id,
            request_id: delete.request_id,
            path,
            entries,
            all,
        }
    }
}
