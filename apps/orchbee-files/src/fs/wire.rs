//! The file engine's values as the File Bar protocol spells them. A listing page is cut by the
//! JSON its result encodes to, so a page never passes the 128 KiB a result record carries.

use std::path::Path;

use orchbee_files_protocol::{
    Descendant as WireDescendant, EncodingName, Entry, ErrorCode, FilePath, FilesError,
    FsListResult, List, MAXIMUM_REPLY_BYTES, PageCursor, ReadResult, SafeInteger,
};

use super::{
    listing::{Page, PageBudget},
    transfers::ReadFacts,
};

/// What a result's JSON spends besides its entries: the members, a cursor and the brackets.
const RESULT_OVERHEAD: usize =
    r#"{"entries":[],"cursor":"00000000000000000000000000000000","descendants":[]}"#.len();

impl ReadFacts {
    pub(crate) fn to_wire(self) -> Result<ReadResult, FilesError> {
        let failed = |_| FilesError::new(ErrorCode::Failed);
        Ok(ReadResult {
            size: SafeInteger::new(self.size).map_err(failed)?,
            mtime_ms: self.mtime_ms,
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
    serde_json::to_vec(entry).map_or(0, |json| json.len() + 1)
}

/// A descendant's JSON without its entries, and the comma before it; `None` for a path a record
/// cannot carry.
fn descendant_bytes(path: &Path) -> Option<usize> {
    let descendant = WireDescendant {
        path: file_path(path)?,
        entries: List::new(Vec::new()).ok()?,
    };
    serde_json::to_vec(&descendant)
        .ok()
        .map(|json| json.len() + 1)
}

fn file_path(path: &Path) -> Option<FilePath> {
    path.to_str().and_then(|path| FilePath::new(path).ok())
}

/// A page as `fs.list` returns it.
pub(crate) fn list_result(page: Page) -> Result<FsListResult, FilesError> {
    let failed = || FilesError::new(ErrorCode::Failed);
    let descendants = page
        .descendants
        .into_iter()
        .map(|descendant| {
            Ok(WireDescendant {
                // The page budget admitted only descendants whose path a record carries.
                path: file_path(&descendant.path).ok_or_else(failed)?,
                entries: List::new(descendant.entries).map_err(|_| failed())?,
            })
        })
        .collect::<Result<Vec<_>, FilesError>>()?;
    Ok(FsListResult {
        entries: List::new(page.entries).map_err(|_| failed())?,
        cursor: page
            .cursor
            .map(|cursor| PageCursor::from_bytes(*cursor.as_bytes())),
        descendants: if descendants.is_empty() {
            None
        } else {
            Some(List::new(descendants).map_err(|_| failed())?)
        },
    })
}
