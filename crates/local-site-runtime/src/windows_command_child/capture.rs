//! Bounded, nonblocking capture on the retained Windows Job owner thread.
use super::WindowsCommandChild;
use orchbee_local_ipc::{ChildInputWriter, ChildOutputReader};
use serde::Serialize;
use std::{
    io::{self, Read, Write},
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
    command: Command,
    deadline: Option<Instant>,
    interrupted: impl FnMut() -> io::Result<bool>,
) -> WindowsCommandOutput {
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
    let (mut writer, stdin) = match input {
        Some(_) => match ChildInputWriter::pipe() {
            Ok((writer, stdin)) => (Some(writer), stdin),
            Err(_) => return result.failed(Failure::Io),
        },
        None => (None, Stdio::null()),
    };
    let mut remaining = input.unwrap_or_default();
    command.stdin(stdin).stdout(stdout).stderr(stderr);
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
    result: &mut WindowsCommandOutput,
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
mod tests {
    use super::*;
    fn powershell(script: &str) -> Command {
        let path = std::path::PathBuf::from(winsafe::GetSystemDirectory().unwrap())
            .join("WindowsPowerShell/v1.0/powershell.exe");
        let mut command = Command::new(path);
        command.args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            script,
        ]);
        command
    }
    #[test]
    fn full_duplex_exchange_drains_before_input_and_delivers_eof_and_exit() {
        let script = "$o=[Console]::OpenStandardOutput();$b=[byte[]]::new(131072);$o.Write($b,0,$b.Length);$i=[Console]::OpenStandardInput();$i.CopyTo($o);exit 42";
        let input = (0..1024 * 1024)
            .map(|i| (i % 256) as u8)
            .collect::<Vec<_>>();
        let output = run_windows_command_exchange(
            powershell(script),
            Some(&input),
            2 * 1024 * 1024,
            Some(Instant::now() + Duration::from_secs(15)),
            || Ok(false),
        );
        assert_eq!(
            output.failure,
            Some(WindowsCommandFailure::ExitFailure),
            "{output:?}"
        );
        assert_eq!(output.exit_code, Some(42));
        assert!(!output.stdout_truncated);
        assert_eq!(&output.stdout[..131072], vec![0; 131072]);
        assert_eq!(&output.stdout[131072..], input);
    }
    #[test]
    fn exchange_bounds_both_outputs_and_settles_stalled_input() {
        let output = run_windows_command_exchange(
            powershell(
                "$b=[byte[]]::new(100000);[Console]::OpenStandardOutput().Write($b,0,$b.Length);[Console]::OpenStandardError().Write($b,0,$b.Length)",
            ),
            None,
            100,
            Some(Instant::now() + Duration::from_secs(10)),
            || Ok(false),
        );
        assert_eq!(output.failure, None);
        assert_eq!((output.stdout.len(), output.stderr.len()), (100, 100));
        assert!(output.stdout_truncated && output.stderr_truncated);
        let output = run_windows_command_exchange(
            powershell("Start-Sleep -Seconds 30"),
            Some(&vec![0; 1024 * 1024]),
            100,
            Some(Instant::now() + Duration::from_secs(1)),
            || Ok(false),
        );
        assert_eq!(output.failure, Some(WindowsCommandFailure::Timeout));
    }
    #[test]
    fn exchange_cancels_before_spawn_and_during_blocked_write() {
        let mut invalid = Command::new("orchbee-nonexistent-command.exe");
        invalid.arg("never");
        assert_eq!(
            run_windows_command_exchange(invalid, Some(b"x"), 10, None, || Ok(true)).failure,
            Some(WindowsCommandFailure::Cancelled)
        );
        let start = Instant::now();
        let output = run_windows_command_exchange(
            powershell("Start-Sleep -Seconds 30"),
            Some(&vec![0; 1024 * 1024]),
            100,
            Some(start + Duration::from_secs(10)),
            || Ok(start.elapsed() > Duration::from_millis(700)),
        );
        assert_eq!(output.failure, Some(WindowsCommandFailure::Cancelled));
    }
}
