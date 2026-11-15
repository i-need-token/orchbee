//! One transient input capability owns a prompt-preserving clear or a full reset and its cleanup.
use super::{
    AttachTerminal, AttachmentCapabilityState, AttachmentControl, AttachmentId, AttachmentTarget,
    ConnectionControl, CorrelationId, Error, HostBrokerConnection, NonZeroU64, OperationId,
    Outcome, PayloadEncoding, Rejection, TerminalControlRequest, TerminalId, UnixStream,
    decode_attachment_control, decode_connection_control, decode_transport_envelope,
    diagnostic_events, encode_attachment_control, fresh_identity,
};
use crate::diagnostic_events::Site;
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

/// The Host screen control one transient attachment performs.
#[derive(Clone, Copy)]
pub(super) enum Operation {
    Clear,
    Reset,
}
impl Operation {
    fn command(self, attachment_id: AttachmentId) -> ScreenControl {
        match self {
            Self::Clear => ScreenControl::ClearTerminal(ClearTerminal {
                attachment_id,
                preserve_prompt: true,
            }),
            Self::Reset => ScreenControl::ResetTerminal(attachment_id),
        }
    }
    const fn acknowledged(self) -> ScreenControlOperation {
        match self {
            Self::Clear => ScreenControlOperation::Clear,
            Self::Reset => ScreenControlOperation::Reset,
        }
    }
    const fn applied(
        self,
        generation: NonZeroU64,
        revision: NonZeroU64,
        cleanup_confirmed: bool,
    ) -> Outcome {
        match self {
            Self::Clear => Outcome::Cleared {
                generation,
                revision,
                cleanup_confirmed,
            },
            Self::Reset => Outcome::ResetAccepted {
                generation,
                revision,
                cleanup_confirmed,
            },
        }
    }
    const fn unknown(self) -> Outcome {
        match self {
            Self::Clear => Outcome::ClearOutcomeUnknown,
            Self::Reset => Outcome::ResetOutcomeUnknown,
        }
    }
}

/// A screen control starts its Host step only within this time after the request arrives; each
/// exchange itself waits up to the connection's Host exchange bound.
const START_BUDGET: Duration = Duration::from_secs(3);

/// A correlated Host answer to one transient step.
enum Answer<T> {
    Accepted(T),
    Refused(Rejection),
    /// Another correlated answer, of this message kind.
    Unexpected(u16),
}

struct Transient<'a> {
    host: &'a HostBrokerConnection,
    server: &'a UnixStream,
    id: AttachmentId,
    controller: OperationId,
    may_exist: bool,
}
impl Drop for Transient<'_> {
    fn drop(&mut self) {
        if self.may_exist {
            // An exchange failed, so the stream is out of step; connection teardown releases it.
            let _ = self.host.stream().shutdown(Shutdown::Both);
        }
    }
}
impl Transient<'_> {
    /// Detaches the transient attachment. Any correlated answer settles it; only the exact
    /// acknowledgement confirms the cleanup.
    fn detach(&mut self) -> bool {
        let target = AttachmentTarget {
            attachment_id: self.id,
        };
        let answered = (|| -> Result<(bool, u16), Box<dyn Error>> {
            let command = encode_attachment_control(&AttachmentControl::DetachTerminal(target))?;
            let record = control_exchange(
                self.host,
                self.server,
                CorrelationId::from_bytes(fresh_identity()?)?,
                command.kind,
                &command.payload,
            )?;
            let envelope = decode_transport_envelope(&record)?;
            Ok((
                decode_attachment_control(envelope.kind, envelope.role, envelope.payload).ok()
                    == Some(AttachmentControl::DetachTerminalAccepted(target)),
                envelope.kind,
            ))
        })();
        let Ok((confirmed, kind)) = answered else {
            return false;
        };
        if !confirmed {
            diagnostic_events::host_unexpected(Site::Control, kind);
        }
        self.may_exist = false;
        self.host.forget_controller(self.controller);
        confirmed
    }
}

