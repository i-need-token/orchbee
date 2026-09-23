use orchbee_local_ipc::LocalStream as UnixStream;
use std::{error::Error, io, num::NonZeroU64, time::Duration};

use orchbee_capability_node_protocol::{
    MAXIMUM_VIEWER_RECORD_BYTES, TerminalViewerAction, TerminalViewerOutcome,
    TerminalViewerRequest, TerminalViewerResult, ViewerHistoryDemand, ViewerProjectionDemand,
    ViewerRejection, ViewerRowsRejectedReason, ViewerSnapshotReason, ViewportRowOrigin,
};
use orchbee_local_site_protocol::WorkerId;
use orchbee_terminal_protocol::{
    AttachTerminal, AttachmentCapabilityState, AttachmentControl, AttachmentId, AttachmentTarget,
    CURRENT_PROTOCOL_VERSION, ConnectionControl, CorrelationId, EncodedRowAccessControl, Envelope,
    FrameApplied, HistoryControl, HistoryDemand, HistoryObserved, MessageRole, NeedSnapshot,
    PayloadEncoding, ProjectionDemand, ProjectionDemandControl, ProjectionFeedbackControl,
    ReadRows, RowAccessControl, RowRevisionPolicy, RowWindowDetail, RowWindowOrigin,
    RowWindowPurpose, RowsApplied, SetHistoryDemand, SetProjectionDemand, SnapshotReason,
    TRANSPORT_ENVELOPE_HEADER_BYTES, TerminalId, decode_attachment_control,
    decode_connection_control, decode_history_control, decode_projection_demand_control,
    decode_projection_feedback_control, decode_row_access_control, decode_transport_envelope,
    encode_attachment_control, encode_history_control, encode_projection_demand_control,
    encode_projection_feedback_control, encode_row_access_control, encode_transport_envelope,
    message_contract,
};

use crate::host_connection::{HostBrokerConnection, HostExchange, exchange};
use crate::worker_identity::fresh_identity;

mod process_info;

fn invalid(error: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, error.to_string())
}

pub(crate) fn execute(
    request: TerminalViewerRequest,
    worker_id: WorkerId,
    generation: NonZeroU64,
    host: &HostBrokerConnection,
    server: &UnixStream,
) -> Result<TerminalViewerResult, Box<dyn Error>> {
    let outcome = if request.worker_id.as_bytes() != worker_id.as_bytes() {
        TerminalViewerOutcome::Rejected(ViewerRejection::WrongWorker)
    } else if request.worker_generation.get() != generation.get() {
        TerminalViewerOutcome::Rejected(ViewerRejection::StaleWorkerGeneration)
    } else {
        apply(request, host, server)?
    };
    Ok(TerminalViewerResult {
        request_id: request.request_id,
        attachment_id: request.attachment_id,
        outcome,
    })
}

// Queue saturation retires only that viewer. Cleanup runs before the next Server
// operation, through the same Host writer; it cannot retain another output frame.
pub(crate) fn cleanup_closed(
    host: &HostBrokerConnection,
    server: &UnixStream,
) -> Result<(), Box<dyn Error>> {
    let ids = host.viewers.borrow().closed_ids();
    for bytes in ids {
        let id = orchbee_capability_node_protocol::OperationId::from_bytes(bytes)?;
        crate::terminal_control_operation::release_viewer(host, server, id)?;
        let target = AttachmentTarget {
            attachment_id: AttachmentId::from_bytes(bytes)?,
        };
        let command = encode_attachment_control(&AttachmentControl::DetachTerminal(target))?;
        let correlation = CorrelationId::from_bytes(fresh_identity()?)?;
        let response = exchange(
            host,
            server,
            correlation,
            command.kind,
            &command.payload,
            PayloadEncoding::ControlJson,
        )?;
        let envelope = decode_transport_envelope(&response)?;
        if decode_attachment_control(envelope.kind, envelope.role, envelope.payload)?
            != AttachmentControl::DetachTerminalAccepted(target)
        {
            return Err(invalid("Host did not clean up the closed viewer").into());
        }
        host.viewers.borrow_mut().remove(id);
    }
    Ok(())
}

