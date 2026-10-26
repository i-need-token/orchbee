//! Preserve the installed Windows release verifier's native file observations.
use super::{Unsupported, VerifiedRelease};
use orchbee_local_runtime::Cohort;
use std::path::Path;

pub(super) fn observe(
    executable: &Path,
    admitted: Option<&Cohort>,
    version: &str,
) -> Result<Result<Option<VerifiedRelease>, ()>, Unsupported> {
    Ok(crate::cli::update::observe_installation(
        executable, admitted, version,
    ))
}
