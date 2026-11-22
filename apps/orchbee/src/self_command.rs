//! This program run as a child with its output captured, in the Session this invocation selected.
//! `workspace set-machine` runs each step on this computer this way, and the TUI's Change machine
//! runs that command this way, so a person, a Client and an Agent get one result.

// module-layer: workflow

use orchbee_local_runtime::CommandOutput;
use std::{io, process::Command, time::Instant};

/// This program with `arguments`, in the Session this invocation selected.
pub(crate) fn command(arguments: Vec<String>) -> io::Result<Command> {
    let mut command = Command::new(std::env::current_exe()?);
    if let Some(session) = crate::local_activation::selected_session_name() {
        command.args(["--session", session]);
    }
    command.args(arguments);
    Ok(command)
}

/// Runs `command` with its output captured, until it ends, `deadline` passes or `interrupted`
/// says to stop it. A command writes at most the CLI's output bound on each stream, so the whole
/// of what it wrote fits.
#[cfg(windows)]
pub(crate) fn exchange(
    command: Command,
    deadline: Instant,
    interrupted: impl FnMut() -> io::Result<bool>,
) -> CommandOutput {
    orchbee_local_runtime::run_windows_command_exchange(
        command,
        None,
        crate::cli_outcome::OUTPUT_LIMIT,
        Some(deadline),
        interrupted,
    )
}

/// The child starts with no signal blocked, whatever the calling thread keeps blocked.
#[cfg(unix)]
pub(crate) fn exchange(
    command: Command,
    deadline: Instant,
    interrupted: impl FnMut() -> io::Result<bool>,
) -> CommandOutput {
    orchbee_local_runtime::run_unmasked_unix_command(
        command,
        orchbee_local_runtime::SpawnEnvironment::Inherited,
        crate::cli_outcome::OUTPUT_LIMIT,
        Some(deadline),
        interrupted,
    )
}
