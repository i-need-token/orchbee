//! Private Windows cache files; applications own names, versions and content validation.
use crate::windows_permissions as permissions;
use std::{
    fs::{self, File, TryLockError},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};
const LOCK: &str = ".orchbee-update.lock";
const PENDING: &str = ".orchbee-update.pending";

/// Pins private cache ancestors and holds the cross-process writer lock until dropped.
/// The two reserved `.orchbee-update.*` leaves belong to this owner, not user inputs.
pub struct WindowsPrivateCache {
    path: PathBuf,
    _lock: File,
    _pins: Vec<File>,
}

/// One read whose admitted private ancestors remain pinned until validation completes.
pub(crate) struct WindowsPrivateRead {
    bytes: Option<Vec<u8>>,
    _pins: Vec<File>,
    _source: Option<File>,
}

/// One guarded configuration publication, with a definite observed-source conflict.
pub(crate) enum ConfigurationWriteFailure {
    SourceChanged,
    Io,
}

impl WindowsPrivateRead {
    pub(crate) fn bytes(&self) -> Option<&[u8]> {
        self.bytes.as_deref()
    }

    fn into_bytes(self) -> Option<Vec<u8>> {
        self.bytes
    }
}

impl WindowsPrivateCache {
    /// Creates missing private descendants; rejects unsafe existing objects without repair.
    pub fn acquire(path: &Path) -> io::Result<Self> {
        let pins = create_tree(path)?;
        let lock_path = path.join(LOCK);
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
            path: path.to_owned(),
            _lock: lock,
            _pins: pins,
        };
        cache.recover_pending()?;
        Ok(cache)
    }
    /// No symlinks/reparse points, hard links, nonregular files or foreign ACLs are followed.
    pub fn read(&self, name: &str, maximum: usize) -> io::Result<Option<Vec<u8>>> {
        read_leaf(&self.leaf(name)?, maximum)
    }
    /// Reads an existing private cache without creating directories or acquiring a writer lock.
    pub fn read_existing(path: &Path, name: &str, maximum: usize) -> io::Result<Option<Vec<u8>>> {
        Self::read_existing_pinned(path, name, maximum).map(WindowsPrivateRead::into_bytes)
    }
    /// Reads while retaining every admitted ancestor handle for caller-side content validation.
    pub(crate) fn read_existing_pinned(
        path: &Path,
        name: &str,
        maximum: usize,
    ) -> io::Result<WindowsPrivateRead> {
        let leaf = cache_leaf(path, name)?;
        let pins = match permissions::open_tree(path) {
            Ok(pins) => pins,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(WindowsPrivateRead {
                    bytes: None,
                    _pins: Vec::new(),
                    _source: None,
                });
            }
            Err(error) => return Err(error),
        };
        let bytes = read_leaf(&leaf, maximum)?;
        Ok(WindowsPrivateRead {
            bytes,
            _pins: pins,
            _source: None,
        })
    }
    /// The frozen user-configuration leaf accepts Unicode names while rejecting Windows aliases.
    pub(crate) fn read_configuration_existing_pinned(
        path: &Path,
        maximum: usize,
    ) -> io::Result<WindowsPrivateRead> {
        let parent = path.parent().ok_or(io::ErrorKind::InvalidInput)?;
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(io::ErrorKind::InvalidInput)?;
        let leaf = configuration_leaf(parent, name)?;
        let pins = match permissions::open_tree(parent) {
            Ok(pins) => pins,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(WindowsPrivateRead {
                    bytes: None,
                    _pins: Vec::new(),
                    _source: None,
                });
            }
            Err(error) => return Err(error),
        };
        let (bytes, source) = read_snapshot_leaf(&leaf, maximum)?;
        Ok(WindowsPrivateRead {
            bytes,
            _pins: pins,
            _source: source,
        })
    }
    /// Read one admitted configuration snapshot under the already held writer lock.
    pub(crate) fn read_configuration(
        &self,
        name: &str,
        maximum: usize,
    ) -> io::Result<Option<Vec<u8>>> {
        read_snapshot_leaf(&configuration_leaf(&self.path, name)?, maximum).map(|(bytes, _)| bytes)
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
        let target =
            configuration_leaf(&self.path, name).map_err(|_| ConfigurationWriteFailure::Io)?;
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
        let pending = self.path.join(PENDING);
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
        let pending = self.path.join(PENDING);
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
    fn leaf(&self, name: &str) -> io::Result<PathBuf> {
        cache_leaf(&self.path, name)
    }
    fn recover_pending(&mut self) -> io::Result<()> {
        let path = self.path.join(PENDING);
        match permissions::open_file(&path, false, false) {
            Ok(file) => {
                drop(file);
                fs::remove_file(path)
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
        || name.eq_ignore_ascii_case(LOCK)
        || name.eq_ignore_ascii_case(PENDING)
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
fn create_tree(path: &Path) -> io::Result<Vec<File>> {
    match permissions::open_tree(path) {
        Ok(pins) => Ok(pins),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let parent = path
                .parent()
                .filter(|p| *p != path)
                .ok_or(io::ErrorKind::InvalidInput)?;
            let mut pins = create_tree(parent)?;
            let created = match fs::create_dir(path) {
                Ok(()) => true,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => false,
                Err(error) => return Err(error),
            };
            let mut file = permissions::open_directory(path, created)?;
            if created {
                permissions::protect_new(&mut file)?;
            } else {
                permissions::require_private(&file)?;
            }
            pins.push(file);
            Ok(pins)
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests;
