//! POSIX lock folder and replacement; `rename` replaces the file atomically.
use super::{SettingsEditError, io_failure};
use std::{fs, io, os::unix::fs::DirBuilderExt, path::Path};

/// The data root's state folder, readable only by its owner.
pub(super) fn create_lock_directory(directory: &Path) -> io::Result<()> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(directory)
}

pub(super) fn replace(pending: &Path, target: &Path) -> Result<(), SettingsEditError> {
    fs::rename(pending, target).map_err(io_failure)
}
