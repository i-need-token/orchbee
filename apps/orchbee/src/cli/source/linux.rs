//! `terminal send --file/--stdin`: materialize one declared local source
//! into the bounded record its declared form selects — literal bytes, or
//! the Host-bracketed paste content under `--paste`. Everything
//! here runs before session admission, snapshot, request preparation, and
//! submission — a source failure therefore has no request identity and is
//! reported through `orchbee.cli.failure.source` (timeout/cancelled) or the
//! existing `invalid-usage` path (validation), never as transport loss.
//!
//! Signal capture is private on Linux: [`ArmedSignals`] owns a signalfd over
//! the eligible subset of a command stage's own watch set — everything not
//! already blocked or ignored at arm time — for that stage's guarded
//! interval only. The source stage watches the four-signal [`GUARDED`] set
//! for its materialization interval; the Agent read owner borrows the same
//! mechanism for its narrower [`INTERRUPTS`] pair only — SIGHUP/SIGQUIT are
//! never masked or captured there. Arming is FD-first: the bounded
//! `/proc/self/status` `SigIgn` parse and the descriptor creation run under
//! inherited dispositions, then one atomic thread-mask swap both blocks the
//! eligible set and captures the exact old mask. Dispositions are never
//! touched, inherited blocked/ignored state is preserved (an empty eligible
//! set is a real armed policy, not a fallback), and the restore is owed to
//! the common emit finalizer as a noncloneable token whenever a captured
//! signal selects cancellation.

use std::{
    io::{self, IsTerminal, Read},
    os::{
        fd::{AsFd, BorrowedFd},
        unix::fs::{FileTypeExt, OpenOptionsExt},
    },
    time::Instant,
};

use nix::{
    errno::Errno,
    fcntl::OFlag,
    poll::{PollFd, PollFlags, PollTimeout, poll},
    sys::{
        signal::{SigSet, SigmaskHow, Signal},
        signalfd::{SfdFlags, SignalFd},
        stat::{SFlag, fstat},
    },
};

use super::input::{limit_for, usage};
#[cfg(test)]
use super::materialize;
use crate::cli::input_source::{InputSource, SourceFailure};
use crate::cli::{Failure, FailureKind, SourceFinalization};
use crate::invocation::SendSourceCause;
pub(super) type SignalError = Errno;

/// The captured-signal channel polled beside the source descriptor. `take`
/// distinguishes all three outcomes — a captured signal, no pending signal,
/// and a real signalfd error — so an error is never silently read as "none".
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

/// `/proc/self/status` is read through a hard bound so the status file can
/// never produce an unbounded buffer, and its `SigIgn` field must appear
/// exactly once in valid hex — anything else is an internal failure.
const PROC_STATUS_LIMIT: usize = 64 * 1024;

/// A setup failure before the mask swap; every variant maps to
/// `internal-failure` at the command boundary and needs no restore because
/// nothing was changed yet.
#[derive(Debug)]
pub(crate) enum ArmError {
    /// Reading `/proc/self/status` failed at the OS level.
    Io(io::Error),
    /// A signal syscall (mask query, signalfd creation, mask swap) failed.
    Errno(Errno),
    /// The status file was oversized, non-UTF-8, or its `SigIgn` field was
    /// missing, duplicated, or malformed.
    MalformedStatus,
}

impl From<io::Error> for ArmError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<Errno> for ArmError {
    fn from(errno: Errno) -> Self {
        Self::Errno(errno)
    }
}

impl std::fmt::Display for ArmError {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(out, "{error}"),
            Self::Errno(errno) => write!(out, "{errno}"),
            Self::MalformedStatus => out.write_str("malformed /proc/self/status"),
        }
    }
}

/// How the capture interval ended. `Clear` means the descriptor is closed
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

/// The private source signal owner: a nonblocking signalfd over the eligible
/// set plus the inherited thread mask captured at arm. The capture interval
/// begins at the mask swap, before any source I/O; a setup fault before the
/// swap closes the descriptor and owes nothing.
pub(crate) struct ArmedSignals {
    fd: SignalFd,
    old_mask: SigSet,
}

