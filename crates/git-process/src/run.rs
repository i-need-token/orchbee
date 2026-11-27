//! One Git command run to its end under one process-tree owner, a process group on Unix and a Job
//! on Windows, with bounded output. Whatever ends a run early, the tree is settled before the run
//! returns, or the run reports that it could not prove it.
use orchbee_local_ipc::{AsLocalIo, ChildOutputReader, PollFd, PollFlags, PollTimeout, poll};
use orchbee_local_runtime::CommandChild;
use std::{
    io::{self, Read},
    mem,
    process::{Command, ExitStatus, Stdio},
    thread,
    time::Duration,
};

/// The longest a run waits for output before it looks at the root and its caller's check again.
const WAIT_SLICE: Duration = Duration::from_millis(10);
const READ_SIZE: usize = 16 * 1024;
/// Reads of one output per turn, so a busy output delays neither the other nor the caller's check.
const READS_PER_TURN: usize = 16;

/// What a run keeps of the command's output.
#[derive(Clone, Copy, Debug)]
pub struct OutputBounds {
    /// Standard output past this many bytes ends the run as [`RunError::TooLarge`].
    pub stdout: usize,
    /// Standard error keeps its last this many bytes, because Git writes its final error after any
    /// progress.
    pub stderr_tail: usize,
}

/// A command that ran to its end, with its whole tree settled.
#[derive(Debug)]
pub struct Output {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    /// The retained tail of standard error.
    pub stderr: Vec<u8>,
}

/// Why a run ended without an [`Output`]. Except after `Unsettled`, no process the run started is
/// left.
#[derive(Debug)]
pub enum RunError<E> {
    /// The output pipes could not be made, so no process was started.
    Pipe,
    /// No process was created; `NotFound` means the program does not exist.
    Spawn(io::ErrorKind),
    /// A process was created and its tree could not be proven settled.
    Unsettled,
    /// The caller's check ended the run.
    Stopped(E),
    /// Standard output passed its bound. Carries standard error's retained tail.
    TooLarge(Vec<u8>),
    /// Reading the output or observing the root failed. Carries standard error's retained tail.
    Io(Vec<u8>),
}

/// One Git command whose program, arguments, working directory and environment its consumer chose.
pub struct Run {
    command: Command,
    bounds: OutputBounds,
}

impl Run {
    pub fn new(command: Command, bounds: OutputBounds) -> Self {
        Self { command, bounds }
    }

    /// Starts the command, reading the null device, and drains both outputs until its tree settles.
    /// `check` runs before every turn and while the last output is drained; its error settles the
    /// tree and ends the run as [`RunError::Stopped`].
    pub fn wait<E>(self, mut check: impl FnMut() -> Result<(), E>) -> Result<Output, RunError<E>> {
        let Self {
            mut command,
            bounds,
        } = self;
        let (out, stdout) = ChildOutputReader::pipe().map_err(|_| RunError::Pipe)?;
        let (err, stderr) = ChildOutputReader::pipe().map_err(|_| RunError::Pipe)?;
        command.stdin(Stdio::null()).stdout(stdout).stderr(stderr);
        let mut child = CommandChild::spawn(command).map_err(|failure| {
            if failure.created {
                RunError::Unsettled
            } else {
                RunError::Spawn(failure.error.kind())
            }
        })?;
        let mut outputs = Outputs {
            out: Some(out),
            err: Some(err),
            stdout: Vec::new(),
            stderr: Vec::new(),
            buffer: vec![0; READ_SIZE],
            bounds,
        };
        loop {
            if let Err(reason) = check() {
                return Err(settled(&mut child, RunError::Stopped(reason)));
            }
            if let Err(failure) = outputs.drain(false) {
                let error = outputs.failed(failure);
                return Err(settled(&mut child, error));
            }
            match child.try_wait() {
                Ok(Some(status)) => return outputs.finish(status, &mut check),
                Ok(None) => outputs.wait(),
                Err(_) => {
                    let error = outputs.failed(Failure::Io);
                    return Err(settled(&mut child, error));
                }
            }
        }
    }
}

/// `error`, unless the tree cannot be proven settled.
fn settled<E>(child: &mut CommandChild, error: RunError<E>) -> RunError<E> {
    match child.settle() {
        Ok(_) => error,
        Err(_) => RunError::Unsettled,
    }
}

enum Failure {
    TooLarge,
    Io,
}

struct Outputs {
    out: Option<ChildOutputReader>,
    err: Option<ChildOutputReader>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    buffer: Vec<u8>,
    bounds: OutputBounds,
}

impl Outputs {
    /// Takes what both outputs hold now. Once the tree has settled, `settled` closes an output that
    /// holds nothing more, because no writer is left to end it.
    fn drain(&mut self, settled: bool) -> Result<(), Failure> {
        let OutputBounds {
            stdout: limit,
            stderr_tail: tail,
        } = self.bounds;
        let stdout = &mut self.stdout;
        drain(&mut self.out, &mut self.buffer, settled, |bytes| {
            if stdout.len() + bytes.len() > limit {
                return Err(Failure::TooLarge);
            }
            stdout.extend_from_slice(bytes);
            Ok(())
        })?;
        let stderr = &mut self.stderr;
        drain(&mut self.err, &mut self.buffer, settled, |bytes| {
            stderr.extend_from_slice(bytes);
            let excess = stderr.len().saturating_sub(tail);
            stderr.drain(..excess);
            Ok(())
        })
    }

    /// Waits until an open output has bytes or closes, at most one slice.
    fn wait(&self) {
        let mut sources = Vec::with_capacity(2);
        for reader in [&self.out, &self.err].into_iter().flatten() {
            sources.push(PollFd::new(reader.as_local_io(), PollFlags::POLLIN));
        }
        let polled = !sources.is_empty()
            && PollTimeout::try_from(WAIT_SLICE)
                .ok()
                .is_some_and(|timeout| poll(&mut sources, timeout).is_ok());
        if !polled {
            thread::sleep(WAIT_SLICE);
        }
    }

    /// Drains the finite tail a settled tree left behind; no owned descendant can still write.
    fn finish<E>(
        mut self,
        status: ExitStatus,
        check: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Output, RunError<E>> {
        while self.out.is_some() || self.err.is_some() {
            check().map_err(RunError::Stopped)?;
            if let Err(failure) = self.drain(true) {
                return Err(self.failed(failure));
            }
        }
        Ok(Output {
            status,
            stdout: self.stdout,
            stderr: self.stderr,
        })
    }

    fn failed<E>(&mut self, failure: Failure) -> RunError<E> {
        let stderr = mem::take(&mut self.stderr);
        match failure {
            Failure::TooLarge => RunError::TooLarge(stderr),
            Failure::Io => RunError::Io(stderr),
        }
    }
}

fn drain(
    reader: &mut Option<ChildOutputReader>,
    buffer: &mut [u8],
    settled: bool,
    mut keep: impl FnMut(&[u8]) -> Result<(), Failure>,
) -> Result<(), Failure> {
    let Some(pipe) = reader.as_mut() else {
        return Ok(());
    };
    for _ in 0..READS_PER_TURN {
        match pipe.read(buffer) {
            Ok(0) => {
                *reader = None;
                return Ok(());
            }
            Ok(count) => keep(buffer.get(..count).unwrap_or_default())?,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if settled {
                    *reader = None;
                }
                return Ok(());
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => return Err(Failure::Io),
        }
    }
    Ok(())
}
