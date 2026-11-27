//! How a failed filesystem call reaches a Client: the File Bar error code that classifies the
//! operating system's error, with the error's number. The system's message text never leaves the
//! process, since it can carry a path.

use std::io;

/// A File Bar error code the file engine reports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FsErrorCode {
    /// `not-found`.
    NotFound,
    /// `not-directory`.
    NotDirectory,
    /// `is-directory`.
    IsDirectory,
    /// `invalid-name`: the system refused the name or the path, such as for its length.
    InvalidName,
    /// `permission-denied`.
    PermissionDenied,
    /// `busy`: another process holds the file.
    Busy,
    /// `too-large`: the file exceeds the Client's limit.
    TooLarge,
    /// `timeout`: the helper passed its deadline.
    Timeout,
    /// `cancelled`.
    Cancelled,
    /// `invalid-request`: well formed but not valid now, such as an expired cursor.
    InvalidRequest,
    /// `unavailable` with reason `stuck-filesystem`: too many helpers are abandoned.
    StuckFilesystem,
    /// `unavailable` with reason `overloaded`: a per-process or per-connection bound is full.
    Overloaded,
    /// `failed`.
    Failed,
}

/// A failed file engine operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FsError {
    /// How the failure is classified.
    pub(crate) code: FsErrorCode,
    /// The operating system's error number, when a system call failed.
    pub(crate) os_error: Option<i32>,
}

impl FsError {
    /// A failure the engine decided, with no system error behind it.
    pub(crate) const fn new(code: FsErrorCode) -> Self {
        Self {
            code,
            os_error: None,
        }
    }

    /// The classification of a failed system call.
    pub(crate) fn io(error: &io::Error) -> Self {
        let code = match error.kind() {
            io::ErrorKind::NotFound => FsErrorCode::NotFound,
            io::ErrorKind::PermissionDenied => FsErrorCode::PermissionDenied,
            io::ErrorKind::NotADirectory => FsErrorCode::NotDirectory,
            io::ErrorKind::IsADirectory => FsErrorCode::IsDirectory,
            io::ErrorKind::InvalidFilename => FsErrorCode::InvalidName,
            io::ErrorKind::ResourceBusy => FsErrorCode::Busy,
            _ if held_by_another_process(error) => FsErrorCode::Busy,
            _ => FsErrorCode::Failed,
        };
        Self {
            code,
            os_error: error.raw_os_error(),
        }
    }
}

/// `ERROR_SHARING_VIOLATION` and `ERROR_LOCK_VIOLATION`, which the standard library leaves
/// uncategorized.
#[cfg(windows)]
fn held_by_another_process(error: &io::Error) -> bool {
    matches!(error.raw_os_error(), Some(32 | 33))
}

#[cfg(not(windows))]
fn held_by_another_process(_: &io::Error) -> bool {
    false
}
