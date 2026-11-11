//! Windows Agent probe lookup, copied from the Client's empty-board `PATH` hint
//! (`workbench-client` `agent_board/start/jobs/client_path.rs` and `client_path/windows.rs`).
//! Path resolution runs in the fixed system `where.exe` process, not in this process, on the
//! search path [`shell_path`](super::shell_path) projects. A Job owns each query so a mapped
//! drive, reparse target, or provider filesystem stall remains killable at the probe deadline.
use super::shell_path;
use ::windows::Win32::System::Threading::CREATE_NO_WINDOW;
use orchbee_capability_node_protocol::AgentProbeResult;
use process_wrap::std::{ChildWrapper, CommandWrap, CreationFlags, JobObject, KillOnJobClose};
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

/// The whole probe.
pub(super) const DEADLINE: Duration = Duration::from_secs(6);
/// One query.
const SLOT: Duration = Duration::from_millis(900);
/// Threads that each run one killable query at a time.
const WORKERS: usize = 4;
// The owned OS process wrapper exposes try_wait, not an exit event. Sampling is bounded by
// the lookup deadline and stops when the job is cancelled or exits.
const POLL: Duration = Duration::from_millis(10);
const TEARDOWN: Duration = Duration::from_millis(600);

/// Answers every command, `Unavailable` where no query finished with an answer. The registry is
/// read once per probe, as the Host reads it once per launch.
pub(super) fn run(commands: &[String], cancelled: &AtomicBool) -> Vec<AgentProbeResult> {
    let deadline = Instant::now() + DEADLINE;
    let mut results = vec![AgentProbeResult::Unavailable; commands.len()];
    if let Some(path) = shell_path::current() {
        probe(commands, &mut results, &path, cancelled, deadline);
    }
    results
}

/// Four owned workers, at most one killable lookup per worker. All workers join before the
/// probe returns; a worker that cannot start leaves its commands `Unavailable`.
pub(super) fn probe(
    commands: &[String],
    results: &mut [AgentProbeResult],
    path: &OsStr,
    cancelled: &AtomicBool,
    deadline: Instant,
) {
    if stopped(cancelled, deadline) {
        return;
    }
    let Ok(system) = directory() else {
        return;
    };
    let executable = system.join("where.exe");
    let Some(run_deadline) = deadline.checked_sub(TEARDOWN) else {
        return;
    };
    let chunk = commands.len().div_ceil(WORKERS).max(1);
    thread::scope(|scope| {
        for (commands, results) in commands.chunks(chunk).zip(results.chunks_mut(chunk)) {
            let executable = &executable;
            let system = &system;
            let _worker = thread::Builder::new()
                .name("orchbee-agent-probe-query".into())
                .spawn_scoped(scope, move || {
                    resolve_commands(
                        commands,
                        results,
                        || stopped(cancelled, run_deadline),
                        |command| {
                            let slot = Instant::now()
                                .checked_add(SLOT)
                                .unwrap_or(run_deadline)
                                .min(run_deadline);
                            query(executable, system, path, command, cancelled, slot, deadline)
                        },
                    );
                });
        }
    });
}

/// A failed query never discards an answer already found or skips a later command; a stop leaves
/// every remaining command as it was.
pub(super) fn resolve_commands(
    commands: &[String],
    results: &mut [AgentProbeResult],
    mut should_stop: impl FnMut() -> bool,
    mut lookup: impl FnMut(&str) -> AgentProbeResult,
) {
    for (command, result) in commands.iter().zip(results.iter_mut()) {
        if should_stop() {
            return;
        }
        *result = lookup(command);
    }
}

fn directory() -> std::io::Result<PathBuf> {
    winsafe::GetSystemDirectory()
        .map(PathBuf::from)
        .map_err(std::io::Error::other)
}

/// The Job's kill-on-close policy is the final process-tree ownership guard.
/// Normal paths additionally request termination and confirm Job exit.
struct OwnedJob(Box<dyn ChildWrapper>);

impl Drop for OwnedJob {
    fn drop(&mut self) {
        let _ = self.0.start_kill();
    }
}

/// Runs one query until `run_deadline`, then tears it down until `teardown_limit` at the latest.
fn query(
    executable: &Path,
    system: &Path,
    path: &OsStr,
    name: &str,
    cancelled: &AtomicBool,
    run_deadline: Instant,
    teardown_limit: Instant,
) -> AgentProbeResult {
    if stopped(cancelled, run_deadline) {
        return AgentProbeResult::Unavailable;
    }
    let Some(root) = system.parent() else {
        return AgentProbeResult::Unavailable;
    };
    let mut command = Command::new(executable);
    command
        .current_dir(system)
        .env_clear()
        .env("SystemRoot", root)
        .env("PATH", path)
        .env("PATHEXT", ".EXE;.COM;.CMD;.BAT;.PS1")
        // `$PATH:` excludes the helper's working directory. `/Q` makes the
        // output bound exactly zero; only the documented exit code is read.
        .arg("/Q")
        .arg(format!("$PATH:{name}"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut wrapped = CommandWrap::from(command);
    wrapped
        .wrap(CreationFlags(CREATE_NO_WINDOW))
        .wrap(KillOnJobClose)
        .wrap(JobObject);
    if stopped(cancelled, run_deadline) {
        return AgentProbeResult::Unavailable;
    }
    let mut child = match wrapped.spawn() {
        Ok(child) => OwnedJob(child),
        Err(_) => return AgentProbeResult::Unavailable,
    };
    drop(wrapped);

    loop {
        if stopped(cancelled, run_deadline) {
            break;
        }
        match child.0.try_wait() {
            Ok(Some(status)) => {
                return match status.code() {
                    Some(0) => AgentProbeResult::Found,
                    Some(1) => AgentProbeResult::Missing,
                    _ => AgentProbeResult::Unavailable,
                };
            }
            Ok(None) => {
                thread::sleep(POLL.min(run_deadline.saturating_duration_since(Instant::now())))
            }
            Err(_) => break,
        }
    }

    let _ = child.0.start_kill();
    let teardown_deadline = teardown_limit.min(
        Instant::now()
            .checked_add(TEARDOWN)
            .unwrap_or(teardown_limit),
    );
    while matches!(child.0.try_wait(), Ok(None)) && Instant::now() < teardown_deadline {
        thread::sleep(POLL.min(teardown_deadline.saturating_duration_since(Instant::now())));
    }
    AgentProbeResult::Unavailable
}

fn stopped(cancelled: &AtomicBool, deadline: Instant) -> bool {
    cancelled.load(Ordering::Acquire) || Instant::now() >= deadline
}
