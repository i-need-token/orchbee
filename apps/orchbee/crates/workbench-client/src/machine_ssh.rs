//! Shared Windows SSH command policy and installed-Client admission, without catalog or UI ownership.
//! Admission compares no product version, protocol, or cohort across machines: the remote Session
//! judges this Client through the handshake a hub Session stream carries.
use base64::Engine as _;
use serde_json::Value;
use std::{path::Path, process::Command};

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
/// handshake through a hub Session stream compares the exact environment.
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
/// The remote store folders under `%LOCALAPPDATA%\Programs` and the Client names the launch line
/// tries in order. A development or test build first looks for its own store and name and then for
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
/// The `ssh` of `machine add`'s preparation: batch mode, strict host keys that `ssh` never
/// rewrites in `known_hosts` by itself, no forwarding, no multiplexing and no askpass.
pub fn command(ssh: &Path, target: &str, remote: &str) -> Command {
    let mut command = with_options(
        ssh,
        target,
        remote,
        ["BatchMode=yes", "StrictHostKeyChecking=yes"],
    );
    command
        .env_remove("SSH_ASKPASS")
        .env("SSH_ASKPASS_REQUIRE", "never");
    command
}
/// The machine hub's carrier `ssh`: as [`command`], but without batch mode and with
/// `StrictHostKeyChecking=ask`, so `ssh` puts every login question to the askpass program the hub
/// sets and refuses a changed host key without asking.
pub fn carrier_command(ssh: &Path, target: &str, remote: &str) -> Command {
    with_options(
        ssh,
        target,
        remote,
        ["BatchMode=no", "StrictHostKeyChecking=ask"],
    )
}
/// `ssh` with `policy`, the options every Orchbee `ssh` shares, and `-- <target> <remote>`.
fn with_options(ssh: &Path, target: &str, remote: &str, policy: [&str; 2]) -> Command {
    let mut command = Command::new(ssh);
    command.arg("-T");
    for option in policy.into_iter().chain([
        // OpenSSH 8.5 and later otherwise add and remove a host's keys after a login.
        "UpdateHostKeys=no",
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
    ]) {
        command.args(["-o", option]);
    }
    command.args(["--", target, remote]);
    command
}

#[cfg(windows)]
#[path = "machine_ssh/windows.rs"]
mod platform;
#[cfg(not(windows))]
#[path = "machine_ssh/unix.rs"]
mod platform;
pub use platform::{executable, execute};

use orchbee_local_runtime::CommandOutput as Output;

#[cfg(test)]
mod tests;
