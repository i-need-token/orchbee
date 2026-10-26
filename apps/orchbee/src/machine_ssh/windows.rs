//! Windows OpenSSH discovery and process ownership.
use super::Output;
use std::{io, path::PathBuf, process::Command, time::Instant};
pub(super) fn execute(
    command: Command,
    deadline: Option<Instant>,
    interrupted: impl FnMut() -> io::Result<bool>,
) -> io::Result<Output> {
    Ok(orchbee_local_runtime::run_windows_command(
        command,
        deadline,
        interrupted,
    ))
}
/// The OS OpenSSH client, retaining the user's authentication/configuration owner.
pub(crate) fn executable() -> io::Result<PathBuf> {
    let path = PathBuf::from(winsafe::GetSystemDirectory().map_err(io::Error::other)?)
        .join("OpenSSH/ssh.exe");
    if !path.is_file() {
        return Err(io::ErrorKind::NotFound.into());
    }
    Ok(path)
}
