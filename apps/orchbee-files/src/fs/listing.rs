//! Directory pages: one directory's entries in the order the system returns them, in pages of at
//! most 2,000 entries within the caller's byte budget, continued through cursors. A connection
//! holds at most 16 cursors, the least recently used giving way to a new one, and a cursor idle
//! for 30 s expires, closing its directory handle; a page request naming a cursor that no longer
//! exists is invalid. A directory that changes between pages may repeat or miss entries, which
//! the watch then reports. A child whose own metadata cannot be read, such as one removed after
//! the directory was read, is left out, as VS Code leaves it out. With the single-child chain,
//! each directory that is its parent's only entry is listed as well, up to 64 levels, while its
//! whole listing fits the page; the caller enters each one before it is read, as it does the
//! listed directory, and may end the chain there.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use orchbee_files_protocol::{ConnectionId, Entry, EntryKind, ErrorCode, FilesError};

use super::timer::Timer;
use crate::{entry, error, staging};

/// Entries in one page, or in one descendant's listing.
const MAXIMUM_PAGE_ENTRIES: usize = 2_000;
/// Cursors one connection holds at once.
const MAXIMUM_CURSORS: usize = 16;
/// How long a cursor lives without a page request.
const CURSOR_IDLE: Duration = Duration::from_secs(30);
/// Levels of the single-child chain below the listed directory.
const MAXIMUM_DESCENDANTS: usize = 64;
/// Entries read between checks for the call's cancellation.
const CANCELLATION_STRIDE: usize = 64;

/// A page's continuation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct CursorId([u8; 16]);

impl CursorId {
    /// The continuation a Client sent back.
    pub(crate) const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// The continuation's wire bytes.
    pub(crate) const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

/// How many bytes a page may take, as the caller measures its own encoding.
pub(crate) struct PageBudget<'a> {
    /// The bytes the page's entries and descendants may take together. It holds any single
    /// entry, since a page always takes the next entry when it has none.
    pub(crate) bytes: usize,
    /// The bytes one entry takes.
    pub(crate) entry: &'a dyn Fn(&Entry) -> usize,
    /// The bytes one descendant takes besides its entries; `None` for a path a page cannot carry.
    pub(crate) descendant: &'a dyn Fn(&Path) -> Option<usize>,
}

/// One directory of the single-child chain, listed completely.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Descendant {
    /// The directory, below the listed one.
    pub(crate) path: PathBuf,
    /// Its entries.
    pub(crate) entries: Vec<Entry>,
}

/// One page of a directory.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Page {
    /// The page's entries, unsorted.
    pub(crate) entries: Vec<Entry>,
    /// The next page's continuation; absent on the last page.
    pub(crate) cursor: Option<CursorId>,
    /// The single-child chain below the directory, when asked for and the page is the whole
    /// directory.
    pub(crate) descendants: Vec<Descendant>,
}

/// The open cursors of every connection.
pub(crate) struct Listings {
    shared: Arc<Shared>,
    timer: Arc<Timer>,
    idle: Duration,
}

/// The lock guards the cursors and the counter that names them; it is never held across a
/// directory read. A panic ends the process, so a poisoned lock is never observed.
struct Shared {
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    open: HashMap<ConnectionId, Vec<Cursor>>,
    next: u128,
}

/// A directory partway read: the entry read past the last page waits for the next one.
struct Cursor {
    id: CursorId,
    path: PathBuf,
    remaining: Remaining,
    used: Instant,
}

struct Remaining {
    entries: fs::ReadDir,
    next: Option<Entry>,
}

/// What one bounded read of a directory produced.
struct Read {
    entries: Vec<Entry>,
    bytes: usize,
    /// The directory has entries beyond these.
    more: bool,
}

impl Listings {
    /// No cursors yet; `timer` expires idle ones.
    pub(crate) fn new(timer: Arc<Timer>) -> Self {
        Self::with_idle(timer, CURSOR_IDLE)
    }

