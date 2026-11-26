//! Workspace file and Git facts for the right bar: one directory listing, one chunk of a file's
//! content from the working tree, the index or `HEAD`, or the Workspace's change list; as a State
//! stream, how far what the right bar shows has changed since; or, as an Events stream, the
//! matches of a text search, page by page as they are found. The writing operations save a
//! working-tree file only while the file is still the version the client read, replace the
//! matches a search found only while the files still hold them, and stage, unstage, discard and
//! commit changes with Git. Paths are
//! Workspace-relative and `/`-separated; the Server resolves the Workspace's own directory, so a
//! client never names an absolute path.
use crate::{
    ContractError, ProtocolVersion, SessionId, WorkbenchRequestId, WorkbenchRevision,
    WorkbenchWorkspaceId,
};
use serde::{Deserialize, Serialize};

/// The encoded request stays small: one relative path and a closed operation, or the paths a
/// watch follows.
pub const MAXIMUM_WORKSPACE_FILES_REQUEST_BYTES: usize = 64 * 1024;
/// An encoded `write-file` request: the largest editable text, room for its JSON escapes, and the
/// identity around it. A client checks the encoded size before it asks.
pub const MAXIMUM_WORKSPACE_FILES_WRITE_REQUEST_BYTES: usize = 24 * 1024 * 1024;
/// The encoded result fits the general Workbench response budget.
pub const MAXIMUM_WORKSPACE_FILES_RESULT_BYTES: usize = 8 * 1024 * 1024;
/// Workspace-relative path bytes.
pub const MAXIMUM_WORKSPACE_FILE_PATH_BYTES: usize = 4096;
/// One directory entry's name bytes.
pub const MAXIMUM_WORKSPACE_FILE_NAME_BYTES: usize = 1024;
/// Entries one listing or change list carries before it reports truncation.
pub const MAXIMUM_WORKSPACE_FILE_ENTRIES: usize = 5000;
/// Content bytes one file read returns; a client reads a larger file chunk by chunk.
pub const WORKSPACE_FILE_CHUNK_BYTES: usize = 256 * 1024;
/// The largest working-tree file that opens as text at all; larger ones answer `too-large`.
pub const MAXIMUM_WORKSPACE_FILE_OPEN_BYTES: u64 = 50 * 1024 * 1024;
/// The largest file that carries a content hash in its version, opens for editing, or reads from
/// the index or `HEAD` for a diff.
pub const MAXIMUM_WORKSPACE_FILE_EDITABLE_BYTES: u64 = 10 * 1024 * 1024;
/// The largest file that reads as bytes, such as an image to show.
pub const MAXIMUM_WORKSPACE_FILE_BYTES_READ: u64 = 10 * 1024 * 1024;
/// Directories one watch names: the ones the right bar shows expanded.
pub const MAXIMUM_WORKSPACE_FILES_WATCHED_DIRECTORIES: usize = 256;
/// Files one watch names: the ones open from the right bar.
pub const MAXIMUM_WORKSPACE_FILES_WATCHED_FILES: usize = 64;
/// Paths one Git operation names; a client sends a longer list in several requests.
pub const MAXIMUM_WORKSPACE_GIT_PATHS: usize = 256;
/// A commit message's bytes.
pub const MAXIMUM_WORKSPACE_GIT_MESSAGE_BYTES: usize = 32 * 1024;
/// The bytes of Git's own output a refused Git operation reports.
pub const MAXIMUM_WORKSPACE_GIT_OUTPUT_BYTES: usize = 8 * 1024;
/// An encoded `search` request: the files it may be limited to and the identity around it. A
/// client leaves out the files that would not fit.
pub const MAXIMUM_WORKSPACE_FILES_SEARCH_REQUEST_BYTES: usize = 2 * 1024 * 1024;
/// A search pattern's bytes.
pub const MAXIMUM_WORKSPACE_SEARCH_PATTERN_BYTES: usize = 8 * 1024;
/// Glob patterns one search includes, and as many again that it excludes.
pub const MAXIMUM_WORKSPACE_SEARCH_GLOBS: usize = 64;
/// One glob pattern's bytes.
pub const MAXIMUM_WORKSPACE_SEARCH_GLOB_BYTES: usize = 1024;
/// Files a search limited to named files names: the files open, or the changed ones.
pub const MAXIMUM_WORKSPACE_SEARCH_PATHS: usize = MAXIMUM_WORKSPACE_FILE_ENTRIES;
/// Matches one search reports; at this many it stops and says the limit was reached.
pub const MAXIMUM_WORKSPACE_SEARCH_MATCHES: u64 = 20_000;
/// UTF-16 code units one match preview holds; a longer line shows the part around its matches.
pub const MAXIMUM_WORKSPACE_SEARCH_PREVIEW_UNITS: usize = 1000;
/// A replacement's bytes.
pub const MAXIMUM_WORKSPACE_REPLACEMENT_BYTES: usize = MAXIMUM_WORKSPACE_SEARCH_PATTERN_BYTES;
/// Files one `replace-matches` request names; a client replaces in more files with several
/// requests.
pub const MAXIMUM_WORKSPACE_REPLACE_FILES: usize = MAXIMUM_WORKSPACE_GIT_PATHS;
/// Matches one `replace-matches` request names across its files: every match one search reports.
pub const MAXIMUM_WORKSPACE_REPLACE_MATCHES: usize = MAXIMUM_WORKSPACE_SEARCH_MATCHES as usize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceFileSource {
    Worktree,
    Index,
    Head,
}

