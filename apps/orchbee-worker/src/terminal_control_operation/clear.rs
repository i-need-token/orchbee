//! One transient input capability owns a prompt-preserving clear and its cleanup.
use super::{
    AttachTerminal, AttachmentCapabilityState, AttachmentControl, AttachmentId, AttachmentTarget,
    ConnectionControl, CorrelationId, Error, HostBrokerConnection, NonZeroU64, Outcome,
    PayloadEncoding, Rejection, TerminalControlRequest, TerminalId, UnixStream,
    decode_attachment_control, decode_connection_control, decode_transport_envelope,
    encode_attachment_control, fresh_identity, invalid,
};
use crate::host_connection::exchange_before;
use orchbee_terminal_protocol::{CURRENT_PROTOCOL_VERSION, Envelope, MessageRole};
use orchbee_terminal_protocol::{
    ClearTerminal, ScreenControl, ScreenControlOperation, decode_screen_control,
    encode_screen_control,
};
use std::{
    net::Shutdown,
    time::{Duration, Instant},
};

const CAPABILITIES: AttachmentCapabilityState = AttachmentCapabilityState {
    read: false,
    input: true,
    resize: false,
};

struct Transient<'a> {
    host: &'a HostBrokerConnection,
    server: &'a UnixStream,
    id: AttachmentId,
    may_exist: bool,
}
impl Drop for Transient<'_> {
    fn drop(&mut self) {
        if self.may_exist {
            // An unconfirmed attachment cannot be reused; connection teardown releases it.
            let _ = self.host.stream().shutdown(Shutdown::Both);
        }
    }
}
impl Transient<'_> {
    fn detach(&mut self) -> bool {
        let target = AttachmentTarget {
            attachment_id: self.id,
        };
        let settled = (|| -> Result<bool, Box<dyn Error>> {
            let command = encode_attachment_control(&AttachmentControl::DetachTerminal(target))?;
            let record = control_exchange(
                self.host,
                self.server,
                CorrelationId::from_bytes(fresh_identity()?)?,
                command.kind,
                &command.payload,
                Instant::now() + Duration::from_secs(1),
            )?;
            let envelope = decode_transport_envelope(&record)?;
            Ok(
                decode_attachment_control(envelope.kind, envelope.role, envelope.payload)?
                    == AttachmentControl::DetachTerminalAccepted(target),
            )
        })()
        .unwrap_or(false);
        if settled {
            self.may_exist = false;
        }
        settled
    }
}

pub(super) fn apply(
    request: &TerminalControlRequest,
    host: &HostBrokerConnection,
    server: &UnixStream,
) -> Result<Outcome, Box<dyn Error>> {
    let Some(target) = host.viewers.borrow().target(request.viewer_id) else {
        return Ok(Outcome::Rejected(Rejection::ViewerUnavailable));
    };
    if request.viewer_id == request.controller_id || host.disposed_terminal(target) {
        return Ok(Outcome::Rejected(Rejection::ViewerUnavailable));
    }
    let terminal_id = TerminalId::from_bytes(target.as_bytes())?;
    let id = AttachmentId::from_bytes(request.controller_id.as_bytes())?;
    let deadline = Instant::now() + Duration::from_secs(3);
    let command = encode_attachment_control(&AttachmentControl::AttachTerminal(AttachTerminal {
        attachment_id: id,
        capabilities: CAPABILITIES,
        request_resize_lease: false,
        terminal_id,
    }))?;
    let mut transient = Transient {
        host,
        server,
        id,
        may_exist: true,
    };
    let attached = (|| -> Result<bool, Box<dyn Error>> {
        let record = control_exchange(
            host,
            server,
            CorrelationId::from_bytes(fresh_identity()?)?,
            command.kind,
            &command.payload,
            deadline.min(Instant::now() + Duration::from_secs(1)),
        )?;
        let envelope = decode_transport_envelope(&record)?;
        if let Ok(ConnectionControl::TypedFailure(_)) =
            decode_connection_control(envelope.kind, envelope.role, envelope.payload)
        {
            transient.may_exist = false;
            return Ok(false);
        }
        let AttachmentControl::AttachmentCapabilities(accepted) =
            decode_attachment_control(envelope.kind, envelope.role, envelope.payload)?
        else {
            return Err(invalid("Host did not grant clear attachment"));
        };
        if accepted.attachment_id != id
            || accepted.terminal_id != terminal_id
            || accepted.capabilities != CAPABILITIES
            || accepted.resize_lease
        {
            return Err(invalid("Host clear attachment disagrees"));
        }
        Ok(true)
    })();
    match attached {
        Ok(true) => {}
        Ok(false) => return Ok(Outcome::Rejected(Rejection::HostRejected)),
        Err(_) => return Ok(Outcome::Rejected(Rejection::HostInputUnavailable)),
    }
    let command = encode_screen_control(ScreenControl::ClearTerminal(ClearTerminal {
        attachment_id: id,
        preserve_prompt: true,
    }))?;
    if Instant::now() >= deadline {
        transient.detach();
        return Ok(Outcome::Rejected(Rejection::HostInputUnavailable));
    }
    // Once this exchange begins, every untrusted response means unknown effect.
    let acknowledged = (|| -> Result<Outcome, Box<dyn Error>> {
        let record = control_exchange(
            host,
            server,
            CorrelationId::from_bytes(request.request_id.as_bytes())?,
            command.kind,
            &command.payload,
            deadline.min(Instant::now() + Duration::from_secs(1)),
        )?;
        let envelope = decode_transport_envelope(&record)?;
        if let Ok(ConnectionControl::TypedFailure(failure)) =
            decode_connection_control(envelope.kind, envelope.role, envelope.payload)
        {
            return Ok(Outcome::Rejected(
                if failure.code == "terminal-clear-alternate-screen" {
                    Rejection::HostClearAlternateScreen
                } else {
                    Rejection::HostRejected
                },
            ));
        }
        let ScreenControl::TerminalControlAccepted(accepted) =
            decode_screen_control(envelope.kind, envelope.role, envelope.payload)?
        else {
            return Err(invalid("Host did not acknowledge clear"));
        };
        if accepted.attachment_id != id
            || accepted.terminal_id != terminal_id
            || accepted.operation != ScreenControlOperation::Clear
        {
            return Err(invalid("Host clear acknowledgement disagrees"));
        }
        Ok(Outcome::Cleared {
            generation: accepted.generation,
            revision: NonZeroU64::new(accepted.revision)
                .ok_or_else(|| invalid("Host clear revision did not advance"))?,
            cleanup_confirmed: false,
        })
    })();
    let mut outcome = match acknowledged {
        Ok(outcome) => outcome,
        Err(_) => return Ok(Outcome::ClearOutcomeUnknown),
    };
    let cleanup_confirmed = transient.detach();
    if let Outcome::Cleared {
        cleanup_confirmed: cleanup,
        ..
    } = &mut outcome
    {
        *cleanup = cleanup_confirmed;
    }
    Ok(outcome)
}

fn control_exchange(
    host: &HostBrokerConnection,
    server: &UnixStream,
    correlation: CorrelationId,
    kind: u16,
    payload: &[u8],
    deadline: Instant,
) -> Result<Vec<u8>, Box<dyn Error>> {
    exchange_before(
        host,
        server,
        Envelope {
            correlation_id: Some(correlation),
            host_epoch: Some(host.host_epoch()),
            kind,
            optional_message: false,
            payload,
            payload_encoding: PayloadEncoding::ControlJson,
            protocol: CURRENT_PROTOCOL_VERSION,
            role: MessageRole::Request,
        },
        deadline,
    )
}
