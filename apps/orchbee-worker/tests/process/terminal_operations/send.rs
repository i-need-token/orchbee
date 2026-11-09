use super::*;
use orchbee_capability_node_protocol::{
    HostCreationId, HostTerminalIdentity, SendAcceptedBytes, SendCleanupStatus, SendKey,
    SendPrecondition, SendProgress, SendRecord, SendRecords, SendRejection, SendText,
    SendTextInput, SendUnavailable, TerminalSendOutcome, TerminalSendRequest, TerminalSendResult,
};
use orchbee_terminal_protocol::{
    AttachTerminal, AttachmentCapabilities, AttachmentCapabilityState, AttachmentControl,
    AttachmentId, AttachmentTarget, ConnectionControl, EncodedAttachmentControl,
    EncodedConnectionControl, EncodedInputMessage, InputAccepted, InputMessage, KeyAction,
    TerminalId as HostTerminalId, TypedFailure, WriteInput, decode_attachment_control,
    decode_input_message, encode_attachment_control, encode_connection_control,
    encode_input_message,
};

const SCRIPT_EPOCH: [u8; 16] = [0x44; 16];
const CREATION: [u8; 16] = [0x55; 16];
const TERMINAL: [u8; 16] = [0x66; 16];
const ATTACHMENT: [u8; 16] = [0x77; 16];
const REVISION: u64 = 41;

fn identity(epoch: [u8; 16], terminal: [u8; 16]) -> TestResult<HostTerminalIdentity> {
    Ok(HostTerminalIdentity::new(
        HostCreationId::from_bytes(CREATION)?,
        orchbee_capability_node_protocol::HostEpoch::from_bytes(epoch)?,
        orchbee_capability_node_protocol::HostTerminalId::from_bytes(terminal)?,
    ))
}

fn send_request(
    request: u8,
    generation: u64,
    target: HostTerminalIdentity,
    records: SendRecords,
) -> TestResult<TerminalSendRequest> {
    Ok(TerminalSendRequest {
        request_id: OperationId::from_bytes([request; 16])?,
        worker_id: CapabilityWorkerId::from_bytes(LOCAL_WORKER_ID.as_bytes())?,
        worker_generation: WorkerGeneration::new(nonzero(generation)?),
        target,
        attachment_id: OperationId::from_bytes(ATTACHMENT)?,
        precondition: SendPrecondition::None,
        records,
    })
}

fn send(server: &mut UnixStream, request: TerminalSendRequest) -> TestResult<TerminalSendResult> {
    write_obcn(
        server,
        &CapabilityNodeMessage::TerminalSendRequest(request.clone()),
    )?;
    let CapabilityNodeMessage::TerminalSendResult(result) = read_obcn(server)? else {
        return Err(io::Error::other("missing Send result").into());
    };
    assert_eq!(result.request_id(), request.request_id);
    Ok(result)
}

fn text(value: &str, enter: bool) -> TestResult<SendRecords> {
    Ok(SendRecords::new(vec![SendRecord::Text(
        SendTextInput::new(SendText::new(value.to_owned())?, enter)?,
    )])?)
}

fn one_key(key: SendKey) -> TestResult<SendRecords> {
    Ok(SendRecords::new(vec![SendRecord::Stroke(
        orchbee_capability_node_protocol::SendKeyStroke::new(
            orchbee_capability_node_protocol::SendStrokeKey::Named(key),
            0,
        )?,
    )])?)
}

fn scripted_epoch() -> TestResult<HostEpoch> {
    Ok(HostEpoch::from_bytes(SCRIPT_EPOCH)?)
}

fn scripted_attachment() -> TestResult<AttachmentId> {
    Ok(AttachmentId::from_bytes(ATTACHMENT)?)
}

fn expect_attach(host: &mut UnixStream) -> TestResult<(AttachTerminal, CorrelationId)> {
    let record = read_terminal_record(host)?;
    let envelope = decode_transport_envelope(&record)?;
    assert_eq!(envelope.role, MessageRole::Request);
    assert_eq!(envelope.host_epoch, Some(scripted_epoch()?));
    let AttachmentControl::AttachTerminal(attach) =
        decode_attachment_control(envelope.kind, envelope.role, envelope.payload)?
    else {
        return Err(io::Error::other("missing AttachTerminal").into());
    };
    Ok((
        attach,
        envelope
            .correlation_id
            .ok_or_else(|| io::Error::other("missing attach correlation"))?,
    ))
}

fn expect_input(host: &mut UnixStream) -> TestResult<(InputMessage, CorrelationId)> {
    let record = read_terminal_record(host)?;
    let envelope = decode_transport_envelope(&record)?;
    assert_eq!(envelope.role, MessageRole::Request);
    let input = decode_input_message(envelope.kind, envelope.role, envelope.payload)?;
    Ok((
        input,
        envelope
            .correlation_id
            .ok_or_else(|| io::Error::other("missing input correlation"))?,
    ))
}

fn expect_detach(host: &mut UnixStream) -> TestResult<(AttachmentTarget, CorrelationId)> {
    let record = read_terminal_record(host)?;
    let envelope = decode_transport_envelope(&record)?;
    assert_eq!(envelope.role, MessageRole::Request);
    let AttachmentControl::DetachTerminal(target) =
        decode_attachment_control(envelope.kind, envelope.role, envelope.payload)?
    else {
        return Err(io::Error::other("missing DetachTerminal").into());
    };
    Ok((
        target,
        envelope
            .correlation_id
            .ok_or_else(|| io::Error::other("missing detach correlation"))?,
    ))
}

fn respond_capabilities(
    host: &mut UnixStream,
    correlation: CorrelationId,
    attach: &AttachTerminal,
) -> TestResult {
    let EncodedAttachmentControl {
        kind,
        role,
        payload,
    } = encode_attachment_control(&AttachmentControl::AttachmentCapabilities(
        AttachmentCapabilities {
            attachment_id: attach.attachment_id,
            capability_revision: nonzero(REVISION)?,
            capabilities: attach.capabilities,
            resize_lease: false,
            terminal_id: attach.terminal_id,
        },
    ))?;
    host.write_all(&host_record(
        kind,
        role,
        &payload,
        scripted_epoch()?,
        correlation,
    )?)?;
    Ok(())
}

