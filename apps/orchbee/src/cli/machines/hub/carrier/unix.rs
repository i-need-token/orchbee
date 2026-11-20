//! The macOS and Linux carrier child: started as the leader of its own process group, which the
//! hub ends whole, in the root folder, with the hub's pipes as its standard streams.

use nix::{
    sys::signal::{SigSet, Signal, killpg},
    unistd::Pid,
};
use orchbee_local_runtime::{CommandChild, CommandSpawnFailure, SpawnEnvironment};
use std::{
    io::{PipeReader, PipeWriter},
    path::PathBuf,
    process::Command,
};

/// The carrier's process group, whose ID its unreaped root keeps from being given away.
pub(super) type Root = Pid;

/// The folder every carrier and probe runs in, which no removal can be blocked by.
pub(super) fn folder() -> Option<PathBuf> {
    Some(PathBuf::from("/"))
}

/// Starts the child as the leader of a new process group, with the hub's pipe ends as its standard
/// input, output and error, and exactly the variables `command` sets. On macOS the child inherits no
/// other descriptor, whatever the hub's threads create meanwhile. The owner thread, in the hub that
/// owns no signal, unblocks every signal first, so the child starts with an empty signal mask and
/// `ssh`, its askpass helpers and a stand-in can be interrupted and stopped.
pub(super) fn spawn(
    command: Command,
    (input, output, errors): (PipeReader, PipeWriter, PipeWriter),
) -> Result<CommandChild, CommandSpawnFailure> {
    SigSet::empty()
        .thread_set_mask()
        .map_err(|error| CommandSpawnFailure {
            error: error.into(),
            created: false,
        })?;
    CommandChild::spawn_with_streams(
        command,
        SpawnEnvironment::Cleared,
        [input.into(), output.into(), errors.into()],
    )
}

/// The process group `child` leads.
pub(super) fn root(child: &CommandChild) -> Option<Root> {
    i32::try_from(child.id()).ok().map(Pid::from_raw)
}

/// Ends the whole group; its owner then reaps the root and waits until no member is left.
pub(super) fn end(root: &Root) {
    // A group whose members have all exited refuses, with `EPERM` on macOS while only the
    // unreaped root is left; its owner is settling the group either way.
    let _killed = killpg(*root, Signal::SIGKILL);
}