impl ArmedSignals {
    /// Compute the eligible set — [`GUARDED`] minus every signal the process
    /// already blocks or ignores — create its signalfd, then atomically swap
    /// the thread mask and capture the old one.
    pub(crate) fn arm() -> Result<Self, ArmError> {
        Self::arm_set(&GUARDED)
    }

    /// The Agent read owner borrows the identical private arming mechanism
    /// for its own narrower interruption set — [`INTERRUPTS`] minus every
    /// signal the process already blocks or ignores. The eligibility,
    /// atomic mask swap, and restore contracts are exactly `arm`'s; only
    /// membership differs, so SIGHUP/SIGQUIT are never captured or masked.
    pub(crate) fn arm_interrupts() -> Result<Self, ArmError> {
        Self::arm_set(&INTERRUPTS)
    }

    /// The shared private arming body: `watched` minus every inherited
    /// blocked/ignored member becomes the eligible set.
    fn arm_set(watched: &[Signal]) -> Result<Self, ArmError> {
        let ignored = inherited_ignored()?;
        let old_mask = SigSet::thread_get_mask()?;
        let mut eligible = SigSet::empty();
        for signal in watched {
            let bit = 1_u64 << (*signal as u32 - 1);
            if ignored & bit == 0 && !old_mask.contains(*signal) {
                eligible.add(*signal);
            }
        }
        let fd = SignalFd::with_flags(&eligible, SfdFlags::SFD_NONBLOCK | SfdFlags::SFD_CLOEXEC)?;
        // Block the eligible set while keeping every inherited block: the
        // swap is atomic and hands back the exact prior mask.
        let mut swap = old_mask;
        for signal in eligible.iter() {
            swap.add(signal);
        }
        let old_mask = swap.thread_swap_mask(SigmaskHow::SIG_SETMASK)?;
        Ok(Self { fd, old_mask })
    }

    /// Linearization point: the last nonblocking signal read decides
    /// ownership. A captured signal wins cancellation and keeps the mask
    /// held for the output window; `EAGAIN` means the source no longer owns
    /// cancellation — the descriptor closes and the exact old mask is
    /// restored before any runtime admission. A signal arriving after that
    /// read stays pending for its original disposition. A read error still
    /// attempts the owed restore before reporting; a restore failure is
    /// terminal either way.
    pub(crate) fn finish(self) -> Result<Finish, Errno> {
        let read = self.fd.read_signal();
        self.finish_with(read, || {})
    }

