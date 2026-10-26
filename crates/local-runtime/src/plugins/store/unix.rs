//! Unix registry persistence is unsupported; no path is opened.
use super::Error;
use std::path::Path;
pub(super) fn require() -> Result<(), Error> {
    Err(Error::Unsupported)
}
pub(super) fn fixed_local_volume(_path: &Path) -> bool {
    false
}
