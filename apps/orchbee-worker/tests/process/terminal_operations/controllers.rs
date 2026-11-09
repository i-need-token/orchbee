use super::*;
use orchbee_capability_node_protocol::{
    ControlInput, TerminalControlAction, TerminalControlOutcome, TerminalControlRequest,
    TerminalViewerAction, TerminalViewerRequest,
};
use orchbee_terminal_protocol::{
    AttachmentCapabilities, AttachmentControl, InputAccepted, InputMessage,
    ProjectionDemandAccepted, ProjectionDemandControl, TypedFailure, decode_attachment_control,
    decode_input_message, decode_projection_demand_control, encode_attachment_control,
    encode_input_message, encode_projection_demand_control,
};

#[test]
fn silent_or_contradictory_host_input_result_retires_worker_without_replay() -> TestResult {
    for contradictory in [false, true] {
        let fixture = RuntimeFixture::new()?;
        let listener = fixture.bind_host_listener()?;
        let host = thread::spawn(move || -> io::Result<()> {
            let mut host = admit_host(&listener)?;
            // Viewer Attach and its demand, then a distinct control Attach and its demand.
            for expected in [20, 47, 20, 47] {
                let record = read_terminal_record(&mut host)?;
                let envelope = decode_transport_envelope(&record).map_err(protocol_io)?;
                assert_eq!(envelope.kind, expected);
                let (kind, payload) = if expected == 20 {
                    let AttachmentControl::AttachTerminal(request) =
                        decode_attachment_control(envelope.kind, envelope.role, envelope.payload)
                            .map_err(protocol_io)?
                    else {
                        return Err(io::Error::other("missing Attach"));
                    };
                    assert_eq!(request.capabilities.input, request.request_resize_lease);
                    assert_eq!(request.capabilities.resize, request.request_resize_lease);
                    assert_ne!(request.capabilities.read, request.request_resize_lease);
                    let response = encode_attachment_control(
                        &AttachmentControl::AttachmentCapabilities(AttachmentCapabilities {
                            attachment_id: request.attachment_id,
                            terminal_id: request.terminal_id,
                            capabilities: request.capabilities,
                            capability_revision: NonZeroU64::new(7)
                                .ok_or_else(|| io::Error::other("zero revision"))?,
                            resize_lease: request.request_resize_lease,
                        }),
                    )
                    .map_err(protocol_io)?;
                    (response.kind, response.payload)
                } else {
                    let ProjectionDemandControl::SetProjectionDemand(request) =
                        decode_projection_demand_control(
                            envelope.kind,
                            envelope.role,
                            envelope.payload,
                        )
                        .map_err(protocol_io)?
                    else {
                        return Err(io::Error::other("missing demand"));
                    };
                    let response = encode_projection_demand_control(
                        &ProjectionDemandControl::ProjectionDemandAccepted(
                            ProjectionDemandAccepted {
                                attachment_id: request.attachment_id,
                                demand: request.demand,
                            },
                        ),
                    )
                    .map_err(protocol_io)?;
                    (response.kind, response.payload)
                };
                let response = host_record(
                    kind,
                    MessageRole::Response,
                    &payload,
                    HostEpoch::from_bytes([0x44; 16]).map_err(protocol_io)?,
                    envelope
                        .correlation_id
                        .ok_or_else(|| io::Error::other("missing correlation"))?,
                )
                .map_err(protocol_io)?;
                host.write_all(&response)?;
            }
            let record = read_terminal_record(&mut host)?;
            let envelope = decode_transport_envelope(&record).map_err(protocol_io)?;
            assert_eq!(envelope.payload_encoding, PayloadEncoding::RawBytes);
            let InputMessage::WriteInput(input) =
                decode_input_message(envelope.kind, envelope.role, envelope.payload)
                    .map_err(protocol_io)?
            else {
                return Err(io::Error::other("missing input"));
            };
            assert_eq!(input.bytes, [0, 0x1b, 0xff]);
            assert_eq!(input.capability_revision.get(), 7);
            if contradictory {
                let response = encode_input_message(&InputMessage::InputAccepted(InputAccepted {
                    attachment_id: input.attachment_id,
                    capability_revision: input.capability_revision,
                    accepted_bytes: 0,
                }))
                .map_err(protocol_io)?;
                host.write_all(
                    &host_record(
                        response.kind,
                        response.role,
                        &response.payload,
                        HostEpoch::from_bytes([0x44; 16]).map_err(protocol_io)?,
                        envelope
                            .correlation_id
                            .ok_or_else(|| io::Error::other("missing correlation"))?,
                    )
                    .map_err(protocol_io)?,
                )?;
            }
            require_connection_closed(&mut host)
        });
        let worker = RunningWorker::start(&fixture)?;
        let (mut server, _) = admit_worker(&fixture.server_listener)?;
        assert_eq!(worker.await_readiness()?, Some(*b"ready\n"));
        let viewer = OperationId::from_bytes([11; 16])?;
        let controller = OperationId::from_bytes([22; 16])?;
        write_obcn(
            &mut server,
            &CapabilityNodeMessage::TerminalViewerRequest(TerminalViewerRequest {
                request_id: OperationId::from_bytes([1; 16])?,
                worker_id: CapabilityWorkerId::from_bytes(LOCAL_WORKER_ID.as_bytes())?,
                worker_generation: WorkerGeneration::new(
                    NonZeroU64::new(7).ok_or("zero generation")?,
                ),
                attachment_id: viewer,
                action: TerminalViewerAction::Attach {
                    host_epoch: orchbee_capability_node_protocol::HostEpoch::from_bytes(
                        [0x44; 16],
                    )?,
                    terminal_id: orchbee_capability_node_protocol::HostTerminalId::from_bytes(
                        [33; 16],
                    )?,
                },
            }),
        )?;
        assert!(
            matches!(read_obcn(&mut server)?,CapabilityNodeMessage::TerminalViewerResult(result) if result.outcome==orchbee_capability_node_protocol::TerminalViewerOutcome::Attached)
        );
        let request = TerminalControlRequest {
            request_id: OperationId::from_bytes([2; 16])?,
            worker_id: CapabilityWorkerId::from_bytes(LOCAL_WORKER_ID.as_bytes())?,
            worker_generation: WorkerGeneration::new(NonZeroU64::new(7).ok_or("zero generation")?),
            viewer_id: viewer,
            controller_id: controller,
            action: TerminalControlAction::Acquire,
        };
        write_obcn(
            &mut server,
            &CapabilityNodeMessage::TerminalControlRequest(request.clone()),
        )?;
        assert!(
            matches!(read_obcn(&mut server)?,CapabilityNodeMessage::TerminalControlResult(result) if result.outcome==TerminalControlOutcome::Acquired)
        );
        write_obcn(
            &mut server,
            &CapabilityNodeMessage::TerminalControlRequest(TerminalControlRequest {
                request_id: OperationId::from_bytes([3; 16])?,
                action: TerminalControlAction::Input {
                    sequence: NonZeroU64::MIN,
                    bytes: ControlInput::new(vec![0, 0x1b, 0xff])?,
                },
                ..request
            }),
        )?;
        require_connection_closed(&mut server)?;
        assert_failed_after_exact_readiness(worker.finish()?);
        join_peer(host, "unknown input Host")?;
    }
    Ok(())
}

