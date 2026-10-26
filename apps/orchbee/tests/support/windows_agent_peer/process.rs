//! Full-duplex fixture exchange on one retained Windows Job owner thread.
use orchbee_local_ipc::{ChildInputWriter, ChildOutputReader};
use orchbee_local_runtime::CommandChild;
use std::{
    fmt,
    io::{self, Read, Write},
    process::{Command, ExitStatus, Output, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FailureKind {
    Cancelled,
    Deadline,
    OutputLimit,
    Io,
    Spawn,
    PhysicalUnknown,
}

#[derive(Debug)]
pub(crate) struct Failure {
    pub(crate) kind: FailureKind,
    pub(crate) status: Option<ExitStatus>,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
    detail: String,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?}: {}; {}",
            self.kind,
            self.detail,
            diagnostic(self.status, &self.stdout, &self.stderr)
        )
    }
}

pub(super) fn diagnostic(status: Option<ExitStatus>, stdout: &[u8], stderr: &[u8]) -> String {
    const DIAGNOSTIC_LIMIT: usize = 8192;
    format!(
        "status={status:?}\nstdout ({} bytes, first {DIAGNOSTIC_LIMIT} at most)={}\nstderr ({} bytes, first {DIAGNOSTIC_LIMIT} at most)={}",
        stdout.len(),
        String::from_utf8_lossy(&stdout[..stdout.len().min(DIAGNOSTIC_LIMIT)]),
        stderr.len(),
        String::from_utf8_lossy(&stderr[..stderr.len().min(DIAGNOSTIC_LIMIT)]),
    )
}

#[derive(Debug)]
struct Problem(FailureKind, String);
impl From<io::Error> for Problem {
    fn from(error: io::Error) -> Self {
        Self(FailureKind::Io, error.to_string())
    }
}

#[derive(Default)]
struct Capture {
    status: Option<ExitStatus>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

// The production owner kills on Drop without waiting. This synchronous fixture
// also attempts settlement if its launch callback or another invariant unwinds.
struct Child(CommandChild);
impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.0.settle();
    }
}

pub(super) fn run(
    command: Command,
    input: Option<&[u8]>,
    budget: Duration,
    output_limit: usize,
    cancelled: &AtomicBool,
    launched: impl FnOnce(Instant),
) -> Result<Output, Failure> {
    let mut capture = Capture::default();
    match exchange(
        command,
        input,
        budget,
        output_limit,
        cancelled,
        launched,
        &mut capture,
    ) {
        Ok(status) => Ok(Output {
            status,
            stdout: capture.stdout,
            stderr: capture.stderr,
        }),
        Err(Problem(kind, detail)) => Err(Failure {
            kind,
            detail,
            status: capture.status,
            stdout: capture.stdout,
            stderr: capture.stderr,
        }),
    }
}

