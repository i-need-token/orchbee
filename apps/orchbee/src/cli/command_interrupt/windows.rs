//! Borrow the original Windows console interrupt guard and its deferred finalization token.
use super::NativeSignals;
use crate::cli::{
    SourceFinalization,
    source::{ArmedSignals, Finish, SignalWatch},
};
use orchbee_local_ipc::LocalIo;
use std::io;

pub(super) fn arm() -> io::Result<Box<dyn NativeSignals>> {
    ArmedSignals::arm_interrupts().map(|signals| Box::new(signals) as Box<dyn NativeSignals>)
}

impl NativeSignals for ArmedSignals {
    fn wake(&self) -> Option<LocalIo<'_>> {
        SignalWatch::poll_fd(self)
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
}
