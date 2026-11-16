//! `terminal send --file/--stdin`: materialize one declared local source
//! into the bounded record its declared form selects — literal bytes, or
//! the Host-bracketed paste content under `--paste`. Everything
//! here runs before session admission, snapshot, request preparation, and
//! submission — a source failure therefore has no request identity and is
//! reported through `orchbee.cli.failure.source` (timeout/cancelled) or the
//! existing `invalid-usage` path (validation), never as transport loss.
//!
//! Signal capture is private on macOS and Linux: [`ArmedSignals`] captures
//! the eligible subset of a command stage's own watch set — everything not
//! already blocked or ignored at arm time — for that stage's guarded
//! interval only. The source stage watches the four-signal [`GUARDED`] set
//! for its materialization interval; the Agent read owner borrows the same
//! mechanism for its narrower [`INTERRUPTS`] pair only — SIGHUP/SIGQUIT are
//! never masked or captured there. Arming blocks the eligible set on the
//! owner's thread and captures the exact old mask; the native capture (a
//! Linux signalfd, a macOS kqueue) reports pending members without
//! delivering them. Dispositions are never touched, inherited
//! blocked/ignored state is preserved (an empty eligible set is a real armed
//! policy, not a fallback), and the restore is owed to the common emit
//! finalizer as a noncloneable token whenever a captured signal selects
//! cancellation.
//!
//! A child process inherits the blocked set: the standard library's spawn
//! does not clear the signal mask. A child started while an owner is armed
//! must therefore clear its own mask, as the macOS retained-child adapter
//! does, or it ignores the interrupts the owner blocked.

use std::{
    io::{self, IsTerminal, Read},
    os::{
        fd::{AsFd, BorrowedFd},
        unix::fs::OpenOptionsExt,
    },
    time::Instant,
};

use nix::{
    errno::Errno,
    fcntl::OFlag,
    poll::{PollFd, PollFlags, PollTimeout, poll},
    sys::{
        signal::{SigSet, SigmaskHow, Signal},
        stat::{SFlag, fstat},
    },
};

use super::limit_for;
#[cfg(test)]
use super::materialize;
use crate::cli::input_source::usage;
use crate::cli::input_source::{InputSource, SourceFailure};
use crate::cli::{Failure, FailureKind, SourceFinalization};
use crate::invocation::SendSourceCause;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as native;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as native;

pub(crate) use native::ArmError;
pub(super) type SignalError = Errno;

/// The captured-signal channel polled beside the source descriptor. `take`
/// distinguishes all three outcomes — a captured signal, no pending signal,
/// and a real capture error — so an error is never silently read as "none".
pub(crate) trait SignalWatch {
    /// The descriptor to include in the wait set, or none when no signal is
    /// captured in this process.
    fn poll_fd(&self) -> Option<BorrowedFd<'_>>;
    /// Drain captured signals; `Ok(Some(signo))` means source acquisition
    /// ends in a typed cancellation for that signal, `Ok(None)` means no
    /// signal is pending, and `Err` is an internal failure.
    fn take(&mut self) -> Result<Option<i32>, Errno>;
}

/// The bounded termination set the source stage is willing to capture —
/// nothing outside it is ever eligible.
const GUARDED: [Signal; 4] = [
    Signal::SIGHUP,
    Signal::SIGINT,
    Signal::SIGQUIT,
    Signal::SIGTERM,
];

/// The Agent read owner's own interruption set over the same private arming
/// mechanism: only the process's cancel pair is eligible. SIGHUP and SIGQUIT
/// keep their inherited disposition entirely — the Agent owner never masks
/// or captures them.
const INTERRUPTS: [Signal; 2] = [Signal::SIGINT, Signal::SIGTERM];

/// How the capture interval ended. `Clear` means the capture is closed
/// and the exact inherited mask already restored — runtime admission may
/// proceed. `Cancelled` carries the first-read signo plus the noncloneable
/// token whose `release` performs the same exact restore only after the
/// common emit path finishes its writes.
pub(crate) enum Finish {
    Clear,
    Cancelled {
        signo: i32,
        finalization: SourceFinalization,
    },
}

/// The private source signal owner: the native capture over the eligible
/// set plus the inherited thread mask captured at arm. The capture interval
/// begins at the mask swap, before any source I/O; a setup fault owes
/// nothing more, because the native arm restores what it may before
/// reporting.
pub(crate) struct ArmedSignals {
    capture: native::Capture,
    old_mask: SigSet,
    /// In-process tests arm one at a time on macOS; see [`serial`].
    #[cfg(all(test, target_os = "macos"))]
    _serial: serial::Serial,
}

