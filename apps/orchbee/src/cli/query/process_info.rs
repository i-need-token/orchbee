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
    read_terminal_processes(client, deadline, snapshot, &terminal)
        .map(|(source, document)| (source, Projection::PanelProcessInfo(document)))
}

pub(crate) fn read_terminal_processes(
    client: &mut LocalClient,
    deadline: Instant,
    snapshot: &Snapshot,
    terminal: &Target,
) -> Result<(Value, results::PanelProcessInfoDoc), Failure> {
    read_terminal_processes_cancellable(client, deadline, snapshot, terminal, None)
}

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
    Ok(project(snapshot, row, observed))
}

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
    Ok(project(snapshot, row, observed))
}

fn project(
    snapshot: &Snapshot,
    row: results::TerminalRow,
    observed: crate::agent_runtime::startup::processes::Observation,
) -> (Value, results::PanelProcessInfoDoc) {
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
    (
        source,
        results::PanelProcessInfoDoc {
            terminal: row,
            info: observed.info,
        },
    )
}

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