#[test]
fn paste_auto_input_reaches_host_and_maps_typed_failures_without_replay() -> TestResult {
    const OPAQUE: &[u8] = b"\x00paste\x1b[200~\xff\xfepayload\x02\x03";
    let fixture = RuntimeFixture::new()?;
    let listener = fixture.bind_host_listener()?;
    let host = thread::spawn(move || -> io::Result<()> {
        let mut host = admit_host(&listener)?;
        for expected in [20, 47, 20, 47] {
            let record = read_terminal_record(&mut host)?;
            let envelope = decode_transport_envelope(&record).map_err(protocol_io)?;
            assert_eq!(envelope.kind, expected);
            let (kind, payload) = if expected == 20 {
                let AttachmentControl::AttachTerminal(request) =
                    decode_attachment_control(envelope.kind, envelope.role, envelope.payload)
                        .map_err(protocol_io)?
                else {
                    return Err(io::Error::other("missing Attach"));
                };
                let response = encode_attachment_control(
                    &AttachmentControl::AttachmentCapabilities(AttachmentCapabilities {
                        attachment_id: request.attachment_id,
                        terminal_id: request.terminal_id,
                        capabilities: request.capabilities,
                        capability_revision: NonZeroU64::new(7)
                            .ok_or_else(|| io::Error::other("zero revision"))?,
                        resize_lease: request.request_resize_lease,
                    }),
                )
                .map_err(protocol_io)?;
                (response.kind, response.payload)
            } else {
                let ProjectionDemandControl::SetProjectionDemand(request) =
                    decode_projection_demand_control(
                        envelope.kind,
                        envelope.role,
                        envelope.payload,
                    )
                    .map_err(protocol_io)?
                else {
                    return Err(io::Error::other("missing demand"));
                };
                let response = encode_projection_demand_control(
                    &ProjectionDemandControl::ProjectionDemandAccepted(ProjectionDemandAccepted {
                        attachment_id: request.attachment_id,
                        demand: request.demand,
                    }),
                )
                .map_err(protocol_io)?;
                (response.kind, response.payload)
            };
            let response = host_record(
                kind,
                MessageRole::Response,
                &payload,
                HostEpoch::from_bytes([0x44; 16]).map_err(protocol_io)?,
                envelope
                    .correlation_id
                    .ok_or_else(|| io::Error::other("missing correlation"))?,
            )
            .map_err(protocol_io)?;
            host.write_all(&response)?;
        }
        let mut pastes = 0_u8;
        loop {
            let record = read_terminal_record(&mut host)?;
            let envelope = decode_transport_envelope(&record).map_err(protocol_io)?;
            assert_eq!(envelope.payload_encoding, PayloadEncoding::RawBytes);
            let InputMessage::PasteAutoInput(paste) =
                decode_input_message(envelope.kind, envelope.role, envelope.payload)
                    .map_err(protocol_io)?
            else {
                return Err(io::Error::other("missing paste"));
            };
            assert_eq!(paste.content, OPAQUE);
            assert_eq!(paste.capability_revision.get(), 7);
            pastes += 1;
            let (kind, payload) = match pastes {
                1 => {
                    let response =
                        encode_input_message(&InputMessage::InputAccepted(InputAccepted {
                            attachment_id: paste.attachment_id,
                            capability_revision: paste.capability_revision,
                            accepted_bytes: u32::try_from(OPAQUE.len() + 12)
                                .map_err(protocol_io)?,
                        }))
                        .map_err(protocol_io)?;
                    (response.kind, response.payload)
                }
                2..=6 => {
                    let code = [
                        "invalid-field",
                        "limit-exceeded",
                        "input-backpressure",
                        "invalid-phase",
                        "unmapped-host-code",
                    ][usize::from(pastes - 2)];
                    let response =
                        encode_connection_control(&ConnectionControl::TypedFailure(TypedFailure {
                            code: code.to_string(),
                            message: format!("scripted {code}"),
                            retryable: false,
                            reason: None,
                        }))
                        .map_err(protocol_io)?;
                    (response.kind, response.payload)
                }
                _ => {
                    // A contradictory acknowledgement must retire the Worker instead of
                    // being answered or replayed.
                    let response =
                        encode_input_message(&InputMessage::InputAccepted(InputAccepted {
                            attachment_id: paste.attachment_id,
                            capability_revision: paste.capability_revision,
                            accepted_bytes: 0,
                        }))
                        .map_err(protocol_io)?;
                    (response.kind, response.payload)
                }
            };
            let response = encode_transport_envelope(&Envelope {
                correlation_id: Some(
                    envelope
                        .correlation_id
                        .ok_or_else(|| io::Error::other("missing correlation"))?,
                ),
                host_epoch: Some(HostEpoch::from_bytes([0x44; 16]).map_err(protocol_io)?),
                kind,
                optional_message: false,
                payload: &payload,
                payload_encoding: PayloadEncoding::ControlJson,
                protocol: CURRENT_PROTOCOL_VERSION,
                role: MessageRole::Response,
            })
            .map_err(protocol_io)?;
            host.write_all(&response)?;
            if pastes == 7 {
                return require_connection_closed(&mut host);
            }
        }
    });
    let worker = RunningWorker::start(&fixture)?;
    let (mut server, _) = admit_worker(&fixture.server_listener)?;
    assert_eq!(worker.await_readiness()?, Some(*b"ready\n"));
    let viewer = OperationId::from_bytes([11; 16])?;
    let controller = OperationId::from_bytes([22; 16])?;
    write_obcn(
        &mut server,
        &CapabilityNodeMessage::TerminalViewerRequest(TerminalViewerRequest {
            request_id: OperationId::from_bytes([1; 16])?,
            worker_id: CapabilityWorkerId::from_bytes(LOCAL_WORKER_ID.as_bytes())?,
            worker_generation: WorkerGeneration::new(NonZeroU64::new(7).ok_or("zero generation")?),
            attachment_id: viewer,
            action: TerminalViewerAction::Attach {
                host_epoch: orchbee_capability_node_protocol::HostEpoch::from_bytes([0x44; 16])?,
                terminal_id: orchbee_capability_node_protocol::HostTerminalId::from_bytes(
                    [33; 16],
                )?,
            },
        }),
    )?;
    assert!(
        matches!(read_obcn(&mut server).map_err(|e| format!("attach: {e}"))?,CapabilityNodeMessage::TerminalViewerResult(result) if result.outcome==orchbee_capability_node_protocol::TerminalViewerOutcome::Attached)
    );
    let request = TerminalControlRequest {
        request_id: OperationId::from_bytes([2; 16])?,
        worker_id: CapabilityWorkerId::from_bytes(LOCAL_WORKER_ID.as_bytes())?,
        worker_generation: WorkerGeneration::new(NonZeroU64::new(7).ok_or("zero generation")?),
        viewer_id: viewer,
        controller_id: controller,
        action: TerminalControlAction::Acquire,
    };
    write_obcn(
        &mut server,
        &CapabilityNodeMessage::TerminalControlRequest(request.clone()),
    )?;
    assert!(
        matches!(read_obcn(&mut server).map_err(|e| format!("acquire: {e}"))?,CapabilityNodeMessage::TerminalControlResult(result) if result.outcome==TerminalControlOutcome::Acquired)
    );
    let expected = [
        TerminalControlOutcome::Pasted {
            accepted_bytes: orchbee_capability_node_protocol::PasteAcceptedBytes::new(
                u32::try_from(OPAQUE.len() + 12)?,
            )?,
        },
        TerminalControlOutcome::Rejected(
            orchbee_capability_node_protocol::ControlRejection::HostInputInvalid,
        ),
        TerminalControlOutcome::Rejected(
            orchbee_capability_node_protocol::ControlRejection::HostInputLimitExceeded,
        ),
        TerminalControlOutcome::Rejected(
            orchbee_capability_node_protocol::ControlRejection::HostInputBackpressure,
        ),
        TerminalControlOutcome::Rejected(
            orchbee_capability_node_protocol::ControlRejection::HostInputUnavailable,
        ),
        TerminalControlOutcome::Rejected(
            orchbee_capability_node_protocol::ControlRejection::HostRejected,
        ),
    ];
    for (index, outcome) in expected.iter().enumerate() {
        write_obcn(
            &mut server,
            &CapabilityNodeMessage::TerminalControlRequest(TerminalControlRequest {
                request_id: OperationId::from_bytes([3 + index as u8; 16])?,
                action: TerminalControlAction::Paste {
                    sequence: NonZeroU64::new(index as u64 + 1).ok_or("zero sequence")?,
                    content: orchbee_capability_node_protocol::PasteContent::new(OPAQUE.to_vec())?,
                },
                ..request.clone()
            }),
        )?;
        assert!(
            matches!(read_obcn(&mut server).map_err(|e| format!("paste{index}: {e}"))?,CapabilityNodeMessage::TerminalControlResult(result) if &result.outcome==outcome)
        );
    }
    // The contradictory acknowledgement consumes its sequence and retires the Worker.
    write_obcn(
        &mut server,
        &CapabilityNodeMessage::TerminalControlRequest(TerminalControlRequest {
            request_id: OperationId::from_bytes([9; 16])?,
            action: TerminalControlAction::Paste {
                sequence: NonZeroU64::new(7).ok_or("zero sequence")?,
                content: orchbee_capability_node_protocol::PasteContent::new(OPAQUE.to_vec())?,
            },
            ..request
        }),
    )?;
    require_connection_closed(&mut server)?;
    assert_failed_after_exact_readiness(worker.finish()?);
    join_peer(host, "paste Host")?;
    Ok(())
}
