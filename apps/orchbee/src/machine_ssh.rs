//! Shared Windows SSH command policy and installed-Client admission, without catalog or UI ownership.
use base64::Engine as _;
use orchbee_local_site_runtime::{WindowsCommandFailure, run_windows_command};
use serde_json::Value;
use std::{
    io,
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

#[derive(Clone, Copy, Debug)]
pub(crate) enum AdmissionError {
    Protocol,
    Incompatible,
}
pub(crate) struct Client {
    pub(crate) executable: String,
    pub(crate) cohort: String,
}
pub(crate) fn client(value: &Value) -> Result<Client, AdmissionError> {
    if value["schema"] != "orchbee.status.client" || value["version"] != 2 {
        return Err(AdmissionError::Protocol);
    }
    let r = &value["result"];
    if r["version"] != env!("CARGO_PKG_VERSION")
        || r["platform"] != "windows"
        || r["architecture"] != std::env::consts::ARCH
        || !protocols(&r["protocols"])
        || r["cohort"]["state"] != "available"
    {
        return Err(AdmissionError::Incompatible);
    }
    let cohort = r["cohort"]["identity"]
        .as_str()
        .filter(|s| hex64(s))
        .ok_or(AdmissionError::Protocol)?;
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
                && Path::new(s)
                    .file_name()
                    .is_some_and(|n| n.eq_ignore_ascii_case("orchbee.exe"))
        })
        .ok_or(AdmissionError::Protocol)?;
    Ok(Client {
        executable: executable.to_owned(),
        cohort: cohort.to_owned(),
    })
}
pub(crate) fn protocols(value: &Value) -> bool {
    value["admission"] == orchbee_local_client_protocol::PROTOCOL_VERSION
        && value["terminal"] == orchbee_client_server_protocol::PROTOCOL_VERSION
}
fn hex64(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub(crate) fn literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}
pub(crate) fn program(executable: Option<&str>, session: &str, action: &str) -> String {
    let resolve = executable.map_or_else(
        || "$program=(Get-Command orchbee.exe -CommandType Application -ErrorAction Stop | Select-Object -First 1).Path".to_owned(),
        |path| format!("$program={}", literal(path)));
    let args = if action == "start" {
        "'site' 'start'".to_owned()
    } else {
        format!("'status' {}", literal(action))
    };
    let script = format!(
        "$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue';\
        [Console]::OutputEncoding=New-Object System.Text.UTF8Encoding($false);\
        $OutputEncoding=[Console]::OutputEncoding;\
        try{{{resolve};if(-not $program){{exit 125}};\
        & $program '--site' {} {args} '--format' 'json';exit $LASTEXITCODE}}catch{{exit 125}}",
        literal(session)
    );
    let encoded = base64::engine::general_purpose::STANDARD.encode(
        script
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>(),
    );
    format!("powershell.exe -NoLogo -NoProfile -NonInteractive -EncodedCommand {encoded}")
}
pub(crate) fn command(ssh: &Path, target: &str, remote: &str) -> Command {
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

/// Probe only; no Site activation, catalog write, or automatic retry.
pub(crate) fn probe(
    draft: &crate::machines::Draft,
    deadline: Instant,
    interrupted: impl FnMut() -> io::Result<bool>,
) -> io::Result<Client> {
    let ssh = executable()?;
    let result = run_windows_command(
        command(
            &ssh,
            &draft.target,
            &program(None, draft.session.as_str(), "client"),
        ),
        Some(deadline),
        interrupted,
    );
    match result.failure {
        Some(WindowsCommandFailure::Cancelled) => return Err(io::ErrorKind::Interrupted.into()),
        Some(WindowsCommandFailure::Timeout) => return Err(io::ErrorKind::TimedOut.into()),
        Some(WindowsCommandFailure::PhysicalUnknown) => {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        Some(WindowsCommandFailure::SpawnUnavailable) => return Err(io::ErrorKind::NotFound.into()),
        Some(WindowsCommandFailure::ExitFailure) => {
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
    client(&value).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "remote Client admission refused",
        )
    })
}
// Inspect bounded diagnostics only to classify retry policy; never surface remote stderr.
fn probe_exit_kind(code: Option<i32>, stderr: &[u8]) -> io::ErrorKind {
    if code == Some(125) {
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

/// The OS OpenSSH client, retaining the user's authentication/configuration owner.
pub(crate) fn executable() -> io::Result<PathBuf> {
    let path = PathBuf::from(winsafe::GetSystemDirectory().map_err(io::Error::other)?)
        .join("OpenSSH/ssh.exe");
    if !path.is_file() {
        return Err(io::ErrorKind::NotFound.into());
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn machine_probe_refusals_and_transient_network_errors_have_distinct_retry_policy() {
        assert_eq!(
            probe_exit_kind(Some(255), b"Permission denied (publickey)."),
            io::ErrorKind::PermissionDenied
        );
        assert_eq!(
            probe_exit_kind(Some(255), b"Host key verification failed."),
            io::ErrorKind::PermissionDenied
        );
        assert_eq!(probe_exit_kind(Some(125), b""), io::ErrorKind::NotFound);
        assert_eq!(
            probe_exit_kind(Some(255), b"Connection timed out"),
            io::ErrorKind::ConnectionRefused
        );
    }
}
