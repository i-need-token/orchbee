use std::{
    error::Error,
    net::Shutdown,
    num::NonZeroU64,
    os::unix::net::UnixStream,
    time::{Duration, Instant},
};

use orchbee_capability_node_protocol::{
    SendAcceptedBytes, SendCleanupStatus, SendComboBase, SendKey, SendProgress, SendRecord,
    SendRejection, SendUnavailable, TerminalSendOutcome, TerminalSendRequest, TerminalSendResult,
};
use orchbee_local_site_protocol::WorkerId;
use orchbee_terminal_protocol::{
    AttachTerminal, AttachmentCapabilityState, AttachmentControl, AttachmentId, AttachmentTarget,
    CURRENT_PROTOCOL_VERSION, ConnectionControl, CorrelationId, Envelope, InputAccepted,
    InputMessage, KeyAction, KeyInput, MessageRole, PasteAutoInput, PayloadEncoding, TerminalId,
    TerminalKeyCode, TerminalModifiers, WriteInput, decode_attachment_control,
    decode_connection_control, decode_input_message, decode_transport_envelope,
    encode_attachment_control, encode_input_message, encode_transport_envelope,
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

/// One absolute input budget sampled once when the operation starts; it
/// covers the attach exchange plus every record exchange, and each exchange
/// gets at most the lesser of the shared step budget and what remains.
const INPUT_BUDGET: Duration = Duration::from_secs(3);
/// A single Host exchange never exceeds this step budget.
const HOST_STEP_BUDGET: Duration = Duration::from_secs(1);
/// Cleanup detach is a separate allowance outside the input budget; it never
/// renews input time and it never runs more than once.
const CLEANUP_BUDGET: Duration = Duration::from_secs(1);

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
    } else if negotiated_excess(&request, host) {
        // The negotiated-bound check runs before the attachment exists, so
        // the honest frontier is zero: no record was ever submitted.
        return respond(
            &request,
            TerminalSendOutcome::Rejected {
                reason: SendRejection::HostInputLimitExceeded,
                progress: SendProgress::none(),
                cleanup: SendCleanupStatus::NotNeeded,
            },
            host,
            false,
        );
    } else {
        None
    };
    if let Some(reason) = outcome {
        return respond(
            &request,
            TerminalSendOutcome::Rejected {
                reason,
                progress: SendProgress::none(),
                cleanup: SendCleanupStatus::NotNeeded,
            },
            host,
            false,
        );
    }
    apply(request, host, server)
}

/// The negotiated Host input bound — distinct from the static record-level
/// bounds — still gates every byte-bearing record before the attachment is
/// opened. A semantic key or combo carries no raw bytes; only literal
/// content can exceed the negotiated limit. The check runs at
/// pre-admission — a true answer means no record may be dispatched.
fn negotiated_excess(request: &TerminalSendRequest, host: &HostBrokerConnection) -> bool {
    let limit = u64::from(host.negotiated_limits().input_bytes);
    request.records.iter().any(|record| {
        let bytes = match record {
            SendRecord::Text(input) => input.record_bytes() as u64,
            SendRecord::Bytes { bytes } => bytes.as_bytes().len() as u64,
            SendRecord::Paste { content } => content.as_bytes().len() as u64,
            SendRecord::Key { .. } | SendRecord::Combo { .. } => 0,
        };
        bytes > limit
    })
}

