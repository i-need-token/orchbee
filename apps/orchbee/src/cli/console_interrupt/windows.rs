//! The existing console guard remains the sole handler and deferred-finalization owner.

use super::NativeSignals;
use crate::cli::{
    SourceFinalization,
    source::{ArmedSignals, Finish, SignalWatch},
};
use orchbee_local_ipc::LocalIo;
use std::io;

pub(super) fn acquire_document(
    path: Option<&str>,
    limit: usize,
    deadline: std::time::Instant,
    signals: &mut super::ArmedSignals,
) -> Result<Vec<u8>, super::SourceFailure> {
    crate::cli::source::acquire_document(path, limit, deadline, signals)
}

pub(super) fn arm() -> io::Result<Box<dyn NativeSignals>> {
    ArmedSignals::arm_interrupts().map(|signals| Box::new(signals) as Box<dyn NativeSignals>)
}

impl NativeSignals for ArmedSignals {
    fn requested(&self) -> bool {
        ArmedSignals::requested(self)
    }
    fn wake(&self) -> Option<LocalIo<'_>> {
        self.poll_fd()
    }
    fn take(&mut self) -> io::Result<Option<i32>> {
        SignalWatch::take(self)
    }
    fn finish(self: Box<Self>) -> io::Result<Option<SourceFinalization>> {
        ArmedSignals::finish(*self).map(|finish| match finish {
            Finish::Clear => None,
            Finish::Cancelled { finalization, .. } => Some(finalization),
        })
    }
    fn into_finalization(self: Box<Self>) -> SourceFinalization {
        ArmedSignals::into_finalization(*self)
    }
}

impl SignalWatch for super::ArmedSignals {
    fn poll_fd(&self) -> Option<LocalIo<'_>> {
        self.poll_fd()
    }
    fn take(&mut self) -> io::Result<Option<i32>> {
        self.take()
    }
}

impl super::SignalWatch for ArmedSignals {
    fn poll_fd(&self) -> Option<LocalIo<'_>> {
        SignalWatch::poll_fd(self)
    }
    fn take(&mut self) -> io::Result<Option<i32>> {
        SignalWatch::take(self)
    }
}
