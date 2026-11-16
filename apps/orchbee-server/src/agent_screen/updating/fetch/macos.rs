//! The system curl as one bounded process group per attempt, through the local runtime's Unix
//! command owner: the group is stopped and has left before any result, so no descendant or output
//! writer outlives the attempt. curl lives only as long as its own fifteen-second limit, so the
//! standard library's spawn starts it, as it starts other short-lived commands.
use super::super::{CatalogUrl, Failure, LIMIT};
use orchbee_local_runtime::{CommandFailure, run_unix_command};
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};

/// Part of every macOS installation, so no search path or user-installed copy is consulted.
const CURL: &str = "/usr/bin/curl";

pub(super) fn curl() -> Result<PathBuf, Failure> {
    Ok(PathBuf::from(CURL))
}

pub(super) fn once(
    program: &Path,
    source: &CatalogUrl,
    stop: &AtomicBool,
    deadline: Instant,
) -> Result<String, Failure> {
    let mut command = Command::new(program);
    // No variable reaches curl, so no proxy, configuration or home-folder setting steers it.
    command
        .env_clear()
        .args(super::CURL_ARGUMENTS)
        .arg(&source.0);
    let output = run_unix_command(command, LIMIT, Some(deadline), || {
        Ok(stop.load(Ordering::Acquire))
    });
    match output.failure {
        Some(CommandFailure::Cancelled) => return Err(Failure::Cancelled),
        Some(CommandFailure::Timeout) => return Err(Failure::Deadline),
        Some(
            CommandFailure::SpawnUnavailable
            | CommandFailure::SpawnFailed
            | CommandFailure::Io
            | CommandFailure::PhysicalUnknown,
        ) => return Err(Failure::Process),
        Some(CommandFailure::ExitFailure) | None => {}
    }
    if output.stdout_truncated {
        return Err(Failure::TooLarge);
    }
    match output.exit_code {
        Some(0) => String::from_utf8(output.stdout).map_err(|_| Failure::InvalidUtf8),
        // curl's own refusal of a body over `--max-filesize`.
        Some(63) => Err(Failure::TooLarge),
        _ => Err(Failure::Fetch),
    }
}
