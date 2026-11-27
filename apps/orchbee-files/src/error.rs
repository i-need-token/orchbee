//! The protocol's failure for an operating system error. The code is what a Client localizes, and
//! the error number travels beside it, as VS Code shows it; nothing else about the error leaves
//! the machine, since its text may carry a path.

use std::io;

use orchbee_files_protocol::{ErrorCode, FilesError, Signed};

/// The failure `error` reports to a Client.
pub(crate) fn from_io(error: &io::Error) -> FilesError {
    let mut failure = FilesError::new(code(error));
    failure.os_error = error
        .raw_os_error()
        .and_then(|number| Signed::new(i64::from(number)).ok());
    failure
}

fn code(error: &io::Error) -> ErrorCode {
    if sharing_violation(error) {
        return ErrorCode::Busy;
    }
    match error.kind() {
        io::ErrorKind::NotFound => ErrorCode::NotFound,
        io::ErrorKind::PermissionDenied => ErrorCode::PermissionDenied,
        io::ErrorKind::AlreadyExists => ErrorCode::Exists,
        io::ErrorKind::IsADirectory => ErrorCode::IsDirectory,
        io::ErrorKind::NotADirectory => ErrorCode::NotDirectory,
        io::ErrorKind::InvalidFilename => ErrorCode::InvalidName,
        io::ErrorKind::ReadOnlyFilesystem => ErrorCode::ReadOnly,
        io::ErrorKind::StorageFull => ErrorCode::DiskFull,
        io::ErrorKind::ResourceBusy | io::ErrorKind::ExecutableFileBusy => ErrorCode::Busy,
        io::ErrorKind::TimedOut => ErrorCode::Timeout,
        _ => ErrorCode::Failed,
    }
}

/// Windows reports a file another process holds open without sharing, or has locked, by error
/// number alone.
#[cfg(windows)]
fn sharing_violation(error: &io::Error) -> bool {
    const ERROR_SHARING_VIOLATION: i32 = 32;
    const ERROR_LOCK_VIOLATION: i32 = 33;
    matches!(
        error.raw_os_error(),
        Some(ERROR_SHARING_VIOLATION | ERROR_LOCK_VIOLATION)
    )
}

#[cfg(not(windows))]
const fn sharing_violation(_error: &io::Error) -> bool {
    false
}