pub(super) fn apply(
    request: &TerminalControlRequest,
    host: &HostBrokerConnection,
    server: &UnixStream,
    operation: Operation,
) -> Result<Outcome, Box<dyn Error>> {
    let Some(target) = host.viewers.borrow().target(request.viewer_id) else {
        return Ok(Outcome::Rejected(Rejection::ViewerUnavailable));
    };
    if request.viewer_id == request.controller_id || host.disposed_terminal(target) {
        return Ok(Outcome::Rejected(Rejection::ViewerUnavailable));
    }
    let terminal_id = TerminalId::from_bytes(target.as_bytes())?;
    let id = AttachmentId::from_bytes(request.controller_id.as_bytes())?;
    let start_deadline = Instant::now() + START_BUDGET;
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
        controller: request.controller_id,
        may_exist: true,
    };
    let attached = (|| -> Result<Answer<()>, Box<dyn Error>> {
        let record = control_exchange(
            host,
            server,
            CorrelationId::from_bytes(fresh_identity()?)?,
            command.kind,
            &command.payload,
        )?;
        let envelope = decode_transport_envelope(&record)?;
        if let Ok(ConnectionControl::TypedFailure(_)) =
            decode_connection_control(envelope.kind, envelope.role, envelope.payload)
        {
            transient.may_exist = false;
            return Ok(Answer::Refused(Rejection::HostRejected));
        }
        let Ok(AttachmentControl::AttachmentCapabilities(accepted)) =
            decode_attachment_control(envelope.kind, envelope.role, envelope.payload)
        else {
            return Ok(Answer::Unexpected(envelope.kind));
        };
        if accepted.attachment_id != id
            || accepted.terminal_id != terminal_id
            || accepted.capabilities != CAPABILITIES
            || accepted.resize_lease
        {
            return Ok(Answer::Unexpected(envelope.kind));
        }
        Ok(Answer::Accepted(()))
    })();
    match attached {
        Ok(Answer::Accepted(())) => {}
        Ok(Answer::Refused(rejection)) => return Ok(Outcome::Rejected(rejection)),
        // A grant other than the one asked for is never used: it is detached and refused.
        Ok(Answer::Unexpected(kind)) => {
            diagnostic_events::host_unexpected(Site::Control, kind);
            transient.detach();
            return Ok(Outcome::Rejected(Rejection::HostRejected));
        }
        Err(_) => return Ok(Outcome::Rejected(Rejection::HostInputUnavailable)),
    }
    let command = encode_screen_control(operation.command(id))?;
    if Instant::now() >= start_deadline {
        transient.detach();
        return Ok(Outcome::Rejected(Rejection::HostInputUnavailable));
    }
    // Once this exchange begins, every untrusted response means unknown effect.
    let acknowledged = (|| -> Result<Answer<_>, Box<dyn Error>> {
        let record = control_exchange(
            host,
            server,
            CorrelationId::from_bytes(request.request_id.as_bytes())?,
            command.kind,
            &command.payload,
        )?;
        let envelope = decode_transport_envelope(&record)?;
        if let Ok(ConnectionControl::TypedFailure(failure)) =
            decode_connection_control(envelope.kind, envelope.role, envelope.payload)
        {
            return Ok(Answer::Refused(
                if failure.code == "terminal-clear-alternate-screen" {
                    Rejection::HostClearAlternateScreen
                } else {
                    Rejection::HostRejected
                },
            ));
        }
        let Ok(ScreenControl::TerminalControlAccepted(accepted)) =
            decode_screen_control(envelope.kind, envelope.role, envelope.payload)
        else {
            return Ok(Answer::Unexpected(envelope.kind));
        };
        let revision = NonZeroU64::new(accepted.revision);
        match revision {
            Some(revision)
                if accepted.attachment_id == id
                    && accepted.terminal_id == terminal_id
                    && accepted.operation == operation.acknowledged() =>
            {
                Ok(Answer::Accepted((accepted.generation, revision)))
            }
            _ => Ok(Answer::Unexpected(envelope.kind)),
        }
    })();
    // A failed exchange leaves the stream out of step, and dropping the transient ends the
    // connection; a correlated answer leaves it in step, so the attachment is detached.
    let Ok(acknowledged) = acknowledged else {
        return Ok(operation.unknown());
    };
    let cleanup_confirmed = transient.detach();
    Ok(match acknowledged {
        Answer::Accepted((generation, revision)) => {
            operation.applied(generation, revision, cleanup_confirmed)
        }
        Answer::Refused(rejection) => Outcome::Rejected(rejection),
        Answer::Unexpected(kind) => {
            diagnostic_events::host_unexpected(Site::Control, kind);
            operation.unknown()
        }
    })
}
fn control_exchange(
    host: &HostBrokerConnection,
    server: &UnixStream,
    correlation: CorrelationId,
    kind: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Box<dyn Error>> {
    let deadline = host
        .pacing
        .deadline(Instant::now(), host.pacing.exchange_bound());
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
