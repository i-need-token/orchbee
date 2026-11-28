//! File Bar replies: the JSON of each operation's Result record, or the error that replaces it.
//! An error carries a stable code and content Orchbee did not author, never presentation text;
//! the frontend localizes codes. Which Workspace or connection a reply reaches is the relay's
//! concern.

use serde::{Deserialize, Serialize};

mod edit;
mod history;
mod search;

pub use edit::{
    ExportEntry, ExportFacts, ExportKind, FsDeleteResult, FsEntryResult, FsExistsResult,
    FsExportEntriesResult, FsImportFileResult, FsImportResult, FsTransferResult, FsWriteResult,
    ImportOutcome, PickerListResult, PickerOpenResult, TransferItem,
};
pub use history::{
    BlameCommit, BlameRange, GitBlameResult, GitChange, GitChangeKind, GitChangesResult,
    GitLogCommit, GitLogResult, GitMergeBaseResult, GitStash, GitStashesResult, GitStatistics,
    HistoryEntry, HistoryListResult, HistorySource,
};
pub use search::{
    FoundFile, SearchContextLine, SearchFile, SearchFindFilesResult, SearchMatch, SearchNextResult,
    SearchRange, SearchSpan,
};

use crate::{
    json::read_object,
    request::Operation,
    value::{
        CommitName, Counter, EncodingName, FilePath, GitName, InvalidValue, List, PageCursor,
        SafeInteger, Signed, Text, Token, is_false, nullable, some,
    },
};

/// Why an operation failed.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ErrorCode {
    /// The connection's Session grants do not allow the operation.
    Denied,
    /// The path is outside every member of the Workspace's scope.
    OutsideScope,
    /// A mutation's real target is outside the scope through a link.
    LinkTargetOutside,
    /// The location's form is not served, such as a UNC root.
    UnsupportedLocation,
    /// Nothing exists at the path.
    NotFound,
    /// Something already exists at the path.
    Exists,
    /// The path is a directory where a file was expected.
    IsDirectory,
    /// The path is not a directory where one was expected.
    NotDirectory,
    /// The name is not valid on the machine.
    InvalidName,
    /// The name is not valid UTF-8, so it cannot be operated on.
    UnrepresentableName,
    /// The operating system refused access.
    PermissionDenied,
    /// The file is read-only.
    ReadOnly,
    /// The volume is full.
    DiskFull,
    /// Another process holds the file.
    Busy,
    /// The content exceeds the Client's limit.
    TooLarge,
    /// The file changed since the Client read it.
    ContentNewer,
    /// A write failed after its commit point.
    WriteIncomplete,
    /// The machine has no trash for the path.
    TrashUnavailable,
    /// Moving to the trash failed.
    TrashFailed,
    /// The undo record is gone.
    UndoExpired,
    /// A newer status replaced the revision.
    StaleRevision,
    /// The operation passed its deadline.
    Timeout,
    /// The operation was cancelled.
    Cancelled,
    /// The service cannot serve now; `reason` says why.
    Unavailable,
    /// Git refuses the repository's ownership.
    UnsafeRepository,
    /// Git failed; `gitErrorCode` classifies it when it can.
    GitFailed,
    /// The search pattern is not a valid regular expression.
    InvalidRegex,
    /// The request is well formed but not valid now, such as an unknown transfer.
    InvalidRequest,
    /// Any other failure.
    Failed,
}

/// An operation's failure.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "WireError", into = "WireError")]
pub struct FilesError {
    /// Why it failed.
    pub code: ErrorCode,
    /// Why the service is unavailable, present exactly with [`ErrorCode::Unavailable`].
    pub reason: Option<Token>,
    /// The operating system's error number.
    pub os_error: Option<Signed>,
    /// VS Code's `GitErrorCodes` name in kebab-case, such as `repository-is-locked`.
    pub git_error_code: Option<Token>,
    /// Content Orchbee did not author, such as git's standard error, at most 4 KiB.
    pub detail: Option<Text<0, 4096>>,
}

