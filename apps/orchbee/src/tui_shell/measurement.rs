//! Opt-in, bounded observations of the existing TUI loop and writer. Samples carry timing and
//! counts, never terminal content, and cannot authorize input, acknowledge a frame, or schedule work.

use std::{
    cell::RefCell,
    ffi::OsString,
    fmt,
    fs::{File, OpenOptions},
    io::{self, IoSlice, Write},
    path::Path,
    rc::Rc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use crate::localization::Locale;

const SAMPLE_LIMIT: usize = 65_536;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Batch(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReadResult {
    Ready,
    Idle,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WriteCall {
    Write,
    WriteAll,
    Vectored,
}

#[derive(Debug)]
enum Event {
    InputRead {
        batch: Option<Batch>,
        bytes: usize,
        result: ReadResult,
    },
    Dispatch {
        batch: Option<Batch>,
        succeeded: bool,
    },
    Control {
        succeeded: bool,
    },
    Frame {
        id: u64,
        last_input_batch: Option<Batch>,
        buffer_started_ns: Option<u64>,
        buffer_prepared_ns: Option<u64>,
        succeeded: bool,
    },
    Write {
        call: WriteCall,
        requested: Option<usize>,
        accepted: Option<usize>,
        succeeded: bool,
    },
    Flush {
        succeeded: bool,
    },
}

#[derive(Debug)]
struct Sample {
    started_ns: u64,
    ended_ns: u64,
    event: Event,
}

struct Journal {
    samples: Vec<Sample>,
    limit: usize,
    dropped: u64,
    abandoned: u64,
    outstanding: usize,
    failed: u64,
    invalid: bool,
    batches: u64,
    frames: u64,
    last_input_batch: Option<Batch>,
}

impl Journal {
    fn new(limit: usize) -> io::Result<Self> {
        let mut samples = Vec::new();
        samples.try_reserve_exact(limit).map_err(io::Error::other)?;
        Ok(Self {
            samples,
            limit,
            dropped: 0,
            abandoned: 0,
            outstanding: 0,
            failed: 0,
            invalid: false,
            batches: 0,
            frames: 0,
            last_input_batch: None,
        })
    }

    fn drop_sample(&mut self) {
        match self.dropped.checked_add(1) {
            Some(dropped) => self.dropped = dropped,
            None => self.invalid = true,
        }
    }

    fn has_capacity(&mut self) -> bool {
        if self.samples.len() < self.limit {
            true
        } else {
            self.drop_sample();
            false
        }
    }

    fn push(&mut self, sample: Sample) {
        if sample.event.failed() {
            match self.failed.checked_add(1) {
                Some(failed) => self.failed = failed,
                None => self.invalid = true,
            }
        }
        if self.has_capacity() {
            self.samples.push(sample);
        }
    }

    fn next_batch(&mut self) -> Option<Batch> {
        match self.batches.checked_add(1) {
            Some(id) => {
                self.batches = id;
                let batch = Batch(id);
                self.last_input_batch = Some(batch);
                Some(batch)
            }
            None => {
                self.invalid = true;
                None
            }
        }
    }

    fn next_frame(&mut self) -> Option<u64> {
        match self.frames.checked_add(1) {
            Some(id) => {
                self.frames = id;
                Some(id)
            }
            None => {
                self.invalid = true;
                None
            }
        }
    }
}

struct Active {
    origin: Instant,
    utc_alignment: Option<UtcAlignment>,
    journal: RefCell<Journal>,
}

struct UtcAlignment {
    unix_ns: String,
    monotonic_after_ns: u64,
}

impl Active {
    fn offset(&self, instant: Instant) -> Option<u64> {
        let offset = instant
            .checked_duration_since(self.origin)
            .and_then(|duration| u64::try_from(duration.as_nanos()).ok());
        if offset.is_none() {
            self.journal.borrow_mut().invalid = true;
        }
        offset
    }
}

/// A disabled observer is an inline `None`: its hot paths neither allocate nor sample time.
#[derive(Clone, Default)]
pub(super) struct Measurements(Option<Rc<Active>>);

impl Measurements {
    pub(super) fn start(&self) -> Option<Started> {
        self.start_with(Instant::now)
    }

    fn start_with(&self, now: impl FnOnce() -> Instant) -> Option<Started> {
        let active = self.0.as_ref()?;
        if !active.journal.borrow_mut().has_capacity() {
            return None;
        }
        {
            let mut journal = active.journal.borrow_mut();
            let Some(outstanding) = journal.outstanding.checked_add(1) else {
                journal.invalid = true;
                journal.drop_sample();
                return None;
            };
            journal.outstanding = outstanding;
        }
        Some(Started {
            active: Rc::clone(active),
            started: Some(now()),
        })
    }

    pub(super) fn frame(&self) -> Option<Frame> {
        let started = self.start()?;
        let (id, last_input_batch) = {
            let mut journal = started.active.journal.borrow_mut();
            (journal.next_frame()?, journal.last_input_batch)
        };
        Some(Frame {
            started,
            id,
            last_input_batch,
            buffer_started_ns: None,
            buffer_prepared_ns: None,
        })
    }
}

pub(super) struct Started {
    active: Rc<Active>,
    started: Option<Instant>,
}

impl Started {
    fn finish(self, event: Event) {
        self.finish_at(event, Instant::now());
    }

    fn finish_at(mut self, event: Event, ended: Instant) {
        let Some(started) = self.started.take() else {
            return;
        };
        self.active.journal.borrow_mut().outstanding -= 1;
        let Some(started_ns) = self.active.offset(started) else {
            self.active.journal.borrow_mut().drop_sample();
            return;
        };
        let Some(ended_ns) = self.active.offset(ended).filter(|end| *end >= started_ns) else {
            let mut journal = self.active.journal.borrow_mut();
            journal.invalid = true;
            journal.drop_sample();
            return;
        };
        self.active.journal.borrow_mut().push(Sample {
            started_ns,
            ended_ns,
            event,
        });
    }

    pub(super) fn input_completed(self, input: &io::Result<Option<Vec<u8>>>) -> Option<Batch> {
        let ended = Instant::now();
        let (batch, bytes, result) = match input {
            Ok(Some(bytes)) => (
                self.active.journal.borrow_mut().next_batch(),
                bytes.len(),
                ReadResult::Ready,
            ),
            Ok(None) => (None, 0, ReadResult::Idle),
            Err(_) => (None, 0, ReadResult::Failed),
        };
        self.finish_at(
            Event::InputRead {
                batch,
                bytes,
                result,
            },
            ended,
        );
        batch
    }

    pub(super) fn dispatch_completed(self, batch: Option<Batch>, succeeded: bool) {
        self.finish(Event::Dispatch { batch, succeeded });
    }

    pub(super) fn control_completed(self, succeeded: bool) {
        self.finish(Event::Control { succeeded });
    }
}

impl Drop for Started {
    fn drop(&mut self) {
        if self.started.is_some() {
            let mut journal = self.active.journal.borrow_mut();
            // Each live ticket incremented this count before it could escape `start_with`.
            journal.outstanding -= 1;
            match journal.abandoned.checked_add(1) {
                Some(abandoned) => journal.abandoned = abandoned,
                None => journal.invalid = true,
            }
        }
    }
}

pub(super) struct Frame {
    started: Started,
    id: u64,
    last_input_batch: Option<Batch>,
    buffer_started_ns: Option<u64>,
    buffer_prepared_ns: Option<u64>,
}

impl Frame {
    pub(super) fn buffer_started(&mut self) {
        self.buffer_started_ns = self.started.active.offset(Instant::now());
    }

    pub(super) fn buffer_prepared(&mut self) {
        self.buffer_prepared_ns = self.started.active.offset(Instant::now());
    }

    pub(super) fn finish(self, succeeded: bool) {
        if succeeded
            && !matches!(
                (self.buffer_started_ns, self.buffer_prepared_ns),
                (Some(started), Some(prepared)) if started <= prepared
            )
        {
            self.started.active.journal.borrow_mut().invalid = true;
        }
        self.started.finish(Event::Frame {
            id: self.id,
            last_input_batch: self.last_input_batch,
            buffer_started_ns: self.buffer_started_ns,
            buffer_prepared_ns: self.buffer_prepared_ns,
            succeeded,
        });
    }
}

pub(super) struct MeasuredWriter<W> {
    inner: W,
    measurements: Measurements,
}

impl<W> MeasuredWriter<W> {
    pub(super) fn new(inner: W, measurements: Measurements) -> Self {
        Self {
            inner,
            measurements,
        }
    }
}

impl<W: Write> Write for MeasuredWriter<W> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let started = self.measurements.start();
        let result = self.inner.write(buffer);
        if let Some(started) = started {
            started.finish(Event::Write {
                call: WriteCall::Write,
                requested: Some(buffer.len()),
                accepted: result.as_ref().ok().copied(),
                succeeded: result.is_ok(),
            });
        }
        result
    }

    fn write_all(&mut self, buffer: &[u8]) -> io::Result<()> {
        let started = self.measurements.start();
        let result = self.inner.write_all(buffer);
        if let Some(started) = started {
            started.finish(Event::Write {
                call: WriteCall::WriteAll,
                requested: Some(buffer.len()),
                accepted: result.is_ok().then_some(buffer.len()),
                succeeded: result.is_ok(),
            });
        }
        result
    }

    fn write_vectored(&mut self, buffers: &[IoSlice<'_>]) -> io::Result<usize> {
        let started = self.measurements.start();
        let result = self.inner.write_vectored(buffers);
        if let Some(started) = started {
            let requested = buffers
                .iter()
                .try_fold(0_usize, |sum, buffer| sum.checked_add(buffer.len()));
            if requested.is_none() {
                started.active.journal.borrow_mut().invalid = true;
            }
            started.finish(Event::Write {
                call: WriteCall::Vectored,
                requested,
                accepted: result.as_ref().ok().copied(),
                succeeded: result.is_ok(),
            });
        }
        result
    }

    fn flush(&mut self) -> io::Result<()> {
        let started = self.measurements.start();
        let result = self.inner.flush();
        if let Some(started) = started {
            started.finish(Event::Flush {
                succeeded: result.is_ok(),
            });
        }
        result
    }
}

struct Output {
    file: File,
    active: Rc<Active>,
}

#[derive(Default)]
pub(super) struct Session(Option<Output>);

impl Session {
    pub(super) fn open(value: Option<OsString>, locale: Locale) -> io::Result<Self> {
        let Some(path) = value else {
            return Ok(Self::default());
        };
        let path = Path::new(&path);
        if !path.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                locale.tui_measurement_path_invalid(),
            ));
        }
        let journal = Journal::new(SAMPLE_LIMIT)?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(path)?;
        let origin = Instant::now();
        let unix_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .map(|duration| duration.as_nanos().to_string());
        let aligned = Instant::now()
            .checked_duration_since(origin)
            .and_then(|duration| u64::try_from(duration.as_nanos()).ok());
        let utc_alignment =
            unix_ns
                .zip(aligned)
                .map(|(unix_ns, monotonic_after_ns)| UtcAlignment {
                    unix_ns,
                    monotonic_after_ns,
                });
        Ok(Self(Some(Output {
            file,
            active: Rc::new(Active {
                origin,
                utc_alignment,
                journal: RefCell::new(journal),
            }),
        })))
    }

    pub(super) fn measurements(&self) -> Measurements {
        Measurements(self.0.as_ref().map(|output| Rc::clone(&output.active)))
    }

    fn finish(self, runtime_succeeded: bool, terminal_restored: bool) -> io::Result<()> {
        let Some(mut output) = self.0 else {
            return Ok(());
        };
        write_journal(
            &mut output.file,
            &output.active,
            runtime_succeeded,
            terminal_restored,
        )?;
        output.file.flush()
    }

    /// Restore even on a runtime fault; diagnostic I/O cannot delay or replace that obligation.
    pub(super) fn finish_after_restore(
        self,
        runtime: io::Result<()>,
        restore: impl FnOnce() -> io::Result<()>,
        locale: Locale,
    ) -> io::Result<()> {
        finish_run(
            runtime,
            restore,
            |ran, restored| self.finish(ran, restored),
            locale,
        )
    }
}

