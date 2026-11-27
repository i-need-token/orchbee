//! A repository path as the filesystem resolves it, in the system's spelling and in Git's.
use crate::platform;
use std::{borrow::Cow, io, path::Path};

/// An absolute UTF-8 path with links, case and short names resolved by the filesystem, without a
/// verbatim prefix.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalPath(String);

impl CanonicalPath {
    /// The system's spelling, which the platform's path calls and native-form fields take.
    pub fn as_native(&self) -> &str {
        &self.0
    }

    /// The spelling Git prints and accepts, as in `safe.directory`: forward slashes on Windows.
    pub fn git_spelling(&self) -> Cow<'_, str> {
        platform::git_spelling(&self.0)
    }
}

/// Why a path has no [`CanonicalPath`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalPathError {
    /// Not a path this platform's Git owners accept: relative, or on Windows not a drive path.
    Inadmissible,
    /// The filesystem could not resolve it.
    Unresolved(io::ErrorKind),
    /// It resolves to a network location, which Windows Git owners refuse.
    NetworkLocation,
    /// Its resolved spelling is not UTF-8.
    NotUnicode,
}

/// Resolves `path` on the filesystem. That belongs to the Git owner, never to serialized ingress.
pub fn canonical_path(path: &Path) -> Result<CanonicalPath, CanonicalPathError> {
    if !platform::is_admissible_path(path) {
        return Err(CanonicalPathError::Inadmissible);
    }
    let resolved = std::fs::canonicalize(path)
        .map_err(|error| CanonicalPathError::Unresolved(error.kind()))?
        .into_os_string()
        .into_string()
        .map_err(|_| CanonicalPathError::NotUnicode)?;
    platform::native_spelling(resolved)
        .map(CanonicalPath)
        .ok_or(CanonicalPathError::NetworkLocation)
}
