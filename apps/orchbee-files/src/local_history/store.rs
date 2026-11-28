//! The store in the data root's `internal/local-history/`. `index.json` maps each file with
//! history to its numbered folder, which holds `entries.json` and one copy per entry, named by the
//! entry's time in UTC and the file's extension:
//!
//! ```text
//! internal/local-history/
//! ├── index.json
//! ├── history.lock
//! └── 000001/
//!     ├── entries.json
//!     └── 2026-11-27T08-30-15-123Z.ts
//! ```
//!
//! `index.json` holds `{version: 1, folders: [{folder, source, size, oldest}]}`, and each
//! `entries.json` holds `{version: 1, source, entries: [{id, file, source, label?, timestamp,
//! size}]}`.
//!
//! Every process sharing the data root reads and changes the store only while it holds
//! `history.lock`, which it waits for at most 500 ms, and reads the index again under each hold, so
//! a later hold in any Session sees what an earlier one recorded. The holds of one process queue
//! for one another before they take the lock, so only another Session's hold can make one wait
//! past that. `entries.json` and `index.json` are replaced atomically; a copy is complete before an
//! `entries.json` names it, and a new folder joins the index before it holds anything, so an
//! interrupted change leaves at worst a listed empty folder or a copy nothing names, which the
//! folder's next change removes.
//!
//! An index this build cannot read is rebuilt from the folders' `entries.json`. A record of a newer
//! version is never rewritten: a newer index leaves the whole store to that version, and a folder
//! whose `entries.json` is newer stays out of the index, untouched. A folder whose record holds a
//! value the File Bar protocol cannot carry, such as an id that is not 32 hexadecimal digits,
//! counts as empty.

use std::{
    collections::HashSet,
    fs::{self, File, TryLockError},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard, PoisonError},
    thread,
    time::{Duration, Instant},
};

use orchbee_files_protocol::{HistoryEntryId, MAXIMUM_SAFE_INTEGER};
use orchbee_local_runtime::{HeldLock, create_private_directory, private_filesystem};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::scope::path::NativePath;

const VERSION: u64 = 1;
const INDEX: &str = "index.json";
const INDEX_PENDING: &str = "index.pending";
const ENTRIES: &str = "entries.json";
const ENTRIES_PENDING: &str = "entries.pending";
const LOCK: &str = "history.lock";
/// How long a hold waits while another process holds the store.
const LOCK_WAIT: Duration = Duration::from_millis(500);
const LOCK_POLL: Duration = Duration::from_millis(5);
/// The largest index read; it lists one folder per file with history.
const INDEX_BYTES: u64 = 64 << 20;
/// The largest `entries.json` read; it lists at most [`Limits::entries`] entries.
const ENTRIES_BYTES: u64 = 4 << 20;
/// The most bytes a label keeps.
const LABEL_LIMIT: usize = 1024;
/// The most names one copy tries before its folder counts as full.
const NAME_ATTEMPTS: u32 = 1000;

/// What the store keeps. [`Limits::DEFAULT`] holds VS Code's defaults at ef5ca45 and the total
/// VS Code does not have.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Limits {
    /// A larger file records nothing.
    pub(crate) content: u64,
    /// The most entries a file keeps; the oldest leave first.
    pub(crate) entries: usize,
    /// A record this many milliseconds or less after the file's newest entry, from the same
    /// source, replaces that entry unless it carries a label.
    pub(crate) merge: i64,
    /// The most bytes all copies take; past it the oldest entries of any file leave first.
    pub(crate) total: u64,
}

impl Limits {
    pub(crate) const DEFAULT: Self = Self {
        content: 256 * 1024,
        entries: 50,
        merge: 10_000,
        total: 1 << 30,
    };
}

/// Why an entry was recorded.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Source {
    /// The File Bar saved the file.
    Save,
    /// An entry's content was restored.
    Restore,
    /// The file moved to another folder.
    Move,
    /// The file was renamed in its folder.
    Rename,
}

