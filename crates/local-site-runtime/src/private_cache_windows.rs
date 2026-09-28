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
        let leaf = cache_leaf(path, name)?;
        let _pins = match permissions::open_tree(path) {
            Ok(pins) => pins,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        read_leaf(&leaf, maximum)
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
mod tests {
    use super::*;
    #[test]
    fn private_cache_serializes_writers_and_preserves_rejected_targets() {
        struct Root(PathBuf);
        impl Drop for Root {
            fn drop(&mut self) {
                let actual = self.0.canonicalize().unwrap();
                assert_eq!(
                    actual.parent(),
                    Some(std::env::temp_dir().canonicalize().unwrap().as_path())
                );
                assert!(
                    actual
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .starts_with("orchbee-private-cache-")
                );
                fs::remove_dir_all(actual).unwrap();
            }
        }
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = Root(std::env::temp_dir().join(format!("orchbee-private-cache-{:032x}", nonce)));
        fs::create_dir(&root.0).unwrap();
        let path = root.0.join("nested").join("remote");
        assert!(
            WindowsPrivateCache::read_existing(&path, "status.json", 10)
                .unwrap()
                .is_none()
        );
        assert!(!path.exists());
        let mut cache = WindowsPrivateCache::acquire(&path).unwrap();
        assert!(
            matches!(WindowsPrivateCache::acquire(&path),Err(e) if e.kind()==io::ErrorKind::WouldBlock)
        );
        assert!(cache.read("codex.toml", 4).unwrap().is_none());
        cache.replace("codex.toml", b"old").unwrap();
        cache.replace("codex.toml", b"new").unwrap();
        assert_eq!(cache.read("codex.toml", 3).unwrap(), Some(b"new".to_vec()));
        assert_eq!(
            cache.read("codex.toml", 2).unwrap_err().kind(),
            io::ErrorKind::FileTooLarge
        );
        assert!(cache.replace("../other", b"bad").is_err());
        fs::hard_link(path.join("codex.toml"), path.join("alias.toml")).unwrap();
        assert!(cache.replace("codex.toml", b"bad").is_err());
        assert_eq!(fs::read(path.join("codex.toml")).unwrap(), b"new");
        fs::remove_file(path.join("alias.toml")).unwrap();
        assert!(fs::rename(&path, root.0.join("redirected")).is_err());
        assert!(!path.join(PENDING).exists());
        drop(cache);
        fs::write(path.join(PENDING), b"interrupted stage").unwrap();
        let cache = WindowsPrivateCache::acquire(&path).unwrap();
        assert!(!path.join(PENDING).exists());
        drop(cache);
    }
}