fn apply(
    request: TerminalViewerRequest,
    host: &HostBrokerConnection,
    server: &UnixStream,
) -> Result<TerminalViewerOutcome, Box<dyn Error>> {
    let id = AttachmentId::from_bytes(request.attachment_id.as_bytes())?;
    let correlation = CorrelationId::from_bytes(request.request_id.as_bytes())?;
    match request.action {
        TerminalViewerAction::Attach {
            host_epoch,
            terminal_id,
        } => {
            if host_epoch.as_bytes() != *host.host_epoch().as_bytes() {
                return Ok(TerminalViewerOutcome::Rejected(
                    ViewerRejection::StaleHostEpoch,
                ));
            }
            if host.disposed_terminal(terminal_id) || host.disposed_viewer(request.attachment_id) {
                return Ok(TerminalViewerOutcome::Rejected(
                    ViewerRejection::AttachmentUnavailable,
                ));
            }
            if host.controllers.borrow().contains(request.attachment_id) {
                return Ok(TerminalViewerOutcome::Rejected(
                    ViewerRejection::AttachmentAlreadyExists,
                ));
            }
            let admission = host.viewers.borrow_mut().admit(&request);
            if let Err(reason) = admission {
                return Ok(TerminalViewerOutcome::Rejected(reason));
            }
            let terminal_id = TerminalId::from_bytes(terminal_id.as_bytes())?;
            let capabilities = AttachmentCapabilityState {
                read: true,
                input: false,
                resize: false,
            };
            let command =
                encode_attachment_control(&AttachmentControl::AttachTerminal(AttachTerminal {
                    attachment_id: id,
                    capabilities,
                    request_resize_lease: false,
                    terminal_id,
                }))?;
            let response = exchange(
                host,
                server,
                correlation,
                command.kind,
                &command.payload,
                PayloadEncoding::ControlJson,
            )?;
            let envelope = decode_transport_envelope(&response)?;
            if matches!(
                decode_connection_control(envelope.kind, envelope.role, envelope.payload),
                Ok(ConnectionControl::TypedFailure(_))
            ) {
                host.viewers.borrow_mut().remove(request.attachment_id);
                return Ok(TerminalViewerOutcome::Rejected(
                    ViewerRejection::HostRejected,
                ));
            }
            let AttachmentControl::AttachmentCapabilities(accepted) =
                decode_attachment_control(envelope.kind, envelope.role, envelope.payload)?
            else {
                return Err(invalid("Host did not acknowledge viewer attachment").into());
            };
            if accepted.attachment_id != id
                || accepted.terminal_id != terminal_id
                || accepted.capabilities != capabilities
                || accepted.resize_lease
            {
                return Err(invalid("Host viewer capabilities exceed the Server grant").into());
            }
            let demand = SetProjectionDemand {
                attachment_id: id,
                demand: ProjectionDemand::Active,
            };
            let command = encode_projection_demand_control(
                &ProjectionDemandControl::SetProjectionDemand(demand),
            )?;
            let response = exchange(
                host,
                server,
                correlation,
                command.kind,
                &command.payload,
                PayloadEncoding::ControlJson,
            )?;
            let envelope = decode_transport_envelope(&response)?;
            let ProjectionDemandControl::ProjectionDemandAccepted(accepted) =
                decode_projection_demand_control(envelope.kind, envelope.role, envelope.payload)?
            else {
                return Err(invalid("Host did not activate viewer projection").into());
            };
            if accepted.attachment_id != id || accepted.demand != ProjectionDemand::Active {
                return Err(invalid("Host projection demand acknowledgement disagrees").into());
            }
            Ok(TerminalViewerOutcome::Attached)
        }
        TerminalViewerAction::Poll => Ok(host.viewers.borrow_mut().poll(request.attachment_id)),
        TerminalViewerAction::ReadProcessInfo => {
            process_info::read(host, server, request.attachment_id, correlation)
        }
        TerminalViewerAction::ReadRows {
            start_row,
            row_count,
            revision,
            generation,
            request_sequence,
            maximum_bytes,
        } => read_rows(
            host,
            server,
            correlation,
            request.attachment_id,
            ReadRows {
                attachment_id: id,
                detail: RowWindowDetail::Text,
                generation,
                maximum_bytes,
                origin: RowWindowOrigin::Absolute { row: start_row },
                purpose: RowWindowPurpose::Buffer,
                request_sequence,
                revision,
                revision_policy: RowRevisionPolicy::Exact,
                row_count,
            },
        ),
        TerminalViewerAction::ReadViewportRows {
            origin,
            row_count,
            revision,
            generation,
            request_sequence,
            maximum_bytes,
        } => read_rows(
            host,
            server,
            correlation,
            request.attachment_id,
            ReadRows {
                attachment_id: id,
                detail: RowWindowDetail::Text,
                generation,
                maximum_bytes,
                origin: match origin {
                    ViewportRowOrigin::Tail { rows_before } => {
                        RowWindowOrigin::Tail { rows_before }
                    }
                    ViewportRowOrigin::Committed { delta } => RowWindowOrigin::Committed { delta },
                },
                purpose: RowWindowPurpose::Viewport,
                request_sequence,
                revision,
                revision_policy: RowRevisionPolicy::CurrentView,
                row_count,
            },
        ),
        TerminalViewerAction::Detach => {
            if !host.viewers.borrow().contains(request.attachment_id) {
                if host.disposed_viewer(request.attachment_id) {
                    return Ok(TerminalViewerOutcome::Detached);
                }
                return Ok(TerminalViewerOutcome::Rejected(
                    ViewerRejection::AttachmentUnavailable,
                ));
            }
            crate::terminal_control_operation::release_viewer(host, server, request.attachment_id)?;
            let target = AttachmentTarget { attachment_id: id };
            let command = encode_attachment_control(&AttachmentControl::DetachTerminal(target))?;
            let response = exchange(
                host,
                server,
                correlation,
                command.kind,
                &command.payload,
                PayloadEncoding::ControlJson,
            )?;
            let envelope = decode_transport_envelope(&response)?;
            if decode_attachment_control(envelope.kind, envelope.role, envelope.payload)?
                != AttachmentControl::DetachTerminalAccepted(target)
            {
                return Err(invalid("Host did not detach the exact viewer").into());
            }
            host.viewers.borrow_mut().remove(request.attachment_id);
            Ok(TerminalViewerOutcome::Detached)
        }
        TerminalViewerAction::SetProjectionDemand { demand } => {
            if !host.viewers.borrow().open(request.attachment_id) {
                return Ok(TerminalViewerOutcome::Rejected(
                    ViewerRejection::AttachmentUnavailable,
                ));
            }
            let demand = match demand {
                ViewerProjectionDemand::Active => ProjectionDemand::Active,
                ViewerProjectionDemand::Paused => ProjectionDemand::Paused,
            };
            let command = encode_projection_demand_control(
                &ProjectionDemandControl::SetProjectionDemand(SetProjectionDemand {
                    attachment_id: id,
                    demand,
                }),
            )?;
            let Some(response) =
                ack_round_trip(host, server, correlation, command.kind, &command.payload)?
            else {
                return Ok(TerminalViewerOutcome::Rejected(
                    ViewerRejection::HostRejected,
                ));
            };
            let envelope = decode_transport_envelope(&response)?;
            let ProjectionDemandControl::ProjectionDemandAccepted(accepted) =
                decode_projection_demand_control(envelope.kind, envelope.role, envelope.payload)?
            else {
                return Err(
                    invalid("Host did not acknowledge the viewer projection demand").into(),
                );
            };
            if accepted.attachment_id != id || accepted.demand != demand {
                return Err(invalid("Host projection demand acknowledgement disagrees").into());
            }
            Ok(TerminalViewerOutcome::FeedbackAccepted)
        }
        TerminalViewerAction::SetHistoryDemand { demand } => {
            if !host.viewers.borrow().open(request.attachment_id) {
                return Ok(TerminalViewerOutcome::Rejected(
                    ViewerRejection::AttachmentUnavailable,
                ));
            }
            let demand = match demand {
                ViewerHistoryDemand::FollowingBottom => HistoryDemand::FollowingBottom,
                ViewerHistoryDemand::ViewingHistory => HistoryDemand::ViewingHistory,
            };
            let command =
                encode_history_control(&HistoryControl::SetHistoryDemand(SetHistoryDemand {
                    attachment_id: id,
                    demand,
                }))?;
            let Some(response) =
                ack_round_trip(host, server, correlation, command.kind, &command.payload)?
            else {
                return Ok(TerminalViewerOutcome::Rejected(
                    ViewerRejection::HostRejected,
                ));
            };
            let envelope = decode_transport_envelope(&response)?;
            let HistoryControl::HistoryDemandAccepted(accepted) =
                decode_history_control(envelope.kind, envelope.role, envelope.payload)?
            else {
                return Err(invalid("Host did not acknowledge the viewer history demand").into());
            };
            if accepted.attachment_id != id || accepted.demand != demand {
                return Err(invalid("Host history demand acknowledgement disagrees").into());
            }
            Ok(TerminalViewerOutcome::FeedbackAccepted)
        }
        TerminalViewerAction::RowsApplied {
            request_sequence,
            generation,
            revision,
        } => {
            if !host.viewers.borrow().open(request.attachment_id) {
                return Ok(TerminalViewerOutcome::Rejected(
                    ViewerRejection::AttachmentUnavailable,
                ));
            }
            let applied = RowsApplied {
                attachment_id: id,
                generation,
                request_sequence,
                revision,
            };
            let command =
                encode_row_access_control(&RowAccessControl::RowsAppliedRequest(applied))?;
            let Some(response) =
                ack_round_trip(host, server, correlation, command.kind, &command.payload)?
            else {
                return Ok(TerminalViewerOutcome::Rejected(
                    ViewerRejection::HostRejected,
                ));
            };
            let envelope = decode_transport_envelope(&response)?;
            if decode_row_access_control(envelope.kind, envelope.role, envelope.payload)?
                != RowAccessControl::RowsAppliedResponse(applied)
            {
                return Err(invalid("Host did not acknowledge the exact rows applied").into());
            }
            Ok(TerminalViewerOutcome::FeedbackAccepted)
        }
        TerminalViewerAction::HistoryObserved {
            sequence,
            generation,
            revision,
        } => {
            if !host.viewers.borrow().open(request.attachment_id) {
                return Ok(TerminalViewerOutcome::Rejected(
                    ViewerRejection::AttachmentUnavailable,
                ));
            }
            let observed = HistoryObserved {
                attachment_id: id,
                generation,
                revision,
                sequence,
            };
            let command =
                encode_history_control(&HistoryControl::HistoryObservedRequest(observed))?;
            let Some(response) =
                ack_round_trip(host, server, correlation, command.kind, &command.payload)?
            else {
                return Ok(TerminalViewerOutcome::Rejected(
                    ViewerRejection::HostRejected,
                ));
            };
            let envelope = decode_transport_envelope(&response)?;
            if decode_history_control(envelope.kind, envelope.role, envelope.payload)?
                != HistoryControl::HistoryObservedResponse(observed)
            {
                return Err(invalid("Host did not acknowledge the exact history observed").into());
            }
            Ok(TerminalViewerOutcome::FeedbackAccepted)
        }
        TerminalViewerAction::FrameApplied { .. } | TerminalViewerAction::NeedSnapshot { .. } => {
            // Drain pending events before changing feedback state. HostExchange's preflight
            // repeats this check to preserve the shared broker's no-pipelining boundary.
            let preflight = HostExchange::new(host, server, Duration::from_secs(1))
                .ok_or_else(|| invalid("invalid viewer budget"))?;
            preflight
                .prepare()
                .map_err(|_| invalid("viewer Host preflight failed"))?;
            let admission = host.viewers.borrow_mut().feedback(&request);
            if let Err(reason) = admission {
                return Ok(TerminalViewerOutcome::Rejected(reason));
            }
            let (command, expected) = match request.action {
                TerminalViewerAction::FrameApplied {
                    sequence,
                    generation,
                    revision,
                } => {
                    let feedback = FrameApplied {
                        attachment_id: id,
                        sequence,
                        generation,
                        revision,
                    };
                    (
                        ProjectionFeedbackControl::FrameAppliedRequest(feedback),
                        ProjectionFeedbackControl::FrameAppliedResponse(feedback),
                    )
                }
                TerminalViewerAction::NeedSnapshot { sequence, reason } => {
                    let feedback = NeedSnapshot {
                        attachment_id: id,
                        sequence,
                        reason: snapshot_reason(reason),
                    };
                    (
                        ProjectionFeedbackControl::NeedSnapshotRequest(feedback),
                        ProjectionFeedbackControl::NeedSnapshotResponse(feedback),
                    )
                }
                _ => unreachable!("feedback action was matched"),
            };
            let command = encode_projection_feedback_control(&command)?;
            let response = exchange(
                host,
                server,
                correlation,
                command.kind,
                &command.payload,
                PayloadEncoding::ControlJson,
            )?;
            let envelope = decode_transport_envelope(&response)?;
            if decode_projection_feedback_control(envelope.kind, envelope.role, envelope.payload)?
                != expected
            {
                return Err(invalid("Host did not acknowledge the exact viewer feedback").into());
            }
            Ok(TerminalViewerOutcome::FeedbackAccepted)
        }
    }
}

