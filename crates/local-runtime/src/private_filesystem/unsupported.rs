//! The Windows pinned-file capability is not implemented on this host; Unix runtime admission has separate descriptor adapters.
use std::{fs::File, io, path::Path};
pub fn file_identity(_file: &File) -> io::Result<(u64, u64)> {
    Err(io::ErrorKind::Unsupported.into())
}
pub fn require_private(_file: &File) -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}
pub fn protect_new(_file: &mut File) -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}
pub fn open_directory(_path: &Path, _writable_acl: bool) -> io::Result<File> {
    Err(io::ErrorKind::Unsupported.into())
}
pub fn open_tree(_path: &Path) -> io::Result<Vec<File>> {
    Err(io::ErrorKind::Unsupported.into())
}
pub fn open_file(_path: &Path, _create: bool, _write: bool) -> io::Result<File> {
    Err(io::ErrorKind::Unsupported.into())
}
pub fn open_snapshot_file(_path: &Path) -> io::Result<File> {
    Err(io::ErrorKind::Unsupported.into())
}
