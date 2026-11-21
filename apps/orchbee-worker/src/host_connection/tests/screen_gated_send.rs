//! Screen-gated send witness: a `Screen` request is one Host `ScreenGatedKeyInput` between the
//! transient attach and detach; each screen refusal comes back with its state and zero progress,
//! leaves the attachment detached and serves the next send; a hold is accepted with zero bytes;
//! and a Host without `screen-gated-input` is refused before any record reaches it.
use super::super::*;
use orchbee_capability_node_protocol::{
    HostCreationId, HostTerminalId, HostTerminalIdentity, OperationId, SendCleanupStatus, SendKey,
    SendKeyStroke, SendPrecondition, SendProgress, SendRecord, SendRecords, SendRejection,
    SendScreenGate, SendScreenState, SendStrokeKey, TerminalSendOutcome, TerminalSendRequest,
    WorkerGeneration, WorkerId,
};
use orchbee_terminal_protocol::{
    AttachmentCapabilities, AttachmentControl, AttachmentId, ConnectionControl, Envelope,
    HostEpoch, InputAccepted, InputMessage, MessageRole, PayloadEncoding, ScreenGatedAction,
    ScreenGatedKey, ScreenGatedKeyInput, ScreenGatedKeyRequest, ScreenGatedProcess,
    TRANSPORT_ENVELOPE_HEADER_BYTES, TypedFailure, decode_attachment_control, decode_input_message,
    decode_transport_envelope, encode_attachment_control, encode_connection_control,
    encode_input_message, encode_transport_envelope,
};
use std::{
    cell::RefCell,
    error::Error,
    io::{Read, Write},
    num::{NonZeroU32, NonZeroU64},
    rc::Rc,
    thread,
};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
type PeerResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

const EPOCH: [u8; 16] = [4; 16];
const TERMINAL: [u8; 16] = [5; 16];
const REVISION: u64 = 9;

