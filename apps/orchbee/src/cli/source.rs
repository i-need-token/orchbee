//! `terminal send --file/--stdin`: materialize one declared local source
//! into the same bounded text record `--text` already carries. Everything
//! here runs before session admission, snapshot, request preparation, and
//! submission — a source failure therefore has no request identity and is
//! reported through `orchbee.cli.failure.source` (timeout/cancelled) or the
//! existing `invalid-usage` path (validation), never as transport loss.
//!
//! Signal capture is private to the source stage on Linux: [`ArmedSignals`]
//! owns a signalfd over the eligible subset of the termination set —
//! everything not already blocked or ignored at arm time — for the
//! materialization interval only. Arming is FD-first: the bounded
//! `/proc/self/status` `SigIgn` parse and the descriptor creation run under
//! inherited dispositions, then one atomic thread-mask swap both blocks the
//! eligible set and captures the exact old mask. Dispositions are never
//! touched, inherited blocked/ignored state is preserved (an empty eligible
//! set is a real armed policy, not a fallback), and the restore is owed to
//! the common emit finalizer as a noncloneable token whenever a captured
//! signal selects cancellation.

use std::{
    borrow::Cow,
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
use serde_json::{Map, Value, json};

use super::{Failure, FailureKind, SourceFinalization};
use crate::invocation::{SendSourceCause, TerminalSendInput, UsageReason};

/// The wire payload bound a materialized source must satisfy — the literal
/// bytes plus an optional folded CR, identical to `--text`.
const LIMIT: usize = crate::invocation::MAXIMUM_SEND_TEXT_BYTES;

/// Which declared source a source-stage failure names in its typed details.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InputSource {
    File,
    Stdin,
}

impl InputSource {
    const fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Stdin => "stdin",
        }
    }

    const fn option(self) -> &'static str {
        match self {
            Self::File => "--file",
            Self::Stdin => "--stdin",
        }
    }
}

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
        let ignored = inherited_ignored()?;
        let old_mask = SigSet::thread_get_mask()?;
        let mut eligible = SigSet::empty();
        for signal in GUARDED {
            let bit = 1_u64 << (signal as u32 - 1);
            if ignored & bit == 0 && !old_mask.contains(signal) {
                eligible.add(signal);
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

/// How a source stage ends before any runtime effect. `Standard` failures
/// keep the existing `orchbee.cli.failure` envelope (usage/validation/local
/// kinds); `Timeout`/`Cancelled` carry the `orchbee.cli.failure.source`
/// envelope through the explicit typed conversion in `send.rs`.
#[derive(Debug)]
pub(crate) enum SourceFailure {
    /// Validation or local failure on the standard failure path.
    Standard(Failure),
    /// The shared invocation deadline elapsed before the source completed.
    Timeout { source: InputSource },
    /// An eligible captured signal ended source acquisition.
    Cancelled { source: InputSource, signal: i32 },
}

/// The typed details object the `orchbee.cli.failure.source` envelope
/// carries: which declared source and, for a timeout, the configured shared
/// budget — never a measured duration, path, or payload byte.
pub(crate) fn source_details(source: InputSource, timeout_ms: Option<u64>) -> Map<String, Value> {
    let mut details = Map::from_iter([
        ("stage".to_owned(), json!("input")),
        ("inputSource".to_owned(), json!(source.as_str())),
    ]);
    if let Some(timeout_ms) = timeout_ms {
        details.insert("timeoutMs".to_owned(), json!(timeout_ms));
    }
    details
}

/// A source-stage usage failure on the standard `invalid-usage` path: the
/// machine envelope carries no details; the localized stderr names the
/// declared option and its concrete local cause.
fn usage(option: &'static str, value: Option<String>, cause: SendSourceCause) -> SourceFailure {
    SourceFailure::Standard(Failure {
        kind: FailureKind::InvalidUsage,
        details: None,
        source: None,
        failure_schema: None,
        usage: Some(Box::new(UsageReason::SendSource {
            option,
            value,
            cause,
        })),
    })
}

/// Materialize the parsed input declaration into the record the send path
/// already submits. `Text`/`Key` pass through untouched; `File`/`Stdin` are
/// acquired, bounded, and validated here — the returned `Text` variant keeps
/// the existing single-request lifecycle byte-identical downstream.
pub(crate) fn materialize<'a>(
    input: &'a TerminalSendInput,
    deadline: Instant,
    signals: &mut dyn SignalWatch,
) -> Result<Cow<'a, TerminalSendInput>, SourceFailure> {
    let (bytes, enter, option, value) = match input {
        TerminalSendInput::File { path, enter } => (
            acquire_file(path, deadline, signals)?,
            *enter,
            "--file",
            Some(path.clone()),
        ),
        TerminalSendInput::Stdin { enter } => {
            (acquire_stdin(deadline, signals)?, *enter, "--stdin", None)
        }
        TerminalSendInput::Text { .. } | TerminalSendInput::Key(_) => {
            return Ok(Cow::Borrowed(input));
        }
    };
    if bytes.len() + usize::from(enter) > LIMIT {
        return Err(usage(option, value, SendSourceCause::Oversize));
    }
    let text =
        String::from_utf8(bytes).map_err(|_| usage(option, value, SendSourceCause::NotUtf8))?;
    // `--enter` stays the wire's folded-CR flag, so a materialized source is
    // indistinguishable from `--text` downstream — the folded carriage
    // return counts toward the same record bound.
    Ok(Cow::Owned(TerminalSendInput::Text { text, enter }))
}

