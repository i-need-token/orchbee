//! Absolute-cap IO wrapper for the client_process fixtures.
//!
//! One `Instant` deadline bounds every operation through the wrapper: each blocking call
//! re-arms the kernel socket timeout to the *remaining* cap and the cap is re-evaluated
//! after every actual IO, so the outcome at the cap is always the "process witness
//! deadline" marker — the criterion the never-split witnesses assert.

use crate::TestResult;
use std::{
    collections::VecDeque,
    io::{self, Read, Write},
    os::unix::net::UnixStream,
    time::{Duration, Instant},
};

pub(super) fn remaining(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|remaining| !remaining.is_zero())
        .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "process witness deadline"))
}

pub(super) fn check_deadline(deadline: Instant) -> io::Result<()> {
    if Instant::now() >= deadline {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "process witness deadline",
        ));
    }
    Ok(())
}

/// The blocking surface `DeadlineStream` needs: `Read`/`Write`/`flush` plus per-call
/// timeout setters re-armed to the remaining cap. The real `UnixStream` implements it;
/// the scripted witness seam below implements it to inject operation-level results.
pub(super) trait DeadlineIo: Read + Write {
    fn arm_read_timeout(&mut self, timeout: Duration) -> io::Result<()>;
    fn arm_write_timeout(&mut self, timeout: Duration) -> io::Result<()>;
}

impl DeadlineIo for UnixStream {
    fn arm_read_timeout(&mut self, timeout: Duration) -> io::Result<()> {
        self.set_read_timeout(Some(timeout))
    }

    fn arm_write_timeout(&mut self, timeout: Duration) -> io::Result<()> {
        self.set_write_timeout(Some(timeout))
    }
}

pub(super) struct DeadlineStream<'a, S: DeadlineIo> {
    stream: &'a mut S,
    deadline: Instant,
}

impl<'a, S: DeadlineIo> DeadlineStream<'a, S> {
    pub(super) fn new(stream: &'a mut S, deadline: Instant) -> io::Result<Self> {
        check_deadline(deadline)?;
        Ok(Self { stream, deadline })
    }

    fn finish<T>(&self, result: io::Result<T>) -> io::Result<T> {
        check_deadline(self.deadline)?;
        result
    }
}

impl<S: DeadlineIo> Read for DeadlineStream<'_, S> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        loop {
            self.stream
                .arm_read_timeout(remaining(self.deadline)?)?;
            match self.stream.read(buffer) {
                // A socket bound can legally expire before the Instant cap (independent
                // clocks). `WouldBlock`/`Interrupted` report zero transferred bytes, so
                // retrying re-checks the real cap instead of surfacing the early expiry.
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    check_deadline(self.deadline)?;
                    std::thread::yield_now();
                }
                result => return self.finish(result),
            }
        }
    }
}

impl<S: DeadlineIo> Write for DeadlineStream<'_, S> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        loop {
            self.stream
                .arm_write_timeout(remaining(self.deadline)?)?;
            match self.stream.write(buffer) {
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    check_deadline(self.deadline)?;
                    std::thread::yield_now();
                }
                result => return self.finish(result),
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        loop {
            self.stream
                .arm_write_timeout(remaining(self.deadline)?)?;
            match self.stream.flush() {
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    check_deadline(self.deadline)?;
                    std::thread::yield_now();
                }
                result => return self.finish(result),
            }
        }
    }
}