const fn snapshot_reason(reason: ViewerSnapshotReason) -> SnapshotReason {
    match reason {
        ViewerSnapshotReason::ActivityStale => SnapshotReason::ActivityStale,
        ViewerSnapshotReason::BaseMismatch => SnapshotReason::BaseMismatch,
        ViewerSnapshotReason::DecoderRejected => SnapshotReason::DecoderRejected,
        ViewerSnapshotReason::DimensionsMismatch => SnapshotReason::DimensionsMismatch,
        ViewerSnapshotReason::GenerationMismatch => SnapshotReason::GenerationMismatch,
        ViewerSnapshotReason::IdentityMismatch => SnapshotReason::IdentityMismatch,
        ViewerSnapshotReason::SequenceGap => SnapshotReason::SequenceGap,
        ViewerSnapshotReason::StoreReset => SnapshotReason::StoreReset,
    }
}

/// A row read needs an admitted, open viewer; unknown or closed attachments are
/// the existing unavailability, never a row rejection. The answer is either the
/// correlated row-window record or a typed failure mapped to a rows rejection.
fn read_rows(
    host: &HostBrokerConnection,
    server: &UnixStream,
    correlation: CorrelationId,
    attachment: orchbee_capability_node_protocol::OperationId,
    read: ReadRows,
) -> Result<TerminalViewerOutcome, Box<dyn Error>> {
    if !host.viewers.borrow().open(attachment) {
        return Ok(TerminalViewerOutcome::Rejected(
            ViewerRejection::AttachmentUnavailable,
        ));
    }
    // The effective budget is the smallest of the requested bound, the
    // negotiated row-window limit, and what the viewer record carrier can
    // hold after its envelope header.
    let carrier_payload =
        u32::try_from(MAXIMUM_VIEWER_RECORD_BYTES.saturating_sub(TRANSPORT_ENVELOPE_HEADER_BYTES))
            .unwrap_or(u32::MAX);
    let effective_maximum = read
        .maximum_bytes
        .min(host.negotiated_limits().row_window_bytes)
        .min(carrier_payload);
    let command = encode_row_access_control(&RowAccessControl::ReadRows(ReadRows {
        maximum_bytes: effective_maximum,
        ..read
    }))?;
    let response = exchange_rows(
        host,
        server,
        correlation,
        &command,
        TRANSPORT_ENVELOPE_HEADER_BYTES.saturating_add(effective_maximum as usize),
    )?;
    let envelope = decode_transport_envelope(&response)?;
    if envelope.payload_encoding == PayloadEncoding::RowWindow {
        // The bounded exchange already proved the response record fits
        // the effective budget; confirm the payload length itself.
        if envelope.payload.len() > effective_maximum as usize {
            return Err(invalid("Host row window exceeds its byte budget").into());
        }
        return Ok(TerminalViewerOutcome::Record(response));
    }
    let Ok(ConnectionControl::TypedFailure(failure)) =
        decode_connection_control(envelope.kind, envelope.role, envelope.payload)
    else {
        return Err(invalid("Host did not answer the row request").into());
    };
    Ok(TerminalViewerOutcome::RowsRejected(
        rows_rejection_reason(&failure.code)
            .ok_or_else(|| invalid("Host row rejection used an unknown reason"))?,
    ))
}

