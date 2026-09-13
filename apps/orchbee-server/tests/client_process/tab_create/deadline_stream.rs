//! Absolute-cap IO wrapper for the client_process fixtures.
//!
//! One `Instant` deadline bounds every operation through the wrapper: each blocking call
//! re-arms the kernel socket timeout to the *remaining* cap and the cap is re-evaluated
//! after every actual IO, so the outcome at the cap is always the "process witness
//! deadline" marker — the criterion the never-split witnesses assert.
//!
//! The wrapper takes its `Instant` reads through a single narrow seam so the scripted
//! witnesses below can drive the same `remaining`/pre/post checks with a controllable
//! monotonic clock. The real `UnixStream` path always uses `Instant::now`.

use crate::TestResult;
use std::{
    cell::Cell,
    collections::VecDeque,
    io::{self, Read, Write},
    os::unix::net::UnixStream,
    rc::Rc,
    time::{Duration, Instant},
};

fn remaining_at(deadline: Instant, now: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(now)
        .filter(|remaining| !remaining.is_zero())
        .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "process witness deadline"))
}

fn check_deadline_at(deadline: Instant, now: Instant) -> io::Result<()> {
    if now >= deadline {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "process witness deadline",
        ));
    }
    Ok(())
}

pub(super) fn check_deadline(deadline: Instant) -> io::Result<()> {
    check_deadline_at(deadline, Instant::now())
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
    now: Box<dyn Fn() -> Instant>,
}

impl<'a> DeadlineStream<'a, UnixStream> {
    /// Real fixture path: the process clock is `Instant::now`.
    pub(super) fn new(stream: &'a mut UnixStream, deadline: Instant) -> io::Result<Self> {
        Self::with_clock(stream, deadline, Box::new(Instant::now))
    }
}

impl<'a, S: DeadlineIo> DeadlineStream<'a, S> {
    /// Same wrapper, same checks — only the `Instant` source differs. Witnesses inject a
    /// scripted clock; nothing else about the algorithm changes.
    fn with_clock(
        stream: &'a mut S,
        deadline: Instant,
        now: Box<dyn Fn() -> Instant>,
    ) -> io::Result<Self> {
        check_deadline_at(deadline, now())?;
        Ok(Self {
            stream,
            deadline,
            now,
        })
    }

    fn now(&self) -> Instant {
        (self.now)()
    }

    fn finish<T>(&self, result: io::Result<T>) -> io::Result<T> {
        check_deadline_at(self.deadline, self.now())?;
        result
    }

    /// One shared attempt loop for read/write/flush: arm the remaining cap, run the
    /// operation, re-check the cap after it. The observed defect this guards is narrow:
    /// in the first CLI gate an operation-level `WouldBlock` surfaced while the
    /// fixture's `Instant` cap was still ahead. `WouldBlock`/`Interrupted` transfer zero
    /// bytes, so they retry after re-checking the real cap; every other result keeps its
    /// own error or byte count, still bounded by the post-call check.
    fn operate<T>(
        &mut self,
        mut arm: impl FnMut(&mut S, Duration) -> io::Result<()>,
        mut op: impl FnMut(&mut S) -> io::Result<T>,
    ) -> io::Result<T> {
        loop {
            arm(self.stream, remaining_at(self.deadline, self.now())?)?;
            match op(self.stream) {
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    check_deadline_at(self.deadline, self.now())?;
                    std::thread::yield_now();
                }
                result => return self.finish(result),
            }
        }
    }
}

impl<S: DeadlineIo> Read for DeadlineStream<'_, S> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.operate(S::arm_read_timeout, |stream| stream.read(buffer))
    }
}

impl<S: DeadlineIo> Write for DeadlineStream<'_, S> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.operate(S::arm_write_timeout, |stream| stream.write(buffer))
    }

    fn flush(&mut self) -> io::Result<()> {
        self.operate(S::arm_write_timeout, S::flush)
    }
}

