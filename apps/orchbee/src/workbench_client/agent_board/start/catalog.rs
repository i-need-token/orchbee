//! Cancellable Client-PATH hints for the empty Agent board.
//!
//! Windows path resolution runs in the fixed system `where.exe` process, not
//! in this process. A Job owns each query so a mapped drive, reparse target,
//! or provider filesystem stall remains killable at the catalog deadline.

use std::{
    ffi::OsStr,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

#[cfg(windows)]
#[path = "catalog/windows.rs"]
mod platform;
#[cfg(not(windows))]
#[path = "catalog/unix.rs"]
mod platform;

#[cfg(windows)]
const POLL: Duration = Duration::from_millis(10);
const TEARDOWN: Duration = Duration::from_millis(600);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Query {
    Found,
    Missing,
    Unavailable,
}

pub(super) fn client_path_candidates(
    path: Option<&OsStr>,
    cancelled: &AtomicBool,
    deadline: Instant,
    found: impl Fn(&'static str) + Sync,
) -> bool {
    if stopped(cancelled, deadline) {
        return false;
    }
    let Ok(system) = platform::directory() else {
        return false;
    };
    let executable = system.join("where.exe");
    let Some(run_deadline) = deadline.checked_sub(TEARDOWN) else {
        return false;
    };
    // Four owned workers, at most one killable lookup per worker. Partial results
    // publish immediately; all workers join before the catalog task retires.
    thread::scope(|scope| {
        let mut workers = Vec::new();
        let mut complete = true;
        for providers in crate::agent_provider::CANONICAL_IDS.chunks(
            crate::agent_provider::CANONICAL_IDS
                .len()
                .div_ceil(4)
                .max(1),
        ) {
            let found = &found;
            let executable = &executable;
            let system = &system;
            match thread::Builder::new()
                .name("orchbee-provider-lookup".into())
                .spawn_scoped(scope, move || {
                    resolve_candidates(
                        providers,
                        || stopped(cancelled, run_deadline),
                        |command| {
                            let slot = Instant::now()
                                .checked_add(Duration::from_millis(900))
                                .unwrap_or(run_deadline)
                                .min(run_deadline);
                            query(executable, system, path, command, cancelled, slot, deadline)
                        },
                        found,
                    )
                }) {
                Ok(worker) => workers.push(worker),
                Err(_) => complete = false,
            }
        }
        for worker in workers {
            complete &= worker.join().unwrap_or(false);
        }
        complete
    })
}

/// A failed lookup never discards candidates already found or skips unrelated providers.
pub(super) fn resolve_candidates(
    providers: &[&'static str],
    mut should_stop: impl FnMut() -> bool,
    mut lookup: impl FnMut(&str) -> Query,
    mut found: impl FnMut(&'static str),
) -> bool {
    let mut complete = true;
    for provider in providers.iter().copied() {
        if should_stop() {
            return false;
        }
        let Some(command) = crate::agent_provider::detection_executable(provider) else {
            continue;
        };
        match lookup(command) {
            Query::Found => found(provider),
            Query::Missing => {}
            Query::Unavailable => complete = false,
        }
    }
    complete
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
    match platform::query(
        executable,
        system,
        path,
        name,
        cancelled,
        run_deadline,
        slot_deadline,
    ) {
        Ok(true) => Query::Found,
        Ok(false) => Query::Missing,
        Err(()) => Query::Unavailable,
    }
}

fn stopped(cancelled: &AtomicBool, deadline: Instant) -> bool {
    cancelled.load(Ordering::Acquire) || Instant::now() >= deadline
}