/// How a read returns content: text that must be UTF-8, or bytes in base64, such as an image.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceFileEncoding {
    Text,
    Bytes,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorkspaceFilesOperation {
    ListDirectory {
        path: String,
    },
    /// One chunk of a file from `offset`, a byte position a previous chunk named.
    ReadFile {
        path: String,
        source: WorkspaceFileSource,
        offset: u64,
        encoding: WorkspaceFileEncoding,
    },
    GitStatus {},
    /// Follows changes as a State stream: each frame carries the Workspace's revisions, and the
    /// Server sends a new frame when one moves. `directories` are the ones shown, the empty path
    /// naming the Workspace itself, and `files` the ones open in the working tree. A platform that
    /// watches recursively also sees changes beneath directories nobody shows, which move only
    /// `repository`.
    Watch {
        directories: Vec<String>,
        files: Vec<String>,
    },
    /// Replaces a working-tree file's whole content with `text`, but only while the file is still
    /// the one the client read: `expected` is that read's SHA-256, or null for a file that must
    /// not exist yet. Any other file answers `conflict` and is left as it was.
    WriteFile {
        path: String,
        text: String,
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        expected: Option<String>,
    },
    /// Stages `paths` as they are in the working tree, a deletion included; staging a conflict
    /// marks it resolved. Null stages every change in the Workspace's directory that is not a
    /// conflict, untracked files included.
    GitStage {
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        paths: Option<Vec<String>>,
    },
    /// Takes `paths` out of the index, back to what `HEAD` holds; null takes out everything
    /// staged in the Workspace's directory.
    GitUnstage {
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        paths: Option<Vec<String>>,
    },
    /// Puts `paths` back to what the index holds: each untracked file goes to the system trash,
    /// where it can be restored, and then Git checks the tracked ones out of the index, dropping
    /// their unstaged changes. Git refuses the whole checkout, changing nothing, when a path is a
    /// conflict or not known to it; the files already in the trash stay there.
    GitDiscard {
        paths: Vec<String>,
    },
    /// Commits what is staged with `message`, running the repository's hooks. `stage_all` first
    /// stages what a null `git-stage` would. `amend` replaces the last commit instead; an empty
    /// message then keeps that commit's own.
    GitCommit {
        message: String,
        amend: bool,
        stage_all: bool,
    },
    /// Searches the Workspace's files for `query`, sent only as an Events stream: the first frame
    /// starts the search and each later one carries the matches found since, until a frame's
    /// state says the search ended. Closing the stream stops the search, as does a later search of
    /// the same Workspace on the same connection.
    Search {
        query: WorkspaceSearchQuery,
    },
    /// Replaces matches a search found in `files`, each named by where it starts, with
    /// `query.replacement`. A file is searched again for `query`, and written only while every
    /// match named still starts where it was found and the file did not change while it was
    /// read; any other file is left as it was. Files are replaced one at a time, in order, and
    /// the answer says for each what became of it.
    ReplaceMatches {
        query: WorkspaceReplaceQuery,
        files: Vec<WorkspaceReplaceFile>,
    },
}

