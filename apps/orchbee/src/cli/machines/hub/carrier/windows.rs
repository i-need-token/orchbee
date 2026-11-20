//! The Windows carrier child: started inside a Job only the hub holds, its root process held
//! open to end it, in the system folder.

use orchbee_local_runtime::{CommandChild, CommandSpawnFailure};
use std::{
    io::{PipeReader, PipeWriter},
    path::PathBuf,
    process::Command,
};
use winsafe::{HPROCESS, co, guard::CloseHandleGuard};

/// The exit status the hub gives a carrier it ends.
const ENDED_BY_HUB: u32 = 1;

/// The root process, held open so ending it never reaches another process.
pub(super) type Root = CloseHandleGuard<HPROCESS>;

/// The folder every carrier and probe runs in, which no removal can be blocked by.
pub(super) fn folder() -> Option<PathBuf> {
    winsafe::GetSystemDirectory().ok().map(PathBuf::from)
}

/// Starts the child inside its own Job, with the hub's pipe ends as its standard streams.
pub(super) fn spawn(
    mut command: Command,
    (input, output, errors): (PipeReader, PipeWriter, PipeWriter),
) -> Result<CommandChild, CommandSpawnFailure> {
    command.stdin(input).stdout(output).stderr(errors);
    CommandChild::spawn_contained(command)
}

/// The root process of `child`, opened to wait for and end it.
pub(super) fn root(child: &CommandChild) -> Option<Root> {
    HPROCESS::OpenProcess(
        co::PROCESS::SYNCHRONIZE | co::PROCESS::TERMINATE,
        false,
        child.id(),
    )
    .ok()
}

/// Ends the root process; its owner then ends the rest of the Job.
pub(super) fn end(root: &Root) {
    // A root that already exited refuses; its owner is settling the Job either way.
    let _terminated = root.TerminateProcess(ENDED_BY_HUB);
}
