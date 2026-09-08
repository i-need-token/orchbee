use std::{error::Error, num::NonZeroU64, os::unix::net::UnixStream, time::Duration};

use orchbee_capability_node_protocol::{
    TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest, TerminalViewerResult,
    ViewerRejection, ViewerSnapshotReason,
};
use orchbee_local_site_protocol::WorkerId;
use orchbee_terminal_protocol::{
    AttachTerminal, AttachmentCapabilityState, AttachmentControl, AttachmentId, AttachmentTarget,
    ConnectionControl, CorrelationId, FrameApplied, NeedSnapshot, PayloadEncoding,
    ProjectionDemand, ProjectionDemandControl, ProjectionFeedbackControl, SetProjectionDemand,
    SnapshotReason, TerminalId, decode_attachment_control, decode_connection_control,
    decode_projection_demand_control, decode_projection_feedback_control,
    decode_transport_envelope, encode_attachment_control, encode_projection_demand_control,
    encode_projection_feedback_control,
};

use crate::host_connection::{HostBrokerConnection, HostExchange, exchange};
use crate::worker_identity::fresh_identity;

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
