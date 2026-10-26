//! Bounded OpenSSH preparation through the host process port. User SSH configuration remains authoritative.
use super::{Context, Error, Remote};
use crate::machines::Draft;
use orchbee_local_runtime::CommandFailure;
use serde_json::Value;
use std::path::Path;

use crate::machine_ssh::protocols;
pub(super) use crate::machine_ssh::{Client, command, literal, program};
pub(super) fn prepare(
    draft: &Draft,
    context: &mut Context,
    remote: &mut Remote,
) -> Result<(), Error> {
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
) -> Result<(), Error> {
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
    server(&ready, &client)?;
    *remote = Remote::Ready;
    Ok(())
}
pub(super) fn client(value: &Value) -> Result<Client, Error> {
    crate::machine_ssh::client(value).map_err(|error| match error {
        crate::machine_ssh::AdmissionError::Protocol => Error::RemoteProtocol,
        crate::machine_ssh::AdmissionError::Incompatible => Error::RemoteIncompatible,
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
