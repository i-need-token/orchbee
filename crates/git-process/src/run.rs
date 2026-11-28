//! One Git command run to its end under one process-tree owner, a process group on Unix and a Job
//! on Windows, with its input fed and its output drained as they flow. Whatever ends a run early,
//! the tree is settled before the run returns, or the run reports that it could not prove it.
use orchbee_local_ipc::{
    AsLocalIo, ChildInputWriter, ChildOutputReader, PollFd, PollFlags, PollTimeout, poll,
};
use orchbee_local_runtime::CommandChild;
use std::{
    io::{self, Read, Write},
    marker::PhantomData,
    process::{Command, ExitStatus, Stdio},
    thread,
    time::Duration,
};

/// The longest a run waits for output before it looks at the root and its caller's check again.
const WAIT_SLICE: Duration = Duration::from_millis(10);
/// The wait while input remains, so a command that reads quickly is fed quickly.
const FEED_SLICE: Duration = Duration::from_millis(1);
const READ_SIZE: usize = 16 * 1024;
/// Reads of one output per turn, so a busy output delays neither the other nor the caller's check.
const READS_PER_TURN: usize = 16;

/// What [`Run::wait`] keeps of the command's output.
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

/// Why [`Run::wait`] ended without an [`Output`]. Except after `Unsettled`, no process the run
/// started is left.
#[derive(Debug)]
pub enum RunError<E> {
    /// The pipes could not be made, so no process was started.
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

/// The consumer of a run's output as it arrives, which may end the run.
pub trait Observer {
    /// Why the observer ended the run.
    type Stop;
    /// Runs before every turn and while the last output is drained.
    fn check(&mut self) -> Result<(), Self::Stop>;
    /// Takes the next bytes of standard output, which may end anywhere.
    fn stdout(&mut self, bytes: &[u8]) -> Result<(), Self::Stop>;
    /// Takes the next bytes of standard error, which may end anywhere.
    fn stderr(&mut self, bytes: &[u8]);
    /// Sees the command's root once it started, before the run reads anything; `child` owns the
    /// command's tree until the run returns.
    fn started(&mut self, _child: &CommandChild) {}
}

/// Why [`Run::observe`] ended before the command did. Except after `Unsettled`, no process the run
/// started is left.
#[derive(Debug)]
pub enum ObserveError<S> {
    /// The pipes could not be made, so no process was started.
    Pipe,
    /// No process was created; `NotFound` means the program does not exist.
    Spawn(io::ErrorKind),
    /// A process was created and its tree could not be proven settled.
    Unsettled,
    /// The observer ended the run.
    Stopped(S),
    /// Reading the output or observing the root failed.
    Io,
}

/// One Git command whose program, arguments, working directory and environment its consumer chose.
pub struct Run {
    command: Command,
    input: Vec<u8>,
}

impl Run {
    /// The command reads the null device unless it is given [`Self::input`].
    pub fn new(command: Command) -> Self {
        Self {
            command,
            input: Vec::new(),
        }
    }

    /// The bytes the command reads as its standard input, written as it takes them and then ended.
    /// A command that stops reading early leaves the rest unwritten.
    #[must_use]
    pub fn input(mut self, bytes: Vec<u8>) -> Self {
        self.input = bytes;
        self
    }

    /// Runs the command, keeping standard output up to `bounds` and the tail of standard error.
    /// `check` runs before every turn and while the last output is drained; its error settles the
    /// tree and ends the run as [`RunError::Stopped`].
    pub fn wait<E>(
        self,
        bounds: OutputBounds,
        check: impl FnMut() -> Result<(), E>,
    ) -> Result<Output, RunError<E>> {
        let mut kept = Kept {
            check,
            bounds,
            stdout: Vec::new(),
            stderr: Vec::new(),
            stop: PhantomData,
        };
        match self.observe(&mut kept) {
            Ok(status) => Ok(Output {
                status,
                stdout: kept.stdout,
                stderr: kept.stderr,
            }),
            Err(error) => Err(match error {
                ObserveError::Pipe => RunError::Pipe,
                ObserveError::Spawn(kind) => RunError::Spawn(kind),
                ObserveError::Unsettled => RunError::Unsettled,
                ObserveError::Stopped(KeptStop::Caller(reason)) => RunError::Stopped(reason),
                ObserveError::Stopped(KeptStop::TooLarge) => RunError::TooLarge(kept.stderr),
                ObserveError::Io => RunError::Io(kept.stderr),
            }),
        }
    }

