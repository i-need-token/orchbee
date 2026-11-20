//! The Windows path limits of the child processes an update starts. CreateProcess refuses a program
//! whose absolute path has 260 or more UTF-16 units and a working directory of 259 or more, though the
//! update's own file operations accept longer paths. An install whose program is that long would
//! activate and then fail to start, so the update checks these limits before it changes anything.
//! Other platforms set no such limit.
use std::path::Path;

/// The longest program path CreateProcess starts, in UTF-16 units.
#[cfg(windows)]
const PROGRAM_UNITS: usize = 259;
/// The longest working directory CreateProcess accepts, in UTF-16 units.
#[cfg(windows)]
const DIRECTORY_UNITS: usize = 258;

/// Whether `path` may be the absolute path of a program that a child process starts.
#[cfg(windows)]
pub(super) fn program_fits(path: &Path) -> bool {
    units(path) <= PROGRAM_UNITS
}

/// Whether `path` may be the working directory of a child process.
#[cfg(windows)]
pub(super) fn directory_fits(path: &Path) -> bool {
    units(path) <= DIRECTORY_UNITS
}

#[cfg(windows)]
fn units(path: &Path) -> usize {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str().encode_wide().count()
}

#[cfg(not(windows))]
pub(super) fn program_fits(_path: &Path) -> bool {
    true
}

#[cfg(not(windows))]
pub(super) fn directory_fits(_path: &Path) -> bool {
    true
}
