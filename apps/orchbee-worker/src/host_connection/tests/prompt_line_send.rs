//! Idle-prompt send witness: the whole prompt line is one Host `PromptLineInput` between the
//! transient attach and detach, the Host's refusal state comes back with zero progress, a refused
//! Detach counts as released while another answer leaves it unconfirmed and the carrier serves the
//! next send either way, and a Host without `prompt-gated-input` is refused before any record
//! reaches it.
use super::super::*;
use orchbee_capability_node_protocol::{
    HostCreationId, HostTerminalId, HostTerminalIdentity, OperationId, SendCleanupStatus, SendKey,
    SendKeyStroke, SendPasteContent, SendPrecondition, SendProgress, SendPromptState, SendRecord,
    SendRecords, SendRejection, SendStrokeKey, TerminalSendOutcome, TerminalSendRequest,
    WorkerGeneration, WorkerId,
};
use orchbee_terminal_protocol::{
    AttachmentCapabilities, AttachmentControl, AttachmentId, AttachmentTarget, ConnectionControl,
    Envelope, HostEpoch, InputAccepted, InputMessage, MessageRole, PROMPT_UNCONFIRMED_FAILURE_CODE,
    PayloadEncoding, PromptLineInput, PromptUnconfirmedReason, TRANSPORT_ENVELOPE_HEADER_BYTES,
    TypedFailure, decode_attachment_control, decode_input_message, decode_transport_envelope,
    encode_attachment_control, encode_connection_control, encode_input_message,
    encode_transport_envelope,
};
use std::{
    cell::RefCell,
    error::Error,
    io::{Read, Write},
    num::NonZeroU64,
    rc::Rc,
    thread,
};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
type PeerResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

const EPOCH: [u8; 16] = [4; 16];
const TERMINAL: [u8; 16] = [5; 16];
const REVISION: u64 = 9;

fn host(prompt_gated_input: bool) -> TestResult<(HostBrokerConnection, UnixStream)> {
    let (stream, peer) = UnixStream::pair()?;
    Ok((
        HostBrokerConnection {
            endpoint: std::path::PathBuf::new(),
            host_epoch: HostEpoch::from_bytes(EPOCH)?,
            prompt_gated_input,
            screen_gated_send: prompt_gated_input,
            #[cfg(windows)]
            keeper: crate::host_connection::KeeperTerms::default(),
            #[cfg(windows)]
            keeper_events: std::cell::Cell::default(),
            background_programs: false,
            program_exits: std::cell::RefCell::default(),
            negotiated_limits: NegotiatedHostLimits {
                frame_bytes: 1_048_576,
                input_bytes: 4096,
                row_window_bytes: 4096,
            },
            stream,
            viewers: RefCell::new(viewers::HostViewers::default()),
            controllers: RefCell::new(controllers::HostControllers::default()),
            retired_targets: RefCell::new(target_cleanup::RetiredTargets::default()),
            titles: RefCell::new(titles::Titles::default()),
            host_colors: RefCell::new(crate::host_connection::HostColors::default()),
            drain: Rc::new(RefCell::new(crate::agent_drain::AgentDrain::new())),
            pacing: crate::host_connection::Pacing::default(),
            inbound: Default::default(),
        },
        peer,
    ))
}

fn stroke(key: SendKey) -> TestResult<SendRecord> {
    Ok(SendRecord::Stroke(SendKeyStroke::new(
        SendStrokeKey::Named(key),
        0,
    )?))
}

fn gated(request: u8) -> TestResult<TerminalSendRequest> {
    Ok(TerminalSendRequest {
        request_id: OperationId::from_bytes([request; 16])?,
        worker_id: WorkerId::from_bytes([3; 16])?,
        worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
        target: HostTerminalIdentity::new(
            HostCreationId::from_bytes([6; 16])?,
            orchbee_capability_node_protocol::HostEpoch::from_bytes(EPOCH)?,
            HostTerminalId::from_bytes(TERMINAL)?,
        ),
        attachment_id: OperationId::from_bytes([request.wrapping_add(100); 16])?,
        precondition: SendPrecondition::IdlePrompt,
        records: SendRecords::new(vec![
            SendRecord::Paste {
                content: SendPasteContent::new("codex resume '中文'".as_bytes().to_vec())?,
            },
            stroke(SendKey::Right)?,
            stroke(SendKey::Enter)?,
        ])?,
    })
}

