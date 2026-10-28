//! Creation defaults from the running Session's published configuration snapshot. Creation never
//! reads `config.toml`; the Server owns file errors and keeps its last valid snapshot.
use super::{
    Failure, FailureKind,
    console_interrupt::{ArmedSignals, SignalWatch},
    query,
};
use crate::{
    invocation::CreationDirectory,
    local_client::{LocalClient, workbench::metadata_exchange::MetadataExchangeReason},
};
use orchbee_workbench_protocol::{
    ConfigurationAction, ConfigurationFailure, ConfigurationOutcome, ConfigurationRequest,
    ConfigurationSnapshot, LaunchOptions, ProtocolVersion, SessionId, WorkbenchDirectoryPath,
};
use std::time::Instant;

/// Replaces an omitted directory with `terminal.new_cwd`; any explicit policy skips the read.
pub(super) fn apply_directory(
    client: &mut LocalClient,
    session_id: &SessionId,
    deadline: Instant,
    signals: &mut ArmedSignals,
    directory: &mut CreationDirectory,
    options: &mut LaunchOptions,
) -> Result<(), Failure> {
    if *directory != CreationDirectory::Default {
        return Ok(());
    }
    let snapshot = current(client, session_id, deadline, signals)?;
    *directory = match snapshot.new_cwd.as_str() {
        "follow" => CreationDirectory::Follow,
        "home" => CreationDirectory::Home,
        "current" => CreationDirectory::Current,
        "workspace" => CreationDirectory::Workspace,
        path => {
            options.cwd = Some(admitted(path)?);
            CreationDirectory::Explicit
        }
    };
    Ok(())
}

/// `worktrees.directory`; `None` selects the sibling checkout.
pub(super) fn worktree_root(
    client: &mut LocalClient,
    session_id: &SessionId,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Option<WorkbenchDirectoryPath>, Failure> {
    current(client, session_id, deadline, signals)?
        .worktrees_directory
        .as_deref()
        .map(admitted)
        .transpose()
}

fn admitted(path: &str) -> Result<WorkbenchDirectoryPath, Failure> {
    WorkbenchDirectoryPath::parse(path).map_err(|_| Failure::new(FailureKind::ProtocolInvalid))
}

/// A read has no effect, so a reply lost after submission is plain transport loss.
fn current(
    client: &mut LocalClient,
    session_id: &SessionId,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<ConfigurationSnapshot, Failure> {
    signals.check_before(deadline)?;
    let request = ConfigurationRequest {
        protocol_version: ProtocolVersion,
        request_id: query::request_id()?,
        session_id: session_id.clone(),
        action: ConfigurationAction::Current,
    };
    let reply = match client.configuration(request, deadline, signals.poll_fd()) {
        Ok(reply) => reply,
        Err(error) => {
            signals.check_before(deadline)?;
            return Err(Failure::new(match error.reason {
                _ if error.submitted => FailureKind::TransportLoss,
                MetadataExchangeReason::ConnectionUnavailable => FailureKind::ServiceUnavailable,
                MetadataExchangeReason::Encoding => FailureKind::InternalFailure,
                MetadataExchangeReason::Deadline => FailureKind::Timeout,
                _ => FailureKind::TransportLoss,
            }));
        }
    };
    let kind = match reply.outcome {
        ConfigurationOutcome::Current { snapshot } => return Ok(snapshot),
        ConfigurationOutcome::Failed {
            reason: ConfigurationFailure::Denied,
            ..
        } => FailureKind::Denied,
        ConfigurationOutcome::Failed {
            reason: ConfigurationFailure::Unavailable | ConfigurationFailure::SessionMismatch,
            ..
        } => FailureKind::ServiceUnavailable,
        // A current read never reloads or reports busy, so no other reply is valid.
        ConfigurationOutcome::Failed { .. } | ConfigurationOutcome::Reloaded { .. } => {
            FailureKind::ProtocolInvalid
        }
    };
    Err(Failure::new(kind))
}