    fn with_idle(timer: Arc<Timer>, idle: Duration) -> Self {
        Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State::default()),
            }),
            timer,
            idle,
        }
    }

    /// The page of `path` that follows `cursor`, or its first page. With `chain`, a first page that
    /// is the whole directory also lists the single-child chain below it, and `chain` enters each
    /// directory of the chain before it is read; a directory it refuses ends the chain. A
    /// directory entered and then left out, because it cannot be read or does not fit, is the last
    /// one entered. It reads the directory, so it runs on a helper.
    ///
    /// # Errors
    ///
    /// `invalid-request` for a cursor that is unknown, expired, in use or for another path;
    /// `cancelled` when the call was cancelled; otherwise the classified error of reading the
    /// directory, `not-directory` for an object that is not one.
    pub(crate) fn page(
        &self,
        connection: ConnectionId,
        path: &Path,
        cursor: Option<CursorId>,
        chain: Option<&mut dyn FnMut(&Path) -> bool>,
        budget: &PageBudget<'_>,
        cancelled: &AtomicBool,
    ) -> Result<Page, FilesError> {
        let mut remaining = match cursor {
            Some(cursor) => self.take(connection, cursor, path)?,
            None => open(path)?,
        };
        let read = read(&mut remaining, budget.bytes, budget, cancelled)?;
        let mut page = Page {
            entries: read.entries,
            cursor: None,
            descendants: Vec::new(),
        };
        if read.more {
            page.cursor = Some(self.store(connection, path, remaining));
        } else if cursor.is_none()
            && let Some(enter) = chain
        {
            let left = budget.bytes.saturating_sub(read.bytes);
            page.descendants = descendants(path, &page.entries, left, budget, cancelled, enter)?;
        }
        Ok(page)
    }

    /// Closes every cursor `connection` holds.
    pub(crate) fn connection_lost(&self, connection: ConnectionId) {
        self.shared.lock().open.remove(&connection);
    }

    fn take(
        &self,
        connection: ConnectionId,
        id: CursorId,
        path: &Path,
    ) -> Result<Remaining, FilesError> {
        let mut state = self.shared.lock();
        let cursors = state.open.get_mut(&connection);
        let position = cursors
            .as_ref()
            .and_then(|cursors| cursors.iter().position(|cursor| cursor.id == id));
        match (cursors, position) {
            (Some(cursors), Some(position)) if cursors[position].path == path => {
                Ok(cursors.swap_remove(position).remaining)
            }
            _ => Err(FilesError::new(ErrorCode::InvalidRequest)),
        }
    }

    fn store(&self, connection: ConnectionId, path: &Path, remaining: Remaining) -> CursorId {
        let used = Instant::now();
        let mut state = self.shared.lock();
        state.next += 1;
        let id = CursorId(state.next.to_be_bytes());
        let cursors = state.open.entry(connection).or_default();
        // Every page names a new cursor, so the smallest name is the least recently used.
        if cursors.len() >= MAXIMUM_CURSORS
            && let Some(oldest) = cursors
                .iter()
                .enumerate()
                .min_by_key(|(_, cursor)| u128::from_be_bytes(cursor.id.0))
                .map(|(position, _)| position)
        {
            cursors.swap_remove(oldest);
        }
        cursors.push(Cursor {
            id,
            path: path.to_path_buf(),
            remaining,
            used,
        });
        drop(state);
        let shared = Arc::downgrade(&self.shared);
        // Without a timer thread a cursor lives until it is the least recently used or its
        // connection ends.
        let _ = self.timer.schedule(used + self.idle, move || {
            if let Some(shared) = shared.upgrade() {
                shared.expire(connection, id, used);
            }
        });
        id
    }
}

