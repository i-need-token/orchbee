use super::*;
use crate::worker_obcn_carrier::{WorkerOperationRequest, WorkerOperationResult};
use orchbee_capability_node_protocol::{
    OperationId, TerminalControlAction as WorkerControlAction,
    TerminalControlOutcome as WorkerControlOutcome, TerminalControlResult as WorkerControlResult,
    TerminalViewerAction as WorkerViewerAction, TerminalViewerOutcome as WorkerViewerOutcome,
    TerminalViewerRequest as WorkerViewerRequest, TerminalViewerResult as WorkerViewerResult,
};
use orchbee_client_server_protocol::{
    ClientMessage, ControlRejectedReason, RequestId as ClientRequestId, ServerMessage,
    TerminalControlAction as ControlAction, TerminalControlOutcome as ControlOutcome,
    TerminalControlRequest, TerminalViewerAction as ViewerAction,
    TerminalViewerOutcome as ViewerOutcome, TerminalViewerRequest, ViewerRejectedReason,
};

fn client_id(value: u128) -> ClientRequestId {
    ClientRequestId::parse(&format!("{value:032x}")).unwrap()
}

fn viewer(
    session: &mut crate::local_terminal_service::viewer_session::LocalTerminalSession<'_>,
    action: ViewerAction,
) -> ViewerOutcome {
    let ServerMessage::TerminalViewerResult(result) = session.execute(
        ClientMessage::TerminalViewer(TerminalViewerRequest::new(client_id(1000), action)),
    ) else {
        panic!("Viewer result")
    };
    result.into_outcome()
}

fn acquire(
    session: &mut crate::local_terminal_service::viewer_session::LocalTerminalSession<'_>,
    attachment_id: &orchbee_client_server_protocol::ViewerId,
) -> ControlOutcome {
    let ServerMessage::TerminalControlResult(result) =
        session.execute(ClientMessage::TerminalControl(TerminalControlRequest::new(
            client_id(1001),
            ControlAction::Acquire {
                attachment_id: attachment_id.clone(),
            },
        )))
    else {
        panic!("control result")
    };
    result.into_outcome()
}