impl FilesError {
    /// The failure `code` without further facts.
    ///
    /// # Panics
    ///
    /// For [`ErrorCode::Unavailable`], which needs [`Self::unavailable`].
    #[must_use]
    pub fn new(code: ErrorCode) -> Self {
        assert!(code != ErrorCode::Unavailable, "unavailable needs a reason");
        Self {
            code,
            reason: None,
            os_error: None,
            git_error_code: None,
            detail: None,
        }
    }

    /// The service is unavailable for `reason`.
    #[must_use]
    pub const fn unavailable(reason: Token) -> Self {
        Self {
            code: ErrorCode::Unavailable,
            reason: Some(reason),
            os_error: None,
            git_error_code: None,
            detail: None,
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WireError {
    code: ErrorCode,
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    reason: Option<Token>,
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    os_error: Option<Signed>,
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    git_error_code: Option<Token>,
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    detail: Option<Text<0, 4096>>,
}

impl TryFrom<WireError> for FilesError {
    type Error = InvalidValue;

    fn try_from(wire: WireError) -> Result<Self, InvalidValue> {
        if (wire.code == ErrorCode::Unavailable) != wire.reason.is_some() {
            return Err(InvalidValue);
        }
        Ok(Self {
            code: wire.code,
            reason: wire.reason,
            os_error: wire.os_error,
            git_error_code: wire.git_error_code,
            detail: wire.detail,
        })
    }
}

impl From<FilesError> for WireError {
    fn from(error: FilesError) -> Self {
        Self {
            code: error.code,
            reason: error.reason,
            os_error: error.os_error,
            git_error_code: error.git_error_code,
            detail: error.detail,
        }
    }
}

/// A Result record that carries an error instead of the operation's result.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ErrorReply {
    /// The failure.
    pub error: FilesError,
}

/// The result of an operation that reports nothing but its success: `{}`.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Empty {}

/// What a directory entry, or a link's target, is.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EntryKind {
    /// A file.
    File,
    /// A directory.
    Directory,
    /// Neither, or a link whose target cannot be read.
    Unknown,
}

/// One directory entry.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Entry {
    /// The final component, at most 1,024 bytes; empty for a filesystem root.
    pub name: Text<0, 1024>,
    /// What the entry is; for a link, what its target is.
    #[serde(rename = "type")]
    pub kind: EntryKind,
    /// The entry is a link; junctions count as links.
    pub symlink: bool,
    /// The size in bytes.
    pub size: SafeInteger,
    /// The modification time in milliseconds since the Unix epoch.
    pub mtime_ms: Signed,
    /// The file is read-only.
    pub readonly: bool,
    /// The name is not valid UTF-8: `name` is a lossy spelling, and the entry cannot be operated
    /// on.
    #[serde(default, skip_serializing_if = "is_false")]
    pub unrepresentable: bool,
}

/// One directory of a single-child chain below a listed directory.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Descendant {
    /// The directory.
    pub path: FilePath,
    /// Its entries.
    pub entries: List<Entry, 0, 2000>,
}

/// `fs.list`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FsListResult {
    /// The page's entries, unsorted.
    pub entries: List<Entry, 0, 2000>,
    /// The next page's continuation; absent on the last page.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub cursor: Option<PageCursor>,
    /// The single-child chain below the directory, when the request asked for it.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub descendants: Option<List<Descendant, 1, 64>>,
}

/// One `fs.stat` answer.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "WireStatItem", into = "WireStatItem")]
pub struct StatItem {
    /// The path as requested.
    pub path: FilePath,
    /// Its entry, or why it has none.
    pub outcome: Result<Entry, FilesError>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireStatItem {
    path: FilePath,
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    entry: Option<Entry>,
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    error: Option<FilesError>,
}

impl TryFrom<WireStatItem> for StatItem {
    type Error = InvalidValue;