/// One entry of a file's history.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Entry {
    pub(crate) id: String,
    pub(crate) source: Source,
    pub(crate) label: Option<String>,
    /// Milliseconds since the Unix epoch: the file's modification time for a save or a restore,
    /// the time of the move for a move or a rename.
    pub(crate) timestamp: i64,
    /// The copy's bytes.
    pub(crate) size: u64,
}

/// Why the store could not serve.
#[derive(Debug)]
pub(crate) enum StoreError {
    /// Another process held the store past the wait.
    Busy,
    /// A newer version of Orchbee wrote the index or the file's entries, which this build leaves
    /// as they are.
    Newer,
    /// The file has no such entry.
    NotFound,
    Io(io::Error),
}

impl From<io::Error> for StoreError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct IndexRecord {
    version: u64,
    folders: Vec<IndexFolder>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct IndexFolder {
    folder: String,
    source: String,
    size: u64,
    oldest: i64,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EntriesRecord {
    version: u64,
    source: String,
    entries: Vec<Stored>,
}

/// An entry as `entries.json` keeps it.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    id: String,
    file: String,
    source: Source,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    timestamp: i64,
    size: u64,
}

/// A folder the index lists.
#[derive(Clone, Debug)]
struct Folder {
    number: u64,
    source: NativePath,
    key: String,
    size: u64,
    oldest: i64,
}

/// What a folder's `entries.json` holds for this build.
enum Listing {
    /// No usable record: none, or one this build refuses, whose copies nothing names.
    Empty,
    Current(NativePath, Vec<Stored>),
    Newer,
}

/// The store of one data root.
pub(crate) struct Store {
    root: PathBuf,
    limits: Limits,
    /// Taken before `history.lock`, so the holds of this process wait for one another in turn
    /// rather than poll the lock against its deadline.
    gate: Mutex<()>,
}

impl Store {
    /// The store at `root`, the data root's `internal/local-history/` unless a test keeps its own.
    pub(crate) const fn new(root: PathBuf, limits: Limits) -> Self {
        Self {
            root,
            limits,
            gate: Mutex::new(()),
        }
    }

    /// The largest content one entry keeps.
    pub(crate) const fn content_limit(&self) -> u64 {
        self.limits.content
    }

