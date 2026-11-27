//! File Bar notices: the JSON of the Snapshot and Update records a subscription streams. A Client
//! treats a notice it cannot read as absent; after a Gap it re-lists its loaded folders or takes
//! the next Snapshot.

use serde::{Deserialize, Serialize};

use crate::{
    identity::PromptId,
    json::read_object,
    request::Operation,
    value::{CommitName, Counter, FilePath, InvalidValue, List, SafeInteger, Text, some},
};

/// Which notice of a subscription a record is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NoticeKind {
    /// The first notice, which states everything.
    Snapshot,
    /// A later notice, which states what changed.
    Update,
}

/// A Workspace's root as `orchbee-files` sees it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Root {
    /// The Workspace's current path exists at this canonical path.
    Present(FilePath),
    /// The Workspace's current path, spelled as the Workbench reported it, no longer exists.
    Missing(FilePath),
    /// The Workspace has no current path, so only its connection's granted files are in scope.
    None,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum RootState {
    Present,
    Missing,
    None,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireRoot {
    state: RootState,
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    path: Option<FilePath>,
}

impl TryFrom<WireRoot> for Root {
    type Error = InvalidValue;

    fn try_from(wire: WireRoot) -> Result<Self, InvalidValue> {
        match (wire.state, wire.path) {
            (RootState::Present, Some(path)) => Ok(Self::Present(path)),
            (RootState::Missing, Some(path)) => Ok(Self::Missing(path)),
            (RootState::None, None) => Ok(Self::None),
            _ => Err(InvalidValue),
        }
    }
}

impl From<Root> for WireRoot {
    fn from(root: Root) -> Self {
        match root {
            Root::Present(path) => Self {
                state: RootState::Present,
                path: Some(path),
            },
            Root::Missing(path) => Self {
                state: RootState::Missing,
                path: Some(path),
            },
            Root::None => Self {
                state: RootState::None,
                path: None,
            },
        }
    }
}

impl Serialize for Root {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        WireRoot::from(self.clone()).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Root {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        WireRoot::deserialize(deserializer)?
            .try_into()
            .map_err(|_| {
                serde::de::Error::custom("a root state with a path exactly when present or missing")
            })
    }
}

/// How completely the watcher delivers changes.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WatchState {
    /// Every loaded folder and opened file is watched.
    Full,
    /// Some are not, such as after the operating system's watch limit; the Client refreshes on
    /// focus.
    Degraded,
    /// Nothing is watched.
    Stopped,
}

/// The machine's operating system, which decides path spelling and comparison.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OperatingSystem {
    /// Windows.
    Windows,
    /// macOS.
    Macos,
    /// Linux.
    Linux,
}

/// The machine's path separator.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Separator {
    /// `\`.
    #[serde(rename = "\\")]
    Backslash,
    /// `/`.
    #[serde(rename = "/")]
    Slash,
}

/// `fs.subscribe`'s Snapshot.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FsSnapshot {
    /// The Workspace's root.
    pub root: Root,
    /// The machine's operating system.
    pub os: OperatingSystem,
    /// The machine's path separator.
    pub separator: Separator,
    /// Whether the root's volume compares names case-sensitively.
    pub case_sensitive: bool,
    /// How completely changes are delivered.
    pub watch: WatchState,
}

/// What happened to a watched item.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChangeKind {
    /// It appeared.
    Added,
    /// Its content or metadata changed.
    Changed,
    /// It disappeared.
    Deleted,
}

/// One watched change.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Change {
    /// What happened.
    #[serde(rename = "type")]
    pub kind: ChangeKind,
    /// The item.
    pub path: FilePath,
    /// The item is or was a directory, as far as the watcher knows.
    pub directory: bool,
}

