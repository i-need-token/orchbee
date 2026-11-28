//! Exports, which serve Download. `fs.export-begin` captures up to 256 selected items under the
//! session the Client chose, each in the boundary as a read admits it; `fs.export-entries` pages
//! every entry below them, numbered in order, each directory before what it holds and a folder's
//! children in the order the system lists them; and `fs.read` with the export and a file entry's
//! number reads that file. A link to a directory is not followed, and a link to a file only when
//! its target lies in the boundary, as a change to it would have to; each is otherwise skipped,
//! as is an entry whose metadata or folder cannot be read, or that is neither a file nor a
//! directory. An entry whose path a record cannot carry, such as a name that is not UTF-8, is left
//! out with what it holds.
//!
//! A page holds at most 2,000 entries within a Result's bound and names the next page's cursor,
//! which the next request must name. An export numbers at most 100,000 entries; the page past them
//! is `too-large`. A connection holds at most 16 exports, the least recently used giving way to a
//! new one, and an export ends ten minutes after its last request or with its connection.

use std::{
    collections::HashMap,
    fs::{self, Metadata},
    io,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::{Duration, Instant},
};

use orchbee_files_protocol::{
    ConnectionId, ErrorCode, ExportEntry, ExportFacts, ExportKind, FilePath, FilesError,
    FsExportEntriesResult, List, MAXIMUM_REPLY_BYTES, Operation, PageCursor, RelativePath,
    SafeInteger, SessionId, WorkspaceId,
};

use super::names;
use crate::{
    blocking::Helpers,
    entry,
    scope::{Change, View, path::NativePath},
};

/// How long an export lives without a request.
const IDLE: Duration = Duration::from_secs(10 * 60);
/// Exports one connection holds at once.
const MAXIMUM_EXPORTS: usize = 16;
/// Entries one export numbers.
const MAXIMUM_ENTRIES: u64 = 100_000;
/// Entries in one page.
const MAXIMUM_PAGE_ENTRIES: usize = 2_000;
/// What a page's entries may take of a Result: its bound, less the members around them.
const PAGE_BYTES: usize = MAXIMUM_REPLY_BYTES - 256;

/// The open exports of every connection.
pub(super) struct Exports {
    open: Mutex<HashMap<(ConnectionId, SessionId), Held>>,
}

struct Held {
    workspace: WorkspaceId,
    touched: Instant,
    export: Arc<Mutex<Export>>,
}

/// The file entries an export numbered, by number, which reads take while a page is produced.
type Files = Arc<Mutex<HashMap<u64, NativePath>>>;

/// One export's walk and what it numbered.
struct Export {
    view: Arc<View>,
    /// The cursor the next page request names; `None` until the first page.
    cursor: Option<PageCursor>,
    /// Absent while a helper produces a page, and for good once one was abandoned.
    walk: Option<Walk>,
    files: Files,
}

/// What an export's walk still has to number.
struct Walk {
    /// The items still to visit, the next on top: the selected items, then each folder's children
    /// as its walk enters it.
    pending: Vec<Pending>,
    /// An entry numbered for a page it did not fit.
    carried: Option<ExportEntry>,
    next: u64,
    files: Files,
}

struct Pending {
    path: NativePath,
    relative: String,
}

impl Exports {
    pub(super) fn new() -> Self {
        Self {
            open: Mutex::new(HashMap::new()),
        }
    }

