//! Private immutable releases. The caller owns release contents, validation,
//! activation and retention; this owner serializes publishers and cleans only its own stage.
use super::platform;
use orchbee_local_runtime::{PrivateCache, private_filesystem as permissions};
use std::{
    collections::BTreeSet,
    fs::File,
    io,
    path::{Path, PathBuf},
};

/// Exclusive publisher for one managed installation, with pinned private ancestors.
pub(crate) struct ReleaseStore {
    root: PathBuf,
    _lock: PrivateCache,
}
impl ReleaseStore {
    /// Creates missing private directories and refuses unsafe existing installations.
    pub(crate) fn acquire(root: &Path) -> io::Result<Self> {
        let lock = PrivateCache::acquire(root)?;
        Ok(Self {
            root: root.to_owned(),
            _lock: lock,
        })
    }
    /// The immutable destination for a validated lowercase SHA-256 manifest identity.
    pub(crate) fn release_path(&self, identity: &str) -> io::Result<PathBuf> {
        if identity.len() != 64
            || !identity
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        Ok(self.root.join(identity))
    }
    /// Creates a unique stage. Its lifetime cannot outlive the publisher lock.
    pub(crate) fn stage(&self) -> io::Result<ReleaseStage<'_>> {
        let (path, pins) = platform::create_stage(&self.root)?;
        Ok(ReleaseStage {
            store: self,
            pinned_directories: BTreeSet::from([path.clone()]),
            path,
            pins,
            owned: true,
        })
    }
}

/// One unpublished directory, removed on explicit cleanup or as a Drop fallback.
pub(crate) struct ReleaseStage<'a> {
    store: &'a ReleaseStore,
    path: PathBuf,
    pins: Vec<File>,
    pinned_directories: BTreeSet<PathBuf>,
    owned: bool,
}
impl ReleaseStage<'_> {
    /// Absolute stage root for validators; it is never an active installation.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// Creates an exclusive private regular file at a portable relative release path.
    pub(crate) fn create_file(&mut self, relative: &str) -> io::Result<(PathBuf, File)> {
        if !valid_release_path(relative) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let mut parent = self.path.clone();
        let mut parts = relative.split('/').peekable();
        while let Some(part) = parts.next() {
            if parts.peek().is_none() {
                break;
            }
            parent.push(part);
            match platform::create_directory(&parent) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
            if !self.pinned_directories.contains(&parent) {
                // Earlier components are already pinned by this stage.
                let file = permissions::open_directory(&parent, false)?;
                permissions::require_private(&file)?;
                self.pins.push(file);
                self.pinned_directories.insert(parent.clone());
            }
        }
        let path = self.path.join(relative);
        let file = permissions::open_file(&path, true, true)?;
        Ok((path, file))
    }
    /// Publishes without replacing an existing directory. Failure leaves this stage owned.
    pub(crate) fn publish(&mut self, identity: &str) -> io::Result<PathBuf> {
        let target = self.store.release_path(identity)?;
        self.pins.clear();
        platform::publish(&self.path, &target)?;
        self.owned = false;
        Ok(target)
    }
    /// Releases only this invocation's stage; a failure leaves its path available for recovery.
    pub(crate) fn cleanup(&mut self) -> io::Result<()> {
        if !self.owned {
            return Ok(());
        }
        self.pins.clear();
        let pins = permissions::open_tree(&self.path)?;
        if self.path.parent() != Some(self.store.root.as_path()) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        drop(pins);
        platform::remove_stage(&self.path)?;
        self.owned = false;
        Ok(())
    }
    /// Transfers cleanup to explicit recovery when a child may still be using the stage.
    /// The caller must retain and report the returned path; Drop no longer deletes it.
    pub(crate) fn retain_for_recovery(&mut self) -> PathBuf {
        self.pins.clear();
        self.owned = false;
        self.path.clone()
    }
}
impl Drop for ReleaseStage<'_> {
    fn drop(&mut self) {
        // Normal callers report cleanup errors explicitly before Drop. This fallback
        // covers unwinding only and must not panic or delete an unowned release.
        let _cleanup = self.cleanup();
    }
}

/// Portable release leaf grammar; rejects aliases, device names, traversal and NTFS streams.
pub(crate) fn valid_release_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 240
        && value.split('/').count() <= 12
        && value.split('/').all(|part| {
            !part.is_empty()
                && part.len() <= 120
                && !matches!(part, "." | "..")
                && !part.ends_with('.')
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                && !matches!(
                    part.split('.')
                        .next()
                        .unwrap_or("")
                        .to_ascii_uppercase()
                        .as_str(),
                    "CON"
                        | "PRN"
                        | "AUX"
                        | "NUL"
                        | "COM1"
                        | "COM2"
                        | "COM3"
                        | "COM4"
                        | "COM5"
                        | "COM6"
                        | "COM7"
                        | "COM8"
                        | "COM9"
                        | "LPT1"
                        | "LPT2"
                        | "LPT3"
                        | "LPT4"
                        | "LPT5"
                        | "LPT6"
                        | "LPT7"
                        | "LPT8"
                        | "LPT9"
                )
        })
}

/// Pins ancestors while opening a private release artifact without following links.
pub(crate) fn open_release_file(directory: &Path, relative: &str) -> io::Result<File> {
    if !valid_release_path(relative) {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let path = directory.join(relative);
    let _pins = permissions::open_tree(path.parent().ok_or(io::ErrorKind::InvalidInput)?)?;
    permissions::open_file(&path, false, false)
}