    /// Runs `read` while holding the store; `None` while there is no store, since a read creates
    /// nothing.
    pub(crate) fn read<T>(
        &self,
        read: impl FnOnce(&Hold<'_>) -> Result<T, StoreError>,
    ) -> Result<Option<T>, StoreError> {
        let Some(pins) = self.existing()? else {
            return Ok(None);
        };
        let hold = self.hold(pins)?;
        read(&hold).map(Some)
    }

    /// Runs `write` while holding the store, creating it when missing, and replaces the index when
    /// the change, or a rebuild, changed it.
    pub(crate) fn write<T>(
        &self,
        write: impl FnOnce(&mut Hold<'_>) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        let pins = create_private_directory(&self.root)?;
        self.changed(pins, write)
    }

    /// Runs `change` as [`Self::write`] does, on a store that exists: `None` while there is none,
    /// since a change of entries nothing holds creates nothing.
    pub(crate) fn change<T>(
        &self,
        change: impl FnOnce(&mut Hold<'_>) -> Result<T, StoreError>,
    ) -> Result<Option<T>, StoreError> {
        let Some(pins) = self.existing()? else {
            return Ok(None);
        };
        self.changed(pins, change).map(Some)
    }

    /// The store's folders, held open, or `None` while there is no store.
    fn existing(&self) -> Result<Option<Vec<File>>, StoreError> {
        match private_filesystem::open_tree(&self.root) {
            Ok(pins) => Ok(Some(pins)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn changed<T>(
        &self,
        pins: Vec<File>,
        change: impl FnOnce(&mut Hold<'_>) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        let mut hold = self.hold(pins)?;
        let written = change(&mut hold);
        // An interrupted change keeps what it finished in the index.
        let saved = if hold.changed {
            hold.save_index()
        } else {
            Ok(())
        };
        written.and_then(|value| saved.map(|()| value))
    }

    fn hold(&self, pins: Vec<File>) -> Result<Hold<'_>, StoreError> {
        let gate = self.gate.lock().unwrap_or_else(PoisonError::into_inner);
        let lock = lock(&self.root.join(LOCK))?;
        let (index, changed) = match read_bounded(&self.root.join(INDEX), INDEX_BYTES)? {
            Some(bytes) if version(&bytes).is_some_and(|version| version > VERSION) => {
                return Err(StoreError::Newer);
            }
            Some(bytes) => match parse_index(&bytes) {
                Some(index) => (index, false),
                None => (self.rebuild()?, true),
            },
            None => {
                let index = self.rebuild()?;
                let changed = !index.is_empty();
                (index, changed)
            }
        };
        Ok(Hold {
            store: self,
            index,
            changed,
            _lock: lock,
            _pins: pins,
            _gate: gate,
        })
    }

    /// The index the folders' own records describe, in folder order; the first folder of a file
    /// keeps it.
    fn rebuild(&self) -> io::Result<Vec<Folder>> {
        let mut index = Vec::new();
        for item in fs::read_dir(&self.root)? {
            let item = item?;
            let Some(number) = item.file_name().to_str().and_then(folder_number) else {
                continue;
            };
            if !item.file_type()?.is_dir() {
                continue;
            }
            if let Listing::Current(source, entries) = listing(&item.path())? {
                index.push(described(number, source, &entries));
            }
        }
        index.sort_by_key(|folder: &Folder| folder.number);
        Ok(distinct(index))
    }
}

/// The store while this process holds its lock.
pub(crate) struct Hold<'a> {
    store: &'a Store,
    index: Vec<Folder>,
    /// The index differs from what `index.json` holds.
    changed: bool,
    _lock: HeldLock,
    _pins: Vec<File>,
    _gate: MutexGuard<'a, ()>,
}

impl Hold<'_> {
    /// The entries of `path`, oldest first.
    pub(crate) fn entries(&self, path: &NativePath) -> Result<Vec<Entry>, StoreError> {
        let Some(number) = self.find(path) else {
            return Ok(Vec::new());
        };
        let entries = self.entries_of(number)?.ok_or(StoreError::Newer)?;
        Ok(entries
            .into_iter()
            .map(|stored| Entry {
                id: stored.id,
                source: stored.source,
                label: stored.label,
                timestamp: stored.timestamp,
                size: stored.size,
            })
            .collect())
    }

    /// The copy entry `id` of `path` holds.
    pub(crate) fn content(&self, path: &NativePath, id: &str) -> Result<Vec<u8>, StoreError> {
        let number = self.find(path).ok_or(StoreError::NotFound)?;
        let entries = self.entries_of(number)?.ok_or(StoreError::Newer)?;
        let entry = entries
            .iter()
            .find(|entry| entry.id == id)
            .ok_or(StoreError::NotFound)?;
        let copy = self.folder(number).join(&entry.file);
        read_bounded(&copy, self.store.limits.content)?.ok_or(StoreError::NotFound)
    }

    /// Records `content`, the file at `path` once `source` wrote it, at `timestamp`. A larger file
    /// than [`Limits::content`] records nothing; a file with entries of a newer version neither.
    pub(crate) fn record(
        &mut self,
        path: &NativePath,
        source: Source,
        content: &[u8],
        timestamp: i64,
    ) -> Result<(), StoreError> {
        if length(content) > self.store.limits.content {
            return Ok(());
        }
        let number = match self.find(path) {
            Some(number) => number,
            None => self.create(path, timestamp)?,
        };
        let Some(mut entries) = self.entries_of(number)? else {
            return Ok(());
        };
        self.add(number, path, &mut entries, source, content, timestamp)?;
        self.write_folder(number, path, entries)?;
        self.prune()
    }

    /// Takes the history of `from` and of every file below it to the path each has below `to`, as
    /// VS Code does: a file that already has history there keeps both, and each file then records
    /// the content `current` reads at its new path, at `now`, as a rename when its folder stayed
    /// and a move otherwise.
    pub(crate) fn relocate(
        &mut self,
        from: &NativePath,
        to: &NativePath,
        now: i64,
        current: &mut dyn FnMut(&NativePath) -> Option<Vec<u8>>,
    ) -> Result<(), StoreError> {
        let moving: Vec<(u64, NativePath)> = self
            .index
            .iter()
            .filter(|folder| folder.source.is_within(from))
            .map(|folder| (folder.number, folder.source.clone()))
            .collect();
        for (number, source) in moving {
            let Some(target) = source.rebased(from, to) else {
                continue;
            };
            let Some(entries) = self.entries_of(number)? else {
                continue;
            };
            let (holder, mut entries) = match self.find(&target).filter(|other| *other != number) {
                Some(other) => {
                    let Some(kept) = self.entries_of(other)? else {
                        continue;
                    };
                    let merged = self.merge(number, other, entries, kept)?;
                    self.write_folder(number, &source, Vec::new())?;
                    (other, merged)
                }
                None => (number, entries),
            };
            let renamed = source.parent().map(|parent| parent.key())
                == target.parent().map(|parent| parent.key());
            let kind = if renamed {
                Source::Rename
            } else {
                Source::Move
            };
            if let Some(content) =
                current(&target).filter(|content| length(content) <= self.store.limits.content)
            {
                self.add(holder, &target, &mut entries, kind, &content, now)?;
            }
            self.write_folder(holder, &target, entries)?;
        }
        self.prune()
    }

    /// Labels entry `id` of `path`, which then no later record replaces.
    pub(crate) fn rename(
        &mut self,
        path: &NativePath,
        id: &str,
        label: String,
    ) -> Result<(), StoreError> {
        let number = self.find(path).ok_or(StoreError::NotFound)?;
        let mut entries = self.entries_of(number)?.ok_or(StoreError::Newer)?;
        let entry = entries
            .iter_mut()
            .find(|entry| entry.id == id)
            .ok_or(StoreError::NotFound)?;
        entry.label = Some(label);
        self.write_folder(number, path, entries)
    }

    /// Deletes the entries of `path` that `ids` names; an id it does not have is already gone.
    pub(crate) fn delete(&mut self, path: &NativePath, ids: &[String]) -> Result<(), StoreError> {
        let Some(number) = self.find(path) else {
            return Ok(());
        };
        let mut entries = self.entries_of(number)?.ok_or(StoreError::Newer)?;
        entries.retain(|entry| !ids.contains(&entry.id));
        self.write_folder(number, path, entries)
    }

    /// Deletes every entry of every file, except the folders a newer version keeps.
    pub(crate) fn delete_all(&mut self) -> Result<(), StoreError> {
        for item in fs::read_dir(&self.store.root)? {
            let item = item?;
            if item.file_name().to_str().and_then(folder_number).is_none()
                || !item.file_type()?.is_dir()
            {
                continue;
            }
            if !matches!(listing(&item.path())?, Listing::Newer) {
                remove_tree(&item.path())?;
            }
        }
        self.index.clear();
        self.changed = true;
        Ok(())
    }

    fn find(&self, path: &NativePath) -> Option<u64> {
        let key = path.key();
        self.index
            .iter()
            .find(|folder| folder.key == key)
            .map(|folder| folder.number)
    }

    fn folder(&self, number: u64) -> PathBuf {
        self.store.root.join(folder_name(number))
    }

    /// The entries of folder `number`; `None` when a newer version keeps them.
    fn entries_of(&self, number: u64) -> io::Result<Option<Vec<Stored>>> {
        Ok(match listing(&self.folder(number))? {
            Listing::Empty => Some(Vec::new()),
            Listing::Current(_, entries) => Some(entries),
            Listing::Newer => None,
        })
    }

    /// A new folder for `path`, which joins the index before anything is written into it.
    fn create(&mut self, path: &NativePath, timestamp: i64) -> Result<u64, StoreError> {
        let mut number = self
            .index
            .iter()
            .map(|folder| folder.number)
            .max()
            .unwrap_or(0);
        loop {
            number += 1;
            match create_folder(&self.folder(number)) {
                Ok(()) => break,
                // Left by an interrupted change, or kept for a newer version.
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
        }
        self.index.push(Folder {
            number,
            source: path.clone(),
            key: path.key(),
            size: 0,
            oldest: timestamp,
        });
        self.save_index()?;
        Ok(number)
    }

    /// Writes `content` as a copy in folder `number` and adds it to `entries`: in place of the
    /// newest entry when VS Code's merge window allows, as a new entry otherwise, leaving out the
    /// oldest entries past the limit.
    fn add(
        &self,
        number: u64,
        path: &NativePath,
        entries: &mut Vec<Stored>,
        source: Source,
        content: &[u8],
        timestamp: i64,
    ) -> Result<(), StoreError> {
        let folder = self.folder(number);
        match create_folder(&folder) {
            Err(error) if error.kind() != io::ErrorKind::AlreadyExists => return Err(error.into()),
            _ => {}
        }
        let file = write_copy(
            &folder,
            &time_name(timestamp),
            &extension(path.name()),
            entries,
            content,
        )?;
        let size = length(content);
        let merge = self.store.limits.merge;
        let newest = entries.last_mut().filter(|newest| {
            newest.source == source
                && newest.label.is_none()
                && timestamp.saturating_sub(newest.timestamp) <= merge
        });
        match newest {
            Some(newest) => {
                newest.file = file;
                newest.timestamp = timestamp;
                newest.size = size;
            }
            None => entries.push(Stored {
                id: new_id()?,
                file,
                source,
                label: None,
                timestamp,
                size,
            }),
        }
        entries.sort_by_key(|entry| entry.timestamp);
        let excess = entries.len().saturating_sub(self.store.limits.entries);
        entries.drain(..excess);
        Ok(())
    }

    /// Moves the copies of folder `from` into folder `into` and returns both histories, oldest
    /// first, within the limit.
    fn merge(
        &self,
        from: u64,
        into: u64,
        moving: Vec<Stored>,
        mut kept: Vec<Stored>,
    ) -> Result<Vec<Stored>, StoreError> {
        let (source, target) = (self.folder(from), self.folder(into));
        match create_folder(&target) {
            Err(error) if error.kind() != io::ErrorKind::AlreadyExists => return Err(error.into()),
            _ => {}
        }
        for mut entry in moving {
            if kept.iter().any(|kept| kept.id == entry.id) {
                continue;
            }
            if let Some(file) = move_copy(&source, &target, &entry.file, &kept)? {
                entry.file = file;
                kept.push(entry);
            }
        }
        kept.sort_by_key(|entry| entry.timestamp);
        let excess = kept.len().saturating_sub(self.store.limits.entries);
        kept.drain(..excess);
        Ok(kept)
    }

    /// Replaces folder `number`'s record with `entries` of `path` and removes every copy they do
    /// not name; no entries remove the folder from the store.
    fn write_folder(
        &mut self,
        number: u64,
        path: &NativePath,
        entries: Vec<Stored>,
    ) -> Result<(), StoreError> {
        let folder = self.folder(number);
        self.changed = true;
        if entries.is_empty() {
            remove_tree(&folder)?;
            self.index.retain(|listed| listed.number != number);
            return Ok(());
        }
        let record = EntriesRecord {
            version: VERSION,
            source: path.as_str().to_owned(),
            entries,
        };
        let bytes = serde_json::to_vec(&record).map_err(io::Error::other)?;
        replace(&folder, ENTRIES, ENTRIES_PENDING, &bytes)?;
        for item in fs::read_dir(&folder)? {
            let item = item?;
            let name = item.file_name();
            let named = name.to_str().is_some_and(|name| {
                name == ENTRIES || record.entries.iter().any(|entry| entry.file == name)
            });
            if !named && item.file_type()?.is_file() {
                remove_file(&item.path())?;
            }
        }
        let described = described(number, path.clone(), &record.entries);
        match self.index.iter_mut().find(|listed| listed.number == number) {
            Some(listed) => *listed = described,
            None => self.index.push(described),
        }
        Ok(())
    }

    /// Removes the oldest entries of any file while all copies together exceed the total.
    fn prune(&mut self) -> Result<(), StoreError> {
        while self.index.iter().map(|folder| folder.size).sum::<u64>() > self.store.limits.total {
            let Some(oldest) = self
                .index
                .iter()
                .min_by_key(|folder| (folder.oldest, folder.number))
                .cloned()
            else {
                break;
            };
            match self.entries_of(oldest.number)? {
                Some(mut entries) => {
                    if !entries.is_empty() {
                        entries.remove(0);
                    }
                    self.write_folder(oldest.number, &oldest.source, entries)?;
                }
                // A newer version keeps that folder, which this build then leaves out.
                None => {
                    self.index.retain(|folder| folder.number != oldest.number);
                    self.changed = true;
                }
            }
        }
        Ok(())
    }

    fn save_index(&mut self) -> Result<(), StoreError> {
        let record = IndexRecord {
            version: VERSION,
            folders: self
                .index
                .iter()
                .map(|folder| IndexFolder {
                    folder: folder_name(folder.number),
                    source: folder.source.as_str().to_owned(),
                    size: folder.size,
                    oldest: folder.oldest,
                })
                .collect(),
        };
        let bytes = serde_json::to_vec(&record).map_err(io::Error::other)?;
        replace(&self.store.root, INDEX, INDEX_PENDING, &bytes)?;
        self.changed = false;
        Ok(())
    }
}

/// The index entry of folder `number`, which holds `entries` of `source`.
fn described(number: u64, source: NativePath, entries: &[Stored]) -> Folder {
    Folder {
        number,
        key: source.key(),
        source,
        size: entries.iter().map(|entry| entry.size).sum(),
        oldest: entries
            .iter()
            .map(|entry| entry.timestamp)
            .min()
            .unwrap_or(0),
    }
}

/// `index`, keeping the first folder of each file and of each number.
fn distinct(index: Vec<Folder>) -> Vec<Folder> {
    let (mut keys, mut numbers) = (HashSet::new(), HashSet::new());
    let mut kept = Vec::with_capacity(index.len());
    for folder in index {
        if !keys.contains(&folder.key) && !numbers.contains(&folder.number) {
            keys.insert(folder.key.clone());
            numbers.insert(folder.number);
            kept.push(folder);
        }
    }
    kept
}

/// The index `bytes` hold, when it is one this build writes.
fn parse_index(bytes: &[u8]) -> Option<Vec<Folder>> {
    let record = serde_json::from_slice::<IndexRecord>(bytes).ok()?;
    if record.version != VERSION {
        return None;
    }
    let mut index = Vec::with_capacity(record.folders.len());
    for folder in record.folders {
        let source = NativePath::parse(&folder.source).ok()?;
        index.push(Folder {
            number: folder_number(&folder.folder)?,
            key: source.key(),
            source,
            size: folder.size,
            oldest: folder.oldest,
        });
    }
    Some(distinct(index))
}

/// What the `entries.json` of `folder` holds for this build.
fn listing(folder: &Path) -> io::Result<Listing> {
    let bytes = match read_bounded(&folder.join(ENTRIES), ENTRIES_BYTES) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return Ok(Listing::Empty),
        Err(error) if error.kind() == io::ErrorKind::FileTooLarge => return Ok(Listing::Empty),
        Err(error) => return Err(error),
    };
    if version(&bytes).is_some_and(|version| version > VERSION) {
        return Ok(Listing::Newer);
    }
    let Ok(record) = serde_json::from_slice::<EntriesRecord>(&bytes) else {
        return Ok(Listing::Empty);
    };
    let admitted = record.version == VERSION
        && record.entries.iter().all(|entry| {
            HistoryEntryId::parse(&entry.id).is_ok()
                && valid_file(&entry.file)
                && entry.label.as_ref().is_none_or(|label| valid_label(label))
                && entry.timestamp.unsigned_abs() <= MAXIMUM_SAFE_INTEGER
                && entry.size <= MAXIMUM_SAFE_INTEGER
        });
    match NativePath::parse(&record.source) {
        Ok(source) if admitted => {
            let mut entries = record.entries;
            entries.sort_by_key(|entry| entry.timestamp);
            Ok(Listing::Current(source, entries))
        }
        _ => Ok(Listing::Empty),
    }
}

/// The top-level integer `version` of a record.
fn version(bytes: &[u8]) -> Option<u64> {
    serde_json::from_slice::<Value>(bytes)
        .ok()?
        .get("version")?
        .as_u64()
}

/// Takes `history.lock`, waiting for another holder at most [`LOCK_WAIT`].
fn lock(path: &Path) -> Result<HeldLock, StoreError> {
    let deadline = Instant::now() + LOCK_WAIT;
    loop {
        let file = match private_filesystem::open_file(path, false, true) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                match private_filesystem::open_file(path, true, true) {
                    // Another process created it meanwhile.
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                    opened => opened?,
                }
            }
            opened => opened?,
        };
        match HeldLock::try_exclusive(file) {
            Ok(lock) => return Ok(lock),
            Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                thread::sleep(LOCK_POLL);
            }
            Err(TryLockError::WouldBlock) => return Err(StoreError::Busy),
            Err(TryLockError::Error(error)) => return Err(error.into()),
        }
    }
}

/// The private file at `path`, at most `limit` bytes; `None` when there is none.
fn read_bounded(path: &Path, limit: u64) -> io::Result<Option<Vec<u8>>> {
    let file = match private_filesystem::open_file(path, false, false) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1)).read_to_end(&mut bytes)?;
    if length(&bytes) > limit {
        return Err(io::ErrorKind::FileTooLarge.into());
    }
    Ok(Some(bytes))
}