    fn try_from(wire: WireStatItem) -> Result<Self, InvalidValue> {
        let outcome = match (wire.entry, wire.error) {
            (Some(entry), None) => Ok(entry),
            (None, Some(error)) => Err(error),
            _ => return Err(InvalidValue),
        };
        Ok(Self {
            path: wire.path,
            outcome,
        })
    }
}

impl From<StatItem> for WireStatItem {
    fn from(item: StatItem) -> Self {
        let (entry, error) = match item.outcome {
            Ok(entry) => (Some(entry), None),
            Err(error) => (None, Some(error)),
        };
        Self {
            path: item.path,
            entry,
            error,
        }
    }
}

/// `fs.stat`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FsStatResult {
    /// One answer per requested path, in request order.
    pub items: List<StatItem, 1, 256>,
}

/// `fs.grant`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FsGrantResult {
    /// The granted file's entry; absent only for a Save As destination that does not exist yet.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub entry: Option<Entry>,
}

/// `fs.read` and `git.show`: what the read found before its first chunk.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ReadResult {
    /// The content's size in bytes.
    pub size: SafeInteger,
    /// The file's modification time in milliseconds since the Unix epoch.
    pub mtime_ms: Signed,
    /// The encoding text chunks decode from; null for a byte read or an undecoded binary file.
    #[serde(deserialize_with = "nullable")]
    pub encoding: Option<EncodingName>,
    /// The content starts with a byte order mark, which chunks omit.
    pub bom: bool,
    /// The content is binary; without `forceText` no chunk follows.
    pub binary: bool,
}

/// `fs.guess-encoding`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GuessEncodingResult {
    /// The guessed encoding; null when the content suggests none.
    #[serde(deserialize_with = "nullable")]
    pub encoding: Option<EncodingName>,
}

/// One column of a porcelain v2 status code.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum StatusCode {
    /// Unmodified.
    #[serde(rename = ".")]
    Unmodified,
    /// Modified.
    #[serde(rename = "M")]
    Modified,
    /// Type changed.
    #[serde(rename = "T")]
    TypeChanged,
    /// Added.
    #[serde(rename = "A")]
    Added,
    /// Deleted.
    #[serde(rename = "D")]
    Deleted,
    /// Renamed.
    #[serde(rename = "R")]
    Renamed,
    /// Copied.
    #[serde(rename = "C")]
    Copied,
    /// Updated but unmerged.
    #[serde(rename = "U")]
    Unmerged,
    /// Untracked.
    #[serde(rename = "?")]
    Untracked,
    /// Ignored.
    #[serde(rename = "!")]
    Ignored,
}

/// One status entry, in porcelain order.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StatusEntry {
    /// The index column.
    pub x: StatusCode,
    /// The working-tree column.
    pub y: StatusCode,
    /// The path.
    pub path: FilePath,
    /// The source of a rename or copy.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub original: Option<FilePath>,
    /// The path is a submodule.
    pub submodule: bool,
}

/// `git.status-page`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitStatusPageResult {
    /// The page's entries.
    pub entries: List<StatusEntry, 0, 2000>,
    /// The next page's continuation; absent on the last page.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub cursor: Option<PageCursor>,
}

/// `git.check-ignore`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitCheckIgnoreResult {
    /// The requested paths the repository ignores.
    pub ignored: List<FilePath, 0, 1000>,
}

/// What a reference is.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RefKind {
    /// A local branch.
    Head,
    /// A remote-tracking branch.
    RemoteHead,
    /// A tag.
    Tag,
}

/// A branch's upstream.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RefUpstream {
    /// The remote.
    pub remote: GitName,
    /// The branch on the remote.
    pub name: GitName,
}

/// One reference.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitRef {
    /// The short name.
    pub name: Text<1, 1024>,
    /// What the reference is.
    #[serde(rename = "type")]
    pub kind: RefKind,
    /// The commit it names, peeled.
    pub commit: CommitName,
    /// A local branch's upstream.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub upstream: Option<RefUpstream>,
}