/// One scripted outcome per `read` call: advance the scripted clock by `advance`, then
/// transfer `bytes` or fail with `kind`. Advancing the shared clock cell is what makes
/// the boundary cases exact — no wall-clock sleep is involved anywhere.
#[derive(Clone, Copy)]
enum ReadStep {
    Deliver(Duration, &'static [u8]),
    FailAfter(Duration, io::ErrorKind),
}

/// One scripted outcome per `write` call: advance the clock, then `Deliver` accepts at
/// most `limit` offered bytes (a partial write) while `FailAfter` transfers nothing.
enum WriteStep {
    Deliver(Duration, usize),
    FailAfter(Duration, io::ErrorKind),
}

/// One scripted outcome per `flush` call.
enum FlushStep {
    Deliver(Duration),
    FailAfter(Duration, io::ErrorKind),
}

/// Deterministic operation-level seam sharing the wrapper's clock cell: scripts each
/// call's outcome, counts actual IO invocations (`ops`), records every armed timeout and
/// every offered/accepted write buffer so the witnesses can verify exact progress, exact
/// armed bounds, and that a retry never reserializes a request.
struct ScriptedStream {
    now: Rc<Cell<Instant>>,
    reads: VecDeque<ReadStep>,
    writes: VecDeque<WriteStep>,
    flushes: VecDeque<FlushStep>,
    ops: usize,
    offered: Vec<Vec<u8>>,
    accepted: Vec<u8>,
    armed_reads: Vec<Duration>,
    armed_writes: Vec<Duration>,
}

impl ScriptedStream {
    fn new(now: Rc<Cell<Instant>>) -> Self {
        Self {
            now,
            reads: VecDeque::new(),
            writes: VecDeque::new(),
            flushes: VecDeque::new(),
            ops: 0,
            offered: Vec::new(),
            accepted: Vec::new(),
            armed_reads: Vec::new(),
            armed_writes: Vec::new(),
        }
    }

    fn advance(&self, advance: Duration) {
        self.now.set(self.now.get() + advance);
    }
}

impl Read for ScriptedStream {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.ops += 1;
        match self.reads.pop_front().expect("scripted read step missing") {
            ReadStep::Deliver(advance, bytes) => {
                self.advance(advance);
                buffer[..bytes.len()].copy_from_slice(bytes);
                Ok(bytes.len())
            }
            ReadStep::FailAfter(advance, kind) => {
                self.advance(advance);
                Err(io::Error::new(kind, "scripted failure"))
            }
        }
    }
}

