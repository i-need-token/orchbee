//! Bounded Windows OpenSSH preparation. User OpenSSH configuration remains authoritative.
use super::{Context, Error, Remote};
use crate::machines::Draft;
use base64::Engine as _;
use orchbee_local_site_runtime::{WindowsCommandFailure, run_windows_command};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    process::Command,
};

struct Client {
    executable: String,
    cohort: String,
}
pub(super) fn prepare(
    draft: &Draft,
    context: &mut Context,
    remote: &mut Remote,
) -> Result<(), Error> {
    context.check()?;
    let ssh = PathBuf::from(winsafe::GetSystemDirectory().map_err(|_| Error::SshUnavailable)?)
        .join("OpenSSH/ssh.exe");
    if !ssh.is_file() {
        return Err(Error::SshUnavailable);
    }
    prepare_session(remote, |path, action| {
        exchange(
            &ssh,
            &draft.target,
            &program(path, draft.session.as_str(), action),
            context,
        )
    })
}
fn prepare_session(
    remote: &mut Remote,
    mut call: impl FnMut(Option<&str>, &str) -> Result<Value, Error>,
) -> Result<(), Error> {
    *remote = Remote::Unchanged;
    let probe = call(None, "client")?;
    let client = client(&probe)?;
    *remote = Remote::Unchanged;
    *remote = Remote::PossiblyStarted;
    let started = call(Some(&client.executable), "start")?;
    if started["schema"] != "orchbee.site.start"
        || started["version"] != 1
        || !matches!(
            started["result"]["outcome"].as_str(),
            Some("started" | "already-running")
        )
    {
        return Err(Error::RemoteNotReady);
    }
    // The first SSH connection has settled. Read readiness using a fresh connection.
    let ready = call(Some(&client.executable), "server")?;
    server(&ready, &client)?;
    *remote = Remote::Ready;
    Ok(())
}
fn client(value: &Value) -> Result<Client, Error> {
    if value["schema"] != "orchbee.status.client" || value["version"] != 2 {
        return Err(Error::RemoteProtocol);
    }
    let r = &value["result"];
    if r["version"] != env!("CARGO_PKG_VERSION")
        || r["platform"] != "windows"
        || r["architecture"] != std::env::consts::ARCH
        || !protocols(&r["protocols"])
        || r["cohort"]["state"] != "available"
    {
        return Err(Error::RemoteIncompatible);
    }
    let cohort = r["cohort"]["identity"]
        .as_str()
        .filter(|s| hex64(s))
        .ok_or(Error::RemoteProtocol)?;
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
        .ok_or(Error::RemoteProtocol)?;
    Ok(Client {
        executable: executable.to_owned(),
        cohort: cohort.to_owned(),
    })
}
fn server(value: &Value, client: &Client) -> Result<(), Error> {
    if value["schema"] != "orchbee.status.server" || value["version"] != 1 {
        return Err(Error::RemoteProtocol);
    }
    let r = &value["result"];
    if r["server"]["state"] != "running"
        || r["server"]["stopping"] != false
        || r["server"]["version"] != env!("CARGO_PKG_VERSION")
        || r["server"]["cohort"].as_str() != Some(client.cohort.as_str())
        || r["localCohort"]["identity"].as_str() != Some(client.cohort.as_str())
        || r["compatibility"]["compatible"] != true
        || !protocols(&r["server"]["protocols"])
    {
        return Err(Error::RemoteNotReady);
    }
    Ok(())
}
fn protocols(value: &Value) -> bool {
    value["admission"] == orchbee_local_client_protocol::PROTOCOL_VERSION
        && value["terminal"] == orchbee_client_server_protocol::PROTOCOL_VERSION
}
fn hex64(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}
fn program(executable: Option<&str>, session: &str, action: &str) -> String {
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
fn command(ssh: &Path, target: &str, remote: &str) -> Command {
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
fn exchange(ssh: &Path, target: &str, remote: &str, context: &mut Context) -> Result<Value, Error> {
    context.check()?;
    let result = run_windows_command(command(ssh, target, remote), Some(context.deadline), || {
        context.interrupted()
    });
    match result.failure {
        Some(WindowsCommandFailure::Cancelled) => return Err(Error::Cancelled),
        Some(WindowsCommandFailure::Timeout) => return Err(Error::Timeout),
        Some(WindowsCommandFailure::PhysicalUnknown) => return Err(Error::PhysicalUnknown),
        Some(WindowsCommandFailure::SpawnUnavailable) => return Err(Error::SshUnavailable),
        Some(_) => return Err(Error::SshFailed),
        None => {}
    }
    if result.exit_code != Some(0) {
        return Err(Error::SshFailed);
    }
    if result.stdout_truncated {
        return Err(Error::RemoteProtocol);
    }
    serde_json::from_slice(&result.stdout).map_err(|_| Error::RemoteProtocol)
}
#[cfg(test)]
mod tests;