fn respond_desynced_capabilities(
    host: &mut UnixStream,
    correlation: CorrelationId,
    attach: &AttachTerminal,
) -> TestResult {
    let EncodedAttachmentControl {
        kind,
        role,
        payload,
    } = encode_attachment_control(&AttachmentControl::AttachmentCapabilities(
        AttachmentCapabilities {
            attachment_id: AttachmentId::from_bytes([0x12; 16])?,
            capability_revision: nonzero(REVISION)?,
            capabilities: attach.capabilities,
            resize_lease: false,
            terminal_id: attach.terminal_id,
        },
    ))?;
    host.write_all(&host_record(
        kind,
        role,
        &payload,
        scripted_epoch()?,
        correlation,
    )?)?;
    Ok(())
}

fn respond_input_accepted(
    host: &mut UnixStream,
    correlation: CorrelationId,
    attachment_id: AttachmentId,
    revision: u64,
    bytes: u32,
) -> TestResult {
    let EncodedInputMessage {
        kind,
        role,
        payload,
    } = encode_input_message(&InputMessage::InputAccepted(InputAccepted {
        accepted_bytes: bytes,
        attachment_id,
        capability_revision: nonzero(revision)?,
    }))?;
    host.write_all(&host_record(
        kind,
        role,
        &payload,
        scripted_epoch()?,
        correlation,
    )?)?;
    Ok(())
}

fn respond_detach(
    host: &mut UnixStream,
    correlation: CorrelationId,
    target: AttachmentTarget,
) -> TestResult {
    let EncodedAttachmentControl {
        kind,
        role,
        payload,
    } = encode_attachment_control(&AttachmentControl::DetachTerminalAccepted(target))?;
    host.write_all(&host_record(
        kind,
        role,
        &payload,
        scripted_epoch()?,
        correlation,
    )?)?;
    Ok(())
}

fn respond_failure(host: &mut UnixStream, correlation: CorrelationId, code: &str) -> TestResult {
    let EncodedConnectionControl {
        kind,
        role,
        payload,
    } = encode_connection_control(&ConnectionControl::TypedFailure(TypedFailure {
        code: code.to_owned(),
        message: format!("scripted Host refusal: {code}"),
        retryable: false,
        reason: None,
    }))?;
    host.write_all(&host_record(
        kind,
        role,
        &payload,
        scripted_epoch()?,
        correlation,
    )?)?;
    Ok(())
}

/// Attach → input → detach happy path against one scripted Host connection.
/// Returns after the detach acknowledgement was sent. Every step rides its
/// own fresh correlation; the three must be pairwise distinct.
fn drive_send_happy_path(host: &mut UnixStream, input_bytes: u32) -> TestResult {
    let (attach, attach_correlation) = expect_attach(host)?;
    assert_eq!(attach.attachment_id, scripted_attachment()?);
    assert_eq!(
        attach.capabilities,
        AttachmentCapabilityState {
            read: false,
            input: true,
            resize: false,
        }
    );
    assert!(!attach.request_resize_lease);
    respond_capabilities(host, attach_correlation, &attach)?;
    let (input, input_correlation) = expect_input(host)?;
    let InputMessage::WriteInput(WriteInput {
        attachment_id: written_attachment,
        capability_revision,
        bytes,
    }) = input
    else {
        return Err(io::Error::other("missing WriteInput").into());
    };
    assert_eq!(written_attachment, scripted_attachment()?);
    assert_eq!(capability_revision, nonzero(REVISION)?);
    assert_eq!(bytes.len() as u32, input_bytes);
    respond_input_accepted(
        host,
        input_correlation,
        attach.attachment_id,
        REVISION,
        input_bytes,
    )?;
    let (target, detach_correlation) = expect_detach(host)?;
    assert_eq!(target.attachment_id, attach.attachment_id);
    // Fresh correlation per Host step: attach, input, and detach identities
    // must be pairwise distinct.
    assert_ne!(attach_correlation, input_correlation);
    assert_ne!(attach_correlation, detach_correlation);
    assert_ne!(input_correlation, detach_correlation);
    respond_detach(host, detach_correlation, target)
}

#[test]
fn send_fence_rejections_never_reach_the_host() -> TestResult {
    let fixture = RuntimeFixture::new()?;
    let listener = fixture.bind_host_listener()?;
    let host = thread::spawn(move || -> io::Result<()> {
        let mut host = admit_host(&listener)?;
        // Fenced sends must produce no Host records at all; the connection
        // stays healthy and only closes when the Worker exits.
        require_connection_closed(&mut host)
    });
    let worker = RunningWorker::start(&fixture)?;
    let (mut server, _) = admit_worker(&fixture.server_listener)?;
    assert_eq!(worker.await_readiness()?, Some(*b"ready\n"));

    let mut wrong_worker =
        send_request(1, 7, identity(SCRIPT_EPOCH, TERMINAL)?, text("ls", false)?)?;
    wrong_worker.worker_id = CapabilityWorkerId::from_bytes([0x99; 16])?;
    assert_eq!(
        send(&mut server, wrong_worker)?.outcome(),
        TerminalSendOutcome::Rejected {
            progress: SendProgress::none(),
            reason: SendRejection::WrongWorker,
            cleanup: SendCleanupStatus::NotNeeded,
        }
    );
    assert_eq!(
        send(
            &mut server,
            send_request(2, 8, identity(SCRIPT_EPOCH, TERMINAL)?, text("ls", false)?)?,
        )?
        .outcome(),
        TerminalSendOutcome::Rejected {
            progress: SendProgress::none(),
            reason: SendRejection::StaleWorkerGeneration,
            cleanup: SendCleanupStatus::NotNeeded,
        }
    );
    assert_eq!(
        send(
            &mut server,
            send_request(3, 7, identity([0x99; 16], TERMINAL)?, text("ls", false)?)?,
        )?
        .outcome(),
        TerminalSendOutcome::Rejected {
            progress: SendProgress::none(),
            reason: SendRejection::StaleHostEpoch,
            cleanup: SendCleanupStatus::NotNeeded,
        }
    );
    drop(server);
    assert_failed_after_exact_readiness(worker.finish()?);
    join_peer(host, "Host fence observer")?;
    Ok(())
}

