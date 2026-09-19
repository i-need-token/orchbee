use std::{
    error::Error, net::Shutdown, num::NonZeroU64, os::unix::net::UnixStream, time::Duration,
};

use orchbee_capability_node_protocol::{
    SendAcceptedBytes, SendCleanupStatus, SendKey, SendRejection, SendUnavailable,
    TerminalSendOutcome, TerminalSendPayload, TerminalSendRequest, TerminalSendResult,
};
use orchbee_local_site_protocol::WorkerId;
use orchbee_terminal_protocol::{
    AttachTerminal, AttachmentCapabilityState, AttachmentControl, AttachmentId, AttachmentTarget,
    CURRENT_PROTOCOL_VERSION, ConnectionControl, CorrelationId, Envelope, InputAccepted,
    InputMessage, KeyAction, KeyInput, MessageRole, PayloadEncoding, TerminalId, TerminalKeyCode,
    TerminalModifiers, WriteInput, decode_attachment_control, decode_connection_control,
    decode_input_message, decode_transport_envelope, encode_attachment_control,
    encode_input_message, encode_transport_envelope,
};

use crate::host_connection::{HostBrokerConnection, HostExchange, PeerFailure};
use crate::worker_identity::fresh_identity;

/// A send never opens a readable or resizable surface: the temporary
/// attachment carries exactly the input capability it needs and no lease.
const INPUT_CAPABILITIES: AttachmentCapabilityState = AttachmentCapabilityState {
    read: false,
    input: true,
    resize: false,
};

/// One Host round trip per send step, matching the shared exchange budget.
const HOST_STEP_BUDGET: Duration = Duration::from_secs(1);

/// The operation result plus whether the Host carrier is still trustworthy.
/// `host_failed` asks the caller to deliver the result and then end the
/// session: a desynchronized or closed Host stream must not serve further
/// exchanges.
pub(crate) struct SendExecution {
    pub(crate) result: TerminalSendResult,
    pub(crate) host_failed: bool,
}

/// One Host step's answer, classified by where the exchange ended. Only a
/// well-formed correlated response lets the operation continue; every other
/// answer ends the carrier without another exchange on the same stream.
enum StepAnswer {
    /// A complete correlated response record to interpret.
    Record(Vec<u8>),
    /// The request could not even be constructed locally — a failed identity
    /// draw or envelope encoding. The request provably never reached the Host.
    LocalFault,
    /// The carrier died in flight — refused write, EOF, timeout, or an
    /// unparseable frame. The request may or may not have reached the Host.
    TransportLost,
    /// A response arrived but failed the correlation/epoch/role echo check.
    /// The stream is desynchronized; another exchange on it would be
    /// fabrication, so the carrier is shut down immediately.
    ProtocolFault,
}

enum AttachOutcome {
    Revision(NonZeroU64),
    Refused(SendRejection),
    Desync,
}

enum InputOutcome {
    Accepted(SendAcceptedBytes),
    Refused(SendRejection),
    Desync,
}

#[derive(Clone, Copy)]
enum DetachOutcome {
    Released,
    Refused,
    Unreachable,
}

impl DetachOutcome {
    /// Only a confirmed release keeps the carrier healthy. A typed refusal or
    /// an unreachable cleanup both leave the attachment's fate unknown, and an
    /// unconfirmed cleanup must not keep accepting sends that would stack
    /// more input-only attachments on the same carrier — the Host is closed.
    fn cleanup(self) -> (SendCleanupStatus, bool) {
        match self {
            Self::Released => (SendCleanupStatus::Confirmed, false),
            Self::Refused | Self::Unreachable => (SendCleanupStatus::Unconfirmed, true),
        }
    }
}