fn host(screen_gated_input: bool) -> TestResult<(HostBrokerConnection, UnixStream)> {
    let (stream, peer) = UnixStream::pair()?;
    Ok((
        HostBrokerConnection {
            endpoint: std::path::PathBuf::new(),
            host_epoch: HostEpoch::from_bytes(EPOCH)?,
            prompt_gated_input: true,
            screen_gated_input,
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

fn enter() -> TestResult<SendRecord> {
    Ok(SendRecord::Stroke(SendKeyStroke::new(
        SendStrokeKey::Named(SendKey::Enter),
        0,
    )?))
}

/// The gate the Server decided on for the Enter: the frame it read and the process it answers.
fn gate() -> TestResult<SendScreenGate> {
    Ok(SendScreenGate {
        pid: NonZeroU32::new(4242).ok_or("zero process")?,
        creation_time: 77,
        generation: NonZeroU64::MIN,
        revision: 12,
        first_revision: 11,
    })
}

fn screen_request(request: u8, precondition: SendPrecondition) -> TestResult<TerminalSendRequest> {
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
        precondition,
        records: SendRecords::new(vec![enter()?])?,
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

/// Serves one screen send: the attach, one input that `answer` replies to, then the Detach, which
/// the scripted Host accepts. Returns the input the Host received.
fn serve_one_send(
    peer: &mut UnixStream,
    answer: impl FnOnce(&ScreenGatedKeyInput) -> PeerResult<(u16, MessageRole, Vec<u8>)>,
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
    let InputMessage::ScreenGatedKeyInput(key) = &input else {
        return Err("the screen send did not type one ScreenGatedKeyInput".into());
    };
    let (kind, role, payload) = answer(key)?;
    reply(peer, &record, kind, role, &payload)?;

    let record = read_record(peer)?;
    let envelope = decode_transport_envelope(&record)?;
    let AttachmentControl::DetachTerminal(target) =
        decode_attachment_control(envelope.kind, envelope.role, envelope.payload)?
    else {
        return Err("the send did not close with DetachTerminal".into());
    };
    let released = encode_attachment_control(&AttachmentControl::DetachTerminalAccepted(target))?;
    reply(
        peer,
        &record,
        released.kind,
        released.role,
        &released.payload,
    )?;
    Ok(input)
}

/// Accepts the input, typing `bytes` of it.
fn accept(key: &ScreenGatedKeyInput, bytes: u32) -> PeerResult<(u16, MessageRole, Vec<u8>)> {
    let accepted = encode_input_message(&InputMessage::InputAccepted(InputAccepted {
        accepted_bytes: bytes,
        attachment_id: key.attachment_id,
        capability_revision: key.capability_revision,
    }))?;
    Ok((accepted.kind, accepted.role, accepted.payload))
}

/// Refuses the input with the Host's typed failure for one screen rule.
fn refuse(code: &str) -> PeerResult<(u16, MessageRole, Vec<u8>)> {
    let failure = encode_connection_control(&ConnectionControl::TypedFailure(TypedFailure {
        code: code.to_owned(),
        message: "the screen refused the key".to_owned(),
        retryable: false,
        reason: None,
    }))?;
    Ok((failure.kind, failure.role, failure.payload))
}

#[test]
fn screen_send_types_one_gated_key_and_reports_the_host_bytes() -> TestResult {
    let (host, mut peer) = host(true)?;
    let served = thread::spawn(move || -> PeerResult<InputMessage> {
        serve_one_send(&mut peer, |key| accept(key, 2))
    });
    assert_eq!(
        execute(
            screen_request(20, SendPrecondition::Screen(gate()?))?,
            &host
        )?,
        TerminalSendOutcome::Accepted {
            progress: SendProgress::complete(1, 2),
            cleanup: SendCleanupStatus::Confirmed,
        }
    );
    let input = served
        .join()
        .map_err(|_| "the scripted Host panicked")?
        .map_err(|error| -> Box<dyn Error> { error })?;
    let InputMessage::ScreenGatedKeyInput(key) = input else {
        return Err("the scripted Host received a non-screen input".into());
    };
    assert_eq!(key.attachment_id, AttachmentId::from_bytes([120; 16])?);
    assert_eq!(
        key.action,
        ScreenGatedAction::Key(ScreenGatedKeyRequest {
            key: ScreenGatedKey::Enter,
            process: ScreenGatedProcess {
                pid: NonZeroU32::new(4242).ok_or("zero process")?,
                creation_time: 77,
            },
            generation: NonZeroU64::MIN,
            revision: 12,
            first_revision: 11,
        })
    );
    Ok(())
}

#[test]
fn every_screen_refusal_types_nothing_and_keeps_the_carrier_for_the_next_send() -> TestResult {
    const REFUSALS: [(&str, SendScreenState); 6] = [
        ("screen-agent-gone", SendScreenState::AgentGone),
        ("screen-changed", SendScreenState::Changed),
        ("screen-already-typed", SendScreenState::AlreadyTyped),
        ("screen-input-pending", SendScreenState::InputPending),
        ("screen-person-input", SendScreenState::PersonInput),
        ("screen-person-recent", SendScreenState::PersonRecent),
    ];
    let (host, mut peer) = host(true)?;
    let served = thread::spawn(move || -> PeerResult {
        for (code, _) in REFUSALS {
            serve_one_send(&mut peer, |_| refuse(code))?;
        }
        serve_one_send(&mut peer, |key| accept(key, 2))?;
        Ok(())
    });
    for (index, (_, state)) in REFUSALS.into_iter().enumerate() {
        let request = screen_request(30 + index as u8, SendPrecondition::Screen(gate()?))?;
        assert_eq!(
            execute(request, &host)?,
            TerminalSendOutcome::Rejected {
                reason: SendRejection::ScreenRefused(state),
                progress: SendProgress::none(),
                cleanup: SendCleanupStatus::Confirmed,
            }
        );
    }
    assert_eq!(
        execute(
            screen_request(40, SendPrecondition::Screen(gate()?))?,
            &host
        )?,
        TerminalSendOutcome::Accepted {
            progress: SendProgress::complete(1, 2),
            cleanup: SendCleanupStatus::Confirmed,
        }
    );
    served
        .join()
        .map_err(|_| "the scripted Host panicked")?
        .map_err(|error| -> Box<dyn Error> { error })?;
    assert!(host.pending_detaches().is_empty());
    assert_eq!(host.pacing.failure(), None);
    Ok(())
}

#[test]
fn screen_hold_types_nothing_and_is_accepted_with_zero_bytes() -> TestResult {
    let (host, mut peer) = host(true)?;
    let served = thread::spawn(move || -> PeerResult<InputMessage> {
        serve_one_send(&mut peer, |key| accept(key, 0))
    });
    assert_eq!(
        execute(screen_request(50, SendPrecondition::ScreenHold)?, &host)?,
        TerminalSendOutcome::Accepted {
            progress: SendProgress::complete(1, 0),
            cleanup: SendCleanupStatus::Confirmed,
        }
    );
    let input = served
        .join()
        .map_err(|_| "the scripted Host panicked")?
        .map_err(|error| -> Box<dyn Error> { error })?;
    let InputMessage::ScreenGatedKeyInput(key) = input else {
        return Err("the scripted Host received a non-screen input".into());
    };
    assert_eq!(key.action, ScreenGatedAction::Hold);
    Ok(())
}

#[test]
fn screen_send_to_an_ungated_host_is_refused_before_any_record() -> TestResult {
    let (host, mut peer) = host(false)?;
    for (request, precondition) in [
        (60, SendPrecondition::Screen(gate()?)),
        (61, SendPrecondition::ScreenHold),
    ] {
        assert_eq!(
            execute(screen_request(request, precondition)?, &host)?,
            TerminalSendOutcome::Rejected {
                reason: SendRejection::HostScreenGateUnsupported,
                progress: SendProgress::none(),
                cleanup: SendCleanupStatus::NotNeeded,
            }
        );
    }
    drop(host);
    let mut written = Vec::new();
    peer.read_to_end(&mut written)?;
    assert!(written.is_empty(), "the ungated Host received {written:?}");
    Ok(())
}
