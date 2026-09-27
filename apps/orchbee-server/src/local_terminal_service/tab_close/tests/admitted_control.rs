use super::*;
use crate::worker_obcn_carrier::{WorkerOperationRequest, WorkerOperationResult};
use orchbee_capability_node_protocol::{
    LifecycleInventoryFailure, OperationId, TerminalControlAction as WorkerControlAction,
    TerminalControlOutcome as WorkerControlOutcome, TerminalControlResult as WorkerControlResult,
    TerminalViewerAction as WorkerViewerAction, TerminalViewerOutcome as WorkerViewerOutcome,
    TerminalViewerRequest as WorkerViewerRequest, TerminalViewerResult as WorkerViewerResult,
};
use orchbee_client_server_protocol::{
    ClientMessage, ControlInput, ControlRejectedReason, Geometry, RequestId as ClientRequestId,
    ServerMessage, TerminalControlAction as ControlAction,
    TerminalControlOutcome as ControlOutcome, TerminalControlRequest,
    TerminalViewerAction as ViewerAction, TerminalViewerOutcome as ViewerOutcome,
    TerminalViewerRequest,
};

fn client_id(value: u128) -> ClientRequestId {
    ClientRequestId::parse(&format!("{value:032x}")).unwrap()
}

fn execute(
    session: &mut crate::local_terminal_service::viewer_session::LocalTerminalSession<'_>,
    value: u128,
    action: ControlAction,
) -> ControlOutcome {
    let ServerMessage::TerminalControlResult(result) = session.execute(
        ClientMessage::TerminalControl(TerminalControlRequest::new(client_id(value), action)),
    ) else {
        panic!("control result")
    };
    result.into_outcome()
}