fn execute(
    request: TerminalSendRequest,
    host: &HostBrokerConnection,
) -> TestResult<TerminalSendOutcome> {
    let (server, _quiet) = UnixStream::pair()?;
    let execution = crate::terminal_send_operation::execute(
        request,
        orchbee_local_worker_protocol::WorkerId::from_bytes([3; 16]),
        NonZeroU64::MIN,
        host,
        &server,
    )?;
    assert!(!execution.host_failed);
    Ok(execution.result.outcome())
}

fn read_record(peer: &mut UnixStream) -> PeerResult<Vec<u8>> {
    let mut header = [0_u8; TRANSPORT_ENVELOPE_HEADER_BYTES];
    peer.read_exact(&mut header)?;
    let length = usize::try_from(u32::from_le_bytes(header[8..12].try_into()?))?;
    let mut record = vec![0_u8; length];
    record[..TRANSPORT_ENVELOPE_HEADER_BYTES].copy_from_slice(&header);
    peer.read_exact(&mut record[TRANSPORT_ENVELOPE_HEADER_BYTES..])?;
    Ok(record)
}

fn reply(
    peer: &mut UnixStream,
    request: &[u8],
    kind: u16,
    role: MessageRole,
    payload: &[u8],
) -> PeerResult {
    let request = decode_transport_envelope(request)?;
    peer.write_all(&encode_transport_envelope(&Envelope {
        correlation_id: request.correlation_id,
        host_epoch: request.host_epoch,
        kind,
        optional_message: false,
        payload,
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: request.protocol,
        role,
    })?)?;
    Ok(())
}

/// How the scripted Host answers a send's Detach.
#[derive(Clone, Copy)]
enum Detach {
    Accept,
    Refuse,
    /// Accepts the Detach of another attachment.
    Mismatch,
}

/// Serves attach, one input, and detach; `answer` decides the input's reply and `detach` the
/// Detach's. Returns the input.
fn serve_one_send(
    peer: &mut UnixStream,
    answer: impl FnOnce(&PromptLineInput) -> PeerResult<(u16, MessageRole, Vec<u8>)>,
    detach: Detach,
) -> PeerResult<InputMessage> {
    let record = read_record(peer)?;
    let envelope = decode_transport_envelope(&record)?;
    let AttachmentControl::AttachTerminal(attach) =
        decode_attachment_control(envelope.kind, envelope.role, envelope.payload)?
    else {
        return Err("the send did not open with AttachTerminal".into());
    };
    let accepted = encode_attachment_control(&AttachmentControl::AttachmentCapabilities(
        AttachmentCapabilities {
            attachment_id: attach.attachment_id,
            capability_revision: NonZeroU64::new(REVISION).ok_or("zero revision")?,
            capabilities: attach.capabilities,
            resize_lease: false,
            terminal_id: attach.terminal_id,
        },
    ))?;
    reply(
        peer,
        &record,
        accepted.kind,
        accepted.role,
        &accepted.payload,
    )?;

    let record = read_record(peer)?;
    let envelope = decode_transport_envelope(&record)?;
    assert_eq!(envelope.payload_encoding, PayloadEncoding::RawBytes);
    let input = decode_input_message(envelope.kind, envelope.role, envelope.payload)?;
    let InputMessage::PromptLineInput(line) = &input else {
        return Err("the gated send did not type one PromptLineInput".into());
    };
    let (kind, role, payload) = answer(line)?;
    reply(peer, &record, kind, role, &payload)?;

    let record = read_record(peer)?;
    let envelope = decode_transport_envelope(&record)?;
    let AttachmentControl::DetachTerminal(target) =
        decode_attachment_control(envelope.kind, envelope.role, envelope.payload)?
    else {
        return Err("the send did not close with DetachTerminal".into());
    };
    let (kind, role, payload) = match detach {
        Detach::Accept => {
            let released =
                encode_attachment_control(&AttachmentControl::DetachTerminalAccepted(target))?;
            (released.kind, released.role, released.payload)
        }
        Detach::Mismatch => {
            let other = encode_attachment_control(&AttachmentControl::DetachTerminalAccepted(
                AttachmentTarget {
                    attachment_id: AttachmentId::from_bytes([0x12; 16])?,
                },
            ))?;
            (other.kind, other.role, other.payload)
        }
        Detach::Refuse => {
            let refused =
                encode_connection_control(&ConnectionControl::TypedFailure(TypedFailure {
                    code: "attachment-unavailable".to_owned(),
                    message: "no such attachment".to_owned(),
                    retryable: false,
                    reason: None,
                }))?;
            (refused.kind, refused.role, refused.payload)
        }
    };
    reply(peer, &record, kind, role, &payload)?;
    Ok(input)
}

