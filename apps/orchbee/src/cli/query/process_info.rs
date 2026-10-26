//! One read-only viewer query against an exact Terminal. Host facts
//! remain separate from snapshot routing and never grant input authority.

use super::{
    AttachmentId, BorrowedFd, Failure, FailureKind, HostEpoch, Instant, LocalClient, Snapshot,
    Target, TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest, ViewerId,
    attach_viewer_cancellable, classify_viewer, classify_viewer_rejected, obcs_request_id,
    rows_details, viewer_protocol_invalid, viewer_unavailable,
};
use super::{NETWORK_BUDGET, Projection, deadline_after, snapshot_session_until};
use crate::cli::caller_context::PanelTarget;
use crate::cli::results;
use orchbee_terminal_protocol::{
    ConnectionControl, MessageRole, PayloadEncoding, ProcessInfoControl, TerminalProcessInfo,
    decode_connection_control, decode_process_info_control, decode_transport_envelope,
};
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
    let (row, viewer, attachment, epoch, _) =
        attach_viewer_cancellable(client, deadline, snapshot, terminal, wake)?;
    // All fallible work after attachment returns here; even request
    // construction failure reaches detach. Dropping the session is the fallback.
    let outcome = read_attached(
        client,
        deadline,
        snapshot,
        row,
        (&viewer, attachment, epoch),
        wake,
    );
    if let Ok(request_id) = obcs_request_id() {
        let _ = client.submit_terminal_viewer_cancellable(
            TerminalViewerRequest::new(
                request_id,
                TerminalViewerAction::Detach {
                    attachment_id: viewer,
                },
            ),
            Some(deadline),
            wake,
        );
    }
    outcome
}

pub(super) fn read_attached(
    client: &mut LocalClient,
    deadline: Instant,
    snapshot: &Snapshot,
    row: results::TerminalRow,
    lease: (&ViewerId, AttachmentId, HostEpoch),
    wake: Option<BorrowedFd<'_>>,
) -> Result<(Value, results::PanelProcessInfoDoc), Failure> {
    let (viewer, attachment, epoch) = lease;
    let request_id = obcs_request_id()?;
    let request = TerminalViewerRequest::new(
        request_id.clone(),
        TerminalViewerAction::ReadProcessInfo {
            attachment_id: viewer.clone(),
        },
    );
    let result = client
        .submit_terminal_viewer_cancellable(request, Some(deadline), wake)
        .map_err(classify_viewer)?;
    if Instant::now() >= deadline {
        return Err(Failure::new(FailureKind::Timeout));
    }
    let record = match result.outcome() {
        TerminalViewerOutcome::Record(record) => record,
        TerminalViewerOutcome::Rejected { reason } => {
            return Err(classify_viewer_rejected(*reason));
        }
        TerminalViewerOutcome::Unavailable { .. } => return Err(viewer_unavailable()),
        TerminalViewerOutcome::Attached { .. }
        | TerminalViewerOutcome::Empty { .. }
        | TerminalViewerOutcome::FeedbackAccepted { .. }
        | TerminalViewerOutcome::Detached { .. }
        | TerminalViewerOutcome::RowsRejected { .. } => return Err(viewer_protocol_invalid()),
    };
    let (info, host_request_id) = decode(record, attachment, epoch)?;
    let source = json!({
        "kind": "terminal-process-query",
        "sessionId": snapshot.session_id.as_str(),
        "snapshotRevision": snapshot.revision,
        "requestId": request_id.as_str(),
        "hostRequestId": host_request_id,
        "hostEpoch": epoch.to_text(),
        "attachmentId": attachment.to_text(),
        "hostTerminalId": info.terminal_id.to_text(),
    });
    Ok((
        source,
        results::PanelProcessInfoDoc {
            terminal: row,
            info,
        },
    ))
}

pub(crate) use crate::agent_runtime::startup::primary_terminal;

fn decode(
    record: &[u8],
    attachment: AttachmentId,
    epoch: HostEpoch,
) -> Result<(TerminalProcessInfo, String), Failure> {
    if record.len() > 64 + orchbee_terminal_protocol::MAXIMUM_CONTROL_JSON_BYTES {
        return Err(viewer_protocol_invalid());
    }
    let envelope = decode_transport_envelope(record).map_err(|_| viewer_protocol_invalid())?;
    if envelope.protocol != orchbee_terminal_protocol::CURRENT_PROTOCOL_VERSION
        || envelope.role != MessageRole::Response
        || envelope.optional_message
        || envelope.payload_encoding != PayloadEncoding::ControlJson
        || envelope.host_epoch != Some(epoch)
    {
        return Err(viewer_protocol_invalid());
    }
    // The Server owns the Worker/Host correlation and physical Terminal binding.
    // That correlation differs from the CLI's OBCS request identity.
    let correlation = envelope
        .correlation_id
        .ok_or_else(viewer_protocol_invalid)?;
    if let Ok(ProcessInfoControl::ProcessInfo(info)) =
        decode_process_info_control(envelope.kind, envelope.role, envelope.payload)
    {
        if info.attachment_id != attachment {
            return Err(viewer_protocol_invalid());
        }
        return Ok((info, correlation.to_text()));
    }
    if let Ok(ConnectionControl::TypedFailure(failure)) =
        decode_connection_control(envelope.kind, envelope.role, envelope.payload)
    {
        let kind = match failure.code.as_str() {
            "limit-exceeded" => FailureKind::Rejected,
            "missing-resource"
            | "process-sample-unsupported"
            | "process-sample-changed"
            | "process-sample-unavailable" => FailureKind::ServiceUnavailable,
            _ => return Err(viewer_protocol_invalid()),
        };
        return Err(Failure::new(kind)
            .failure_schema("orchbee.cli.failure.process-info")
            .details(rows_details("read-process-info", &failure.code)));
    }
    Err(viewer_protocol_invalid())
}

#[cfg(test)]
mod tests;