pub(crate) fn execute(
    request: TerminalSendRequest,
    worker_id: WorkerId,
    generation: NonZeroU64,
    host: &HostBrokerConnection,
    server: &UnixStream,
) -> Result<SendExecution, Box<dyn Error>> {
    let outcome = if request.worker_id.as_bytes() != worker_id.as_bytes() {
        Some(SendRejection::WrongWorker)
    } else if request.worker_generation.get() != generation.get() {
        Some(SendRejection::StaleWorkerGeneration)
    } else if request.target.host_epoch().as_bytes() != *host.host_epoch().as_bytes() {
        Some(SendRejection::StaleHostEpoch)
    } else if host.disposed_terminal(request.target.terminal_id()) {
        Some(SendRejection::TerminalUnavailable)
    } else if exceeds_negotiated_input(&request, host) {
        Some(SendRejection::HostInputLimitExceeded)
    } else {
        None
    };
    if let Some(reason) = outcome {
        return respond(
            request,
            TerminalSendOutcome::Rejected {
                reason,
                cleanup: SendCleanupStatus::NotNeeded,
            },
            host,
            false,
        );
    }
    apply(request, host, server)
}

/// The negotiated Host input bound — distinct from the static command-level
/// payload bound — still gates the final write. A semantic key carries no
/// raw bytes; only a write payload can exceed the negotiated limit.
fn exceeds_negotiated_input(request: &TerminalSendRequest, host: &HostBrokerConnection) -> bool {
    match &request.payload {
        TerminalSendPayload::Text(input) => {
            input.record_bytes() as u64 > u64::from(host.negotiated_limits().input_bytes)
        }
        TerminalSendPayload::Key { .. } => false,
    }
}

fn apply(
    request: TerminalSendRequest,
    host: &HostBrokerConnection,
    server: &UnixStream,
) -> Result<SendExecution, Box<dyn Error>> {
    let attachment_id = AttachmentId::from_bytes(request.attachment_id.as_bytes())?;
    let terminal_id = TerminalId::from_bytes(request.target.terminal_id().as_bytes())?;
    let command = encode_attachment_control(&AttachmentControl::AttachTerminal(AttachTerminal {
        attachment_id,
        capabilities: INPUT_CAPABILITIES,
        request_resize_lease: false,
        terminal_id,
    }))?;
    let capability_revision = match exchange_fresh(
        host,
        server,
        command.kind,
        &command.payload,
        PayloadEncoding::ControlJson,
    )? {
        StepAnswer::Record(record) => match attach_outcome(&record, attachment_id, terminal_id)? {
            AttachOutcome::Revision(revision) => revision,
            AttachOutcome::Refused(reason) => {
                return respond(
                    request,
                    TerminalSendOutcome::Rejected {
                        reason,
                        cleanup: SendCleanupStatus::NotNeeded,
                    },
                    host,
                    false,
                );
            }
            AttachOutcome::Desync => {
                return close(
                    request,
                    TerminalSendOutcome::Unavailable {
                        reason: SendUnavailable::HostProtocolFault,
                        cleanup: SendCleanupStatus::Unconfirmed,
                    },
                    host,
                );
            }
        },
        StepAnswer::ProtocolFault => {
            return close(
                request,
                TerminalSendOutcome::Unavailable {
                    reason: SendUnavailable::HostProtocolFault,
                    cleanup: SendCleanupStatus::Unconfirmed,
                },
                host,
            );
        }
        StepAnswer::LocalFault | StepAnswer::TransportLost => {
            // Nothing trustworthy came back; the attachment may exist on the
            // Host but the stream can never carry a reliable DetachTerminal.
            // Shutdown lets the pending attachment die with the connection.
            return close(
                request,
                TerminalSendOutcome::Unavailable {
                    reason: SendUnavailable::HostUnavailable,
                    cleanup: SendCleanupStatus::Unconfirmed,
                },
                host,
            );
        }
    };
    let (command, expected_bytes) = input_command(&request, attachment_id, capability_revision)?;
    // Input messages ride the raw-bytes encoding while every response is
    // still ControlJson.
    let accepted_bytes = match exchange_fresh(
        host,
        server,
        command.kind,
        &command.payload,
        PayloadEncoding::RawBytes,
    )? {
        StepAnswer::Record(record) => {
            match input_outcome(&record, attachment_id, capability_revision, expected_bytes)? {
                InputOutcome::Accepted(bytes) => bytes,
                InputOutcome::Refused(reason) => {
                    let (cleanup, host_failed) = detach(host, server, attachment_id).cleanup();
                    return respond(
                        request,
                        TerminalSendOutcome::Rejected { reason, cleanup },
                        host,
                        host_failed,
                    );
                }
                InputOutcome::Desync => {
                    return close(
                        request,
                        TerminalSendOutcome::OutcomeUnknown {
                            cleanup: SendCleanupStatus::Unconfirmed,
                        },
                        host,
                    );
                }
            }
        }
        // The input may have been applied before the carrier failed or the
        // response desynced; it is never resent and no further exchange is
        // attempted on this stream.
        StepAnswer::ProtocolFault | StepAnswer::TransportLost => {
            return close(
                request,
                TerminalSendOutcome::OutcomeUnknown {
                    cleanup: SendCleanupStatus::Unconfirmed,
                },
                host,
            );
        }
        // The input provably never left this Worker.
        StepAnswer::LocalFault => {
            return close(
                request,
                TerminalSendOutcome::Unavailable {
                    reason: SendUnavailable::HostUnavailable,
                    cleanup: SendCleanupStatus::Unconfirmed,
                },
                host,
            );
        }
    };
    let (cleanup, host_failed) = detach(host, server, attachment_id).cleanup();
    respond(
        request,
        TerminalSendOutcome::Accepted {
            accepted_bytes,
            cleanup,
        },
        host,
        host_failed,
    )
}

