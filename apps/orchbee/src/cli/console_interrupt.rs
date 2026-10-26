//! Scoped console interrupts for commands admitted to the Windows execution capability.
//! Shared command workflows own arbitration; only the native port registers a handler.

pub(super) use super::input_source::SourceFailure;
use super::{Failure, FailureKind, SourceFinalization};
use orchbee_local_ipc::LocalIo;
use std::{io, time::Instant};

#[cfg(windows)]
#[path = "console_interrupt/windows.rs"]
mod platform;
#[cfg(not(windows))]
#[path = "console_interrupt/unix.rs"]
mod platform;

trait NativeSignals {
    fn wake(&self) -> Option<LocalIo<'_>>;
    fn take(&mut self) -> io::Result<Option<i32>>;
    fn finish(self: Box<Self>) -> io::Result<Option<SourceFinalization>>;
    fn into_finalization(self: Box<Self>) -> SourceFinalization;
}

pub(super) struct ArmedSignals(Box<dyn NativeSignals>);

/// Borrow cancellation from its real owner; observing a workflow never re-registers signals.
pub(super) trait SignalWatch {
    fn poll_fd(&self) -> Option<LocalIo<'_>>;
    fn take(&mut self) -> io::Result<Option<i32>>;
    fn check_before(&mut self, deadline: Instant) -> Result<(), Failure> {
        if self
            .take()
            .map_err(|_| Failure::new(FailureKind::InternalFailure))?
            .is_some()
        {
            Err(Failure::new(FailureKind::Cancelled))
        } else if Instant::now() >= deadline {
            Err(Failure::new(FailureKind::Timeout))
        } else {
            Ok(())
        }
    }
}

impl SignalWatch for ArmedSignals {
    fn poll_fd(&self) -> Option<LocalIo<'_>> {
        self.poll_fd()
    }
    fn take(&mut self) -> io::Result<Option<i32>> {
        self.take()
    }
}

pub(super) enum Finish {
    Clear,
    Cancelled { finalization: SourceFinalization },
}

pub(super) fn acquire_document(
    path: Option<&str>,
    limit: usize,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Vec<u8>, SourceFailure> {
    platform::acquire_document(path, limit, deadline, signals)
}

impl ArmedSignals {
    pub(super) fn arm_interrupts() -> io::Result<Self> {
        platform::arm().map(Self)
    }

    pub(super) fn poll_fd(&self) -> Option<LocalIo<'_>> {
        self.0.wake()
    }

    pub(super) fn take(&mut self) -> io::Result<Option<i32>> {
        self.0.take()
    }

    pub(super) fn finish(self) -> io::Result<Finish> {
        self.0.finish().map(|finalization| match finalization {
            None => Finish::Clear,
            Some(finalization) => Finish::Cancelled { finalization },
        })
    }

    pub(super) fn into_finalization(self) -> SourceFinalization {
        self.0.into_finalization()
    }
}
