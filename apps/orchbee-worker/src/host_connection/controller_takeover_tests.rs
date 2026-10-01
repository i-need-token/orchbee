mod clear;
use super::*;
use orchbee_capability_node_protocol::{
    ControlInput, ControlRejection, HostTerminalId, OperationId, TerminalControlAction as Action,
    TerminalControlOutcome as Outcome, TerminalControlRequest, TerminalPixelGeometry,
    TerminalViewerAction, TerminalViewerRequest, WorkerGeneration, WorkerId,
};
use orchbee_terminal_protocol::{
    AttachmentCapabilities, AttachmentControl, DecodedEnvelope, HostEpoch, InputAccepted,
    InputMessage, ResizeAccepted, ResizeControl, TypedFailure, decode_attachment_control,
    decode_input_message, decode_resize_control, encode_attachment_control, encode_input_message,
    encode_resize_control,
};
use std::{num::NonZeroU64, thread};

type TestResult = Result<(), Box<dyn Error>>;

fn id(byte: u8) -> OperationId {
    OperationId::from_bytes([byte; 16]).unwrap()
}

fn request(viewer: u8, controller: u8, action: Action) -> TerminalControlRequest {
    TerminalControlRequest {
        request_id: id(90),
        viewer_id: id(viewer),
        controller_id: id(controller),
        worker_id: WorkerId::from_bytes([3; 16]).unwrap(),
        worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
        action,
    }
}

fn fixture() -> Result<(HostBrokerConnection, UnixStream), Box<dyn Error>> {
    let (stream, peer) = UnixStream::pair()?;
    peer.set_read_timeout(Some(Duration::from_secs(2)))?;
    peer.set_write_timeout(Some(Duration::from_secs(2)))?;
    let host = HostBrokerConnection {
        host_epoch: HostEpoch::from_bytes([4; 16])?,
        negotiated_limits: NegotiatedHostLimits {
            frame_bytes: 1_048_576,
            input_bytes: 4096,
            row_window_bytes: 4096,
        },
        stream,
        viewers: RefCell::new(HostViewers::default()),
        controllers: RefCell::new(controllers::HostControllers::default()),
        retired_targets: RefCell::new(target_cleanup::RetiredTargets::default()),
        titles: RefCell::new(titles::Titles::default()),
        drain: Rc::new(RefCell::new(AgentDrain::new())),
    };
    for viewer in [10, 20] {
        host.viewers
            .borrow_mut()
            .admit(&TerminalViewerRequest {
                request_id: id(91),
                worker_id: WorkerId::from_bytes([3; 16])?,
                worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
                attachment_id: id(viewer),
                action: TerminalViewerAction::Attach {
                    host_epoch: orchbee_capability_node_protocol::HostEpoch::from_bytes([4; 16])?,
                    terminal_id: HostTerminalId::from_bytes([5; 16])?,
                },
            })
            .map_err(|reason| format!("viewer: {reason:?}"))?;
    }
    host.controllers
        .borrow_mut()
        .reserve(id(10), id(11), HostTerminalId::from_bytes([5; 16])?)
        .map_err(|reason| format!("controller: {reason:?}"))?;
    host.controllers
        .borrow_mut()
        .commit(id(11), NonZeroU64::MIN);
    Ok((host, peer))
}

fn execute(
    host: &HostBrokerConnection,
    server: &UnixStream,
    request: TerminalControlRequest,
) -> Result<Outcome, Box<dyn Error>> {
    crate::terminal_control_operation::execute(
        request,
        orchbee_local_site_protocol::WorkerId::from_bytes([3; 16]),
        NonZeroU64::MIN,
        host,
        server,
    )
    .map(|result| result.outcome)
}

fn respond(
    peer: &mut UnixStream,
    request: &DecodedEnvelope<'_>,
    kind: u16,
    payload: &[u8],
    encoding: PayloadEncoding,
) {
    peer.write_all(
        &encode_transport_envelope(&Envelope {
            correlation_id: request.correlation_id,
            host_epoch: request.host_epoch,
            kind,
            optional_message: false,
            payload,
            payload_encoding: encoding,
            protocol: CURRENT_PROTOCOL_VERSION,
            role: MessageRole::Response,
        })
        .unwrap(),
    )
    .unwrap();
}

