//! Bounded, nonblocking capture of one Unix command, whose whole process group settles before a
//! known result.
use super::super::output_drain::{drain, drain_tail};
use super::{CommandChild, SpawnEnvironment};
use crate::{CommandFailure, CommandOutput, CommandSpawnFailure};
use orchbee_local_ipc::ChildOutputReader;
use std::{
    io::{self, PipeWriter, Write},
    os::fd::OwnedFd,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

/// The caller owns this synchronous workflow and its cancellation and deadline policy. The command
/// reads no input; its process group settles before a known result, and each output keeps at most
/// `output_limit` bytes.
pub fn run_unix_command(
    command: Command,
    output_limit: usize,
    deadline: Option<Instant>,
    interrupted: impl FnMut() -> io::Result<bool>,
) -> CommandOutput {
    run_unix_command_exchange(command, None, output_limit, deadline, interrupted)
}

/// [`run_unix_command`] whose command reads `input` on its standard input, which then ends; with
/// `None` it reads the null device. The input is written without blocking between the output
/// drains, so a command that never reads it still ends by its own exit, its cancellation or its
/// deadline.
pub fn run_unix_command_exchange(
    command: Command,
    input: Option<&[u8]>,
    output_limit: usize,
    deadline: Option<Instant>,
    interrupted: impl FnMut() -> io::Result<bool>,
) -> CommandOutput {
    run(
        input,
        output_limit,
        deadline,
        interrupted,
        move |stdin, stdout, stderr| {
            let mut command = command;
            command
                .stdin(stdin)
                .stdout(Stdio::from(stdout))
                .stderr(Stdio::from(stderr));
            CommandChild::spawn(command)
        },
    )
}

/// [`run_unix_command`] for a caller whose thread blocks signals, such as one that captures
/// `SIGINT` and `SIGTERM` to settle its children: the command starts through `posix_spawn` with an
/// empty signal mask, `SIGPIPE` at its default action and the `environment` it states, and takes no
/// working directory. On macOS it inherits no descriptor but its three standard streams.
pub fn run_unmasked_unix_command(
    command: Command,
    environment: SpawnEnvironment,
    output_limit: usize,
    deadline: Option<Instant>,
    interrupted: impl FnMut() -> io::Result<bool>,
) -> CommandOutput {
    run(
        None,
        output_limit,
        deadline,
        interrupted,
        move |_stdin, stdout, stderr| {
            // The parent's copies of the output writers close when this returns, before the drain.
            super::spawn::spawn(&command, environment, &stdout, &stderr)
        },
    )
}

/// Starts the command with `start`, which receives its standard input, the null device or the
/// read end of the `input` pipe, and the output writers, and drains both outputs until its group
/// settles.
fn run(
    input: Option<&[u8]>,
    output_limit: usize,
    deadline: Option<Instant>,
    mut interrupted: impl FnMut() -> io::Result<bool>,
    start: impl FnOnce(Stdio, OwnedFd, OwnedFd) -> Result<CommandChild, CommandSpawnFailure>,
) -> CommandOutput {
    use CommandFailure as Failure;
    let result = CommandOutput::empty();
    let mut stop = || match interrupted() {
        Ok(true) => Some(Failure::Cancelled),
        Err(_) => Some(Failure::Io),
        Ok(false) if deadline.is_some_and(|d| Instant::now() >= d) => Some(Failure::Timeout),
        Ok(false) => None,
    };
    if let Some(reason) = stop() {
        return result.failed(reason);
    }
    let Ok((out, stdout)) = ChildOutputReader::pipe_descriptor() else {
        return result.failed(Failure::Io);
    };
    let Ok((err, stderr)) = ChildOutputReader::pipe_descriptor() else {
        return result.failed(Failure::Io);
    };
    let (stdin, mut feed) = match input {
        None => (Stdio::null(), None),
        Some(input) => {
            let Ok((reader, writer)) = io::pipe() else {
                return result.failed(Failure::Io);
            };
            if rustix::io::ioctl_fionbio(&writer, true).is_err() {
                return result.failed(Failure::Io);
            }
            (Stdio::from(reader), Some((writer, input)))
        }
    };
    let mut child = match start(stdin, stdout, stderr) {
        Ok(child) => child,
        Err(failure) => {
            return result.failed(if failure.created {
                Failure::PhysicalUnknown
            } else if failure.error.kind() == io::ErrorKind::NotFound {
                Failure::SpawnUnavailable
            } else {
                Failure::SpawnFailed
            });
        }
    };
    let mut result = result;
    let mut out = Some(out);
    let mut err = Some(err);
    loop {
        feed_input(&mut feed);
        let reason = stop().or_else(|| {
            drain(
                &mut out,
                &mut result.stdout,
                &mut result.stdout_truncated,
                false,
                output_limit,
            )
            .and_then(|()| {
                drain(
                    &mut err,
                    &mut result.stderr,
                    &mut result.stderr_truncated,
                    false,
                    output_limit,
                )
            })
            .err()
            .map(|_| Failure::Io)
        });
        if let Some(reason) = reason {
            let reason = if child.settle().is_err() {
                Failure::PhysicalUnknown
            } else {
                reason
            };
            let _drained = drain_tail(&mut out, &mut err, &mut result, output_limit);
            return result.failed(reason);
        }
        match child.try_wait() {
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Ok(Some(status)) => {
                if drain_tail(&mut out, &mut err, &mut result, output_limit).is_err() {
                    return result.failed(Failure::Io);
                }
                result.exit_code = status.code();
                result.failure = (!status.success()).then_some(Failure::ExitFailure);
                return result;
            }
            Err(_) => {
                let reason = if child.settle().is_err() {
                    Failure::PhysicalUnknown
                } else {
                    Failure::Io
                };
                return result.failed(reason);
            }
        }
    }
}

/// Writes what the pipe takes now of the input left, and closes the pipe once it is all written or
/// the command closed its end.
fn feed_input(feed: &mut Option<(PipeWriter, &[u8])>) {
    let Some((writer, rest)) = feed else {
        return;
    };
    while !rest.is_empty() {
        match writer.write(rest) {
            Ok(written) if written > 0 => *rest = rest.get(written..).unwrap_or_default(),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return,
            Ok(_) | Err(_) => break,
        }
    }
    *feed = None;
}
