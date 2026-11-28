//! One Git command run under its deadline, its cancellation and, for a command that talks to a
//! remote, a watchdog that fails it once it has written nothing for five minutes while no prompt
//! waits on the user. Every run leaves its command line and standard error in the Workspace's Git
//! Output; neither reaches the diagnostic log.

use super::{
    errors::{Classifier, GitError, GitErrorCode},
    output::GitOutput,
};
use orchbee_files_protocol::ErrorCode;
use orchbee_git_process::{ObserveError, Observer, Run};
use orchbee_local_runtime::{LogLevel, LogValue, log_event};
use std::{
    io,
    process::{Command, ExitStatus},
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

/// Standard output past this ends the command as `git-failed` with `output-too-large`.
const STDOUT_LIMIT: usize = 16 * 1024 * 1024;
/// What a run keeps of standard error: enough for the 4 KiB detail and for Git Output.
const STDERR_TAIL: usize = 64 * 1024;

/// What may stop a run besides its own end.
#[derive(Clone, Debug)]
pub(super) struct Limits {
    /// The kind of command, for the diagnostic log's timeout record.
    pub(super) kind: &'static str,
    pub(super) deadline: Duration,
    /// Fails a command that writes nothing for this long while no prompt waits.
    pub(super) idle: Option<Duration>,
    pub(super) cancel: Arc<AtomicBool>,
    /// Prompts this run's helpers have open, during which the watchdog waits.
    pub(super) prompts: Option<Arc<AtomicUsize>>,
}

impl Limits {
    pub(super) fn new(kind: &'static str, deadline: Duration, cancel: Arc<AtomicBool>) -> Self {
        Self {
            kind,
            deadline,
            idle: None,
            cancel,
            prompts: None,
        }
    }
}

/// A command that ran to its end, or that its consumer stopped once it had read enough.
#[derive(Debug)]
pub(super) struct Ran {
    /// `None` when the consumer stopped the command.
    pub(super) status: Option<ExitStatus>,
    pub(super) stdout: Vec<u8>,
    /// The retained tail of standard error.
    pub(super) stderr: Vec<u8>,
}

impl Ran {
    /// The run, unless Git reported failure, which `classifier` reads.
    pub(super) fn checked(self, classifier: Classifier) -> Result<Self, GitError> {
        if self.status.is_none_or(|status| status.success()) {
            Ok(self)
        } else {
            Err(GitError::failed(classifier, &self.stdout, &self.stderr))
        }
    }

    /// Whether Git exited with `code`.
    pub(super) fn exited_with(&self, code: i32) -> bool {
        self.status.and_then(|status| status.code()) == Some(code)
    }
}

/// What a streaming consumer decides after each piece of standard output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Flow {
    More,
    /// The consumer has what it wants: the command is stopped and the run ends without a status.
    Enough,
    /// The output passed its bound: `git-failed` with `output-too-large`.
    TooLarge,
    /// The output cannot be read: `failed`.
    Malformed,
}

/// Runs `command`, keeping at most 16 MiB of standard output.
pub(super) fn run(
    command: Command,
    input: Vec<u8>,
    limits: &Limits,
    output: &Mutex<GitOutput>,
) -> Result<Ran, GitError> {
    let mut kept = Vec::new();
    let mut ran = stream(command, input, limits, output, |bytes| {
        if kept.len() + bytes.len() > STDOUT_LIMIT {
            return Flow::TooLarge;
        }
        kept.extend_from_slice(bytes);
        Flow::More
    })?;
    ran.stdout = kept;
    Ok(ran)
}