#[test]
fn send_text_runs_one_attach_input_detach_and_reports_confirmed() -> TestResult {
    let fixture = RuntimeFixture::new()?;
    let listener = fixture.bind_host_listener()?;
    let host = thread::spawn(move || -> io::Result<()> {
        let mut host = admit_host(&listener)?;
        drive_send_happy_path(&mut host, 7).map_err(protocol_io)?;
        // The carrier stays healthy: a second send reuses it for a full cycle.
        drive_send_happy_path(&mut host, 2).map_err(protocol_io)?;
        require_connection_closed(&mut host)
    });
    let worker = RunningWorker::start(&fixture)?;
    let (mut server, _) = admit_worker(&fixture.server_listener)?;
    assert_eq!(worker.await_readiness()?, Some(*b"ready\n"));
    assert_eq!(
        send(
            &mut server,
            send_request(
                1,
                7,
                identity(SCRIPT_EPOCH, TERMINAL)?,
                text("ls -la", true)?
            )?,
        )?
        .outcome(),
        TerminalSendOutcome::Accepted {
            progress: SendProgress::complete(1, 7),
            cleanup: SendCleanupStatus::Confirmed,
        }
    );
    assert_eq!(
        send(
            &mut server,
            send_request(2, 7, identity(SCRIPT_EPOCH, TERMINAL)?, text("ls", false)?)?,
        )?
        .outcome(),
        TerminalSendOutcome::Accepted {
            progress: SendProgress::complete(1, 2),
            cleanup: SendCleanupStatus::Confirmed,
        }
    );
    drop(server);
    assert_failed_after_exact_readiness(worker.finish()?);
    join_peer(host, "Host send observer")?;
    Ok(())
}

#[test]
fn send_key_emits_one_semantic_record_and_echoes_host_byte_count() -> TestResult {
    let fixture = RuntimeFixture::new()?;
    let listener = fixture.bind_host_listener()?;
    let host = thread::spawn(move || -> io::Result<()> {
        let mut host = admit_host(&listener)?;
        let (attach, correlation) = expect_attach(&mut host).map_err(protocol_io)?;
        respond_capabilities(&mut host, correlation, &attach).map_err(protocol_io)?;
        let (input, correlation) = expect_input(&mut host).map_err(protocol_io)?;
        let InputMessage::KeyInput(key) = input else {
            return Err(io::Error::other("missing KeyInput"));
        };
        assert_eq!(key.action, KeyAction::Press);
        assert_eq!(
            key.attachment_id,
            scripted_attachment().map_err(protocol_io)?
        );
        assert_eq!(
            key.capability_revision,
            nonzero(REVISION).map_err(protocol_io)?
        );
        assert_eq!(key.key.as_str(), "KeyC");
        assert!(key.modifiers.control);
        assert!(!key.modifiers.shift && !key.modifiers.alt);
        assert_eq!(key.unshifted_codepoint, u32::from(b'c'));
        // The Host reports its own encoded byte count; the Worker echoes it.
        respond_input_accepted(&mut host, correlation, attach.attachment_id, REVISION, 1)
            .map_err(protocol_io)?;
        let (target, correlation) = expect_detach(&mut host).map_err(protocol_io)?;
        respond_detach(&mut host, correlation, target).map_err(protocol_io)?;
        require_connection_closed(&mut host)
    });
    let worker = RunningWorker::start(&fixture)?;
    let (mut server, _) = admit_worker(&fixture.server_listener)?;
    assert_eq!(worker.await_readiness()?, Some(*b"ready\n"));
    assert_eq!(
        send(
            &mut server,
            send_request(
                1,
                7,
                identity(SCRIPT_EPOCH, TERMINAL)?,
                SendRecords::new(vec![SendRecord::Stroke(
                    orchbee_capability_node_protocol::SendKeyStroke::new(
                        orchbee_capability_node_protocol::SendStrokeKey::Character('c'),
                        1
                    )?
                )])?,
            )?,
        )?
        .outcome(),
        TerminalSendOutcome::Accepted {
            progress: SendProgress::complete(1, 1),
            cleanup: SendCleanupStatus::Confirmed,
        }
    );
    drop(server);
    assert_failed_after_exact_readiness(worker.finish()?);
    join_peer(host, "Host key observer")?;
    Ok(())
}

#[test]
fn send_attach_typed_failures_are_typed_rejections_without_cleanup() -> TestResult {
    let fixture = RuntimeFixture::new()?;
    let listener = fixture.bind_host_listener()?;
    let host = thread::spawn(move || -> io::Result<()> {
        let mut host = admit_host(&listener)?;
        // Both refusals produce no DetachTerminal: nothing was ever created.
        let (attach, correlation) = expect_attach(&mut host).map_err(protocol_io)?;
        let _ = attach;
        respond_failure(&mut host, correlation, "missing-resource").map_err(protocol_io)?;
        let (_attach, correlation) = expect_attach(&mut host).map_err(protocol_io)?;
        respond_failure(&mut host, correlation, "invalid-phase").map_err(protocol_io)?;
        // A following send still completes on the healthy carrier.
        drive_send_happy_path(&mut host, 2).map_err(protocol_io)?;
        require_connection_closed(&mut host)
    });
    let worker = RunningWorker::start(&fixture)?;
    let (mut server, _) = admit_worker(&fixture.server_listener)?;
    assert_eq!(worker.await_readiness()?, Some(*b"ready\n"));
    for (request, reason) in [
        (1, SendRejection::TerminalUnavailable),
        (2, SendRejection::HostRejected),
    ] {
        assert_eq!(
            send(
                &mut server,
                send_request(
                    request,
                    7,
                    identity(SCRIPT_EPOCH, TERMINAL)?,
                    text("ls", false)?
                )?,
            )?
            .outcome(),
            TerminalSendOutcome::Rejected {
                progress: SendProgress::none(),
                reason,
                cleanup: SendCleanupStatus::NotNeeded,
            }
        );
    }
    assert!(matches!(
        send(
            &mut server,
            send_request(3, 7, identity(SCRIPT_EPOCH, TERMINAL)?, text("ls", false)?)?,
        )?
        .outcome(),
        TerminalSendOutcome::Accepted {
            cleanup: SendCleanupStatus::Confirmed,
            ..
        }
    ));
    drop(server);
    assert_failed_after_exact_readiness(worker.finish()?);
    join_peer(host, "Host refusal observer")?;
    Ok(())
}

