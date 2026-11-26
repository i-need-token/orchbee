//! Platform adapter for the Agent probe lookup. Windows resolves each command with the fixed
//! system `where.exe` on the registry projection of `PATH`, the search path a shell starts with
//! before its profile runs: a directory stored in the registry `Path`, such as a version manager's
//! shim directory, is searched, and a directory a shell profile adds is not. macOS and Linux start
//! the person's shell as an interactive login shell and resolve each command with `command -v` on
//! the `PATH` its startup files leave, as a new terminal there starts a login shell.

#[cfg(windows)]
mod shell_path;
#[cfg(all(test, windows))]
mod tests;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

/// The lookup this platform runs.
#[cfg(windows)]
pub(super) fn executor() -> super::Execute {
    windows::run
}

/// The lookup this platform runs.
#[cfg(unix)]
pub(super) fn executor() -> super::Execute {
    unix::run
}