/// Replaces `name` in `folder` with `bytes` through the staging file `pending`, flushed first, so
/// a reader finds the old record or the new one.
fn replace(folder: &Path, name: &str, pending: &str, bytes: &[u8]) -> io::Result<()> {
    let (target, staged) = (folder.join(name), folder.join(pending));
    // One an interrupted change left.
    remove_file(&staged)?;
    let mut file = private_filesystem::open_file(&staged, true, true)?;
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    let replaced = written.and_then(|()| outlasting(|| fs::rename(&staged, &target)));
    if replaced.is_err() {
        let _ = fs::remove_file(&staged);
    }
    replaced
}

/// Writes `content` as a new copy in `folder`, named `<time><extension>`, or `<time>-<n>` before
/// the extension when that name is taken, and returns its name.
fn write_copy(
    folder: &Path,
    time: &str,
    extension: &str,
    taken: &[Stored],
    content: &[u8],
) -> io::Result<String> {
    for attempt in 1..=NAME_ATTEMPTS {
        let name = if attempt == 1 {
            format!("{time}{extension}")
        } else {
            format!("{time}-{attempt}{extension}")
        };
        if taken.iter().any(|entry| entry.file == name) {
            continue;
        }
        let path = folder.join(&name);
        let mut file = match private_filesystem::open_file(&path, true, true) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        };
        let written = file.write_all(content).and_then(|()| file.sync_all());
        drop(file);
        return match written {
            Ok(()) => Ok(name),
            Err(error) => {
                let _ = fs::remove_file(&path);
                Err(error)
            }
        };
    }
    Err(io::ErrorKind::AlreadyExists.into())
}

