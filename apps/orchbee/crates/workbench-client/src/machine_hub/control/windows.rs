//! A Windows hub process is watched through a handle, which pins the process once it is open.

use super::Open;
use std::time::Instant;
use winsafe::{self as w, co, guard::CloseHandleGuard};

pub(super) type Process = CloseHandleGuard<w::HPROCESS>;

/// Opens the hub process to wait for its exit.
pub(super) fn open(process_id: u32) -> Result<Process, Open> {
    match w::HPROCESS::OpenProcess(
        co::PROCESS::SYNCHRONIZE | co::PROCESS::QUERY_LIMITED_INFORMATION,
        false,
        process_id,
    ) {
        Ok(process) => Ok(process),
        Err(error) if error == co::ERROR::INVALID_PARAMETER => Err(Open::Gone),
        Err(error) if error == co::ERROR::ACCESS_DENIED => Err(Open::Denied),
        Err(_) => Err(Open::Unavailable),
    }
}

/// Opens the hub process only to learn when it goes.
pub(super) fn watch(process_id: u32) -> Option<Process> {
    w::HPROCESS::OpenProcess(co::PROCESS::SYNCHRONIZE, false, process_id).ok()
}

/// Whether `process` exited by `until`.
pub(super) fn exited(process: &Process, until: Instant) -> bool {
    loop {
        let remaining = until.saturating_duration_since(Instant::now());
        let milliseconds = u32::try_from(remaining.as_millis()).unwrap_or(u32::MAX);
        match process.WaitForSingleObject(Some(milliseconds)) {
            Ok(co::WAIT::OBJECT_0) => return true,
            Ok(_) if remaining.is_zero() => return false,
            Ok(_) => {}
            Err(_) => return false,
        }
    }
}
