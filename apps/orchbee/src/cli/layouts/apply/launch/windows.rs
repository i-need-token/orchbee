//! Preserve the admitted Windows executable search, file check and native spelling.
use super::super::error;
use crate::cli::{Failure, FailureKind};
use std::path::{Path, PathBuf};

pub(super) fn resolve(
    program: &str,
    search: Option<&std::ffi::OsStr>,
    current: &Path,
) -> Result<String, Failure> {
    let failure = || error(FailureKind::CheckFailed, "program-unavailable");
    let path = Path::new(program);
    let candidates = if path.is_absolute() {
        vec![path.to_path_buf()]
    } else if program.contains(['/', '\\']) {
        vec![current.join(path)]
    } else {
        std::env::split_paths(search.ok_or_else(failure)?)
            .take(256)
            .filter(|directory| directory.is_absolute() && directory.as_os_str().len() <= 4096)
            .map(|directory| {
                directory.join(if path.extension().is_none() {
                    format!("{program}.exe")
                } else {
                    program.to_owned()
                })
            })
            .collect::<Vec<PathBuf>>()
    };
    for candidate in candidates {
        if candidate.is_file() {
            // Consistent Windows executable spelling also avoids cmd.exe treating '/' in argv0 as switches.
            let value = candidate.to_str().ok_or_else(failure)?.replace('/', "\\");
            return Ok(value);
        }
    }
    Err(failure())
}
