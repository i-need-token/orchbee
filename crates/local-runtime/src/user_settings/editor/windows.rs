//! Windows lock folder and replacement; an editor that holds the file open is waited out briefly.
use super::{RETRY_PAUSE, SettingsEditError, io_failure};
use std::{fs, io, path::Path, thread, time::Instant};

/// How long a replacement waits for another program to close the file.
const IN_USE_WAIT: std::time::Duration = std::time::Duration::from_secs(2);
const ERROR_ACCESS_DENIED: i32 = 5;
const ERROR_SHARING_VIOLATION: i32 = 32;
const ERROR_LOCK_VIOLATION: i32 = 33;

/// The data root's private state folder, created like every other one.
pub(super) fn create_lock_directory(directory: &Path) -> io::Result<()> {
    crate::data_root::create_state_directory(directory).map(drop)
}

/// Replace `target` with `pending`. The target was checked writable before the edit, so a denied
/// replacement means another program holds the file open without sharing deletion.
pub(super) fn replace(pending: &Path, target: &Path) -> Result<(), SettingsEditError> {
    let deadline = Instant::now() + IN_USE_WAIT;
    loop {
        match fs::rename(pending, target) {
            Ok(()) => return Ok(()),
            Err(error)
                if matches!(
                    error.raw_os_error(),
                    Some(ERROR_ACCESS_DENIED | ERROR_SHARING_VIOLATION | ERROR_LOCK_VIOLATION)
                ) =>
            {
                if Instant::now() >= deadline {
                    return Err(SettingsEditError::InUse);
                }
                thread::sleep(RETRY_PAUSE);
            }
            Err(error) => return Err(io_failure(error)),
        }
    }
}
