//! Delegate to the admitted host's original Agent read signal producer and restoration token.
use super::NativeSignals;
use crate::cli::{
    SourceFinalization,
    source::{ArmedSignals, Finish, SignalWatch},
};
use nix::{
    errno::Errno,
    poll::{PollFd, PollFlags, PollTimeout, poll},
};
use orchbee_local_ipc::LocalIo;
use std::io;

pub(super) fn arm() -> io::Result<Box<dyn NativeSignals>> {
    ArmedSignals::arm_interrupts()
        .map(|signals| Box::new(signals) as Box<dyn NativeSignals>)
        .map_err(|error| io::Error::other(error.to_string()))
}

impl NativeSignals for ArmedSignals {
    fn wake(&self) -> Option<LocalIo<'_>> {
        SignalWatch::poll_fd(self)
    }

    /// The capture descriptor stays readable until `take` drains it, so a zero-timeout poll
    /// observes a captured signal and leaves it for the owner's `take` or `finish`.
    fn requested(&self) -> bool {
        SignalWatch::poll_fd(self).is_some_and(|fd| {
            let mut descriptors = [PollFd::new(fd, PollFlags::POLLIN)];
            matches!(poll(&mut descriptors, PollTimeout::ZERO), Ok(count) if count > 0)
        })
    }

    fn take(&mut self) -> io::Result<Option<i32>> {
        SignalWatch::take(self).map_err(io::Error::from)
    }

    fn finish(self: Box<Self>) -> io::Result<Option<SourceFinalization>> {
        ArmedSignals::finish(*self)
            .map(|finish| match finish {
                Finish::Clear => None,
                Finish::Cancelled { finalization, .. } => Some(finalization),
            })
            .map_err(io::Error::from)
    }
}

impl SignalWatch for super::ArmedSignals {
    fn poll_fd(&self) -> Option<LocalIo<'_>> {
        self.poll_fd()
    }

    fn take(&mut self) -> Result<Option<i32>, Errno> {
        self.0
            .take()
            .map_err(|error| error.raw_os_error().map_or(Errno::EIO, Errno::from_raw))
    }
}
