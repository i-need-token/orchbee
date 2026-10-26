//! Bounded, nonblocking capture on the retained Windows Job owner thread.
use super::CommandChild;
use crate::{CommandFailure, CommandOutput};
use orchbee_local_ipc::{ChildInputWriter, ChildOutputReader};
use std::{
    io::{self, Read, Write},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const OUTPUT_LIMIT: usize = 64 * 1024;

impl CommandOutput {
    fn empty() -> Self {
        Self {
            exit_code: None,
            failure: None,
            stdout: vec![],
            stderr: vec![],
            stdout_truncated: false,
            stderr_truncated: false,
        }
    }
    fn failed(mut self, reason: CommandFailure) -> Self {
        self.failure = Some(reason);
        self
    }
}

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
    mut command: Command,
    input: Option<&[u8]>,
    output_limit: usize,
    deadline: Option<Instant>,
    mut interrupted: impl FnMut() -> io::Result<bool>,
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
    let mut child = match CommandChild::spawn(command) {
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
fn drain_tail(
    out: &mut Option<ChildOutputReader>,
    err: &mut Option<ChildOutputReader>,
    result: &mut CommandOutput,
    output_limit: usize,
) -> io::Result<()> {
    for _ in 0..64 {
        drain(
            out,
            &mut result.stdout,
            &mut result.stdout_truncated,
            true,
            output_limit,
        )?;
        drain(
            err,
            &mut result.stderr,
            &mut result.stderr_truncated,
            true,
            output_limit,
        )?;
        if out.is_none() && err.is_none() {
            return Ok(());
        }
    }
    Err(io::ErrorKind::TimedOut.into())
}
fn drain(
    reader: &mut Option<ChildOutputReader>,
    kept: &mut Vec<u8>,
    truncated: &mut bool,
    settled: bool,
    output_limit: usize,
) -> io::Result<()> {
    let Some(pipe) = reader.as_mut() else {
        return Ok(());
    };
    let mut bytes = [0; 4096];
    for _ in 0..8 {
        match pipe.read(&mut bytes) {
            Ok(0) => {
                *reader = None;
                return Ok(());
            }
            Ok(count) => {
                let keep = count.min(output_limit.saturating_sub(kept.len()));
                kept.extend_from_slice(&bytes[..keep]);
                *truncated |= keep < count;
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                if settled {
                    *reader = None;
                }
                return Ok(());
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(())
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