/// What one search looks for and where.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceSearchQuery {
    /// The text to find, or a regular expression when `regex` is set; a line break in it lets a
    /// match span lines.
    pub pattern: String,
    pub regex: bool,
    pub match_case: bool,
    /// Matches only where the pattern starts and ends at word boundaries.
    pub whole_word: bool,
    /// Glob patterns the files searched must match, none meaning every file. A pattern starting
    /// with `./` names a path from the Workspace's directory, any other one matches at any depth,
    /// and a pattern that names a directory includes what lies in it.
    pub includes: Vec<String>,
    /// Glob patterns, read as `includes` are, of files left out.
    pub excludes: Vec<String>,
    /// Whether Git's ignore rules and `.ignore` files leave files out.
    pub use_ignore_files: bool,
    /// When not null, only these files are searched: the files open, or the changed ones.
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub paths: Option<Vec<String>>,
}
impl WorkspaceSearchQuery {
    fn is_valid(&self) -> bool {
        !self.pattern.is_empty()
            && self.pattern.len() <= MAXIMUM_WORKSPACE_SEARCH_PATTERN_BYTES
            && is_glob_list(&self.includes)
            && is_glob_list(&self.excludes)
            && self.paths.as_deref().is_none_or(|paths| {
                paths.len() <= MAXIMUM_WORKSPACE_SEARCH_PATHS
                    && paths.iter().all(|path| is_relative_path(path))
            })
    }
}

/// What a replacement looks for, as the search that found the matches did, and what it writes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceReplaceQuery {
    /// The search's text or regular expression.
    pub pattern: String,
    pub regex: bool,
    pub match_case: bool,
    pub whole_word: bool,
    /// What each match becomes. With `regex`, `$1` to `$99` and `${name}` insert a group, `$0`
    /// and `$&` the whole match and `$$` a dollar sign; `\n`, `\t` and `\\` insert a line feed, a
    /// tab and a backslash; and `\u`, `\l`, `\U` and `\L` make the next group's first letter, or
    /// all of it, upper or lower case. Without `regex` it is written as it stands.
    pub replacement: String,
    /// Writes a replacement without groups in the case of the text it replaces: all upper, all
    /// lower or with a capital first letter, part by part when both hold the same hyphens or
    /// underscores.
    pub preserve_case: bool,
}

/// A file's matches to replace.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceReplaceFile {
    pub path: String,
    /// Where each match starts, in order through the file and apart.
    pub matches: Vec<WorkspaceReplaceMatch>,
}

/// Where a match starts: the line counted from 1 and the UTF-16 code units of that line before
/// it, as a search reports `line` and `preview_start` plus the match's `start`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceReplaceMatch {
    pub line: u64,
    pub column: u32,
}

impl WorkspaceReplaceQuery {
    fn is_valid(&self) -> bool {
        !self.pattern.is_empty()
            && self.pattern.len() <= MAXIMUM_WORKSPACE_SEARCH_PATTERN_BYTES
            && self.replacement.len() <= MAXIMUM_WORKSPACE_REPLACEMENT_BYTES
    }
}