#[test]
fn send_input_typed_failure_still_detaches_and_reports_cleanup() -> TestResult {
    let fixture = RuntimeFixture::new()?;
    let listener = fixture.bind_host_listener()?;
    let host = thread::spawn(move || -> io::Result<()> {
        let mut host = admit_host(&listener)?;
        let (attach, correlation) = expect_attach(&mut host).map_err(protocol_io)?;
        respond_capabilities(&mut host, correlation, &attach).map_err(protocol_io)?;
        let (_input, correlation) = expect_input(&mut host).map_err(protocol_io)?;
        respond_failure(&mut host, correlation, "limit-exceeded").map_err(protocol_io)?;
        let (target, correlation) = expect_detach(&mut host).map_err(protocol_io)?;
        respond_detach(&mut host, correlation, target).map_err(protocol_io)?;
        require_connection_closed(&mut host)
    });
    let worker = RunningWorker::start(&fixture)?;
    let (mut server, _) = admit_worker(&fixture.server_listener)?;
    assert_eq!(worker.await_readiness()?, Some(*b"ready\n"));
    assert_eq!(
        send(
            &mut server,
            send_request(1, 7, identity(SCRIPT_EPOCH, TERMINAL)?, text("ls", false)?)?,
        )?
        .outcome(),
        TerminalSendOutcome::Rejected {
            progress: SendProgress::none(),
            reason: SendRejection::HostInputLimitExceeded,
            cleanup: SendCleanupStatus::Confirmed,
        }
    );
    drop(server);
    assert_failed_after_exact_readiness(worker.finish()?);
    join_peer(host, "Host input-refusal observer")?;
    Ok(())
}

/// A corrupt input acknowledgement is desynchronization: the Worker never
/// resends and never issues another exchange on the stream — the peer must
/// observe that no third Host request ever left.
#[test]
fn send_corrupt_input_acknowledgement_never_resends_and_closes_host() -> TestResult {
    for (wrong_attachment, wrong_revision, wrong_bytes) in [
        (true, false, false),
        (false, true, false),
        (false, false, true),
    ] {
        let fixture = RuntimeFixture::new()?;
        let listener = fixture.bind_host_listener()?;
        let host = thread::spawn(move || -> io::Result<()> {
            let mut host = admit_host(&listener)?;
            let (attach, correlation) = expect_attach(&mut host).map_err(protocol_io)?;
            respond_capabilities(&mut host, correlation, &attach).map_err(protocol_io)?;
            let (_input, correlation) = expect_input(&mut host).map_err(protocol_io)?;
            let attachment = if wrong_attachment {
                AttachmentId::from_bytes([0x12; 16]).map_err(protocol_io)?
            } else {
                attach.attachment_id
            };
            let revision = if wrong_revision {
                REVISION + 1
            } else {
                REVISION
            };
            let bytes = if wrong_bytes { 1 } else { 2 };
            respond_input_accepted(&mut host, correlation, attachment, revision, bytes)
                .map_err(protocol_io)?;
            // No DetachTerminal and no resend: the desynchronized stream is
            // shut down without any further exchange.
            require_connection_closed(&mut host)
        });
        let worker = RunningWorker::start(&fixture)?;
        let (mut server, _) = admit_worker(&fixture.server_listener)?;
        assert_eq!(worker.await_readiness()?, Some(*b"ready\n"));
        assert_eq!(
            send(
                &mut server,
                send_request(1, 7, identity(SCRIPT_EPOCH, TERMINAL)?, text("ls", false)?)?,
            )?
            .outcome(),
            TerminalSendOutcome::OutcomeUnknown {
                progress: SendProgress::none(),
                cleanup: SendCleanupStatus::Unconfirmed,
            }
        );
        // A second request must never be answered: the session ended with the
        // first result instead of discovering the dead carrier on a later poll.
        let _written = write_obcn(
            &mut server,
            &CapabilityNodeMessage::TerminalSendRequest(send_request(
                9,
                7,
                identity(SCRIPT_EPOCH, TERMINAL)?,
                text("ls", false)?,
            )?),
        );
        require_connection_closed(&mut server)?;
        assert_failed_after_exact_readiness(worker.finish()?);
        join_peer(host, "Host desync observer")?;
    }
    Ok(())
}