#[test]
fn takeover_pipe_detaches_before_new_grant_and_rejects_old_input_resize_and_cleanup() -> TestResult
{
    let (host, mut peer) = fixture()?;
    let (server, _quiet) = UnixStream::pair()?;
    thread::scope(|scope| -> TestResult {
        scope.spawn(move || {
            for step in 0..5 {
                let record = read_transport_record(&mut peer).unwrap();
                let envelope = decode_transport_envelope(&record).unwrap();
                if step == 0 || step == 4 {
                    let AttachmentControl::DetachTerminal(target) =
                        decode_attachment_control(envelope.kind, envelope.role, envelope.payload)
                            .unwrap()
                    else {
                        panic!("expected exact detach")
                    };
                    assert_eq!(
                        *target.attachment_id.as_bytes(),
                        id(if step == 0 { 11 } else { 21 }).as_bytes()
                    );
                    let response = encode_attachment_control(
                        &AttachmentControl::DetachTerminalAccepted(target),
                    )
                    .unwrap();
                    respond(
                        &mut peer,
                        &envelope,
                        response.kind,
                        &response.payload,
                        PayloadEncoding::ControlJson,
                    );
                } else if step == 1 {
                    let AttachmentControl::AttachTerminal(attach) =
                        decode_attachment_control(envelope.kind, envelope.role, envelope.payload)
                            .unwrap()
                    else {
                        panic!("expected new attach")
                    };
                    assert_eq!(*attach.attachment_id.as_bytes(), id(21).as_bytes());
                    assert_eq!(*attach.terminal_id.as_bytes(), [5; 16]);
                    assert!(
                        !attach.capabilities.read
                            && attach.capabilities.input
                            && attach.capabilities.resize
                            && attach.request_resize_lease
                    );
                    let response = encode_attachment_control(
                        &AttachmentControl::AttachmentCapabilities(AttachmentCapabilities {
                            attachment_id: attach.attachment_id,
                            terminal_id: attach.terminal_id,
                            capabilities: attach.capabilities,
                            capability_revision: NonZeroU64::new(8).unwrap(),
                            resize_lease: true,
                        }),
                    )
                    .unwrap();
                    respond(
                        &mut peer,
                        &envelope,
                        response.kind,
                        &response.payload,
                        PayloadEncoding::ControlJson,
                    );
                } else if step == 2 {
                    let InputMessage::WriteInput(input) =
                        decode_input_message(envelope.kind, envelope.role, envelope.payload)
                            .unwrap()
                    else {
                        panic!("expected new input")
                    };
                    assert_eq!(*input.attachment_id.as_bytes(), id(21).as_bytes());
                    assert_eq!(input.capability_revision.get(), 8);
                    assert_eq!(input.bytes, b"new");
                    let response =
                        encode_input_message(&InputMessage::InputAccepted(InputAccepted {
                            attachment_id: input.attachment_id,
                            capability_revision: input.capability_revision,
                            accepted_bytes: 3,
                        }))
                        .unwrap();
                    respond(
                        &mut peer,
                        &envelope,
                        response.kind,
                        &response.payload,
                        PayloadEncoding::ControlJson,
                    );
                } else {
                    let ResizeControl::ResizeTerminal(resize) =
                        decode_resize_control(envelope.kind, envelope.role, envelope.payload)
                            .unwrap()
                    else {
                        panic!("expected new resize")
                    };
                    assert_eq!(*resize.attachment_id.as_bytes(), id(21).as_bytes());
                    let response =
                        encode_resize_control(&ResizeControl::ResizeAccepted(ResizeAccepted {
                            attachment_id: resize.attachment_id,
                            geometry: resize.geometry,
                            revision: 1,
                        }))
                        .unwrap();
                    respond(
                        &mut peer,
                        &envelope,
                        response.kind,
                        &response.payload,
                        PayloadEncoding::ControlJson,
                    );
                }
            }
        });
        assert_eq!(
            execute(
                &host,
                &server,
                request(
                    20,
                    21,
                    Action::Takeover {
                        previous_controller_id: id(11)
                    }
                )
            )?,
            Outcome::Acquired
        );
        let input = Action::Input {
            sequence: NonZeroU64::MIN,
            bytes: ControlInput::new(b"new".to_vec())?,
        };
        let geometry = TerminalPixelGeometry::new(110, 32, 0, 0)?;
        let resize = Action::Resize {
            sequence: NonZeroU64::new(2).unwrap(),
            geometry,
        };
        for action in [input.clone(), resize.clone()] {
            assert_eq!(
                execute(&host, &server, request(10, 11, action))?,
                Outcome::Rejected(ControlRejection::NotController)
            );
        }
        assert_eq!(
            execute(&host, &server, request(10, 11, Action::Release))?,
            Outcome::Released
        );
        assert_eq!(
            execute(&host, &server, request(20, 21, input))?,
            Outcome::InputAccepted
        );
        assert_eq!(
            execute(&host, &server, request(20, 21, resize))?,
            Outcome::Resized(geometry)
        );
        assert_eq!(
            execute(&host, &server, request(20, 21, Action::Release))?,
            Outcome::Released
        );
        assert!(host.controllers.borrow().for_viewer(id(20)).is_none());
        assert!(host.viewers.borrow().target(id(10)).is_some());
        assert!(host.viewers.borrow().target(id(20)).is_some());
        Ok(())
    })
}