/// Moves copy `file` from folder `source` into folder `target` under a name no copy there takes;
/// `None` when the copy is gone.
fn move_copy(
    source: &Path,
    target: &Path,
    file: &str,
    taken: &[Stored],
) -> io::Result<Option<String>> {
    let (stem, extension) = file.find('.').map_or((file, ""), |dot| file.split_at(dot));
    for attempt in 1..=NAME_ATTEMPTS {
        let name = if attempt == 1 {
            file.to_owned()
        } else {
            format!("{stem}-{attempt}{extension}")
        };
        if name == ENTRIES || taken.iter().any(|entry| entry.file == name) {
            continue;
        }
        let destination = target.join(&name);
        // Renaming would replace an existing file; under the lock nothing creates one meanwhile.
        match fs::symlink_metadata(&destination) {
            Ok(_) => continue,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        return match outlasting(|| fs::rename(source.join(file), &destination)) {
            Ok(()) => Ok(Some(name)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        };
    }
    Err(io::ErrorKind::AlreadyExists.into())
}

/// A folder only this user reaches; on Windows it inherits the store's private access list.
pub(super) fn create_folder(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::DirBuilderExt::mode(&mut fs::DirBuilder::new(), 0o700).create(path)
    }
    #[cfg(not(unix))]
    {
        fs::create_dir(path)
    }
}

fn remove_file(path: &Path) -> io::Result<()> {
    match outlasting(|| fs::remove_file(path)) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        removed => removed,
    }
}