#[test]
fn send_host_closure_positions_map_to_unavailable_or_unknown() -> TestResult {
    #[derive(Clone, Copy)]
    enum CloseAt {
        Handshake,
        AfterAttach,
        AfterInput,
        DuringDetach,
    }
    for (close_at, expected) in [
        (
            CloseAt::Handshake,
            TerminalSendOutcome::Unavailable {
                progress: SendProgress::none(),
                reason: SendUnavailable::HostUnavailable,
                cleanup: SendCleanupStatus::Unconfirmed,
            },
        ),
        (
            CloseAt::AfterAttach,
            TerminalSendOutcome::Unavailable {
                progress: SendProgress::none(),
                reason: SendUnavailable::HostUnavailable,
                cleanup: SendCleanupStatus::Unconfirmed,
            },
        ),
        (
            CloseAt::AfterInput,
            TerminalSendOutcome::OutcomeUnknown {
                progress: SendProgress::none(),
                cleanup: SendCleanupStatus::Unconfirmed,
            },
        ),
        (
            CloseAt::DuringDetach,
            TerminalSendOutcome::Accepted {
                progress: SendProgress::complete(1, 2),
                cleanup: SendCleanupStatus::Unconfirmed,
            },
        ),
    ] {
        let fixture = RuntimeFixture::new()?;
        let listener = fixture.bind_host_listener()?;
        let host = thread::spawn(move || -> io::Result<()> {
            let mut host = admit_host(&listener)?;
            match close_at {
                CloseAt::Handshake => {}
                CloseAt::AfterAttach => {
                    let _ = expect_attach(&mut host).map_err(protocol_io)?;
                }
                CloseAt::AfterInput => {
                    let (attach, correlation) = expect_attach(&mut host).map_err(protocol_io)?;
                    respond_capabilities(&mut host, correlation, &attach).map_err(protocol_io)?;
                    let _ = expect_input(&mut host).map_err(protocol_io)?;
                }
                CloseAt::DuringDetach => {
                    let (attach, correlation) = expect_attach(&mut host).map_err(protocol_io)?;
                    respond_capabilities(&mut host, correlation, &attach).map_err(protocol_io)?;
                    let (_input, correlation) = expect_input(&mut host).map_err(protocol_io)?;
                    respond_input_accepted(
                        &mut host,
                        correlation,
                        attach.attachment_id,
                        REVISION,
                        2,
                    )
                    .map_err(protocol_io)?;
                    let _ = expect_detach(&mut host).map_err(protocol_io)?;
                }
            }
            // Dropping the scripted side is the Host-closed condition.
            Ok(())
        });
        let worker = RunningWorker::start(&fixture)?;
        let (mut server, _) = admit_worker(&fixture.server_listener)?;
        assert_eq!(worker.await_readiness()?, Some(*b"ready\n"));
        let request = send_request(1, 7, identity(SCRIPT_EPOCH, TERMINAL)?, text("ls", false)?)?;
        if matches!(close_at, CloseAt::Handshake) {
            // The Host may die before the request is ever read: the Worker
            // exits on the dead carrier without answering. If the request
            // still made it in, the answer must be the Unavailable result —
            // either way the session ends immediately.
            let _written = write_obcn(
                &mut server,
                &CapabilityNodeMessage::TerminalSendRequest(request),
            );
            if let Ok(CapabilityNodeMessage::TerminalSendResult(result)) = read_obcn(&mut server) {
                assert_eq!(result.outcome(), expected);
            }
        } else {
            assert_eq!(send(&mut server, request)?.outcome(), expected);
        }
        require_connection_closed(&mut server)?;
        assert_failed_after_exact_readiness(worker.finish()?);
        join_peer(host, "closing Host")?;
    }
    Ok(())
}

/// A typed detach refusal is legible but proves nothing about cleanup: the
/// attachment may linger, so the carrier is closed — the accepted input fact
/// and the unconfirmed cleanup are still reported, and no further send may
/// stack another input-only attachment on this Host.
#[test]
fn send_detach_typed_failure_reports_unconfirmed_and_closes_the_carrier() -> TestResult {
    let fixture = RuntimeFixture::new()?;
    let listener = fixture.bind_host_listener()?;
    let host = thread::spawn(move || -> io::Result<()> {
        let mut host = admit_host(&listener)?;
        let (attach, correlation) = expect_attach(&mut host).map_err(protocol_io)?;
        respond_capabilities(&mut host, correlation, &attach).map_err(protocol_io)?;
        let (_input, correlation) = expect_input(&mut host).map_err(protocol_io)?;
        respond_input_accepted(&mut host, correlation, attach.attachment_id, REVISION, 2)
            .map_err(protocol_io)?;
        let (_target, correlation) = expect_detach(&mut host).map_err(protocol_io)?;
        respond_failure(&mut host, correlation, "invalid-phase").map_err(protocol_io)?;
        // The unconfirmed cleanup ends the carrier; nothing else is exchanged.
        require_connection_closed(&mut host)
    });
    let worker = RunningWorker::start(&fixture)?;
    let (mut server, _) = admit_worker(&fixture.server_listener)?;
    assert_eq!(worker.await_readiness()?, Some(*b"ready\n"));
    assert_eq!(
        send(
            &mut server,
            send_request(1, 7, identity(SCRIPT_EPOCH, TERMINAL)?, text("ls", false)?)?,
        )?
        .outcome(),
        TerminalSendOutcome::Accepted {
            progress: SendProgress::complete(1, 2),
            cleanup: SendCleanupStatus::Unconfirmed,
        }
    );
    let _written = write_obcn(
        &mut server,
        &CapabilityNodeMessage::TerminalSendRequest(send_request(
            9,
            7,
            identity(SCRIPT_EPOCH, TERMINAL)?,
            text("ls", false)?,
        )?),
    );
    require_connection_closed(&mut server)?;
    assert_failed_after_exact_readiness(worker.finish()?);
    join_peer(host, "Host detach-refusal observer")?;
    Ok(())
}

/// A desynchronized attach acknowledgement is a protocol fault: the stream is
/// shut down immediately — the peer must observe that no DetachTerminal or
/// any other request followed.
#[test]
fn send_attach_desync_shuts_down_without_another_exchange() -> TestResult {
    let fixture = RuntimeFixture::new()?;
    let listener = fixture.bind_host_listener()?;
    let host = thread::spawn(move || -> io::Result<()> {
        let mut host = admit_host(&listener)?;
        let (attach, correlation) = expect_attach(&mut host).map_err(protocol_io)?;
        // Acknowledge the wrong attachment identity: desync, not a refusal.
        respond_desynced_capabilities(&mut host, correlation, &attach).map_err(protocol_io)?;
        require_connection_closed(&mut host)
    });
    let worker = RunningWorker::start(&fixture)?;
    let (mut server, _) = admit_worker(&fixture.server_listener)?;
    assert_eq!(worker.await_readiness()?, Some(*b"ready\n"));
    assert_eq!(
        send(
            &mut server,
            send_request(1, 7, identity(SCRIPT_EPOCH, TERMINAL)?, text("ls", false)?)?,
        )?
        .outcome(),
        TerminalSendOutcome::Unavailable {
            progress: SendProgress::none(),
            reason: SendUnavailable::HostProtocolFault,
            cleanup: SendCleanupStatus::Unconfirmed,
        }
    );
    require_connection_closed(&mut server)?;
    assert_failed_after_exact_readiness(worker.finish()?);
    join_peer(host, "Host attach-desync observer")?;
    Ok(())
}

