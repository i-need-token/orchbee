//! The Windows process exchange of the far side of a routed command; the shared runner owns packets
//! and outcomes.

use orchbee_local_runtime::CommandOutput;
use std::{io, process::Command, time::Instant};

pub(super) fn exchange(
    command: Command,
    input: Option<&[u8]>,
    limit: usize,
    deadline: Option<Instant>,
    interrupted: impl FnMut() -> io::Result<bool>,
) -> io::Result<CommandOutput> {
    Ok(orchbee_local_runtime::run_windows_command_exchange(
        command,
        input,
        limit,
        deadline,
        interrupted,
    ))
}
