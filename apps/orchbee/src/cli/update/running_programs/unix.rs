//! No release store runs here, so whether a process runs from a set of files is never told.
use std::{io, path::Path};

/// Always `Unsupported`, so nothing here is removed as unused.
pub(in crate::cli::update) fn no_process_runs_from(_folder: &Path) -> io::Result<bool> {
    Err(io::ErrorKind::Unsupported.into())
}
