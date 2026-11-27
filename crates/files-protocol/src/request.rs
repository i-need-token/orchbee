//! File Bar requests: the closed set of `files` operations, the facts a Server needs to admit each
//! one before it relays it (its stream mode, the Session grant it needs, and whether it may name a
//! Workspace that has closed), and the strict JSON of each request. Values are validated here;
//! whether a path lies inside a Workspace's scope is decided by `orchbee-files` when the operation
//! starts.

use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    identity::{PromptId, RequestId, TransferId, WorkspaceId},
    json::{read_value, strict_object},
    link::GrantClass,
    value::{
        CommitName, Counter, EncodingName, FilePath, GitName, Integer, InvalidValue, List,
        PageCursor, SafeInteger, Text, is_false, some,
    },
};

/// How an operation's Client stream runs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationStream {
    /// One Request, one Result or Chunk, then the close.
    Request,
    /// One subscribing Request, a Snapshot, then Updates until either side closes.
    Events,
}

/// Why a Request record's JSON was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestError {
    /// `op` names no operation this build serves; the stream closes as unsupported.
    Unsupported,
    /// The JSON is malformed or breaks its operation's rules; the stream closes as an invalid
    /// request.
    Invalid,
}

impl std::fmt::Display for RequestError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Unsupported => "unsupported File Bar operation",
            Self::Invalid => "invalid File Bar request",
        })
    }
}

impl std::error::Error for RequestError {}