impl Shared {
    /// Closes cursor `id` unless a page request used it after `used`.
    fn expire(&self, connection: ConnectionId, id: CursorId, used: Instant) {
        let expired = {
            let mut state = self.lock();
            let Some(cursors) = state.open.get_mut(&connection) else {
                return;
            };
            let expired = cursors
                .iter()
                .position(|cursor| cursor.id == id && cursor.used == used)
                .map(|position| cursors.swap_remove(position));
            if cursors.is_empty() {
                state.open.remove(&connection);
            }
            expired
        };
        // The directory handle closes here, outside the lock.
        drop(expired);
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn open(path: &Path) -> Result<Remaining, FilesError> {
    match fs::read_dir(path) {
        Ok(entries) => Ok(Remaining {
            entries,
            next: None,
        }),
        Err(failure) => {
            let mut refused = error::from_io(&failure);
            // Windows reports a file read as a directory as a missing path.
            if failure.kind() == std::io::ErrorKind::NotFound
                && fs::metadata(path).is_ok_and(|metadata| !metadata.is_dir())
            {
                refused.code = ErrorCode::NotDirectory;
            }
            Err(refused)
        }
    }
}

/// Reads entries until 2,000, until the next one would pass `bytes`, or to the directory's end,
/// which a read past the last entry establishes.
fn read(
    remaining: &mut Remaining,
    bytes: usize,
    budget: &PageBudget<'_>,
    cancelled: &AtomicBool,
) -> Result<Read, FilesError> {
    let mut read = Read {
        entries: Vec::new(),
        bytes: 0,
        more: false,
    };
    loop {
        if read.entries.len().is_multiple_of(CANCELLATION_STRIDE)
            && cancelled.load(Ordering::Acquire)
        {
            return Err(FilesError::new(ErrorCode::Cancelled));
        }
        let Some(entry) = next(remaining)? else {
            return Ok(read);
        };
        let size = (budget.entry)(&entry);
        let fits = read
            .bytes
            .checked_add(size)
            .is_some_and(|total| total <= bytes);
        if read.entries.len() >= MAXIMUM_PAGE_ENTRIES || (!fits && !read.entries.is_empty()) {
            remaining.next = Some(entry);
            read.more = true;
            return Ok(read);
        }
        read.bytes = read.bytes.saturating_add(size);
        read.entries.push(entry);
    }
}

fn next(remaining: &mut Remaining) -> Result<Option<Entry>, FilesError> {
    if let Some(entry) = remaining.next.take() {
        return Ok(Some(entry));
    }
    loop {
        match remaining.entries.next() {
            None => return Ok(None),
            Some(Err(failure)) => return Err(error::from_io(&failure)),
            Some(Ok(child)) => {
                if let Some(entry) = describe(&child) {
                    return Ok(Some(entry));
                }
            }
        }
    }
}

/// The entry of a child the directory read returned, from the metadata that read already holds
/// where the system provides it; `None` when the child's own metadata cannot be read, its name is
/// longer than an entry carries, or it is a file an import is still staging.
fn describe(child: &fs::DirEntry) -> Option<Entry> {
    if child.file_name().to_str().is_some_and(staging::is_staging) {
        return None;
    }
    let own = child.metadata().ok()?;
    entry::describe(&child.file_name(), &own, || fs::metadata(child.path())).ok()
}

/// The single-child chain below `path`, whose complete listing is `entries`, within `bytes`, each
/// directory entered through `enter` before it is read. The chain ends before a directory that
/// cannot be read, does not fit, has a path a page cannot carry or that `enter` refuses, and the
/// Client lists that one itself.
fn descendants(
    path: &Path,
    entries: &[Entry],
    mut bytes: usize,
    budget: &PageBudget<'_>,
    cancelled: &AtomicBool,
    enter: &mut dyn FnMut(&Path) -> bool,
) -> Result<Vec<Descendant>, FilesError> {
    let mut chain: Vec<Descendant> = Vec::new();
    let mut directory = path.to_path_buf();
    let mut only = only_directory(entries);
    while let Some(name) = only.take()
        && chain.len() < MAXIMUM_DESCENDANTS
    {
        let child = directory.join(name);
        let Some(left) = (budget.descendant)(&child).and_then(|cost| bytes.checked_sub(cost))
        else {
            break;
        };
        if !enter(&child) {
            break;
        }
        let Ok(mut remaining) = open(&child) else {
            break;
        };
        let read = match read(&mut remaining, left, budget, cancelled) {
            Ok(read) => read,
            Err(error) if error.code == ErrorCode::Cancelled => return Err(error),
            Err(_) => break,
        };
        if read.more || read.bytes > left {
            break;
        }
        bytes = left - read.bytes;
        only = only_directory(&read.entries);
        chain.push(Descendant {
            path: child.clone(),
            entries: read.entries,
        });
        directory = child;
    }
    Ok(chain)
}

/// The name of a listing's only entry when it is a directory a path can name.
fn only_directory(entries: &[Entry]) -> Option<String> {
    match entries {
        [entry] if entry.kind == EntryKind::Directory && !entry.unrepresentable => {
            Some(entry.name.as_str().to_owned())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;