/// A control acknowledgement round trip answers either the control family's
/// response or a typed failure; the caller maps the failure to host-rejected.
fn ack_round_trip(
    host: &HostBrokerConnection,
    server: &UnixStream,
    correlation: CorrelationId,
    kind: u16,
    payload: &[u8],
) -> Result<Option<Vec<u8>>, Box<dyn Error>> {
    let response = exchange(
        host,
        server,
        correlation,
        kind,
        payload,
        PayloadEncoding::ControlJson,
    )?;
    let envelope = decode_transport_envelope(&response)?;
    if matches!(
        decode_connection_control(envelope.kind, envelope.role, envelope.payload),
        Ok(ConnectionControl::TypedFailure(_))
    ) {
        return Ok(None);
    }
    Ok(Some(response))
}

fn rows_rejection_reason(code: &str) -> Option<ViewerRowsRejectedReason> {
    Some(match code {
        "stale-revision" => ViewerRowsRejectedReason::StaleRevision,
        "stale-generation" => ViewerRowsRejectedReason::StaleGeneration,
        "history-evicted" => ViewerRowsRejectedReason::HistoryEvicted,
        "missing-resource" => ViewerRowsRejectedReason::MissingResource,
        "invalid-field" => ViewerRowsRejectedReason::InvalidField,
        "invalid-acknowledgement" => ViewerRowsRejectedReason::InvalidAcknowledgement,
        "limit-exceeded" => ViewerRowsRejectedReason::LimitExceeded,
        "resource-exhausted" => ViewerRowsRejectedReason::ResourceExhausted,
        "invalid-phase" => ViewerRowsRejectedReason::InvalidPhase,
        _ => return None,
    })
}

