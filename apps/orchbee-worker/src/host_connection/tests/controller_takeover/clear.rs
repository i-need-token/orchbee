use super::*;
use orchbee_terminal_protocol::{
    AttachmentId, ClearTerminal, ScreenControl, ScreenControlOperation, TerminalControlAccepted,
    TerminalId, decode_screen_control, encode_screen_control,
};

#[test]
fn clear_pipe_owns_one_input_attachment_preserves_controller_and_reports_effect_frontier()
-> TestResult {
    for mode in [
        "cleared",
        "cleanup-lost",
        "effect-lost",
        "wrong-target",
        "wrong-operation",
        "zero-revision",
        "alternate",
        "attach-mismatch",
    ] {
        let (host, mut peer) = fixture()?;
        let (server, _quiet) = UnixStream::pair()?;
        thread::scope(|scope| -> TestResult {
            scope.spawn(move || {
                let record = read_transport_record(&mut peer).unwrap();
                let envelope = decode_transport_envelope(&record).unwrap();
                let AttachmentControl::AttachTerminal(attach) =
                    decode_attachment_control(envelope.kind, envelope.role, envelope.payload)
                        .unwrap()
                else {
                    panic!("clear attach")
                };
                assert_eq!(*attach.attachment_id.as_bytes(), id(21).as_bytes());
                assert!(
                    !attach.capabilities.read
                        && attach.capabilities.input
                        && !attach.capabilities.resize
                        && !attach.request_resize_lease
                );
                let granted = encode_attachment_control(
                    &AttachmentControl::AttachmentCapabilities(AttachmentCapabilities {
                        attachment_id: if mode == "attach-mismatch" {
                            AttachmentId::from_bytes([22; 16]).unwrap()
                        } else {
                            attach.attachment_id
                        },
                        terminal_id: attach.terminal_id,
                        capability_revision: NonZeroU64::MIN,
                        capabilities: attach.capabilities,
                        resize_lease: false,
                    }),
                )
                .unwrap();
                respond(
                    &mut peer,
                    &envelope,
                    granted.kind,
                    &granted.payload,
                    PayloadEncoding::ControlJson,
                );
                if mode == "attach-mismatch" {
                    return;
                }
                let record = read_transport_record(&mut peer).unwrap();
                let envelope = decode_transport_envelope(&record).unwrap();
                assert_eq!(
                    decode_screen_control(envelope.kind, envelope.role, envelope.payload).unwrap(),
                    ScreenControl::ClearTerminal(ClearTerminal {
                        attachment_id: attach.attachment_id,
                        preserve_prompt: true
                    })
                );
                if mode == "effect-lost" {
                    return;
                }
                if mode == "alternate" {
                    let denied =
                        encode_connection_control(&ConnectionControl::TypedFailure(TypedFailure {
                            code: "terminal-clear-alternate-screen".into(),
                            message: "primary required".into(),
                            retryable: false,
                            reason: None,
                        }))
                        .unwrap();
                    respond(
                        &mut peer,
                        &envelope,
                        denied.kind,
                        &denied.payload,
                        PayloadEncoding::ControlJson,
                    );
                } else {
                    let ack = encode_screen_control(ScreenControl::TerminalControlAccepted(
                        TerminalControlAccepted {
                            attachment_id: attach.attachment_id,
                            terminal_id: if mode == "wrong-target" {
                                TerminalId::from_bytes([6; 16]).unwrap()
                            } else {
                                attach.terminal_id
                            },
                            generation: NonZeroU64::new(2).unwrap(),
                            revision: if mode == "zero-revision" { 0 } else { 7 },
                            operation: if mode == "wrong-operation" {
                                ScreenControlOperation::Reset
                            } else {
                                ScreenControlOperation::Clear
                            },
                        },
                    ))
                    .unwrap();
                    respond(
                        &mut peer,
                        &envelope,
                        ack.kind,
                        &ack.payload,
                        PayloadEncoding::ControlJson,
                    );
                }
                if matches!(mode, "wrong-target" | "wrong-operation" | "zero-revision") {
                    return;
                }
                let record = read_transport_record(&mut peer).unwrap();
                let envelope = decode_transport_envelope(&record).unwrap();
                let AttachmentControl::DetachTerminal(target) =
                    decode_attachment_control(envelope.kind, envelope.role, envelope.payload)
                        .unwrap()
                else {
                    panic!("one clear detach")
                };
                assert_eq!(target.attachment_id, attach.attachment_id);
                if mode == "cleanup-lost" {
                    return;
                }
                let ack =
                    encode_attachment_control(&AttachmentControl::DetachTerminalAccepted(target))
                        .unwrap();
                respond(
                    &mut peer,
                    &envelope,
                    ack.kind,
                    &ack.payload,
                    PayloadEncoding::ControlJson,
                );
            });
            let result = execute(&host, &server, request(20, 21, Action::Clear))?;
            match mode {
                "cleared" | "cleanup-lost" => assert_eq!(
                    result,
                    Outcome::Cleared {
                        generation: NonZeroU64::new(2).unwrap(),
                        revision: NonZeroU64::new(7).unwrap(),
                        cleanup_confirmed: mode == "cleared"
                    }
                ),
                "alternate" => assert_eq!(
                    result,
                    Outcome::Rejected(ControlRejection::HostClearAlternateScreen)
                ),
                "attach-mismatch" => assert_eq!(
                    result,
                    Outcome::Rejected(ControlRejection::HostInputUnavailable)
                ),
                _ => assert_eq!(result, Outcome::ClearOutcomeUnknown),
            }
            let incumbent = host.controllers.borrow().for_viewer(id(10)).unwrap();
            assert_eq!(incumbent.controller_id, id(11));
            assert!(
                host.controllers
                    .borrow_mut()
                    .admit(id(10), id(11), NonZeroU64::MIN)
                    .is_ok()
            );
            assert!(host.controllers.borrow().for_viewer(id(20)).is_none());
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn clear_pipe_refuses_foreign_worker_generation_and_unavailable_viewer_before_host_effects()
-> TestResult {
    let (host, mut peer) = fixture()?;
    let (server, _quiet) = UnixStream::pair()?;
    for (kind, expected) in [
        (0, ControlRejection::WrongWorker),
        (1, ControlRejection::StaleWorkerGeneration),
        (2, ControlRejection::ViewerUnavailable),
        (3, ControlRejection::ViewerUnavailable),
    ] {
        let mut r = request(20, 21, Action::Clear);
        match kind {
            0 => r.worker_id = WorkerId::from_bytes([9; 16])?,
            1 => r.worker_generation = WorkerGeneration::new(NonZeroU64::new(2).unwrap()),
            2 => r.viewer_id = id(22),
            _ => r.controller_id = r.viewer_id,
        };
        assert_eq!(execute(&host, &server, r)?, Outcome::Rejected(expected));
    }
    peer.set_nonblocking(true)?;
    let mut bytes = [0];
    assert_eq!(
        peer.read(&mut bytes).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    Ok(())
}
