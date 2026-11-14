//! The read side of the per-user release store: where the store root is, which version its pointer
//! names active, and whether a folder is one of its version folders. The Client's install, update
//! and uninstall commands and its Session start read the store through this owner, so the root rule
//! and the pointer's file name and bound exist once. Reading takes no lock, creates nothing and
//! starts no process; the Client's release store owns every write.
use crate::PrivateCache;
use std::{
    ffi::OsString,
    io,
    path::{Path, PathBuf},
};

/// The store folder that keeps version folders apart from the fixed names beside them.
pub const RELEASE_VERSIONS: &str = "versions";
/// The pointer that alone names the store's active version.
pub const RELEASE_POINTER: &str = "active.json";
/// The most pointer bytes any reader or writer accepts.
pub const RELEASE_POINTER_LIMIT: usize = 16 * 1024;

/// The version the store's pointer names active, and its immutable folder under the store root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveRelease {
    pub version: String,
    pub directory: PathBuf,
}

/// `ORCHBEE_INSTALL_DIR`, or `%LOCALAPPDATA%\Programs\Orchbee` for release and
/// `%LOCALAPPDATA%\Programs\<stem>` for a development or test build, so only release names the
/// release store. A relative path, or one longer than 2048 bytes or holding `;`, `%`, a quote or a
/// control character, is refused.
#[cfg(windows)]
pub fn release_store_root(env: impl Fn(&str) -> Option<OsString>) -> io::Result<PathBuf> {
    let path = match env("ORCHBEE_INSTALL_DIR") {
        Some(value) => PathBuf::from(value),
        None => PathBuf::from(env("LOCALAPPDATA").ok_or(io::ErrorKind::NotFound)?)
            .join("Programs")
            .join(if crate::is_release() {
                "Orchbee"
            } else {
                crate::environment_stem()
            }),
    };
    if !path.is_absolute()
        || path
            .to_str()
            .is_none_or(|s| s.len() > 2048 || s.contains([';', '%', '\0', '\r', '\n', '"']))
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    Ok(path)
}

/// macOS and Linux keep no release store yet.
#[cfg(not(windows))]
pub fn release_store_root(_env: impl Fn(&str) -> Option<OsString>) -> io::Result<PathBuf> {
    Err(io::ErrorKind::Unsupported.into())
}

/// The store's active version under the store's private admission: a missing root or pointer, or
/// bytes that are no pointer record, mean none. An unsafe root or pointer, or a pointer over the
/// bound, is an error. A replacement is a whole-file rename, so a reader sees the old pointer or the
/// new one; a read that meets the rename's momentary hold fails and may be retried.
pub fn active_release(root: &Path) -> io::Result<Option<ActiveRelease>> {
    let Some(bytes) = PrivateCache::read_existing(root, RELEASE_POINTER, RELEASE_POINTER_LIMIT)?
    else {
        return Ok(None);
    };
    Ok(
        orchbee_release_records::Pointer::parse(&bytes).map(|pointer| ActiveRelease {
            directory: root.join(RELEASE_VERSIONS).join(&pointer.active.product),
            version: pointer.active.product,
        }),
    )
}

/// Whether `directory` has the shape of a store version folder: its parent is named `versions`, in
/// any letter case as Windows resolves it, and its own name is a version name. Any store's version
/// folders qualify, whatever its root.
pub fn is_release_version_directory(directory: &Path) -> bool {
    fn name(path: &Path) -> Option<&str> {
        path.file_name()?.to_str()
    }
    name(directory).is_some_and(orchbee_release_records::valid_version)
        && directory
            .parent()
            .and_then(name)
            .is_some_and(|parent| parent.eq_ignore_ascii_case(RELEASE_VERSIONS))
}
