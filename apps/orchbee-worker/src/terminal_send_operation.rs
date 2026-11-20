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

use crate::diagnostic_events::{Anomaly, Site, host_unexpected, send_outcome_unbuildable};
use crate::host_connection::{
    HostBrokerConnection, HostExchange, HostFailure, PeerFailure, record_kind, response_envelope,
};
use crate::worker_identity::fresh_identity;

/// A send never opens a readable or resizable surface: the temporary
/// attachment carries exactly the input capability it needs and no lease.
const INPUT_CAPABILITIES: AttachmentCapabilityState = AttachmentCapabilityState {
    read: false,
    input: true,
    resize: false,
};

/// One absolute input budget sampled once when the operation starts: the
/// attach and every record exchange start only before it passes. An exchange
/// once started waits up to the connection's Host exchange bound, within the
/// request's bound, so a slow but answering Host never loses the carrier.
/// Cleanup detach is not gated by it and never runs more than once.
const INPUT_BUDGET: Duration = Duration::from_secs(3);

/// The operation result plus whether the Host carrier is still trustworthy.
/// `host_failed` asks the caller to deliver the result and then end the
/// session: a desynchronized or closed Host stream must not serve further
/// exchanges.
pub(crate) struct SendExecution {
    pub(crate) result: TerminalSendResult,
    pub(crate) host_failed: bool,
}

/// One Host step's answer, classified by where the exchange ended. A
/// correlated response that arrived whole, in or out of contract, and a step
/// that wrote nothing leave the carrier in step; a lost carrier or framing ends
/// the carrier without another exchange on the same stream.
enum StepAnswer {
    /// A complete correlated response record to interpret.
    Record(Vec<u8>),
    /// A complete correlated response, of this message kind, whose envelope
    /// breaks its contract. It arrived whole, so the carrier stays in step and
    /// the step is treated like any other answer it cannot accept.
    Desync(u16),
    /// The request had no room left for this exchange; nothing was written.
    NotStarted,
    /// The request could not even be constructed locally — a failed identity
    /// draw or envelope encoding. The request provably never reached the Host.
    LocalFault,
    /// The carrier died in flight — refused write, EOF, timeout, or an
    /// unparseable frame. The request may or may not have reached the Host.
    TransportLost,
    /// The Host lost its framing: no later byte on the stream can be trusted,
    /// so the carrier is shut down immediately.
    ProtocolFault,
}

enum AttachOutcome {
    Revision(NonZeroU64),
    Refused(SendRejection),
    /// An intact correlated answer, of this message kind, that grants something
    /// other than the attachment asked for.
    Desync(u16),
}

enum InputOutcome {
    Accepted(SendAcceptedBytes),
    Refused(SendRejection),
    /// An intact correlated answer, of this message kind, that contradicts the
    /// record.
    Desync(u16),
}

#[derive(Clone, Copy)]
enum DetachOutcome {
    /// The Host accepted the Detach, or refused it: the attachment is gone.
    Released,
    /// The attachment may remain, and a later cleanup detaches it; the carrier
    /// stays in step.
    Unconfirmed,
    /// The carrier failed while detaching.
    Lost,
}