/// One Host round trip under a fresh correlation. The answer is classified by
/// where the exchange ended so the caller can tell a dead carrier from a
/// desynchronized one; a Server-side failure is fatal to the whole session
/// and propagates instead.
fn exchange_fresh(
    host: &HostBrokerConnection,
    server: &UnixStream,
    kind: u16,
    payload: &[u8],
    encoding: PayloadEncoding,
) -> Result<StepAnswer, Box<dyn Error>> {
    let local = (|| -> Result<(CorrelationId, Vec<u8>), Box<dyn Error>> {
        let correlation = CorrelationId::from_bytes(fresh_identity()?)?;
        let record = encode_transport_envelope(&Envelope {
            correlation_id: Some(correlation),
            host_epoch: Some(host.host_epoch()),
            kind,
            optional_message: false,
            payload,
            payload_encoding: encoding,
            protocol: CURRENT_PROTOCOL_VERSION,
            role: MessageRole::Request,
        })?;
        Ok((correlation, record))
    })();
    let Ok((correlation, record)) = local else {
        return Ok(StepAnswer::LocalFault);
    };
    let Some(mut exchange) = HostExchange::new(host, server, HOST_STEP_BUDGET) else {
        return Ok(StepAnswer::LocalFault);
    };
    match exchange.prepare() {
        Ok(()) => {}
        Err(PeerFailure::Host) => return Ok(StepAnswer::TransportLost),
        Err(PeerFailure::Protocol) => return Ok(StepAnswer::ProtocolFault),
        Err(PeerFailure::Server) => return Err(server_interference()),
    }
    let record = match exchange.round_trip(&record) {
        Ok(record) => record,
        Err(PeerFailure::Host) => return Ok(StepAnswer::TransportLost),
        Err(PeerFailure::Protocol) => return Ok(StepAnswer::ProtocolFault),
        Err(PeerFailure::Server) => return Err(server_interference()),
    };
    let Ok(envelope) = decode_transport_envelope(&record) else {
        return Ok(StepAnswer::ProtocolFault);
    };
    if envelope.correlation_id != Some(correlation)
        || envelope.host_epoch != Some(host.host_epoch())
        || envelope.role != MessageRole::Response
        || envelope.protocol != CURRENT_PROTOCOL_VERSION
        || envelope.optional_message
        || envelope.payload_encoding != PayloadEncoding::ControlJson
    {
        return Ok(StepAnswer::ProtocolFault);
    }
    Ok(StepAnswer::Record(record))
}