/// One scripted outcome per `read` call: exact bytes, a failure, or the same after a
/// stall (used to land results inside or beyond the cap deterministically).
#[derive(Clone, Copy)]
enum ReadStep {
    Bytes(&'static [u8]),
    Fail(io::ErrorKind),
    StallBytes(Duration, &'static [u8]),
    StallFail(Duration, io::ErrorKind),
}

/// One scripted outcome per `write` call: `Limit` accepts at most that many offered
/// bytes (a partial write); `Fail` transfers nothing.
enum WriteStep {
    Limit(usize),
    Fail(io::ErrorKind),
}

/// Deterministic operation-level seam: scripts each call's outcome, records every armed
/// timeout and every accepted write byte so the witnesses can verify exact progress and
/// that the armed bound only ever shrinks (the one cap is never renewed).
#[derive(Default)]
struct ScriptedStream {
    reads: VecDeque<ReadStep>,
    writes: VecDeque<WriteStep>,
    accepted: Vec<u8>,
    armed_reads: Vec<Duration>,
    armed_writes: Vec<Duration>,
}

impl Read for ScriptedStream {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        match self.reads.pop_front().expect("scripted read step missing") {
            ReadStep::Bytes(bytes) => {
                buffer[..bytes.len()].copy_from_slice(bytes);
                Ok(bytes.len())
            }
            ReadStep::Fail(kind) => Err(io::Error::new(kind, "scripted failure")),
            ReadStep::StallBytes(stall, bytes) => {
                std::thread::sleep(stall);
                buffer[..bytes.len()].copy_from_slice(bytes);
                Ok(bytes.len())
            }
            ReadStep::StallFail(stall, kind) => {
                std::thread::sleep(stall);
                Err(io::Error::new(kind, "scripted failure"))
            }
        }
    }
}

impl Write for ScriptedStream {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        match self.writes.pop_front().expect("scripted write step missing") {
            WriteStep::Limit(limit) => {
                let accepted = limit.min(buffer.len());
                self.accepted.extend_from_slice(&buffer[..accepted]);
                Ok(accepted)
            }
            WriteStep::Fail(kind) => Err(io::Error::new(kind, "scripted failure")),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl DeadlineIo for ScriptedStream {
    fn arm_read_timeout(&mut self, timeout: Duration) -> io::Result<()> {
        self.armed_reads.push(timeout);
        Ok(())
    }

    fn arm_write_timeout(&mut self, timeout: Duration) -> io::Result<()> {
        self.armed_writes.push(timeout);
        Ok(())
    }
}

const TEST_CAP: Duration = Duration::from_millis(200);
const STEP_STALL: Duration = Duration::from_millis(40);

fn is_process_deadline(error: &io::Error) -> bool {
    error.to_string().contains("process witness deadline")
}

/// An operation-level `Interrupted`/`WouldBlock` reports zero transferred bytes — a
/// socket bound that fired before the fixture cap. The wrapper must re-check the cap
/// and keep waiting within the remaining budget rather than surfacing it early.
#[test]
fn early_retryable_errors_retry_inside_the_same_cap() -> TestResult {
    let mut stream = ScriptedStream {
        reads: VecDeque::from([
            ReadStep::Fail(io::ErrorKind::Interrupted),
            ReadStep::Fail(io::ErrorKind::WouldBlock),
            ReadStep::Bytes(b"reply"),
        ]),
        ..Default::default()
    };
    let cap = Instant::now() + TEST_CAP;
    let mut buffer = [0_u8; 8];
    let read = DeadlineStream::new(&mut stream, cap)?.read(&mut buffer)?;
    assert_eq!(&buffer[..read], b"reply");
    assert_eq!(stream.armed_reads.len(), 3);
    assert!(
        stream.armed_reads.windows(2).all(|pair| pair[1] <= pair[0]),
        "armed bounds must only shrink toward the one cap: {:?}",
        stream.armed_reads
    );
    Ok(())
}

/// A socket timer that keeps expiring before the cap must still end in the absolute
/// deadline marker at the original cap — not as `WouldBlock`, and not renewed.
#[test]
fn repeated_early_would_block_terminates_at_the_original_cap() -> TestResult {
    let mut stream = ScriptedStream {
        reads: VecDeque::from([ReadStep::StallFail(
            STEP_STALL,
            io::ErrorKind::WouldBlock,
        ); 32]),
        ..Default::default()
    };
    let cap = Instant::now() + TEST_CAP;
    let start = Instant::now();
    let mut buffer = [0_u8; 8];
    let error = DeadlineStream::new(&mut stream, cap)?
        .read(&mut buffer)
        .expect_err("the wait must end in the absolute-deadline error");
    assert!(
        is_process_deadline(&error),
        "early socket expiries must end at the deadline marker: {error}"
    );
    let elapsed = start.elapsed();
    assert!(elapsed >= TEST_CAP, "terminated before the cap: {elapsed:?}");
    assert!(
        elapsed <= TEST_CAP + Duration::from_secs(1),
        "wait overran the original cap: {elapsed:?}"
    );
    assert!(
        stream.armed_reads.windows(2).all(|pair| pair[1] <= pair[0]),
        "armed bounds must only shrink toward the one cap: {:?}",
        stream.armed_reads
    );
    Ok(())
}

/// Partial progress followed by `WouldBlock`: the retried operation must resume exactly
/// where it stopped — no duplicated accepted bytes on write, no dropped or duplicated
/// frame bytes on read.
#[test]
fn partial_progress_then_would_block_transfers_each_byte_once() -> TestResult {
    let mut stream = ScriptedStream {
        writes: VecDeque::from([
            WriteStep::Limit(4),
            WriteStep::Fail(io::ErrorKind::WouldBlock),
            WriteStep::Limit(usize::MAX),
        ]),
        reads: VecDeque::from([
            ReadStep::Bytes(b"ab"),
            ReadStep::Fail(io::ErrorKind::WouldBlock),
            ReadStep::Bytes(b"cd"),
        ]),
        ..Default::default()
    };
    let cap = Instant::now() + TEST_CAP;
    DeadlineStream::new(&mut stream, cap)?.write_all(b"0123456789")?;
    assert_eq!(stream.accepted, b"0123456789");
    let mut buffer = [0_u8; 4];
    DeadlineStream::new(&mut stream, cap)?.read_exact(&mut buffer)?;
    assert_eq!(&buffer, b"abcd");
    Ok(())
}

/// A positive result that only completes at/after the cap is still rejected with the
/// deadline marker — retrying an early `WouldBlock` must not smuggle a late `Ok`.
#[test]
fn positive_result_landing_after_the_cap_is_rejected() -> TestResult {
    let mut stream = ScriptedStream {
        reads: VecDeque::from([
            ReadStep::Fail(io::ErrorKind::WouldBlock),
            ReadStep::StallBytes(TEST_CAP + Duration::from_millis(100), b"late"),
        ]),
        ..Default::default()
    };
    let cap = Instant::now() + TEST_CAP;
    let mut buffer = [0_u8; 8];
    let error = DeadlineStream::new(&mut stream, cap)?
        .read(&mut buffer)
        .expect_err("a result completing after the cap must be rejected");
    assert!(
        is_process_deadline(&error),
        "late success must surface as the deadline error: {error}"
    );
    Ok(())
}

/// Only genuinely retryable operation-level errors retry: an unrelated error before the
/// cap surfaces with its own kind, never converted to the deadline marker.
#[test]
fn unrelated_error_keeps_its_kind_before_the_cap() -> TestResult {
    let mut stream = ScriptedStream {
        reads: VecDeque::from([
            ReadStep::Fail(io::ErrorKind::WouldBlock),
            ReadStep::Fail(io::ErrorKind::PermissionDenied),
        ]),
        ..Default::default()
    };
    let cap = Instant::now() + TEST_CAP;
    let mut buffer = [0_u8; 8];
    let error = DeadlineStream::new(&mut stream, cap)?
        .read(&mut buffer)
        .expect_err("the unrelated error must surface unchanged");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied, "{error}");
    assert!(!is_process_deadline(&error), "{error}");
    Ok(())
}
