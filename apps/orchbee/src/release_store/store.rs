//! Private immutable releases under one store root: `versions/<version>/`, the pointer
//! `active.json` and the writer lock `install.lock`. The caller owns release contents, validation,
//! the pointer's meaning and retention; this owner serializes writers, publishes whole version
//! directories, replaces the pointer atomically and cleans only its own stage. It opens no other
//! name in the root, so a folder another installer keeps there, such as `desktop`, is never read.
//!
//! Creation order: whichever writer comes first creates the root. A missing root is created with a
//! protected private access rule, so a `desktop` folder written afterwards inherits it; an existing
//! root, such as one an installer created for `desktop`, is admitted only when it is already
//! private and is never repaired, so a foreign owner or access rule refuses before anything is
//! written. Private is the shared rule of `private_filesystem`: on Windows the user, SYSTEM or
//! Administrators owns it, as an elevated session's objects belong to Administrators, and no other
//! account is let in.
use super::platform;
// The shared store reader owns the folder and pointer names and the pointer bound, so Session start
// and this writer spell them once.
use orchbee_local_runtime::{
    PrivateCache, RELEASE_POINTER as POINTER, RELEASE_POINTER_LIMIT as POINTER_LIMIT,
    RELEASE_VERSIONS as VERSIONS, private_filesystem as permissions,
};
use std::{
    collections::BTreeSet,
    fs::File,
    io,
    path::{Path, PathBuf},
};

/// Exclusive writer for one store root, holding `install.lock` with pinned private ancestors.
pub(crate) struct ReleaseStore {
    root: PathBuf,
    pub(super) lock: PrivateCache,
}
impl ReleaseStore {
    /// Creates a missing root and `versions/` as private directories and refuses unsafe existing
    /// ones; a crashed writer's `active.pending` is removed before the lock is returned.
    pub(crate) fn acquire(root: &Path) -> io::Result<Self> {
        let lock = PrivateCache::acquire_with_lock(root, "install", "active")?;
        PrivateCache::prepare(&root.join(VERSIONS))?;
        Ok(Self {
            root: root.to_owned(),
            lock,
        })
    }
    /// The immutable directory of one version under this root.
    pub(crate) fn version_path(&self, version: &str) -> io::Result<PathBuf> {
        version_path(&self.root, version)
    }
    /// The pointer's bytes under the lock; absent when no pointer was written.
    pub(crate) fn pointer(&self) -> io::Result<Option<Vec<u8>>> {
        self.lock.read(POINTER, POINTER_LIMIT)
    }
    /// Replaces the pointer by a flushed rename of `active.pending`; no partial pointer is visible.
    pub(crate) fn replace_pointer(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.lock.replace(POINTER, bytes)
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
                permissions::require_private(&file)
                    .map_err(|error| permissions::naming(error, &parent))?;
                self.pins.push(file);
                self.pinned_directories.insert(parent.clone());
            }
        }
        let path = self.path.join(relative);
        let file = permissions::open_file(&path, true, true)?;
        Ok((path, file))
    }
    /// Publishes as `versions/<version>` without replacing an existing directory. Failure leaves
    /// this stage owned.
    pub(crate) fn publish(&mut self, version: &str) -> io::Result<PathBuf> {
        let target = self.store.version_path(version)?;
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

/// The directory of `version` under a store root. A version name is one portable path part that
/// starts with a digit, as every SemVer version does.
pub(crate) fn version_path(root: &Path, version: &str) -> io::Result<PathBuf> {
    if version.len() > 64
        || !version.starts_with(|c: char| c.is_ascii_digit())
        || version.contains('/')
        || !valid_release_path(version)
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    Ok(root.join(VERSIONS).join(version))
}

/// The store root and version name of a version directory, `<root>/versions/<version>`.
pub(crate) fn version_directory_parts(directory: &Path) -> Option<(&Path, &str)> {
    let version = directory.file_name()?.to_str()?;
    let versions = directory.parent()?;
    let root = versions.parent()?;
    (versions.file_name()? == VERSIONS && version_path(root, version).is_ok())
        .then_some((root, version))
}

/// Reads the pointer of an existing store without creating anything or taking the lock; absent
/// when the root or the pointer is missing. A replacement is a whole-file rename, so a reader sees
/// the old pointer or the new one.
pub(crate) fn read_pointer(root: &Path) -> io::Result<Option<Vec<u8>>> {
    PrivateCache::read_existing(root, POINTER, POINTER_LIMIT)
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
