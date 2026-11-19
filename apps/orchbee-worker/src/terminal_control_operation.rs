//! Fences controller requests and translates admitted acquire, takeover, input, resize, and
//! release operations into exact Host exchanges. It maintains connection grant state without
//! deciding Server-side Client authority or replaying uncertain effects.

use crate::{
    diagnostic_events::{self, Site},
    host_connection::{HostBrokerConnection, NotStarted, exchange},
    terminal_viewer_operation::{DemandAnswer, UnexpectedAnswer, set_attachment_demand},
    worker_identity::fresh_identity,
};
mod host_colors;
mod screen_control;
#[cfg(test)]
mod tests;
use orchbee_capability_node_protocol::{
    ControlRejection as Rejection, OperationId, TerminalControlAction as Action,
    TerminalControlOutcome as Outcome, TerminalControlRequest, TerminalControlResult,
};
use orchbee_local_ipc::LocalStream as UnixStream;
use orchbee_local_worker_protocol::WorkerId;
use orchbee_terminal_protocol::{
    AttachTerminal, AttachmentCapabilityState, AttachmentControl, AttachmentId, AttachmentTarget,
    ConnectionControl, CorrelationId, InputMessage, MessageRole, PasteAutoInput, PayloadEncoding,
    ProjectionDemand, ResizeControl, ResizeTerminal, TerminalId, WriteInput,
    decode_attachment_control, decode_connection_control, decode_input_message,
    decode_resize_control, decode_transport_envelope, encode_attachment_control,
    encode_input_message, encode_resize_control,
};
use std::{error::Error, num::NonZeroU64};
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
        match apply(&request, host, server) {
            Ok(outcome) => outcome,
            Err(error) if NotStarted::is(error.as_ref()) => not_started(&request.action),
            Err(error) => contain(error, host, server, &request)?,
        }
    };
    Ok(TerminalControlResult {
        request_id: request.request_id,
        viewer_id: request.viewer_id,
        controller_id: request.controller_id,
        outcome,
    })
}