    /// Runs the command, passing each output to `observer` as it arrives, until its tree settles.
    pub fn observe<O: Observer>(
        self,
        observer: &mut O,
    ) -> Result<ExitStatus, ObserveError<O::Stop>> {
        let Self { mut command, input } = self;
        let (out, stdout) = ChildOutputReader::pipe().map_err(|_| ObserveError::Pipe)?;
        let (err, stderr) = ChildOutputReader::pipe().map_err(|_| ObserveError::Pipe)?;
        let mut feed = if input.is_empty() {
            command.stdin(Stdio::null());
            None
        } else {
            let (writer, stdin) = ChildInputWriter::pipe().map_err(|_| ObserveError::Pipe)?;
            command.stdin(stdin);
            Some(Feed {
                writer,
                input,
                written: 0,
            })
        };
        command.stdout(stdout).stderr(stderr);
        let mut child = CommandChild::spawn(command).map_err(|failure| {
            if failure.created {
                ObserveError::Unsettled
            } else {
                ObserveError::Spawn(failure.error.kind())
            }
        })?;
        observer.started(&child);
        let mut outputs = Outputs {
            out: Some(out),
            err: Some(err),
            buffer: vec![0; READ_SIZE],
        };
        loop {
            if let Err(reason) = observer.check() {
                return Err(settled(&mut child, ObserveError::Stopped(reason)));
            }
            if feed.as_mut().is_some_and(Feed::push) {
                feed = None;
            }
            if let Err(failure) = outputs.drain(false, observer) {
                return Err(settled(&mut child, failure));
            }
            match child.try_wait() {
                Ok(Some(status)) => {
                    drop(feed);
                    return outputs.finish(status, observer);
                }
                Ok(None) => outputs.wait(if feed.is_some() {
                    FEED_SLICE
                } else {
                    WAIT_SLICE
                }),
                Err(_) => return Err(settled(&mut child, ObserveError::Io)),
            }
        }
    }
}

/// `error`, unless the tree cannot be proven settled.
fn settled<S>(child: &mut CommandChild, error: ObserveError<S>) -> ObserveError<S> {
    match child.settle() {
        Ok(_) => error,
        Err(_) => ObserveError::Unsettled,
    }
}

/// The input not yet written.
struct Feed {
    writer: ChildInputWriter,
    input: Vec<u8>,
    written: usize,
}

impl Feed {
    /// Writes what the pipe takes now; true once everything is written or the reader is gone.
    fn push(&mut self) -> bool {
        while let Some(rest) = self
            .input
            .get(self.written..)
            .filter(|rest| !rest.is_empty())
        {
            match self.writer.write(rest) {
                Ok(0) => return true,
                Ok(count) => self.written += count,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return false,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                // The command closed its input; what it did not read stays unwritten.
                Err(_) => return true,
            }
        }
        true
    }
}

struct Outputs {
    out: Option<ChildOutputReader>,
    err: Option<ChildOutputReader>,
    buffer: Vec<u8>,
}

impl Outputs {
    /// Takes what both outputs hold now. Once the tree has settled, `settled` closes an output that
    /// holds nothing more, because no writer is left to end it.
    fn drain<O: Observer>(
        &mut self,
        settled: bool,
        observer: &mut O,
    ) -> Result<(), ObserveError<O::Stop>> {
        drain(&mut self.out, &mut self.buffer, settled, |bytes| {
            observer.stdout(bytes).map_err(ObserveError::Stopped)
        })?;
        drain(&mut self.err, &mut self.buffer, settled, |bytes| {
            observer.stderr(bytes);
            Ok(())
        })
    }

    /// Waits until an open output has bytes or closes, at most `slice`.
    fn wait(&self, slice: Duration) {
        let mut sources = Vec::with_capacity(2);
        for reader in [&self.out, &self.err].into_iter().flatten() {
            sources.push(PollFd::new(reader.as_local_io(), PollFlags::POLLIN));
        }
        let polled = !sources.is_empty()
            && PollTimeout::try_from(slice)
                .ok()
                .is_some_and(|timeout| poll(&mut sources, timeout).is_ok());
        if !polled {
            thread::sleep(slice);
        }
    }

    /// Drains the finite tail a settled tree left behind; no owned descendant can still write.
    fn finish<O: Observer>(
        mut self,
        status: ExitStatus,
        observer: &mut O,
    ) -> Result<ExitStatus, ObserveError<O::Stop>> {
        while self.out.is_some() || self.err.is_some() {
            observer.check().map_err(ObserveError::Stopped)?;
            self.drain(true, observer)?;
        }
        Ok(status)
    }
}

fn drain<S>(
    reader: &mut Option<ChildOutputReader>,
    buffer: &mut [u8],
    settled: bool,
    mut keep: impl FnMut(&[u8]) -> Result<(), ObserveError<S>>,
) -> Result<(), ObserveError<S>> {
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
            Err(_) => return Err(ObserveError::Io),
        }
    }
    Ok(())
}

/// The observer behind [`Run::wait`].
struct Kept<F, E> {
    check: F,
    bounds: OutputBounds,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    stop: PhantomData<fn() -> E>,
}

enum KeptStop<E> {
    Caller(E),
    TooLarge,
}

impl<F: FnMut() -> Result<(), E>, E> Observer for Kept<F, E> {
    type Stop = KeptStop<E>;

    fn check(&mut self) -> Result<(), Self::Stop> {
        (self.check)().map_err(KeptStop::Caller)
    }

    fn stdout(&mut self, bytes: &[u8]) -> Result<(), Self::Stop> {
        if self.stdout.len() + bytes.len() > self.bounds.stdout {
            return Err(KeptStop::TooLarge);
        }
        self.stdout.extend_from_slice(bytes);
        Ok(())
    }

    fn stderr(&mut self, bytes: &[u8]) {
        self.stderr.extend_from_slice(bytes);
        let excess = self.stderr.len().saturating_sub(self.bounds.stderr_tail);
        self.stderr.drain(..excess);
    }
}

#[cfg(test)]
mod tests;