/// A row read accepts either the correlated RowWindow response or a control
/// TypedFailure; every other kind, encoding, or role is a Host violation.
fn exchange_rows(
    host: &HostBrokerConnection,
    server: &UnixStream,
    correlation: CorrelationId,
    command: &EncodedRowAccessControl,
    maximum_record_bytes: usize,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let expected = message_contract(command.kind)
        .and_then(|contract| contract.request)
        .map(|request| request.success_kind)
        .ok_or_else(|| invalid("Host row request kind has no response contract"))?;
    let record = encode_transport_envelope(&Envelope {
        correlation_id: Some(correlation),
        host_epoch: Some(host.host_epoch()),
        kind: command.kind,
        optional_message: false,
        payload: &command.payload,
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Request,
    })?;
    let mut exchange = HostExchange::new(host, server, Duration::from_secs(1))
        .ok_or_else(|| io::Error::other("invalid Host request budget"))?;
    exchange
        .prepare()
        .map_err(|_| io::Error::other("Host request preflight failed"))?;
    let record = exchange
        .round_trip_bounded(&record, maximum_record_bytes)
        .map_err(|_| io::Error::other("Host request exchange failed"))?;
    let envelope = decode_transport_envelope(&record)?;
    let row_window =
        envelope.kind == expected && envelope.payload_encoding == PayloadEncoding::RowWindow;
    let typed_failure = envelope.payload_encoding == PayloadEncoding::ControlJson
        && matches!(
            decode_connection_control(envelope.kind, envelope.role, envelope.payload),
            Ok(ConnectionControl::TypedFailure(_))
        );
    if envelope.correlation_id != Some(correlation)
        || envelope.host_epoch != Some(host.host_epoch())
        || envelope.role != MessageRole::Response
        || envelope.protocol != CURRENT_PROTOCOL_VERSION
        || envelope.optional_message
        || !(row_window || typed_failure)
    {
        return Err(io::Error::other("unexpected Host row response envelope").into());
    }
    Ok(record)
}
