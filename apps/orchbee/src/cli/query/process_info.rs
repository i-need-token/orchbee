//! One read-only viewer query against an exact Terminal. Host facts
//! remain separate from snapshot routing and never grant input authority.

use super::{
    AttachmentId, BorrowedFd, Failure, FailureKind, HostEpoch, Instant, LocalClient, Snapshot,
    Target, ViewerId, classify_viewer, classify_viewer_rejected, rows_details,
    viewer_protocol_invalid, viewer_unavailable,
};
use super::{NETWORK_BUDGET, Projection, deadline_after, snapshot_session_until};
use crate::cli::caller_context::PanelTarget;
use crate::cli::results;
use crate::local_client::MetadataExchangeReason;
use orchbee_workbench_protocol::PanelAgentFailure;
use serde_json::{Value, json};

pub(crate) fn panel_process_info(target: Option<&Target>) -> Result<(Value, Projection), Failure> {
    crate::cli::execution::admit(crate::cli::execution::Capability::CanonicalPanelProcess)?;
    let deadline = deadline_after(NETWORK_BUDGET)?;
    let prepared = PanelTarget::prepare(target, |name| std::env::var_os(name))?;
    let (mut client, deadline, snapshot) = snapshot_session_until(deadline)?;
    let snapshot_source = results::snapshot_source(&snapshot);
    let outcome = read_on_session(&mut client, deadline, &snapshot, prepared);
    drop(client);
    outcome.map_err(|error| error.source(snapshot_source))
}

fn read_on_session(
    client: &mut LocalClient,
    deadline: Instant,
    snapshot: &Snapshot,
    prepared: PanelTarget,
) -> Result<(Value, Projection), Failure> {
    let target = prepared.resolve(snapshot)?;
    let terminal = primary_terminal(snapshot, &target)?;
    let (source, document) = read_terminal_processes(client, deadline, snapshot, &terminal)?;
    Ok((source, Projection::PanelProcessInfo(document)))
}

/// Read the Server's identity result and run anchor for the sampled Terminal after its process
/// sample. The two are separate observations; a different Host Terminal fails. The reply's
/// Workbench revision is no fence: the identity belongs to the sampled Terminal, and each command
/// keeps its own snapshot fences.
///
/// # Errors
/// Map transport and Server failures; a changed Host Terminal is `workbench-changed`.
fn read_identity(
    client: &mut LocalClient,
    deadline: Instant,
    snapshot: &Snapshot,
    row: &results::TerminalRow,
    info: &orchbee_terminal_protocol::TerminalProcessInfo,
    wake: Option<BorrowedFd<'_>>,
) -> Result<crate::cli::processes::ServerIdentity, Failure> {
    use orchbee_workbench_protocol::{
        PanelAgentAction, PanelAgentOutcome, PanelAgentRequest, TerminalPresentationId,
    };
    let presentation_id = TerminalPresentationId::parse(&row.presentation_id)
        .map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    let request = PanelAgentRequest {
        protocol_version: snapshot.protocol_version,
        request_id: super::request_id()?,
        session_id: snapshot.session_id.clone(),
        action: PanelAgentAction::Identify { presentation_id },
    };
    let reply = client
        .panel_agent(request, deadline, wake)
        .map_err(|error| Failure::new(metadata_failure(error.reason)))?;
    if Instant::now() >= deadline {
        return Err(Failure::new(FailureKind::Timeout));
    }
    match reply.outcome {
        PanelAgentOutcome::Identified {
            binding,
            identity,
            anchor,
            ..
        } => {
            if binding.host_terminal_id.as_str() != info.terminal_id.to_text() {
                return Err(super::workbench_failure("workbench-changed"));
            }
            Ok(crate::cli::processes::ServerIdentity { identity, anchor })
        }
        PanelAgentOutcome::Failed { reason } => Err(panel_agent_failure(reason)),
        _ => Err(viewer_protocol_invalid()),
    }
}

/// Resolve a CLI terminal against the admitted Snapshot and project one shared process observation.
///
/// # Errors
/// Preserve CLI selection errors and process-query failures; this form supplies no cancellation wake.
pub(crate) fn read_terminal_processes(
    client: &mut LocalClient,
    deadline: Instant,
    snapshot: &Snapshot,
    terminal: &Target,
) -> Result<(Value, results::PanelProcessInfoDoc), Failure> {
    read_terminal_processes_cancellable(client, deadline, snapshot, terminal, None)
}

/// Keep CLI target selection and document projection around the shared attach/read/detach operation,
/// followed by the Server identity read.
///
/// # Errors
/// Preserve selection, identity and process-query failures under the caller's deadline and wake.
pub(crate) fn read_terminal_processes_cancellable(
    client: &mut LocalClient,
    deadline: Instant,
    snapshot: &Snapshot,
    terminal: &Target,
    wake: Option<BorrowedFd<'_>>,
) -> Result<(Value, results::PanelProcessInfoDoc), Failure> {
    let row = results::terminal_select(snapshot, terminal)?;
    let binding = crate::agent_runtime::startup::binding::Binding::parse(
        snapshot.session_id.as_str(),
        &row.worker_id,
        &row.terminal_id,
    )
    .map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    let observed = crate::agent_runtime::startup::processes::read(client, deadline, &binding, wake)
        .map_err(classify_process)?;
    project(client, deadline, snapshot, row, observed, wake)
}

