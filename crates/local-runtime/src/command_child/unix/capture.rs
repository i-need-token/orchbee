//! Bounded, nonblocking capture of one Unix command, whose whole process group settles before a
//! known result.
use super::super::output_drain::{drain, drain_tail};
use super::CommandChild;
use crate::{CommandFailure, CommandOutput};
use orchbee_local_ipc::ChildOutputReader;
use std::{
    io,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

/// The caller owns this synchronous workflow and its cancellation and deadline policy. The command
/// reads no input; its process group settles before a known result, and each output keeps at most
/// `output_limit` bytes.
pub fn run_unix_command(
    mut command: Command,
    output_limit: usize,
    deadline: Option<Instant>,
    mut interrupted: impl FnMut() -> io::Result<bool>,
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
    let Ok((out, stdout)) = ChildOutputReader::pipe() else {
        return result.failed(Failure::Io);
    };
    let Ok((err, stderr)) = ChildOutputReader::pipe() else {
        return result.failed(Failure::Io);
    };
    command.stdin(Stdio::null()).stdout(stdout).stderr(stderr);
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