#[test]
fn exact_disposed_viewer_reuses_either_attach_form_without_old_host_cleanup() {
    for by_binding in [false, true] {
        let Fixture {
            service,
            mut peer,
            worker_thread,
            mut close,
        } = fixture();
        add_terminal(&service, 1);
        close.expected_revision = read_snapshot(&service).revision;
        let marker_id = OperationId::from_bytes([250; 16]).unwrap();
        let (observed_send, observed) = mpsc::sync_channel(1);
        let (release, released) = mpsc::sync_channel(1);
        let peer_thread = thread::spawn(move || {
            let mut events = Vec::new();
            loop {
                match read_message(&mut peer).unwrap() {
                    CapabilityNodeMessage::TerminalViewerRequest(request) => {
                        let marker = request.request_id == marker_id;
                        if !marker {
                            assert!(
                                matches!(request.action, WorkerViewerAction::Attach { .. }),
                                "disposed old Viewer must not Detach: {request:?}"
                            );
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
                        assert_eq!(
                            request.action,
                            WorkerControlAction::Acquire,
                            "disposed old controller must not Release"
                        );
                        events.push("acquire");
                        write_message(
                            &mut peer,
                            &CapabilityNodeMessage::TerminalControlResult(WorkerControlResult {
                                request_id: request.request_id,
                                viewer_id: request.viewer_id,
                                controller_id: request.controller_id,
                                outcome: WorkerControlOutcome::Acquired,
                            }),
                        )
                        .unwrap();
                    }
                    CapabilityNodeMessage::TerminalLifecycleRequest(request) => {
                        let outcome = match request.action {
                            TerminalLifecycleAction::InventoryPage { .. } => {
                                observed_send.send(()).unwrap();
                                released.recv_timeout(Duration::from_secs(3)).unwrap();
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
        let mut session = service.session();
        let ViewerOutcome::Attached {
            attachment_id: old_attachment,
            ..
        } = viewer(
            &mut session,
            ViewerAction::Attach {
                terminal_create_request_id: client_id(11),
            },
        )
        else {
            panic!("old attachment")
        };
        let ControlOutcome::Acquired {
            controller_id: old_controller,
        } = acquire(&mut session, &old_attachment)
        else {
            panic!("old controller")
        };
        assert_eq!(
            viewer(
                &mut session,
                ViewerAction::Attach {
                    terminal_create_request_id: client_id(11)
                }
            ),
            ViewerOutcome::Rejected {
                reason: ViewerRejectedReason::AlreadyAttached
            }
        );
        service.start_close_maintenance().unwrap();
        assert!(matches!(
            service
                .execute_terminal_tab_close(close.clone())
                .unwrap()
                .outcome,
            WorkbenchCommandOutcome::TabCloseAccepted { .. }
        ));
        observed.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_eq!(
            viewer(
                &mut session,
                ViewerAction::Attach {
                    terminal_create_request_id: client_id(11)
                }
            ),
            ViewerOutcome::Rejected {
                reason: ViewerRejectedReason::TerminalUnavailable
            },
            "Closing fences new attachment resolution"
        );
        assert_eq!(
            acquire(&mut session, &old_attachment),
            ControlOutcome::Rejected {
                reason: ControlRejectedReason::ControllerBusy
            },
            "Closing alone cannot reap the still-owned attachment",
        );
        release.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while !matches!(
            read(&service, &close),
            WorkbenchTabCloseReadOutcome::Available {
                status: WorkbenchTabCloseStatus::Closed { .. },
                ..
            }
        ) {
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(5));
        }
        let snapshot = read_snapshot(&service);
        let context = snapshot
            .terminal_contexts
            .iter()
            .find(|context| {
                context.terminal_context_id == snapshot.navigation.standalone_terminal_context_id
            })
            .unwrap();
        let orchbee_workbench_protocol::PanelNode::Panel {
            primary_presentation,
            ..
        } = &context.tabs[0].root
        else {
            panic!("standalone panel")
        };
        let binding = primary_presentation.binding.as_ref().unwrap();
        let site_id =
            orchbee_client_server_protocol::SiteId::parse(snapshot.site_id.as_str()).unwrap();
        let worker_id =
            orchbee_client_server_protocol::WorkerId::parse(binding.worker_id.as_str()).unwrap();
        let terminal_id =
            orchbee_client_server_protocol::TerminalId::parse(binding.terminal_id.as_str())
                .unwrap();
        assert_eq!(
            viewer(
                &mut session,
                ViewerAction::AttachBinding {
                    site_id: orchbee_client_server_protocol::SiteId::parse(&format!(
                        "{:032x}",
                        999
                    ))
                    .unwrap(),
                    worker_id: worker_id.clone(),
                    terminal_id: terminal_id.clone(),
                }
            ),
            ViewerOutcome::Rejected {
                reason: ViewerRejectedReason::TerminalUnavailable
            }
        );
        assert_eq!(
            acquire(&mut session, &old_attachment),
            ControlOutcome::Rejected {
                reason: ControlRejectedReason::ViewerUnavailable
            },
            "admission reaps an exactly disposed attachment before resolving the foreign target"
        );
        assert_eq!(
            viewer(
                &mut session,
                ViewerAction::Attach {
                    terminal_create_request_id: client_id(999)
                }
            ),
            ViewerOutcome::Rejected {
                reason: ViewerRejectedReason::TerminalUnavailable
            }
        );
        assert_eq!(
            acquire(&mut session, &old_attachment),
            ControlOutcome::Rejected {
                reason: ControlRejectedReason::ViewerUnavailable
            }
        );
        let action = if by_binding {
            ViewerAction::AttachBinding {
                site_id,
                worker_id,
                terminal_id,
            }
        } else {
            ViewerAction::Attach {
                terminal_create_request_id: client_id(111),
            }
        };
        let ViewerOutcome::Attached {
            attachment_id: fresh_attachment,
            ..
        } = viewer(&mut session, action)
        else {
            panic!("fresh attachment")
        };
        assert_ne!(fresh_attachment, old_attachment);
        let ControlOutcome::Acquired {
            controller_id: fresh_controller,
        } = acquire(&mut session, &fresh_attachment)
        else {
            panic!("fresh controller")
        };
        assert_ne!(fresh_controller, old_controller);
        assert_eq!(service.controllers.lock().unwrap().len(), 1);
        let handle = service.workers.lock().unwrap().request_handle().unwrap();
        assert!(
            matches!(handle.blocking_admit_for_test(WorkerOperationRequest::Viewer(WorkerViewerRequest {
            request_id: marker_id, attachment_id: OperationId::from_bytes([249; 16]).unwrap(),
            worker_id: orchbee_capability_node_protocol::WorkerId::from_bytes([9; 16]).unwrap(), worker_generation: WorkerGeneration::new(NonZeroU64::MIN), action: WorkerViewerAction::Poll,
        })), Ok(WorkerOperationResult::Viewer(result)) if result.outcome == WorkerViewerOutcome::Empty)
        );
        assert_eq!(
            peer_thread.join().unwrap(),
            vec!["attach", "acquire", "dispose", "attach", "acquire"]
        );
        service.stop_close_maintenance();
        let _exit = worker_thread.join().unwrap();
        drop(session);
    }
}
