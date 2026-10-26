//! No private release-store filesystem capability is implemented on Unix.
use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
};

pub(super) fn create_stage(_root: &Path) -> io::Result<(PathBuf, Vec<File>)> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(super) fn create_directory(_path: &Path) -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(super) fn publish(_stage: &Path, _target: &Path) -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(super) fn remove_stage(_path: &Path) -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}
