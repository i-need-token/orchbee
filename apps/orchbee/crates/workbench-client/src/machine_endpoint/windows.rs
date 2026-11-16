//! Windows machine endpoints are the local pipe adapter's private same-user pipes; a pipe name
//! leaves nothing behind, so no folder is held and a locked bind needs no cleanup.
pub use orchbee_local_ipc::{LocalListener, LocalStream, Readiness, endpoint_exists};
use std::{io, path::Path};

/// A pipe name carries no suffix.
pub(super) const SUFFIX: &str = "";

pub(super) fn prefix(role: &str) -> String {
    format!(r"\\.\pipe\orchbee-{role}-")
}

/// Pipes need no folder.
pub struct FolderHold;

/// Nothing to create or hold.
///
/// # Errors
///
/// Never.
pub const fn hold_folder() -> io::Result<FolderHold> {
    Ok(FolderHold)
}

/// Binds `path` for a caller whose lock proves no other listener of it serves; a pipe a killed
/// predecessor left is gone with it, so this is an ordinary bind.
///
/// # Errors
///
/// The bind's own failure.
pub fn bind_locked(path: &Path) -> io::Result<LocalListener> {
    LocalListener::bind(path)
}
