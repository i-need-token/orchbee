//! Windows machine endpoints are the local pipe adapter's private same-user pipes; a pipe name
//! leaves nothing behind, so no folder is held and a locked bind needs no cleanup.
pub use orchbee_local_ipc::{LocalListener, LocalStream, Readiness, endpoint_exists};
use std::{io, path::Path};

/// A pipe name carries no suffix.
pub(super) const SUFFIX: &str = "";

/// What a refused open reports: the pipe's access list or its High label turned this process away.
const ACCESS_DENIED: i32 = 5;

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

/// Binds `path` for this user's processes at every integrity level the pipe's label lets in: a
/// client proves its user by the token it opened the pipe with, so an endpoint a standard process
/// binds admits an elevated client, while one an elevated process binds is labeled High and
/// refuses every standard client at the open.
///
/// # Errors
///
/// The bind's own failure.
pub fn bind(path: &Path) -> io::Result<LocalListener> {
    LocalListener::bind_admitting_elevated(path)
}

/// Binds `path` for a caller whose lock proves no other listener of it serves; a pipe a killed
/// predecessor left is gone with it, so this is an ordinary [`bind`].
///
/// # Errors
///
/// The bind's own failure.
pub fn bind_locked(path: &Path) -> io::Result<LocalListener> {
    bind(path)
}

/// Whether `error`, from connecting to an endpoint, means one an elevated process bound refused
/// this standard process: access denied while this process runs below High. Another user's
/// endpoint of the same name refuses the same way, which this cannot tell apart.
pub fn refused_from_above(error: &io::Error) -> bool {
    error.raw_os_error() == Some(ACCESS_DENIED)
        && matches!(orchbee_local_ipc::runs_at_high_integrity(), Ok(false))
}
