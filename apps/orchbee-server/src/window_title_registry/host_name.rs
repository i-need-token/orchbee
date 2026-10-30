//! Reads the computer name a window title may display. Builds without the Windows query report no
//! name, which renders like a failed query.

/// The local computer name, or `None` when the Windows query fails or this build is not Windows.
#[cfg(windows)]
pub(super) fn host_name() -> Option<String> {
    winsafe::GetComputerName().ok()
}

/// The local computer name; this build has no Windows query.
#[cfg(not(windows))]
pub(super) fn host_name() -> Option<String> {
    None
}