impl ArmedSignals {
    /// Capture [`GUARDED`] minus every signal the process already blocks or
    /// ignores, blocking that eligible set and recording the old mask.
    pub(crate) fn arm() -> Result<Self, ArmError> {
        Self::arm_set(&GUARDED)
    }

    /// The Agent read owner borrows the identical private arming mechanism
    /// for its own narrower interruption set — [`INTERRUPTS`] minus every
    /// signal the process already blocks or ignores. The eligibility,
    /// mask, and restore contracts are exactly `arm`'s; only membership
    /// differs, so SIGHUP/SIGQUIT are never captured or masked.
    pub(crate) fn arm_interrupts() -> Result<Self, ArmError> {
        Self::arm_set(&INTERRUPTS)
    }

    fn arm_set(watched: &[Signal]) -> Result<Self, ArmError> {
        #[cfg(all(test, target_os = "macos"))]
        let serial = serial::hold();
        native::Capture::arm(watched).map(|(capture, old_mask)| Self {
            capture,
            old_mask,
            #[cfg(all(test, target_os = "macos"))]
            _serial: serial,
        })
    }

    /// Linearization point: the last nonblocking signal read decides
    /// ownership. A captured signal wins cancellation and keeps the mask
    /// held for the output window; no pending signal means the source no
    /// longer owns cancellation — the capture closes and the exact old mask
    /// is restored before any runtime admission. A signal arriving after
    /// that read stays pending for its original disposition. A read error
    /// still attempts the owed restore before reporting; a restore failure
    /// is terminal either way.
    pub(crate) fn finish(mut self) -> Result<Finish, Errno> {
        let read = self.capture.take();
        self.finish_with(read, || {})
    }

    /// The shared arbitration body behind `finish`: `read` is the capture
    /// result to linearize on (injectable so error routing is provable), and
    /// `barrier` runs between that read and the owed restore — the documented
    /// late-signal window a test can place a signal inside.
    fn finish_with(
        mut self,
        read: Result<Option<i32>, Errno>,
        barrier: impl FnOnce(),
    ) -> Result<Finish, Errno> {
        let captured = match read {
            Err(errno) => {
                self.restore()?;
                return Err(errno);
            }
            Ok(captured) => captured,
        };
        barrier();
        match captured {
            Some(signo) => Ok(Finish::Cancelled {
                signo,
                finalization: self.into_finalization(),
            }),
            None => {
                self.restore()?;
                Ok(Finish::Clear)
            }
        }
    }

    /// Test-only `finish` whose barrier runs in the late-signal window.
    #[cfg(test)]
    fn finish_at(mut self, barrier: impl FnOnce()) -> Result<Finish, Errno> {
        let read = self.capture.take();
        self.finish_with(read, barrier)
    }

    /// Restore the captured thread mask exactly.
    fn restore(&mut self) -> Result<(), Errno> {
        self.old_mask
            .thread_swap_mask(SigmaskHow::SIG_SETMASK)
            .map(drop)
    }

    /// Hand the owed restore to the common emit finalizer; the mask stays
    /// blocked through the output window and the capture closes here.
    pub(crate) fn into_finalization(self) -> SourceFinalization {
        SourceFinalization::new(self.old_mask)
    }
}

impl SignalWatch for ArmedSignals {
    fn poll_fd(&self) -> Option<BorrowedFd<'_>> {
        Some(self.capture.poll_fd())
    }

    fn take(&mut self) -> Result<Option<i32>, Errno> {
        self.capture.take()
    }
}

/// Acquire one complete document of at most `limit` bytes from a `--file`
/// path or noninteractive stdin through the same cancellable bounded reads
/// as a declared `terminal send` source.
pub(in crate::cli) fn acquire_document(
    path: Option<&str>,
    limit: usize,
    deadline: Instant,
    signals: &mut dyn SignalWatch,
) -> Result<Vec<u8>, SourceFailure> {
    match path {
        Some(path) => acquire_file_limit(path, limit, deadline, signals),
        None => acquire_stdin_limit(limit, deadline, signals),
    }
}

pub(super) fn acquire_file(
    path: &str,
    paste: bool,
    deadline: Instant,
    signals: &mut dyn SignalWatch,
) -> Result<Vec<u8>, SourceFailure> {
    acquire_file_limit(path, limit_for(paste), deadline, signals)
}