/// Acquire `--file`: open without blocking on non-regular paths, `fstat` the
/// already-open descriptor so a swapped path still reports its true type,
/// and accept only a regular file.
fn acquire_file(
    path: &str,
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
fn acquire_stdin(
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
/// deadline, and the declared source identity for diagnostics. `operand`
/// is the declared source value (the `--file` path) echoed verbatim in
/// usage diagnostics — never added to machine details.
struct ReadPlan<'a> {
    fd: BorrowedFd<'a>,
    pollable: bool,
    deadline: Instant,
    source: InputSource,
    operand: Option<&'a str>,
}

/// The shared bounded read loop: check cancellation and the deadline before
/// every wait and every read, poll the source (plus the injected signal
/// channel) when the descriptor is nonblocking, and drain `POLLHUP` data
/// before counting EOF. The buffer never exceeds `LIMIT + 1`, so exactly
/// 4,096 bytes succeeds and 4,097 is rejected without draining further. A
/// non-pollable regular file answers each read directly — a kernel-stalled
/// read there is the documented non-preemptible residual.
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
        source,
        operand,
    } = plan;
    let mut bytes = Vec::with_capacity(LIMIT + 1);
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
        let spare = (LIMIT + 1).saturating_sub(bytes.len());
        if spare == 0 {
            return Err(usage(
                source.option(),
                operand.map(str::to_owned),
                SendSourceCause::Oversize,
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

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        os::{fd::BorrowedFd, unix::process::ExitStatusExt},
        path::{Path, PathBuf},
        process::{Command, Stdio},
        sync::atomic::{AtomicU32, Ordering},
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    use nix::{
        errno::Errno,
        sys::{
            signal::{self, SigSet, Signal},
            stat::Mode,
        },
        unistd::mkfifo,
    };

    use super::*;
    use crate::invocation::{SendSourceCause, TerminalSendInput, UsageReason};

    /// A watch that never reports a signal — the injected baseline for
    /// materialization tests that do not exercise the armed guard.
    struct IdleWatch;

    impl SignalWatch for IdleWatch {
        fn poll_fd(&self) -> Option<BorrowedFd<'_>> {
            None
        }

        fn take(&mut self) -> Result<Option<i32>, Errno> {
            Ok(None)
        }
    }

    /// A watch whose `take` always fails — mock evidence that a signal-read
    /// error routes to internal failure and never collapses to "none".
    struct ErrWatch;

    impl SignalWatch for ErrWatch {
        fn poll_fd(&self) -> Option<BorrowedFd<'_>> {
            None
        }

        fn take(&mut self) -> Result<Option<i32>, Errno> {
            Err(Errno::EIO)
        }
    }

    /// A watch reporting a scripted signal — mock evidence for mid-read
    /// cancellation routing without touching real process signals.
    struct ScriptedSignal(i32);

    impl SignalWatch for ScriptedSignal {
        fn poll_fd(&self) -> Option<BorrowedFd<'_>> {
            None
        }

        fn take(&mut self) -> Result<Option<i32>, Errno> {
            Ok(Some(self.0))
        }
    }

    fn workspace(tag: &str) -> PathBuf {
        static SEQUENCE: AtomicU32 = AtomicU32::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = env::temp_dir().join(format!(
            "obsrc-{tag}-{}-{nanos}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).expect("create source test workspace");
        dir
    }

    fn write_file(dir: &Path, name: &str, bytes: &[u8]) -> String {
        let path = dir.join(name);
        fs::write(&path, bytes).expect("write fixture file");
        path.to_str().expect("fixture path is UTF-8").to_owned()
    }

    fn deadline() -> Instant {
        Instant::now() + Duration::from_secs(30)
    }

    fn file_input(path: &str, enter: bool) -> TerminalSendInput {
        TerminalSendInput::File {
            path: path.to_owned(),
            enter,
        }
    }

    /// The localized usage cause a standard source failure carries.
    fn cause(failure: &SourceFailure) -> SendSourceCause {
        let SourceFailure::Standard(Failure {
            usage: Some(reason),
            ..
        }) = failure
        else {
            panic!("expected a usage failure, got {failure:?}")
        };
        let UsageReason::SendSource { cause, .. } = reason.as_ref() else {
            panic!("expected a send-source usage reason")
        };
        *cause
    }

    /// The declared operand a source usage diagnostic echoes, when it does.
    fn echoed_value(failure: &SourceFailure) -> Option<&str> {
        let SourceFailure::Standard(Failure {
            usage: Some(reason),
            ..
        }) = failure
        else {
            panic!("expected a usage failure, got {failure:?}")
        };
        let UsageReason::SendSource { value, .. } = reason.as_ref() else {
            panic!("expected a send-source usage reason")
        };
        value.as_deref()
    }

    /// The text record a successful materialization produced.
    fn materialized_text(
        input: &TerminalSendInput,
        signals: &mut dyn SignalWatch,
    ) -> (String, bool) {
        match materialize(input, deadline(), signals)
            .expect("materialize")
            .into_owned()
        {
            TerminalSendInput::Text { text, enter } => (text, enter),
            _ => panic!("materialization did not fold into text"),
        }
    }

    /// The first guarded signal neither inherited-blocked nor inherited-
    /// ignored, or none when the process excludes the whole set.
    fn first_eligible() -> Option<Signal> {
        let ignored = inherited_ignored().expect("read inherited ignored set");
        let mask = SigSet::thread_get_mask().expect("read thread mask");
        GUARDED.into_iter().find(|signal| {
            ignored & (1_u64 << (*signal as u32 - 1)) == 0 && !mask.contains(*signal)
        })
    }

    /// Proof that a signal stayed pending: a fresh signalfd over the probed
    /// set reports the preserved pending signal (and drains it, so the test
    /// leaves nothing behind that could fire later).
    fn pending_signo(set: SigSet) -> Option<i32> {
        SignalFd::with_flags(&set, SfdFlags::SFD_NONBLOCK)
            .expect("probe signalfd")
            .read_signal()
            .expect("probe read")
            .map(|info| i32::try_from(info.ssi_signo).unwrap_or(0))
    }

    #[test]
    fn status_field_accepts_only_one_valid_hex_value() {
        let good = "Name:\tfoo\nSigIgn:\t0000000000000006\nSigBlk:\t0\n";
        assert_eq!(status_field(good, "SigIgn:").unwrap(), 6);
        assert_eq!(
            status_field("SigIgn:\tffffffffffffffff\n", "SigIgn:").unwrap(),
            u64::MAX
        );
        for bad in [
            "SigIgn:\n",                    // empty
            "SigIgn:   \n",                 // whitespace only
            "SigIgn:\t\n",                  // tab only
            "SigIgn:\t00000000000000006\n", // seventeen digits
            "SigIgn:\t6 extra\n",           // trailing token
            "SigIgn:\tzz\n",                // non-hex
            "NoSuch:\t1\n",                 // missing field
            "SigIgn:\t1\nSigIgn:\t2\n",     // duplicated field
        ] {
            assert!(
                matches!(status_field(bad, "SigIgn:"), Err(ArmError::MalformedStatus)),
                "{bad:?} must be rejected"
            );
        }
    }

    #[test]
    fn file_source_materializes_verbatim() {
        let dir = workspace("file-ok");
        let path = write_file(&dir, "input", b"line one\nline two\n");
        let mut watch = IdleWatch;
        let (text, enter) = materialized_text(&file_input(&path, false), &mut watch);
        assert_eq!(text, "line one\nline two\n");
        assert!(!enter);
    }

    #[test]
    fn file_record_boundaries() {
        let dir = workspace("file-cap");
        let mut watch = IdleWatch;
        // LIMIT-1 bytes + --enter fits: the folded-CR flag counts +1 byte.
        let path = write_file(&dir, "fold-ok", &vec![b'x'; LIMIT - 1]);
        let (text, enter) = materialized_text(&file_input(&path, true), &mut watch);
        assert_eq!(text.len(), LIMIT - 1);
        assert!(enter, "--enter survives as the wire fold flag");
        // LIMIT bytes without --enter is exactly the bound.
        let path = write_file(&dir, "exact", &vec![b'y'; LIMIT]);
        let (text, enter) = materialized_text(&file_input(&path, false), &mut watch);
        assert_eq!(text.len(), LIMIT);
        assert!(!enter);
        // LIMIT bytes + --enter crosses the bound by one.
        let failure = materialize(&file_input(&path, true), deadline(), &mut watch).unwrap_err();
        assert_eq!(cause(&failure), SendSourceCause::Oversize);
        // LIMIT+1 bytes crosses outright.
        let path = write_file(&dir, "over", &vec![b'z'; LIMIT + 1]);
        let failure = materialize(&file_input(&path, false), deadline(), &mut watch).unwrap_err();
        assert_eq!(cause(&failure), SendSourceCause::Oversize);
    }

    /// Every `--file` diagnostic echoes the path the caller named — the
    /// oversize caught inside the bounded read used to lose it, so both
    /// oversize routes are pinned to the declared path.
    #[test]
    fn file_oversize_echoes_its_declared_path() {
        let dir = workspace("file-oversize-path");
        let mut watch = IdleWatch;
        // The read loop hits LIMIT+1.
        let path = write_file(&dir, "over", &vec![b'z'; LIMIT + 1]);
        let failure = materialize(&file_input(&path, false), deadline(), &mut watch).unwrap_err();
        assert_eq!(cause(&failure), SendSourceCause::Oversize);
        assert_eq!(echoed_value(&failure), Some(path.as_str()));
        // LIMIT + --enter overflows after a complete read — same echo.
        let path = write_file(&dir, "fold-over", &vec![b'y'; LIMIT]);
        let failure = materialize(&file_input(&path, true), deadline(), &mut watch).unwrap_err();
        assert_eq!(cause(&failure), SendSourceCause::Oversize);
        assert_eq!(echoed_value(&failure), Some(path.as_str()));
    }

    #[test]
    fn file_source_rejects_invalid_and_unsupported() {
        let dir = workspace("file-bad");
        let mut watch = IdleWatch;
        // Non-UTF-8 bytes.
        let path = write_file(&dir, "binary", &[0xff, 0xfe, 0xfd]);
        let failure = materialize(&file_input(&path, false), deadline(), &mut watch).unwrap_err();
        assert_eq!(cause(&failure), SendSourceCause::NotUtf8);
        // Empty.
        let path = write_file(&dir, "empty", b"");
        let failure = materialize(&file_input(&path, false), deadline(), &mut watch).unwrap_err();
        assert_eq!(cause(&failure), SendSourceCause::Empty);
        // A directory is not a regular file.
        let failure = materialize(
            &file_input(dir.to_str().expect("dir path"), false),
            deadline(),
            &mut watch,
        )
        .unwrap_err();
        assert_eq!(cause(&failure), SendSourceCause::NotRegularFile);
        // A FIFO is not a regular file — the open must not block on it.
        let fifo_path = dir.join("pipe");
        mkfifo(&fifo_path, Mode::S_IRUSR | Mode::S_IWUSR).expect("mkfifo");
        let failure = materialize(
            &file_input(fifo_path.to_str().expect("fifo path"), false),
            deadline(),
            &mut watch,
        )
        .unwrap_err();
        assert_eq!(cause(&failure), SendSourceCause::NotRegularFile);
        // A missing path is unreadable, never not-regular.
        let failure = materialize(
            &file_input("/nonexistent-obsrc-source-path", false),
            deadline(),
            &mut watch,
        )
        .unwrap_err();
        assert_eq!(cause(&failure), SendSourceCause::Unreadable);
    }

    #[test]
    fn source_deadline_elapses_cleanly() {
        let dir = workspace("file-timeout");
        let path = write_file(&dir, "data", b"payload");
        let mut watch = IdleWatch;
        let past = Instant::now()
            .checked_sub(Duration::from_millis(1))
            .expect("past deadline");
        match materialize(&file_input(&path, false), past, &mut watch) {
            Err(SourceFailure::Timeout {
                source: InputSource::File,
            }) => {}
            other => panic!("past deadline must be a typed timeout, got {other:?}"),
        }
    }

    #[test]
    fn signal_read_error_is_internal_not_none() {
        // Mock evidence: a failing watch must route internal failure — a
        // signal-read error never collapses to "no signal".
        let dir = workspace("file-err");
        let path = write_file(&dir, "data", b"payload");
        let mut watch = ErrWatch;
        match materialize(&file_input(&path, false), deadline(), &mut watch) {
            Err(SourceFailure::Standard(Failure {
                kind: FailureKind::InternalFailure,
                ..
            })) => {}
            other => panic!("signal read error must be internal failure, got {other:?}"),
        }
    }

    #[test]
    fn mid_read_signal_selects_typed_cancellation() {
        let dir = workspace("file-int");
        let path = write_file(&dir, "data", b"payload");
        let mut watch = ScriptedSignal(15);
        match materialize(&file_input(&path, false), deadline(), &mut watch) {
            Err(SourceFailure::Cancelled {
                source: InputSource::File,
                signal: 15,
            }) => {}
            other => panic!("captured signal must select cancellation, got {other:?}"),
        }
    }

    #[test]
    fn armed_capture_and_finalization_restore_exact_mask() {
        let before = SigSet::thread_get_mask().expect("thread mask");
        let Some(signal) = first_eligible() else {
            // The whole set excluded: an empty eligible set is still a real
            // armed policy — arm, observe no capture, finish clear, restore.
            let mut armed = ArmedSignals::arm().expect("arm empty eligible set");
            assert_eq!(armed.take().expect("take"), None);
            match armed.finish().expect("finish") {
                Finish::Clear => {}
                _ => panic!("an empty eligible set must finish clear"),
            }
            assert_eq!(SigSet::thread_get_mask().unwrap(), before);
            return;
        };
        let mut armed = ArmedSignals::arm().expect("arm signals");
        signal::raise(signal).expect("raise captured signal");
        assert_eq!(armed.take().expect("take"), Some(signal as i32));
        armed
            .into_finalization()
            .release()
            .expect("release restores mask");
        assert_eq!(SigSet::thread_get_mask().unwrap(), before);
    }

    #[test]
    fn inherited_blocked_signal_excluded_and_pending_preserved() {
        // SIGINT blocked before arm: excluded from the captured set, and its
        // blocked+pending state must survive arm/finish untouched.
        let mut blocked = SigSet::empty();
        blocked.add(Signal::SIGINT);
        blocked.thread_block().expect("block SIGINT");
        signal::raise(Signal::SIGINT).expect("pend SIGINT");
        let before = SigSet::thread_get_mask().expect("thread mask");
        assert!(before.contains(Signal::SIGINT));
        let armed = ArmedSignals::arm().expect("arm with inherited block");
        match armed.finish().expect("finish") {
            Finish::Clear => {}
            _ => panic!("an excluded signal must not select cancellation"),
        }
        let after = SigSet::thread_get_mask().expect("thread mask");
        assert_eq!(before, after, "exact inherited mask restored");
        assert!(after.contains(Signal::SIGINT));
        // The inherited-blocked signal was excluded from the captured set:
        // a probe descriptor over SIGINT reports the preserved pending one.
        let mut probe = SigSet::empty();
        probe.add(Signal::SIGINT);
        assert_eq!(
            pending_signo(probe),
            Some(Signal::SIGINT as i32),
            "the pending SIGINT must be preserved, not swallowed"
        );
        // The signal stays blocked: unblocking it would deliver a further
        // pending SIGINT and terminate the test process.
    }

    #[test]
    fn finish_read_error_restores_mask_then_reports() {
        // Mock evidence: a signalfd read error at the arbitration point
        // still attempts the owed restore before reporting the error.
        let before = SigSet::thread_get_mask().expect("thread mask");
        let armed = ArmedSignals::arm().expect("arm");
        match armed.finish_with(Err(Errno::EIO), || {}) {
            Err(error) => assert_eq!(error, Errno::EIO),
            Ok(_) => panic!("a signal read error must report failure"),
        }
        assert_eq!(SigSet::thread_get_mask().unwrap(), before);
    }

    #[test]
    fn finish_captured_signal_holds_mask_until_release() {
        // A captured signal at the finish boundary keeps the eligible mask
        // blocked; the exact inherited mask is restored only by the token.
        let Some(signal) = first_eligible() else {
            return;
        };
        let before = SigSet::thread_get_mask().expect("thread mask");
        let armed = ArmedSignals::arm().expect("arm");
        signal::raise(signal).expect("raise captured signal");
        match armed.finish().expect("finish") {
            Finish::Cancelled {
                signo,
                finalization,
            } => {
                assert_eq!(signo, signal as i32);
                assert!(
                    SigSet::thread_get_mask().unwrap().contains(signal),
                    "the captured signal stays masked until release"
                );
                finalization.release().expect("release");
            }
            _ => panic!("captured signal must select cancellation"),
        }
        assert_eq!(SigSet::thread_get_mask().unwrap(), before);
    }

    /// A scripted monotonic clock for the `read_bounded_at` seam: each
    /// `now` call returns the next scripted instant, then holds the last
    /// one — the test decides exactly which deadline probe expires, so no
    /// wall-clock sleep can pass at an earlier boundary.
    struct ScriptedClock {
        times: Vec<Instant>,
        next: usize,
    }

    impl ScriptedClock {
        fn now(&mut self) -> Instant {
            let index = self.next.min(self.times.len().saturating_sub(1));
            self.next += 1;
            self.times[index]
        }
    }

    /// A reader that counts `read` invocations — the discriminating
    /// observation for which boundary rejected the source.
    struct CountingReader<R> {
        inner: R,
        reads: usize,
    }

    impl<R: Read> Read for CountingReader<R> {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            self.reads += 1;
            self.inner.read(buffer)
        }
    }

    /// A watch whose injected channel is always poll-ready and never
    /// reports a signal — pairs with the scripted clock to reach the
    /// post-wait arbitration point without writer timing.
    struct ReadyWatch<'a>(BorrowedFd<'a>);

    impl SignalWatch for ReadyWatch<'_> {
        fn poll_fd(&self) -> Option<BorrowedFd<'_>> {
            Some(self.0)
        }

        fn take(&mut self) -> Result<Option<i32>, Errno> {
            Ok(None)
        }
    }

    #[test]
    fn deadline_arbitrates_ready_data_after_wait() {
        // Script: the loop-top probe is inside the budget; the post-wait,
        // pre-read probe lands past it. Both descriptors are poll-ready so
        // the wait itself never gates on the clock, and the reader count
        // proves the ready byte was rejected before it could be read.
        let t0 = Instant::now();
        let deadline = t0 + Duration::from_secs(1);
        let (source_read, source_write) = nix::unistd::pipe().expect("source pipe");
        let (signal_read, signal_write) = nix::unistd::pipe().expect("signal pipe");
        nix::unistd::write(&source_write, b"x").expect("seed source byte");
        nix::unistd::write(&signal_write, b"s").expect("seed signal byte");
        let source_file = fs::File::from(source_read);
        let mut watch = ReadyWatch(signal_read.as_fd());
        let mut clock = ScriptedClock {
            times: vec![
                t0 + Duration::from_millis(10),
                deadline + Duration::from_millis(10),
            ],
            next: 0,
        };
        let mut reader = CountingReader {
            inner: &source_file,
            reads: 0,
        };
        let failure = read_bounded_at(
            &mut reader,
            ReadPlan {
                fd: source_file.as_fd(),
                pollable: true,
                deadline,
                source: InputSource::File,
                operand: None,
            },
            &mut watch,
            || clock.now(),
        )
        .unwrap_err();
        assert!(
            matches!(failure, SourceFailure::Timeout { .. }),
            "ready data after deadline expiry must be timeout, got {failure:?}"
        );
        assert_eq!(
            reader.reads, 0,
            "the post-wait check must reject before any read"
        );
        drop(signal_write);
    }

    #[test]
    fn deadline_arbitrates_eof_at_success_boundary() {
        // Script: loop-top and pre-read probes inside the budget, the
        // completion-boundary probe past it. The reader count proves the
        // EOF was actually consumed — the final check fired, not an
        // earlier loop check.
        let t0 = Instant::now();
        let deadline = t0 + Duration::from_secs(1);
        let (read_end, _write_end) = nix::unistd::pipe().expect("pipe");
        let mut watch = IdleWatch;
        let mut clock = ScriptedClock {
            times: vec![
                t0 + Duration::from_millis(10),
                t0 + Duration::from_millis(20),
                deadline + Duration::from_millis(10),
            ],
            next: 0,
        };
        let mut reader = CountingReader {
            inner: io::empty(),
            reads: 0,
        };
        let failure = read_bounded_at(
            &mut reader,
            ReadPlan {
                fd: read_end.as_fd(),
                pollable: false,
                deadline,
                source: InputSource::Stdin,
                operand: None,
            },
            &mut watch,
            || clock.now(),
        )
        .unwrap_err();
        assert!(
            matches!(failure, SourceFailure::Timeout { .. }),
            "a late EOF must arbitrate to timeout, got {failure:?}"
        );
        assert_eq!(
            reader.reads, 1,
            "the EOF was consumed — the boundary check itself fired"
        );
    }

    /// A stateful watch reporting nothing on the first drain and the
    /// scripted signal on the second — so the test reaches the completion
    /// boundary before cancellation is consumed (unlike `ScriptedSignal`,
    /// which fires at the loop-top drain).
    struct BoundarySignal {
        signal: i32,
        takes: usize,
    }

    impl SignalWatch for BoundarySignal {
        fn poll_fd(&self) -> Option<BorrowedFd<'_>> {
            None
        }

        fn take(&mut self) -> Result<Option<i32>, Errno> {
            self.takes += 1;
            Ok((self.takes > 1).then_some(self.signal))
        }
    }

    #[test]
    fn signal_wins_over_expired_deadline_at_boundary() {
        // Deterministic trace: drain#1 none, clock#1/#2 in budget, reader
        // returns EOF once, drain#2 yields the signal — cancellation is
        // selected before clock#3 would report the budget closed.
        let t0 = Instant::now();
        let deadline = t0 + Duration::from_secs(1);
        let (read_end, _write_end) = nix::unistd::pipe().expect("pipe");
        let mut watch = BoundarySignal {
            signal: 2,
            takes: 0,
        };
        let mut clock = ScriptedClock {
            times: vec![
                t0 + Duration::from_millis(10),
                t0 + Duration::from_millis(20),
                deadline + Duration::from_millis(10),
            ],
            next: 0,
        };
        let mut reader = CountingReader {
            inner: io::empty(),
            reads: 0,
        };
        let failure = read_bounded_at(
            &mut reader,
            ReadPlan {
                fd: read_end.as_fd(),
                pollable: false,
                deadline,
                source: InputSource::Stdin,
                operand: None,
            },
            &mut watch,
            || clock.now(),
        )
        .unwrap_err();
        match failure {
            SourceFailure::Cancelled { source, signal } => {
                assert!(matches!(source, InputSource::Stdin));
                assert_eq!(signal, 2);
            }
            other => panic!("a captured signal must outrank the expired deadline, got {other:?}"),
        }
        assert_eq!(reader.reads, 1, "the EOF was consumed — boundary reached");
        assert_eq!(watch.takes, 2, "the second drain selected cancellation");
        assert_eq!(
            clock.next, 2,
            "the expired clock probe was never consumed — signal won first"
        );
    }

    /// Child-mode probe: a signal raised in the documented late window —
    /// after the last arbitration read and before the owed restore — must
    /// reach its original disposition, i.e. OS termination before any
    /// runtime admission, never typed cancellation or silence.
    const LATE_CHILD_ENV: &str = "ORCHBEE_TEST_LATE_SIGNAL_CHILD";

    #[test]
    fn late_signal_in_finish_window_dies_by_os_disposition() {
        if env::var_os(LATE_CHILD_ENV).is_some() {
            // Child: the barrier runs strictly after the final arbitration
            // read; the restore must unmask the late signal into its
            // original disposition — the process dies by that signal.
            let signal = first_eligible().expect("an eligible signal to probe");
            let armed = ArmedSignals::arm().expect("arm");
            let _ = armed.finish_at(|| signal::raise(signal).expect("raise late signal"));
            // Reaching here means the late signal was silently swallowed.
            std::process::abort();
        }
        let output = Command::new(env::current_exe().expect("test binary"))
            .args([
                "--exact",
                "cli::source::tests::late_signal_in_finish_window_dies_by_os_disposition",
                "--test-threads=1",
                "--nocapture",
            ])
            .env(LATE_CHILD_ENV, "1")
            .stdin(Stdio::null())
            .output()
            .expect("spawn child test");
        let signo = output
            .status
            .signal()
            .expect("child must die by a signal, not exit or abort");
        assert!(
            GUARDED.iter().any(|guarded| *guarded as i32 == signo),
            "death must come from the probed guarded set, got {signo}"
        );
    }
}
