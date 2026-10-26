//! Windows relay process and OpenSSH path effects; the shared runner owns packets and outcomes.

use orchbee_local_runtime::CommandOutput;
use std::{io, path::PathBuf, process::Command, time::Instant};

pub(super) fn ssh_path() -> io::Result<PathBuf> {
    Ok(
        PathBuf::from(winsafe::GetSystemDirectory().map_err(io::Error::other)?)
            .join("OpenSSH/ssh.exe"),
    )
}

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