/// Acquire `--file`: open without blocking on non-regular paths, `fstat` the
/// already-open descriptor so a swapped path still reports its true type,
/// and accept only a regular file.
fn acquire_file_limit(
    path: &str,
    limit: usize,
    deadline: Instant,
    signals: &mut dyn SignalWatch,
) -> Result<Vec<u8>, SourceFailure> {
    let flags = (OFlag::O_NONBLOCK | OFlag::O_CLOEXEC).bits();
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(flags)
        .open(path)
        .map_err(|_| usage("--file", Some(path.to_owned()), SendSourceCause::Unreadable))?;
    // `metadata` is `fstat` on the open descriptor: a path swapped after the
    // open still reports the real type.
    let regular = file
        .metadata()
        .map(|metadata| metadata.file_type().is_file())
        .map_err(|_| usage("--file", Some(path.to_owned()), SendSourceCause::Unreadable))?;
    if !regular {
        return Err(usage(
            "--file",
            Some(path.to_owned()),
            SendSourceCause::NotRegularFile,
        ));
    }
    let mut bytes = read_bounded(
        &mut &file,
        ReadPlan {
            fd: file.as_fd(),
            pollable: true,
            deadline,
            limit,
            source: InputSource::File,
            operand: Some(path),
        },
        signals,
    )?;
    if bytes.is_empty() {
        return Err(usage(
            "--file",
            Some(path.to_owned()),
            SendSourceCause::Empty,
        ));
    }
    bytes.shrink_to_fit();
    Ok(bytes)
}

/// The unbuffered borrowed-descriptor reader for the inherited stdin: a
/// `read(2)` on the shared description consumes exactly the bytes returned —
/// no read-ahead buffer consumes beyond the acquisition cap.
struct BorrowedReader<'a>(BorrowedFd<'a>);

impl Read for BorrowedReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        nix::unistd::read(self.0, buffer).map_err(io::Error::from)
    }
}

pub(super) fn acquire_stdin(
    paste: bool,
    deadline: Instant,
    signals: &mut dyn SignalWatch,
) -> Result<Vec<u8>, SourceFailure> {
    acquire_stdin_limit(limit_for(paste), deadline, signals)
}

/// Acquire `--stdin`: refuse a terminal, then dispatch on the inherited
/// descriptor's exact `S_IFMT` type — a regular file keeps its shared
/// offset, a FIFO is read through the native port without mutating the
/// caller's descriptor flags, and anything else is rejected outright.
fn acquire_stdin_limit(
    limit: usize,
    deadline: Instant,
    signals: &mut dyn SignalWatch,
) -> Result<Vec<u8>, SourceFailure> {
    let stdin = io::stdin();
    if stdin.is_terminal() {
        return Err(usage("--stdin", None, SendSourceCause::InteractiveTerminal));
    }
    // Exact type classification: the `S_IFMT` field alone decides, never a
    // superset bitmask match.
    let file_type = fstat(stdin.as_fd())
        .map(|stat| SFlag::from_bits_truncate(stat.st_mode & SFlag::S_IFMT.bits()))
        .map_err(|_| usage("--stdin", None, SendSourceCause::Unreadable))?;
    let bytes = if file_type == SFlag::S_IFREG {
        // Borrowed descriptor, inherited offset, unbuffered reads, no flag
        // mutation: a regular file answers reads promptly, and a
        // kernel-stalled read is the documented non-preemptible residual,
        // not a poll case.
        let mut reader = BorrowedReader(stdin.as_fd());
        read_bounded(
            &mut reader,
            stdin_plan(stdin.as_fd(), false, deadline, limit),
            signals,
        )?
    } else if file_type == SFlag::S_IFIFO {
        #[cfg(target_os = "linux")]
        {
            let private = linux::reopen_fifo_stdin()?;
            read_bounded(
                &mut &private,
                stdin_plan(private.as_fd(), true, deadline, limit),
                signals,
            )?
        }
        // macOS has no private reopen (`/dev/fd/0` shares the caller's file
        // description), so the inherited descriptor is read in place without
        // touching its flags, each read gated on its poll. A read can stall
        // only when another reader of the same pipe drains it between that
        // poll and the read — the documented non-preemptible residual.
        #[cfg(target_os = "macos")]
        {
            let mut reader = BorrowedReader(stdin.as_fd());
            read_bounded(
                &mut reader,
                stdin_plan(stdin.as_fd(), true, deadline, limit),
                signals,
            )?
        }
    } else {
        return Err(usage("--stdin", None, SendSourceCause::UnsupportedStream));
    };
    if bytes.is_empty() {
        return Err(usage("--stdin", None, SendSourceCause::Empty));
    }
    Ok(bytes)
}