/// Accepts a whole prompt line.
fn accept_line(line: &PromptLineInput) -> PeerResult<(u16, MessageRole, Vec<u8>)> {
    let accepted = encode_input_message(&InputMessage::InputAccepted(InputAccepted {
        accepted_bytes: 40,
        attachment_id: line.attachment_id,
        capability_revision: line.capability_revision,
    }))?;
    Ok((accepted.kind, accepted.role, accepted.payload))
}

#[test]
fn idle_prompt_send_types_one_prompt_line_and_returns_the_host_refusal() -> TestResult {
    let (host, mut peer) = host(true)?;
    let served = thread::spawn(move || -> PeerResult<Vec<InputMessage>> {
        let accepted = serve_one_send(&mut peer, accept_line, Detach::Accept)?;
        let refused = serve_one_send(
            &mut peer,
            |_| {
                let failure =
                    encode_connection_control(&ConnectionControl::TypedFailure(TypedFailure {
                        code: PROMPT_UNCONFIRMED_FAILURE_CODE.to_owned(),
                        message: "the prompt is still settling".to_owned(),
                        retryable: true,
                        reason: Some(PromptUnconfirmedReason::Settling),
                    }))?;
                Ok((failure.kind, failure.role, failure.payload))
            },
            Detach::Accept,
        )?;
        Ok(vec![accepted, refused])
    });

    assert_eq!(
        execute(gated(20)?, &host)?,
        TerminalSendOutcome::Accepted {
            progress: SendProgress::complete(3, 40),
            cleanup: SendCleanupStatus::Confirmed,
        }
    );
    // Settling is returned, not waited out: the caller owns the single retry.
    assert_eq!(
        execute(gated(21)?, &host)?,
        TerminalSendOutcome::Rejected {
            reason: SendRejection::PromptUnconfirmed(SendPromptState::Settling),
            progress: SendProgress::none(),
            cleanup: SendCleanupStatus::Confirmed,
        }
    );
    let inputs = served
        .join()
        .map_err(|_| "the scripted Host panicked")?
        .map_err(|error| -> Box<dyn Error> { error })?;
    for input in inputs {
        let InputMessage::PromptLineInput(line) = input else {
            return Err("the scripted Host returned a non-prompt-line input".into());
        };
        assert_eq!(line.content, "codex resume '中文'");
        assert!(line.right_arrow);
        assert_eq!(line.capability_revision.get(), REVISION);
    }
    Ok(())
}

#[test]
fn a_send_detach_refused_or_answered_otherwise_keeps_the_carrier_for_the_next_send() -> TestResult {
    let (host, mut peer) = host(true)?;
    let served = thread::spawn(move || -> PeerResult<UnixStream> {
        for detach in [Detach::Refuse, Detach::Mismatch, Detach::Accept] {
            serve_one_send(&mut peer, accept_line, detach)?;
        }
        Ok(peer)
    });
    let accepted = |cleanup| TerminalSendOutcome::Accepted {
        progress: SendProgress::complete(3, 40),
        cleanup,
    };
    // The Host no longer holds the attachment it refuses to detach: nothing waits for cleanup.
    assert_eq!(
        execute(gated(40)?, &host)?,
        accepted(SendCleanupStatus::Confirmed)
    );
    assert!(host.pending_detaches().is_empty());
    // Another answer leaves the attachment to a later cleanup, unconfirmed.
    assert_eq!(
        execute(gated(41)?, &host)?,
        accepted(SendCleanupStatus::Unconfirmed)
    );
    assert_eq!(host.pending_detaches(), [[141; 16]]);
    assert_eq!(
        execute(gated(42)?, &host)?,
        accepted(SendCleanupStatus::Confirmed)
    );
    let mut peer = served
        .join()
        .map_err(|_| "the scripted Host panicked")?
        .map_err(|error| -> Box<dyn Error> { error })?;
    assert_eq!(host.pacing.failure(), None);
    peer.set_nonblocking(true)?;
    assert_eq!(
        peer.read(&mut [0]).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    Ok(())
}

#[test]
fn idle_prompt_send_to_an_ungated_host_is_refused_before_any_record() -> TestResult {
    let (host, mut peer) = host(false)?;
    assert_eq!(
        execute(gated(30)?, &host)?,
        TerminalSendOutcome::Rejected {
            reason: SendRejection::HostPromptGateUnsupported,
            progress: SendProgress::none(),
            cleanup: SendCleanupStatus::NotNeeded,
        }
    );
    drop(host);
    let mut written = Vec::new();
    peer.read_to_end(&mut written)?;
    assert!(written.is_empty(), "the ungated Host received {written:?}");
    Ok(())
}