#[test]
fn send_text_boundary_lengths_encode_one_exact_record() -> TestResult {
    let fixture = RuntimeFixture::new()?;
    let listener = fixture.bind_host_listener()?;
    let host = thread::spawn(move || -> io::Result<()> {
        let mut host = admit_host(&listener)?;
        for expected in [4096_u32, 4096_u32] {
            let (attach, correlation) = expect_attach(&mut host).map_err(protocol_io)?;
            respond_capabilities(&mut host, correlation, &attach).map_err(protocol_io)?;
            let (input, correlation) = expect_input(&mut host).map_err(protocol_io)?;
            let InputMessage::WriteInput(write) = input else {
                return Err(io::Error::other("missing WriteInput"));
            };
            assert_eq!(write.bytes.len() as u32, expected);
            respond_input_accepted(
                &mut host,
                correlation,
                attach.attachment_id,
                REVISION,
                expected,
            )
            .map_err(protocol_io)?;
            let (target, correlation) = expect_detach(&mut host).map_err(protocol_io)?;
            respond_detach(&mut host, correlation, target).map_err(protocol_io)?;
        }
        require_connection_closed(&mut host)
    });
    let worker = RunningWorker::start(&fixture)?;
    let (mut server, _) = admit_worker(&fixture.server_listener)?;
    assert_eq!(worker.await_readiness()?, Some(*b"ready\n"));
    // 4096-byte text without enter and 4095-byte text with enter both encode
    // exactly 4096 input bytes; each still rides one complete cycle.
    assert_eq!(
        send(
            &mut server,
            send_request(
                1,
                7,
                identity(SCRIPT_EPOCH, TERMINAL)?,
                text(&"x".repeat(4096), false)?
            )?,
        )?
        .outcome(),
        TerminalSendOutcome::Accepted {
            progress: SendProgress::complete(1, 4096),
            cleanup: SendCleanupStatus::Confirmed,
        }
    );
    assert_eq!(
        send(
            &mut server,
            send_request(
                2,
                7,
                identity(SCRIPT_EPOCH, TERMINAL)?,
                text(&"x".repeat(4095), true)?
            )?,
        )?
        .outcome(),
        TerminalSendOutcome::Accepted {
            progress: SendProgress::complete(1, 4096),
            cleanup: SendCleanupStatus::Confirmed,
        }
    );
    drop(server);
    assert_failed_after_exact_readiness(worker.finish()?);
    join_peer(host, "Host boundary observer")?;
    Ok(())
}

/// A capabilities response with caller-chosen echo fields, for desync cases.
fn respond_capabilities_with(
    host: &mut UnixStream,
    correlation: CorrelationId,
    attachment_id: AttachmentId,
    capabilities: AttachmentCapabilityState,
    resize_lease: bool,
    terminal_id: HostTerminalId,
) -> TestResult {
    let EncodedAttachmentControl {
        kind,
        role,
        payload,
    } = encode_attachment_control(&AttachmentControl::AttachmentCapabilities(
        AttachmentCapabilities {
            attachment_id,
            capability_revision: nonzero(REVISION)?,
            capabilities,
            resize_lease,
            terminal_id,
        },
    ))?;
    host.write_all(&host_record(
        kind,
        role,
        &payload,
        scripted_epoch()?,
        correlation,
    )?)?;
    Ok(())
}

/// The raw record bytes of a correct capabilities acknowledgement, so a
/// caller can splice one envelope field before writing it.
fn capabilities_record(attach: &AttachTerminal, correlation: CorrelationId) -> TestResult<Vec<u8>> {
    let EncodedAttachmentControl {
        kind,
        role,
        payload,
    } = encode_attachment_control(&AttachmentControl::AttachmentCapabilities(
        AttachmentCapabilities {
            attachment_id: attach.attachment_id,
            capability_revision: nonzero(REVISION)?,
            capabilities: attach.capabilities,
            resize_lease: false,
            terminal_id: attach.terminal_id,
        },
    ))?;
    host_record(kind, role, &payload, scripted_epoch()?, correlation)
}

/// Transport header offsets used to splice one field out of a well-formed
/// response record: the flag word at [6..8] carries the role bits, and the
/// encoding code lives at [22].
const FLAG_OFFSET: usize = 6;
const ENCODING_OFFSET: usize = 22;
const RESPONSE_FLAG: u8 = 2;
const RAW_BYTES_CODE: u8 = 2;

