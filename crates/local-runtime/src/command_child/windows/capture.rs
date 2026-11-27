//! Bounded, nonblocking capture on the retained Windows Job owner thread.
use super::super::output_drain::{ErrorOutput, drain, drain_tail, ignore_output};
use super::CommandChild;
use crate::{CommandFailure, CommandOutput};
use orchbee_local_ipc::{ChildInputWriter, ChildOutputReader};
use std::{
    io::{self, Write},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const OUTPUT_LIMIT: usize = 64 * 1024;

/// The caller owns this synchronous workflow and its cancellation/deadline policy.
/// All children settle before a known receipt; output buffers never exceed 64 KiB each.
pub fn run_windows_command(
    command: Command,
    deadline: Option<Instant>,
    interrupted: impl FnMut() -> io::Result<bool>,
) -> CommandOutput {
    run_windows_command_exchange(command, None, OUTPUT_LIMIT, deadline, interrupted)
}

/// Full-duplex bounded exchange. Input is borrowed until child settlement; no
/// worker thread or pipe owner outlives this call. Limits apply to each output.
pub fn run_windows_command_exchange(
    command: Command,
    input: Option<&[u8]>,
    output_limit: usize,
    deadline: Option<Instant>,
    interrupted: impl FnMut() -> io::Result<bool>,
) -> CommandOutput {
    exchange(
        command,
        input,
        output_limit,
        deadline,
        interrupted,
        &mut ignore_output,
        CommandChild::spawn,
    )
}

/// As [`run_windows_command_exchange`], in a Job started by [`CommandChild::spawn_contained`]:
/// when the calling process ends, however abruptly, the command and everything it started end
/// with it, so no child outlives its caller to keep writing files the caller owned.
pub fn run_contained_command_exchange(
    command: Command,
    input: Option<&[u8]>,
    output_limit: usize,
    deadline: Option<Instant>,
    interrupted: impl FnMut() -> io::Result<bool>,
) -> CommandOutput {
    exchange(
        command,
        input,
        output_limit,
        deadline,
        interrupted,
        &mut ignore_output,
        CommandChild::spawn_contained,
    )
}

/// As [`run_contained_command_exchange`] without input, in a Job started by
/// [`CommandChild::spawn_background`], so the command and everything it starts run at below-normal
/// priority. `error_output` receives what the command writes on standard error as it arrives, on
/// the calling thread between polls, including what lies past `output_limit`.
pub fn run_background_command_exchange(
    command: Command,
    output_limit: usize,
    deadline: Option<Instant>,
    interrupted: impl FnMut() -> io::Result<bool>,
    mut error_output: impl FnMut(&[u8]),
) -> CommandOutput {
    exchange(
        command,
        None,
        output_limit,
        deadline,
        interrupted,
        &mut error_output,
        CommandChild::spawn_background,
    )
}

fn exchange(
    mut command: Command,
    input: Option<&[u8]>,
    output_limit: usize,
    deadline: Option<Instant>,
    mut interrupted: impl FnMut() -> io::Result<bool>,
    error_output: ErrorOutput<'_>,
    spawn: fn(Command) -> Result<CommandChild, super::CommandSpawnFailure>,
) -> CommandOutput {
    use CommandFailure as Failure;
    let result = CommandOutput::empty();
    let stop = |interrupted: &mut dyn FnMut() -> io::Result<bool>| match interrupted() {
        Ok(true) => Some(Failure::Cancelled),
        Err(_) => Some(Failure::Io),
        Ok(false) if deadline.is_some_and(|d| Instant::now() >= d) => Some(Failure::Timeout),
        Ok(false) => None,
    };
    if let Some(reason) = stop(&mut interrupted) {
        return result.failed(reason);
    }
    let (out, stdout) = match ChildOutputReader::pipe() {
        Ok(v) => v,
        Err(_) => return result.failed(Failure::Io),
    };
    let (err, stderr) = match ChildOutputReader::pipe() {
        Ok(v) => v,
        Err(_) => return result.failed(Failure::Io),
    };
    let (mut writer, stdin) = match input {
        Some(_) => match ChildInputWriter::pipe() {
            Ok((writer, stdin)) => (Some(writer), stdin),
            Err(_) => return result.failed(Failure::Io),
        },
        None => (None, Stdio::null()),
    };
    let mut remaining = input.unwrap_or_default();
    command.stdin(stdin).stdout(stdout).stderr(stderr);
    let mut child = match spawn(command) {
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
        let reason = stop(&mut interrupted).or_else(|| {
            drain(
                &mut out,
                &mut result.stdout,
                &mut result.stdout_truncated,
                false,
                output_limit,
                &mut ignore_output,
            )
            .and_then(|()| {
                drain(
                    &mut err,
                    &mut result.stderr,
                    &mut result.stderr_truncated,
                    false,
                    output_limit,
                    &mut *error_output,
                )
            })
            .and_then(|()| feed(&mut writer, &mut remaining))
            .err()
            .map(|_| Failure::Io)
        });
        if let Some(reason) = reason {
            let reason = if child.settle().is_err() {
                Failure::PhysicalUnknown
            } else {
                reason
            };
            let _drained = drain_tail(&mut out, &mut err, &mut result, output_limit, error_output);
            return result.failed(reason);
        }
        match child.try_wait() {
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Ok(Some(status)) => {
                if drain_tail(&mut out, &mut err, &mut result, output_limit, error_output).is_err()
                {
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
fn feed(writer: &mut Option<ChildInputWriter>, remaining: &mut &[u8]) -> io::Result<()> {
    let Some(pipe) = writer.as_mut() else {
        return Ok(());
    };
    for _ in 0..8 {
        if remaining.is_empty() {
            *writer = None; // EOF belongs to this exchange, never inherited stdin.
            return Ok(());
        }
        match pipe.write(&remaining[..remaining.len().min(32 * 1024)]) {
            Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
            Ok(n) => *remaining = &remaining[n..],
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(()),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            // A command can reject without consuming its input. Preserve its
            // stdout/status; closing its input is not a transport failure.
            Err(e) if e.kind() == io::ErrorKind::BrokenPipe => {
                *writer = None;
                return Ok(());
            }
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
