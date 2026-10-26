//! Unavailable source acquisition returns before registering signals or observing input.
use crate::cli::{Failure, FailureKind, SourceFinalization, input_source::SourceFailure};
use std::{io, time::Instant};

pub(super) type ArmError = io::Error;
pub(super) type SignalError = io::Error;

// No source guard can be constructed here. Shared materialization may only pass the
// opaque borrow to the rejecting acquisition ports; there is no native wake to expose.
pub(crate) trait SignalWatch {}

pub(super) enum ArmedSignals {}

impl ArmedSignals {
    pub(super) fn arm() -> io::Result<Self> {
        Err(io::ErrorKind::Unsupported.into())
    }

    pub(super) fn into_finalization(self) -> SourceFinalization {
        match self {}
    }
}

pub(super) fn finish(signals: ArmedSignals) -> io::Result<Option<(i32, SourceFinalization)>> {
    match signals {}
}

impl SignalWatch for super::ArmedSignals {}

fn unavailable() -> SourceFailure {
    SourceFailure::Standard(Failure::new(FailureKind::ServiceUnavailable).details(
        crate::cli::stage_reason_details("platform", "unsupported-platform"),
    ))
}

pub(super) fn acquire_file(
    _path: &str,
    _paste: bool,
    _deadline: Instant,
    _signals: &mut dyn SignalWatch,
) -> Result<Vec<u8>, SourceFailure> {
    Err(unavailable())
}

pub(super) fn acquire_stdin(
    _paste: bool,
    _deadline: Instant,
    _signals: &mut dyn SignalWatch,
) -> Result<Vec<u8>, SourceFailure> {
    Err(unavailable())
}