fn apply(
    request: TerminalSendRequest,
    host: &HostBrokerConnection,
    server: &UnixStream,
) -> Result<SendExecution, Box<dyn Error>> {
    let attachment_id = AttachmentId::from_bytes(request.attachment_id.as_bytes())?;
    let terminal_id = TerminalId::from_bytes(request.target.terminal_id().as_bytes())?;
    let input_deadline = Instant::now()
        .checked_add(INPUT_BUDGET)
        .ok_or_else(|| invalid("input deadline overflow"))?;
    let command = encode_attachment_control(&AttachmentControl::AttachTerminal(AttachTerminal {
        attachment_id,
        capabilities: INPUT_CAPABILITIES,
        request_resize_lease: false,
        terminal_id,
    }))?;
    let Some(attach_budget) = step_budget(input_deadline) else {
        return respond(
            &request,
            TerminalSendOutcome::Unavailable {
                reason: SendUnavailable::InputBudgetExhausted,
                progress: SendProgress::none(),
                cleanup: SendCleanupStatus::NotNeeded,
            },
            host,
            false,
        );
    };
    let capability_revision = match exchange_fresh(
        host,
        server,
        command.kind,
        &command.payload,
        PayloadEncoding::ControlJson,
        attach_budget,
    )? {
        StepAnswer::Record(record) => match attach_outcome(&record, attachment_id, terminal_id)? {
            AttachOutcome::Revision(revision) => revision,
            AttachOutcome::Refused(reason) => {
                return respond(
                    &request,
                    TerminalSendOutcome::Rejected {
                        reason,
                        progress: SendProgress::none(),
                        cleanup: SendCleanupStatus::NotNeeded,
                    },
                    host,
                    false,
                );
            }
            AttachOutcome::Desync => {
                return close(
                    &request,
                    TerminalSendOutcome::Unavailable {
                        reason: SendUnavailable::HostProtocolFault,
                        progress: SendProgress::none(),
                        cleanup: SendCleanupStatus::Unconfirmed,
                    },
                    host,
                );
            }
        },
        StepAnswer::ProtocolFault => {
            return close(
                &request,
                TerminalSendOutcome::Unavailable {
                    reason: SendUnavailable::HostProtocolFault,
                    progress: SendProgress::none(),
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
                &request,
                TerminalSendOutcome::Unavailable {
                    reason: SendUnavailable::HostUnavailable,
                    progress: SendProgress::none(),
                    cleanup: SendCleanupStatus::Unconfirmed,
                },
                host,
            );
        }
    };

    // Ordered correlated exchanges, one per record. The frontier cursor moves
    // only on a validated acknowledgement; the first stop ends the sequence
    // and no record is ever replayed.
    let mut accepted_encoded_bytes = 0_u64;
    let record_count = request.records.len();
    for index in 0..record_count {
        let record = &request.records.as_slice()[index];
        let Some(budget) = step_budget(input_deadline) else {
            // The input deadline expired between records: a determinate stop.
            // The confirmed prefix stands, the stopped record provably never
            // left this Worker, and cleanup still gets its own allowance.
            let (cleanup, host_failed) = detach(host, server, attachment_id).cleanup();
            return respond(
                &request,
                TerminalSendOutcome::Unavailable {
                    reason: SendUnavailable::InputBudgetExhausted,
                    progress: SendProgress::stopped(index, accepted_encoded_bytes),
                    cleanup,
                },
                host,
                host_failed,
            );
        };
        let (command, expected_bytes) = input_command(record, attachment_id, capability_revision)?;
        // Input messages ride the raw-bytes encoding while every response is
        // still ControlJson.
        match exchange_fresh(
            host,
            server,
            command.kind,
            &command.payload,
            PayloadEncoding::RawBytes,
            budget,
        )? {
            StepAnswer::Record(record) => {
                match input_outcome(&record, attachment_id, capability_revision, expected_bytes)? {
                    InputOutcome::Accepted(bytes) => {
                        accepted_encoded_bytes += u64::from(bytes.get());
                    }
                    InputOutcome::Refused(reason) => {
                        let (cleanup, host_failed) = detach(host, server, attachment_id).cleanup();
                        return respond(
                            &request,
                            TerminalSendOutcome::Rejected {
                                reason,
                                progress: SendProgress::stopped(index, accepted_encoded_bytes),
                                cleanup,
                            },
                            host,
                            host_failed,
                        );
                    }
                    InputOutcome::Desync => {
                        return close(
                            &request,
                            TerminalSendOutcome::OutcomeUnknown {
                                progress: SendProgress::stopped(index, accepted_encoded_bytes),
                                cleanup: SendCleanupStatus::Unconfirmed,
                            },
                            host,
                        );
                    }
                }
            }
            // The record may have been applied before the carrier failed or
            // the response desynced; it is never resent and no further
            // exchange is attempted on this stream.
            StepAnswer::ProtocolFault | StepAnswer::TransportLost => {
                return close(
                    &request,
                    TerminalSendOutcome::OutcomeUnknown {
                        progress: SendProgress::stopped(index, accepted_encoded_bytes),
                        cleanup: SendCleanupStatus::Unconfirmed,
                    },
                    host,
                );
            }
            // The record provably never left this Worker.
            StepAnswer::LocalFault => {
                return close(
                    &request,
                    TerminalSendOutcome::Unavailable {
                        reason: SendUnavailable::HostUnavailable,
                        progress: SendProgress::stopped(index, accepted_encoded_bytes),
                        cleanup: SendCleanupStatus::Unconfirmed,
                    },
                    host,
                );
            }
        }
    }
    let (cleanup, host_failed) = detach(host, server, attachment_id).cleanup();
    respond(
        &request,
        TerminalSendOutcome::Accepted {
            progress: SendProgress::complete(record_count, accepted_encoded_bytes),
            cleanup,
        },
        host,
        host_failed,
    )
}

/// What one exchange may spend: at most the shared step budget and never
/// past the absolute input deadline. `None` means the deadline already ran
/// out — the caller must not start another record.
fn step_budget(input_deadline: Instant) -> Option<Duration> {
    let remaining = input_deadline.checked_duration_since(Instant::now())?;
    Some(remaining.min(HOST_STEP_BUDGET))
}

/// One Host round trip under a fresh correlation and the given budget. The
/// answer is classified by where the exchange ended so the caller can tell a
/// dead carrier from a desynchronized one; a Server-side failure is fatal to
/// the whole session and propagates instead.
fn exchange_fresh(
    host: &HostBrokerConnection,
    server: &UnixStream,
    kind: u16,
    payload: &[u8],
    encoding: PayloadEncoding,
    budget: Duration,
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
    let Some(mut exchange) = HostExchange::new(host, server, budget) else {
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
    // A raw write is admitted byte-exact or not at all; a semantic key and a
    // mode-encoded paste report whatever byte count the Host produced under
    // the terminal's own modes, so only the identity echo and the protocol
    // byte bound apply to those records.
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

/// Best-effort detach under its own cleanup allowance for the paths where
/// the carrier stayed healthy enough to try: a typed input refusal, a
/// validated InputAccepted, or a determinate budget stop between records. A
/// local construction failure still counts as unreachable — the resulting
/// shutdown drops the lingering attachment with the connection instead of
/// silently retaining it.
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
        CLEANUP_BUDGET,
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

/// The Host input message for one record plus the byte-exactness expectation
/// the acknowledgement must satisfy. `Text` and `Bytes` are raw writes, so
/// the Host's accepted count must equal the exact encoded byte count;
/// `Paste` content gets bracket markers from the terminal's own mode and
/// key records are semantic encodings, so neither has a predictable count.
fn input_command(
    record: &SendRecord,
    attachment_id: AttachmentId,
    capability_revision: NonZeroU64,
) -> Result<(orchbee_terminal_protocol::EncodedInputMessage, Option<u32>), Box<dyn Error>> {
    match record {
        SendRecord::Text(input) => {
            let mut bytes = Vec::with_capacity(input.record_bytes());
            bytes.extend_from_slice(input.text().as_str().as_bytes());
            if input.enter() {
                bytes.push(b'\r');
            }
            raw_write(attachment_id, capability_revision, bytes)
        }
        SendRecord::Bytes { bytes } => raw_write(
            attachment_id,
            capability_revision,
            bytes.as_bytes().to_vec(),
        ),
        SendRecord::Paste { content } => Ok((
            encode_input_message(&InputMessage::PasteAutoInput(PasteAutoInput {
                attachment_id,
                capability_revision,
                content: content.as_bytes().to_vec(),
            }))?,
            None,
        )),
        SendRecord::Key { key } => {
            let (code, control, unshifted_codepoint) = key_fields(*key);
            key_press(
                attachment_id,
                capability_revision,
                &code,
                TerminalModifiers {
                    control,
                    ..TerminalModifiers::default()
                },
                unshifted_codepoint,
            )
        }
        SendRecord::Combo { modifiers, base } => {
            let (code, unshifted_codepoint) = combo_base_fields(*base);
            key_press(
                attachment_id,
                capability_revision,
                &code,
                TerminalModifiers {
                    alt: modifiers.alt,
                    control: modifiers.control,
                    shift: modifiers.shift,
                    ..TerminalModifiers::default()
                },
                unshifted_codepoint,
            )
        }
    }
}

fn raw_write(
    attachment_id: AttachmentId,
    capability_revision: NonZeroU64,
    bytes: Vec<u8>,
) -> Result<(orchbee_terminal_protocol::EncodedInputMessage, Option<u32>), Box<dyn Error>> {
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

fn key_press(
    attachment_id: AttachmentId,
    capability_revision: NonZeroU64,
    code: &str,
    modifiers: TerminalModifiers,
    unshifted_codepoint: u32,
) -> Result<(orchbee_terminal_protocol::EncodedInputMessage, Option<u32>), Box<dyn Error>> {
    Ok((
        encode_input_message(&InputMessage::KeyInput(KeyInput {
            action: KeyAction::Press,
            attachment_id,
            capability_revision,
            composing: false,
            consumed_modifiers: TerminalModifiers::default(),
            key: TerminalKeyCode::parse(code)?,
            modifiers,
            option_as_alt: false,
            text: String::new(),
            unshifted_codepoint,
        }))?,
        None,
    ))
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

/// The terminal-protocol key identity and neutral codepoint for one combo
/// base. `Named` reuses the semantic-key spellings (the already-modified
/// `CtrlC`/`CtrlD` bases cannot occur); `Letter`/`Digit` select the exact
/// physical `KeyX`/`DigitN` code the Host encoder expects, with the ASCII
/// codepoint as the unshifted value.
fn combo_base_fields(base: SendComboBase) -> (String, u32) {
    match base {
        SendComboBase::Named(key) => {
            let (code, _control, codepoint) = key_fields(key);
            (code.to_owned(), codepoint)
        }
        SendComboBase::Insert => ("Insert".to_owned(), 0),
        SendComboBase::Function(number) => (format!("F{number}"), 0),
        SendComboBase::Space => ("Space".to_owned(), u32::from(b' ')),
        SendComboBase::Letter(letter) => (
            format!("Key{}", (letter - b'a' + b'A') as char),
            u32::from(letter),
        ),
        SendComboBase::Digit(digit) => (format!("Digit{digit}"), u32::from(digit)),
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
    request: &TerminalSendRequest,
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
    request: &TerminalSendRequest,
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