macro_rules! operations {
    ($(
        $(#[$meta:meta])*
        $variant:ident($request:ty) = $name:literal, $stream:ident, $grant:ident, $closed:literal;
    )+) => {
        /// Every operation the `files` capability serves.
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub enum Operation {
            $($(#[$meta])* $variant,)+
        }

        impl Operation {
            /// Every operation, in the order the conformance corpus lists them.
            pub const ALL: &[Self] = &[$(Self::$variant,)+];

            /// The `op` that names the operation.
            #[must_use]
            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)+
                }
            }

            /// The operation `op` names, if this build serves it.
            #[must_use]
            pub fn from_name(name: &str) -> Option<Self> {
                Self::ALL.iter().copied().find(|operation| operation.name() == name)
            }

            /// How the operation's stream runs; a request on a stream of the other mode is
            /// invalid.
            #[must_use]
            pub const fn stream(self) -> OperationStream {
                match self {
                    $(Self::$variant => OperationStream::$stream,)+
                }
            }

            /// The least Session grant the requesting connection must hold.
            #[must_use]
            pub const fn grant(self) -> GrantClass {
                match self {
                    $(Self::$variant => GrantClass::$grant,)+
                }
            }

            /// Whether the request may name a Workspace of the Session that has closed, where
            /// only the files its connection was granted remain in scope.
            #[must_use]
            pub const fn after_close(self) -> bool {
                match self {
                    $(Self::$variant => $closed,)+
                }
            }
        }

        /// One Request record's JSON body.
        #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
        #[serde(tag = "op")]
        pub enum FilesRequest {
            $($(#[$meta])* #[serde(rename = $name)] $variant($request),)+
        }

        impl FilesRequest {
            /// The requested operation.
            #[must_use]
            pub const fn operation(&self) -> Operation {
                match self {
                    $(Self::$variant(_) => Operation::$variant,)+
                }
            }

            /// The Workspace the request names.
            #[must_use]
            pub const fn workspace(&self) -> WorkspaceId {
                match self {
                    $(Self::$variant(request) => request.workspace_id,)+
                }
            }
        }
    };
}

operations! {
    /// Root facts, then the changes of the folders the connection loaded and the files it opened.
    FsSubscribe(WorkspaceRequest) = "fs.subscribe", Events, TerminalCreate, false;
    /// One page of a directory's entries; loads the directory for change delivery.
    FsList(FsList) = "fs.list", Request, TerminalCreate, false;
    /// Unloads directories.
    FsRelease(PathsRequest) = "fs.release", Request, TerminalCreate, false;
    /// The entries of paths, or why each has none.
    FsStat(PathsRequest) = "fs.stat", Request, TerminalCreate, true;
    /// Grants the connection one opened file.
    FsGrant(FsGrant) = "fs.grant", Request, TerminalCreate, false;
    /// Withdraws opened-file grants.
    FsUngrant(PathsRequest) = "fs.ungrant", Request, TerminalCreate, true;
    /// Opens a read transfer.
    FsRead(FsRead) = "fs.read", Request, TerminalCreate, true;
    /// One chunk of an open read transfer.
    FsReadChunk(FsReadChunk) = "fs.read-chunk", Request, TerminalCreate, true;
    /// The encoding a file's content suggests.
    FsGuessEncoding(PathRequest) = "fs.guess-encoding", Request, TerminalCreate, true;
    /// Ends a transfer and frees what it holds.
    TransferClose(TransferRequest) = "transfer.close", Request, TerminalCreate, true;
    /// The Workspace's source control summary, then every change of it.
    ScmSubscribe(WorkspaceRequest) = "scm.subscribe", Events, TerminalCreate, false;
    /// One page of a repository's status at a revision.
    GitStatusPage(GitStatusPage) = "git.status-page", Request, TerminalCreate, false;
    /// Opens a read transfer of a file's content at a reference.
    GitShow(GitShow) = "git.show", Request, TerminalCreate, false;
    /// Which paths the repository ignores.
    GitCheckIgnore(GitCheckIgnore) = "git.check-ignore", Request, TerminalCreate, false;
    /// One page of a repository's references, and its remotes.
    GitRefs(GitRefs) = "git.refs", Request, TerminalCreate, false;
    /// The user name and email the repository's configuration sets.
    GitUserConfig(RepositoryRequest) = "git.user-config", Request, TerminalCreate, false;
    /// The Workspace's Git Output from a cursor.
    GitOutput(GitOutput) = "git.output", Request, TerminalCreate, false;
    /// Stages paths.
    GitStage(GitPaths) = "git.stage", Request, Lifecycle, false;
    /// Unstages paths.
    GitUnstage(GitPaths) = "git.unstage", Request, Lifecycle, false;
    /// Discards working-tree changes of paths.
    GitDiscard(GitPaths) = "git.discard", Request, Lifecycle, false;
    /// Replaces a path's index content with the transfer's Puts.
    GitStageContent(GitStageContent) = "git.stage-content", Request, Lifecycle, false;
    /// Removes untracked paths.
    GitClean(GitClean) = "git.clean", Request, Lifecycle, false;
    /// Commits.
    GitCommit(GitCommit) = "git.commit", Request, Lifecycle, false;
    /// Fetches.
    GitFetch(GitFetch) = "git.fetch", Request, Lifecycle, false;
    /// Pulls.
    GitPull(GitPull) = "git.pull", Request, Lifecycle, false;
    /// Pulls, then pushes.
    GitSync(GitPull) = "git.sync", Request, Lifecycle, false;
    /// Pushes.
    GitPush(GitPush) = "git.push", Request, Lifecycle, false;
    /// Creates a repository.
    GitInit(GitPath) = "git.init", Request, Lifecycle, false;
    /// Adds a repository to the user's `safe.directory`.
    GitMarkSafe(GitPath) = "git.mark-safe", Request, Lifecycle, false;
    /// Admits a repository above the root or an opened file after the user's consent.
    RepositoryConsent(RepositoryRequest) = "repository.consent", Request, TerminalCreate, false;
    /// Withdraws a repository's consent.
    RepositoryClose(RepositoryRequest) = "repository.close", Request, TerminalCreate, false;
    /// The credential, host-key and editor prompts routed to the connection.
    RelaySubscribe(WorkspaceRequest) = "relay.subscribe", Events, TerminalCreate, false;
    /// Answers a prompt.
    RelayAnswer(RelayAnswer) = "relay.answer", Request, Lifecycle, false;
    /// Whether the connection's window has focus, which status refreshes wait for.
    ClientFocus(ClientFocus) = "client.focus", Request, TerminalCreate, false;
    /// How an earlier mutation ended.
    RequestOutcome(RequestOutcome) = "request.outcome", Request, TerminalCreate, true;
}

/// Reads one Request record's JSON body.
///
/// # Errors
///
/// [`RequestError::Unsupported`] for an `op` this build does not serve, and
/// [`RequestError::Invalid`] for anything else the operation's rules refuse, malformed JSON and
/// duplicate keys included.
pub fn parse_request(json: &[u8]) -> Result<FilesRequest, RequestError> {
    let object = strict_object(json).map_err(|_| RequestError::Invalid)?;
    match object.get("op").and_then(serde_json::Value::as_str) {
        Some(name) if Operation::from_name(name).is_some() => {
            read_value(&object).map_err(|_| RequestError::Invalid)
        }
        Some(_) => Err(RequestError::Unsupported),
        None => Err(RequestError::Invalid),
    }
}

/// The canonical JSON of `request`.
#[must_use]
pub fn encode_request(request: &FilesRequest) -> Vec<u8> {
    serde_json::to_vec(request).expect("every request value encodes as JSON")
}

/// A request that names only its Workspace.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct WorkspaceRequest {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
}

/// A request about up to 256 paths.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PathsRequest {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// 1 to 256 paths.
    pub paths: List<FilePath, 1, 256>,
}