/// Envelope echo violations on the attach response are protocol faults, not
/// transport loss: the stream is shut down with no further exchange and the
/// outcome reports HostProtocolFault rather than HostUnavailable.
#[test]
fn send_attach_response_envelope_violations_are_protocol_faults() -> TestResult {
    #[derive(Clone, Copy)]
    enum Corrupt {
        Correlation,
        Epoch,
        Role,
        Encoding,
        Kind,
    }
    for corruption in [
        Corrupt::Correlation,
        Corrupt::Epoch,
        Corrupt::Role,
        Corrupt::Encoding,
        Corrupt::Kind,
    ] {
        let fixture = RuntimeFixture::new()?;
        let listener = fixture.bind_host_listener()?;
        let host = thread::spawn(move || -> io::Result<()> {
            let mut host = admit_host(&listener)?;
            let (attach, correlation) = expect_attach(&mut host).map_err(protocol_io)?;
            match corruption {
                Corrupt::Correlation => {
                    let EncodedAttachmentControl {
                        kind,
                        role,
                        payload,
                    } = encode_attachment_control(&AttachmentControl::AttachmentCapabilities(
                        AttachmentCapabilities {
                            attachment_id: attach.attachment_id,
                            capability_revision: nonzero(REVISION).map_err(protocol_io)?,
                            capabilities: attach.capabilities,
                            resize_lease: false,
                            terminal_id: attach.terminal_id,
                        },
                    ))
                    .map_err(protocol_io)?;
                    let wrong = CorrelationId::from_bytes([0x99; 16]).map_err(protocol_io)?;
                    host.write_all(
                        &host_record(
                            kind,
                            role,
                            &payload,
                            scripted_epoch().map_err(protocol_io)?,
                            wrong,
                        )
                        .map_err(protocol_io)?,
                    )?;
                }
                Corrupt::Epoch => {
                    // Same acknowledgement under a different Host epoch identity.
                    let EncodedAttachmentControl {
                        kind,
                        role,
                        payload,
                    } = encode_attachment_control(&AttachmentControl::AttachmentCapabilities(
                        AttachmentCapabilities {
                            attachment_id: attach.attachment_id,
                            capability_revision: nonzero(REVISION).map_err(protocol_io)?,
                            capabilities: attach.capabilities,
                            resize_lease: false,
                            terminal_id: attach.terminal_id,
                        },
                    ))
                    .map_err(protocol_io)?;
                    host.write_all(
                        &host_record(
                            kind,
                            role,
                            &payload,
                            HostEpoch::from_bytes([0x99; 16]).map_err(protocol_io)?,
                            correlation,
                        )
                        .map_err(protocol_io)?,
                    )?;
                }
                Corrupt::Role => {
                    let mut record =
                        capabilities_record(&attach, correlation).map_err(protocol_io)?;
                    record[FLAG_OFFSET] &= !RESPONSE_FLAG;
                    host.write_all(&record)?;
                }
                Corrupt::Encoding => {
                    let mut record =
                        capabilities_record(&attach, correlation).map_err(protocol_io)?;
                    record[ENCODING_OFFSET] = RAW_BYTES_CODE;
                    host.write_all(&record)?;
                }
                Corrupt::Kind => {
                    // A different, well-formed response kind: legible on the
                    // wire but not the attach acknowledgement.
                    let EncodedAttachmentControl {
                        kind,
                        role,
                        payload,
                    } = encode_attachment_control(&AttachmentControl::DetachTerminalAccepted(
                        AttachmentTarget {
                            attachment_id: attach.attachment_id,
                        },
                    ))
                    .map_err(protocol_io)?;
                    host.write_all(
                        &host_record(
                            kind,
                            role,
                            &payload,
                            scripted_epoch().map_err(protocol_io)?,
                            correlation,
                        )
                        .map_err(protocol_io)?,
                    )?;
                }
            }
            require_connection_closed(&mut host)
        });
        let worker = RunningWorker::start(&fixture)?;
        let (mut server, _) = admit_worker(&fixture.server_listener)?;
        assert_eq!(worker.await_readiness()?, Some(*b"ready\n"));
        assert_eq!(
            send(
                &mut server,
                send_request(1, 7, identity(SCRIPT_EPOCH, TERMINAL)?, text("ls", false)?)?,
            )?
            .outcome(),
            TerminalSendOutcome::Unavailable {
                progress: SendProgress::none(),
                reason: SendUnavailable::HostProtocolFault,
                cleanup: SendCleanupStatus::Unconfirmed,
            }
        );
        require_connection_closed(&mut server)?;
        assert_failed_after_exact_readiness(worker.finish()?);
        join_peer(host, "Host echo-violation observer")?;
    }
    Ok(())
}

/// Capability echo mismatches on the attach response are the same protocol
/// desync class: the acknowledgement does not echo what was requested.
#[test]
fn send_attach_capability_echo_mismatches_are_protocol_faults() -> TestResult {
    // A granted resize lease without resize capability is already rejected by
    // the sanctioned encoder, so only constructible mismatches are covered.
    #[derive(Clone, Copy)]
    enum Mismatch {
        Capabilities,
        TerminalId,
    }
    for mismatch in [Mismatch::Capabilities, Mismatch::TerminalId] {
        let fixture = RuntimeFixture::new()?;
        let listener = fixture.bind_host_listener()?;
        let host = thread::spawn(move || -> io::Result<()> {
            let mut host = admit_host(&listener)?;
            let (attach, correlation) = expect_attach(&mut host).map_err(protocol_io)?;
            let (capabilities, resize_lease, terminal_id) = match mismatch {
                Mismatch::Capabilities => (
                    AttachmentCapabilityState {
                        read: true,
                        input: true,
                        resize: false,
                    },
                    false,
                    attach.terminal_id,
                ),
                Mismatch::TerminalId => (
                    attach.capabilities,
                    false,
                    HostTerminalId::from_bytes([0x99; 16]).map_err(protocol_io)?,
                ),
            };
            respond_capabilities_with(
                &mut host,
                correlation,
                attach.attachment_id,
                capabilities,
                resize_lease,
                terminal_id,
            )
            .map_err(protocol_io)?;
            require_connection_closed(&mut host)
        });
        let worker = RunningWorker::start(&fixture)?;
        let (mut server, _) = admit_worker(&fixture.server_listener)?;
        assert_eq!(worker.await_readiness()?, Some(*b"ready\n"));
        assert_eq!(
            send(
                &mut server,
                send_request(1, 7, identity(SCRIPT_EPOCH, TERMINAL)?, text("ls", false)?)?,
            )?
            .outcome(),
            TerminalSendOutcome::Unavailable {
                progress: SendProgress::none(),
                reason: SendUnavailable::HostProtocolFault,
                cleanup: SendCleanupStatus::Unconfirmed,
            }
        );
        require_connection_closed(&mut server)?;
        assert_failed_after_exact_readiness(worker.finish()?);
        join_peer(host, "Host capability-mismatch observer")?;
    }
    Ok(())
}

/// A silent Host burns the step budget: the exchange ends as transport loss,
/// the outcome is HostUnavailable with unconfirmed cleanup, and the session
/// ends without another Host request.
#[test]
fn send_attach_response_deadline_is_unavailable() -> TestResult {
    let fixture = RuntimeFixture::new()?;
    let listener = fixture.bind_host_listener()?;
    let host = thread::spawn(move || -> io::Result<()> {
        let mut host = admit_host(&listener)?;
        let _attach = expect_attach(&mut host).map_err(protocol_io)?;
        // No response ever arrives; the one-second exchange budget expires.
        require_connection_closed(&mut host)
    });
    let worker = RunningWorker::start(&fixture)?;
    let (mut server, _) = admit_worker(&fixture.server_listener)?;
    assert_eq!(worker.await_readiness()?, Some(*b"ready\n"));
    assert_eq!(
        send(
            &mut server,
            send_request(1, 7, identity(SCRIPT_EPOCH, TERMINAL)?, text("ls", false)?)?,
        )?
        .outcome(),
        TerminalSendOutcome::Unavailable {
            progress: SendProgress::none(),
            reason: SendUnavailable::HostUnavailable,
            cleanup: SendCleanupStatus::Unconfirmed,
        }
    );
    require_connection_closed(&mut server)?;
    assert_failed_after_exact_readiness(worker.finish()?);
    join_peer(host, "silent Host")?;
    Ok(())
}

