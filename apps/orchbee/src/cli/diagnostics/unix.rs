//! POSIX bundle facts: the macOS product version or the Linux distribution and kernel, and a
//! process's program.
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// Room for one line of `sw_vers` or `ps` output.
const LINE_LIMIT: usize = 4096;

/// `macos` and its product version, which `sw_vers` reports.
#[cfg(target_os = "macos")]
pub(super) fn release() -> Option<(&'static str, Value)> {
    let version = first_line("/usr/bin/sw_vers", &["-productVersion"]);
    Some(("macos", json!({ "version": version })))
}

/// `linux` with the distribution's name and version from `os-release`, and the kernel release.
#[cfg(not(target_os = "macos"))]
pub(super) fn release() -> Option<(&'static str, Value)> {
    let fields = ["/etc/os-release", "/usr/lib/os-release"]
        .iter()
        .find_map(|path| std::fs::read_to_string(path).ok())
        .map(|text| os_release(&text))
        .unwrap_or_default();
    let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .ok()
        .map(|text| text.trim().to_owned());
    Some((
        "linux",
        json!({
            "distribution": fields.0,
            "version": fields.1,
            "kernel": kernel,
        }),
    ))
}

/// `PRETTY_NAME` and `VERSION_ID` from an `os-release` text, each unquoted.
#[cfg(not(target_os = "macos"))]
fn os_release(text: &str) -> (Option<String>, Option<String>) {
    let value = |key: &str| {
        text.lines()
            .filter_map(|line| line.split_once('='))
            .find(|(name, _)| name.trim() == key)
            .map(|(_, value)| value.trim().trim_matches('"').trim_matches('\'').to_owned())
            .filter(|value| !value.is_empty())
    };
    (value("PRETTY_NAME"), value("VERSION_ID"))
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
