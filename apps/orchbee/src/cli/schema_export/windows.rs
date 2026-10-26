//! Windows schema publication retains the original reparse refusal and exclusive temporary open.
use std::{
    fs::{File, Metadata, OpenOptions},
    io,
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};

pub(super) fn check_destination(metadata: &Metadata) -> io::Result<()> {
    if metadata.is_file() && metadata.file_attributes() & 0x400 == 0 {
        Ok(())
    } else {
        Err(io::ErrorKind::InvalidInput.into())
    }
}

pub(super) fn create_temporary(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .share_mode(0)
        .open(path)
}