/// `fs.subscribe`'s Update.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum FsUpdate {
    /// Up to 500 changes, coalesced.
    Changes {
        /// The changes.
        changes: List<Change, 1, 500>,
    },
    /// Changes were lost under these folders; the Client re-lists what it loaded there.
    Rescan {
        /// The folders.
        paths: List<FilePath, 1, 500>,
    },
    /// The root changed.
    Root {
        /// The new root.
        root: Root,
    },
    /// The watch's completeness changed.
    Watch {
        /// The new state.
        watch: WatchState,
    },
}

/// How a repository came into a Workspace's summary.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RepositoryKind {
    /// Its toplevel is the root.
    Root,
    /// It lies below the root.
    Nested,
    /// It contains the root, admitted by consent.
    Parent,
    /// It contains an opened file outside the root, admitted by consent.
    File,
}

/// A repository's `HEAD`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Head {
    /// The branch's short name, absent when detached.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub branch: Option<Text<1, 1024>>,
    /// The commit, absent while unborn.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub commit: Option<CommitName>,
    /// `HEAD` names a commit rather than a branch.
    pub detached: bool,
    /// The branch has no commit yet.
    pub unborn: bool,
}

/// A branch's upstream and how far apart they are.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Upstream {
    /// The upstream's short name, such as `origin/main`.
    pub name: Text<1, 1024>,
    /// Commits the branch has that the upstream lacks.
    pub ahead: SafeInteger,
    /// Commits the upstream has that the branch lacks.
    pub behind: SafeInteger,
}

/// A multi-step operation a repository is in.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RepositoryOperation {
    /// None.
    None,
    /// A merge.
    Merge,
    /// A rebase.
    Rebase,
    /// A cherry-pick.
    CherryPick,
    /// A revert.
    Revert,
}

/// How many status entries each group holds.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StatusCounts {
    /// Unmerged entries.
    pub merge: SafeInteger,
    /// Staged entries.
    pub index: SafeInteger,
    /// Unstaged changes to tracked files.
    pub working_tree: SafeInteger,
    /// Untracked files.
    pub untracked: SafeInteger,
}

/// The mutation a repository is running, which disables conflicting actions.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BusyOperation {
    /// `git.stage`.
    Stage,
    /// `git.unstage`.
    Unstage,
    /// `git.discard`.
    Discard,
    /// `git.stage-content`.
    StageContent,
    /// `git.clean`.
    Clean,
    /// `git.commit`.
    Commit,
    /// `git.fetch`.
    Fetch,
    /// `git.pull`.
    Pull,
    /// `git.sync`.
    Sync,
    /// `git.push`.
    Push,
}

/// One repository of a Workspace's summary.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ScmRepository {
    /// The canonical toplevel path.
    pub repository: FilePath,
    /// How it came into the summary.
    pub kind: RepositoryKind,
    /// Its `HEAD`.
    pub head: Head,
    /// The branch's upstream, when it has one.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub upstream: Option<Upstream>,
    /// The multi-step operation it is in.
    pub operation: RepositoryOperation,
    /// How many entries each status group holds.
    pub counts: StatusCounts,
    /// The status exceeded its bound, so pages hold only a part.
    pub truncated: bool,
    /// The status revision `git.status-page` names.
    pub revision: Counter,
    /// The mutation it is running.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub busy: Option<BusyOperation>,
    /// The status was read since `orchbee-files` started.
    pub warm: bool,
    /// A newer status is being read.
    pub stale: bool,
}

/// Whether Git can serve.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GitState {
    /// It can.
    Ok,
    /// No Git program was found.
    Missing,
    /// The Git program is older than the oldest served version.
    TooOld,
}

/// The machine's Git program.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitProgram {
    /// Whether it can serve.
    pub state: GitState,
    /// Its version, when known.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub version: Option<Text<1, 64>>,
}

