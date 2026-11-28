//! The results of the `files-history` operations. A page holds whole values while their JSON fits
//! one Result record, and its continuation says where the next page starts. Times are whole
//! milliseconds since the Unix epoch, as Git and Local History record them.

use serde::{Deserialize, Serialize};

use crate::{
    identity::HistoryEntryId,
    value::{CommitName, FilePath, List, PageCursor, SafeInteger, Signed, Text, is_false, some},
};

/// How many files and lines a commit changed.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitStatistics {
    /// Files changed.
    pub files: SafeInteger,
    /// Lines added.
    pub insertions: SafeInteger,
    /// Lines removed.
    pub deletions: SafeInteger,
}

/// One commit of a `git.log` page.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitLogCommit {
    /// The commit.
    pub commit: CommitName,
    /// Its parents, at most 256.
    pub parents: List<CommitName, 0, 256>,
    /// The author's name, at most 1 KiB.
    pub author_name: Text<0, 1024>,
    /// The author's email address, at most 1 KiB.
    pub author_email: Text<0, 1024>,
    /// When it was authored.
    pub author_time_ms: Signed,
    /// When it was committed.
    pub commit_time_ms: Signed,
    /// The references at the commit as Git decorates them in full, such as
    /// `HEAD -> refs/heads/main` and `tag: refs/tags/v1`; at most 256.
    pub refs: List<Text<1, 1024>, 0, 256>,
    /// The raw message without its last line end, at most 64 KiB.
    pub message: Text<0, 65_536>,
    /// The message was longer and is cut.
    #[serde(default, skip_serializing_if = "is_false")]
    pub message_cut: bool,
    /// What it changed, when the request asked; a merge's are its first parent's diff.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub statistics: Option<GitStatistics>,
    /// The followed file's path in this commit, when Git reports it.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub path: Option<FilePath>,
}

/// `git.log`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitLogResult {
    /// The page's commits, in Git's order.
    pub commits: List<GitLogCommit, 0, 1000>,
    /// The next page's continuation; absent on the last page.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub cursor: Option<PageCursor>,
}

/// A run of lines one commit last changed, numbered from 1, both ends included.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BlameRange {
    /// The first line.
    pub start: SafeInteger,
    /// The last line.
    pub end: SafeInteger,
    /// The commit; the all-zero name holds the lines no commit holds.
    pub commit: CommitName,
}

/// A commit a `git.blame` page's ranges name.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BlameCommit {
    /// The commit.
    pub commit: CommitName,
    /// The author's name, at most 1 KiB.
    pub author_name: Text<0, 1024>,
    /// The author's email address, at most 1 KiB.
    pub author_email: Text<0, 1024>,
    /// When it was authored.
    pub author_time_ms: Signed,
    /// The first line of its message, at most 64 KiB.
    pub summary: Text<0, 65_536>,
}

/// `git.blame`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitBlameResult {
    /// The page's ranges, in the order Git attributed them.
    pub ranges: List<BlameRange, 0, 10_000>,
    /// Each commit the page's ranges name, once.
    pub commits: List<BlameCommit, 0, 10_000>,
    /// The next page's continuation; absent on the last page.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub cursor: Option<PageCursor>,
}

/// How a path changed between two commits.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GitChangeKind {
    /// Added.
    Added,
    /// Modified.
    Modified,
    /// Deleted.
    Deleted,
    /// Renamed.
    Renamed,
}

/// One path of a `git.changes` page.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitChange {
    /// How it changed.
    #[serde(rename = "type")]
    pub kind: GitChangeKind,
    /// Its path in the later commit; a deleted path's in the earlier one.
    pub path: FilePath,
    /// A renamed path's path in the earlier commit.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub original: Option<FilePath>,
    /// Lines added; absent for a binary file.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub insertions: Option<SafeInteger>,
    /// Lines removed; absent for a binary file.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub deletions: Option<SafeInteger>,
}

/// `git.changes`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitChangesResult {
    /// The page's changes, in Git's order.
    pub changes: List<GitChange, 0, 2000>,
    /// The next page's continuation; absent on the last page.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub cursor: Option<PageCursor>,
}

/// `git.merge-base`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitMergeBaseResult {
    /// The merge base; absent when the histories never meet.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub commit: Option<CommitName>,
}

/// One entry of a `git.stashes` page.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitStash {
    /// `n` of `stash@{n}`.
    pub index: SafeInteger,
    /// The stash's commit.
    pub commit: CommitName,
    /// The commit it was taken on, its index, and with untracked files their commit.
    pub parents: List<CommitName, 0, 256>,
    /// The branch it was taken on, at most 1 KiB.
    pub branch: Text<0, 1024>,
    /// Its message, or HEAD's commit and subject when it was taken without one; at most 64 KiB.
    pub description: Text<0, 65_536>,
    /// It was taken without a message.
    #[serde(default, skip_serializing_if = "is_false")]
    pub wip: bool,
    /// When it was authored.
    pub author_time_ms: Signed,
    /// When it was committed.
    pub commit_time_ms: Signed,
}

/// `git.stashes`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitStashesResult {
    /// The page's entries, newest first.
    pub stashes: List<GitStash, 0, 1000>,
    /// The next page's continuation; absent on the last page.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub cursor: Option<PageCursor>,
}

/// How a Local History entry came to be.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HistorySource {
    /// A save.
    Save,
    /// A save that restored earlier content, or `history.restore`.
    Restore,
    /// A move to another folder.
    Move,
    /// A rename in its folder.
    Rename,
}

/// One Local History entry.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct HistoryEntry {
    /// The entry.
    pub id: HistoryEntryId,
    /// How it came to be.
    pub source: HistorySource,
    /// The name `history.rename` gave it.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub label: Option<Text<1, 1024>>,
    /// When the file had this content.
    pub timestamp: Signed,
    /// The content's size in bytes.
    pub size: SafeInteger,
}

/// `history.list`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct HistoryListResult {
    /// The file's entries, oldest first, at most 50.
    pub entries: List<HistoryEntry, 0, 50>,
}