#[test]
fn takeover_pipe_failure_never_restores_old_authority_or_grants_without_detach_proof() -> TestResult
{
    for fail_detach in [true, false] {
        let (host, mut peer) = fixture()?;
        let (server, _quiet) = UnixStream::pair()?;
        thread::scope(|scope| -> TestResult {
            scope.spawn(move || {
                let record = read_transport_record(&mut peer).unwrap();
                let envelope = decode_transport_envelope(&record).unwrap();
                let AttachmentControl::DetachTerminal(target) =
                    decode_attachment_control(envelope.kind, envelope.role, envelope.payload)
                        .unwrap()
                else {
                    panic!("expected old detach")
                };
                if fail_detach {
                    return;
                } // EOF without proof: no new Attach may be sent.
                let response =
                    encode_attachment_control(&AttachmentControl::DetachTerminalAccepted(target))
                        .unwrap();
                respond(
                    &mut peer,
                    &envelope,
                    response.kind,
                    &response.payload,
                    PayloadEncoding::ControlJson,
                );
                let record = read_transport_record(&mut peer).unwrap();
                let envelope = decode_transport_envelope(&record).unwrap();
                assert!(matches!(
                    decode_attachment_control(envelope.kind, envelope.role, envelope.payload)
                        .unwrap(),
                    AttachmentControl::AttachTerminal(_)
                ));
                let response =
                    encode_connection_control(&ConnectionControl::TypedFailure(TypedFailure {
                        code: "limit-exceeded".into(),
                        message: "test capacity".into(),
                        retryable: false,
                    }))
                    .unwrap();
                respond(
                    &mut peer,
                    &envelope,
                    response.kind,
                    &response.payload,
                    PayloadEncoding::ControlJson,
                );
            });
            let result = execute(
                &host,
                &server,
                request(
                    20,
                    21,
                    Action::Takeover {
                        previous_controller_id: id(11),
                    },
                ),
            );
            if fail_detach {
                assert!(result.is_err());
            } else {
                assert_eq!(result?, Outcome::Rejected(ControlRejection::HostRejected));
                assert!(host.controllers.borrow().for_viewer(id(10)).is_none());
            }
            assert!(host.controllers.borrow().for_viewer(id(20)).is_none());
            Ok(())
        })?;
    }
    Ok(())
}