/// A request about one path.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PathRequest {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The path.
    pub path: FilePath,
}

/// A request about one transfer.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TransferRequest {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The transfer.
    pub transfer: TransferId,
}

/// A request about one repository.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RepositoryRequest {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The repository's canonical toplevel path.
    pub repository: FilePath,
}

/// `fs.list`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FsList {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The directory.
    pub path: FilePath,
    /// The previous page's continuation; absent for the first page.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub cursor: Option<PageCursor>,
    /// Also list the chain of single-child directories below the directory, up to 64 levels.
    #[serde(default, skip_serializing_if = "is_false")]
    pub single_child: bool,
}

/// The gesture that opened a file, which `fs.grant` records.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GrantSource {
    /// The Explorer or a source control view.
    Tree,
    /// A file link in a terminal.
    TerminalLink,
    /// A tab restored from layout memory.
    Restored,
    /// The operating system opened the file with Orchbee.
    OsOpen,
    /// A Save As destination the picker chose.
    SaveAs,
    /// One side of a diff.
    Diff,
}

/// `fs.grant`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FsGrant {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The file.
    pub path: FilePath,
    /// The gesture that opened it.
    pub source: GrantSource,
}

/// Whether a read decodes text.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReadMode {
    /// Decoded to UTF-8, served in offset order.
    Text,
    /// The file's bytes, served in any order.
    Bytes,
}

/// `fs.read`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FsRead {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The transfer the Client chose for the read.
    pub transfer: TransferId,
    /// The file.
    pub path: FilePath,
    /// Whether the read decodes text.
    pub mode: ReadMode,
    /// The encoding to decode without a byte order mark; absent means UTF-8.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub encoding: Option<EncodingName>,
    /// Decode a binary file as text.
    #[serde(default, skip_serializing_if = "is_false")]
    pub force_text: bool,
    /// The largest file the Client accepts, in bytes; a larger one is `too-large`.
    pub limit: SafeInteger,
}

/// A chunk length the Client chooses: 16 to 128 KiB.
pub type ChunkLength = Integer<16_384, 131_072>;

/// `fs.read-chunk`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FsReadChunk {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The open transfer.
    pub transfer: TransferId,
    /// Where the chunk starts: in decoded UTF-8 output bytes for a text read, in file bytes for a
    /// byte read.
    pub offset: Counter,
    /// The most bytes the chunk carries.
    pub length: ChunkLength,
}

/// `git.status-page`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitStatusPage {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The repository.
    pub repository: FilePath,
    /// The summary revision the pages belong to; a newer status makes it `stale-revision`.
    pub revision: Counter,
    /// The previous page's continuation; absent for the first page.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub cursor: Option<PageCursor>,
}

/// The version of a file `git.show` reads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ShowRef {
    /// The commit `HEAD` names.
    Head,
    /// The index.
    Index,
    /// A commit.
    Commit(CommitName),
}

impl Serialize for ShowRef {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(match self {
            Self::Head => "HEAD",
            Self::Index => "index",
            Self::Commit(commit) => commit.as_str(),
        })
    }
}

impl<'de> Deserialize<'de> for ShowRef {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        match text.as_str() {
            "HEAD" => Ok(Self::Head),
            "index" => Ok(Self::Index),
            _ => CommitName::new(text)
                .map(Self::Commit)
                .map_err(|_| serde::de::Error::custom("HEAD, index or a full commit name")),
        }
    }
}

/// `git.show`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitShow {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The transfer the Client chose for the read.
    pub transfer: TransferId,
    /// The repository.
    pub repository: FilePath,
    /// The version to read.
    #[serde(rename = "ref")]
    pub reference: ShowRef,
    /// The file.
    pub path: FilePath,
    /// Whether the read decodes text.
    pub mode: ReadMode,
    /// The encoding to decode without a byte order mark; absent means UTF-8.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub encoding: Option<EncodingName>,
    /// The largest content the Client accepts, in bytes.
    pub limit: SafeInteger,
}

/// `git.check-ignore`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitCheckIgnore {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The repository.
    pub repository: FilePath,
    /// 1 to 1,000 paths.
    pub paths: List<FilePath, 1, 1000>,
}