fn finish_run(
    runtime: io::Result<()>,
    restore: impl FnOnce() -> io::Result<()>,
    persist: impl FnOnce(bool, bool) -> io::Result<()>,
    locale: Locale,
) -> io::Result<()> {
    let restoration = restore();
    let diagnostic = persist(runtime.is_ok(), restoration.is_ok());
    let primary = match (runtime, restoration) {
        (Err(runtime), Err(restoration)) => Err(io::Error::new(
            runtime.kind(),
            CompletionFailure {
                primary: runtime,
                secondary: restoration,
                context: locale.tui_restoration_also_failed(),
            },
        )),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    };
    match (primary, diagnostic) {
        (Err(primary), Err(diagnostic)) => Err(io::Error::new(
            primary.kind(),
            CompletionFailure {
                primary,
                secondary: diagnostic,
                context: locale.tui_measurement_also_failed(),
            },
        )),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
}

#[derive(Debug)]
struct CompletionFailure {
    primary: io::Error,
    secondary: io::Error,
    context: &'static str,
}

impl fmt::Display for CompletionFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}; {}: {}",
            self.primary, self.context, self.secondary
        )
    }
}

impl std::error::Error for CompletionFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.primary)
    }
}

fn write_journal(
    writer: &mut impl Write,
    active: &Active,
    runtime_succeeded: bool,
    terminal_restored: bool,
) -> io::Result<()> {
    let journal = active.journal.borrow();
    let samples: Vec<_> = journal.samples.iter().map(Sample::json).collect();
    let utc_alignment = active.utc_alignment.as_ref().map(|alignment| {
        serde_json::json!({
            "unix_ns": alignment.unix_ns,
            "monotonic_before_ns": 0,
            "monotonic_after_ns": alignment.monotonic_after_ns,
        })
    });
    let value = serde_json::json!({
        "schema": "orchbee.tui.measurements",
        "version": 1,
        "process_id": std::process::id(),
        "utc_alignment": utc_alignment,
        "runtime_succeeded": runtime_succeeded,
        "terminal_restored": terminal_restored,
        "sample_limit": journal.limit,
        "dropped_samples": journal.dropped,
        "abandoned_samples": journal.abandoned,
        "outstanding_samples": journal.outstanding,
        "failed_samples": journal.failed,
        "invalid_clock_or_counts": journal.invalid,
        "complete": runtime_succeeded && terminal_restored && journal.dropped == 0
            && journal.abandoned == 0 && journal.outstanding == 0 && journal.failed == 0
            && !journal.invalid,
        "samples": samples,
    });
    drop(journal);
    serde_json::to_writer(writer, &value).map_err(io::Error::other)
}

