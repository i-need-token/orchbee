use super::*;
use crate::worker_obcn_carrier::{WorkerOperationRequest, WorkerOperationResult};
use orchbee_capability_node_protocol::{
    OperationId, TerminalControlAction as WorkerControlAction,
    TerminalControlOutcome as WorkerControlOutcome, TerminalControlResult as WorkerControlResult,
    TerminalViewerAction as WorkerViewerAction, TerminalViewerOutcome as WorkerViewerOutcome,
    TerminalViewerRequest as WorkerViewerRequest, TerminalViewerResult as WorkerViewerResult,
};
use orchbee_client_server_protocol::{
    ClientMessage, ControlInput, RequestId as ClientRequestId, ServerMessage,
    TerminalControlAction as ControlAction, TerminalControlOutcome as ControlOutcome,
    TerminalControlRequest, TerminalViewerAction as ViewerAction,
    TerminalViewerOutcome as ViewerOutcome, TerminalViewerRequest,
};

fn client_id(value: u128) -> ClientRequestId {
    ClientRequestId::parse(&format!("{value:032x}")).unwrap()
}

#[test]
fn maintenance_frees_one_of_eight_actual_grants_before_any_old_session_action() {
    let Fixture {
        service,
        mut peer,
        worker_thread,
        mut close,
    } = fixture();
    for index in 1..8 {
        add_terminal(&service, index);
    }
    close.expected_revision = read_snapshot(&service).revision;
    let marker_id = OperationId::from_bytes([250; 16]).unwrap();
    let peer_thread = thread::spawn(move || {
        let mut events = Vec::new();
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
                    assert!(
                        matches!(request.action, WorkerViewerAction::Attach { .. }),
                        "old idle Viewer sent cleanup: {request:?}"
                    );
                    events.push("attach");
                    write_message(
                        &mut peer,
                        &CapabilityNodeMessage::TerminalViewerResult(WorkerViewerResult {
                            request_id: request.request_id,
                            attachment_id: request.attachment_id,
                            outcome: WorkerViewerOutcome::Attached,
                        }),
                    )
                    .unwrap();
                }
                CapabilityNodeMessage::TerminalControlRequest(request) => {
                    let outcome = match request.action {
                        WorkerControlAction::Acquire => {
                            events.push("acquire");
                            WorkerControlOutcome::Acquired
                        }
                        WorkerControlAction::Input { .. } => {
                            events.push("input");
                            WorkerControlOutcome::InputAccepted
                        }
                        _ => panic!("old idle controller sent cleanup: {request:?}"),
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
                    let outcome = match request.action {
                        TerminalLifecycleAction::InventoryPage { .. } => {
                            events.push("inventory");
                            settled_page()
                        }
                        TerminalLifecycleAction::Dispose => {
                            events.push("dispose");
                            TerminalLifecycleOutcome::Disposed
                        }
                        _ => panic!("settled fixture must not signal"),
                    };
                    reply(&mut peer, request, outcome);
                }
                _ => panic!("unexpected carrier message"),
            }
        }
    });
    let mut sessions = Vec::new();
    let mut grants = Vec::new();
    let mut original = None;
    for index in 0..9_u8 {
        if index == 8 {
            let before = service.controllers.lock().unwrap().clone();
            assert_eq!(
                before.len(),
                8,
                "eight actual Acquire replies must own eight slots"
            );
            original = Some(before.clone());
            service.start_close_maintenance().unwrap();
            assert!(matches!(
                service
                    .execute_terminal_tab_close(close.clone())
                    .unwrap()
                    .outcome,
                WorkbenchCommandOutcome::TabCloseAccepted { .. }
            ));
            let deadline = Instant::now() + Duration::from_secs(3);
            while !matches!(
                read(&service, &close),
                WorkbenchTabCloseReadOutcome::Available {
                    status: WorkbenchTabCloseStatus::Closed { .. },
                    ..
                }
            ) {
                assert!(Instant::now() < deadline, "idle Close did not publish");
                thread::sleep(Duration::from_millis(5));
            }
            let mut expected = before;
            expected.retain(|target, _| target.terminal != [5; 16]);
            assert_eq!(expected.len(), 7);
            assert_eq!(
                *service.controllers.lock().unwrap(),
                expected,
                "maintenance must free exactly one slot before any session action or Drop"
            );
            assert_eq!(sessions.len(), 8);
            add_terminal(&service, 8);
        }
        let mut session = service.session();
        let attached = session.execute(ClientMessage::TerminalViewer(TerminalViewerRequest::new(
            client_id(2000 + u128::from(index)),
            ViewerAction::Attach {
                terminal_create_request_id: client_id(11 + u128::from(index) * 100),
            },
        )));
        assert!(
            matches!(attached, ServerMessage::TerminalViewerResult(result) if matches!(result.outcome(), ViewerOutcome::Attached { .. }))
        );
        let acquired =
            session.execute(ClientMessage::TerminalControl(TerminalControlRequest::new(
                client_id(2100 + u128::from(index)),
                ControlAction::Acquire {},
            )));
        let ServerMessage::TerminalControlResult(acquired) = acquired else {
            panic!("control result")
        };
        let ControlOutcome::Acquired { controller_id } = acquired.into_outcome() else {
            panic!("fresh slot must admit controller")
        };
        grants.push(controller_id);
        sessions.push(session);
    }
    let owners = service.controllers.lock().unwrap().clone();
    assert_eq!(owners.len(), 8);
    for (target, controller) in original.unwrap() {
        if target.terminal != [5; 16] {
            assert_eq!(owners.get(&target), Some(&controller));
        }
    }
    assert!(
        matches!(sessions[1].execute(ClientMessage::TerminalControl(TerminalControlRequest::new(client_id(2200), ControlAction::Input {
        controller_id: grants[1].clone(), sequence: NonZeroU64::MIN, bytes: ControlInput::new(b"unrelated still executes".to_vec()).unwrap(),
    }))), ServerMessage::TerminalControlResult(result) if result.outcome() == &ControlOutcome::InputAccepted {})
    );
    let handle = service.workers.lock().unwrap().request_handle().unwrap();
    let marker = handle.request(WorkerOperationRequest::Viewer(WorkerViewerRequest {
        request_id: marker_id,
        attachment_id: OperationId::from_bytes([249; 16]).unwrap(),
        worker_id: orchbee_capability_node_protocol::WorkerId::from_bytes([9; 16]).unwrap(),
        worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
        action: WorkerViewerAction::Poll,
    }));
    assert!(
        matches!(marker, Ok(WorkerOperationResult::Viewer(result)) if result.outcome == WorkerViewerOutcome::Empty)
    );
    let events = peer_thread.join().unwrap();
    assert_eq!(events.iter().filter(|event| **event == "attach").count(), 9);
    assert_eq!(
        events.iter().filter(|event| **event == "acquire").count(),
        9
    );
    assert_eq!(
        events.iter().filter(|event| **event == "dispose").count(),
        1
    );
    assert_eq!(events.iter().filter(|event| **event == "input").count(), 1);
    service.stop_close_maintenance();
    let _exit = worker_thread.join().unwrap();
    drop(sessions);
}
