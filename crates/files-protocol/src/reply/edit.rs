//! The results of the `files-edit` operations.

use serde::{Deserialize, Serialize};

use super::{Entry, FilesError, PathItem};
use crate::{
    identity::StashId,
    value::{
        FileName, FilePath, InvalidValue, List, PageCursor, RelativePath, SafeInteger,
        Sha256Digest, Signed, some,
    },
};

/// `fs.write`: the file as written.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FsWriteResult {
    /// Its modification time in milliseconds since the Unix epoch.
    pub mtime_ms: Signed,
    /// Its size in bytes.
    pub size: SafeInteger,
    /// The characters the encoding cannot represent, each written as `?`, or as `??` beyond the
    /// Basic Multilingual Plane in a single-byte encoding, as VS Code writes them.
    pub replaced: SafeInteger,
    /// The SHA-256 digest of its bytes, the precondition of the next save.
    pub sha256: Sha256Digest,
}

/// `fs.create` and `fs.rename`: the item's entry at its new path.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FsEntryResult {
    /// Its entry.
    pub entry: Entry,
}

/// One `fs.move` or `fs.copy` answer.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "WireTransferItem", into = "WireTransferItem")]
pub struct TransferItem {
    /// The source as requested.
    pub source: FilePath,
    /// The item's final path; with `error`, a move across volumes copied it there but could not
    /// remove the source.
    pub path: Option<FilePath>,
    /// Why it failed.
    pub error: Option<FilesError>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireTransferItem {
    source: FilePath,
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
    error: Option<FilesError>,
}

impl TryFrom<WireTransferItem> for TransferItem {
    type Error = InvalidValue;

    fn try_from(wire: WireTransferItem) -> Result<Self, InvalidValue> {
        if wire.path.is_none() && wire.error.is_none() {
            return Err(InvalidValue);
        }
        Ok(Self {
            source: wire.source,
            path: wire.path,
            error: wire.error,
        })
    }
}

impl From<TransferItem> for WireTransferItem {
    fn from(item: TransferItem) -> Self {
        Self {
            source: item.source,
            path: item.path,
            error: item.error,
        }
    }
}

/// `fs.move` and `fs.copy`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FsTransferResult {
    /// One answer per source, in request order.
    pub items: List<TransferItem, 1, 256>,
}

/// `fs.exists`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FsExistsResult {
    /// The requested names that exist in the directory, as requested.
    pub existing: List<FileName, 0, 1000>,
}

/// `fs.delete`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FsDeleteResult {
    /// One answer per requested path, in request order.
    pub items: List<PathItem, 1, 256>,
    /// The undo stash, when the delete captured something.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub stash: Option<StashId>,
}

/// How one directory item of an import ended when it was declared.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "WireImportOutcome", into = "WireImportOutcome")]
pub struct ImportOutcome {
    /// The item's number.
    pub index: SafeInteger,
    /// Its final path below the target directory, or why it failed.
    pub outcome: Result<RelativePath, FilesError>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireImportOutcome {
    index: SafeInteger,
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    path: Option<RelativePath>,
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    error: Option<FilesError>,
}

impl TryFrom<WireImportOutcome> for ImportOutcome {
    type Error = InvalidValue;

    fn try_from(wire: WireImportOutcome) -> Result<Self, InvalidValue> {
        let outcome = match (wire.path, wire.error) {
            (Some(path), None) => Ok(path),
            (None, Some(error)) => Err(error),
            _ => return Err(InvalidValue),
        };
        Ok(Self {
            index: wire.index,
            outcome,
        })
    }
}

impl From<ImportOutcome> for WireImportOutcome {
    fn from(item: ImportOutcome) -> Self {
        let (path, error) = match item.outcome {
            Ok(path) => (Some(path), None),
            Err(error) => (None, Some(error)),
        };
        Self {
            index: item.index,
            path,
            error,
        }
    }
}

/// `fs.import-begin` and `fs.import-items`: how the directory items they declared ended.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FsImportResult {
    /// One answer per directory item, in request order; none when the request declared only
    /// files.
    pub items: List<ImportOutcome, 0, 1000>,
}

/// `fs.import-file`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FsImportFileResult {
    /// The file's final path below the target directory.
    pub path: RelativePath,
}

/// What an export entry is.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExportKind {
    /// A file, which `fs.read` with the export and the entry's number reads.
    File,
    /// A directory.
    Directory,
}

/// One entry of an export.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "WireExportEntry", into = "WireExportEntry")]
pub struct ExportEntry {
    /// The entry's number in the export.
    pub index: SafeInteger,
    /// Its path below the directory the download goes into, starting with the selected item's
    /// name.
    pub path: RelativePath,
    /// What it is.
    pub kind: ExportKind,
    /// Its size and modification time, or `None` for an entry the export skips: a link to a
    /// directory, which it never enters, a link whose target leaves the boundary, or an entry it
    /// could not read or that is neither a file nor a directory.
    pub facts: Option<ExportFacts>,
}

/// An exported entry's size and modification time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExportFacts {
    /// The size in bytes.
    pub size: SafeInteger,
    /// The modification time in milliseconds since the Unix epoch.
    pub mtime_ms: Signed,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WireExportEntry {
    index: SafeInteger,
    path: RelativePath,
    #[serde(rename = "type")]
    kind: ExportKind,
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    size: Option<SafeInteger>,
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    mtime_ms: Option<Signed>,
    /// Present only as `true`.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    skipped: Option<bool>,
}

impl TryFrom<WireExportEntry> for ExportEntry {
    type Error = InvalidValue;

    fn try_from(wire: WireExportEntry) -> Result<Self, InvalidValue> {
        let facts = match (wire.size, wire.mtime_ms, wire.skipped) {
            (Some(size), Some(mtime_ms), None) => Some(ExportFacts { size, mtime_ms }),
            (None, None, Some(true)) => None,
            _ => return Err(InvalidValue),
        };
        Ok(Self {
            index: wire.index,
            path: wire.path,
            kind: wire.kind,
            facts,
        })
    }
}

impl From<ExportEntry> for WireExportEntry {
    fn from(entry: ExportEntry) -> Self {
        Self {
            index: entry.index,
            path: entry.path,
            kind: entry.kind,
            size: entry.facts.map(|facts| facts.size),
            mtime_ms: entry.facts.map(|facts| facts.mtime_ms),
            skipped: entry.facts.is_none().then_some(true),
        }
    }
}

/// `fs.export-entries`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FsExportEntriesResult {
    /// The page's entries, numbered in order, each directory before what it holds.
    pub entries: List<ExportEntry, 0, 2000>,
    /// The next page's continuation; absent on the last page.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub cursor: Option<PageCursor>,
}

/// `picker.open`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PickerOpenResult {
    /// The user's home directory.
    pub home: FilePath,
    /// The machine's filesystem roots: `/`, or each drive's root on Windows.
    pub roots: List<FilePath, 1, 26>,
}

/// `picker.list`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PickerListResult {
    /// The page's entries, unsorted.
    pub entries: List<Entry, 0, 2000>,
    /// The next page's continuation; absent on the last page.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub cursor: Option<PageCursor>,
}