/// `git.refs`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitRefs {
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

/// `git.output`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitOutput {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// Where to continue; absent for the oldest output the ring keeps.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub cursor: Option<Counter>,
}

/// `git.stage`, `git.unstage` and `git.discard`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitPaths {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation's identity.
    pub request_id: RequestId,
    /// The repository.
    pub repository: FilePath,
    /// 1 to 256 paths.
    pub paths: List<FilePath, 1, 256>,
}

/// `git.stage-content`; the index content follows as the transfer's Puts, the last flagged
/// `commit`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitStageContent {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation's identity.
    pub request_id: RequestId,
    /// The transfer that carries the content.
    pub transfer: TransferId,
    /// The repository.
    pub repository: FilePath,
    /// The file.
    pub path: FilePath,
    /// The content's length in bytes.
    pub size: SafeInteger,
}

/// How `git.clean` removes untracked paths.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CleanMode {
    /// To the machine's trash.
    Trash,
    /// Permanently.
    Permanent,
}

/// `git.clean`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitClean {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation's identity.
    pub request_id: RequestId,
    /// The repository.
    pub repository: FilePath,
    /// 1 to 256 untracked paths.
    pub paths: List<FilePath, 1, 256>,
    /// How they are removed.
    pub mode: CleanMode,
}

/// Where a commit's message comes from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommitMessage {
    /// This text, at most 16 KiB.
    Text(Text<0, 16_384>),
    /// The relay's editor prompt.
    Editor,
}

/// Which changes a commit includes beyond the index.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CommitAll {
    /// The index only.
    None,
    /// Also every tracked change.
    Tracked,
    /// Also every untracked file.
    All,
}

/// `git.commit`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "WireGitCommit", into = "WireGitCommit")]
pub struct GitCommit {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation's identity.
    pub request_id: RequestId,
    /// The repository.
    pub repository: FilePath,
    /// Where the message comes from.
    pub message: CommitMessage,
    /// Amend the last commit.
    pub amend: bool,
    /// Add a sign-off trailer.
    pub signoff: bool,
    /// Which changes beyond the index the commit includes.
    pub all: CommitAll,
    /// Skip the commit hooks.
    pub no_verify: bool,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WireGitCommit {
    workspace_id: WorkspaceId,
    request_id: RequestId,
    repository: FilePath,
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    message: Option<Text<0, 16_384>>,
    #[serde(default, skip_serializing_if = "is_false")]
    editor: bool,
    amend: bool,
    signoff: bool,
    all: CommitAll,
    no_verify: bool,
}

impl TryFrom<WireGitCommit> for GitCommit {
    type Error = InvalidValue;

    fn try_from(wire: WireGitCommit) -> Result<Self, InvalidValue> {
        let message = match (wire.message, wire.editor) {
            (Some(text), false) => CommitMessage::Text(text),
            (None, true) => CommitMessage::Editor,
            _ => return Err(InvalidValue),
        };
        Ok(Self {
            workspace_id: wire.workspace_id,
            request_id: wire.request_id,
            repository: wire.repository,
            message,
            amend: wire.amend,
            signoff: wire.signoff,
            all: wire.all,
            no_verify: wire.no_verify,
        })
    }
}

impl From<GitCommit> for WireGitCommit {
    fn from(commit: GitCommit) -> Self {
        let (message, editor) = match commit.message {
            CommitMessage::Text(text) => (Some(text), false),
            CommitMessage::Editor => (None, true),
        };
        Self {
            workspace_id: commit.workspace_id,
            request_id: commit.request_id,
            repository: commit.repository,
            message,
            editor,
            amend: commit.amend,
            signoff: commit.signoff,
            all: commit.all,
            no_verify: commit.no_verify,
        }
    }
}

/// Which remotes `git.fetch` fetches.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FetchRemotes {
    /// The branch's default remote.
    Default,
    /// This remote.
    Remote(GitName),
    /// Every remote.
    All,
}

/// `git.fetch`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "WireGitFetch", into = "WireGitFetch")]
pub struct GitFetch {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation's identity.
    pub request_id: RequestId,
    /// The repository.
    pub repository: FilePath,
    /// Which remotes to fetch.
    pub remotes: FetchRemotes,
    /// Prune remote-tracking references that no longer exist.
    pub prune: bool,
    /// An automatic fetch, which never prompts.
    pub background: bool,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WireGitFetch {
    workspace_id: WorkspaceId,
    request_id: RequestId,
    repository: FilePath,
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    remote: Option<GitName>,
    #[serde(default, skip_serializing_if = "is_false")]
    all: bool,
    prune: bool,
    background: bool,
}

impl TryFrom<WireGitFetch> for GitFetch {
    type Error = InvalidValue;