/// Project a process observation for an already selected row without taking viewer release ownership.
///
/// # Errors
/// Return the shared read's classified failure; the caller still owns the supplied viewer lease.
pub(super) fn read_attached(
    client: &mut LocalClient,
    deadline: Instant,
    snapshot: &Snapshot,
    row: results::TerminalRow,
    lease: (&ViewerId, AttachmentId, HostEpoch),
    wake: Option<BorrowedFd<'_>>,
) -> Result<(Value, results::PanelProcessInfoDoc), Failure> {
    let observed =
        crate::agent_runtime::startup::processes::read_attached(client, deadline, lease, wake)
            .map_err(classify_process)?;
    project(client, deadline, snapshot, row, observed, wake)
}

/// Pair the sample with the Server's identity of the same Terminal, read after it.
fn project(
    client: &mut LocalClient,
    deadline: Instant,
    snapshot: &Snapshot,
    row: results::TerminalRow,
    observed: crate::agent_runtime::startup::processes::Observation,
    wake: Option<BorrowedFd<'_>>,
) -> Result<(Value, results::PanelProcessInfoDoc), Failure> {
    let server = read_identity(client, deadline, snapshot, &row, &observed.info, wake)?;
    let source = json!({
        "kind": "terminal-process-query",
        "sessionId": snapshot.session_id.as_str(),
        "snapshotRevision": snapshot.revision,
        "requestId": observed.request.as_str(),
        "hostRequestId": observed.host_request,
        "hostEpoch": observed.epoch.to_text(),
        "attachmentId": observed.attachment.to_text(),
        "hostTerminalId": observed.info.terminal_id.to_text(),
    });
    Ok((
        source,
        results::PanelProcessInfoDoc {
            terminal: row,
            info: observed.info,
            server,
        },
    ))
}

/// Map shared process evidence to the existing viewer failures and process-info schema/Host codes.
pub(in crate::cli) fn classify_process(
    error: crate::agent_runtime::startup::processes::Error,
) -> Failure {
    use crate::agent_runtime::startup::processes::{Error, HostFailure};
    match error {
        Error::Identity => Failure::new(FailureKind::InternalFailure),
        Error::Exchange(error) => classify_viewer(error),
        Error::Timeout => Failure::new(FailureKind::Timeout),
        Error::Rejected(reason) => classify_viewer_rejected(reason),
        Error::Unavailable => viewer_unavailable(),
        Error::Protocol => viewer_protocol_invalid(),
        Error::Host(reason) => Failure::new(if reason == HostFailure::LimitExceeded {
            FailureKind::Rejected
        } else {
            FailureKind::ServiceUnavailable
        })
        .failure_schema("orchbee.cli.failure.process-info")
        .details(rows_details("read-process-info", reason.as_str())),
    }
}

/// Map a Server Panel Agent refusal to the generic CLI failure kind.
pub(in crate::cli) fn panel_agent_failure(reason: PanelAgentFailure) -> Failure {
    Failure::new(match reason {
        PanelAgentFailure::Denied => FailureKind::Denied,
        PanelAgentFailure::NotFound
        | PanelAgentFailure::SessionMismatch
        | PanelAgentFailure::BindingChanged => FailureKind::Rejected,
        PanelAgentFailure::Unbound
        | PanelAgentFailure::InvalidRequest
        | PanelAgentFailure::SourceLimit
        | PanelAgentFailure::HistoryLimit
        | PanelAgentFailure::ResourceLimit => FailureKind::ProtocolInvalid,
        PanelAgentFailure::RevisionExhausted | PanelAgentFailure::ClockOverflow => {
            FailureKind::InternalFailure
        }
        PanelAgentFailure::Unavailable => FailureKind::ServiceUnavailable,
        PanelAgentFailure::ResultTooLarge => FailureKind::OutputTooLarge,
    })
}

/// Map a Workbench exchange failure to the generic CLI failure kind.
pub(in crate::cli) fn metadata_failure(reason: MetadataExchangeReason) -> FailureKind {
    match reason {
        MetadataExchangeReason::ConnectionUnavailable => FailureKind::ServiceUnavailable,
        MetadataExchangeReason::Encoding => FailureKind::InternalFailure,
        MetadataExchangeReason::Deadline | MetadataExchangeReason::ResponseDeadline => {
            FailureKind::Timeout
        }
        MetadataExchangeReason::Correlation | MetadataExchangeReason::Protocol => {
            FailureKind::ProtocolInvalid
        }
        MetadataExchangeReason::NotSubmitted
        | MetadataExchangeReason::Write
        | MetadataExchangeReason::Read => FailureKind::TransportLoss,
    }
}

/// Resolve CLI Panel syntax to its exact primary terminal through the existing Panel projection.
///
/// # Errors
/// Preserve Panel selection errors and report an unbound primary as not found; no child fallback exists.
pub(crate) fn primary_terminal(snapshot: &Snapshot, target: &Target) -> Result<Target, Failure> {
    let Projection::PanelGet(panel) = results::project(
        &crate::invocation::QueryCommand::PanelGet(target.clone()),
        snapshot,
    )?
    else {
        return Err(Failure::new(FailureKind::InternalFailure));
    };
    panel
        .primary_presentation
        .terminal_id
        .map(Target::Id)
        .ok_or_else(|| {
            Failure::new(FailureKind::NotFound).details(crate::cli::target_details(
                crate::invocation::Resource::Panel,
                &panel.row.panel_id,
            ))
        })
}

#[cfg(test)]
mod tests;