impl Event {
    fn failed(&self) -> bool {
        match self {
            Self::InputRead { result, .. } => *result == ReadResult::Failed,
            Self::Dispatch { succeeded, .. }
            | Self::Control { succeeded }
            | Self::Frame { succeeded, .. }
            | Self::Write { succeeded, .. }
            | Self::Flush { succeeded } => !succeeded,
        }
    }
}

impl Sample {
    fn json(&self) -> serde_json::Value {
        let event = match &self.event {
            Event::InputRead {
                batch,
                bytes,
                result,
            } => serde_json::json!({
                "kind": "input_read",
                "batch": batch.map(|batch| batch.0),
                "bytes": bytes,
                "result": match result {
                    ReadResult::Ready => "ready",
                    ReadResult::Idle => "idle",
                    ReadResult::Failed => "failed",
                },
            }),
            Event::Dispatch { batch, succeeded } => serde_json::json!({
                "kind": "dispatch", "batch": batch.map(|batch| batch.0), "succeeded": succeeded,
            }),
            Event::Control { succeeded } => serde_json::json!({
                "kind": "terminal_control", "succeeded": succeeded,
            }),
            Event::Frame {
                id,
                last_input_batch,
                buffer_started_ns,
                buffer_prepared_ns,
                succeeded,
            } => serde_json::json!({
                "kind": "frame", "id": id,
                "last_input_batch": last_input_batch.map(|batch| batch.0),
                "buffer_started_ns": buffer_started_ns,
                "buffer_prepared_ns": buffer_prepared_ns,
                "succeeded": succeeded,
            }),
            Event::Write {
                call,
                requested,
                accepted,
                succeeded,
            } => serde_json::json!({
                "kind": "writer_call",
                "call": match call {
                    WriteCall::Write => "write",
                    WriteCall::WriteAll => "write_all",
                    WriteCall::Vectored => "write_vectored",
                },
                "requested_bytes": requested, "accepted_bytes": accepted, "succeeded": succeeded,
            }),
            Event::Flush { succeeded } => serde_json::json!({
                "kind": "flush", "succeeded": succeeded,
            }),
        };
        serde_json::json!({ "started_ns": self.started_ns, "ended_ns": self.ended_ns, "event": event })
    }
}

#[cfg(test)]
mod tests;