/// An echo violation on the input acknowledgement is the same desync class as
/// a dead carrier after the input may have landed: unknown, never resent,
/// and no third request leaves on the stream.
#[test]
fn send_input_response_echo_violation_is_unknown_and_never_resent() -> TestResult {
    let fixture = RuntimeFixture::new()?;
    let listener = fixture.bind_host_listener()?;
    let host = thread::spawn(move || -> io::Result<()> {
        let mut host = admit_host(&listener)?;
        let (attach, correlation) = expect_attach(&mut host).map_err(protocol_io)?;
        respond_capabilities(&mut host, correlation, &attach).map_err(protocol_io)?;
        let (_input, _correlation) = expect_input(&mut host).map_err(protocol_io)?;
        // The acknowledgement rides the wrong correlation identity.
        let wrong = CorrelationId::from_bytes([0x99; 16]).map_err(protocol_io)?;
        respond_input_accepted(&mut host, wrong, attach.attachment_id, REVISION, 2)
            .map_err(protocol_io)?;
        require_connection_closed(&mut host)
    });
    let worker = RunningWorker::start(&fixture)?;
    let (mut server, _) = admit_worker(&fixture.server_listener)?;
    assert_eq!(worker.await_readiness()?, Some(*b"ready\n"));
    assert_eq!(
        send(
            &mut server,
            send_request(1, 7, identity(SCRIPT_EPOCH, TERMINAL)?, text("ls", false)?)?,
        )?
        .outcome(),
        TerminalSendOutcome::OutcomeUnknown {
            progress: SendProgress::none(),
            cleanup: SendCleanupStatus::Unconfirmed,
        }
    );
    require_connection_closed(&mut server)?;
    assert_failed_after_exact_readiness(worker.finish()?);
    join_peer(host, "Host input-echo observer")?;
    Ok(())
}

/// Semantic-key acknowledgements echo the Host-encoded byte count: zero is a
/// legal count (a mode may swallow the key), while a count beyond the
/// protocol bound is desync — unknown, never resent, carrier closed.
#[test]
fn send_key_acknowledgement_byte_bounds() -> TestResult {
    for (acknowledged, expected) in [
        (
            0_u32,
            TerminalSendOutcome::Accepted {
                progress: SendProgress::complete(1, 0),
                cleanup: SendCleanupStatus::Confirmed,
            },
        ),
        (
            SendAcceptedBytes::MAXIMUM + 1,
            TerminalSendOutcome::OutcomeUnknown {
                progress: SendProgress::none(),
                cleanup: SendCleanupStatus::Unconfirmed,
            },
        ),
    ] {
        let desynced = acknowledged > SendAcceptedBytes::MAXIMUM;
        let fixture = RuntimeFixture::new()?;
        let listener = fixture.bind_host_listener()?;
        let host = thread::spawn(move || -> io::Result<()> {
            let mut host = admit_host(&listener)?;
            let (attach, correlation) = expect_attach(&mut host).map_err(protocol_io)?;
            respond_capabilities(&mut host, correlation, &attach).map_err(protocol_io)?;
            let (input, correlation) = expect_input(&mut host).map_err(protocol_io)?;
            let InputMessage::KeyInput(key) = input else {
                return Err(io::Error::other("missing KeyInput"));
            };
            assert_eq!(key.key.as_str(), "Enter");
            if desynced {
                // The sanctioned encoder refuses an out-of-range count, so the
                // over-limit acknowledgement is written as a raw payload.
                let EncodedInputMessage { kind, role, .. } =
                    encode_input_message(&InputMessage::InputAccepted(InputAccepted {
                        accepted_bytes: 1,
                        attachment_id: attach.attachment_id,
                        capability_revision: nonzero(REVISION).map_err(protocol_io)?,
                    }))
                    .map_err(protocol_io)?;
                let payload = format!(
                    "{{\"acceptedBytes\":{acknowledged},\"attachmentId\":\"{}\",\"capabilityRevision\":\"{REVISION}\"}}",
                    attach.attachment_id.to_text(),
                )
                .into_bytes();
                let record = host_record(
                    kind,
                    role,
                    &payload,
                    scripted_epoch().map_err(protocol_io)?,
                    correlation,
                )
                .map_err(protocol_io)?;
                host.write_all(&record)?;
            } else {
                respond_input_accepted(
                    &mut host,
                    correlation,
                    attach.attachment_id,
                    REVISION,
                    acknowledged,
                )
                .map_err(protocol_io)?;
            }
            if desynced {
                // The desynced stream ends without a third request.
                return require_connection_closed(&mut host);
            }
            let (target, correlation) = expect_detach(&mut host).map_err(protocol_io)?;
            respond_detach(&mut host, correlation, target).map_err(protocol_io)?;
            require_connection_closed(&mut host)
        });
        let worker = RunningWorker::start(&fixture)?;
        let (mut server, _) = admit_worker(&fixture.server_listener)?;
        assert_eq!(worker.await_readiness()?, Some(*b"ready\n"));
        assert_eq!(
            send(
                &mut server,
                send_request(
                    1,
                    7,
                    identity(SCRIPT_EPOCH, TERMINAL)?,
                    one_key(SendKey::Enter)?,
                )?,
            )?
            .outcome(),
            expected
        );
        if desynced {
            require_connection_closed(&mut server)?;
        } else {
            // A clean acknowledgement keeps the carrier: the session ends
            // when the Server side drops, not through Worker shutdown.
            drop(server);
        }
        assert_failed_after_exact_readiness(worker.finish()?);
        join_peer(host, "Host key-bound observer")?;
    }
    Ok(())
}
