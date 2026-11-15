//! Linux captures through a nonblocking signalfd over the eligible set. Arming is FD-first: the
//! bounded `/proc/self/status` `SigIgn` parse and the descriptor creation run under inherited
//! dispositions, then one atomic thread-mask swap both blocks the eligible set and captures the
//! exact old mask. The FIFO stdin port reopens `/proc/self/fd/0` as a private nonblocking file
//! description, so the caller's descriptor flags are never mutated.

use std::{
    io::{self, Read},
    os::{
        fd::{AsFd, BorrowedFd},
        unix::fs::{FileTypeExt, OpenOptionsExt},
    },
};

use nix::{
    errno::Errno,
    fcntl::OFlag,
    sys::{
        signal::{SigSet, SigmaskHow, Signal},
        signalfd::{SfdFlags, SignalFd},
    },
};

use crate::cli::input_source::{SourceFailure, usage};
use crate::invocation::SendSourceCause;

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

/// The signalfd over the eligible set; closing it ends the capture.
pub(super) struct Capture(SignalFd);

impl Capture {
    /// `watched` minus every inherited blocked/ignored member becomes the
    /// eligible set: create its signalfd, then block it while keeping every
    /// inherited block. The swap is atomic and hands back the exact prior
    /// mask, returned beside the capture.
    pub(super) fn arm(watched: &[Signal]) -> Result<(Self, SigSet), ArmError> {
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
        let mut swap = old_mask;
        for signal in eligible.iter() {
            swap.add(signal);
        }
        let old_mask = swap.thread_swap_mask(SigmaskHow::SIG_SETMASK)?;
        Ok((Self(fd), old_mask))
    }

    pub(super) fn poll_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }

    /// One nonblocking read: the first pending eligible signal, dequeued.
    pub(super) fn take(&mut self) -> Result<Option<i32>, Errno> {
        self.0
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

/// Reopen the inherited FIFO stdin through `/proc/self/fd/0` as a private
/// nonblocking file description. The reopen must describe the same stream
/// it replaces; anything else is a substitution refused rather than read.
pub(super) fn reopen_fifo_stdin() -> Result<std::fs::File, SourceFailure> {
    let flags = (OFlag::O_NONBLOCK | OFlag::O_CLOEXEC).bits();
    let private = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(flags)
        .open("/proc/self/fd/0")
        .map_err(|_| usage("--stdin", None, SendSourceCause::Unreadable))?;
    let is_fifo = private
        .metadata()
        .map(|metadata| metadata.file_type().is_fifo())
        .map_err(|_| usage("--stdin", None, SendSourceCause::Unreadable))?;
    if !is_fifo {
        return Err(usage("--stdin", None, SendSourceCause::UnsupportedStream));
    }
    Ok(private)
}

/// The watched members this process could capture now: neither blocked on
/// this thread nor inherited as ignored.
#[cfg(test)]
pub(super) fn eligible_now(watched: &[Signal]) -> Vec<Signal> {
    let ignored = inherited_ignored().expect("read inherited ignored set");
    let mask = SigSet::thread_get_mask().expect("read thread mask");
    watched
        .iter()
        .copied()
        .filter(|signal| ignored & (1_u64 << (*signal as u32 - 1)) == 0 && !mask.contains(*signal))
        .collect()
}

/// Dequeue a pending member of `set` without waiting — a fresh signalfd over
/// the probed set reports it — so a test can prove a signal stayed pending
/// and leave nothing behind that could fire later.
#[cfg(test)]
pub(super) fn pending_signo(set: SigSet) -> Option<i32> {
    SignalFd::with_flags(&set, SfdFlags::SFD_NONBLOCK)
        .expect("probe signalfd")
        .read_signal()
        .expect("probe read")
        .map(|info| i32::try_from(info.ssi_signo).unwrap_or(0))
}

#[cfg(test)]
mod tests;