/// The answer to a control request whose own Host exchange had no room in the request. Nothing
/// reached the Host and the grant stays as it was. A request's own exchange starts within the
/// record budget of its arrival, well inside the start limit, so only a Worker that stalled for
/// seconds meets this.
const fn not_started(action: &Action) -> Outcome {
    Outcome::Rejected(match action {
        Action::Acquire | Action::Takeover { .. } => Rejection::ControllerBusy,
        Action::SemanticInput { .. } | Action::Paste { .. } => Rejection::HostInputBackpressure,
        Action::Resize { .. } => Rejection::HostRejected,
        Action::Input { .. }
        | Action::Release
        | Action::Clear
        | Action::Reset
        | Action::HostColors { .. } => Rejection::HostInputUnavailable,
    })
}
/// An unexpected answer refuses this request and releases its viewer's controller, so a Client
/// that wants control again acquires it anew and nothing is replayed; every other failure still
/// ends the connection.
///
/// A request made under a held grant answers `NotController`, which for such a request always
/// means this Worker holds no grant for it, so the Server drops its lease too; every
/// `HostRejected` such a request receives leaves the grant in place. A refused Acquire or
/// Takeover never installed a lease and keeps its `HostRejected`. Release, Clear and Reset never
/// meet an unexpected answer here.
fn contain(
    error: Box<dyn Error>,
    host: &HostBrokerConnection,
    server: &UnixStream,
    request: &TerminalControlRequest,
) -> Result<Outcome, Box<dyn Error>> {
    let answer = error.downcast::<UnexpectedAnswer>()?;
    diagnostic_events::host_unexpected(
        Site::Control,
        answer.kind,
        diagnostic_events::Anomaly::Answer,
    );
    release_viewer(host, server, request.viewer_id)?;
    Ok(Outcome::Rejected(match request.action {
        Action::Acquire | Action::Takeover { .. } => Rejection::HostRejected,
        Action::Input { .. }
        | Action::SemanticInput { .. }
        | Action::Resize { .. }
        | Action::Paste { .. }
        | Action::HostColors { .. }
        | Action::Release
        | Action::Clear
        | Action::Reset => Rejection::NotController,
    }))
}
fn apply(
    request: &TerminalControlRequest,
    host: &HostBrokerConnection,
    server: &UnixStream,
) -> Result<Outcome, Box<dyn Error>> {
    match request.action {
        Action::Clear => {
            return screen_control::apply(request, host, server, screen_control::Operation::Clear);
        }
        Action::Reset => {
            return screen_control::apply(request, host, server, screen_control::Operation::Reset);
        }
        _ => {}
    }
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
    // A Takeover naming the viewer's own grant as its replacement only confirms that grant: it
    // touches no Host attachment, so no deferred Detach or full backlog stands in its way, and a
    // refusal always means this Worker holds no such grant for the viewer.
    if let Action::Takeover {
        previous_controller_id,
    } = request.action
        && previous_controller_id == request.controller_id
    {
        let held =
            host.controllers
                .borrow()
                .confirm_own(request.viewer_id, request.controller_id, target);
        return Ok(held.map_or_else(Outcome::Rejected, |()| Outcome::Acquired));
    }
    let id = AttachmentId::from_bytes(request.controller_id.as_bytes())?;
    let correlation = CorrelationId::from_bytes(request.request_id.as_bytes())?;
    if matches!(request.action, Action::Acquire | Action::Takeover { .. }) {
        // While Detaches wait for cleanup, the Host still holds those attachments; no new
        // controller is opened until cleanup has caught up.
        if host.viewers.borrow().contains(request.controller_id) || host.detach_backlog_full() {
            return Ok(Outcome::Rejected(Rejection::ControllerBusy));
        }
        // A deferred Detach's attachment may still hold this terminal's resize lease, which the
        // Attach below asks for, so the Host would refuse the new controller until cleanup ran.
        // Those Detaches go first; with no room for one, control is busy and any incumbent keeps
        // it.
        crate::terminal_viewer_operation::settle_deferred_detaches(host, server)?;
        if let Action::Takeover {
            previous_controller_id,
        } = request.action
        {
            let incumbent = host.controllers.borrow().takeover_incumbent(
                request.viewer_id,
                request.controller_id,
                target,
                previous_controller_id,
            );
            match incumbent {
                Err(reason) => return Ok(Outcome::Rejected(reason)),
                // The incumbent keeps control unless its Detach starts; only the deferred
                // Detaches above go before it.
                Ok(Some(grant)) => {
                    detach_controller(host, server, grant.controller_id)?;
                    host.forget_controller(grant.controller_id);
                }
                Ok(None) => (),
            }
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
        let Some(response) = NotStarted::optional(exchange(
            host,
            server,
            correlation,
            command.kind,
            &command.payload,
            PayloadEncoding::ControlJson,
        ))?
        else {
            // Only a Takeover's slow incumbent Detach leaves no room for this Attach: control is
            // free now, and the Client may ask again.
            host.controllers.borrow_mut().remove(request.controller_id);
            return Ok(Outcome::Rejected(Rejection::ControllerBusy));
        };
        let envelope = decode_transport_envelope(&response)?;
        if matches!(
            decode_connection_control(envelope.kind, envelope.role, envelope.payload),
            Ok(ConnectionControl::TypedFailure(_))
        ) {
            host.controllers.borrow_mut().remove(request.controller_id);
            return Ok(Outcome::Rejected(Rejection::HostRejected));
        }
        let Ok(AttachmentControl::AttachmentCapabilities(accepted)) =
            decode_attachment_control(envelope.kind, envelope.role, envelope.payload)
        else {
            return Err(UnexpectedAnswer::of(&response).into());
        };
        // A grant that disagrees with the Server's authority is never used: it is released.
        if accepted.attachment_id != id
            || accepted.terminal_id != terminal_id
            || accepted.capabilities != capabilities
            || !accepted.resize_lease
        {
            return Err(UnexpectedAnswer::of(&response).into());
        }
        host.controllers
            .borrow_mut()
            .commit(request.controller_id, accepted.capability_revision);
        // A controller has no projection and starts paused; its focus claim follows its viewer.
        // When this request has no room left for that, a later cleanup sets it.
        let demand = host.viewers.borrow().demand(request.viewer_id);
        if demand == Some(ProjectionDemand::Active) {
            match NotStarted::optional(set_attachment_demand(
                host,
                server,
                correlation,
                id,
                ProjectionDemand::Active,
            ))? {
                Some(DemandAnswer::Accepted) => host
                    .controllers
                    .borrow_mut()
                    .set_demand(request.controller_id, ProjectionDemand::Active),
                Some(DemandAnswer::Refused(_)) => {
                    release_viewer(host, server, request.viewer_id)?;
                    return Ok(Outcome::Rejected(Rejection::HostRejected));
                }
                None => {}
            }
        }
        return Ok(Outcome::Acquired);
    }
    let sequence = match &request.action {
        Action::Input { sequence, .. }
        | Action::SemanticInput { sequence, .. }
        | Action::Resize { sequence, .. }
        | Action::Paste { sequence, .. }
        | Action::HostColors { sequence, .. } => *sequence,
        Action::Acquire
        | Action::Takeover { .. }
        | Action::Release
        | Action::Clear
        | Action::Reset => {
            unreachable!("acquisition, release, clear, and reset handled")
        }
    };
    let grant = match host.controllers.borrow_mut().admit(
        request.viewer_id,
        request.controller_id,
        sequence,
    ) {
        Ok(grant) => grant,
        Err(reason) => return Ok(Outcome::Rejected(reason)),
    };
    if let Action::HostColors { colors, .. } = request.action {
        return host_colors::apply(host, server, target, colors);
    }
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
        Action::SemanticInput { input, .. } => {
            let Some(message) = controlled_semantic_input(input, id, revision) else {
                return Ok(Outcome::Rejected(Rejection::HostInputInvalid));
            };
            let command = encode_input_message(&message)?;
            (command.kind, command.payload, PayloadEncoding::RawBytes)
        }
        Action::Resize { geometry, .. } => {
            let command = encode_resize_control(&ResizeControl::ResizeTerminal(ResizeTerminal {
                attachment_id: id,
                geometry: host_geometry(*geometry),
            }))?;
            (command.kind, command.payload, PayloadEncoding::ControlJson)
        }
        Action::Paste { content, .. } => {
            let command = encode_input_message(&InputMessage::PasteAutoInput(PasteAutoInput {
                attachment_id: id,
                capability_revision: revision,
                content: content.as_bytes().to_vec(),
            }))?;
            (command.kind, command.payload, PayloadEncoding::RawBytes)
        }
        Action::Acquire
        | Action::Takeover { .. }
        | Action::Release
        | Action::Clear
        | Action::Reset
        | Action::HostColors { .. } => unreachable!("effect action matched"),
    };
    let response = exchange(host, server, correlation, kind, &payload, encoding)?;
    let envelope = decode_transport_envelope(&response)?;
    if let Ok(ConnectionControl::TypedFailure(failure)) =
        decode_connection_control(envelope.kind, envelope.role, envelope.payload)
    {
        let rejection = match &request.action {
            Action::Paste { .. } | Action::SemanticInput { .. } => match failure.code.as_str() {
                "invalid-field" => Rejection::HostInputInvalid,
                "limit-exceeded" => Rejection::HostInputLimitExceeded,
                "input-backpressure" => Rejection::HostInputBackpressure,
                "invalid-phase" => Rejection::HostInputUnavailable,
                _ => Rejection::HostRejected,
            },
            Action::Acquire
            | Action::Takeover { .. }
            | Action::Release
            | Action::Clear
            | Action::Reset
            | Action::Input { .. }
            | Action::Resize { .. }
            | Action::HostColors { .. } => Rejection::HostRejected,
        };
        return Ok(Outcome::Rejected(rejection));
    }
    match &request.action {
        Action::Input { bytes, .. } => {
            let Ok(InputMessage::InputAccepted(accepted)) =
                decode_input_message(envelope.kind, envelope.role, envelope.payload)
            else {
                return Err(UnexpectedAnswer::of(&response).into());
            };
            if accepted.attachment_id != id
                || accepted.capability_revision != revision
                || usize::try_from(accepted.accepted_bytes)? != bytes.as_bytes().len()
            {
                return Err(UnexpectedAnswer::of(&response).into());
            }
            Ok(Outcome::InputAccepted)
        }
        Action::SemanticInput { .. } => {
            // Semantic encodings depend on terminal modes, so the admitted count is unpredictable.
            let Ok(InputMessage::InputAccepted(accepted)) =
                decode_input_message(envelope.kind, envelope.role, envelope.payload)
            else {
                return Err(UnexpectedAnswer::of(&response).into());
            };
            if accepted.attachment_id != id || accepted.capability_revision != revision {
                return Err(UnexpectedAnswer::of(&response).into());
            }
            Ok(Outcome::InputAccepted)
        }
        Action::Resize { geometry, .. } => {
            let Ok(ResizeControl::ResizeAccepted(accepted)) =
                decode_resize_control(envelope.kind, envelope.role, envelope.payload)
            else {
                return Err(UnexpectedAnswer::of(&response).into());
            };
            if accepted.attachment_id != id || accepted.geometry != host_geometry(*geometry) {
                return Err(UnexpectedAnswer::of(&response).into());
            }
            Ok(Outcome::Resized(*geometry))
        }
        Action::Paste { content, .. } => {
            let Ok(InputMessage::InputAccepted(accepted)) =
                decode_input_message(envelope.kind, envelope.role, envelope.payload)
            else {
                return Err(UnexpectedAnswer::of(&response).into());
            };
            let content_length = content.as_bytes().len();
            let bound = content_length
                .checked_mul(2)
                .and_then(|value| value.checked_add(12))
                .ok_or_else(|| invalid("paste response bound overflowed"))?
                .min(usize::try_from(host.negotiated_limits().input_bytes)?);
            if accepted.attachment_id != id
                || accepted.capability_revision != revision
                || accepted.accepted_bytes == 0
                || usize::try_from(accepted.accepted_bytes)? > bound
            {
                return Err(UnexpectedAnswer::of(&response).into());
            }
            Ok(Outcome::Pasted {
                accepted_bytes: orchbee_capability_node_protocol::PasteAcceptedBytes::new(
                    accepted.accepted_bytes,
                )?,
            })
        }
        Action::Acquire
        | Action::Takeover { .. }
        | Action::Release
        | Action::Clear
        | Action::Reset
        | Action::HostColors { .. } => unreachable!("effect action matched"),
    }
}
/// Validates a Client's semantic record with the Host codec and rebinds it to the live grant, so
/// the Client's attachment and capability fields carry no authority.
fn controlled_semantic_input(
    input: &orchbee_capability_node_protocol::SemanticInput,
    attachment_id: AttachmentId,
    capability_revision: NonZeroU64,
) -> Option<InputMessage> {
    use orchbee_capability_node_protocol::SemanticInputKind as Kind;
    let kind = match input.kind() {
        Kind::Focus => orchbee_terminal_protocol::FOCUS_INPUT_KIND,
        Kind::Key => orchbee_terminal_protocol::KEY_INPUT_KIND,
        Kind::Mouse => orchbee_terminal_protocol::MOUSE_INPUT_KIND,
        Kind::Text => orchbee_terminal_protocol::TEXT_INPUT_KIND,
    };
    let mut message = decode_input_message(kind, MessageRole::Request, input.payload()).ok()?;
    match &mut message {
        InputMessage::FocusInput(input) => {
            input.attachment_id = attachment_id;
            input.capability_revision = capability_revision;
        }
        InputMessage::KeyInput(input) => {
            input.attachment_id = attachment_id;
            input.capability_revision = capability_revision;
        }
        InputMessage::MouseInput(input) => {
            input.attachment_id = attachment_id;
            input.capability_revision = capability_revision;
        }
        InputMessage::TextInput(input) => {
            input.attachment_id = attachment_id;
            input.capability_revision = capability_revision;
        }
        // The kind above selected this codec; any other shape cannot reach here.
        InputMessage::WriteInput(_)
        | InputMessage::InputAccepted(_)
        | InputMessage::PasteInput(_)
        | InputMessage::PasteAutoInput(_)
        | InputMessage::PromptLineInput(_) => return None,
    }
    Some(message)
}

