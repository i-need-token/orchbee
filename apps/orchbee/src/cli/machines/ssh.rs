//! Bounded OpenSSH preparation through the host process port. User SSH configuration remains authoritative.
use super::{Context, Error, Remote};
use crate::machines::Draft;
use orchbee_local_runtime::CommandFailure;
use serde_json::Value;
use std::path::Path;

pub(super) use crate::machine_ssh::{Client, command, literal, program};
/// Prepares the remote Session and returns the remote product version, which is shown and never
/// compared.
pub(super) fn prepare(
    draft: &Draft,
    context: &mut Context,
    remote: &mut Remote,
) -> Result<String, Error> {
    context.check()?;
    let ssh = crate::machine_ssh::executable().map_err(|_| Error::SshUnavailable)?;
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
) -> Result<String, Error> {
    *remote = Remote::Unchanged;
    let probe = call(None, "client")?;
    let client = client(&probe)?;
    *remote = Remote::Unchanged;
    *remote = Remote::PossiblyStarted;
    let started = call(Some(&client.executable), "start")?;
    if started["schema"] != "orchbee.server.start"
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
    server(&ready)?;
    *remote = Remote::Ready;
    Ok(client.version)
}
pub(super) fn client(value: &Value) -> Result<Client, Error> {
    crate::machine_ssh::client(value).map_err(|error| match error {
        crate::machine_ssh::AdmissionError::Protocol => Error::RemoteProtocol,
        crate::machine_ssh::AdmissionError::Unsupported => Error::RemoteIncompatible,
        crate::machine_ssh::AdmissionError::OtherEnvironment => Error::RemoteOtherEnvironment,
    })
}
/// The remote's own Client reports its Session running with a ready Server. Versions are not
/// compared: the remote Session judges a later tunnel connection through the handshake.
fn server(value: &Value) -> Result<(), Error> {
    if value["schema"] != "orchbee.status.server" || value["version"] != 1 {
        return Err(Error::RemoteProtocol);
    }
    let server = &value["result"]["server"];
    let ready = server["processes"].as_array().is_some_and(|processes| {
        processes
            .iter()
            .any(|process| process["role"] == "server" && process["state"] == "ready")
    });
    if server["state"] != "running" || !ready {
        return Err(Error::RemoteNotReady);
    }
    Ok(())
}
pub(super) fn exchange(
    ssh: &Path,
    target: &str,
    remote: &str,
    context: &mut Context,
) -> Result<Value, Error> {
    context.check()?;
    let result =
        crate::machine_ssh::execute(command(ssh, target, remote), Some(context.deadline), || {
            context.interrupted()
        })
        .map_err(|_| Error::SshUnavailable)?;
    match result.failure {
        Some(CommandFailure::Cancelled) => return Err(Error::Cancelled),
        Some(CommandFailure::Timeout) => return Err(Error::Timeout),
        Some(CommandFailure::PhysicalUnknown) => return Err(Error::PhysicalUnknown),
        Some(CommandFailure::SpawnUnavailable) => return Err(Error::SshUnavailable),
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
