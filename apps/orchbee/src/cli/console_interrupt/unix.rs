//! The optional console-command capability refuses without installing a Unix signal policy.

use super::NativeSignals;
use std::io;

pub(super) fn arm() -> io::Result<Box<dyn NativeSignals>> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(super) fn acquire_document(
    _path: Option<&str>,
    _limit: usize,
    _deadline: std::time::Instant,
    _signals: &mut super::ArmedSignals,
) -> Result<Vec<u8>, super::SourceFailure> {
    Err(super::SourceFailure::Standard(
        crate::cli::Failure::new(crate::cli::FailureKind::ServiceUnavailable).details(
            crate::cli::stage_reason_details("platform", "unsupported-platform"),
        ),
    ))
}
