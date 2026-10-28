//! The bounded provider-command runner — the Rust port of
//! `NodeAgentIntegrationCommandRunner` with the CLI's hardened contract:
//! ambient-PATH resolution only (no login-shell probe), one owned process
//! group per command, SIGTERM→grace→SIGKILL→bounded reap, a 256KiB stdout
//! cap, stderr to /dev/null, and slot admission against the invocation's
//! single monotonic deadline. Provider-side effects beyond the subprocess
//! envelope (its own bookkeeping, detached descendants) are outside this
//! guarantee and are reported, never hidden.

use std::{
    io::Read,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use nix::{
    sys::signal::{Signal, killpg},
    unistd::Pid,
};

use super::exec_budget::CommandDeadlines;
pub(crate) use super::exec_budget::SLOT;

const TERM_GRACE: Duration = Duration::from_millis(500);
const REAP_BOUND: Duration = Duration::from_millis(100);
const POLL_INTERVAL: Duration = Duration::from_millis(20);
const COMMAND_OUTPUT_LIMIT: usize = 256 * 1024;
const MAX_SEARCH_DIRECTORIES: usize = 256;
const MAX_SEARCH_DIRECTORY_BYTES: usize = 4096;

/// The closed result vocabulary a command may end in.
#[derive(Debug)]
pub(crate) enum CommandResult {
    Completed {
        stdout: Vec<u8>,
    },
    Failed,
    OutputTooLarge,
    TimedOut,
    /// The group was killed but the direct child could not be reaped inside
    /// the bound — the honest end-state, never a hung wait.
    Leaked,
    /// The invocation's termination source fired — the in-flight command
    /// took the same group-kill path, never a silent drop.
    Cancelled,
}

/// Resolve `name` on the ambient PATH — first absolute, regular, executable
/// candidate wins. Mirrors `resolveAgentExecutableCandidates` bounds:
/// ≤256 directories per source, ≤4096 chars each, absolute-only, no control
/// characters; an invalid name shape resolves to nothing.
pub(crate) fn resolve_executable(
    name: &str,
    path_env: Option<&std::ffi::OsStr>,
) -> Option<PathBuf> {
    let valid_name = !name.is_empty()
        && name.len() <= 32
        && name.bytes().enumerate().all(|(index, b)| {
            if index == 0 {
                b.is_ascii_lowercase()
            } else {
                b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'
            }
        });
    if !valid_name {
        return None;
    }
    use std::os::unix::ffi::OsStrExt;
    let path_env = path_env?;
    for directory in path_env
        .as_bytes()
        .split(|b| *b == b':')
        .take(MAX_SEARCH_DIRECTORIES)
    {
        if directory.is_empty()
            || directory.len() > MAX_SEARCH_DIRECTORY_BYTES
            || directory[0] != b'/'
            || directory.iter().any(|b| *b <= 0x1f || *b == 0x7f)
        {
            continue;
        }
        let candidate = Path::new(std::ffi::OsStr::from_bytes(directory)).join(name);
        let metadata = match std::fs::metadata(&candidate) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        if !metadata.is_file() {
            continue;
        }
        // Canonical admits the first X_OK regular file — the effective
        // permission check, not just any mode bit.
        if nix::unistd::access(&candidate, nix::unistd::AccessFlags::X_OK).is_ok() {
            return Some(candidate);
        }
    }
    None
}

/// Run one bounded command inside a slot clipped to `invocation_deadline`.
/// Preparation/run and grace/reap consume their preallocated phases and
/// never stretch the slot. `home` is the process working directory.
pub(crate) fn run_command(
    executable: &Path,
    arguments: &[String],
    home: &Path,
    invocation_deadline: Instant,
    cancel: Option<&dyn crate::termination_signal::TerminationSource>,
) -> CommandResult {
    // Test seam: a scripted plan replaces the process spawn entirely — the
    // provider's argv is logged for assertion and the scripted outcome is
    // materialized without touching a real binary.
    #[cfg(any(test, feature = "test-support"))]
    if super::test_support::fault_injection::commands_scripted() {
        let Some(admission) = super::test_support::fault_injection::scripted_admission(cancel)
        else {
            return CommandResult::Cancelled;
        };
        return super::test_support::fault_injection::run_scripted(
            executable,
            arguments,
            invocation_deadline,
            cancel,
            admission,
        );
    }
    if cancel.map(|source| source.requested()).unwrap_or(false) {
        return CommandResult::Cancelled;
    }
    let Some(deadlines) = CommandDeadlines::new(Instant::now(), invocation_deadline) else {
        return CommandResult::TimedOut;
    };
    let mut command = Command::new(executable);
    command
        .args(arguments)
        .current_dir(home)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .env("NO_COLOR", "1")
        // Owned process group: the kill path targets the group, not just
        // the direct child. A parent-death signal primitive would require
        // `pre_exec` — forbidden under this crate's `unsafe_code = "forbid"`
        // — so a SIGINT'd CLI relies on the same group-kill teardown and a
        // surviving descendant reports `Leaked`, never a silent orphan.
        .process_group(0);
    if cancel.map(|source| source.requested()).unwrap_or(false) {
        return CommandResult::Cancelled;
    }
    if Instant::now() >= deadlines.run {
        return CommandResult::TimedOut;
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(_) => return preparation_failure(cancel, deadlines.run),
    };
    let mut stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => return CommandResult::Failed,
    };
    // Nonblocking reads: descendants holding the write end cannot extend the
    // wait — polling only ever runs inside the remaining slot.
    if nix::fcntl::fcntl(
        &stdout,
        nix::fcntl::FcntlArg::F_SETFL(nix::fcntl::OFlag::O_NONBLOCK),
    )
    .is_err()
    {
        // Route through the bounded group teardown — an unbounded direct-child
        // kill+wait would leak the group and could hang on an unstoppable
        // child.
        let outcome = preparation_failure(cancel, deadlines.run);
        return terminate(child, stdout, deadlines.slot, outcome);
    }
    if cancel.map(|source| source.requested()).unwrap_or(false) {
        return terminate(child, stdout, deadlines.slot, CommandResult::Cancelled);
    }
    if Instant::now() >= deadlines.run {
        return terminate(child, stdout, deadlines.slot, CommandResult::TimedOut);
    }
    let mut output = Vec::new();
    loop {
        if cancel.map(|source| source.requested()).unwrap_or(false) {
            return terminate(child, stdout, deadlines.slot, CommandResult::Cancelled);
        }
        if Instant::now() >= deadlines.run {
            return terminate(child, stdout, deadlines.slot, CommandResult::TimedOut);
        }
        match drain(&mut stdout, &mut output) {
            Drain::More => {}
            Drain::Overflow => {
                return terminate(child, stdout, deadlines.slot, CommandResult::OutputTooLarge);
            }
        }
        if cancel.map(|source| source.requested()).unwrap_or(false) {
            return terminate(child, stdout, deadlines.slot, CommandResult::Cancelled);
        }
        if Instant::now() >= deadlines.run {
            return terminate(child, stdout, deadlines.slot, CommandResult::TimedOut);
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                // One last drain: bytes already in the pipe still count
                // toward the cap; what a detached descendant still holds
                // is outside the bound.
                let _ = drain(&mut stdout, &mut output);
                return if output.len() > COMMAND_OUTPUT_LIMIT {
                    CommandResult::OutputTooLarge
                } else if status.success() {
                    CommandResult::Completed { stdout: output }
                } else {
                    CommandResult::Failed
                };
            }
            Ok(None) => {}
            Err(_) => {
                return terminate(child, stdout, deadlines.slot, CommandResult::Failed);
            }
        }
        thread::sleep(POLL_INTERVAL.min(deadlines.run.saturating_duration_since(Instant::now())));
    }
}