fn attach_outcome(
    record: &[u8],
    attachment_id: AttachmentId,
    terminal_id: TerminalId,
) -> Result<AttachOutcome, Box<dyn Error>> {
    let envelope = decode_transport_envelope(record)?;
    if let Ok(ConnectionControl::TypedFailure(failure)) =
        decode_connection_control(envelope.kind, envelope.role, envelope.payload)
    {
        return Ok(AttachOutcome::Refused(attach_rejection(&failure.code)));
    }
    let Ok(AttachmentControl::AttachmentCapabilities(accepted)) =
        decode_attachment_control(envelope.kind, envelope.role, envelope.payload)
    else {
        return Ok(AttachOutcome::Desync);
    };
    if accepted.attachment_id != attachment_id
        || accepted.terminal_id != terminal_id
        || accepted.capabilities != INPUT_CAPABILITIES
        || accepted.resize_lease
    {
        return Ok(AttachOutcome::Desync);
    }
    Ok(AttachOutcome::Revision(accepted.capability_revision))
}

fn input_outcome(
    record: &[u8],
    attachment_id: AttachmentId,
    capability_revision: NonZeroU64,
    expected_bytes: Option<u32>,
) -> Result<InputOutcome, Box<dyn Error>> {
    let envelope = decode_transport_envelope(record)?;
    if let Ok(ConnectionControl::TypedFailure(failure)) =
        decode_connection_control(envelope.kind, envelope.role, envelope.payload)
    {
        return Ok(InputOutcome::Refused(input_rejection(&failure.code)));
    }
    let Ok(InputMessage::InputAccepted(InputAccepted {
        accepted_bytes,
        attachment_id: accepted_attachment,
        capability_revision: accepted_revision,
    })) = decode_input_message(envelope.kind, envelope.role, envelope.payload)
    else {
        return Ok(InputOutcome::Desync);
    };
    // A raw write is admitted byte-exact or not at all; a semantic key reports
    // whatever byte count the Host encoded under the terminal's own modes, so
    // only the identity echo and the protocol byte bound apply.
    if accepted_attachment != attachment_id
        || accepted_revision != capability_revision
        || expected_bytes.is_some_and(|expected| accepted_bytes != expected)
    {
        return Ok(InputOutcome::Desync);
    }
    let Ok(bytes) = SendAcceptedBytes::new(accepted_bytes) else {
        return Ok(InputOutcome::Desync);
    };
    Ok(InputOutcome::Accepted(bytes))
}

/// Best-effort detach for the paths where the carrier stayed healthy enough
/// to try: a typed input refusal or a validated InputAccepted. A local
/// construction failure still counts as unreachable — the resulting shutdown
/// drops the lingering attachment with the connection instead of silently
/// retaining it.
fn detach(
    host: &HostBrokerConnection,
    server: &UnixStream,
    attachment_id: AttachmentId,
) -> DetachOutcome {
    let target = AttachmentTarget { attachment_id };
    let Ok(command) = encode_attachment_control(&AttachmentControl::DetachTerminal(target)) else {
        return DetachOutcome::Unreachable;
    };
    let answer = match exchange_fresh(
        host,
        server,
        command.kind,
        &command.payload,
        PayloadEncoding::ControlJson,
    ) {
        Ok(answer) => answer,
        Err(error) => return unreachable_detach(error),
    };
    let StepAnswer::Record(record) = answer else {
        return DetachOutcome::Unreachable;
    };
    let Ok(envelope) = decode_transport_envelope(&record) else {
        return DetachOutcome::Unreachable;
    };
    if matches!(
        decode_connection_control(envelope.kind, envelope.role, envelope.payload),
        Ok(ConnectionControl::TypedFailure(_))
    ) {
        return DetachOutcome::Refused;
    }
    match decode_attachment_control(envelope.kind, envelope.role, envelope.payload) {
        Ok(AttachmentControl::DetachTerminalAccepted(accepted)) if accepted == target => {
            DetachOutcome::Released
        }
        _ => DetachOutcome::Unreachable,
    }
}

/// A Server-side exchange failure while detaching is fatal to the session
/// anyway; it is reported through the operation's error path so the pending
/// result is never fabricated.
fn unreachable_detach(error: Box<dyn Error>) -> DetachOutcome {
    let _ = error;
    DetachOutcome::Unreachable
}