fn stdin_plan(fd: BorrowedFd<'_>, pollable: bool, deadline: Instant, limit: usize) -> ReadPlan<'_> {
    ReadPlan {
        fd,
        pollable,
        deadline,
        limit,
        source: InputSource::Stdin,
        operand: None,
    }
}

/// What one bounded acquisition reads from and how it is classified: the
/// borrowed descriptor to poll, whether it is pollable at all, the shared
/// deadline, the declared form's acquisition bound, and the declared source
/// identity for diagnostics. `operand` is the declared source value (the
/// `--file` path) echoed verbatim in usage diagnostics — never added to
/// machine details.
struct ReadPlan<'a> {
    fd: BorrowedFd<'a>,
    pollable: bool,
    deadline: Instant,
    limit: usize,
    source: InputSource,
    operand: Option<&'a str>,
}

/// The shared bounded read loop: check cancellation and the deadline before
/// every wait and every read, poll the source (plus the injected signal
/// channel) when the descriptor is pollable, and drain `POLLHUP` data
/// before counting EOF. The buffer never exceeds `limit + 1`, so exactly
/// `limit` bytes succeeds and `limit + 1` is rejected without draining
/// further. A non-pollable regular file answers each read directly — a
/// kernel-stalled read there is the documented non-preemptible residual.
fn read_bounded(
    reader: &mut impl Read,
    plan: ReadPlan<'_>,
    signals: &mut dyn SignalWatch,
) -> Result<Vec<u8>, SourceFailure> {
    // Production binds the real monotonic clock; tests drive the identical
    // loop through `read_bounded_at` with a scripted instant sequence.
    read_bounded_at(reader, plan, signals, Instant::now)
}

/// `read_bounded` with the clock injected — a private test seam, not a
/// runtime abstraction: every deadline probe resolves through `now`, which
/// production always binds to `Instant::now`.
fn read_bounded_at(
    reader: &mut impl Read,
    plan: ReadPlan<'_>,
    signals: &mut dyn SignalWatch,
    mut now: impl FnMut() -> Instant,
) -> Result<Vec<u8>, SourceFailure> {
    let ReadPlan {
        fd,
        pollable,
        deadline,
        limit,
        source,
        operand,
    } = plan;
    let mut bytes = Vec::with_capacity(limit + 1);
    loop {
        match signals.take() {
            Err(_) => {
                return Err(SourceFailure::Standard(Failure::new(
                    FailureKind::InternalFailure,
                )));
            }
            Ok(Some(signal)) => return Err(SourceFailure::Cancelled { source, signal }),
            Ok(None) => {}
        }
        let remaining = deadline.saturating_duration_since(now());
        if remaining.is_zero() {
            return Err(SourceFailure::Timeout { source });
        }
        if pollable {
            // The borrowed signal descriptor is scoped to this block so the
            // mutable `take` below never overlaps its immutable borrow.
            let (signal_ready, ready) = {
                let mut descriptors = Vec::with_capacity(2);
                descriptors.push(PollFd::new(
                    fd,
                    PollFlags::POLLIN | PollFlags::POLLHUP | PollFlags::POLLERR,
                ));
                let signal_index = signals.poll_fd().map(|signal_fd| {
                    descriptors.push(PollFd::new(signal_fd, PollFlags::POLLIN));
                    descriptors.len() - 1
                });
                // Round the sub-millisecond remainder up so the wait never
                // ends before the real deadline; the loop re-checks
                // `Instant` anyway.
                let timeout_ms = u64::try_from(remaining.as_millis())
                    .unwrap_or(u64::MAX)
                    .saturating_add(u64::from(
                        !remaining.subsec_nanos().is_multiple_of(1_000_000),
                    ));
                let timeout = PollTimeout::try_from(timeout_ms).unwrap_or(PollTimeout::MAX);
                match poll(&mut descriptors, timeout) {
                    Err(Errno::EINTR) => continue,
                    Err(_) => {
                        return Err(SourceFailure::Standard(Failure::new(
                            FailureKind::InternalFailure,
                        )));
                    }
                    Ok(0) => continue,
                    Ok(_) => {}
                }
                (
                    signal_index
                        .and_then(|index| descriptors[index].revents())
                        .is_some_and(|flags| !flags.is_empty()),
                    descriptors[0].revents().unwrap_or_else(PollFlags::empty),
                )
            };
            if signal_ready {
                match signals.take() {
                    Err(_) => {
                        return Err(SourceFailure::Standard(Failure::new(
                            FailureKind::InternalFailure,
                        )));
                    }
                    Ok(Some(signal)) => {
                        return Err(SourceFailure::Cancelled { source, signal });
                    }
                    Ok(None) => {}
                }
            }
            if ready.contains(PollFlags::POLLNVAL) {
                return Err(SourceFailure::Standard(Failure::new(
                    FailureKind::InternalFailure,
                )));
            }
            if ready.is_empty() {
                continue;
            }
        }
        // The wait above may have ended at the deadline: arbitration
        // precedes every read, so ready data observed after the budget
        // closes is timeout, not input.
        if deadline.saturating_duration_since(now()).is_zero() {
            return Err(SourceFailure::Timeout { source });
        }
        let spare = (limit + 1).saturating_sub(bytes.len());
        if spare == 0 {
            return Err(usage(
                source.option(),
                operand.map(str::to_owned),
                SendSourceCause::Oversize { limit },
            ));
        }
        let mut chunk = vec![0_u8; spare.min(8192)];
        match reader.read(&mut chunk) {
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock
                ) =>
            {
                continue;
            }
            Err(_) => {
                return Err(usage(
                    source.option(),
                    operand.map(str::to_owned),
                    SendSourceCause::Unreadable,
                ));
            }
            Ok(0) => break,
            Ok(count) => bytes.extend_from_slice(&chunk[..count]),
        }
    }
    // Final drain: a captured signal at the completion boundary still owns
    // cancellation before the materialized text may be handed off.
    match signals.take() {
        Err(_) => {
            return Err(SourceFailure::Standard(Failure::new(
                FailureKind::InternalFailure,
            )));
        }
        Ok(Some(signal)) => return Err(SourceFailure::Cancelled { source, signal }),
        Ok(None) => {}
    }
    // The deadline also arbitrates the success boundary itself: an EOF
    // observed after the budget closed is timeout, never a clean record.
    if deadline.saturating_duration_since(now()).is_zero() {
        return Err(SourceFailure::Timeout { source });
    }
    Ok(bytes)
}