    /// `fs.export-begin` of `paths`, which `view` admitted, under `export` for `connection` in
    /// `workspace`.
    ///
    /// # Errors
    ///
    /// `invalid-request` when the connection holds an open export under that name.
    pub(super) fn begin(
        &self,
        connection: ConnectionId,
        workspace: WorkspaceId,
        view: View,
        export: SessionId,
        paths: Vec<NativePath>,
    ) -> Result<(), FilesError> {
        let now = Instant::now();
        let mut open = self.lock();
        open.retain(|_, held| now.duration_since(held.touched) < IDLE);
        if open.contains_key(&(connection, export)) {
            return Err(FilesError::new(ErrorCode::InvalidRequest));
        }
        let held = open.iter().filter(|(key, _)| key.0 == connection);
        if held.clone().count() >= MAXIMUM_EXPORTS {
            let oldest = held
                .min_by_key(|(_, held)| held.touched)
                .map(|(key, _)| *key);
            if let Some(oldest) = oldest {
                open.remove(&oldest);
            }
        }
        let pending = paths
            .into_iter()
            .rev()
            .map(|path| Pending {
                relative: path.name().to_owned(),
                path,
            })
            .collect();
        let files = Files::default();
        let walk = Walk {
            pending,
            carried: None,
            next: 0,
            files: Arc::clone(&files),
        };
        open.insert(
            (connection, export),
            Held {
                workspace,
                touched: now,
                export: Arc::new(Mutex::new(Export {
                    view: Arc::new(view),
                    cursor: None,
                    walk: Some(walk),
                    files,
                })),
            },
        );
        Ok(())
    }

    /// `fs.export-entries`: the page after `cursor` of `connection`'s `export` in `workspace`,
    /// produced on a helper.
    ///
    /// # Errors
    ///
    /// `invalid-request` for an export that is not open there or a cursor it did not name last,
    /// `too-large` past the entries an export numbers, and the helpers' refusal.
    pub(super) fn page(
        &self,
        helpers: &Helpers,
        connection: ConnectionId,
        workspace: WorkspaceId,
        export: SessionId,
        cursor: Option<PageCursor>,
    ) -> Result<FsExportEntriesResult, FilesError> {
        let invalid = || FilesError::new(ErrorCode::InvalidRequest);
        let export = self
            .find(connection, workspace, export)
            .ok_or_else(invalid)?;
        // The export is free while the helper walks, so reads of its numbered files go on; a
        // second page request meanwhile finds no walk.
        let (mut walk, view) = {
            let mut export = lock(&export);
            if export.cursor != cursor {
                return Err(invalid());
            }
            let walk = export.walk.take().ok_or_else(invalid)?;
            (walk, Arc::clone(&export.view))
        };
        let (walk, entries) = helpers.run(Operation::FsExportEntries, move || {
            let entries = walk.page(&view);
            (walk, entries)
        })?;
        let more = !walk.pending.is_empty() || walk.carried.is_some();
        let mut export = lock(&export);
        export.walk = Some(walk);
        let entries = entries?;
        let next = if more { Some(cursor_id()?) } else { None };
        export.cursor = next;
        let entries = List::new(entries).map_err(|_| FilesError::new(ErrorCode::Failed))?;
        Ok(FsExportEntriesResult {
            entries,
            cursor: next,
        })
    }

