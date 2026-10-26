//! App-owned one-shot cancellation, borrowing the existing transport readiness port.
//! The owner publishes cancellation before notifying; readers never drain or reset its wake.

use orchbee_local_ipc::{LocalIo, PollFd, PollFlags, PollTimeout, poll};
use std::{
    io,
    sync::atomic::{AtomicU8, Ordering},
};

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(unix)]
use unix::Signal;
#[cfg(windows)]
use windows::Signal;

/// A non-owning view of either our cancellation owner or the CLI's existing signal wake.
#[derive(Clone, Copy)]
pub(crate) struct Cancellation<'a> {
    wake: Option<LocalIo<'a>>,
}

impl<'a> Cancellation<'a> {
    /// Borrows readiness without taking responsibility for signal consumption.
    pub(crate) const fn new(wake: Option<LocalIo<'a>>) -> Self {
        Self { wake }
    }

    /// Returns the borrowed readiness source for a transport wait.
    pub(crate) const fn wake(self) -> Option<LocalIo<'a>> {
        self.wake
    }

    /// Observe readiness without consuming a CLI signal or our sticky notification.
    pub(crate) fn is_cancelled(self) -> io::Result<bool> {
        let Some(wake) = self.wake else {
            return Ok(false);
        };
        let mut sources = [PollFd::new(wake, PollFlags::POLLIN)];
        poll(&mut sources, PollTimeout::ZERO)?;
        let ready = sources[0].revents().ok_or(io::ErrorKind::InvalidData)?;
        if ready.contains(PollFlags::POLLIN) {
            Ok(true)
        } else if ready.intersects(PollFlags::POLLHUP | PollFlags::POLLERR | PollFlags::POLLNVAL) {
            Err(io::Error::other("cancellation wake unavailable"))
        } else {
            Ok(false)
        }
    }
}

/// Created before a worker is spawned, retained through cancellation and join.
/// Completion belongs to the worker's JoinHandle, never another cancellation byte value.
pub(crate) struct OwnedCancellation {
    state: AtomicU8,
    signal: Signal,
}

impl OwnedCancellation {
    /// Creates the platform wake before the cancellable task is spawned.
    pub(crate) fn new() -> io::Result<Self> {
        Ok(Self {
            state: AtomicU8::new(0),
            signal: Signal::new()?,
        })
    }

    /// Publishes one-shot cancellation and notifies the platform wake.
    pub(crate) fn cancel(&self) {
        if self.state.swap(1, Ordering::AcqRel) == 0 {
            self.signal.notify();
        }
    }

    /// Reads the producer-owned state without consuming its readiness notification.
    pub(crate) fn is_cancelled(&self) -> bool {
        self.state.load(Ordering::Acquire) == 1
    }

    /// Borrows a sticky signal view for existing Client operations.
    pub(crate) fn borrow(&self) -> Cancellation<'_> {
        Cancellation::new(Some(self.signal.wake(&self.state)))
    }
}

#[cfg(test)]
mod tests;
