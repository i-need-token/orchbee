//! The file engine's values as the File Bar protocol spells them. A listing page is cut by the
//! JSON its result encodes to, so a page never passes the 128 KiB a result record carries.

use std::path::Path;

use orchbee_files_protocol::{
    Descendant as WireDescendant, EncodingName, Entry as WireEntry, EntryKind as WireKind,
    ErrorCode, FilePath, FilesError, FsListResult, List, MAXIMUM_REPLY_BYTES, PageCursor,
    ReadResult, SafeInteger, Signed, Text, Token,
};

use super::{
    listing::{Page, PageBudget},
    metadata::{Entry, EntryKind},
    os_error::{FsError, FsErrorCode},
    reading::ReadFacts,
};

/// What a result's JSON spends besides its entries: the members, a cursor and the brackets.
const RESULT_OVERHEAD: usize =
    r#"{"entries":[],"cursor":"00000000000000000000000000000000","descendants":[]}"#.len();

impl FsError {
    pub(crate) fn to_wire(self) -> FilesError {
        let reason = |reason: &str| Token::new(reason).ok();
        let mut error = match self.code {
            FsErrorCode::StuckFilesystem | FsErrorCode::Overloaded => {
                let reason = if self.code == FsErrorCode::Overloaded {
                    reason("overloaded")
                } else {
                    reason("stuck-filesystem")
                };
                match reason {
                    Some(reason) => FilesError::unavailable(reason),
                    None => FilesError::new(ErrorCode::Failed),
                }
            }
            code => FilesError::new(match code {
                FsErrorCode::NotFound => ErrorCode::NotFound,
                FsErrorCode::NotDirectory => ErrorCode::NotDirectory,
                FsErrorCode::IsDirectory => ErrorCode::IsDirectory,
                FsErrorCode::InvalidName => ErrorCode::InvalidName,
                FsErrorCode::PermissionDenied => ErrorCode::PermissionDenied,
                FsErrorCode::Busy => ErrorCode::Busy,
                FsErrorCode::TooLarge => ErrorCode::TooLarge,
                FsErrorCode::Timeout => ErrorCode::Timeout,
                FsErrorCode::Cancelled => ErrorCode::Cancelled,
                FsErrorCode::InvalidRequest => ErrorCode::InvalidRequest,
                _ => ErrorCode::Failed,
            }),
        };
        error.os_error = self
            .os_error
            .and_then(|number| Signed::new(i64::from(number)).ok());
        error
    }
}

impl Entry {
    /// The entry as a record carries it; `None` for a name past the 1,024 bytes a record holds,
    /// which no filesystem the File Bar serves produces.
    pub(crate) fn to_wire(&self) -> Option<WireEntry> {
        Some(WireEntry {
            name: Text::new(self.name.clone()).ok()?,
            kind: match self.kind {
                EntryKind::File => WireKind::File,
                EntryKind::Directory => WireKind::Directory,
                EntryKind::Unknown => WireKind::Unknown,
            },
            symlink: self.symlink,
            size: SafeInteger::new(self.size).ok()?,
            mtime_ms: Signed::new(self.mtime_ms).ok()?,
            readonly: self.readonly,
            unrepresentable: self.unrepresentable,
        })
    }
}

impl ReadFacts {
    pub(crate) fn to_wire(self) -> Result<ReadResult, FilesError> {
        let failed = |_| FilesError::new(ErrorCode::Failed);
        Ok(ReadResult {
            size: SafeInteger::new(self.size).map_err(failed)?,
            mtime_ms: Signed::new(self.mtime_ms).map_err(failed)?,
            encoding: self
                .encoding
                .map(|encoding| EncodingName::new(encoding.name()))
                .transpose()
                .map_err(failed)?,
            bom: self.bom,
            binary: self.binary,
        })
    }
}

/// The budget that keeps a listing result's JSON within a result record.
pub(crate) fn page_budget() -> PageBudget<'static> {
    PageBudget {
        bytes: MAXIMUM_REPLY_BYTES - RESULT_OVERHEAD,
        entry: &entry_bytes,
        descendant: &descendant_bytes,
    }
}

/// An entry's JSON and the comma before it.
fn entry_bytes(entry: &Entry) -> usize {
    entry
        .to_wire()
        .and_then(|entry| serde_json::to_vec(&entry).ok())
        .map_or(0, |json| json.len() + 1)
}

/// A descendant's JSON without its entries, and the comma before it.
fn descendant_bytes(path: &Path) -> usize {
    let path = path.to_str().and_then(|path| FilePath::new(path).ok());
    path.and_then(|path| {
        serde_json::to_vec(&WireDescendant {
            path,
            entries: List::new(Vec::new()).ok()?,
        })
        .ok()
    })
    .map_or(0, |json| json.len() + 1)
}

/// A page as `fs.list` returns it.
pub(crate) fn list_result(page: Page) -> Result<FsListResult, FilesError> {
    let failed = |_| FilesError::new(ErrorCode::Failed);
    let entries = |entries: Vec<Entry>| {
        List::new(entries.iter().filter_map(Entry::to_wire).collect()).map_err(failed)
    };
    let descendants = page
        .descendants
        .into_iter()
        .map(|descendant| {
            let path = descendant
                .path
                .to_str()
                .ok_or(FilesError::new(ErrorCode::Failed))?;
            Ok(WireDescendant {
                path: FilePath::new(path).map_err(failed)?,
                entries: entries(descendant.entries)?,
            })
        })
        .collect::<Result<Vec<_>, FilesError>>()?;
    Ok(FsListResult {
        entries: entries(page.entries)?,
        cursor: page
            .cursor
            .map(|cursor| PageCursor::from_bytes(*cursor.as_bytes())),
        descendants: if descendants.is_empty() {
            None
        } else {
            Some(List::new(descendants).map_err(failed)?)
        },
    })
}