    /// The file entry `index` of `connection`'s `export` in `workspace`, for `fs.read`.
    ///
    /// # Errors
    ///
    /// `invalid-request` for an export that is not open there, or a number that names no file
    /// entry of it.
    pub(super) fn file(
        &self,
        connection: ConnectionId,
        workspace: WorkspaceId,
        export: SessionId,
        index: u64,
    ) -> Result<NativePath, FilesError> {
        let invalid = || FilesError::new(ErrorCode::InvalidRequest);
        let export = self
            .find(connection, workspace, export)
            .ok_or_else(invalid)?;
        let files = Arc::clone(&lock(&export).files);
        let file = files
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&index)
            .cloned();
        file.ok_or_else(invalid)
    }

    /// Ends every export of `connection`.
    pub(super) fn connection_lost(&self, connection: ConnectionId) {
        self.lock().retain(|key, _| key.0 != connection);
    }

    /// `connection`'s open `export` in `workspace`, which this use keeps open for ten more minutes.
    fn find(
        &self,
        connection: ConnectionId,
        workspace: WorkspaceId,
        export: SessionId,
    ) -> Option<Arc<Mutex<Export>>> {
        let now = Instant::now();
        let mut open = self.lock();
        let held = open.get_mut(&(connection, export))?;
        if now.duration_since(held.touched) >= IDLE {
            open.remove(&(connection, export));
            return None;
        }
        if held.workspace != workspace {
            return None;
        }
        held.touched = now;
        Some(Arc::clone(&held.export))
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<(ConnectionId, SessionId), Held>> {
        self.open.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Walk {
    /// The next page's entries.
    fn page(&mut self, view: &View) -> Result<Vec<ExportEntry>, FilesError> {
        let mut entries = Vec::new();
        let mut bytes = 0;
        loop {
            let entry = match self.carried.take() {
                Some(entry) => entry,
                None if self.pending.is_empty() => break,
                None if self.next >= MAXIMUM_ENTRIES => {
                    return if entries.is_empty() {
                        Err(FilesError::new(ErrorCode::TooLarge))
                    } else {
                        Ok(entries)
                    };
                }
                None => match self.visit(view) {
                    Some(entry) => entry,
                    None => continue,
                },
            };
            let size = serde_json::to_vec(&entry).map_or(0, |json| json.len() + 1);
            if !entries.is_empty()
                && (entries.len() >= MAXIMUM_PAGE_ENTRIES || bytes + size > PAGE_BYTES)
            {
                self.carried = Some(entry);
                break;
            }
            bytes += size;
            entries.push(entry);
        }
        Ok(entries)
    }

    /// Numbers the next pending item, entering it when it is a folder; `None` for an item whose
    /// path a record cannot carry.
    fn visit(&mut self, view: &View) -> Option<ExportEntry> {
        let Pending { path, relative } = self.pending.pop()?;
        let relative_path = RelativePath::new(relative.clone()).ok()?;
        let index = self.next;
        let number = SafeInteger::new(index).ok()?;
        self.next += 1;
        let entry = |kind, facts| ExportEntry {
            index: number,
            path: relative_path.clone(),
            kind,
            facts,
        };
        let Ok(own) = fs::symlink_metadata(path.as_path()) else {
            return Some(entry(ExportKind::File, None));
        };
        if own.is_symlink() {
            // A link counts as what it names, which a directory link never enters.
            let named = fs::metadata(path.as_path());
            return Some(match named {
                Ok(named) if named.is_file() && inside(view, &path) => {
                    self.number(index, path);
                    entry(ExportKind::File, facts(&named))
                }
                Ok(named) if named.is_dir() => entry(ExportKind::Directory, None),
                _ => entry(ExportKind::File, None),
            });
        }
        if own.is_file() {
            self.number(index, path);
            return Some(entry(ExportKind::File, facts(&own)));
        }
        if !own.is_dir() {
            return Some(entry(ExportKind::File, None));
        }
        let listed = fs::read_dir(path.as_path()).and_then(|listed| {
            listed
                .map(|child| child.map(|child| child.file_name()))
                .collect::<io::Result<Vec<_>>>()
        });
        let Ok(children) = listed else {
            return Some(entry(ExportKind::Directory, None));
        };
        let below = children.into_iter().rev().filter_map(|name| {
            let name = name.into_string().ok()?;
            let child = names::child(&path, &name).ok()?;
            Some(Pending {
                path: child,
                relative: format!("{relative}/{name}"),
            })
        });
        self.pending.extend(below);
        Some(entry(ExportKind::Directory, facts(&own)))
    }

    /// Records the file `path` as entry `index`, which `fs.read` reads.
    fn number(&self, index: u64, path: NativePath) {
        let mut files = self.files.lock().unwrap_or_else(PoisonError::into_inner);
        files.insert(index, path);
    }
}

/// Whether the target of the link at `path` lies in `view`'s boundary.
fn inside(view: &View, path: &NativePath) -> bool {
    FilePath::new(path.as_str()).is_ok_and(|path| view.mutable(&path, Change::Content).is_ok())
}

/// An entry's size and modification time; a directory's size is zero.
fn facts(metadata: &Metadata) -> Option<ExportFacts> {
    let size = if metadata.is_dir() { 0 } else { metadata.len() };
    Some(ExportFacts {
        size: SafeInteger::new(size).ok()?,
        mtime_ms: entry::milliseconds(metadata.modified()),
    })
}

/// A fresh page cursor.
fn cursor_id() -> Result<PageCursor, FilesError> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).map_err(|_| FilesError::new(ErrorCode::Failed))?;
    Ok(PageCursor::from_bytes(bytes))
}

fn lock(export: &Mutex<Export>) -> MutexGuard<'_, Export> {
    export.lock().unwrap_or_else(PoisonError::into_inner)
}