/// Releases the controller attached for `viewer`, if any. Any correlated answer ends the
/// controller: a refusal counts as released, and another answer is recorded and the controller
/// forgotten all the same. A Detach the request has no room for is left to a later cleanup; the
/// controller is released here all the same. Only a failed exchange is an error.
pub(crate) fn release_viewer(
    host: &HostBrokerConnection,
    server: &UnixStream,
    viewer: OperationId,
) -> Result<(), Box<dyn Error>> {
    let Some(grant) = host.controllers.borrow().for_viewer(viewer) else {
        return Ok(());
    };
    if NotStarted::optional(detach_controller(host, server, grant.controller_id))?.is_none() {
        host.defer_detach(grant.controller_id.as_bytes());
    }
    host.forget_controller(grant.controller_id);
    Ok(())
}

/// Detaches one controller now: any correlated answer ends it, and one other than the exact
/// acceptance is recorded.
fn detach_controller(
    host: &HostBrokerConnection,
    server: &UnixStream,
    controller: OperationId,
) -> Result<(), Box<dyn Error>> {
    let target = AttachmentTarget {
        attachment_id: AttachmentId::from_bytes(controller.as_bytes())?,
    };
    let command = encode_attachment_control(&AttachmentControl::DetachTerminal(target))?;
    let response = match exchange(
        host,
        server,
        CorrelationId::from_bytes(fresh_identity()?)?,
        command.kind,
        &command.payload,
        PayloadEncoding::ControlJson,
    ) {
        Ok(response) => response,
        Err(error) => {
            let answer = error.downcast::<UnexpectedAnswer>()?;
            diagnostic_events::host_unexpected(
                host.pacing.site(),
                answer.kind,
                diagnostic_events::Anomaly::Answer,
            );
            return Ok(());
        }
    };
    let envelope = decode_transport_envelope(&response)?;
    if decode_attachment_control(envelope.kind, envelope.role, envelope.payload).ok()
        != Some(AttachmentControl::DetachTerminalAccepted(target))
    {
        diagnostic_events::host_unexpected(
            host.pacing.site(),
            envelope.kind,
            diagnostic_events::Anomaly::Answer,
        );
    }
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
