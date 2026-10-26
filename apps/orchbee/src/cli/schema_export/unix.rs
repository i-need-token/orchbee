//! Unix has no schema publication capability; these native operations create no file.
use std::{
    fs::{File, Metadata},
    io,
    path::Path,
};

pub(super) fn check_destination(_metadata: &Metadata) -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(super) fn create_temporary(_path: &Path) -> io::Result<File> {
    Err(io::ErrorKind::Unsupported.into())
}
