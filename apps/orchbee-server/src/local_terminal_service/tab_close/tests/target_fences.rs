use super::*;
use crate::worker_obcn_carrier::{WorkerOperationRequest, WorkerOperationResult};
use orchbee_capability_node_protocol::{
    LifecycleInventoryFailure, OperationId, TerminalControlAction as WorkerControlAction,
    TerminalControlOutcome as WorkerControlOutcome, TerminalControlResult as WorkerControlResult,
    TerminalViewerAction as WorkerViewerAction, TerminalViewerOutcome as WorkerViewerOutcome,
    TerminalViewerRequest as WorkerViewerRequest, TerminalViewerResult as WorkerViewerResult,
};
use orchbee_client_server_protocol::{
    ClientMessage, ControlInput, ControlRejectedReason, ControlUnavailableReason, ControllerId,
    RequestId as ClientRequestId, ServerMessage, TerminalControlAction as ControlAction,
    TerminalControlOutcome as ControlOutcome, TerminalControlRequest,
    TerminalViewerAction as ViewerAction, TerminalViewerOutcome as ViewerOutcome,
    TerminalViewerRequest, ViewerRejectedReason, ViewerUnavailableReason,
};

fn viewer_request(value: u128, action: ViewerAction) -> ClientMessage {
    ClientMessage::TerminalViewer(TerminalViewerRequest::new(client_id(value), action))
}

fn control_request(value: u128, action: ControlAction) -> ClientMessage {
    ClientMessage::TerminalControl(TerminalControlRequest::new(client_id(value), action))
}

fn client_id(value: u128) -> ClientRequestId {
    ClientRequestId::parse(&format!("{value:032x}")).unwrap()
}