/// One to [`MAXIMUM_WORKSPACE_REPLACE_FILES`] distinct files, each with matches in order, and at
/// most [`MAXIMUM_WORKSPACE_REPLACE_MATCHES`] matches in all.
fn is_replace_list(files: &[WorkspaceReplaceFile]) -> bool {
    let mut paths: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
    paths.sort_unstable();
    paths.dedup();
    !files.is_empty()
        && files.len() <= MAXIMUM_WORKSPACE_REPLACE_FILES
        && paths.len() == files.len()
        && files.iter().map(|file| file.matches.len()).sum::<usize>()
            <= MAXIMUM_WORKSPACE_REPLACE_MATCHES
        && files.iter().all(|file| {
            is_relative_path(&file.path)
                && !file.matches.is_empty()
                && file.matches.first().is_some_and(|first| first.line >= 1)
                && file.matches.windows(2).all(|pair| pair[0] < pair[1])
        })
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceFilesRequest {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    #[serde(rename = "sessionId")]
    pub session_id: SessionId,
    pub workspace_id: WorkbenchWorkspaceId,
    pub operation: WorkspaceFilesOperation,
}
impl WorkspaceFilesRequest {
    pub fn validate(&self) -> Result<(), ContractError> {
        self.operation.validate()
    }
}
impl WorkspaceFilesOperation {
    pub fn validate(&self) -> Result<(), ContractError> {
        match self {
            Self::ListDirectory { path } if path.is_empty() || is_relative_path(path) => Ok(()),
            Self::ReadFile { path, .. } if is_relative_path(path) => Ok(()),
            Self::GitStatus {} => Ok(()),
            Self::Watch { directories, files }
                if directories.len() <= MAXIMUM_WORKSPACE_FILES_WATCHED_DIRECTORIES
                    && files.len() <= MAXIMUM_WORKSPACE_FILES_WATCHED_FILES
                    && directories
                        .iter()
                        .all(|path| path.is_empty() || is_relative_path(path))
                    && files.iter().all(|path| is_relative_path(path)) =>
            {
                Ok(())
            }
            Self::WriteFile {
                path,
                text,
                expected,
            } if is_relative_path(path)
                && text.len() as u64 <= MAXIMUM_WORKSPACE_FILE_EDITABLE_BYTES
                && expected.as_deref().is_none_or(is_sha256) =>
            {
                Ok(())
            }
            Self::GitStage { paths } | Self::GitUnstage { paths }
                if paths.as_deref().is_none_or(is_path_list) =>
            {
                Ok(())
            }
            Self::GitDiscard { paths } if is_path_list(paths) => Ok(()),
            Self::GitCommit { message, amend, .. }
                if message.len() <= MAXIMUM_WORKSPACE_GIT_MESSAGE_BYTES
                    && (*amend || !message.trim().is_empty()) =>
            {
                Ok(())
            }
            Self::Search { query } if query.is_valid() => Ok(()),
            Self::ReplaceMatches { query, files } if query.is_valid() && is_replace_list(files) => {
                Ok(())
            }
            Self::ListDirectory { .. }
            | Self::ReadFile { .. }
            | Self::Watch { .. }
            | Self::WriteFile { .. }
            | Self::GitStage { .. }
            | Self::GitUnstage { .. }
            | Self::GitDiscard { .. }
            | Self::GitCommit { .. }
            | Self::Search { .. }
            | Self::ReplaceMatches { .. } => Err(ContractError::InvalidDocument),
        }
    }
    /// Whether the operation changes the Workspace, which needs more than reading permission.
    #[must_use]
    pub fn writes(&self) -> bool {
        matches!(self, Self::WriteFile { .. }) || self.changes_at_length()
    }
    /// Whether the operation is a change that may run for up to a minute: a Git change, whose
    /// hooks may run that long, or replacing matches across many files. It waits in a queue of its
    /// own and never holds up a read.
    #[must_use]
    pub fn changes_at_length(&self) -> bool {
        self.runs_git_write() || matches!(self, Self::ReplaceMatches { .. })
    }
    /// Whether the operation changes the repository through Git.
    #[must_use]
    pub fn runs_git_write(&self) -> bool {
        matches!(
            self,
            Self::GitStage { .. }
                | Self::GitUnstage { .. }
                | Self::GitDiscard { .. }
                | Self::GitCommit { .. }
        )
    }
    /// The encoded size a request carrying this operation may reach.
    #[must_use]
    pub fn request_byte_limit(&self) -> usize {
        match self {
            Self::WriteFile { .. } => MAXIMUM_WORKSPACE_FILES_WRITE_REQUEST_BYTES,
            Self::Search { .. } | Self::ReplaceMatches { .. } => {
                MAXIMUM_WORKSPACE_FILES_SEARCH_REQUEST_BYTES
            }
            _ => MAXIMUM_WORKSPACE_FILES_REQUEST_BYTES,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceEntryKind {
    File,
    Directory,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceDirectoryEntry {
    pub name: String,
    /// What the entry resolves to; a symbolic link reports its target's kind.
    pub entry_kind: WorkspaceEntryKind,
    pub symbolic_link: bool,
    /// Whether the Workspace's Git ignore rules exclude the entry.
    pub ignored: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorkspaceFileContent {
    /// UTF-8 text from `offset`, ending on a character boundary; `next_offset` names where the
    /// next chunk starts, or is null at the end of the file.
    Text {
        offset: u64,
        text: String,
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        next_offset: Option<u64>,
    },
    /// Bytes from `offset` in standard padded base64.
    Bytes {
        offset: u64,
        base64: String,
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        next_offset: Option<u64>,
    },
    Binary {},
    TooLarge {},
    /// The source has no such file: an untracked file in `HEAD`, a deleted file in the tree.
    Absent {},
}

/// What a working-tree file was when it was read, so a later write can refuse to replace a newer
/// one and a chunked read can notice the file changing under it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceFileVersion {
    /// Modification time in nanoseconds since the Unix epoch, in decimal; `0` when unknown.
    pub modified: String,
    /// Lowercase hexadecimal SHA-256 of the whole file; only the first chunk of a file within
    /// [`MAXIMUM_WORKSPACE_FILE_EDITABLE_BYTES`] carries it.
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub sha256: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceGitStatusCode {
    Unmodified,
    Modified,
    TypeChanged,
    Added,
    Deleted,
    Renamed,
    Copied,
    Unmerged,
    Untracked,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceGitStatusEntry {
    pub path: String,
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub original_path: Option<String>,
    pub index: WorkspaceGitStatusCode,
    pub worktree: WorkspaceGitStatusCode,
    pub conflicted: bool,
}

/// Whether a watch follows changes. A client that reads `unavailable` refreshes only when asked.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceFilesWatchState {
    /// The Server has asked to watch and has no answer yet.
    Starting,
    Watching,
    /// The platform refused, or the Workspace holds more than the platform can watch.
    Unavailable,
}

/// Where a search stands.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceSearchState {
    Searching,
    /// Every file was searched.
    Complete,
    /// The search stopped at [`MAXIMUM_WORKSPACE_SEARCH_MATCHES`] matches.
    LimitReached,
    /// The pattern is not a regular expression the search can run.
    InvalidPattern,
    /// An include or exclude pattern is not a glob the search can read.
    InvalidGlob,
    /// The search could not run, or stopped before it searched every file.
    Unavailable,
}

/// One file's matches.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceSearchFile {
    pub path: String,
    /// In order through the file; a long line can show as several previews.
    pub lines: Vec<WorkspaceSearchLine>,
}

/// Matches that start on one line, with the text around them.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceSearchLine {
    /// The line the first match starts on, counted from 1.
    pub line: u64,
    /// UTF-16 code units of the line before the preview starts; more than zero when a long line
    /// shows only the part around its matches.
    pub preview_start: u32,
    /// The line from `preview_start`, or for a match spanning lines those lines joined by `\n`;
    /// bytes that are not UTF-8 read as U+FFFD. At most
    /// [`MAXIMUM_WORKSPACE_SEARCH_PREVIEW_UNITS`] UTF-16 code units.
    pub preview: String,
    /// Each match as UTF-16 code unit offsets into `preview`, in order and apart.
    pub matches: Vec<WorkspaceSearchRange>,
}

/// A span of UTF-16 code units, `start` included and `end` not.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceSearchRange {
    pub start: u32,
    pub end: u32,
}

/// What became of one file a replacement named.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceReplacedFile {
    pub path: String,
    pub result: WorkspaceReplaceResult,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorkspaceReplaceResult {
    /// Every match named was replaced; `version` is what the file is now, with its SHA-256.
    Replaced { version: WorkspaceFileVersion },
    /// The file no longer holds a match where the search found it, or changed while it was
    /// replaced, so nothing was written.
    Changed {},
    /// The file is not UTF-8 text, or it or the replaced text is larger than a file that opens for
    /// editing, so it was left as it was.
    NotEditable {},
    /// Not written for `reason`; `timeout` names a file the replacement had no time left to
    /// start.
    Failed { reason: WorkspaceFilesFailure },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceFilesFailure {
    Denied,
    Unavailable,
    NotFound,
    NotRepository,
    Timeout,
    Malformed,
    /// The system trash did not take every untracked file a discard named; those it did not take
    /// are where they were, and no tracked file was checked out.
    TrashUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorkspaceFilesOutcome {
    Directory {
        entries: Vec<WorkspaceDirectoryEntry>,
        truncated: bool,
    },
    File {
        /// The whole file's size, whichever chunk this is.
        byte_length: u64,
        /// Present for a working-tree file that exists.
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        version: Option<WorkspaceFileVersion>,
        content: WorkspaceFileContent,
    },
    GitStatus {
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        branch: Option<String>,
        entries: Vec<WorkspaceGitStatusEntry>,
        truncated: bool,
    },
    /// A watch's state. `files` moves when an entry of a shown directory, or an open file,
    /// changes; `repository` moves when anything in the working tree changes, or the repository's
    /// index, `HEAD` or references do. Both start again whenever the Server starts following the
    /// Workspace, so only a value that differs from the last frame means anything.
    Watching {
        files: WorkbenchRevision,
        repository: WorkbenchRevision,
        state: WorkspaceFilesWatchState,
    },
    /// The file now holds the written text; `version` is what a later write expects, and always
    /// carries its SHA-256.
    Written {
        version: WorkspaceFileVersion,
    },
    /// The file is no longer the version the write expected, so nothing was written; `version` is
    /// what it is now, or null when no file is there.
    Conflict {
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        version: Option<WorkspaceFileVersion>,
    },
    /// The Git operation ran to its end.
    GitDone {},
    /// Git refused, or a hook stopped it; `output` is the start of what Git and its hooks printed,
    /// lossily decoded and at most [`MAXIMUM_WORKSPACE_GIT_OUTPUT_BYTES`], and `truncated` says
    /// more was printed.
    GitRejected {
        output: String,
        truncated: bool,
    },
    /// One page of a search: the entries found since the last page, after the `from` entries
    /// earlier pages carried. A file with many matching lines comes as several entries for the
    /// same path that follow one another, possibly across pages, each continuing that file's
    /// lines; no other path comes between them. `more` says the Server already holds more, which
    /// the next page carries at once. Once `state` is no longer `searching`, no later page carries
    /// anything.
    SearchResults {
        from: u64,
        files: Vec<WorkspaceSearchFile>,
        more: bool,
        state: WorkspaceSearchState,
    },
    /// What became of each file a `replace-matches` named, in the order it named them.
    Replaced {
        files: Vec<WorkspaceReplacedFile>,
    },
    Failed {
        reason: WorkspaceFilesFailure,
    },
}
impl WorkspaceFilesOutcome {
    /// The invariants serde cannot express; a producer checks them before it answers.
    pub fn validate(&self) -> Result<(), ContractError> {
        let valid = match self {
            Self::Directory { entries, .. } => {
                entries.len() <= MAXIMUM_WORKSPACE_FILE_ENTRIES
                    && entries.iter().all(|entry| is_entry_name(&entry.name))
            }
            Self::File {
                byte_length,
                version,
                content,
            } => {
                version.as_ref().is_none_or(is_version)
                    && match content {
                        WorkspaceFileContent::Text {
                            offset,
                            text,
                            next_offset,
                        } => is_chunk(*byte_length, *offset, text.len(), *next_offset),
                        WorkspaceFileContent::Bytes {
                            offset,
                            base64,
                            next_offset,
                        } => base64_length(base64).is_some_and(|length| {
                            is_chunk(*byte_length, *offset, length, *next_offset)
                        }),
                        WorkspaceFileContent::Absent {} => *byte_length == 0 && version.is_none(),
                        WorkspaceFileContent::Binary {} | WorkspaceFileContent::TooLarge {} => true,
                    }
            }
            Self::GitStatus {
                branch, entries, ..
            } => {
                branch.as_deref().is_none_or(is_branch)
                    && entries.len() <= MAXIMUM_WORKSPACE_FILE_ENTRIES
                    && entries.iter().all(|entry| {
                        is_relative_path(&entry.path)
                            && entry.original_path.as_deref().is_none_or(is_relative_path)
                    })
            }
            Self::Written { version } => is_version(version) && version.sha256.is_some(),
            Self::Conflict { version } => version.as_ref().is_none_or(is_version),
            Self::GitRejected { output, .. } => output.len() <= MAXIMUM_WORKSPACE_GIT_OUTPUT_BYTES,
            Self::SearchResults { files, .. } => {
                files.len() <= MAXIMUM_WORKSPACE_FILE_ENTRIES
                    && files.iter().all(|file| {
                        is_relative_path(&file.path)
                            && !file.lines.is_empty()
                            && file.lines.iter().all(is_search_line)
                    })
            }
            Self::Replaced { files } => {
                files.len() <= MAXIMUM_WORKSPACE_REPLACE_FILES
                    && files.iter().all(|file| {
                        is_relative_path(&file.path)
                            && match &file.result {
                                WorkspaceReplaceResult::Replaced { version } => {
                                    is_version(version) && version.sha256.is_some()
                                }
                                WorkspaceReplaceResult::Changed {}
                                | WorkspaceReplaceResult::NotEditable {}
                                | WorkspaceReplaceResult::Failed { .. } => true,
                            }
                    })
            }
            Self::Watching { .. } | Self::GitDone {} | Self::Failed { .. } => true,
        };
        if valid {
            Ok(())
        } else {
            Err(ContractError::InvalidDocument)
        }
    }
    /// Whether this outcome is a possible answer to `operation`.
    pub fn answers(&self, operation: &WorkspaceFilesOperation) -> bool {
        if let (
            Self::Replaced { files: done },
            WorkspaceFilesOperation::ReplaceMatches { files, .. },
        ) = (self, operation)
        {
            return done.len() == files.len()
                && done
                    .iter()
                    .zip(files)
                    .all(|(done, asked)| done.path == asked.path);
        }
        if let (
            Self::File { content, .. },
            WorkspaceFilesOperation::ReadFile {
                offset, encoding, ..
            },
        ) = (self, operation)
        {
            return match content {
                WorkspaceFileContent::Text { offset: from, .. } => {
                    *encoding == WorkspaceFileEncoding::Text && from == offset
                }
                WorkspaceFileContent::Bytes { offset: from, .. } => {
                    *encoding == WorkspaceFileEncoding::Bytes && from == offset
                }
                WorkspaceFileContent::Binary {}
                | WorkspaceFileContent::TooLarge {}
                | WorkspaceFileContent::Absent {} => true,
            };
        }
        matches!(
            (self, operation),
            (Self::Failed { .. }, _)
                | (
                    Self::Directory { .. },
                    WorkspaceFilesOperation::ListDirectory { .. }
                )
                | (Self::File { .. }, WorkspaceFilesOperation::ReadFile { .. })
                | (
                    Self::GitStatus { .. },
                    WorkspaceFilesOperation::GitStatus {}
                )
                | (Self::Watching { .. }, WorkspaceFilesOperation::Watch { .. })
                | (
                    Self::SearchResults { .. },
                    WorkspaceFilesOperation::Search { .. }
                )
                | (
                    Self::Written { .. } | Self::Conflict { .. },
                    WorkspaceFilesOperation::WriteFile { .. }
                )
        ) || (matches!(self, Self::GitDone {} | Self::GitRejected { .. })
            && operation.runs_git_write())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceFilesResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    #[serde(rename = "sessionId")]
    pub session_id: SessionId,
    pub workspace_id: WorkbenchWorkspaceId,
    pub outcome: WorkspaceFilesOutcome,
}
impl WorkspaceFilesResult {
    pub fn answers(&self, request: &WorkspaceFilesRequest) -> bool {
        self.protocol_version == request.protocol_version
            && self.request_id == request.request_id
            && self.session_id == request.session_id
            && self.workspace_id == request.workspace_id
            && self.outcome.answers(&request.operation)
    }
    pub fn validate(&self) -> Result<(), ContractError> {
        self.outcome.validate()
    }
}

/// A Workspace-relative path: non-empty `/`-separated names, none of them `.` or `..`, no
/// leading or trailing separator, no control character and no backslash.
pub fn is_relative_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= MAXIMUM_WORKSPACE_FILE_PATH_BYTES
        && path.split('/').all(is_entry_name)
}

/// At most [`MAXIMUM_WORKSPACE_SEARCH_GLOBS`] non-empty glob patterns without control characters.
fn is_glob_list(globs: &[String]) -> bool {
    globs.len() <= MAXIMUM_WORKSPACE_SEARCH_GLOBS
        && globs.iter().all(|glob| {
            !glob.is_empty()
                && glob.len() <= MAXIMUM_WORKSPACE_SEARCH_GLOB_BYTES
                && !glob.chars().any(char::is_control)
        })
}

/// A preview within its size, on a line counted from 1, whose matches are non-empty, in order,
/// apart and inside it.
fn is_search_line(line: &WorkspaceSearchLine) -> bool {
    let Ok(units) = u32::try_from(line.preview.encode_utf16().count()) else {
        return false;
    };
    let mut end = 0;
    line.line >= 1
        && units as usize <= MAXIMUM_WORKSPACE_SEARCH_PREVIEW_UNITS
        && !line.matches.is_empty()
        && line.matches.iter().all(|range| {
            let apart = range.start >= end && range.start < range.end && range.end <= units;
            end = range.end;
            apart
        })
}

/// One to [`MAXIMUM_WORKSPACE_GIT_PATHS`] Workspace-relative paths.
fn is_path_list(paths: &[String]) -> bool {
    !paths.is_empty()
        && paths.len() <= MAXIMUM_WORKSPACE_GIT_PATHS
        && paths.iter().all(|path| is_relative_path(path))
}

fn is_entry_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAXIMUM_WORKSPACE_FILE_NAME_BYTES
        && name != "."
        && name != ".."
        && !name
            .chars()
            .any(|c| c == '/' || c == '\\' || c.is_control())
}

/// A chunk of `length` bytes at `offset` within a file of `byte_length`: at most one chunk long,
/// and either the end of the file or followed by the next chunk exactly where it stops.
fn is_chunk(byte_length: u64, offset: u64, length: usize, next_offset: Option<u64>) -> bool {
    let Ok(length) = u64::try_from(length) else {
        return false;
    };
    let Some(end) = offset.checked_add(length) else {
        return false;
    };
    length <= WORKSPACE_FILE_CHUNK_BYTES as u64
        && end <= byte_length
        && match next_offset {
            None => end == byte_length,
            Some(next) => next == end && length > 0 && next < byte_length,
        }
}

/// The byte count standard padded base64 decodes to, or none when it is not that encoding.
fn base64_length(encoded: &str) -> Option<usize> {
    let bytes = encoded.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return None;
    }
    let padding = bytes.iter().rev().take_while(|byte| **byte == b'=').count();
    let body = &bytes[..bytes.len() - padding];
    if padding > 2
        || !body
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'+' || *byte == b'/')
    {
        return None;
    }
    Some(bytes.len() / 4 * 3 - padding)
}

fn is_version(version: &WorkspaceFileVersion) -> bool {
    !version.modified.is_empty()
        && version.modified.len() <= 39
        && version.modified.bytes().all(|byte| byte.is_ascii_digit())
        && version.sha256.as_deref().is_none_or(is_sha256)
}

/// Lowercase hexadecimal SHA-256.
fn is_sha256(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_branch(branch: &str) -> bool {
    !branch.is_empty() && branch.len() <= 1024 && !branch.chars().any(char::is_control)
}