/// One remote.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitRemote {
    /// The name.
    pub name: GitName,
    /// The fetch URL.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub fetch_url: Option<Text<1, 4096>>,
    /// The push URL.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub push_url: Option<Text<1, 4096>>,
}

/// `git.refs`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitRefsResult {
    /// The page's references.
    pub refs: List<GitRef, 0, 2000>,
    /// Every remote on the first page; empty on later pages.
    pub remotes: List<GitRemote, 0, 256>,
    /// The next page's continuation; absent on the last page.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub cursor: Option<PageCursor>,
}

/// `git.user-config`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitUserConfigResult {
    /// `user.name`, when set.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub name: Option<Text<0, 1024>>,
    /// `user.email`, when set.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub email: Option<Text<0, 1024>>,
}

/// `git.output`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitOutputResult {
    /// Command lines and output from the cursor on, as the ring holds them.
    pub text: Text<0, 131_072>,
    /// Where the next read continues.
    pub cursor: Counter,
    /// Bytes the ring overwrote between the requested cursor and `text`.
    pub dropped: Counter,
}

/// One path's answer: the path as requested, and why the operation failed for it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PathItem {
    /// The path as requested.
    pub path: FilePath,
    /// Why the operation failed for it.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub error: Option<FilesError>,
}

/// `git.clean` and `fs.restore`: one answer per path the request named, or the stash held.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PathItemsResult {
    /// The answers, in request order.
    pub items: List<PathItem, 1, 256>,
}

/// `git.commit`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitCommitResult {
    /// The new commit.
    pub commit: CommitName,
}

/// `git.init`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitInitResult {
    /// The new repository's canonical toplevel path.
    pub repository: FilePath,
}

/// A mutation's recorded reply, which `request.outcome` returns. Each variant is a shape of JSON,
/// read in this order; the requester knows which operation it named.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum MutationReply {
    /// The mutation failed.
    Error(ErrorReply),
    /// `git.clean`, `fs.restore`, and an `fs.delete` that kept no stash.
    PathItems(PathItemsResult),
    /// An `fs.delete` that kept a stash.
    FsDelete(FsDeleteResult),
    /// `git.commit`.
    GitCommit(GitCommitResult),
    /// `git.init`.
    GitInit(GitInitResult),
    /// `fs.write`.
    FsWrite(FsWriteResult),
    /// `fs.create` and `fs.rename`.
    FsEntry(FsEntryResult),
    /// `fs.move` and `fs.copy`.
    FsTransfer(FsTransferResult),
    /// `fs.import-begin`.
    FsImport(FsImportResult),
    /// Every other mutation.
    Empty(Empty),
}

impl From<MutationReply> for FilesReply {
    /// The Result record that repeats a recorded reply.
    fn from(reply: MutationReply) -> Self {
        match reply {
            MutationReply::Error(reply) => Self::Error(reply.error),
            MutationReply::PathItems(result) => Self::GitClean(result),
            MutationReply::FsDelete(result) => Self::FsDelete(result),
            MutationReply::GitCommit(result) => Self::GitCommit(result),
            MutationReply::GitInit(result) => Self::GitInit(result),
            MutationReply::FsWrite(result) => Self::FsWrite(result),
            MutationReply::FsEntry(result) => Self::FsEntry(result),
            MutationReply::FsTransfer(result) => Self::FsTransfer(result),
            MutationReply::FsImport(result) => Self::FsImport(result),
            MutationReply::Empty(_) => Self::Empty,
        }
    }
}

/// Whether a mutation ran.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MutationState {
    /// It ended with this reply.
    Completed(MutationReply),
    /// It is running.
    Running,
    /// The process does not know it: it never arrived, its record expired, or a crash lost it.
    Unknown,
}

