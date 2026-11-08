//! The diagnostic log waits for the POSIX data root (ORC-328); nothing is written or read.
use super::Stamp;
use std::{io, path::Path};

pub(super) fn append(_record: &[u8], _capped: &[u8], _stamp: Stamp) -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(super) fn read_daily(_path: &Path) -> io::Result<Option<Vec<u8>>> {
    Err(io::ErrorKind::Unsupported.into())
}