    fn try_from(wire: WireGitFetch) -> Result<Self, InvalidValue> {
        let remotes = match (wire.remote, wire.all) {
            (None, false) => FetchRemotes::Default,
            (Some(remote), false) => FetchRemotes::Remote(remote),
            (None, true) => FetchRemotes::All,
            (Some(_), true) => return Err(InvalidValue),
        };
        Ok(Self {
            workspace_id: wire.workspace_id,
            request_id: wire.request_id,
            repository: wire.repository,
            remotes,
            prune: wire.prune,
            background: wire.background,
        })
    }
}

impl From<GitFetch> for WireGitFetch {
    fn from(fetch: GitFetch) -> Self {
        let (remote, all) = match fetch.remotes {
            FetchRemotes::Default => (None, false),
            FetchRemotes::Remote(remote) => (Some(remote), false),
            FetchRemotes::All => (None, true),
        };
        Self {
            workspace_id: fetch.workspace_id,
            request_id: fetch.request_id,
            repository: fetch.repository,
            remote,
            all,
            prune: fetch.prune,
            background: fetch.background,
        }
    }
}

/// `git.pull` and `git.sync`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitPull {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation's identity.
    pub request_id: RequestId,
    /// The repository.
    pub repository: FilePath,
    /// Rebase instead of merging.
    pub rebase: bool,
}

/// How `git.push` may replace remote history.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PushForce {
    /// Never.
    None,
    /// Only when the remote is where the last fetch saw it.
    Lease,
    /// Always.
    Force,
}

/// `git.push`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitPush {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation's identity.
    pub request_id: RequestId,
    /// The repository.
    pub repository: FilePath,
    /// The remote; absent for the branch's upstream.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub remote: Option<GitName>,
    /// The branch; absent for the current one.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub branch: Option<GitName>,
    /// Set the pushed branch's upstream, which publishes it.
    pub set_upstream: bool,
    /// How remote history may be replaced.
    pub force: PushForce,
    /// Push tags too.
    pub tags: bool,
}

/// `git.init` and `git.mark-safe`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitPath {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation's identity.
    pub request_id: RequestId,
    /// The directory.
    pub path: FilePath,
}

/// What the user answered a prompt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PromptAnswer {
    /// This text, at most 4 KiB; never logged, persisted or echoed.
    Value(Text<0, 4096>),
    /// Nothing: an empty credential, or the editor's tab closed.
    Empty,
    /// The user cancelled the operation.
    Cancelled,
}

/// `relay.answer`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "WireRelayAnswer", into = "WireRelayAnswer")]
pub struct RelayAnswer {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The prompt.
    pub prompt: PromptId,
    /// The answer.
    pub answer: PromptAnswer,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WireRelayAnswer {
    workspace_id: WorkspaceId,
    prompt: PromptId,
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    value: Option<Text<0, 4096>>,
    #[serde(default, skip_serializing_if = "is_false")]
    cancelled: bool,
}

impl TryFrom<WireRelayAnswer> for RelayAnswer {
    type Error = InvalidValue;

    fn try_from(wire: WireRelayAnswer) -> Result<Self, InvalidValue> {
        let answer = match (wire.value, wire.cancelled) {
            (Some(value), false) => PromptAnswer::Value(value),
            (None, false) => PromptAnswer::Empty,
            (None, true) => PromptAnswer::Cancelled,
            (Some(_), true) => return Err(InvalidValue),
        };
        Ok(Self {
            workspace_id: wire.workspace_id,
            prompt: wire.prompt,
            answer,
        })
    }
}

impl From<RelayAnswer> for WireRelayAnswer {
    fn from(answer: RelayAnswer) -> Self {
        let (value, cancelled) = match answer.answer {
            PromptAnswer::Value(value) => (Some(value), false),
            PromptAnswer::Empty => (None, false),
            PromptAnswer::Cancelled => (None, true),
        };
        Self {
            workspace_id: answer.workspace_id,
            prompt: answer.prompt,
            value,
            cancelled,
        }
    }
}

/// `client.focus`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ClientFocus {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// Whether the connection's window has focus.
    pub focused: bool,
}

/// `request.outcome`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RequestOutcome {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation.
    pub request_id: RequestId,
}