/// Runs `command`, handing standard output to `consume` as it arrives; the result keeps none.
pub(super) fn stream(
    command: Command,
    input: Vec<u8>,
    limits: &Limits,
    output: &Mutex<GitOutput>,
    consume: impl FnMut(&[u8]) -> Flow,
) -> Result<Ran, GitError> {
    let arguments = command
        .get_args()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    let started = Instant::now();
    let mut watch = Watch {
        limits,
        deadline: started + limits.deadline,
        last_output: started,
        consume,
        stderr: Vec::new(),
        #[cfg(unix)]
        reported: None,
    };
    let result = Run::new(command).input(input).observe(&mut watch);
    let stderr = watch.stderr;
    let cancelled = matches!(result, Err(ObserveError::Stopped(Stop::Cancelled)));
    record(output, &arguments, started.elapsed(), cancelled, &stderr);
    let stop = match result {
        Ok(status) => {
            return Ok(Ran {
                status: Some(status),
                stdout: Vec::new(),
                stderr,
            });
        }
        Err(ObserveError::Stopped(stop)) => stop,
        Err(ObserveError::Spawn(io::ErrorKind::NotFound)) => {
            return Err(GitError::unavailable("git-missing"));
        }
        Err(ObserveError::Spawn(kind)) => {
            log_event(
                LogLevel::Warning,
                "git-spawn-failed",
                &[
                    ("kind", LogValue::Token(limits.kind)),
                    ("error", LogValue::from(kind)),
                ],
            );
            return Err(GitError::new(ErrorCode::Failed));
        }
        Err(ObserveError::Pipe | ObserveError::Unsettled | ObserveError::Io) => {
            return Err(GitError::new(ErrorCode::Failed));
        }
    };
    match stop {
        Stop::Enough => Ok(Ran {
            status: None,
            stdout: Vec::new(),
            stderr,
        }),
        Stop::Cancelled => Err(GitError::new(ErrorCode::Cancelled)),
        Stop::Timeout => {
            log_event(
                LogLevel::Warning,
                "git-timeout",
                &[("kind", LogValue::Token(limits.kind))],
            );
            Err(GitError::new(ErrorCode::Timeout))
        }
        Stop::TooLarge => Err(GitError::git(GitErrorCode::OutputTooLarge)),
        Stop::Malformed => Err(GitError::new(ErrorCode::Failed)),
    }
}

/// Appends the command line and what Git wrote to standard error, keeping only the last state of
/// each progress line Git redraws with carriage returns.
fn record(
    output: &Mutex<GitOutput>,
    arguments: &[String],
    took: Duration,
    cancelled: bool,
    stderr: &[u8],
) {
    let mut output = output.lock().unwrap_or_else(PoisonError::into_inner);
    output.command(arguments.iter().map(String::as_str), took, cancelled);
    for line in stderr.split(|byte| *byte == b'\n') {
        let last = line
            .rsplit(|byte| *byte == b'\r')
            .find(|part| !part.is_empty())
            .unwrap_or_default();
        if !last.is_empty() {
            output.write(last);
            output.write(b"\n");
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Stop {
    Cancelled,
    Timeout,
    TooLarge,
    Enough,
    Malformed,
}

struct Watch<'a, F> {
    limits: &'a Limits,
    deadline: Instant,
    last_output: Instant,
    consume: F,
    stderr: Vec<u8>,
    /// The Worker hears of the command's group until the run settled it.
    #[cfg(unix)]
    reported: Option<orchbee_local_runtime::ReportedGroup<'static>>,
}

impl<F: FnMut(&[u8]) -> Flow> Observer for Watch<'_, F> {
    type Stop = Stop;

    fn check(&mut self) -> Result<(), Stop> {
        let now = Instant::now();
        if self.limits.cancel.load(Ordering::Acquire) {
            return Err(Stop::Cancelled);
        }
        if now >= self.deadline {
            return Err(Stop::Timeout);
        }
        let prompting = self
            .limits
            .prompts
            .as_ref()
            .is_some_and(|prompts| prompts.load(Ordering::Acquire) > 0);
        if prompting {
            self.last_output = now;
        }
        match self.limits.idle {
            Some(idle) if now.saturating_duration_since(self.last_output) >= idle => {
                Err(Stop::Timeout)
            }
            _ => Ok(()),
        }
    }

    fn stdout(&mut self, bytes: &[u8]) -> Result<(), Stop> {
        self.last_output = Instant::now();
        match (self.consume)(bytes) {
            Flow::More => Ok(()),
            Flow::Enough => Err(Stop::Enough),
            Flow::TooLarge => Err(Stop::TooLarge),
            Flow::Malformed => Err(Stop::Malformed),
        }
    }

    fn stderr(&mut self, bytes: &[u8]) {
        self.last_output = Instant::now();
        self.stderr.extend_from_slice(bytes);
        let excess = self.stderr.len().saturating_sub(STDERR_TAIL);
        self.stderr.drain(..excess);
    }

    #[cfg(unix)]
    fn started(&mut self, child: &orchbee_local_runtime::CommandChild) {
        self.reported = crate::groups::report(child);
    }
}
