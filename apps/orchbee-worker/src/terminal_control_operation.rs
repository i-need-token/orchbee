use crate::{
    host_connection::{HostBrokerConnection, exchange},
    worker_identity::fresh_identity,
};
use orchbee_capability_node_protocol::{
    ControlRejection as Rejection, OperationId, TerminalControlAction as Action,
    TerminalControlOutcome as Outcome, TerminalControlRequest, TerminalControlResult,
};
use orchbee_local_site_protocol::WorkerId;
use orchbee_terminal_protocol::{
    AttachTerminal, AttachmentCapabilityState, AttachmentControl, AttachmentId, AttachmentTarget,
    ConnectionControl, CorrelationId, InputMessage, PayloadEncoding, ResizeControl, ResizeTerminal,
    TerminalId, WriteInput, decode_attachment_control, decode_connection_control,
    decode_input_message, decode_resize_control, decode_transport_envelope,
    encode_attachment_control, encode_input_message, encode_resize_control,
};
use std::{error::Error, num::NonZeroU64, os::unix::net::UnixStream};
fn invalid(message: &str) -> Box<dyn Error> {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message).into()
}

pub(crate) fn execute(
    request: TerminalControlRequest,
    worker: WorkerId,
    generation: NonZeroU64,
    host: &HostBrokerConnection,
    server: &UnixStream,
) -> Result<TerminalControlResult, Box<dyn Error>> {
    let outcome = if request.worker_id.as_bytes() != worker.as_bytes() {
        Outcome::Rejected(Rejection::WrongWorker)
    } else if request.worker_generation.get() != generation.get() {
        Outcome::Rejected(Rejection::StaleWorkerGeneration)
    } else {
        apply(&request, host, server)?
    };
    Ok(TerminalControlResult {
        request_id: request.request_id,
        viewer_id: request.viewer_id,
        controller_id: request.controller_id,
        outcome,
    })
}
fn apply(
    request: &TerminalControlRequest,
    host: &HostBrokerConnection,
    server: &UnixStream,
) -> Result<Outcome, Box<dyn Error>> {
    if matches!(request.action, Action::Release) {
        let grant = host.controllers.borrow().for_viewer(request.viewer_id);
        return match grant {
            None => Ok(Outcome::Released),
            Some(grant) if grant.controller_id == request.controller_id => {
                release_viewer(host, server, request.viewer_id)?;
                Ok(Outcome::Released)
            }
            Some(_) => Ok(Outcome::Rejected(Rejection::NotController)),
        };
    }
    let Some(target) = host.viewers.borrow().target(request.viewer_id) else {
        return Ok(Outcome::Rejected(Rejection::ViewerUnavailable));
    };
    let id = AttachmentId::from_bytes(request.controller_id.as_bytes())?;
    let correlation = CorrelationId::from_bytes(request.request_id.as_bytes())?;
    if matches!(request.action, Action::Acquire) {
        if host.viewers.borrow().contains(request.controller_id) {
            return Ok(Outcome::Rejected(Rejection::ControllerBusy));
        }
        if let Err(reason) =
            host.controllers
                .borrow_mut()
                .reserve(request.viewer_id, request.controller_id, target)
        {
            return Ok(Outcome::Rejected(reason));
        }
        let capabilities = AttachmentCapabilityState {
            read: false,
            input: true,
            resize: true,
        };
        let terminal_id = TerminalId::from_bytes(target.as_bytes())?;
        let command =
            encode_attachment_control(&AttachmentControl::AttachTerminal(AttachTerminal {
                attachment_id: id,
                terminal_id,
                capabilities,
                request_resize_lease: true,
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
            host.controllers.borrow_mut().remove(request.controller_id);
            return Ok(Outcome::Rejected(Rejection::HostRejected));
        }
        let AttachmentControl::AttachmentCapabilities(accepted) =
            decode_attachment_control(envelope.kind, envelope.role, envelope.payload)?
        else {
            return Err(invalid("Host did not acknowledge controller capabilities"));
        };
        if accepted.attachment_id != id
            || accepted.terminal_id != terminal_id
            || accepted.capabilities != capabilities
            || !accepted.resize_lease
        {
            return Err(invalid(
                "Host controller grant disagrees with Server authority",
            ));
        }
        host.controllers
            .borrow_mut()
            .commit(request.controller_id, accepted.capability_revision);
        return Ok(Outcome::Acquired);
    }
    let sequence = match &request.action {
        Action::Input { sequence, .. } | Action::Resize { sequence, .. } => *sequence,
        _ => unreachable!("acquisition and release handled"),
    };
    let grant = match host.controllers.borrow_mut().admit(
        request.viewer_id,
        request.controller_id,
        sequence,
    ) {
        Ok(grant) => grant,
        Err(reason) => return Ok(Outcome::Rejected(reason)),
    };
    let revision = grant
        .capability_revision
        .ok_or_else(|| invalid("controller has no Host capability revision"))?;
    let (kind, payload, encoding) = match &request.action {
        Action::Input { bytes, .. } => {
            if bytes.as_bytes().len() > usize::try_from(host.negotiated_limits().input_bytes)? {
                return Ok(Outcome::Rejected(Rejection::HostRejected));
            }
            let command = encode_input_message(&InputMessage::WriteInput(WriteInput {
                attachment_id: id,
                capability_revision: revision,
                bytes: bytes.as_bytes().to_vec(),
            }))?;
            (command.kind, command.payload, PayloadEncoding::RawBytes)
        }
        Action::Resize { geometry, .. } => {
            let command = encode_resize_control(&ResizeControl::ResizeTerminal(ResizeTerminal {
                attachment_id: id,
                geometry: host_geometry(*geometry),
            }))?;
            (command.kind, command.payload, PayloadEncoding::ControlJson)
        }
        _ => unreachable!("effect action matched"),
    };
    let response = exchange(host, server, correlation, kind, &payload, encoding)?;
    let envelope = decode_transport_envelope(&response)?;
    if matches!(
        decode_connection_control(envelope.kind, envelope.role, envelope.payload),
        Ok(ConnectionControl::TypedFailure(_))
    ) {
        return Ok(Outcome::Rejected(Rejection::HostRejected));
    }
    match &request.action {
        Action::Input { bytes, .. } => {
            let InputMessage::InputAccepted(accepted) =
                decode_input_message(envelope.kind, envelope.role, envelope.payload)?
            else {
                return Err(invalid("Host did not acknowledge input"));
            };
            if accepted.attachment_id != id
                || accepted.capability_revision != revision
                || usize::try_from(accepted.accepted_bytes)? != bytes.as_bytes().len()
            {
                return Err(invalid("Host input acknowledgement disagrees"));
            }
            Ok(Outcome::InputAccepted)
        }
        Action::Resize { geometry, .. } => {
            let ResizeControl::ResizeAccepted(accepted) =
                decode_resize_control(envelope.kind, envelope.role, envelope.payload)?
            else {
                return Err(invalid("Host did not acknowledge resize"));
            };
            if accepted.attachment_id != id || accepted.geometry != host_geometry(*geometry) {
                return Err(invalid("Host resize acknowledgement disagrees"));
            }
            Ok(Outcome::Resized(*geometry))
        }
        _ => unreachable!("effect action matched"),
    }
}
pub(crate) fn release_viewer(
    host: &HostBrokerConnection,
    server: &UnixStream,
    viewer: OperationId,
) -> Result<(), Box<dyn Error>> {
    let Some(grant) = host.controllers.borrow().for_viewer(viewer) else {
        return Ok(());
    };
    let target = AttachmentTarget {
        attachment_id: AttachmentId::from_bytes(grant.controller_id.as_bytes())?,
    };
    let command = encode_attachment_control(&AttachmentControl::DetachTerminal(target))?;
    let response = exchange(
        host,
        server,
        CorrelationId::from_bytes(fresh_identity()?)?,
        command.kind,
        &command.payload,
        PayloadEncoding::ControlJson,
    )?;
    let envelope = decode_transport_envelope(&response)?;
    if decode_attachment_control(envelope.kind, envelope.role, envelope.payload)?
        != AttachmentControl::DetachTerminalAccepted(target)
    {
        return Err(invalid("Host did not release controller"));
    }
    host.controllers.borrow_mut().remove(grant.controller_id);
    Ok(())
}
fn host_geometry(
    value: orchbee_capability_node_protocol::TerminalPixelGeometry,
) -> orchbee_terminal_protocol::TerminalPixelGeometry {
    orchbee_terminal_protocol::TerminalPixelGeometry {
        columns: value.columns(),
        rows: value.rows(),
        cell_pixel_width: value.cell_pixel_width(),
        cell_pixel_height: value.cell_pixel_height(),
    }
}
