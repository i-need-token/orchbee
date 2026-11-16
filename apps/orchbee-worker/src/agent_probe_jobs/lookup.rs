//! Platform adapter for the Agent probe lookup. Windows resolves each command with the fixed
//! system `where.exe` on the registry projection of `PATH`, the search path a shell starts with
//! before its profile runs: a directory stored in the registry `Path`, such as a version manager's
//! shim directory, is searched, and a directory a shell profile adds is not. macOS starts the
//! person's shell as an interactive login shell and resolves each command with `command -v` on the
//! `PATH` its startup files leave, as a new terminal there starts a login shell. Other platforms
//! have no lookup, so their Workers answer every probe `Unsupported`.

#[cfg(target_os = "macos")]
mod macos;
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

/// The lookup this platform runs.
#[cfg(target_os = "macos")]
pub(super) fn executor() -> Option<super::Execute> {
    Some(macos::run)
}

/// No lookup exists here, which the owner answers as `Unsupported`.
#[cfg(not(any(windows, target_os = "macos")))]
pub(super) fn executor() -> Option<super::Execute> {
    None
}
