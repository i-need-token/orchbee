//! Agent reads borrow the existing native interrupt owner through one finish arbitration.
//! The port preserves Linux signal-mask policy and Windows console-handler lifetime.
use super::SourceFinalization;
use orchbee_local_ipc::LocalIo;
use std::io;

#[cfg(target_os = "linux")]
#[path = "agent_interrupt/linux.rs"]
mod platform;
#[cfg(windows)]
#[path = "agent_interrupt/windows.rs"]
mod platform;
#[cfg(not(any(target_os = "linux", windows)))]
#[path = "agent_interrupt/unsupported.rs"]
mod platform;

trait NativeSignals {
    fn wake(&self) -> Option<LocalIo<'_>>;
    fn take(&mut self) -> io::Result<Option<i32>>;
    fn finish(self: Box<Self>) -> io::Result<Option<SourceFinalization>>;
}

pub(super) struct ArmedSignals(Box<dyn NativeSignals>);

pub(super) enum Finish {
    Clear,
    Cancelled { finalization: SourceFinalization },
}

impl ArmedSignals {
    pub(super) fn arm_interrupts() -> io::Result<Self> {
        platform::arm().map(Self)
    }

    pub(super) fn poll_fd(&self) -> Option<LocalIo<'_>> {
        self.0.wake()
    }

    pub(super) fn finish(self) -> io::Result<Finish> {
        self.0.finish().map(|finalization| match finalization {
            None => Finish::Clear,
            Some(finalization) => Finish::Cancelled { finalization },
        })
    }
}

impl super::console_interrupt::SignalWatch for ArmedSignals {
    fn poll_fd(&self) -> Option<LocalIo<'_>> {
        self.poll_fd()
    }

    fn take(&mut self) -> io::Result<Option<i32>> {
        self.0.take()
    }
}
