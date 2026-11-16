//! A Unix hub process is watched by its id, as no handle pins it: a probe that finds no process is
//! an exit. The hub's starter reaps it, or `launchd` once that starter is gone, so an exited hub
//! leaves no process behind for long. The caller compares the id the hub names again, as it does
//! after a Windows open.

use super::Open;
use nix::{errno::Errno, sys::signal::kill, unistd::Pid};
use std::{
    thread,
    time::{Duration, Instant},
};

/// How often an exit wait probes the process.
const PROBE: Duration = Duration::from_millis(50);

pub(super) struct Process(Pid);

/// Probes the hub process before waiting for its exit.
pub(super) fn open(process_id: u32) -> Result<Process, Open> {
    let pid = i32::try_from(process_id)
        .ok()
        .filter(|pid| *pid > 0)
        .map(Pid::from_raw)
        .ok_or(Open::Unavailable)?;
    match kill(pid, None) {
        Ok(()) => Ok(Process(pid)),
        Err(Errno::ESRCH) => Err(Open::Gone),
        // Another user's hub: without waiting for its exit no program file is known free.
        Err(Errno::EPERM) => Err(Open::Denied),
        Err(_) => Err(Open::Unavailable),
    }
}

/// Probes the hub process only to learn when it goes.
pub(super) fn watch(process_id: u32) -> Option<Process> {
    open(process_id).ok()
}

/// Whether `process` exited by `until`.
pub(super) fn exited(process: &Process, until: Instant) -> bool {
    loop {
        if kill(process.0, None) == Err(Errno::ESRCH) {
            return true;
        }
        let remaining = until.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return false;
        }
        thread::sleep(remaining.min(PROBE));
    }
}
