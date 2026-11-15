//! The optional console-command capability refuses without installing a Unix signal policy.

use super::NativeSignals;
use std::io;

/// No signal policy: the invocation keeps its inherited dispositions and never reports a signal.
#[cfg(target_os = "macos")]
pub(super) struct Inherited;

#[cfg(target_os = "macos")]
impl NativeSignals for Inherited {
    fn wake(&self) -> Option<orchbee_local_ipc::LocalIo<'_>> {
        None
    }
    fn requested(&self) -> bool {
        false
    }
    fn take(&mut self) -> io::Result<Option<i32>> {
        Ok(None)
    }
    fn finish(self: Box<Self>) -> io::Result<Option<crate::cli::SourceFinalization>> {
        Ok(None)
    }
    fn into_finalization(self: Box<Self>) -> crate::cli::SourceFinalization {
        crate::cli::SourceFinalization {}
    }
}

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
