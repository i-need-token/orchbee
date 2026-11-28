//! The requests of the `files-edit` operations: writes, file operations, delete and undo, clipboard
//! sources, import, export and the Save As picker. Values are validated here; whether a path lies
//! inside a Workspace's scope, and whether a name is valid on the machine, is decided by
//! `orchbee-files` when the operation starts.

use serde::{Deserialize, Serialize};

use crate::{
    identity::{RequestId, SessionId, StashId, TransferId, WorkspaceId},
    value::{
        EncodingName, FileName, FilePath, InvalidValue, List, PageCursor, RelativePath,
        SafeInteger, Sha256Digest, Signed, is_false, some,
    },
};

/// What a file must be for `fs.write` to replace it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WritePrecondition {
    /// The version the editor last read or wrote: the file's content must still have this
    /// digest, unless it already holds the new content. The time and size describe the same
    /// version.
    Version {
        /// Its modification time in milliseconds since the Unix epoch.
        mtime_ms: Signed,
        /// Its size in bytes.
        size: SafeInteger,
        /// The SHA-256 digest of its bytes.
        sha256: Sha256Digest,
    },
    /// No file may exist at the path; the write creates it.
    Create,
    /// Whatever is there is replaced.
    None,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WireVersion {
    mtime_ms: Signed,
    size: SafeInteger,
    sha256: Sha256Digest,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum WireCondition {
    Create,
    None,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(untagged)]
enum WirePrecondition {
    Version(WireVersion),
    Condition(WireCondition),
}

impl Serialize for WritePrecondition {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match *self {
            Self::Version {
                mtime_ms,
                size,
                sha256,
            } => WirePrecondition::Version(WireVersion {
                mtime_ms,
                size,
                sha256,
            }),
            Self::Create => WirePrecondition::Condition(WireCondition::Create),
            Self::None => WirePrecondition::Condition(WireCondition::None),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for WritePrecondition {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match WirePrecondition::deserialize(deserializer)? {
            WirePrecondition::Version(version) => Self::Version {
                mtime_ms: version.mtime_ms,
                size: version.size,
                sha256: version.sha256,
            },
            WirePrecondition::Condition(WireCondition::Create) => Self::Create,
            WirePrecondition::Condition(WireCondition::None) => Self::None,
        })
    }
}

/// Why a write happens, which Local History records once `files-history` serves it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WriteHistory {
    /// The editor saved.
    Save,
    /// An earlier version was restored.
    Restore,
}

/// `fs.write`; the new content follows as the transfer's Puts, `size` bytes of UTF-8 text, the
/// last flagged `commit`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FsWrite {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation's identity.
    pub request_id: RequestId,
    /// The transfer the Client chose for the content.
    pub transfer: TransferId,
    /// The file.
    pub path: FilePath,
    /// The UTF-8 bytes the Puts carry.
    pub size: SafeInteger,
    /// The encoding the file is written in.
    pub encoding: EncodingName,
    /// Start a UTF-8 file with a byte order mark; UTF-16 always starts with one and other
    /// encodings never do.
    #[serde(default, skip_serializing_if = "is_false")]
    pub bom: bool,
    /// What the file must be for the write to replace it.
    pub precondition: WritePrecondition,
    /// Clear the file's read-only attribute or owner write bit before writing.
    #[serde(default, skip_serializing_if = "is_false")]
    pub unlock: bool,
    /// Why the content is written; absent records no Local History entry.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub history: Option<WriteHistory>,
}

/// What `fs.create` makes.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CreateKind {
    /// An empty file.
    File,
    /// An empty directory.
    Directory,
}

/// `fs.create`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FsCreate {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation's identity.
    pub request_id: RequestId,
    /// The new item; missing directories above it are created.
    pub path: FilePath,
    /// What it is.
    #[serde(rename = "type")]
    pub kind: CreateKind,
}

/// `fs.rename`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FsRename {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation's identity.
    pub request_id: RequestId,
    /// The item.
    pub from: FilePath,
    /// Its new path; missing directories above it are created.
    pub to: FilePath,
}

/// What an operation does where its target already exists.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Conflict {
    /// The item fails with `exists`.
    Fail,
    /// The existing item is removed permanently first.
    Replace,
    /// The item takes VS Code's next free `copy` name.
    Increment,
    /// A directory onto an existing directory combines their contents, each child by the same
    /// rule; anything else is replaced.
    Merge,
}

/// `fs.move` and `fs.copy`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FsTransfer {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation's identity.
    pub request_id: RequestId,
    /// 1 to 256 items.
    pub sources: List<FilePath, 1, 256>,
    /// The directory they go into, each under its own name.
    pub target_directory: FilePath,
    /// What happens where a target exists.
    pub conflict: Conflict,
}

