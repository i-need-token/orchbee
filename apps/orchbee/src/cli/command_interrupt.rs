//! Admitted CLI commands borrow the existing INT/TERM owner through one finish arbitration.
//! The port preserves the macOS and Linux signal-mask policy and the Windows console-handler
//! lifetime.
use super::{SourceFinalization, source::SourceFailure};
use orchbee_local_ipc::LocalIo;
use std::{io, time::Instant};

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[path = "command_interrupt/unix.rs"]
mod platform;
#[cfg(windows)]
#[path = "command_interrupt/windows.rs"]
mod platform;
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
#[path = "command_interrupt/unsupported.rs"]
mod platform;

trait NativeSignals {
    fn wake(&self) -> Option<LocalIo<'_>>;
    /// Whether a captured signal is waiting, without consuming it, so the next `take` or
    /// `finish` still observes it.
    fn requested(&self) -> bool;
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

    pub(super) fn requested(&self) -> bool {
        self.0.requested()
    }

    pub(super) fn finish(self) -> io::Result<Finish> {
        self.0.finish().map(|finalization| match finalization {
            None => Finish::Clear,
            Some(finalization) => Finish::Cancelled { finalization },
        })
    }
}

/// Reads one complete `--file` path or noninteractive stdin document of at most `limit` bytes
/// under this signal watch, as `terminal send` reads its declared source.
pub(super) fn acquire_document(
    path: Option<&str>,
    limit: usize,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Vec<u8>, SourceFailure> {
    super::source::acquire_document(path, limit, deadline, signals)
}

impl super::console_interrupt::SignalWatch for ArmedSignals {
    fn poll_fd(&self) -> Option<LocalIo<'_>> {
        self.poll_fd()
    }

    fn take(&mut self) -> io::Result<Option<i32>> {
        self.0.take()
    }
}