/// Preserve the native finish result while the facade constructs the shared stage outcome.
pub(super) fn finish(
    signals: ArmedSignals,
) -> Result<Option<(i32, SourceFinalization)>, SignalError> {
    signals.finish().map(|finish| match finish {
        Finish::Clear => None,
        Finish::Cancelled {
            signo,
            finalization,
        } => Some((signo, finalization)),
    })
}

impl SignalWatch for super::ArmedSignals {
    fn poll_fd(&self) -> Option<BorrowedFd<'_>> {
        SignalWatch::poll_fd(&self.0)
    }
    fn take(&mut self) -> Result<Option<i32>, Errno> {
        SignalWatch::take(&mut self.0)
    }
}

impl super::ArmedSignals {
    pub(crate) fn arm_interrupts() -> Result<Self, ArmError> {
        ArmedSignals::arm_interrupts().map(Self)
    }
}

/// macOS's `sigwait` takes a pending signal from any thread of the process, so in-process tests
/// that arm the owner at once could dequeue each other's probes and pending signals. A test that
/// holds a pending signal or raises one holds this lock for its whole body, and on macOS every arm in
/// a test build holds it until the capture ends; a thread that already holds it passes through.
#[cfg(test)]
pub(crate) mod serial {
    use std::{
        cell::Cell,
        sync::{Mutex, MutexGuard, PoisonError},
    };

    static OWNER: Mutex<()> = Mutex::new(());

    thread_local! {
        static HELD: Cell<bool> = const { Cell::new(false) };
    }

    pub(crate) struct Serial(Option<MutexGuard<'static, ()>>);

    pub(crate) fn hold() -> Serial {
        if HELD.get() {
            return Serial(None);
        }
        let guard = OWNER.lock().unwrap_or_else(PoisonError::into_inner);
        HELD.set(true);
        Serial(Some(guard))
    }

    impl Drop for Serial {
        fn drop(&mut self) {
            if self.0.take().is_some() {
                HELD.set(false);
            }
        }
    }
}

#[cfg(test)]
mod tests;
