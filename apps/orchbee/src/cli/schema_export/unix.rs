//! Unix schema and bundle publication: only a regular file is replaced, never a link or another
//! object, and the temporary sibling is created exclusively, so an existing name or a link placed
//! there refuses it. The temporary, and so the published file, is private to the user: a bundle
//! carries log records the data root keeps private.
use std::{
    fs::{File, Metadata, OpenOptions},
    io,
    os::unix::fs::OpenOptionsExt,
    path::Path,
};

/// Read and write for the user only.
const FILE_MODE: u32 = 0o600;

pub(super) fn check_destination(metadata: &Metadata) -> io::Result<()> {
    if metadata.is_file() {
        Ok(())
    } else {
        Err(io::ErrorKind::InvalidInput.into())
    }
}

pub(super) fn create_temporary(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(FILE_MODE)
        .open(path)
}