fn remove_tree(path: &Path) -> io::Result<()> {
    match outlasting(|| fs::remove_dir_all(path)) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        removed => removed,
    }
}

/// Runs `step` again while another handle holds its object for a moment, as a scanner or an
/// indexer may on Windows, for at most [`LOCK_WAIT`].
fn outlasting<T>(mut step: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    let deadline = Instant::now() + LOCK_WAIT;
    loop {
        match step() {
            Err(error) if held(&error) && Instant::now() < deadline => thread::sleep(LOCK_POLL),
            outcome => return outcome,
        }
    }
}

/// Access denied or a sharing violation, which Windows answers while another handle holds the
/// object; elsewhere nothing holds an object this way.
fn held(error: &io::Error) -> bool {
    cfg!(windows) && matches!(error.raw_os_error(), Some(5 | 32))
}

/// A new entry's id: 32 random lowercase hexadecimal digits, not all zero, as the protocol names
/// an entry.
fn new_id() -> io::Result<String> {
    loop {
        let mut bytes = [0_u8; 16];
        getrandom::fill(&mut bytes).map_err(|_| io::Error::from(io::ErrorKind::Other))?;
        if let Some(id) = HistoryEntryId::from_bytes(bytes) {
            return Ok(id.to_string());
        }
    }
}

