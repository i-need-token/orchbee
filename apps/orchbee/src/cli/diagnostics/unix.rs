//! POSIX bundle facts: the macOS product version, a process's program, and the running Sessions,
//! which `session list` names where it is admitted and the default Session's Server answers
//! otherwise. Linux carries no system release.
use super::Running;
use crate::cli::console_interrupt::ArmedSignals;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// Room for one line of `sw_vers` or `ps` output.
const LINE_LIMIT: usize = 4096;

/// `macos` and its product version, which `sw_vers` reports.
pub(super) fn release() -> Option<(&'static str, Value)> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let version = first_line("/usr/bin/sw_vers", &["-productVersion"]);
    Some(("macos", json!({ "version": version })))
}

/// Whether the process `pid` runs `expected`: its start path on macOS, which names the program a
/// Session process was started with, and its program file on Linux; `None` when it cannot be read.
pub(super) fn runs(pid: u32, expected: &Path) -> Option<bool> {
    let image = if cfg!(target_os = "macos") {
        first_line("/bin/ps", &["-o", "comm=", "-p", &pid.to_string()]).map(PathBuf::from)?
    } else {
        std::fs::read_link(format!("/proc/{pid}/exe")).ok()?
    };
    let image = std::fs::canonicalize(&image).unwrap_or(image);
    let expected = std::fs::canonicalize(expected).unwrap_or_else(|_| expected.to_owned());
    Some(image == expected)
}

/// The Sessions `session list` named, or else the default Session as its Server's `query` reports
/// it. The query keeps the inherited signal actions, as `doctor` does here.
pub(super) fn running(listed: Option<Vec<Running>>, deadline: Instant) -> Option<Vec<Running>> {
    if listed.is_some() {
        return listed;
    }
    let signals = ArmedSignals::arm_interrupts_or_inherit().ok()?;
    let facts = crate::cli::runtime_status::server_facts(deadline, &signals).ok()?;
    let state = &facts["server"]["state"];
    let pid = facts["server"]["processes"]
        .as_array()
        .and_then(|processes| processes.iter().find(|entry| entry["role"] == "server"))
        .and_then(|entry| entry["processId"].as_u64())
        .and_then(|pid| u32::try_from(pid).ok());
    Some(vec![Running {
        name: facts["scope"].clone(),
        state: state.clone(),
        pid,
        default: true,
        running: state == "running",
    }])
}

/// The first line a system program prints on success, trimmed; `None` on any failure.
fn first_line(program: &str, arguments: &[&str]) -> Option<String> {
    let mut command = std::process::Command::new(program);
    command.args(arguments).env_clear().env("LC_ALL", "C");
    let result = orchbee_local_runtime::run_unmasked_unix_command(
        command,
        orchbee_local_runtime::SpawnEnvironment::Cleared,
        LINE_LIMIT,
        Some(Instant::now() + Duration::from_secs(2)),
        || Ok(false),
    );
    if result.failure.is_some() || result.stdout_truncated {
        return None;
    }
    let text = String::from_utf8_lossy(&result.stdout);
    let line = text.lines().next()?.trim();
    (!line.is_empty()).then(|| line.to_owned())
}
