//! Reads the machine name a window title may display: the computer name on Windows, and on macOS
//! and Linux the network host name up to its first dot, as a shell prompt shows it. A failed read
//! renders the name empty.

/// The local computer name, or `None` when the Windows query fails.
#[cfg(windows)]
pub(super) fn host_name() -> Option<String> {
    winsafe::GetComputerName().ok()
}

/// The local host name without its domain, such as `.local`, or `None` when it cannot be read.
#[cfg(unix)]
pub(super) fn host_name() -> Option<String> {
    let name = orchbee_local_runtime::local_host_name()?;
    name.split('.')
        .next()
        .filter(|short| !short.is_empty())
        .map(str::to_owned)
}

/// The local computer name; this build has no query for it.
#[cfg(not(any(windows, unix)))]
pub(super) fn host_name() -> Option<String> {
    None
}