fn input_command(
    request: &TerminalSendRequest,
    attachment_id: AttachmentId,
    capability_revision: NonZeroU64,
) -> Result<(orchbee_terminal_protocol::EncodedInputMessage, Option<u32>), Box<dyn Error>> {
    match &request.payload {
        TerminalSendPayload::Text(input) => {
            let mut bytes = Vec::with_capacity(input.record_bytes());
            bytes.extend_from_slice(input.text().as_str().as_bytes());
            if input.enter() {
                bytes.push(b'\r');
            }
            let expected = u32::try_from(bytes.len()).map_err(|_| invalid("input too large"))?;
            Ok((
                encode_input_message(&InputMessage::WriteInput(WriteInput {
                    attachment_id,
                    bytes,
                    capability_revision,
                }))?,
                Some(expected),
            ))
        }
        TerminalSendPayload::Key { key } => {
            let (code, control, unshifted_codepoint) = key_fields(*key);
            Ok((
                encode_input_message(&InputMessage::KeyInput(KeyInput {
                    action: KeyAction::Press,
                    attachment_id,
                    capability_revision,
                    composing: false,
                    consumed_modifiers: TerminalModifiers::default(),
                    key: TerminalKeyCode::parse(code)?,
                    modifiers: TerminalModifiers {
                        control,
                        ..TerminalModifiers::default()
                    },
                    option_as_alt: false,
                    text: String::new(),
                    unshifted_codepoint,
                }))?,
                None,
            ))
        }
    }
}

/// The terminal-protocol key identity and neutral modifier state for one
/// closed send key. `ctrl-` combinations are the physical letter under the
/// control modifier, so the Host encoder derives the control byte itself.
fn key_fields(key: SendKey) -> (&'static str, bool, u32) {
    match key {
        SendKey::Enter => ("Enter", false, 0),
        SendKey::Escape => ("Escape", false, 0),
        SendKey::Tab => ("Tab", false, 0),
        SendKey::Backspace => ("Backspace", false, 0),
        SendKey::Delete => ("Delete", false, 0),
        SendKey::Up => ("ArrowUp", false, 0),
        SendKey::Down => ("ArrowDown", false, 0),
        SendKey::Left => ("ArrowLeft", false, 0),
        SendKey::Right => ("ArrowRight", false, 0),
        SendKey::Home => ("Home", false, 0),
        SendKey::End => ("End", false, 0),
        SendKey::PageUp => ("PageUp", false, 0),
        SendKey::PageDown => ("PageDown", false, 0),
        SendKey::CtrlC => ("KeyC", true, u32::from(b'c')),
        SendKey::CtrlD => ("KeyD", true, u32::from(b'd')),
    }
}

fn attach_rejection(code: &str) -> SendRejection {
    match code {
        "missing-resource" => SendRejection::TerminalUnavailable,
        _ => SendRejection::HostRejected,
    }
}

fn input_rejection(code: &str) -> SendRejection {
    match code {
        "missing-resource" | "invalid-phase" | "stale-revision" | "stale-generation" => {
            SendRejection::HostInputUnavailable
        }
        "invalid-field" | "invalid-acknowledgement" => SendRejection::HostInputInvalid,
        "limit-exceeded" => SendRejection::HostInputLimitExceeded,
        "resource-exhausted" => SendRejection::HostInputBackpressure,
        _ => SendRejection::HostRejected,
    }
}

/// A healthy-carrier result: the connection stays usable for the next Server
/// operation. `host_failed` here means the cleanup detach itself desynced.
fn respond(
    request: TerminalSendRequest,
    outcome: TerminalSendOutcome,
    host: &HostBrokerConnection,
    host_failed: bool,
) -> Result<SendExecution, Box<dyn Error>> {
    if host_failed {
        let _closed = host.stream().shutdown(Shutdown::Both);
    }
    Ok(SendExecution {
        result: TerminalSendResult::new(request.request_id, outcome)?,
        host_failed,
    })
}

/// A desynchronized or dead carrier: the stream is shut down so the pending
/// result is the last record this Host session ever produces.
fn close(
    request: TerminalSendRequest,
    outcome: TerminalSendOutcome,
    host: &HostBrokerConnection,
) -> Result<SendExecution, Box<dyn Error>> {
    respond(request, outcome, host, true)
}

fn invalid(message: &'static str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message)
}

fn server_interference() -> Box<dyn Error> {
    std::io::Error::other("Server stream interfered with the Host send exchange").into()
}