/// `fs.exists`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FsExists {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The directory.
    pub directory: FilePath,
    /// 1 to 1,000 names to look for in it.
    pub names: List<FileName, 1, 1000>,
}

/// How `fs.delete` removes items.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeleteMode {
    /// To the machine's trash, never permanently in its place.
    Trash,
    /// Permanently; a link is removed, never its target.
    Permanent,
}

/// `fs.delete`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FsDelete {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation's identity.
    pub request_id: RequestId,
    /// 1 to 256 items.
    pub paths: List<FilePath, 1, 256>,
    /// How they are removed.
    pub mode: DeleteMode,
    /// First keep the folders and the files of at most 5,000,000 bytes in an undo stash.
    #[serde(default, skip_serializing_if = "is_false")]
    pub capture: bool,
}

/// `fs.restore`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FsRestore {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation's identity.
    pub request_id: RequestId,
    /// The stash a capturing delete returned.
    pub stash: StashId,
}

/// `fs.clipboard`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FsClipboard {
    /// The Workspace whose scope holds the paths.
    pub workspace_id: WorkspaceId,
    /// Up to 256 items the connection's clipboard now holds; none clears it.
    pub paths: List<FilePath, 0, 256>,
    /// The items were cut, so a paste may move them.
    #[serde(default, skip_serializing_if = "is_false")]
    pub cut: bool,
}

/// What an import item is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportKind {
    /// A file of this many bytes, which `fs.import-file` brings.
    File(SafeInteger),
    /// A directory, created when the item is declared.
    Directory,
}

/// One item an import brings.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "WireImportItem", into = "WireImportItem")]
pub struct ImportItem {
    /// Its path below the import's target directory; a directory item declared earlier holds
    /// it, unless it has a single name.
    pub path: RelativePath,
    /// What it is.
    pub kind: ImportKind,
    /// What happens where it already exists.
    pub conflict: Conflict,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum WireImportKind {
    File,
    Directory,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WireImportItem {
    path: RelativePath,
    #[serde(rename = "type")]
    kind: WireImportKind,
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    size: Option<SafeInteger>,
    conflict: Conflict,
}

impl TryFrom<WireImportItem> for ImportItem {
    type Error = InvalidValue;

    fn try_from(wire: WireImportItem) -> Result<Self, InvalidValue> {
        let kind = match (wire.kind, wire.size) {
            (WireImportKind::File, Some(size)) => ImportKind::File(size),
            (WireImportKind::Directory, None) => ImportKind::Directory,
            _ => return Err(InvalidValue),
        };
        Ok(Self {
            path: wire.path,
            kind,
            conflict: wire.conflict,
        })
    }
}

impl From<ImportItem> for WireImportItem {
    fn from(item: ImportItem) -> Self {
        let (kind, size) = match item.kind {
            ImportKind::File(size) => (WireImportKind::File, Some(size)),
            ImportKind::Directory => (WireImportKind::Directory, None),
        };
        Self {
            path: item.path,
            kind,
            size,
            conflict: item.conflict,
        }
    }
}

/// `fs.import-begin`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FsImportBegin {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The mutation's identity, which names the import.
    pub request_id: RequestId,
    /// The import session the Client chose.
    pub import: SessionId,
    /// The directory the items go into.
    pub target_directory: FilePath,
    /// The first 1 to 1,000 items, numbered from 0 in this order.
    pub items: List<ImportItem, 1, 1000>,
}

/// `fs.import-items`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FsImportItems {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The open import.
    pub import: SessionId,
    /// 1 to 1,000 more items, numbered on from the import's earlier items.
    pub items: List<ImportItem, 1, 1000>,
}

/// `fs.import-file`; the file's bytes follow as the transfer's Puts, the last flagged `commit`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FsImportFile {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The open import.
    pub import: SessionId,
    /// The file item's number.
    pub index: SafeInteger,
    /// The transfer the Client chose for the bytes.
    pub transfer: TransferId,
}

/// `fs.import-end`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FsImportEnd {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The open import.
    pub import: SessionId,
}

/// `fs.export-begin`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FsExportBegin {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The export session the Client chose.
    pub export: SessionId,
    /// 1 to 256 items to download.
    pub paths: List<FilePath, 1, 256>,
}

/// `fs.export-entries`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FsExportEntries {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The open export.
    pub export: SessionId,
    /// The previous page's continuation; absent for the first page.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub cursor: Option<PageCursor>,
}

/// `picker.open` and `picker.close`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PickerRequest {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The picker session the Client chose.
    pub picker: SessionId,
}

/// `picker.list`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PickerList {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The open picker.
    pub picker: SessionId,
    /// Any directory of the machine.
    pub path: FilePath,
    /// The previous page's continuation; absent for the first page.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub cursor: Option<PageCursor>,
}
