//! Bounded, nonblocking capture on the retained Windows Job owner thread.
use super::WindowsCommandChild;
use orchbee_local_ipc::ChildOutputReader;
use serde::Serialize;
use std::{
    io::{self, Read},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const OUTPUT_LIMIT: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WindowsCommandFailure {
    ExitFailure,
    SpawnUnavailable,
    SpawnFailed,
    Io,
    Cancelled,
    Timeout,
    PhysicalUnknown,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowsCommandOutput {
    pub exit_code: Option<i32>,
    pub failure: Option<WindowsCommandFailure>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}
impl WindowsCommandOutput {
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
    fn failed(mut self, reason: WindowsCommandFailure) -> Self {
        self.failure = Some(reason);
        self
    }
}

/// The caller owns this synchronous workflow and its cancellation/deadline policy.
/// All children settle before a known receipt; output buffers never exceed 64 KiB each.
pub fn run_windows_command(
    mut command: Command,
    deadline: Option<Instant>,
    mut interrupted: impl FnMut() -> io::Result<bool>,
) -> WindowsCommandOutput {
    use WindowsCommandFailure as Failure;
    let result = WindowsCommandOutput::empty();
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
    command.stdin(Stdio::null()).stdout(stdout).stderr(stderr);
    let mut child = match WindowsCommandChild::spawn(command) {
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
            )
            .and_then(|()| {
                drain(
                    &mut err,
                    &mut result.stderr,
                    &mut result.stderr_truncated,
                    false,
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
            let _drained = drain_tail(&mut out, &mut err, &mut result);
            return result.failed(reason);
        }
        match child.try_wait() {
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Ok(Some(status)) => {
                if drain_tail(&mut out, &mut err, &mut result).is_err() {
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
    result: &mut WindowsCommandOutput,
) -> io::Result<()> {
    for _ in 0..64 {
        drain(out, &mut result.stdout, &mut result.stdout_truncated, true)?;
        drain(err, &mut result.stderr, &mut result.stderr_truncated, true)?;
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
                let keep = count.min(OUTPUT_LIMIT.saturating_sub(kept.len()));
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
