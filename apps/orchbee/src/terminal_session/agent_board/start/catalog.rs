//! Cancellable Client-PATH hints for the empty Agent board.
//!
//! Windows path resolution runs in the fixed system `where.exe` process, not
//! in this process. A Job owns each query so a mapped drive, reparse target,
//! or provider filesystem stall remains killable at the catalog deadline.

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

use process_wrap::std::{ChildWrapper, CommandWrap, CreationFlags, JobObject, KillOnJobClose};
use windows::Win32::System::Threading::CREATE_NO_WINDOW;

const POLL: Duration = Duration::from_millis(10);
const TEARDOWN: Duration = Duration::from_millis(600);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Query {
    Found,
    Missing,
    Unavailable,
}

/// The Job's kill-on-close policy is the final process-tree ownership guard.
/// Normal paths additionally request termination and confirm Job exit.
struct OwnedJob(Box<dyn ChildWrapper>);

impl Drop for OwnedJob {
    fn drop(&mut self) {
        let _ = self.0.start_kill();
    }
}

pub(super) fn client_path_candidates(
    path: Option<&OsStr>,
    cancelled: &AtomicBool,
    deadline: Instant,
) -> Option<Vec<&'static str>> {
    if stopped(cancelled, deadline) {
        return None;
    }
    let system = PathBuf::from(winsafe::GetSystemDirectory().ok()?);
    let executable = system.join("where.exe");
    let run_deadline = deadline.checked_sub(TEARDOWN)?;
    resolve_candidates(
        || stopped(cancelled, run_deadline),
        |command| {
            query(
                &executable,
                &system,
                path,
                command,
                cancelled,
                run_deadline,
                deadline,
            )
        },
    )
}

/// Projects the closed provider mapping through one lookup verdict per command.
/// The OS query stays outside this deterministic policy loop so cancellation,
/// mapping, and process behavior can each carry focused evidence.
pub(super) fn resolve_candidates(
    mut should_stop: impl FnMut() -> bool,
    mut lookup: impl FnMut(&str) -> Query,
) -> Option<Vec<&'static str>> {
    let mut candidates = Vec::new();
    for provider in crate::agent_provider::CANONICAL_IDS.iter().copied() {
        if should_stop() {
            return None;
        }
        let Some(command) = crate::agent_provider::executable(provider) else {
            continue;
        };
        match lookup(command) {
            Query::Found => candidates.push(provider),
            Query::Missing => {}
            Query::Unavailable => return None,
        }
    }
    Some(candidates)
}

pub(super) fn query(
    executable: &Path,
    system: &Path,
    path: Option<&OsStr>,
    name: &str,
    cancelled: &AtomicBool,
    run_deadline: Instant,
    slot_deadline: Instant,
) -> Query {
    if stopped(cancelled, run_deadline) {
        return Query::Unavailable;
    }
    let Some(root) = system.parent() else {
        return Query::Unavailable;
    };
    let mut command = Command::new(executable);
    command
        .current_dir(system)
        .env_clear()
        .env("SystemRoot", root)
        .env("PATH", path.unwrap_or_else(|| OsStr::new("")))
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
        return Query::Unavailable;
    }
    let mut child = match wrapped.spawn() {
        Ok(child) => OwnedJob(child),
        Err(_) => return Query::Unavailable,
    };
    drop(wrapped);

    loop {
        if stopped(cancelled, run_deadline) {
            break;
        }
        match child.0.try_wait() {
            Ok(Some(status)) => {
                return match status.code() {
                    Some(0) => Query::Found,
                    Some(1) => Query::Missing,
                    _ => Query::Unavailable,
                };
            }
            Ok(None) => {
                thread::sleep(POLL.min(run_deadline.saturating_duration_since(Instant::now())))
            }
            Err(_) => break,
        }
    }

    let _ = child.0.start_kill();
    let teardown_deadline = slot_deadline.min(
        Instant::now()
            .checked_add(TEARDOWN)
            .unwrap_or(slot_deadline),
    );
    loop {
        match child.0.try_wait() {
            Ok(Some(_)) => return Query::Unavailable,
            Ok(None) if Instant::now() < teardown_deadline => {
                thread::sleep(POLL.min(teardown_deadline.saturating_duration_since(Instant::now())))
            }
            _ => return Query::Unavailable,
        }
    }
}

fn stopped(cancelled: &AtomicBool, deadline: Instant) -> bool {
    cancelled.load(Ordering::Acquire) || Instant::now() >= deadline
}
