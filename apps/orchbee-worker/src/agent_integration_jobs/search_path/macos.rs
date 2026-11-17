//! The `PATH` a new terminal's shell has on macOS. A new terminal starts the system shell as an
//! interactive login shell, so `/etc/zprofile`'s `path_helper`, the person's profile and their
//! interactive startup file set `PATH`; this runs that shell once the same way, as the Agent probe
//! does, and the shell then becomes `/bin/sh`, which prints a marker line and the `PATH` they left.
//! The script reaches the child only through its environment, so the person's shell never parses
//! it. An interactive shell takes a controlling terminal when it has one; this Worker has none,
//! because the Server starts its own session.
use orchbee_local_runtime::{
    SpawnEnvironment, resolve_default_interactive_launch, run_unmasked_unix_command,
};
use std::{
    ffi::OsString,
    os::unix::ffi::OsStringExt,
    process::Command,
    time::{Duration, Instant},
};

/// The lookup's own bound inside the job's budget, the Agent probe's.
const LIMIT: Duration = Duration::from_secs(6);
/// Startup files may print before the answer; output past this is dropped.
const OUTPUT_LIMIT: usize = 256 * 1024;
/// The line `/bin/sh` prints before the `PATH` line.
const MARKER: &[u8] = b"orchbee-login-path";
/// What the person's shell runs once its startup files have run. It names only a variable, so it
/// reads the same in zsh, bash, `sh` and fish.
const HANDOFF: &str = "exec /bin/sh -c \"$ORCHBEE_LOGIN_PATH\"";
/// What `/bin/sh` runs: the marker, then `PATH` on one line.
const SCRIPT: &str = "echo orchbee-login-path; printf '%s\\n' \"$PATH\"";

/// The login shell's `PATH`, or `None` when no shell starts, none prints it before `ceiling` or
/// six seconds pass, or `cancelled` answers true first.
pub(super) fn read(ceiling: Instant, cancelled: impl Fn() -> bool) -> Option<OsString> {
    let shell = resolve_default_interactive_launch().ok()?;
    lookup(
        Command::new(shell.executable),
        ceiling.min(Instant::now() + LIMIT),
        cancelled,
    )
}

/// Runs `shell` once as an interactive login shell that hands over to the script, and reads the
/// line after the last marker.
fn lookup(mut shell: Command, deadline: Instant, cancelled: impl Fn() -> bool) -> Option<OsString> {
    shell
        .args(["-i", "-l", "-c", HANDOFF])
        .env("ORCHBEE_LOGIN_PATH", SCRIPT);
    let output = run_unmasked_unix_command(
        shell,
        SpawnEnvironment::Inherited,
        OUTPUT_LIMIT,
        Some(deadline),
        || Ok(cancelled()),
    );
    let lines: Vec<&[u8]> = output.stdout.split(|byte| *byte == b'\n').collect();
    let marker = lines.iter().rposition(|line| *line == MARKER)?;
    let path = lines.get(marker + 1)?;
    (!path.is_empty()).then(|| OsString::from_vec(path.to_vec()))
}

#[cfg(test)]
mod tests;
