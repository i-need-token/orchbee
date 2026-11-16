//! macOS admits the shared installed-release observer, which reads the release store files beside
//! this executable without filesystem effects; Linux, whose package is not verified, probes no path.
use super::Unsupported;

pub(super) fn admit() -> Result<(), Unsupported> {
    if cfg!(target_os = "macos") {
        Ok(())
    } else {
        Err(Unsupported)
    }
}