/// Classify a preparation failure at the point it becomes observable.
/// Cancellation keeps entry priority; a run phase exhausted inside a
/// blocking OS setup call is a timeout rather than a generic spawn error.
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

enum Drain {
    More,
    Overflow,
}

fn drain(stdout: &mut impl Read, output: &mut Vec<u8>) -> Drain {
    let mut buffer = [0_u8; 8192];
    loop {
        match stdout.read(&mut buffer) {
            Ok(0) => return Drain::More,
            Ok(count) => {
                output.extend_from_slice(&buffer[..count]);
                if output.len() > COMMAND_OUTPUT_LIMIT {
                    return Drain::Overflow;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Drain::More,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Drain::More,
        }
    }
}

/// TERM the group → grace → KILL the group → bounded reap. Every phase is
/// clipped to the slot's absolute deadline; a child still standing when the
/// reap window lapses is `Leaked`, never a blocking wait. The leader exiting
/// during the TERM grace does NOT close the termination path: in-group
/// descendants outlive it, so the KILL is still owed to the group before the
/// reap — the group identity stays valid while any member stands. The grace
/// never calls `try_wait`: reaping the leader early would free its pid and
/// group id while members still stand, and a recycled group id could then
/// receive a KILL meant for the owned group. The `Child` stays unreaped
/// until after the group KILL — only then does the bounded reap run.
fn terminate(
    mut child: Child,
    stdout: impl Read,
    deadline: Instant,
    outcome: CommandResult,
) -> CommandResult {
    let group = Pid::from_raw(child.id() as i32);
    let _ = killpg(group, Signal::SIGTERM);
    // The clipped grace is a fixed window — no `try_wait` observation, so
    // the leader cannot be reaped before the group KILL below.
    let grace_deadline = deadline.min(Instant::now() + TERM_GRACE);
    while Instant::now() < grace_deadline {
        thread::sleep(POLL_INTERVAL.min(grace_deadline.saturating_duration_since(Instant::now())));
    }
    // `child` is still unreaped: its pid is reserved and remains the owned
    // group's id, so this KILL reaches exactly the group that was created.
    let _ = killpg(group, Signal::SIGKILL);
    let reap_deadline = deadline.min(Instant::now() + REAP_BOUND);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return outcome,
            Ok(None) => {}
            // A failed reap after SIGKILL is the honest unknown — the child
            // may be a zombie we can no longer observe, so report the leak
            // rather than claim the original outcome.
            Err(_) => return CommandResult::Leaked,
        }
        if Instant::now() >= reap_deadline {
            // A detached descendant still holds the pipe — drop our end so
            // it cannot extend anything, and report the leak honestly.
            drop(stdout);
            return CommandResult::Leaked;
        }
        thread::sleep(POLL_INTERVAL.min(reap_deadline.saturating_duration_since(Instant::now())));
    }
}
