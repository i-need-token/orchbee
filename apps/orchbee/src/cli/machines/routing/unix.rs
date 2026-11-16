//! The macOS and Linux process exchange of the far side of a routed command; the shared runner owns
//! packets and outcomes.

use nix::sys::signal::SigSet;
use orchbee_local_runtime::{CommandOutput, SessionName};
use std::{io, process::Command, time::Instant};

/// Selects `session` for the routed command. Only the default Session is served here, which a
/// command without `--session` reaches; any other is refused before anything runs.
pub(super) fn select_session(command: &mut Command, session: &str) -> bool {
    // The channel that made the Session ready ignored any scope override, so the command does too.
    command.env_remove("ORCHBEE_RUNTIME_SCOPE");
    session == SessionName::default_session().as_str()
}

/// Runs `command` with `input` on its standard input. It runs only on a link helper's route thread,
/// in a process that owns no signal, so that thread unblocks every signal first: a child keeps the
/// signal mask of the thread that starts it, and the routed command must be interruptible.
pub(super) fn exchange(
    command: Command,
    input: Option<&[u8]>,
    limit: usize,
    deadline: Option<Instant>,
    interrupted: impl FnMut() -> io::Result<bool>,
) -> io::Result<CommandOutput> {
    SigSet::empty().thread_set_mask()?;
    Ok(orchbee_local_runtime::run_unix_command_exchange(
        command,
        input,
        limit,
        deadline,
        interrupted,
    ))
}