impl Write for ScriptedStream {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.ops += 1;
        self.offered.push(buffer.to_vec());
        match self
            .writes
            .pop_front()
            .expect("scripted write step missing")
        {
            WriteStep::Deliver(advance, limit) => {
                self.advance(advance);
                let accepted = limit.min(buffer.len());
                self.accepted.extend_from_slice(&buffer[..accepted]);
                Ok(accepted)
            }
            WriteStep::FailAfter(advance, kind) => {
                self.advance(advance);
                Err(io::Error::new(kind, "scripted failure"))
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.ops += 1;
        match self
            .flushes
            .pop_front()
            .expect("scripted flush step missing")
        {
            FlushStep::Deliver(advance) => {
                self.advance(advance);
                Ok(())
            }
            FlushStep::FailAfter(advance, kind) => {
                self.advance(advance);
                Err(io::Error::new(kind, "scripted failure"))
            }
        }
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
const STEP_ADVANCE: Duration = Duration::from_millis(40);

/// Shared scripted clock: the cell the seam advances and the `Instant` source the
/// wrapper under test reads — one time base, fully deterministic.
fn scripted_clock() -> (Rc<Cell<Instant>>, Instant, Instant) {
    let base = Instant::now();
    let now = Rc::new(Cell::new(base));
    (now.clone(), base, base + TEST_CAP)
}

fn clock_of(now: &Rc<Cell<Instant>>) -> Box<dyn Fn() -> Instant> {
    let now = now.clone();
    Box::new(move || now.get())
}

fn is_process_deadline(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::TimedOut
        && error.to_string().contains("process witness deadline")
}

/// An operation-level `Interrupted`/`WouldBlock` carries zero transferred bytes — the
/// error observed surfacing before the fixture cap in the first gate. The wrapper must
/// re-check the cap and keep waiting within what remains of it rather than surfacing the
/// early error; the armed bounds prove the cap is never renewed.
#[test]
fn early_retryable_errors_retry_inside_the_same_cap() -> TestResult {
    let (now, _base, cap) = scripted_clock();
    let mut stream = ScriptedStream::new(now.clone());
    stream.reads = VecDeque::from([
        ReadStep::FailAfter(Duration::ZERO, io::ErrorKind::Interrupted),
        ReadStep::FailAfter(STEP_ADVANCE, io::ErrorKind::WouldBlock),
        ReadStep::Deliver(Duration::ZERO, b"reply"),
    ]);
    let mut buffer = [0_u8; 8];
    let read = DeadlineStream::with_clock(&mut stream, cap, clock_of(&now))?.read(&mut buffer)?;
    assert_eq!(&buffer[..read], b"reply");
    assert_eq!(stream.ops, 3);
    assert_eq!(
        stream.armed_reads,
        [TEST_CAP, TEST_CAP, TEST_CAP - STEP_ADVANCE],
        "armed bounds must only shrink toward the one cap"
    );
    Ok(())
}

/// A stream that keeps failing before the cap must end in the absolute deadline marker
/// at exactly the original cap — deterministic because the scripted clock itself lands
/// on `cap` after five 40ms advances; no tolerance and no wall-clock assertion.
#[test]
fn repeated_early_would_block_terminates_at_the_original_cap() -> TestResult {
    let (now, base, cap) = scripted_clock();
    let mut stream = ScriptedStream::new(now.clone());
    stream.reads =
        VecDeque::from([ReadStep::FailAfter(STEP_ADVANCE, io::ErrorKind::WouldBlock); 32]);
    let mut buffer = [0_u8; 8];
    let error = DeadlineStream::with_clock(&mut stream, cap, clock_of(&now))?
        .read(&mut buffer)
        .expect_err("the wait must end in the absolute-deadline error");
    assert!(
        is_process_deadline(&error),
        "early socket expiries must end at the deadline marker: {error}"
    );
    assert_eq!(
        now.get(),
        base + TEST_CAP,
        "wait must end at the original cap"
    );
    assert_eq!(stream.ops, 5, "five 40ms advances reach the cap exactly");
    assert_eq!(
        stream.armed_reads,
        [
            TEST_CAP,
            TEST_CAP - STEP_ADVANCE,
            TEST_CAP - 2 * STEP_ADVANCE,
            TEST_CAP - 3 * STEP_ADVANCE,
            TEST_CAP - 4 * STEP_ADVANCE
        ],
        "each retry must arm only what remains of the one cap"
    );
    Ok(())
}

/// Partial progress followed by `WouldBlock`: the retried operation must resume exactly
/// where it stopped. `offered` proves each retry passes only the unsent suffix — no
/// whole-request reserialization — and `accepted` proves every byte lands exactly once.
/// The flush leg covers the third retry loop.
#[test]
fn partial_progress_then_would_block_transfers_each_byte_once() -> TestResult {
    let (now, _base, cap) = scripted_clock();
    let mut stream = ScriptedStream::new(now.clone());
    stream.writes = VecDeque::from([
        WriteStep::Deliver(STEP_ADVANCE, 4),
        WriteStep::FailAfter(Duration::ZERO, io::ErrorKind::WouldBlock),
        WriteStep::Deliver(Duration::ZERO, usize::MAX),
    ]);
    stream.reads = VecDeque::from([
        ReadStep::Deliver(Duration::ZERO, b"ab"),
        ReadStep::FailAfter(Duration::ZERO, io::ErrorKind::WouldBlock),
        ReadStep::Deliver(Duration::ZERO, b"cd"),
    ]);
    stream.flushes = VecDeque::from([
        FlushStep::FailAfter(Duration::ZERO, io::ErrorKind::WouldBlock),
        FlushStep::Deliver(Duration::ZERO),
    ]);
    {
        let mut stream = DeadlineStream::with_clock(&mut stream, cap, clock_of(&now))?;
        stream.write_all(b"0123456789")?;
        stream.flush()?;
        let mut buffer = [0_u8; 4];
        stream.read_exact(&mut buffer)?;
        assert_eq!(&buffer, b"abcd");
    }
    assert_eq!(stream.accepted, b"0123456789");
    assert_eq!(
        stream.offered,
        [
            b"0123456789".to_vec(),
            b"456789".to_vec(),
            b"456789".to_vec()
        ],
        "a retry must offer only the unsent suffix, never the whole request again"
    );
    assert_eq!(
        stream.armed_writes,
        [
            TEST_CAP,
            TEST_CAP - STEP_ADVANCE,
            TEST_CAP - STEP_ADVANCE,
            TEST_CAP - STEP_ADVANCE,
            TEST_CAP - STEP_ADVANCE
        ],
        "partial success and early errors must never renew the cap"
    );
    Ok(())
}

/// The boundary oracle: a positive result must be accepted only while `now < cap`.
/// Exactly-at-cap and beyond-cap completions are rejected with the deadline marker, per
/// direction (read/write/flush), with no retryable error obscuring the check — a missing
/// or weakened (`>` instead of `>=`) post-result check fails this table.
#[test]
fn positive_results_are_rejected_exactly_at_and_beyond_the_cap() -> TestResult {
    const JUST_BEFORE: Duration = Duration::from_nanos(199_999_999);
    const JUST_AFTER: Duration = Duration::from_nanos(200_000_001);
    for (offset, expect_ok) in [(JUST_BEFORE, true), (TEST_CAP, false), (JUST_AFTER, false)] {
        for direction in ["read", "write", "flush"] {
            let (now, _base, cap) = scripted_clock();
            let mut stream = ScriptedStream::new(now.clone());
            match direction {
                "read" => stream.reads.push_back(ReadStep::Deliver(offset, b"x")),
                "write" => stream
                    .writes
                    .push_back(WriteStep::Deliver(offset, usize::MAX)),
                _ => stream.flushes.push_back(FlushStep::Deliver(offset)),
            }
            let mut buffer = [0_u8; 4];
            let result = {
                let mut ds = DeadlineStream::with_clock(&mut stream, cap, clock_of(&now))?;
                match direction {
                    "read" => ds.read(&mut buffer).map(|_| ()),
                    "write" => ds.write(b"data").map(|_| ()),
                    _ => ds.flush(),
                }
            };
            match (result, expect_ok) {
                (Ok(()), true) => {
                    if direction == "read" {
                        assert_eq!(&buffer[..1], b"x");
                    }
                }
                (Err(error), false) => assert!(
                    is_process_deadline(&error),
                    "{direction} offset {offset:?}: expected deadline marker, got {error}"
                ),
                (outcome, _) => panic!(
                    "{direction} outcome {outcome:?} at offset {offset:?} violates the boundary"
                ),
            }
            assert_eq!(stream.ops, 1, "exactly one real IO at offset {offset:?}");
        }
    }
    Ok(())
}

/// Only genuinely retryable operation-level errors retry: an unrelated error before the
/// cap surfaces with its own kind, never converted to the deadline marker.
#[test]
fn unrelated_error_keeps_its_kind_before_the_cap() -> TestResult {
    let (now, _base, cap) = scripted_clock();
    let mut stream = ScriptedStream::new(now.clone());
    stream.reads = VecDeque::from([
        ReadStep::FailAfter(Duration::ZERO, io::ErrorKind::WouldBlock),
        ReadStep::FailAfter(Duration::ZERO, io::ErrorKind::PermissionDenied),
    ]);
    let mut buffer = [0_u8; 8];
    let error = DeadlineStream::with_clock(&mut stream, cap, clock_of(&now))?
        .read(&mut buffer)
        .expect_err("the unrelated error must surface unchanged");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied, "{error}");
    assert!(!is_process_deadline(&error), "{error}");
    assert_eq!(stream.ops, 2);
    Ok(())
}

/// Nothing may begin after the cap: construction against an expired cap fails before the
/// stream is touched, and a call whose pre-check already sees `now >= cap` neither arms
/// a timeout nor consumes a scripted operation.
#[test]
fn no_operation_begins_after_the_cap_has_passed() -> TestResult {
    let (now, base, cap) = scripted_clock();
    let mut stream = ScriptedStream::new(now.clone());
    stream.reads = VecDeque::from([
        ReadStep::Deliver(Duration::ZERO, b"ok"),
        ReadStep::Deliver(Duration::ZERO, b"never"),
    ]);
    {
        let mut ds = DeadlineStream::with_clock(&mut stream, cap, clock_of(&now))?;
        let mut buffer = [0_u8; 4];
        assert_eq!(ds.read(&mut buffer)?, 2);
        now.set(base + TEST_CAP);
        let error = ds
            .read(&mut buffer)
            .expect_err("a call after the cap must not reach the stream");
        assert!(is_process_deadline(&error), "{error}");
    }
    assert_eq!(
        stream.ops, 1,
        "the post-cap read must never start an operation"
    );
    assert_eq!(
        stream.reads.len(),
        1,
        "the second scripted step must be unconsumed"
    );
    assert_eq!(
        stream.armed_reads.len(),
        1,
        "no timeout may be armed after the cap"
    );

    now.set(base);
    let mut stream = ScriptedStream::new(now.clone());
    stream.reads = VecDeque::from([ReadStep::Deliver(Duration::ZERO, b"never")]);
    now.set(base + TEST_CAP);
    let error = DeadlineStream::with_clock(&mut stream, cap, clock_of(&now))
        .map(|_| ())
        .expect_err("construction against an expired cap must fail");
    assert!(is_process_deadline(&error), "{error}");
    assert_eq!(stream.ops, 0, "no operation may begin on an expired cap");
    assert_eq!(stream.reads.len(), 1);
    Ok(())
}
