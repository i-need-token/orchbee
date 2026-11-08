//! Host filesystem admission and executable naming for an otherwise portable cohort verifier.
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(unix)]
pub(super) use unix::{artifact_file_name, has_release_names, open_directory, open_file};
#[cfg(windows)]
pub(super) use windows::{artifact_file_name, has_release_names, open_directory, open_file};
