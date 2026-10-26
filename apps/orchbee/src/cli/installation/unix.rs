//! Installed Windows release verification is unavailable on Unix; no path is probed.
use super::{Unsupported, VerifiedRelease};
use orchbee_local_runtime::Cohort;
use std::path::Path;

pub(super) fn observe(
    _executable: &Path,
    _admitted: Option<&Cohort>,
    _version: &str,
) -> Result<Result<Option<VerifiedRelease>, ()>, Unsupported> {
    Err(Unsupported)
}
