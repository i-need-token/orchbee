//! Private immutable releases under one store root: `versions/<version>/`, the pointer
//! `active.json` and the writer lock `install.lock`. The caller owns release contents, validation,
//! the pointer's meaning and which versions to keep; this owner serializes writers, publishes whole
//! version directories, replaces the pointer atomically, cleans its own stage, and removes a version
//! the caller names by moving it aside as `.remove-<process>-<number>` before deleting it, so
//! `versions/` only ever holds whole versions. Besides those names it opens only the `.stage-*` and
//! `.remove-*` folders writers leave, and a folder its caller names for removal the same way, so a
//! folder another installer keeps in the root, such as `desktop`, is never read; the window part
//! opens `desktop` as a store root of its own. A stage
//! creates files at release paths, or, for a macOS window version, at bundle paths, with the
//! symbolic links that version lists.
//!
//! Creation order: whichever writer comes first creates the root. A missing root is created with a
//! protected private access rule, so a `desktop` folder written afterwards inherits it; an existing
//! root is admitted when it is already private, and any other refuses before anything is written,
//! with the one Windows exception below. Private is the shared rule of `private_filesystem`: on
//! Windows the user, SYSTEM or Administrators owns it, as an elevated session's objects belong to
//! Administrators, and no other account is let in; on macOS and Linux the user owns it with mode
//! 0700 and, on macOS, no access list.
//!
//! On Windows a missing root is created below its existing parent without judging that parent, and
//! a missing parent is created as a private folder below the nearest existing one, which must
//! already be private. The root an installer created for `desktop` inherits the access list of the
//! folder above it, which on some profiles lets another account in: a root that admission refuses
//! only for its access list is made private first when the user, SYSTEM or Administrators owns it,
//! it is no link, and it holds nothing but the installer's own `desktop`, `desktop.previous`,
//! `unins000.exe` and `unins000.dat`. On macOS and Linux an existing root is never repaired, and a
//! missing root's missing ancestors are created first, private as well, since the folders above the
//! root need not be private.
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
    /// Creates a missing root and `versions/` as private directories, makes an installer's root
    /// private as the module says, and refuses other unsafe existing ones; a crashed writer's
    /// `active.pending` is removed before the lock is returned.
    pub(crate) fn acquire(root: &Path) -> io::Result<Self> {
        platform::prepare_root(root)?;
        let lock = PrivateCache::acquire_with_lock(root, "install", "active")?;
        PrivateCache::prepare(&root.join(VERSIONS))?;
        Ok(Self {
            root: root.to_owned(),
            lock,
        })
    }
    /// The root this lock guards.
    #[cfg(not(target_os = "macos"))]
    pub(crate) fn root(&self) -> &Path {
        &self.root
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
    /// The real directories under `versions/` that carry a version name, by name. A link, a file or
    /// another name there is left out, so nothing reached through one is ever removed.
    pub(crate) fn versions(&self) -> io::Result<Vec<String>> {
        Ok(platform::directories(&self.root.join(VERSIONS))?
            .into_iter()
            .filter(|name| version_path(&self.root, name).is_ok())
            .collect())
    }
    /// The real directories in the root that a writer leaves there only while it works: removals
    /// that stopped part way and stages of runs that ended without their cleanup. No writer works
    /// in them while this lock is held.
    pub(crate) fn leftovers(&self) -> io::Result<Vec<PathBuf>> {
        Ok(platform::directories(&self.root)?
            .into_iter()
            .filter(|name| transient_name(name))
            .map(|name| self.root.join(name))
            .collect())
    }
    /// Removes `versions/<version>`: a rename to a fresh `.remove-<process>-<number>` in the root
    /// first, so `versions/` only ever holds whole versions, then a deletion. A directory that cannot
    /// be moved stays whole, and what cannot be deleted stays aside among the leftovers.
    pub(crate) fn remove_version(&self, version: &str) -> io::Result<()> {
        let aside = platform::move_aside(&self.root, &self.version_path(version)?)?;
        platform::remove_directory(&aside)
    }
    /// Removes a real directory below the root that is no version, as the window store removes the
    /// bundled release directory of a flat window layout: a rename to a fresh `.remove-*` in the
    /// root first, then a deletion, as [`Self::remove_version`] does.
    #[cfg(windows)]
    pub(crate) fn remove_aside(&self, directory: &Path) -> io::Result<()> {
        let real = std::fs::symlink_metadata(directory)?;
        if !directory.starts_with(&self.root)
            || directory == self.root
            || !real.is_dir()
            || real.is_symlink()
        {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let aside = platform::move_aside(&self.root, directory)?;
        platform::remove_directory(&aside)
    }
    /// Deletes one directory [`Self::leftovers`] named; what cannot be deleted stays.
    pub(crate) fn remove_leftover(&self, path: &Path) -> io::Result<()> {
        let leftover = path.parent() == Some(self.root.as_path())
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(transient_name);
        if !leftover {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        platform::remove_directory(path)
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
        self.create_file_as(PathGrammar::Release, relative)
    }

    /// Creates an exclusive private regular file at a relative path of `grammar`.
    pub(crate) fn create_file_as(
        &mut self,
        grammar: PathGrammar,
        relative: &str,
    ) -> io::Result<(PathBuf, File)> {
        if !grammar.admits(relative) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        self.create_parents(relative)?;
        let path = self.path.join(relative);
        let file = permissions::open_file(&path, true, true)?;
        Ok((path, file))
    }

    /// Creates a symbolic link at the bundle path `relative` naming `target`, a relative path whose
    /// parts are bundle path parts or `..`; the caller checked that it stays inside the version.
    /// Only a macOS window version lists links, so Windows refuses.
    pub(crate) fn create_link(&mut self, relative: &str, target: &str) -> io::Result<PathBuf> {
        let relative_target = !target.is_empty()
            && target
                .split('/')
                .all(|part| part == ".." || valid_bundle_path(part));
        if !valid_bundle_path(relative) || !relative_target {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        self.create_parents(relative)?;
        let path = self.path.join(relative);
        platform::create_link(target, &path)?;
        Ok(path)
    }

    /// Creates and pins the folders above `relative` in this stage. A folder that exists must be a
    /// private folder, never a link.
    fn create_parents(&mut self, relative: &str) -> io::Result<()> {
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
        Ok(())
    }
    /// Makes a program file this stage created executable, where the system asks that of a
    /// program; the cohort's admission checks it.
    pub(crate) fn mark_executable(&self, file: &File) -> io::Result<()> {
        platform::mark_executable(file)
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
        platform::remove_directory(&self.path)?;
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

/// Whether `name` is a directory a writer leaves in the root only while it works: a stage,
/// `.stage-<process>-<number>`, or a removal, `.remove-<process>-<number>`.
pub(crate) fn transient_name(name: &str) -> bool {
    [".stage-", ".remove-"]
        .iter()
        .any(|prefix| numbered_name(name, prefix))
}

/// Whether `name` is `<prefix><process>-<number>`, both parts decimal, as a writer names what it
/// leaves only while it works.
pub(crate) fn numbered_name(name: &str, prefix: &str) -> bool {
    name.strip_prefix(prefix)
        .and_then(|rest| rest.split_once('-'))
        .is_some_and(|(process, number)| {
            [process, number]
                .iter()
                .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
        })
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

/// The grammar of a path a store version lists: the portable release grammar, or a macOS
/// application bundle's.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PathGrammar {
    Release,
    /// Only macOS window versions use it.
    Bundle,
}

impl PathGrammar {
    pub(crate) fn admits(self, value: &str) -> bool {
        match self {
            Self::Release => valid_release_path(value),
            Self::Bundle => valid_bundle_path(value),
        }
    }
}

/// A bundle path: the release grammar without its device names, whose parts also admit spaces
/// and parentheses, since Electron names its helper applications and frameworks so, such as
/// `Orchbee Helper (GPU).app`. A part never starts or ends with a space and never ends with `.`.
fn valid_bundle_path(value: &str) -> bool {
    value.len() <= 240
        && value.split('/').count() <= 12
        && value.split('/').all(|part| {
            !part.is_empty()
                && part.len() <= 120
                && part.trim() == part
                && !part.ends_with('.')
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._ ()-".contains(&byte))
        })
}

/// Pins ancestors while opening a private release artifact without following links.
pub(crate) fn open_release_file(directory: &Path, relative: &str) -> io::Result<File> {
    open_listed_file(directory, relative, PathGrammar::Release)
}

/// Pins ancestors while opening a private listed file at a path of `grammar` without following
/// links.
pub(crate) fn open_listed_file(
    directory: &Path,
    relative: &str,
    grammar: PathGrammar,
) -> io::Result<File> {
    if !grammar.admits(relative) {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let path = directory.join(relative);
    let _pins = permissions::open_tree(path.parent().ok_or(io::ErrorKind::InvalidInput)?)?;
    permissions::open_file(&path, false, false)
}
