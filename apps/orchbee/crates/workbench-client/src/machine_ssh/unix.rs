//! The system OpenSSH client of macOS and Linux and its bounded runs; the runs keep the caller's
//! signal mask, since a Client's signal owner blocks its signals on every thread, and each run's
//! process group ends by the deadline or the interruption.
use super::Output;
use std::{io, path::PathBuf, process::Command, time::Instant};

/// The most output one run keeps of each stream, as on Windows.
const OUTPUT_LIMIT: usize = 64 * 1024;
/// The OS OpenSSH client, which keeps the user's authentication and configuration owner.
const SSH: &str = "/usr/bin/ssh";

pub fn execute(
    command: Command,
    deadline: Option<Instant>,
    interrupted: impl FnMut() -> io::Result<bool>,
) -> io::Result<Output> {
    Ok(orchbee_local_runtime::run_unix_command(
        command,
        OUTPUT_LIMIT,
        deadline,
        interrupted,
    ))
}

/// The OS OpenSSH client.
pub fn executable() -> io::Result<PathBuf> {
    let path = PathBuf::from(SSH);
    if !path.is_file() {
        return Err(io::ErrorKind::NotFound.into());
    }
    Ok(path)
}
