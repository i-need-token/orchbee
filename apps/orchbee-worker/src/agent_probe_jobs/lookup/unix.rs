//! macOS and Linux Agent probe lookup: one run of the person's shell, the system shell a new
//! terminal starts, as an interactive login shell, so the system profile (on macOS
//! `/etc/zprofile`'s `path_helper`), the person's profile and interactive startup files set `PATH`
//! as they do in a new terminal; that shell then becomes `/bin/sh`, which runs `command -v` for
//! each name on the `PATH` they left. The script and the names reach the child only through its
//! environment, so the person's shell never parses a name, whichever shell it is. The child's
//! process group settles before the answers are read.
use nix::sys::signal::SigSet;
use orchbee_capability_node_protocol::AgentProbeResult;
use std::{
    process::Command,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

/// The whole probe, before its process group settles; the Server waits ten seconds in all.
const DEADLINE: Duration = Duration::from_secs(6);
/// Startup files may print before the answers; output past this is dropped.
const OUTPUT_LIMIT: usize = 256 * 1024;
/// The line `/bin/sh` prints before its answers, one per name in request order.
const MARKER: &str = "orchbee-agent-probe";
/// What the person's shell runs once its startup files have run. It names only variables, so it
/// reads the same in zsh, bash, `sh` and fish.
const HANDOFF: &str = "exec /bin/sh -c \"$ORCHBEE_AGENT_PROBE\"";
/// Globbing is off, so a name never matches files; the names hold no blanks.
const SCRIPT: &str = "set -f; echo orchbee-agent-probe; for c in $ORCHBEE_AGENT_PROBE_COMMANDS; do if command -v -- \"$c\" >/dev/null 2>&1; then echo found; else echo missing; fi; done";

/// Answers every command, `Unavailable` where no answer came back.
pub(super) fn run(commands: &[String], cancelled: &AtomicBool) -> Vec<AgentProbeResult> {
    match orchbee_local_runtime::resolve_default_interactive_launch() {
        Ok(shell) => probe(
            Command::new(shell.executable),
            commands,
            cancelled,
            Instant::now() + DEADLINE,
        ),
        Err(_) => vec![AgentProbeResult::Unavailable; commands.len()],
    }
}

/// Runs `shell` once as an interactive login shell that hands over to the lookup script.
pub(super) fn probe(
    mut shell: Command,
    commands: &[String],
    cancelled: &AtomicBool,
    deadline: Instant,
) -> Vec<AgentProbeResult> {
    let mut results = vec![AgentProbeResult::Unavailable; commands.len()];
    // A child keeps the signal mask of the thread that starts it, and the standard library does not
    // clear it. This lookup thread unblocks every signal first, so the shell and every program its
    // startup files run can be interrupted and stopped.
    if SigSet::empty().thread_set_mask().is_err() {
        return results;
    }
    shell
        .args(["-i", "-l", "-c", HANDOFF])
        .env("ORCHBEE_AGENT_PROBE", SCRIPT)
        .env("ORCHBEE_AGENT_PROBE_COMMANDS", commands.join("\n"));
    let output =
        orchbee_local_runtime::run_unix_command(shell, OUTPUT_LIMIT, Some(deadline), || {
            Ok(cancelled.load(Ordering::Relaxed))
        });
    // Answers printed before a deadline or a cancellation stay answers; the rest stay unavailable.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    if let Some(marker) = lines.iter().rposition(|line| *line == MARKER) {
        for (result, answer) in results.iter_mut().zip(&lines[marker + 1..]) {
            *result = match *answer {
                "found" => AgentProbeResult::Found,
                "missing" => AgentProbeResult::Missing,
                _ => AgentProbeResult::Unavailable,
            };
        }
    }
    results
}

#[cfg(test)]
mod tests;
