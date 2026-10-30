//! Layout execution is unavailable on Unix; executable lookup never inspects the host.
use crate::cli::{Failure, FailureKind, stage_reason_details};
use std::{ffi::OsStr, path::Path};

pub(super) fn resolve(
    _program: &str,
    _search: Option<&OsStr>,
    _current: &Path,
) -> Result<String, Failure> {
    Err(Failure::new(FailureKind::ServiceUnavailable)
        .details(stage_reason_details("platform", "unsupported-platform")))
}
