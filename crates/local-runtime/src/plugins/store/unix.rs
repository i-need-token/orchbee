//! Unix registry persistence is unsupported; no path is opened.
use super::Error;
use std::{fs::File, io, path::Path};
pub(super) fn require() -> Result<(), Error> {
    Err(Error::Unsupported)
}
pub(super) fn fixed_local_volume(_path: &Path) -> bool {
    false
}
pub(super) fn pin_private(_path: &Path) -> io::Result<Vec<File>> {
    Err(io::ErrorKind::Unsupported.into())
}
