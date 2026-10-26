//! Installed Windows release verification is unavailable on Unix; no path is probed.
use super::Unsupported;

pub(super) fn admit() -> Result<(), Unsupported> {
    Err(Unsupported)
}
