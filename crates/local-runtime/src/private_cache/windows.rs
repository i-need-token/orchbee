//! Private Windows cache files; applications own names, versions and content validation.
use crate::private_filesystem as permissions;
use std::{
    fs::{self, File, TryLockError},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};
const LOCK_EXTENSION: &str = "lock";
const PENDING_EXTENSION: &str = "pending";

/// Pins private cache ancestors and holds the cross-process writer lock of one record until
/// dropped. The record's lock `<record>.lock` and staging file `<record>.pending` belong to this
/// owner, not user inputs; both sit beside the record's files unless the owner keeps the lock
/// elsewhere.
pub struct PrivateCache {
    path: PathBuf,
    record: String,
    pending: PathBuf,
    _lock: File,
    _pins: Vec<File>,
}

/// One read whose admitted private ancestors remain pinned until validation completes.
pub(crate) struct PrivateRead {
    bytes: Option<Vec<u8>>,
    _pins: Vec<File>,
}

/// One guarded configuration publication, with a definite observed-source conflict.
pub(crate) enum ConfigurationWriteFailure {
    SourceChanged,
    Io,
}

impl PrivateRead {
    pub(crate) fn bytes(&self) -> Option<&[u8]> {
        self.bytes.as_deref()
    }

    fn into_bytes(self) -> Option<Vec<u8>> {
        self.bytes
    }
}