impl DetachOutcome {
    /// The cleanup status, and whether the carrier must end. An attachment
    /// left for cleanup counts toward the detach backlog, which refuses new
    /// sends while full instead of stacking input-only attachments.
    fn cleanup(self) -> (SendCleanupStatus, bool) {
        match self {
            Self::Released => (SendCleanupStatus::Confirmed, false),
            Self::Unconfirmed => (SendCleanupStatus::Unconfirmed, false),
            Self::Lost => (SendCleanupStatus::Unconfirmed, true),
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
    } else if host.unavailable_terminal(request.target.terminal_id()) {
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
    // A self-built Attach that cannot be encoded never reached the Host: it is answered as the
    // local fault of the attach exchange, and the carrier is untouched.
    let Ok(command) =
        encode_attachment_control(&AttachmentControl::AttachTerminal(AttachTerminal {
            attachment_id,
            capabilities: INPUT_CAPABILITIES,
            request_resize_lease: false,
            terminal_id,
        }))
    else {
        return respond(
            &request,
            TerminalSendOutcome::Unavailable {
                reason: SendUnavailable::HostUnavailable,
                progress: SendProgress::none(),
                cleanup: SendCleanupStatus::NotNeeded,
            },
            host,
            false,
        );
    };
    let not_started = || {
        respond(
            &request,
            TerminalSendOutcome::Unavailable {
                reason: SendUnavailable::InputBudgetExhausted,
                progress: SendProgress::none(),
                cleanup: SendCleanupStatus::NotNeeded,
            },
            host,
            false,
        )
    };
    // While Detaches wait for cleanup, no further input-only attachment is opened.
    if !may_start(input_deadline) || host.detach_backlog_full() {
        return not_started();
    }
    let attached = match exchange_fresh(
        host,
        server,
        command.kind,
        &command.payload,
        PayloadEncoding::ControlJson,
    )? {
        StepAnswer::Record(record) => attach_outcome(&record, attachment_id, terminal_id)?,
        StepAnswer::Desync(kind) => AttachOutcome::Desync(kind),
        StepAnswer::NotStarted => return not_started(),
        // The Attach provably never reached the Host, so the carrier stays in step.
        StepAnswer::LocalFault => {
            return respond(
                &request,
                TerminalSendOutcome::Unavailable {
                    reason: SendUnavailable::HostUnavailable,
                    progress: SendProgress::none(),
                    cleanup: SendCleanupStatus::NotNeeded,
                },
                host,
                false,
            );
        }
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
        StepAnswer::TransportLost => {
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
    let capability_revision = match attached {
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
        // A grant other than the one asked for is never used: this send is refused and the
        // attachment detached. The answer arrived intact, so the carrier stays in step.
        AttachOutcome::Desync(kind) => {
            host_unexpected(Site::Send, kind, Anomaly::Answer);
            let (cleanup, host_failed) = detach(host, server, attachment_id).cleanup();
            return respond(
                &request,
                TerminalSendOutcome::Rejected {
                    reason: SendRejection::HostRejected,
                    progress: SendProgress::none(),
                    cleanup,
                },
                host,
                host_failed,
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
        if !may_start(input_deadline) {
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
        }
        // A record that cannot be built never left this Worker: a determinate stop, as for a
        // record the Host never received, and the attachment is detached.
        let Ok((command, expected_bytes)) = step_command(step, attachment_id, capability_revision)
        else {
            let (cleanup, host_failed) = detach(host, server, attachment_id).cleanup();
            return respond(
                &request,
                TerminalSendOutcome::Unavailable {
                    reason: SendUnavailable::HostUnavailable,
                    progress: SendProgress::stopped(index, accepted_encoded_bytes),
                    cleanup,
                },
                host,
                host_failed,
            );
        };
        // Input messages ride the raw-bytes encoding while every response is
        // still ControlJson.
        let input = match exchange_fresh(
            host,
            server,
            command.kind,
            &command.payload,
            PayloadEncoding::RawBytes,
        )? {
            StepAnswer::Record(record) => input_outcome(
                &record,
                attachment_id,
                capability_revision,
                expected_bytes,
                matches!(step, Step::PromptLine(_)),
            )?,
            StepAnswer::Desync(kind) => InputOutcome::Desync(kind),
            // The request had no room for this record: a determinate stop, as at the input
            // deadline.
            StepAnswer::NotStarted => {
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
            }
            // The record may have been applied before the carrier or its
            // framing failed; it is never resent and no further exchange is
            // attempted on this stream.
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
            // The record provably never left this Worker, so the carrier stays in step.
            StepAnswer::LocalFault => {
                let (cleanup, host_failed) = detach(host, server, attachment_id).cleanup();
                return respond(
                    &request,
                    TerminalSendOutcome::Unavailable {
                        reason: SendUnavailable::HostUnavailable,
                        progress: SendProgress::stopped(index, accepted_encoded_bytes),
                        cleanup,
                    },
                    host,
                    host_failed,
                );
            }
        };
        match input {
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
            // The record may have been typed, so its outcome is unknown and it is never sent
            // again. The answer arrived intact, so the carrier stays in step and the attachment
            // is detached.
            InputOutcome::Desync(kind) => {
                host_unexpected(Site::Send, kind, Anomaly::Answer);
                let (cleanup, host_failed) = detach(host, server, attachment_id).cleanup();
                return respond(
                    &request,
                    TerminalSendOutcome::OutcomeUnknown {
                        progress: SendProgress::stopped(index, accepted_encoded_bytes),
                        cleanup,
                    },
                    host,
                    host_failed,
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

/// Whether another step may start: `false` once the absolute input deadline
/// has passed, and the caller must not start another record.
fn may_start(input_deadline: Instant) -> bool {
    Instant::now() < input_deadline
}

/// One Host round trip under a fresh correlation and the exchange bound. The
/// answer is classified by where the exchange ended so the caller can tell a
/// dead carrier from an answer it cannot accept; a Server-side failure is fatal
/// to the whole session and propagates instead.
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
    let Ok(mut exchange) = HostExchange::open(host, server) else {
        return Ok(StepAnswer::NotStarted);
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
    let in_contract = response_envelope(&record, correlation, host.host_epoch())
        .is_ok_and(|envelope| envelope.payload_encoding == PayloadEncoding::ControlJson);
    if !in_contract {
        return Ok(StepAnswer::Desync(record_kind(&record)));
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
        return Ok(AttachOutcome::Desync(envelope.kind));
    };
    if accepted.attachment_id != attachment_id
        || accepted.terminal_id != terminal_id
        || accepted.capabilities != INPUT_CAPABILITIES
        || accepted.resize_lease
    {
        return Ok(AttachOutcome::Desync(envelope.kind));
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
        return Ok(InputOutcome::Desync(envelope.kind));
    };
    // A raw write is admitted byte-exact or not at all; a semantic key and a
    // mode-encoded paste report whatever byte count the Host produced under
    // the terminal's own modes, so only the identity echo and the protocol
    // byte bound apply to those records.
    if accepted_attachment != attachment_id
        || accepted_revision != capability_revision
        || expected_bytes.is_some_and(|expected| accepted_bytes != expected)
    {
        return Ok(InputOutcome::Desync(envelope.kind));
    }
    let Ok(bytes) = SendAcceptedBytes::new(accepted_bytes) else {
        return Ok(InputOutcome::Desync(envelope.kind));
    };
    Ok(InputOutcome::Accepted(bytes))
}

/// Detaches the send's attachment under the exchange bound, on the paths where
/// the carrier stayed in step: a typed input refusal, a validated
/// InputAccepted, an intact answer this Worker could not accept, or a
/// determinate stop between records. The Host's acceptance or refusal ends the
/// attachment; a refusal is recorded. Another intact answer, a Detach the
/// request has no room for, or one that could not be built leaves the
/// attachment to a later cleanup. Only a carrier that failed while detaching
/// is lost; a Server-side failure here ends the session through that loss, so
/// the pending result is never fabricated.
fn detach(
    host: &HostBrokerConnection,
    server: &UnixStream,
    attachment_id: AttachmentId,
) -> DetachOutcome {
    let deferred = || {
        host.defer_detach(*attachment_id.as_bytes());
        DetachOutcome::Unconfirmed
    };
    let target = AttachmentTarget { attachment_id };
    let Ok(command) = encode_attachment_control(&AttachmentControl::DetachTerminal(target)) else {
        return deferred();
    };
    let answer = match exchange_fresh(
        host,
        server,
        command.kind,
        &command.payload,
        PayloadEncoding::ControlJson,
    ) {
        Ok(answer) => answer,
        Err(_server) => return DetachOutcome::Lost,
    };
    let record = match answer {
        StepAnswer::Record(record) => record,
        StepAnswer::Desync(kind) => {
            host_unexpected(Site::Send, kind, Anomaly::Answer);
            return deferred();
        }
        StepAnswer::NotStarted | StepAnswer::LocalFault => return deferred(),
        StepAnswer::TransportLost | StepAnswer::ProtocolFault => return DetachOutcome::Lost,
    };
    let Ok(envelope) = decode_transport_envelope(&record) else {
        host_unexpected(Site::Send, record_kind(&record), Anomaly::Answer);
        return deferred();
    };
    if matches!(
        decode_connection_control(envelope.kind, envelope.role, envelope.payload),
        Ok(ConnectionControl::TypedFailure(_))
    ) {
        host_unexpected(Site::Send, envelope.kind, Anomaly::Answer);
        return DetachOutcome::Released;
    }
    match decode_attachment_control(envelope.kind, envelope.role, envelope.payload) {
        Ok(AttachmentControl::DetachTerminalAccepted(accepted)) if accepted == target => {
            DetachOutcome::Released
        }
        // Only the requested Host acknowledgment can confirm this attachment/process operation.
        _ => {
            host_unexpected(Site::Send, envelope.kind, Anomaly::Answer);
            deferred()
        }
    }
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
/// operation. `host_failed` here means the carrier failed while detaching.
fn respond(
    request: &TerminalSendRequest,
    outcome: TerminalSendOutcome,
    host: &HostBrokerConnection,
    host_failed: bool,
) -> Result<SendExecution, Box<dyn Error>> {
    if host_failed {
        // A failed exchange recorded its own class first; an answer that left the carrier
        // untrusted is recorded as a protocol failure.
        host.pacing.note_failure(protocol_fault());
        let _closed = host.stream().shutdown(Shutdown::Both);
    }
    let progress = outcome.progress();
    let result = match TerminalSendResult::new(request.request_id, outcome) {
        Ok(result) => result,
        Err(_) => {
            send_outcome_unbuildable();
            conservative_result(request, progress)?
        }
    };
    Ok(SendExecution {
        result,
        host_failed,
    })
}

/// The answer for an outcome this Worker built that the protocol refuses. The input may have
/// reached the Host, so the answer is unknown with cleanup unconfirmed, keeping the progress when
/// it is itself valid. The Worker's own constructors produce valid outcomes today; this path keeps
/// a refusal, now or later, from ending the Worker or the other requests it serves.
fn conservative_result(
    request: &TerminalSendRequest,
    progress: SendProgress,
) -> Result<TerminalSendResult, Box<dyn Error>> {
    match TerminalSendResult::new(
        request.request_id,
        TerminalSendOutcome::OutcomeUnknown {
            progress,
            cleanup: SendCleanupStatus::Unconfirmed,
        },
    ) {
        Ok(result) => Ok(result),
        // Nothing accepted and nothing stopped is valid for every cleanup status.
        Err(_) => Ok(TerminalSendResult::new(
            request.request_id,
            TerminalSendOutcome::OutcomeUnknown {
                progress: SendProgress::none(),
                cleanup: SendCleanupStatus::Unconfirmed,
            },
        )?),
    }
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

/// A carrier ended for an answer it could not trust, with no exchange bound involved.
const fn protocol_fault() -> HostFailure {
    HostFailure {
        class: PeerFailure::Protocol,
        deadline: false,
        waited: Duration::ZERO,
        budget: Duration::ZERO,
    }
}

fn server_interference() -> Box<dyn Error> {
    std::io::Error::other("Server stream interfered with the Host send exchange").into()
}
