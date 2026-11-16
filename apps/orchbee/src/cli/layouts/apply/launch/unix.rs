//! The macOS and Linux executable search: an absolute program names itself, a program with a slash
//! follows the caller's directory, and a bare name takes the first executable regular file on the
//! member's search path, as a POSIX shell would run it.
use super::super::error;
use crate::cli::{Failure, FailureKind};
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

pub(super) fn resolve(
    program: &str,
    search: Option<&OsStr>,
    current: &Path,
) -> Result<String, Failure> {
    let failure = || error(FailureKind::CheckFailed, "program-unavailable");
    let path = Path::new(program);
    let candidates = if path.is_absolute() {
        vec![path.to_path_buf()]
    } else if program.contains('/') {
        vec![current.join(path)]
    } else {
        std::env::split_paths(search.ok_or_else(failure)?)
            .take(256)
            .filter(|directory| directory.is_absolute() && directory.as_os_str().len() <= 4096)
            .map(|directory| directory.join(program))
            .collect::<Vec<PathBuf>>()
    };
    for candidate in candidates {
        if executable(&candidate) {
            return candidate.to_str().map(str::to_owned).ok_or_else(failure);
        }
    }
    Err(failure())
}

#[cfg(unix)]
fn executable(path: &Path) -> bool {
    path.is_file() && nix::unistd::access(path, nix::unistd::AccessFlags::X_OK).is_ok()
}

/// Layout application is refused before any launch on hosts without a POSIX executable check.
#[cfg(not(unix))]
fn executable(_path: &Path) -> bool {
    false
}