/// `scm.subscribe`'s Snapshot and every Update: the whole summary.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ScmSummary {
    /// The machine's Git program.
    pub git: GitProgram,
    /// The repositories in scope, at most 256.
    pub repositories: List<ScmRepository, 0, 256>,
    /// Repositories above the root or an opened file that await the user's consent.
    pub candidates: List<FilePath, 0, 256>,
    /// Repositories Git refuses for their ownership.
    #[serde(rename = "unsafe")]
    pub unsafe_repositories: List<FilePath, 0, 256>,
    /// More repositories were found than the summary holds.
    pub repositories_truncated: bool,
}

/// What a prompt asks for.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PromptKind {
    /// A user name.
    Username,
    /// A password.
    Password,
    /// A key's passphrase.
    Passphrase,
    /// Whether to trust a host key.
    HostKey,
    /// A commit or tag message, edited in a tab.
    Editor,
}

/// One prompt routed to the connection.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Prompt {
    /// The prompt.
    pub id: PromptId,
    /// What it asks for.
    pub kind: PromptKind,
    /// The asking program's own text, shown verbatim.
    pub text: Text<0, 4096>,
    /// The repository whose operation asks.
    pub repository: FilePath,
    /// The file an editor prompt edits.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub path: Option<FilePath>,
}

/// `relay.subscribe`'s Snapshot: the prompts waiting for the connection.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RelaySnapshot {
    /// At most 16 prompts.
    pub prompts: List<Prompt, 0, 16>,
}

/// `relay.subscribe`'s Update.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum RelayUpdate {
    /// A prompt for the connection.
    Prompt {
        /// The prompt.
        prompt: Prompt,
    },
    /// A prompt was answered, timed out or lost its operation.
    Closed {
        /// The prompt.
        id: PromptId,
    },
}

/// One Notice record's JSON body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FilesNotice {
    /// `fs.subscribe`'s Snapshot.
    FsSnapshot(FsSnapshot),
    /// `fs.subscribe`'s Update.
    FsUpdate(FsUpdate),
    /// `scm.subscribe`'s Snapshot or Update.
    ScmSummary(Box<ScmSummary>),
    /// `relay.subscribe`'s Snapshot.
    RelaySnapshot(RelaySnapshot),
    /// `relay.subscribe`'s Update.
    RelayUpdate(RelayUpdate),
}

/// Reads the JSON body of a Notice record that a subscription of `operation` streams as `kind`.
///
/// # Errors
///
/// [`InvalidValue`] for an operation that streams no notices and for JSON its notice's rules
/// refuse.
pub fn parse_notice(
    operation: Operation,
    kind: NoticeKind,
    json: &[u8],
) -> Result<FilesNotice, InvalidValue> {
    Ok(match (operation, kind) {
        (Operation::FsSubscribe, NoticeKind::Snapshot) => {
            FilesNotice::FsSnapshot(read_object(json)?)
        }
        (Operation::FsSubscribe, NoticeKind::Update) => FilesNotice::FsUpdate(read_object(json)?),
        (Operation::ScmSubscribe, _) => FilesNotice::ScmSummary(Box::new(read_object(json)?)),
        (Operation::RelaySubscribe, NoticeKind::Snapshot) => {
            FilesNotice::RelaySnapshot(read_object(json)?)
        }
        (Operation::RelaySubscribe, NoticeKind::Update) => {
            FilesNotice::RelayUpdate(read_object(json)?)
        }
        _ => return Err(InvalidValue),
    })
}

/// The canonical JSON of `notice`.
#[must_use]
pub fn encode_notice(notice: &FilesNotice) -> Vec<u8> {
    let encoded = match notice {
        FilesNotice::FsSnapshot(snapshot) => serde_json::to_vec(snapshot),
        FilesNotice::FsUpdate(update) => serde_json::to_vec(update),
        FilesNotice::ScmSummary(summary) => serde_json::to_vec(summary),
        FilesNotice::RelaySnapshot(snapshot) => serde_json::to_vec(snapshot),
        FilesNotice::RelayUpdate(update) => serde_json::to_vec(update),
    };
    encoded.expect("every notice value encodes as JSON")
}