#[test]
fn parked_attach_and_acquire_success_cannot_install_after_close_acceptance() {
    for acquire in [false, true] {
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
            let mut parked_send = Some(parked_send);
            loop {
                match read_message(&mut peer).unwrap() {
                    CapabilityNodeMessage::TerminalViewerRequest(request) => {
                        if request.request_id == marker_id {
                            write_message(
                                &mut peer,
                                &CapabilityNodeMessage::TerminalViewerResult(WorkerViewerResult {
                                    request_id: request.request_id,
                                    attachment_id: request.attachment_id,
                                    outcome: WorkerViewerOutcome::Empty,
                                }),
                            )
                            .unwrap();
                            return events;
                        }
                        let outcome = match request.action {
                            WorkerViewerAction::Attach { .. } => {
                                events.push("attach");
                                if !acquire {
                                    parked_send
                                        .take()
                                        .unwrap()
                                        .send(request.attachment_id)
                                        .unwrap();
                                    released.recv_timeout(Duration::from_secs(3)).unwrap();
                                }
                                WorkerViewerOutcome::Attached
                            }
                            WorkerViewerAction::Detach => {
                                events.push("detach");
                                WorkerViewerOutcome::Detached
                            }
                            _ => panic!("unexpected Viewer effect"),
                        };
                        write_message(
                            &mut peer,
                            &CapabilityNodeMessage::TerminalViewerResult(WorkerViewerResult {
                                request_id: request.request_id,
                                attachment_id: request.attachment_id,
                                outcome,
                            }),
                        )
                        .unwrap();
                    }
                    CapabilityNodeMessage::TerminalControlRequest(request) => {
                        let outcome = match request.action {
                            WorkerControlAction::Acquire => {
                                events.push("acquire");
                                parked_send.take().unwrap().send(request.viewer_id).unwrap();
                                released.recv_timeout(Duration::from_secs(3)).unwrap();
                                WorkerControlOutcome::Acquired
                            }
                            WorkerControlAction::Release => {
                                events.push("release");
                                WorkerControlOutcome::Released
                            }
                            _ => panic!("fenced controller dispatched an effect"),
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
        let actor = Arc::clone(&service);
        let client = thread::spawn(move || {
            let mut session = actor.session();
            let attached = session.execute(viewer_request(
                100,
                ViewerAction::Attach {
                    terminal_create_request_id: client_id(11),
                },
            ));
            let result = if acquire {
                assert!(
                    matches!(attached, ServerMessage::TerminalViewerResult(result) if matches!(result.outcome(), ViewerOutcome::Attached { .. }))
                );
                session.execute(control_request(101, ControlAction::Acquire {}))
            } else {
                attached
            };
            let after = if acquire {
                session.execute(control_request(
                    102,
                    ControlAction::Input {
                        controller_id: ControllerId::parse(&format!("{:032x}", 1)).unwrap(),
                        sequence: NonZeroU64::MIN,
                        bytes: ControlInput::new(b"must not dispatch".to_vec()).unwrap(),
                    },
                ))
            } else {
                session.execute(viewer_request(103, ViewerAction::Poll {}))
            };
            let acquire_after_close =
                acquire.then(|| session.execute(control_request(104, ControlAction::Acquire {})));
            drop(session);
            (result, after, acquire_after_close)
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
        let (result, after, acquire_after_close) = client.join().unwrap();
        if acquire {
            assert!(
                matches!(acquire_after_close, Some(ServerMessage::TerminalControlResult(result)) if result.outcome() == &ControlOutcome::Rejected { reason: ControlRejectedReason::ViewerUnavailable })
            );
            assert!(
                matches!(result, ServerMessage::TerminalControlResult(result) if result.outcome() == &ControlOutcome::Unavailable { reason: ControlUnavailableReason::TerminalRuntimeUnavailable })
            );
            assert!(
                matches!(after, ServerMessage::TerminalControlResult(result) if result.outcome() == &ControlOutcome::Rejected { reason: ControlRejectedReason::NotController })
            );
        } else {
            assert!(
                matches!(result, ServerMessage::TerminalViewerResult(result) if result.outcome() == &ViewerOutcome::Unavailable { reason: ViewerUnavailableReason::TerminalRuntimeUnavailable })
            );
            assert!(
                matches!(after, ServerMessage::TerminalViewerResult(result) if result.outcome() == &ViewerOutcome::Rejected { reason: ViewerRejectedReason::TerminalUnavailable })
            );
        }
        assert!(service.controllers.lock().unwrap().is_empty());
        let deadline = Instant::now() + Duration::from_secs(3);
        while !matches!(
            read(&service, &close),
            WorkbenchTabCloseReadOutcome::Available {
                status: WorkbenchTabCloseStatus::CleanupBlocked { .. },
                ..
            }
        ) {
            assert!(
                Instant::now() < deadline,
                "inventory did not settle before the marker"
            );
            thread::sleep(Duration::from_millis(5));
        }
        let handle = service.workers.lock().unwrap().request_handle().unwrap();
        let marker = handle.request(WorkerOperationRequest::Viewer(WorkerViewerRequest {
            request_id: marker_id,
            attachment_id: attachment,
            worker_id: orchbee_capability_node_protocol::WorkerId::from_bytes([9; 16]).unwrap(),
            worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
            action: WorkerViewerAction::Poll,
        }));
        assert!(
            matches!(&marker, Ok(WorkerOperationResult::Viewer(result)) if result.outcome == WorkerViewerOutcome::Empty),
            "marker: {marker:?}"
        );
        let events = peer_thread.join().unwrap();
        assert_eq!(events.iter().filter(|event| **event == "attach").count(), 1);
        assert_eq!(events.iter().filter(|event| **event == "detach").count(), 1);
        assert_eq!(
            events.iter().filter(|event| **event == "acquire").count(),
            usize::from(acquire)
        );
        assert_eq!(
            events.iter().filter(|event| **event == "release").count(),
            usize::from(acquire)
        );
        service.stop_close_maintenance();
        let _exit = worker_thread.join().unwrap();
    }
}