#[test]
fn preadmitted_input_and_resize_keep_success_while_new_effects_are_fenced_without_consumption() {
    for resize in [false, true] {
        let Fixture {
            service,
            mut peer,
            worker_thread,
            close,
        } = fixture();
        let (parked_send, parked) = mpsc::sync_channel(1);
        let (release, released) = mpsc::sync_channel(1);
        let marker_id = OperationId::from_bytes([250; 16]).unwrap();
        let peer_thread = thread::spawn(move || {
            let mut events = Vec::new();
            loop {
                match read_message(&mut peer).unwrap() {
                    CapabilityNodeMessage::TerminalViewerRequest(request) => {
                        let marker = request.request_id == marker_id;
                        if !marker {
                            assert!(matches!(request.action, WorkerViewerAction::Attach { .. }));
                            events.push("attach");
                        }
                        write_message(
                            &mut peer,
                            &CapabilityNodeMessage::TerminalViewerResult(WorkerViewerResult {
                                request_id: request.request_id,
                                attachment_id: request.attachment_id,
                                outcome: if marker {
                                    WorkerViewerOutcome::Empty
                                } else {
                                    WorkerViewerOutcome::Attached
                                },
                            }),
                        )
                        .unwrap();
                        if marker {
                            return events;
                        }
                    }
                    CapabilityNodeMessage::TerminalControlRequest(request) => {
                        let outcome = match request.action {
                            WorkerControlAction::Takeover { .. } => {
                                panic!("unexpected takeover in close fixture")
                            }
                            WorkerControlAction::Acquire => {
                                events.push("acquire");
                                WorkerControlOutcome::Acquired
                            }
                            WorkerControlAction::Input { sequence, .. } => {
                                assert!(!resize);
                                assert_eq!(sequence, NonZeroU64::MIN);
                                events.push("input");
                                parked_send.send(request.viewer_id).unwrap();
                                released.recv_timeout(Duration::from_secs(3)).unwrap();
                                WorkerControlOutcome::InputAccepted
                            }
                            WorkerControlAction::Resize { sequence, geometry } => {
                                assert!(resize);
                                assert_eq!(sequence, NonZeroU64::MIN);
                                events.push("resize");
                                parked_send.send(request.viewer_id).unwrap();
                                released.recv_timeout(Duration::from_secs(3)).unwrap();
                                WorkerControlOutcome::Resized(geometry)
                            }
                            WorkerControlAction::Paste { sequence, .. } => {
                                assert_eq!(sequence, NonZeroU64::MIN);
                                events.push("paste");
                                parked_send.send(request.viewer_id).unwrap();
                                released.recv_timeout(Duration::from_secs(3)).unwrap();
                                WorkerControlOutcome::Pasted {
                                    accepted_bytes:
                                        orchbee_capability_node_protocol::PasteAcceptedBytes::new(1)
                                            .expect("bounded"),
                                }
                            }
                            WorkerControlAction::Release => {
                                panic!("session must remain owned until the marker")
                            }
                        };
                        write_message(
                            &mut peer,
                            &CapabilityNodeMessage::TerminalControlResult(WorkerControlResult {
                                request_id: request.request_id,
                                viewer_id: request.viewer_id,
                                controller_id: request.controller_id,
                                outcome,
                            }),
                        )
                        .unwrap();
                    }
                    CapabilityNodeMessage::TerminalLifecycleRequest(request) => {
                        assert!(matches!(
                            request.action,
                            TerminalLifecycleAction::InventoryPage { .. }
                        ));
                        events.push("inventory");
                        reply(
                            &mut peer,
                            request,
                            TerminalLifecycleOutcome::InventoryFailure(
                                LifecycleInventoryFailure::PageRejected,
                            ),
                        );
                    }
                    _ => panic!("unexpected carrier message"),
                }
            }
        });
        let (known_send, known) = mpsc::sync_channel(1);
        let (check, checks) = mpsc::sync_channel(1);
        let (result_send, results) = mpsc::sync_channel(1);
        let (finish, finished) = mpsc::sync_channel(1);
        let actor = Arc::clone(&service);
        let client = thread::spawn(move || {
            let mut session = actor.session();
            let ServerMessage::TerminalViewerResult(attached) =
                session.execute(ClientMessage::TerminalViewer(TerminalViewerRequest::new(
                    client_id(100),
                    ViewerAction::Attach {
                        terminal_create_request_id: client_id(11),
                    },
                )))
            else {
                panic!("viewer result")
            };
            let ViewerOutcome::Attached { attachment_id, .. } = attached.into_outcome() else {
                panic!("attachment")
            };
            let ControlOutcome::Acquired { controller_id } = execute(
                &mut session,
                101,
                ControlAction::Acquire {
                    attachment_id: attachment_id.clone(),
                },
            ) else {
                panic!("controller")
            };
            let geometry = Geometry::new(100, 30, 8, 16).unwrap();
            let action = if resize {
                ControlAction::Resize {
                    attachment_id: attachment_id.clone(),
                    controller_id: controller_id.clone(),
                    sequence: NonZeroU64::MIN,
                    geometry,
                }
            } else {
                ControlAction::Input {
                    attachment_id: attachment_id.clone(),
                    controller_id: controller_id.clone(),
                    sequence: NonZeroU64::MIN,
                    bytes: ControlInput::new(b"preadmitted".to_vec()).unwrap(),
                }
            };
            known_send.send(execute(&mut session, 102, action)).unwrap();
            checks.recv_timeout(Duration::from_secs(3)).unwrap();
            let mut outcomes = Vec::new();
            for repeat in 0..2 {
                outcomes.push(execute(
                    &mut session,
                    103 + repeat * 2,
                    ControlAction::Input {
                        attachment_id: attachment_id.clone(),
                        controller_id: controller_id.clone(),
                        sequence: NonZeroU64::new(2).unwrap(),
                        bytes: ControlInput::new(b"fenced".to_vec()).unwrap(),
                    },
                ));
                outcomes.push(execute(
                    &mut session,
                    104 + repeat * 2,
                    ControlAction::Resize {
                        attachment_id: attachment_id.clone(),
                        controller_id: controller_id.clone(),
                        sequence: NonZeroU64::new(2).unwrap(),
                        geometry,
                    },
                ));
            }
            result_send.send(outcomes).unwrap();
            finished.recv_timeout(Duration::from_secs(3)).unwrap();
            drop(session);
        });
        let attachment = parked.recv_timeout(Duration::from_secs(3)).unwrap();
        service.start_close_maintenance().unwrap();
        assert!(matches!(
            service
                .execute_terminal_tab_close(close.clone())
                .unwrap()
                .outcome,
            WorkbenchCommandOutcome::TabCloseAccepted { .. }
        ));
        release.send(()).unwrap();
        assert_eq!(
            known.recv_timeout(Duration::from_secs(3)).unwrap(),
            if resize {
                ControlOutcome::Resized {
                    geometry: Geometry::new(100, 30, 8, 16).unwrap(),
                }
            } else {
                ControlOutcome::InputAccepted {}
            }
        );
        let deadline = Instant::now() + Duration::from_secs(3);
        while !matches!(
            read(&service, &close),
            WorkbenchTabCloseReadOutcome::Available {
                status: WorkbenchTabCloseStatus::CleanupBlocked { .. },
                ..
            }
        ) {
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(5));
        }
        let cursor = service
            .coordinator
            .lock()
            .unwrap()
            .allocate_identity()
            .unwrap();
        check.send(()).unwrap();
        assert_eq!(
            results.recv_timeout(Duration::from_secs(3)).unwrap(),
            vec![
                ControlOutcome::Rejected {
                    reason: ControlRejectedReason::ViewerUnavailable
                };
                4
            ]
        );
        assert_eq!(
            service.coordinator.lock().unwrap().allocate_identity(),
            Some((u128::from_be_bytes(cursor) + 1).to_be_bytes()),
            "fenced effects must allocate no identity"
        );
        let handle = service.workers.lock().unwrap().request_handle().unwrap();
        assert!(
            matches!(handle.blocking_admit_for_test(WorkerOperationRequest::Viewer(WorkerViewerRequest {
            request_id: marker_id, attachment_id: attachment, worker_id: orchbee_capability_node_protocol::WorkerId::from_bytes([9; 16]).unwrap(), worker_generation: WorkerGeneration::new(NonZeroU64::MIN), action: WorkerViewerAction::Poll,
        })), Ok(WorkerOperationResult::Viewer(result)) if result.outcome == WorkerViewerOutcome::Empty)
        );
        let events = peer_thread.join().unwrap();
        assert_eq!(
            events,
            vec![
                "attach",
                "acquire",
                if resize { "resize" } else { "input" },
                "inventory"
            ]
        );
        service.stop_close_maintenance();
        let _exit = worker_thread.join().unwrap();
        finish.send(()).unwrap();
        client.join().unwrap();
    }
}
