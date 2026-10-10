//! Windows provider commands: structured argv, suspended Job assignment,
//! nonblocking bounded output, and cooperative whole-tree cancellation.
use std::{
    ffi::OsStr,
    io::{self, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use orchbee_local_ipc::ChildOutputReader;
use process_wrap::std::{ChildWrapper, CommandWrap, CreationFlags, JobObject};
use windows::Win32::System::Threading::CREATE_NO_WINDOW;

use super::exec_budget::CommandDeadlines;
pub(crate) use super::exec_budget::SLOT;

const POLL: Duration = Duration::from_millis(10);
const OUTPUT_LIMIT: usize = 256 * 1024;

#[derive(Debug)]
pub(crate) enum CommandResult {
    Completed { stdout: Vec<u8> },
    Failed,
    OutputTooLarge,
    TimedOut,
    Leaked,
    Cancelled,
}

pub(crate) fn resolve_executable(name: &str, path_env: Option<&OsStr>) -> Option<PathBuf> {
    if name.is_empty()
        || name.len() > 32
        || !name.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase() || index > 0 && (byte.is_ascii_digit() || byte == b'-')
        })
    {
        return None;
    }
    for directory in std::env::split_paths(path_env?).take(256) {
        if !super::files::path_admitted(&directory) {
            continue;
        }
        for extension in ["exe", "cmd", "bat"] {
            let candidate = directory.join(format!("{name}.{extension}"));
            if super::files::regular_file(&candidate).is_ok() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Drop is a final safety net; normal exits explicitly terminate before
/// returning so a teardown error remains visible in the result.
struct OwnedJob(Box<dyn ChildWrapper>);

impl Drop for OwnedJob {
    fn drop(&mut self) {
        let _ = self.0.start_kill();
    }
}

pub(crate) fn run_command(
    executable: &Path,
    arguments: &[String],
    home: &Path,
    deadline: Instant,
    cancel: Option<&dyn crate::termination_signal::TerminationSource>,
) -> CommandResult {
    run_scoped_command(executable, arguments, home, None, deadline, cancel)
}
pub(crate) fn run_hermes_command(
    executable: &Path,
    arguments: &[String],
    home: &Path,
    deadline: Instant,
    cancel: Option<&dyn crate::termination_signal::TerminationSource>,
) -> CommandResult {
    let Some(profile) = crate::agent_configuration_windows::hermes_home(home) else {
        return CommandResult::Failed;
    };
    if !super::files::path_admitted(&profile) {
        return CommandResult::Failed;
    }
    run_scoped_command(
        executable,
        arguments,
        home,
        Some(&profile),
        deadline,
        cancel,
    )
}
fn run_scoped_command(
    executable: &Path,
    arguments: &[String],
    home: &Path,
    hermes_home: Option<&Path>,
    invocation_deadline: Instant,
    cancel: Option<&dyn crate::termination_signal::TerminationSource>,
) -> CommandResult {
    if cancel.is_some_and(|source| source.requested()) {
        return CommandResult::Cancelled;
    }
    let Some(deadlines) = CommandDeadlines::new(Instant::now(), invocation_deadline) else {
        return CommandResult::TimedOut;
    };
    let (mut stdout, writer) = match ChildOutputReader::pipe() {
        Ok(pipe) => pipe,
        Err(_) => return preparation_failure(cancel, deadlines.run),
    };
    // Command owns Windows EXE/batch escaping. Never interpolate argv into
    // a shell command or use raw_arg; batch arguments unsafe to encode fail.
    let mut command = Command::new(executable);
    command
        .args(arguments)
        .current_dir(home)
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stdout(writer)
        .stderr(Stdio::null());
    if let Some(profile) = hermes_home {
        command.env("HERMES_HOME", profile);
    }
    let mut wrapped = CommandWrap::from(command);
    wrapped
        .wrap(CreationFlags(CREATE_NO_WINDOW))
        .wrap(JobObject);
    if cancel.is_some_and(|source| source.requested()) {
        return CommandResult::Cancelled;
    }
    if Instant::now() >= deadlines.run {
        return CommandResult::TimedOut;
    }
    let mut child = match wrapped.spawn() {
        Ok(child) => OwnedJob(child),
        Err(_) => return preparation_failure(cancel, deadlines.run),
    };
    // Release the parent's duplicate writer before waiting for EOF.
    drop(wrapped);
    let mut output = Vec::new();
    let result = loop {
        if cancel.is_some_and(|source| source.requested()) {
            break CommandResult::Cancelled;
        }
        if Instant::now() >= deadlines.run {
            break CommandResult::TimedOut;
        }
        match drain(&mut stdout, &mut output) {
            Ok(()) => {}
            Err(true) => break CommandResult::OutputTooLarge,
            Err(false) => break CommandResult::Failed,
        }
        if cancel.is_some_and(|source| source.requested()) {
            break CommandResult::Cancelled;
        }
        if Instant::now() >= deadlines.run {
            break CommandResult::TimedOut;
        }
        match child.0.try_wait() {
            Ok(Some(status)) => {
                break match drain(&mut stdout, &mut output) {
                    Err(true) => CommandResult::OutputTooLarge,
                    Err(false) => CommandResult::Failed,
                    Ok(()) if status.success() => CommandResult::Completed { stdout: output },
                    Ok(()) => CommandResult::Failed,
                };
            }
            Ok(None) => {}
            Err(_) => break CommandResult::Failed,
        }
        thread::sleep(POLL.min(deadlines.run.saturating_duration_since(Instant::now())));
    };
    // Terminate the entire Job even if the leader already exited. Provider
    // grandchildren retaining stdout must not survive a successful command.
    if child.0.start_kill().is_err() {
        return CommandResult::Leaked;
    }
    loop {
        match child.0.try_wait() {
            Ok(Some(_)) => return result,
            Ok(None) if Instant::now() < deadlines.slot => {
                thread::sleep(POLL.min(deadlines.slot.saturating_duration_since(Instant::now())))
            }
            _ => return CommandResult::Leaked,
        }
    }
}

/// Classify a preparation failure at the point it becomes observable.
/// Cancellation keeps entry priority; a run phase exhausted inside pipe
/// creation or process spawn is a timeout rather than a generic failure.
fn preparation_failure(
    cancel: Option<&dyn crate::termination_signal::TerminationSource>,
    run_deadline: Instant,
) -> CommandResult {
    if cancel.is_some_and(|source| source.requested()) {
        CommandResult::Cancelled
    } else if Instant::now() >= run_deadline {
        CommandResult::TimedOut
    } else {
        CommandResult::Failed
    }
}

fn drain(reader: &mut impl Read, output: &mut Vec<u8>) -> Result<(), bool> {
    // A continuously writing provider cannot starve deadline/cancel polling.
    for _ in 0..33 {
        let mut bytes = [0; 8192];
        match reader.read(&mut bytes) {
            Ok(0) => return Ok(()),
            Ok(count) => {
                if output.len() + count > OUTPUT_LIMIT {
                    return Err(true);
                }
                output.extend_from_slice(&bytes[..count]);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(false),
        }
    }
    Ok(())
}
