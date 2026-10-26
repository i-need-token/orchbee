use super::fixture::{Effect, Fixture, client_id, read_snapshot};
use super::*;
use orchbee_workbench_protocol as wb;
use std::sync::atomic::Ordering;

fn source(
    connection: &mut LocalTerminalConnection<'_>,
    fixture: &Fixture,
) -> wb::TerminalEditorRequest {
    let ServerMessage::TerminalViewerResult(attached) =
        connection.execute(ClientMessage::TerminalViewer(TerminalViewerRequest::new(
            client_id(80000),
            fixture.attach(0, true),
        )))
    else {
        panic!()
    };
    let Outcome::Attached { attachment_id, .. } = attached.outcome() else {
        panic!()
    };
    let attachment_id = attachment_id.clone();
    let ServerMessage::TerminalViewerResult(frame) =
        connection.execute(ClientMessage::TerminalViewer(TerminalViewerRequest::new(
            client_id(80001),
            Action::Poll {
                attachment_id: attachment_id.clone(),
            },
        )))
    else {
        panic!()
    };
    assert!(matches!(frame.outcome(), Outcome::Record { .. }));
    let snapshot = read_snapshot(&fixture.service);
    let context = &snapshot.terminal_contexts[0];
    let tab = &context.tabs[0];
    let wb::PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = &tab.root
    else {
        panic!()
    };
    wb::TerminalEditorRequest {
        protocol_version: wb::ProtocolVersion,
        request_id: wb::WorkbenchRequestId::parse(&format!("{:032x}", 80002)).unwrap(),
        session_id: snapshot.session_id,
        action: wb::TerminalEditorAction::Open {
            popup_id: wb::WorkbenchRequestId::parse(&format!("{:032x}", 80003)).unwrap(),
            spec: Box::new(wb::ScrollbackEditorSpec {
                source: wb::ClientPopupSource {
                    terminal_context_id: context.terminal_context_id.clone(),
                    tab_id: tab.tab_id.clone(),
                    panel_id: panel_id.clone(),
                    member: wb::ClientPresentationSelection::Bound {
                        presentation_id: primary_presentation.presentation_id.clone(),
                        binding: primary_presentation.binding.clone().unwrap(),
                        attachment_generation: wb::WorkbenchRevision::new(1),
                        incarnation: 0,
                        viewport: wb::ClientViewportObservation::Unavailable {
                            reason: wb::ClientViewportUnavailable::FrameUnavailable,
                        },
                    },
                },
            }),
            capture: wb::EditorPopupCapture {
                attachment_id: wb::WorkbenchRequestId::parse(attachment_id.as_str()).unwrap(),
                generation: wb::WorkbenchRevision::new(1),
                revision: wb::WorkbenchRevision::new(1),
            },
            geometry: wb::WorkbenchTerminalGeometry::new(60, 12, 0, 0).unwrap(),
        },
    }
}
fn read(request: &wb::TerminalEditorRequest) -> wb::TerminalEditorRequest {
    wb::TerminalEditorRequest {
        action: wb::TerminalEditorAction::Read {
            popup_id: request.action.popup_id().clone(),
        },
        ..request.clone()
    }
}
fn wait_closed(connection: &mut LocalTerminalConnection<'_>, request: &wb::TerminalEditorRequest) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        if matches!(
            connection
                .execute_editor(NonZeroU64::MIN, read(request))
                .outcome,
            wb::TerminalEditorOutcome::Closed { .. }
        ) {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "editor cleanup did not settle"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn editor_requires_connection_source_canonical_membership_and_grants_before_deferred_create_start()
{
    let fixture = Fixture::new();
    fixture.service.start_close_maintenance().unwrap();
    let mut connection = fixture.service.client_connection();
    let request = source(&mut connection, &fixture);
    let snapshot = read_snapshot(&fixture.service);
    let baseline = fixture.barrier().unwrap();
    let mut other = fixture.service.client_connection();
    assert!(matches!(
        other
            .execute_editor(NonZeroU64::new(2).unwrap(), request.clone())
            .outcome,
        wb::TerminalEditorOutcome::Failed {
            reason: wb::TerminalEditorFailure::StaleSource
        }
    ));
    for grant in 0..3 {
        let service = crate::workbench_service::WorkbenchService::new(
            crate::workbench_service::WorkbenchPolicy {
                read: grant != 0,
                manage_navigation: true,
                associate_directory: true,
                create_terminal: grant != 1,
                manage_terminal_lifecycle: grant != 2,
            },
            Arc::clone(&fixture.service),
        );
        let execute = service.client_connection(NonZeroU64::new(30 + grant).unwrap());
        let crate::workbench_service::WorkbenchExecution::Response(
            wb::WorkbenchResponse::TerminalEditor(result),
        ) = execute.execute_with_terminal_connection(
            wb::WorkbenchRequest::TerminalEditor(request.clone()),
            &mut connection,
        )
        else {
            panic!()
        };
        assert!(matches!(
            result.outcome,
            wb::TerminalEditorOutcome::Failed {
                reason: wb::TerminalEditorFailure::Denied
            }
        ));
    }
    for mismatch in 0..7 {
        let mut bad = request.clone();
        let wb::TerminalEditorAction::Open { capture, spec, .. } = &mut bad.action else {
            panic!()
        };
        let id = wb::WorkbenchRequestId::parse(&format!("{:032x}", 99999)).unwrap();
        match mismatch {
            0 => capture.attachment_id = id,
            1 => capture.generation = wb::WorkbenchRevision::new(2),
            2 => capture.revision = wb::WorkbenchRevision::new(2),
            3 => spec.source.panel_id = wb::WorkbenchPanelId::parse(id.as_str()).unwrap(),
            4 => spec.source.tab_id = wb::WorkbenchTabId::parse(id.as_str()).unwrap(),
            5 => {
                spec.source.terminal_context_id =
                    wb::WorkbenchTerminalContextId::parse(id.as_str()).unwrap()
            }
            _ => {
                if let wb::ClientPresentationSelection::Bound { binding, .. } =
                    &mut spec.source.member
                {
                    binding.terminal_id = wb::TerminalId::parse(id.as_str()).unwrap();
                }
            }
        }
        assert!(matches!(
            connection.execute_editor(NonZeroU64::MIN, bad).outcome,
            wb::TerminalEditorOutcome::Failed {
                reason: wb::TerminalEditorFailure::StaleSource
            }
        ));
    }
    assert_eq!(fixture.barrier().unwrap(), baseline);
    let result = connection.execute_editor(NonZeroU64::MIN, request.clone());
    assert!(result.answers(&request));
    assert!(
        matches!(result.outcome,wb::TerminalEditorOutcome::Opened {ref title,..} if title=="editor.exe")
    );
    let effects = fixture.barrier().unwrap();
    let query = effects
        .iter()
        .position(|effect| matches!(effect, Effect::EditorCommand))
        .unwrap();
    let capture = effects
        .iter()
        .position(|effect| matches!(effect, Effect::EditorCapture(_)))
        .unwrap();
    let start = effects
        .iter()
        .position(|effect| matches!(effect, Effect::EditorStart))
        .unwrap();
    assert!(query < capture && capture < start);
    assert!(!effects.iter().any(|effect| matches!(
        effect,
        Effect::Acquire(..) | Effect::Resize(..) | Effect::Input(..) | Effect::Paste(..)
    )));
    assert_eq!(read_snapshot(&fixture.service), snapshot);
    assert!(matches!(
        connection
            .execute_editor(NonZeroU64::MIN, request.clone())
            .outcome,
        wb::TerminalEditorOutcome::Failed {
            reason: wb::TerminalEditorFailure::Busy
        }
    ));
    let plugin = wb::PluginRequest {
        protocol_version: wb::ProtocolVersion,
        request_id: request.request_id.clone(),
        session_id: request.session_id.clone(),
        action: wb::PluginAction::PopupClose {
            popup_id: request.action.popup_id().clone(),
        },
    };
    assert!(matches!(
        fixture
            .service
            .execute_popup(NonZeroU64::MIN, plugin)
            .outcome,
        wb::PluginOutcome::Failed {
            reason: wb::PluginFailure::NotFound
        }
    ));
    let close = wb::TerminalEditorRequest {
        action: wb::TerminalEditorAction::Close {
            popup_id: request.action.popup_id().clone(),
        },
        ..request.clone()
    };
    assert!(matches!(
        connection.execute_editor(NonZeroU64::MIN, close).outcome,
        wb::TerminalEditorOutcome::Closing { .. }
    ));
    wait_closed(&mut connection, &request);
    assert!(
        fixture
            .barrier()
            .unwrap()
            .iter()
            .any(|effect| matches!(effect, Effect::Dispose(10)))
    );
    assert_eq!(read_snapshot(&fixture.service), snapshot);
}

#[test]
fn editor_lost_create_or_start_is_retained_unknown_without_resubmission() {
    for fault in [1, 10] {
        let fixture = Fixture::new();
        fixture.service.start_close_maintenance().unwrap();
        let mut connection = fixture.service.client_connection();
        let request = source(&mut connection, &fixture);
        fixture.popup_fault.store(fault, Ordering::Release);
        assert!(matches!(
            connection
                .execute_editor(NonZeroU64::MIN, request.clone())
                .outcome,
            wb::TerminalEditorOutcome::Unknown { .. }
        ));
        assert!(matches!(
            connection
                .execute_editor(NonZeroU64::MIN, read(&request))
                .outcome,
            wb::TerminalEditorOutcome::Unknown { .. }
        ));
        fixture.service.disconnect_popup(NonZeroU64::MIN);
        fixture.service.popup_maintenance_turn();
        let effects = fixture.effects.lock().unwrap();
        assert_eq!(
            effects
                .iter()
                .filter(|effect| matches!(effect, Effect::EditorCapture(_)))
                .count(),
            1
        );
        assert_eq!(
            effects
                .iter()
                .filter(|effect| matches!(effect, Effect::EditorStart))
                .count(),
            usize::from(fault == 10)
        );
        assert!(
            !effects
                .iter()
                .any(|effect| matches!(effect, Effect::PopupForce(_) | Effect::Dispose(_)))
        );
    }
}

#[test]
fn editor_known_refusals_release_reservations_and_failed_start_disposes_awaiting_start() {
    for fault in [8, 11, 9] {
        let fixture = Fixture::new();
        fixture.service.start_close_maintenance().unwrap();
        let mut connection = fixture.service.client_connection();
        let request = source(&mut connection, &fixture);
        fixture.popup_fault.store(fault, Ordering::Release);
        assert!(matches!(
            connection
                .execute_editor(NonZeroU64::MIN, request.clone())
                .outcome,
            wb::TerminalEditorOutcome::Failed { .. }
        ));
        if fault == 9 {
            wait_closed(&mut connection, &request);
            assert!(
                fixture
                    .barrier()
                    .unwrap()
                    .iter()
                    .any(|effect| matches!(effect, Effect::Dispose(10)))
            );
        } else {
            fixture.popup_fault.store(0, Ordering::Release);
            // A fresh public popup identity starts one new operation after a known refusal.
            let mut fresh = request.clone();
            let wb::TerminalEditorAction::Open { popup_id, .. } = &mut fresh.action else {
                panic!()
            };
            *popup_id = wb::WorkbenchRequestId::parse(&format!("{:032x}", 80004)).unwrap();
            assert!(matches!(
                connection
                    .execute_editor(NonZeroU64::MIN, fresh.clone())
                    .outcome,
                wb::TerminalEditorOutcome::Opened { .. }
            ));
            fixture.service.disconnect_popup(NonZeroU64::MIN);
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            loop {
                fixture.service.popup_maintenance_turn();
                if fixture
                    .barrier()
                    .unwrap()
                    .iter()
                    .any(|effect| matches!(effect, Effect::Dispose(10)))
                {
                    break;
                }
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
    }
}