fn exchange(
    mut command: Command,
    input: Option<&[u8]>,
    budget: Duration,
    output_limit: usize,
    cancelled: &AtomicBool,
    launched: impl FnOnce(Instant),
    capture: &mut Capture,
) -> Result<ExitStatus, Problem> {
    if cancelled.load(Ordering::Acquire) {
        return Err(Problem(
            FailureKind::Cancelled,
            "cancelled before spawn".into(),
        ));
    }
    let (out, stdout) = ChildOutputReader::pipe()?;
    let (err, stderr) = ChildOutputReader::pipe()?;
    let (mut writer, stdin) = match input {
        Some(_) => {
            let (writer, stdin) = ChildInputWriter::pipe()?;
            (Some(writer), stdin)
        }
        None => (None, Stdio::null()),
    };
    command.stdin(stdin).stdout(stdout).stderr(stderr);
    let mut child = Child(CommandChild::spawn(command).map_err(|failure| {
        Problem(
            if failure.created {
                FailureKind::PhysicalUnknown
            } else {
                FailureKind::Spawn
            },
            failure.error.to_string(),
        )
    })?);
    let started = Instant::now();
    launched(started);
    let deadline = started + budget;
    let mut out = Some(out);
    let mut err = Some(err);
    let mut remaining = input.unwrap_or_default();
    let mut room = output_limit;
    let execution = (|| {
        loop {
            if cancelled.load(Ordering::Acquire) {
                return Err(Problem(
                    FailureKind::Cancelled,
                    "peer cancelled the command".into(),
                ));
            }
            if Instant::now() >= deadline {
                return Err(Problem(
                    FailureKind::Deadline,
                    "fixture command deadline expired".into(),
                ));
            }
            // Fair finite turns keep a blocked stdin or a noisy stream from
            // hiding cancellation, the other output, or root exit.
            drain(&mut out, &mut capture.stdout, &mut room, 8)?;
            drain(&mut err, &mut capture.stderr, &mut room, 8)?;
            feed(&mut writer, &mut remaining)?;
            if let Some(status) = child.0.try_wait()? {
                return Ok(status);
            }
            thread::sleep(Duration::from_millis(5));
        }
    })();
    drop(writer);
    let mut problem = match execution {
        Ok(status) => {
            capture.status = Some(status);
            None
        }
        Err(problem) => {
            match child.0.settle() {
                Ok(status) => capture.status = Some(status),
                Err(error) => {
                    return Err(Problem(
                        FailureKind::PhysicalUnknown,
                        format!("{}; Job settlement: {error}", problem.1),
                    ));
                }
            }
            Some(problem)
        }
    };
    // The Job is settled and owns no writers. Each successful read consumes
    // at least one byte; room + 1 also detects overflow after an exact cap.
    // This admits large legal tails without a fixed small iteration ceiling.
    for (reader, kept) in [
        (&mut out, &mut capture.stdout),
        (&mut err, &mut capture.stderr),
    ] {
        let reads = room.saturating_add(1);
        let tail = drain(reader, kept, &mut room, reads).and_then(|complete| {
            if complete {
                Ok(())
            } else {
                Err(Problem(
                    FailureKind::Io,
                    "settled output did not finish within its finite read budget".into(),
                ))
            }
        });
        if let Err(tail) = tail {
            if let Some(first) = problem.as_mut() {
                first.1.push_str(&format!("; tail capture: {}", tail.1));
            } else {
                problem = Some(tail);
            }
        }
    }
    if let Some(problem) = problem {
        Err(problem)
    } else {
        Ok(capture
            .status
            .expect("known Job settlement precedes successful capture"))
    }
}

fn drain(
    reader: &mut Option<ChildOutputReader>,
    kept: &mut Vec<u8>,
    room: &mut usize,
    reads: usize,
) -> Result<bool, Problem> {
    let Some(pipe) = reader.as_mut() else {
        return Ok(true);
    };
    let mut bytes = [0; 4096];
    for _ in 0..reads {
        match pipe.read(&mut bytes) {
            Ok(0) => {
                *reader = None;
                return Ok(true);
            }
            Ok(count) => {
                let keep = count.min(*room);
                kept.extend_from_slice(&bytes[..keep]);
                *room -= keep;
                if keep < count {
                    return Err(Problem(
                        FailureKind::OutputLimit,
                        "combined fixture output limit exceeded".into(),
                    ));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(true),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(false)
}

fn feed(writer: &mut Option<ChildInputWriter>, remaining: &mut &[u8]) -> io::Result<()> {
    let Some(pipe) = writer.as_mut() else {
        return Ok(());
    };
    for _ in 0..8 {
        if remaining.is_empty() {
            *writer = None;
            return Ok(());
        }
        match pipe.write(&remaining[..remaining.len().min(32 * 1024)]) {
            Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
            Ok(count) => *remaining = &remaining[count..],
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) if error.kind() == io::ErrorKind::BrokenPipe => {
                *writer = None;
                return Ok(());
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}
