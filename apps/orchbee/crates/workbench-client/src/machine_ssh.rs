//! Shared Windows SSH command policy and installed-Client admission, without catalog or UI ownership.
//! Admission compares no product version, protocol, or cohort across machines: the remote Session
//! judges this Client through the handshake the tunnel carries.
use crate::machine_refusal::Refusal;
use base64::Engine as _;
use serde_json::Value;
use std::{io, path::Path, process::Command, time::Instant};

/// The exit status of the remote launch line when it finds no Orchbee to run or cannot start it.
pub const PROGRAM_NOT_FOUND: i32 = 125;
/// Why a remote Client status document was not admitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionError {
    /// The document is malformed or names an executable this rule cannot run.
    Protocol,
    /// The remote is not Windows on x64 or ARM64.
    Unsupported,
    /// The remote Client reports no available installation to run.
    NotInstalled,
    /// The remote Client belongs to another environment kind than this build.
    OtherEnvironment,
}
impl AdmissionError {
    /// The refusal the user sees; a malformed document has none and reads as a failed attempt.
    pub fn refusal(self) -> Option<Refusal> {
        match self {
            Self::Protocol => None,
            Self::Unsupported | Self::NotInstalled => Some(Refusal::Unsupported),
            Self::OtherEnvironment => Some(Refusal::OtherEnvironment),
        }
    }
}
/// An admitted remote installed Client.
pub struct Client {
    pub executable: String,
    /// The remote product version, shown to the user and never compared.
    pub version: String,
}
/// Admits a remote `orchbee.status.client` document. The remote must run Windows on x64 or ARM64
/// from an available installation of the same environment kind as this build: release admits only
/// `orchbee.exe`, and a development or test build admits any valid development or test name.
pub fn client(value: &Value) -> Result<Client, AdmissionError> {
    client_of_kind(value, orchbee_local_runtime::is_release())
}
fn client_of_kind(value: &Value, release: bool) -> Result<Client, AdmissionError> {
    if value["schema"] != "orchbee.status.client" || value["version"] != 1 {
        return Err(AdmissionError::Protocol);
    }
    let r = &value["result"];
    if r["platform"] != "windows"
        || !matches!(r["architecture"].as_str(), Some("x86_64" | "aarch64"))
    {
        return Err(AdmissionError::Unsupported);
    }
    let executable = r["executable"]["path"]
        .as_str()
        .filter(|s| {
            s.len() <= 4096
                && !s.chars().any(char::is_control)
                && s.as_bytes().get(1) == Some(&b':')
                && s.as_bytes()[0].is_ascii_alphabetic()
                && s.as_bytes()
                    .get(2)
                    .is_some_and(|b| matches!(b, b'/' | b'\\'))
        })
        .ok_or(AdmissionError::Protocol)?;
    let stem = executable
        .rsplit(['/', '\\'])
        .next()
        .map(str::to_ascii_lowercase)
        .and_then(|name| name.strip_suffix(".exe").map(str::to_owned))
        .filter(|stem| orchbee_local_runtime::valid_environment_stem(stem))
        .ok_or(AdmissionError::Protocol)?;
    if !same_environment_kind(&stem, release) {
        return Err(AdmissionError::OtherEnvironment);
    }
    let version = r["version"]
        .as_str()
        .filter(|version| orchbee_connection_handshake::ProductVersion::new(*version).is_ok())
        .ok_or(AdmissionError::Protocol)?;
    if r["cohort"]["state"] != "available" {
        return Err(AdmissionError::NotInstalled);
    }
    Ok(Client {
        executable: executable.to_owned(),
        version: version.to_owned(),
    })
}
/// Release admits only release; any development or test stem admits any other, because the
/// handshake through the tunnel compares the exact environment.
fn same_environment_kind(stem: &str, release: bool) -> bool {
    (stem == orchbee_connection_handshake::RELEASE_ENVIRONMENT) == release
}
pub fn literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}
/// The remote command that runs `action` through the given executable or, without one, through the
/// active version a remote store's pointer names. An unreadable pointer names none, and nothing is
/// inferred from `Path`.
pub fn program(executable: Option<&str>, session: &str, action: &str) -> String {
    let args = if action == "start" {
        "'server' 'start'".to_owned()
    } else {
        format!("'status' {}", literal(action))
    };
    launch(
        executable,
        &format!("'--session' {} {args} '--format' 'json'", literal(session)),
    )
}
/// The remote command the machine hub's carrier runs: the remote link helper of the active version
/// a remote store's pointer names, resolved as [`program`] resolves it, with exit 125 when there is
/// none.
pub fn link_program() -> String {
    launch(None, "'--machine-link-v1'")
}
/// The encoded PowerShell command that resolves the program and runs it with `arguments`, already
/// quoted.
fn launch(executable: Option<&str>, arguments: &str) -> String {
    let resolve = executable.map_or_else(
        || {
            let (roots, names) = probe_names();
            let list = |items: Vec<String>| {
                items
                    .iter()
                    .map(|item| literal(item))
                    .collect::<Vec<_>>()
                    .join(",")
            };
            format!(
                "$program=$null;\
                $roots=if($env:ORCHBEE_INSTALL_DIR){{,$env:ORCHBEE_INSTALL_DIR}}else{{foreach($name in {}){{Join-Path $env:LOCALAPPDATA ('Programs\\'+$name)}}}};\
                foreach($root in $roots){{if($program){{break}};try{{\
                $pointer=[IO.File]::ReadAllText((Join-Path $root 'active.json'))|ConvertFrom-Json;\
                $version=$pointer.active.product;\
                if($pointer.version -eq 1 -and $version -is [string] -and $version -cmatch '^[0-9][0-9A-Za-z.-]{{0,63}}$'){{\
                foreach($name in {}){{$candidate=Join-Path (Join-Path (Join-Path $root 'versions') $version) $name;\
                if(Test-Path -LiteralPath $candidate -PathType Leaf){{$program=$candidate;break}}}}}}}}catch{{}}}}",
                list(roots),
                list(names)
            )
        },
        |path| format!("$program={}", literal(path)),
    );
    let script = format!(
        "$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue';\
        [Console]::OutputEncoding=New-Object System.Text.UTF8Encoding($false);\
        $OutputEncoding=[Console]::OutputEncoding;\
        try{{{resolve};if(-not $program){{exit {PROGRAM_NOT_FOUND}}};\
        & $program {arguments};exit $LASTEXITCODE}}catch{{exit {PROGRAM_NOT_FOUND}}}"
    );
    let encoded = base64::engine::general_purpose::STANDARD.encode(
        script
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>(),
    );
    format!("powershell.exe -NoLogo -NoProfile -NonInteractive -EncodedCommand {encoded}")
}
/// The remote store folders under `%LOCALAPPDATA%\Programs` and the Client names a probe tries in
/// order. A development or test build first looks for its own store and name and then for
/// release, so a release remote reads as another environment instead of a missing installation.
fn probe_names() -> (Vec<String>, Vec<String>) {
    let release = format!("{}.exe", orchbee_connection_handshake::RELEASE_ENVIRONMENT);
    if orchbee_local_runtime::is_release() {
        (vec!["Orchbee".to_owned()], vec![release])
    } else {
        let stem = orchbee_local_runtime::environment_stem();
        (
            vec![stem.to_owned(), "Orchbee".to_owned()],
            vec![format!("{stem}.exe"), release],
        )
    }
}
pub fn command(ssh: &Path, target: &str, remote: &str) -> Command {
    let mut command = Command::new(ssh);
    command.arg("-T");
    for option in [
        "BatchMode=yes",
        "StrictHostKeyChecking=yes",
        "ConnectionAttempts=1",
        "ConnectTimeout=10",
        "ServerAliveInterval=5",
        "ServerAliveCountMax=2",
        "ClearAllForwardings=yes",
        "ForwardAgent=no",
        "ForwardX11=no",
        "PermitLocalCommand=no",
        "RemoteCommand=none",
        "ControlMaster=no",
        "ControlPath=none",
    ] {
        command.args(["-o", option]);
    }
    command.args(["--", target, remote]);
    command
        .env_remove("SSH_ASKPASS")
        .env("SSH_ASKPASS_REQUIRE", "never");
    command
}

