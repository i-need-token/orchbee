//! The optional console-command capability refuses without installing a Unix signal policy. The
//! release and saved-machine commands on macOS and Linux borrow the source owner's interrupt pair
//! instead, latched, and a routed command reads its declared document through the source owner's
//! reader.

use super::NativeSignals;
use std::io;

/// No signal policy: the invocation keeps its inherited dispositions and never reports a signal.
pub(super) struct Inherited {
    /// The thread's signal mask when the command began, which the owed finalization restores as it
    /// was, since nothing here changes it.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    mask: nix::sys::signal::SigSet,
}

/// Keeps the inherited dispositions, recording the thread's signal mask on macOS and Linux.
pub(super) fn inherit() -> io::Result<Box<dyn NativeSignals>> {
    Ok(Box::new(Inherited {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        mask: nix::sys::signal::SigSet::thread_get_mask()?,
    }))
}

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
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        return crate::cli::SourceFinalization::new(self.mask);
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        crate::cli::SourceFinalization {}
    }
}

pub(super) fn arm() -> io::Result<Box<dyn NativeSignals>> {
    Err(io::ErrorKind::Unsupported.into())
}

/// The source owner's interrupt pair, `SIGINT` and `SIGTERM`, blocked on this thread and captured.
/// The first signal read latches: every later read answers it and settling owes the restore, as
/// the Windows console guard answers a request until it ends, so a workflow that checks between
/// its steps and the settle that ends it see the same interruption.
#[cfg(any(target_os = "linux", target_os = "macos"))]
struct Latched {
    signals: crate::cli::source::ArmedSignals,
    signal: Option<i32>,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(super) fn arm_latched() -> io::Result<Box<dyn NativeSignals>> {
    crate::cli::source::ArmedSignals::arm_interrupts()
        .map(|signals| {
            Box::new(Latched {
                signals,
                signal: None,
            }) as Box<dyn NativeSignals>
        })
        .map_err(|error| io::Error::other(error.to_string()))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl NativeSignals for Latched {
    fn wake(&self) -> Option<orchbee_local_ipc::LocalIo<'_>> {
        crate::cli::source::SignalWatch::poll_fd(&self.signals)
    }
    fn requested(&self) -> bool {
        self.signal.is_some()
    }
    fn take(&mut self) -> io::Result<Option<i32>> {
        if self.signal.is_none() {
            self.signal = crate::cli::source::SignalWatch::take(&mut self.signals)?;
        }
        Ok(self.signal)
    }
    fn finish(self: Box<Self>) -> io::Result<Option<crate::cli::SourceFinalization>> {
        use crate::cli::source::Finish;
        if self.signal.is_some() {
            return Ok(Some(self.signals.into_finalization()));
        }
        self.signals
            .finish()
            .map(|finish| match finish {
                Finish::Clear => None,
                Finish::Cancelled { finalization, .. } => Some(finalization),
            })
            .map_err(io::Error::from)
    }
    fn into_finalization(self: Box<Self>) -> crate::cli::SourceFinalization {
        self.signals.into_finalization()
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(super) fn acquire_document(
    path: Option<&str>,
    limit: usize,
    deadline: &mut std::time::Instant,
    signals: &mut super::ArmedSignals,
) -> Result<Vec<u8>, super::SourceFailure> {
    crate::cli::source::acquire_document(path, limit, deadline, signals)
}

/// The source reader's view of a latched owner: a read error keeps its system code.
#[cfg(any(target_os = "linux", target_os = "macos"))]
impl crate::cli::source::SignalWatch for super::ArmedSignals {
    fn poll_fd(&self) -> Option<std::os::fd::BorrowedFd<'_>> {
        self.poll_fd()
    }
    fn take(&mut self) -> Result<Option<i32>, nix::errno::Errno> {
        self.take().map_err(|error| {
            error
                .raw_os_error()
                .map_or(nix::errno::Errno::EIO, nix::errno::Errno::from_raw)
        })
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub(super) fn acquire_document(
    _path: Option<&str>,
    _limit: usize,
    _deadline: &mut std::time::Instant,
    _signals: &mut super::ArmedSignals,
) -> Result<Vec<u8>, super::SourceFailure> {
    Err(super::SourceFailure::Standard(
        crate::cli::Failure::new(crate::cli::FailureKind::ServiceUnavailable).details(
            crate::cli::stage_reason_details("platform", "unsupported-platform"),
        ),
    ))
}