impl PrivateCache {
    /// Guards `record` in `directory`, creating missing private directories and rejecting unsafe
    /// existing objects without repair. `record` names the lock and staging file: `preferences`
    /// guards `preferences.json` with `preferences.lock` and `preferences.pending`.
    pub fn acquire(directory: &Path, record: &str) -> io::Result<Self> {
        Self::acquire_with_lock_in(directory, record, directory)
    }
    /// Like [`Self::acquire`], but keeps the persistent lock in `lock_directory`, so a record in a
    /// folder a person reads gains only a staging file, and only while a write runs.
    pub(crate) fn acquire_with_lock_in(
        directory: &Path,
        record: &str,
        lock_directory: &Path,
    ) -> io::Result<Self> {
        if !valid_record(record) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let mut pins = crate::data_root::create_state_directory(directory)?;
        if lock_directory != directory {
            pins.extend(crate::data_root::create_state_directory(lock_directory)?);
        }
        let lock_path = lock_directory.join(format!("{record}.{LOCK_EXTENSION}"));
        let lock = match permissions::open_file(&lock_path, true, true) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                permissions::open_file(&lock_path, false, true)?
            }
            Err(error) => return Err(error),
        };
        match lock.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => return Err(io::ErrorKind::WouldBlock.into()),
            Err(TryLockError::Error(error)) => return Err(error),
        }
        let mut cache = Self {
            path: directory.to_owned(),
            record: record.to_owned(),
            pending: directory.join(format!("{record}.{PENDING_EXTENSION}")),
            _lock: lock,
            _pins: pins,
        };
        cache.recover_pending()?;
        Ok(cache)
    }
    /// Creates missing private directories and admits existing ones without taking a lock, for a
    /// folder whose contents another owner writes.
    pub fn prepare(directory: &Path) -> io::Result<()> {
        crate::data_root::create_state_directory(directory).map(drop)
    }
    /// No symlinks/reparse points, hard links, nonregular files or foreign ACLs are followed.
    pub fn read(&self, name: &str, maximum: usize) -> io::Result<Option<Vec<u8>>> {
        read_leaf(&self.leaf(name)?, maximum)
    }
    /// Reads an existing private cache without creating directories or acquiring a writer lock.
    pub fn read_existing(path: &Path, name: &str, maximum: usize) -> io::Result<Option<Vec<u8>>> {
        Self::read_existing_pinned(path, name, maximum).map(PrivateRead::into_bytes)
    }
    /// Reads while retaining every admitted ancestor handle for caller-side content validation.
    pub(crate) fn read_existing_pinned(
        path: &Path,
        name: &str,
        maximum: usize,
    ) -> io::Result<PrivateRead> {
        let leaf = cache_leaf(path, name)?;
        let pins = match permissions::open_tree(path) {
            Ok(pins) => pins,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(PrivateRead {
                    bytes: None,
                    _pins: Vec::new(),
                });
            }
            Err(error) => return Err(error),
        };
        let bytes = read_leaf(&leaf, maximum)?;
        Ok(PrivateRead { bytes, _pins: pins })
    }
    /// Read one admitted configuration snapshot under the already held writer lock.
    pub(crate) fn read_configuration(
        &self,
        name: &str,
        maximum: usize,
    ) -> io::Result<Option<Vec<u8>>> {
        read_snapshot_leaf(&self.configuration_leaf(name)?, maximum).map(|(bytes, _)| bytes)
    }
    /// Compare after flushing the candidate. An absent observed target uses a no-replace move.
    /// The preferences owner admits a larger, separately bounded launch-profile catalog.
    pub(crate) fn publish_configuration_bounded(
        &mut self,
        name: &str,
        before: Option<&[u8]>,
        after: &[u8],
        maximum: usize,
    ) -> Result<bool, ConfigurationWriteFailure> {
        if after.len() > maximum {
            return Err(ConfigurationWriteFailure::Io);
        }
        let target = self
            .configuration_leaf(name)
            .map_err(|_| ConfigurationWriteFailure::Io)?;
        let observed = self
            .read_configuration(name, maximum)
            .map_err(|_| ConfigurationWriteFailure::Io)?;
        if observed.as_deref() != before {
            return Err(ConfigurationWriteFailure::SourceChanged);
        }
        if before == Some(after) {
            return Ok(false);
        }
        self.recover_pending()
            .map_err(|_| ConfigurationWriteFailure::Io)?;
        let pending = self.pending.clone();
        let result = (|| {
            let mut file = permissions::open_file(&pending, true, true)
                .map_err(|_| ConfigurationWriteFailure::Io)?;
            file.write_all(after)
                .and_then(|()| file.sync_all())
                .map_err(|_| ConfigurationWriteFailure::Io)?;
            drop(file);
            let observed = self
                .read_configuration(name, maximum)
                .map_err(|_| ConfigurationWriteFailure::Io)?;
            if observed.as_deref() != before {
                return Err(ConfigurationWriteFailure::SourceChanged);
            }
            winsafe::MoveFileEx(
                pending.to_str().ok_or(ConfigurationWriteFailure::Io)?,
                Some(target.to_str().ok_or(ConfigurationWriteFailure::Io)?),
                if before.is_some() {
                    winsafe::co::MOVEFILE::REPLACE_EXISTING
                } else {
                    winsafe::co::MOVEFILE::WRITE_THROUGH
                },
            )
            .map_err(|_| ConfigurationWriteFailure::Io)?;
            Ok(true)
        })();
        if result.is_err() {
            self.recover_pending()
                .map_err(|_| ConfigurationWriteFailure::Io)?;
        }
        result
    }
    /// Flushes a complete new value before replacing the admitted leaf. No partial target is visible.
    pub fn replace(&mut self, name: &str, bytes: &[u8]) -> io::Result<()> {
        let target = self.leaf(name)?;
        match permissions::open_file(&target, false, false) {
            Ok(file) => drop(file),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        self.recover_pending()?;
        let pending = self.pending.clone();
        let mut file = permissions::open_file(&pending, true, true)?;
        let write = file.write_all(bytes).and_then(|()| file.sync_all());
        drop(file);
        let result = write.and_then(|()| {
            winsafe::MoveFileEx(
                pending.to_str().ok_or(io::ErrorKind::InvalidInput)?,
                Some(target.to_str().ok_or(io::ErrorKind::InvalidInput)?),
                winsafe::co::MOVEFILE::REPLACE_EXISTING,
            )
            .map_err(io::Error::other)
        });
        if result.is_err() {
            self.recover_pending()?;
        }
        result
    }
    /// Removes one admitted leaf under the writer lock; an absent leaf is already removed.
    pub fn remove(&mut self, name: &str) -> io::Result<()> {
        let target = self.leaf(name)?;
        match permissions::open_file(&target, false, false) {
            Ok(file) => drop(file),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        }
        fs::remove_file(target)
    }
    fn leaf(&self, name: &str) -> io::Result<PathBuf> {
        self.require_unreserved(name)?;
        cache_leaf(&self.path, name)
    }
    fn configuration_leaf(&self, name: &str) -> io::Result<PathBuf> {
        self.require_unreserved(name)?;
        configuration_leaf(&self.path, name)
    }
    /// The lock and staging file are this owner's, never a record a caller reads or replaces.
    fn require_unreserved(&self, name: &str) -> io::Result<()> {
        let reserved = [LOCK_EXTENSION, PENDING_EXTENSION]
            .iter()
            .any(|extension| name.eq_ignore_ascii_case(&format!("{}.{extension}", self.record)));
        if reserved {
            Err(io::ErrorKind::InvalidInput.into())
        } else {
            Ok(())
        }
    }
    fn recover_pending(&mut self) -> io::Result<()> {
        match permissions::open_file(&self.pending, false, false) {
            Ok(file) => {
                drop(file);
                fs::remove_file(&self.pending)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
}
fn cache_leaf(path: &Path, name: &str) -> io::Result<PathBuf> {
    if name.is_empty()
        || name.len() > 96
        || name.starts_with('.')
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    Ok(path.join(name))
}
fn configuration_leaf(path: &Path, name: &str) -> io::Result<PathBuf> {
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let reserved = matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || stem
        .strip_prefix("COM")
        .or_else(|| stem.strip_prefix("LPT"))
        .is_some_and(|suffix| {
            matches!(
                suffix,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        });
    if name.is_empty()
        || name.encode_utf16().count() > 255
        || matches!(name, "." | "..")
        || name.ends_with(['.', ' '])
        || name.chars().any(|ch| {
            ch.is_control() || matches!(ch, '/' | '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*')
        })
        || reserved
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    Ok(path.join(name))
}
fn read_snapshot_leaf(path: &Path, maximum: usize) -> io::Result<(Option<Vec<u8>>, Option<File>)> {
    let file = match permissions::open_snapshot_file(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok((None, None)),
        Err(error) => return Err(error),
    };
    let bound = u64::try_from(maximum).map_err(|_| io::ErrorKind::InvalidInput)?;
    if file.metadata()?.len() > bound {
        return Err(io::ErrorKind::FileTooLarge.into());
    }
    let mut bytes = Vec::new();
    (&file)
        .take(bound.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() > maximum {
        return Err(io::ErrorKind::FileTooLarge.into());
    }
    Ok((Some(bytes), Some(file)))
}
fn read_leaf(path: &Path, maximum: usize) -> io::Result<Option<Vec<u8>>> {
    let file = match permissions::open_file(path, false, false) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let bound = u64::try_from(maximum).map_err(|_| io::ErrorKind::InvalidInput)?;
    if file.metadata()?.len() > bound {
        return Err(io::ErrorKind::FileTooLarge.into());
    }
    let mut bytes = Vec::new();
    file.take(bound.saturating_add(1)).read_to_end(&mut bytes)?;
    if bytes.len() > maximum {
        return Err(io::ErrorKind::FileTooLarge.into());
    }
    Ok(Some(bytes))
}
/// A record name is one plain word or hyphenated phrase, so its lock and staging names stay plain.
fn valid_record(record: &str) -> bool {
    !record.is_empty()
        && record.len() <= 64
        && record
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

#[cfg(test)]
mod tests;
