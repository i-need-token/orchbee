//! The File Bar's wire contracts: OBFB, the records File Bar traffic carries inside the Client
//! multiplex, with the strict JSON of every `files`, `files-edit` and `files-search` operation,
//! and OBFL, the private link between a Server and the `orchbee-files` process it admitted. Values
//! are validated at decoding, so a decoded value is valid wherever the protocol carries it.
//! Callers own transport, deadlines, admission and scope; decoding a value grants no authority.

mod identity;
mod json;
mod link;
mod notice;
mod record;
mod reply;
mod request;
mod value;

pub use identity::{
    CacheId, CallId, ConnectionId, InvalidIdentity, LaneId, LinkToken, PromptId, RequestId,
    SearchId, SessionId, StashId, TransferId, WorkspaceId,
};
pub use link::{
    BuildIdentity, CallClose, DrainReason, FrameKind, GrantClass, LINK_MAGIC, LINK_VERSION,
    LinkError, LinkFrame, LinkHello, LinkMessage, LinkRefusal, LinkRequest, LinkSide,
    MAXIMUM_ENVIRONMENT_BYTES, MAXIMUM_LINK_PAYLOAD_BYTES, MAXIMUM_ROOT_BYTES,
    MAXIMUM_VERSION_BYTES, ScopeUpdate, WorkspaceScope, decode_link_message, encode_link_message,
    read_link_message, write_link_message,
};
pub use notice::{
    BusyOperation, Change, ChangeKind, FilesNotice, FsSnapshot, FsUpdate, GitProgram, GitState,
    Head, NoticeKind, OperatingSystem, Prompt, PromptKind, RelaySnapshot, RelayUpdate,
    RepositoryKind, RepositoryOperation, Root, ScmRepository, ScmSummary, Separator, StatusCounts,
    Upstream, WatchState, encode_notice, parse_notice,
};
pub use record::{
    Chunk, ChunkEnd, FilesRecord, LineEnding, MAXIMUM_DATA_BYTES, MAXIMUM_REPLY_BYTES,
    MAXIMUM_REQUEST_BYTES, Put, PutEnd, RECORD_MAGIC, RECORD_VERSION, RecordError, RecordPlace,
    check_files_record, decode_files_record, encode_files_record, is_files_record,
};
pub use reply::{
    Descendant, Empty, Entry, EntryKind, ErrorCode, ErrorReply, ExportEntry, ExportFacts,
    ExportKind, FilesError, FilesReply, FsDeleteResult, FsEntryResult, FsExistsResult,
    FsExportEntriesResult, FsGrantResult, FsImportFileResult, FsImportResult, FsListResult,
    FsStatResult, FsTransferResult, FsWriteResult, GitCheckIgnoreResult, GitCommitResult,
    GitInitResult, GitOutputResult, GitRef, GitRefsResult, GitRemote, GitStatusPageResult,
    GitUserConfigResult, GuessEncodingResult, ImportOutcome, MutationReply, MutationState,
    PathItem, PathItemsResult, PickerListResult, PickerOpenResult, ReadResult, RefKind,
    RefUpstream, RequestOutcomeResult, StatItem, StatusCode, StatusEntry, TransferItem,
    encode_reply, parse_reply,
};
pub use reply::{
    FoundFile, SearchContextLine, SearchFile, SearchFindFilesResult, SearchMatch, SearchNextResult,
    SearchRange, SearchSpan,
};
pub use request::{
    ChunkLength, CleanMode, ClientFocus, CommitAll, CommitMessage, Conflict, CreateKind,
    DeleteMode, FetchRemotes, FilesCapability, FilesRequest, FsClipboard, FsCreate, FsDelete,
    FsExists, FsExportBegin, FsExportEntries, FsGrant, FsImportBegin, FsImportEnd, FsImportFile,
    FsImportItems, FsList, FsRead, FsReadChunk, FsRename, FsRestore, FsTransfer, FsWrite,
    GitCheckIgnore, GitClean, GitCommit, GitFetch, GitOutput, GitPath, GitPaths, GitPull, GitPush,
    GitRefs, GitShow, GitStageContent, GitStatusPage, GrantSource, ImportItem, ImportKind,
    Operation, OperationStream, PathRequest, PathsRequest, PickerList, PickerRequest, PromptAnswer,
    PushForce, ReadMode, ReadSource, RelayAnswer, RepositoryRequest, RequestError, RequestOutcome,
    ShowRef, TransferRequest, WorkspaceRequest, WriteHistory, WritePrecondition, encode_request,
    parse_request,
};
pub use request::{
    FsStat, SearchCloseCache, SearchFindFiles, SearchFolder, SearchGlob, SearchPreview,
    SearchRequest, SearchText, StatSource,
};
pub use value::{
    CommitName, Counter, EncodingName, FileName, FilePath, GitName, Integer, InvalidValue, List,
    MAXIMUM_PATH_BYTES, MAXIMUM_SAFE_INTEGER, PageCursor, RelativePath, SafeInteger, Sha256Digest,
    Signed, Text, Token,
};
