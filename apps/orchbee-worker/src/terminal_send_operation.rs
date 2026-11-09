//! Executes an exact-physical-target send through a transient input attachment, retaining
//! confirmed prefix progress and an independent bounded detach outcome. It does not claim
//! child execution, atomicity, or permission to replay accepted or uncertain records. An
//! idle-prompt send is the one exception to per-record exchanges: its whole prompt line is one
//! Host `PromptLineInput`, typed only at a confirmed fresh prompt, and a Host refusal is returned
//! as is; the Worker never waits for a settling prompt and never falls back to an ungated send.

mod stroke;

use orchbee_local_ipc::LocalStream as UnixStream;
use std::{
    error::Error,
    net::Shutdown,
    num::NonZeroU64,
    time::{Duration, Instant},
};

use orchbee_capability_node_protocol::{
    SendAcceptedBytes, SendCleanupStatus, SendPrecondition, SendProgress, SendPromptLine,
    SendPromptState, SendRecord, SendRejection, SendUnavailable, TerminalSendOutcome,
    TerminalSendRequest, TerminalSendResult,
};
use orchbee_local_worker_protocol::WorkerId;
use orchbee_terminal_protocol::{
    AttachTerminal, AttachmentCapabilityState, AttachmentControl, AttachmentId, AttachmentTarget,
    CURRENT_PROTOCOL_VERSION, ConnectionControl, CorrelationId, Envelope, InputAccepted,
    InputMessage, MessageRole, PasteAutoInput, PayloadEncoding, PromptLineInput,
    PromptUnconfirmedReason, TerminalId, TypedFailure, WriteInput, decode_attachment_control,
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
    } else if request.precondition == SendPrecondition::IdlePrompt
        && request.records.prompt_line().is_none()
    {
        // The codec admits only a prompt line; a hand-built request is refused the same way.
        Some(SendRejection::HostInputInvalid)
    } else if request.precondition == SendPrecondition::IdlePrompt && !host.prompt_gated_input() {
        // A Host that cannot confirm the prompt is too old for this send: it is refused before
        // any attachment, never typed ungated.
        Some(SendRejection::HostPromptGateUnsupported)
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
            SendRecord::Stroke(_) => 0,
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

    // Ordered correlated exchanges, one per step: a record, or the whole
    // idle-prompt line. The frontier cursor moves only on a validated
    // acknowledgement; the first stop ends the sequence and no step is ever
    // replayed.
    let mut accepted_encoded_bytes = 0_u64;
    let record_count = request.records.len();
    let steps = match request.precondition {
        SendPrecondition::None => request.records.iter().map(Step::Record).collect(),
        SendPrecondition::IdlePrompt => request
            .records
            .prompt_line()
            .map(Step::PromptLine)
            .into_iter()
            .collect::<Vec<_>>(),
    };
    let mut index = 0;
    for step in steps {
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
        let (command, expected_bytes) = step_command(step, attachment_id, capability_revision)?;
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
                match input_outcome(
                    &record,
                    attachment_id,
                    capability_revision,
                    expected_bytes,
                    matches!(step, Step::PromptLine(_)),
                )? {
                    InputOutcome::Accepted(bytes) => {
                        accepted_encoded_bytes += u64::from(bytes.get());
                        index += step.records(record_count);
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
    prompt_line: bool,
) -> Result<InputOutcome, Box<dyn Error>> {
    let envelope = decode_transport_envelope(record)?;
    if let Ok(ConnectionControl::TypedFailure(failure)) =
        decode_connection_control(envelope.kind, envelope.role, envelope.payload)
    {
        return Ok(InputOutcome::Refused(input_rejection(
            &failure,
            prompt_line,
        )));
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
        // Only the requested Host acknowledgment can confirm this attachment/process operation.
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

/// One Host input exchange: a single record, or an idle-prompt send's whole line.
#[derive(Clone, Copy)]
enum Step<'a> {
    Record(&'a SendRecord),
    PromptLine(SendPromptLine<'a>),
}

impl Step<'_> {
    /// How many of the request's `record_count` records this step carries.
    const fn records(self, record_count: usize) -> usize {
        match self {
            Self::Record(_) => 1,
            Self::PromptLine(_) => record_count,
        }
    }
}

/// The Host input message for one step plus the byte-exactness expectation
/// the acknowledgement must satisfy. `Text` and `Bytes` are raw writes, so
/// the Host's accepted count must equal the exact encoded byte count;
/// `Paste` content and a prompt line get bracket markers from the terminal's
/// own mode and key records are semantic encodings, so none of them has a
/// predictable count.
fn step_command(
    step: Step<'_>,
    attachment_id: AttachmentId,
    capability_revision: NonZeroU64,
) -> Result<(orchbee_terminal_protocol::EncodedInputMessage, Option<u32>), Box<dyn Error>> {
    let record = match step {
        Step::Record(record) => record,
        Step::PromptLine(line) => {
            return Ok((
                encode_input_message(&InputMessage::PromptLineInput(PromptLineInput {
                    attachment_id,
                    capability_revision,
                    right_arrow: line.right_arrow,
                    content: line.content.to_owned(),
                }))?,
                None,
            ));
        }
    };
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
        SendRecord::Stroke(stroke) => Ok((
            encode_input_message(&InputMessage::KeyInput(stroke::input(
                *stroke,
                attachment_id,
                capability_revision,
            )?))?,
            None,
        )),
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

fn attach_rejection(code: &str) -> SendRejection {
    match code {
        "missing-resource" => SendRejection::TerminalUnavailable,
        _ => SendRejection::HostRejected,
    }
}

/// A typed input refusal. Only the prompt-line step may carry the Host's `prompt-unconfirmed`
/// state, which types nothing; the same code on any other step is an ordinary refusal.
fn input_rejection(failure: &TypedFailure, prompt_line: bool) -> SendRejection {
    if prompt_line && let Some(reason) = failure.reason {
        return SendRejection::PromptUnconfirmed(prompt_state(reason));
    }
    match failure.code.as_str() {
        "missing-resource" | "invalid-phase" | "stale-revision" | "stale-generation" => {
            SendRejection::HostInputUnavailable
        }
        "invalid-field" | "invalid-acknowledgement" => SendRejection::HostInputInvalid,
        "limit-exceeded" => SendRejection::HostInputLimitExceeded,
        "resource-exhausted" => SendRejection::HostInputBackpressure,
        _ => SendRejection::HostRejected,
    }
}

const fn prompt_state(reason: PromptUnconfirmedReason) -> SendPromptState {
    match reason {
        PromptUnconfirmedReason::NotShell => SendPromptState::NotShell,
        PromptUnconfirmedReason::Busy => SendPromptState::Busy,
        PromptUnconfirmedReason::FullScreen => SendPromptState::FullScreen,
        PromptUnconfirmedReason::Unmarked => SendPromptState::Unmarked,
        PromptUnconfirmedReason::NotAtPrompt => SendPromptState::NotAtPrompt,
        PromptUnconfirmedReason::InputPending => SendPromptState::InputPending,
        PromptUnconfirmedReason::Changed => SendPromptState::Changed,
        PromptUnconfirmedReason::Settling => SendPromptState::Settling,
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