/// `request.outcome`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "WireOutcome", into = "WireOutcome")]
pub struct RequestOutcomeResult {
    /// Whether the mutation ran.
    pub state: MutationState,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum WireState {
    Completed,
    Running,
    Unknown,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireOutcome {
    state: WireState,
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    result: Option<MutationReply>,
}

impl TryFrom<WireOutcome> for RequestOutcomeResult {
    type Error = InvalidValue;

    fn try_from(wire: WireOutcome) -> Result<Self, InvalidValue> {
        let state = match (wire.state, wire.result) {
            (WireState::Completed, Some(reply)) => MutationState::Completed(reply),
            (WireState::Running, None) => MutationState::Running,
            (WireState::Unknown, None) => MutationState::Unknown,
            _ => return Err(InvalidValue),
        };
        Ok(Self { state })
    }
}

impl From<RequestOutcomeResult> for WireOutcome {
    fn from(outcome: RequestOutcomeResult) -> Self {
        match outcome.state {
            MutationState::Completed(reply) => Self {
                state: WireState::Completed,
                result: Some(reply),
            },
            MutationState::Running => Self {
                state: WireState::Running,
                result: None,
            },
            MutationState::Unknown => Self {
                state: WireState::Unknown,
                result: None,
            },
        }
    }
}

/// One Result record's JSON body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FilesReply {
    /// Any operation failed.
    Error(FilesError),
    /// An operation that reports only its success.
    Empty,
    /// `fs.list`.
    FsList(FsListResult),
    /// `fs.stat`.
    FsStat(FsStatResult),
    /// `fs.grant`.
    FsGrant(FsGrantResult),
    /// `fs.read` and `git.show`.
    Read(ReadResult),
    /// `fs.guess-encoding`.
    GuessEncoding(GuessEncodingResult),
    /// `git.status-page`.
    GitStatusPage(GitStatusPageResult),
    /// `git.check-ignore`.
    GitCheckIgnore(GitCheckIgnoreResult),
    /// `git.refs`.
    GitRefs(GitRefsResult),
    /// `git.user-config`.
    GitUserConfig(GitUserConfigResult),
    /// `git.output`.
    GitOutput(GitOutputResult),
    /// `git.clean`.
    GitClean(PathItemsResult),
    /// `git.commit`.
    GitCommit(GitCommitResult),
    /// `git.init`.
    GitInit(GitInitResult),
    /// `request.outcome`.
    RequestOutcome(RequestOutcomeResult),
    /// `fs.write`.
    FsWrite(FsWriteResult),
    /// `fs.create` and `fs.rename`.
    FsEntry(FsEntryResult),
    /// `fs.move` and `fs.copy`.
    FsTransfer(FsTransferResult),
    /// `fs.exists`.
    FsExists(FsExistsResult),
    /// `fs.delete`.
    FsDelete(FsDeleteResult),
    /// `fs.restore`.
    FsRestore(PathItemsResult),
    /// `fs.import-begin` and `fs.import-items`.
    FsImport(FsImportResult),
    /// `fs.import-file`.
    FsImportFile(FsImportFileResult),
    /// `fs.export-entries`.
    FsExportEntries(FsExportEntriesResult),
    /// `picker.open`.
    PickerOpen(PickerOpenResult),
    /// `picker.list`.
    PickerList(PickerListResult),
    /// `search.next`.
    SearchNext(SearchNextResult),
    /// `search.find-files`.
    SearchFindFiles(SearchFindFilesResult),
    /// `git.log`.
    GitLog(GitLogResult),
    /// `git.blame`.
    GitBlame(GitBlameResult),
    /// `git.changes`.
    GitChanges(GitChangesResult),
    /// `git.merge-base`.
    GitMergeBase(GitMergeBaseResult),
    /// `git.stashes`.
    GitStashes(GitStashesResult),
    /// `history.list`.
    HistoryList(HistoryListResult),
}

/// Reads the JSON body of the Result record that answers a request of `operation`: its error, or
/// its result. A subscription and `fs.read-chunk` answer with a Result only to fail.
///
/// # Errors
///
/// [`InvalidValue`] for JSON that is neither, malformed JSON and duplicate keys included.
pub fn parse_reply(operation: Operation, json: &[u8]) -> Result<FilesReply, InvalidValue> {
    if let Ok(reply) = read_object::<ErrorReply>(json) {
        return Ok(FilesReply::Error(reply.error));
    }
    Ok(match operation {
        Operation::FsSubscribe
        | Operation::ScmSubscribe
        | Operation::RelaySubscribe
        | Operation::FsReadChunk => return Err(InvalidValue),
        Operation::FsRelease
        | Operation::FsUngrant
        | Operation::TransferClose
        | Operation::GitStage
        | Operation::GitUnstage
        | Operation::GitDiscard
        | Operation::GitStageContent
        | Operation::GitFetch
        | Operation::GitPull
        | Operation::GitSync
        | Operation::GitPush
        | Operation::GitMarkSafe
        | Operation::RepositoryConsent
        | Operation::RepositoryClose
        | Operation::RelayAnswer
        | Operation::ClientFocus
        | Operation::FsClipboard
        | Operation::FsImportEnd
        | Operation::FsExportBegin
        | Operation::PickerClose
        | Operation::SearchText
        | Operation::SearchCloseCache
        | Operation::GitCheckout
        | Operation::GitBranchCreate
        | Operation::GitBranchDelete
        | Operation::GitBranchRename
        | Operation::GitBranchUpstream
        | Operation::GitBranchDeleteRemote
        | Operation::GitMerge
        | Operation::GitRebase
        | Operation::GitCherryPick
        | Operation::GitAbort
        | Operation::GitUndoCommit
        | Operation::GitStashPush
        | Operation::GitStashApply
        | Operation::GitStashPop
        | Operation::GitStashDrop
        | Operation::GitStashClear
        | Operation::GitTagCreate
        | Operation::GitTagDelete
        | Operation::GitTagDeleteRemote
        | Operation::GitRemoteAdd
        | Operation::GitRemoteRemove
        | Operation::HistoryRestore
        | Operation::HistoryRename
        | Operation::HistoryDelete => read_object::<Empty>(json).map(|_| FilesReply::Empty)?,
        Operation::FsList => FilesReply::FsList(read_object(json)?),
        Operation::FsStat => FilesReply::FsStat(read_object(json)?),
        Operation::FsGrant => FilesReply::FsGrant(read_object(json)?),
        Operation::FsRead | Operation::GitShow => FilesReply::Read(read_object(json)?),
        Operation::FsGuessEncoding => FilesReply::GuessEncoding(read_object(json)?),
        Operation::GitStatusPage => FilesReply::GitStatusPage(read_object(json)?),
        Operation::GitCheckIgnore => FilesReply::GitCheckIgnore(read_object(json)?),
        Operation::GitRefs => FilesReply::GitRefs(read_object(json)?),
        Operation::GitUserConfig => FilesReply::GitUserConfig(read_object(json)?),
        Operation::GitOutput => FilesReply::GitOutput(read_object(json)?),
        Operation::GitClean => FilesReply::GitClean(read_object(json)?),
        Operation::GitCommit => FilesReply::GitCommit(read_object(json)?),
        Operation::GitInit => FilesReply::GitInit(read_object(json)?),
        Operation::RequestOutcome => FilesReply::RequestOutcome(read_object(json)?),
        Operation::FsWrite => FilesReply::FsWrite(read_object(json)?),
        Operation::FsCreate | Operation::FsRename => FilesReply::FsEntry(read_object(json)?),
        Operation::FsMove | Operation::FsCopy => FilesReply::FsTransfer(read_object(json)?),
        Operation::FsExists => FilesReply::FsExists(read_object(json)?),
        Operation::FsDelete => FilesReply::FsDelete(read_object(json)?),
        Operation::FsRestore => FilesReply::FsRestore(read_object(json)?),
        Operation::FsImportBegin | Operation::FsImportItems => {
            FilesReply::FsImport(read_object(json)?)
        }
        Operation::FsImportFile => FilesReply::FsImportFile(read_object(json)?),
        Operation::FsExportEntries => FilesReply::FsExportEntries(read_object(json)?),
        Operation::PickerOpen => FilesReply::PickerOpen(read_object(json)?),
        Operation::PickerList => FilesReply::PickerList(read_object(json)?),
        Operation::SearchNext => FilesReply::SearchNext(read_object(json)?),
        Operation::SearchFindFiles => FilesReply::SearchFindFiles(read_object(json)?),
        Operation::GitLog => FilesReply::GitLog(read_object(json)?),
        Operation::GitBlame => FilesReply::GitBlame(read_object(json)?),
        Operation::GitChanges => FilesReply::GitChanges(read_object(json)?),
        Operation::GitMergeBase => FilesReply::GitMergeBase(read_object(json)?),
        Operation::GitStashes => FilesReply::GitStashes(read_object(json)?),
        Operation::HistoryList => FilesReply::HistoryList(read_object(json)?),
    })
}

/// The canonical JSON of `reply`.
#[must_use]
pub fn encode_reply(reply: &FilesReply) -> Vec<u8> {
    let encoded = match reply {
        FilesReply::Error(error) => serde_json::to_vec(&ErrorReply {
            error: error.clone(),
        }),
        FilesReply::Empty => serde_json::to_vec(&Empty {}),
        FilesReply::FsList(result) => serde_json::to_vec(result),
        FilesReply::FsStat(result) => serde_json::to_vec(result),
        FilesReply::FsGrant(result) => serde_json::to_vec(result),
        FilesReply::Read(result) => serde_json::to_vec(result),
        FilesReply::GuessEncoding(result) => serde_json::to_vec(result),
        FilesReply::GitStatusPage(result) => serde_json::to_vec(result),
        FilesReply::GitCheckIgnore(result) => serde_json::to_vec(result),
        FilesReply::GitRefs(result) => serde_json::to_vec(result),
        FilesReply::GitUserConfig(result) => serde_json::to_vec(result),
        FilesReply::GitOutput(result) => serde_json::to_vec(result),
        FilesReply::GitClean(result) => serde_json::to_vec(result),
        FilesReply::GitCommit(result) => serde_json::to_vec(result),
        FilesReply::GitInit(result) => serde_json::to_vec(result),
        FilesReply::RequestOutcome(result) => serde_json::to_vec(result),
        FilesReply::FsWrite(result) => serde_json::to_vec(result),
        FilesReply::FsEntry(result) => serde_json::to_vec(result),
        FilesReply::FsTransfer(result) => serde_json::to_vec(result),
        FilesReply::FsExists(result) => serde_json::to_vec(result),
        FilesReply::FsDelete(result) => serde_json::to_vec(result),
        FilesReply::FsRestore(result) => serde_json::to_vec(result),
        FilesReply::FsImport(result) => serde_json::to_vec(result),
        FilesReply::FsImportFile(result) => serde_json::to_vec(result),
        FilesReply::FsExportEntries(result) => serde_json::to_vec(result),
        FilesReply::PickerOpen(result) => serde_json::to_vec(result),
        FilesReply::PickerList(result) => serde_json::to_vec(result),
        FilesReply::SearchNext(result) => serde_json::to_vec(result),
        FilesReply::SearchFindFiles(result) => serde_json::to_vec(result),
        FilesReply::GitLog(result) => serde_json::to_vec(result),
        FilesReply::GitBlame(result) => serde_json::to_vec(result),
        FilesReply::GitChanges(result) => serde_json::to_vec(result),
        FilesReply::GitMergeBase(result) => serde_json::to_vec(result),
        FilesReply::GitStashes(result) => serde_json::to_vec(result),
        FilesReply::HistoryList(result) => serde_json::to_vec(result),
    };
    encoded.expect("every reply value encodes as JSON")
}