    /// The shared arbitration body behind `finish`: `read` is the descriptor
    /// result to linearize on (injectable so error routing is provable), and
    /// `barrier` runs between that read and the owed restore — the documented
    /// late-signal window a test can place a signal inside.
    fn finish_with(
        mut self,
        read: Result<Option<nix::sys::signalfd::siginfo>, Errno>,
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
            Some(info) => Ok(Finish::Cancelled {
                signo: i32::try_from(info.ssi_signo).unwrap_or(0),
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
    fn finish_at(self, barrier: impl FnOnce()) -> Result<Finish, Errno> {
        let read = self.fd.read_signal();
        self.finish_with(read, barrier)
    }

    /// Restore the captured thread mask exactly.
    fn restore(&mut self) -> Result<(), Errno> {
        self.old_mask
            .thread_swap_mask(SigmaskHow::SIG_SETMASK)
            .map(drop)
    }

    /// Hand the owed restore to the common emit finalizer; the mask stays
    /// blocked through the output window and the descriptor drops here.
    pub(crate) fn into_finalization(self) -> SourceFinalization {
        SourceFinalization::new(self.old_mask)
    }
}

impl SignalWatch for ArmedSignals {
    fn poll_fd(&self) -> Option<BorrowedFd<'_>> {
        Some(self.fd.as_fd())
    }

    fn take(&mut self) -> Result<Option<i32>, Errno> {
        self.fd
            .read_signal()
            .map(|signal| signal.map(|info| i32::try_from(info.ssi_signo).unwrap_or(0)))
    }
}

/// Read the inherited ignored mask from `/proc/self/status`: at most
/// [`PROC_STATUS_LIMIT`] bytes total, exactly one `SigIgn:` line, exactly
/// one nonempty hexadecimal field of at most sixteen digits.
fn inherited_ignored() -> Result<u64, ArmError> {
    let mut bytes = Vec::new();
    std::fs::File::open("/proc/self/status")?
        .take(PROC_STATUS_LIMIT as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > PROC_STATUS_LIMIT {
        return Err(ArmError::MalformedStatus);
    }
    let status = String::from_utf8(bytes).map_err(|_| ArmError::MalformedStatus)?;
    status_field(&status, "SigIgn:")
}

/// The strict unique-field check behind `inherited_ignored`, applied to one
/// hexadecimal field name — injectable so the malformed-input matrix is
/// provable without touching the real status file.
fn status_field(status: &str, prefix: &str) -> Result<u64, ArmError> {
    let mut fields = status.lines().filter_map(|line| line.strip_prefix(prefix));
    let field = fields.next().ok_or(ArmError::MalformedStatus)?.trim();
    if fields.next().is_some()
        || field.is_empty()
        || field.len() > 16
        || !field.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(ArmError::MalformedStatus);
    }
    u64::from_str_radix(field, 16).map_err(|_| ArmError::MalformedStatus)
}

/// Acquire `--file`: open without blocking on non-regular paths, `fstat` the
/// already-open descriptor so a swapped path still reports its true type,
/// and accept only a regular file.
pub(super) fn acquire_file(
    path: &str,
    paste: bool,
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
            limit: limit_for(paste),
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

/// The unbuffered borrowed-descriptor reader for the regular-file stdin
/// path: `read(2)` on the shared description advances the inherited file
/// position by exactly the bytes returned — no read-ahead buffer consumes
/// beyond the acquisition cap.
struct BorrowedReader<'a>(BorrowedFd<'a>);

impl Read for BorrowedReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        nix::unistd::read(self.0, buffer).map_err(io::Error::from)
    }
}

/// Acquire `--stdin`: refuse a terminal, then dispatch on the inherited
/// descriptor's exact `S_IFMT` type — a regular file keeps its shared
/// offset, a FIFO is reopened through `/proc/self/fd/0` as a private
/// nonblocking file description so the caller's descriptor flags are never
/// mutated, and anything else is rejected outright.
pub(super) fn acquire_stdin(
    paste: bool,
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
            ReadPlan {
                fd: stdin.as_fd(),
                pollable: false,
                deadline,
                limit: limit_for(paste),
                source: InputSource::Stdin,
                operand: None,
            },
            signals,
        )?
    } else if file_type == SFlag::S_IFIFO {
        let flags = (OFlag::O_NONBLOCK | OFlag::O_CLOEXEC).bits();
        let private = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(flags)
            .open("/proc/self/fd/0")
            .map_err(|_| usage("--stdin", None, SendSourceCause::Unreadable))?;
        // The reopen must describe the same stream it replaces; anything
        // else is a substitution we refuse rather than read.
        let is_fifo = private
            .metadata()
            .map(|metadata| metadata.file_type().is_fifo())
            .map_err(|_| usage("--stdin", None, SendSourceCause::Unreadable))?;
        if !is_fifo {
            return Err(usage("--stdin", None, SendSourceCause::UnsupportedStream));
        }
        read_bounded(
            &mut &private,
            ReadPlan {
                fd: private.as_fd(),
                pollable: true,
                deadline,
                limit: limit_for(paste),
                source: InputSource::Stdin,
                operand: None,
            },
            signals,
        )?
    } else {
        return Err(usage("--stdin", None, SendSourceCause::UnsupportedStream));
    };
    if bytes.is_empty() {
        return Err(usage("--stdin", None, SendSourceCause::Empty));
    }
    Ok(bytes)
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
/// channel) when the descriptor is nonblocking, and drain `POLLHUP` data
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
    pub(in crate::cli) fn arm_interrupts() -> Result<Self, ArmError> {
        ArmedSignals::arm_interrupts().map(Self)
    }
}

#[cfg(test)]
mod tests;