/// Probe only; no Session activation, catalog write, or automatic retry.
pub(crate) fn probe(
    draft: &crate::machines::Draft,
    deadline: Instant,
    interrupted: impl FnMut() -> io::Result<bool>,
) -> io::Result<Client> {
    let target = draft.ssh_target().ok_or(io::ErrorKind::Unsupported)?;
    let ssh = executable()?;
    let result = execute(
        command(
            &ssh,
            target,
            &program(None, draft.session.as_str(), "client"),
        ),
        Some(deadline),
        interrupted,
    )?;
    match result.failure {
        Some(Failure::Cancelled) => return Err(io::ErrorKind::Interrupted.into()),
        Some(Failure::Timeout) => return Err(io::ErrorKind::TimedOut.into()),
        Some(Failure::PhysicalUnknown) => {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        Some(Failure::SpawnUnavailable) => return Err(io::ErrorKind::NotFound.into()),
        Some(Failure::ExitFailure) => {
            return Err(probe_exit_kind(result.exit_code, &result.stderr).into());
        }
        Some(_) => return Err(io::Error::other("SSH Client probe failed")),
        None => {}
    }
    if result.exit_code != Some(0) {
        return Err(probe_exit_kind(result.exit_code, &result.stderr).into());
    }
    if result.stdout_truncated {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let value = serde_json::from_slice(&result.stdout).map_err(|_| io::ErrorKind::InvalidData)?;
    client(&value).map_err(|error| {
        error.refusal().map_or_else(
            || {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "remote Client status is malformed",
                )
            },
            io::Error::from,
        )
    })
}
// Inspect bounded diagnostics only to classify retry policy; never surface remote stderr.
fn probe_exit_kind(code: Option<i32>, stderr: &[u8]) -> io::ErrorKind {
    if code == Some(PROGRAM_NOT_FOUND) {
        return io::ErrorKind::NotFound;
    }
    let diagnostic = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    if [
        "permission denied",
        "authentication failed",
        "host key verification failed",
        "remote host identification has changed",
        "no matching host key",
        "bad configuration",
        "bad owner or permissions",
        "too many authentication failures",
        "no supported authentication",
    ]
    .iter()
    .any(|text| diagnostic.contains(text))
    {
        io::ErrorKind::PermissionDenied
    } else {
        io::ErrorKind::ConnectionRefused
    }
}

#[cfg(windows)]
#[path = "machine_ssh/windows.rs"]
mod platform;
#[cfg(not(windows))]
#[path = "machine_ssh/unix.rs"]
mod platform;
pub use platform::{executable, execute};

use orchbee_local_runtime::{CommandFailure as Failure, CommandOutput as Output};

#[cfg(test)]
mod tests;
