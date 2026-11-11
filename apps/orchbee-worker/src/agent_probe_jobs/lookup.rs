//! Platform adapter for the Agent probe lookup. Windows resolves each command with the fixed
//! system `where.exe` on the search path a newly started shell gets; other platforms have no
//! lookup, so their Workers answer every probe `Unsupported`.

#[cfg(windows)]
mod shell_path;
#[cfg(all(test, windows))]
mod tests;
#[cfg(windows)]
mod windows;

/// The lookup this platform runs.
#[cfg(windows)]
pub(super) fn executor() -> Option<super::Execute> {
    Some(windows::run)
}

/// No lookup exists here, which the owner answers as `Unsupported`.
#[cfg(not(windows))]
pub(super) fn executor() -> Option<super::Execute> {
    None
}