/// A label the protocol carries: 1 to 1,024 bytes without NUL.
fn valid_label(label: &str) -> bool {
    (1..=LABEL_LIMIT).contains(&label.len()) && !label.contains('\0')
}

/// A copy's name is one plain name in its folder, never the folder's own records.
fn valid_file(file: &str) -> bool {
    !file.is_empty()
        && file.len() <= 255
        && !file.starts_with('.')
        && file != ENTRIES
        && file != ENTRIES_PENDING
        && file
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn folder_name(number: u64) -> String {
    format!("{number:06}")
}

/// The number a folder's name spells: six or more digits.
fn folder_number(name: &str) -> Option<u64> {
    if name.len() < 6 || !name.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    name.parse()
        .ok()
        .filter(|number| folder_name(*number) == name)
}

/// The extension a copy of `name` keeps, such as `.ts`; none when the name has none or one a copy
/// name cannot carry.
fn extension(name: &str) -> String {
    match name.rfind('.') {
        Some(dot)
            if dot > 0
                && (2..=17).contains(&(name.len() - dot))
                && name[dot + 1..]
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')) =>
        {
            name[dot..].to_owned()
        }
        _ => String::new(),
    }
}

/// `2026-11-27T08-30-15-123Z`: the UTC time `timestamp` milliseconds after the Unix epoch, or the
/// epoch for an earlier one.
fn time_name(timestamp: i64) -> String {
    let milliseconds = timestamp.max(0);
    let (seconds, millisecond) = (milliseconds / 1000, milliseconds % 1000);
    let (days, second) = (seconds / 86_400, seconds % 86_400);
    let (year, month, day) = civil(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}-{:02}-{:02}-{millisecond:03}Z",
        second / 3600,
        second / 60 % 60,
        second % 60,
    )
}

/// The proleptic Gregorian date `days` days after 1970-01-01, by Howard Hinnant's
/// `civil_from_days`.
fn civil(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

fn length(bytes: &[u8]) -> u64 {
    u64::try_from(bytes.len()).unwrap_or(u64::MAX)
}
