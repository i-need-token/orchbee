use super::fixture::{Effect, Fixture, read_snapshot};
use orchbee_workbench_protocol::{
    PanelNode, PluginAction, PluginFailure, PluginOutcome, PluginPopupSpec, PluginRequest,
    ProtocolVersion, WorkbenchRequestId, WorkbenchTerminalGeometry,
};
use std::{
    num::NonZeroU64,
    time::{Duration, Instant},
};

fn open(fixture: &Fixture) -> PluginRequest {
    let snapshot = read_snapshot(&fixture.service);
    let context = &snapshot.terminal_contexts[0];
    let tab = &context.tabs[0];
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = &tab.root
    else {
        panic!("source pane");
    };
    let spec: PluginPopupSpec = serde_json::from_value(serde_json::json!({
        "selector":{"pluginId":"tools","entrypointId":"dashboard"},
        "source":{"terminalContextId":context.terminal_context_id,"tabId":tab.tab_id,
            "panelId":panel_id,"member":{"presentationId":primary_presentation.presentation_id,"binding":primary_presentation.binding,
                "attachmentGeneration":"1","incarnation":0,"viewport":{"state":"unavailable","reason":"frame-unavailable"}}},
        "context":{"tabId":tab.tab_id,"focusedPaneId":panel_id},
        "cwd":null,"environment":[],"width":{"kind":"percent","value":75},"height":null,
    })).unwrap();
    PluginRequest {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 90000)).unwrap(),
        session_id: snapshot.session_id,
        action: PluginAction::PopupOpen {
            popup_id: WorkbenchRequestId::parse(&format!("{:032x}", 90001)).unwrap(),
            spec: Box::new(spec),
            geometry: WorkbenchTerminalGeometry::new(60, 12, 0, 0).unwrap(),
        },
    }
}
fn read(request: &PluginRequest) -> PluginRequest {
    let PluginAction::PopupOpen { popup_id, .. } = &request.action else {
        panic!();
    };
    PluginRequest {
        action: PluginAction::PopupRead {
            popup_id: popup_id.clone(),
        },
        ..request.clone()
    }
}

#[test]
fn popup_is_connection_owned_independent_and_closed_by_exact_physical_maintenance() {
    if !super::plugin_overlays::enter_isolated_catalog() {
        return;
    }
    let fixture = Fixture::new();
    fixture.service.start_close_maintenance().unwrap();
    let request = open(&fixture);
    let owner = NonZeroU64::MIN;
    let snapshot = read_snapshot(&fixture.service);
    assert!(matches!(
        fixture.service.execute_plugin(request.clone()).outcome,
        PluginOutcome::Failed {
            reason: PluginFailure::Denied
        }
    ));
    let outcome = fixture.service.execute_popup(owner, request.clone());
    assert!(outcome.answers(&request));
    assert!(matches!(outcome.outcome, PluginOutcome::PopupOpened { .. }));
    assert_eq!(read_snapshot(&fixture.service), snapshot);
    assert!(matches!(
        fixture
            .service
            .execute_popup(owner, request.clone())
            .outcome,
        PluginOutcome::Failed {
            reason: PluginFailure::Busy
        }
    ));
    let other = NonZeroU64::new(2).unwrap();
    assert!(matches!(
        fixture
            .service
            .execute_popup(other, request.clone())
            .outcome,
        PluginOutcome::Failed {
            reason: PluginFailure::Conflict
        }
    ));
    let read = read(&request);
    assert!(matches!(
        fixture.service.execute_popup(other, read.clone()).outcome,
        PluginOutcome::Failed {
            reason: PluginFailure::NotFound
        }
    ));
    assert!(matches!(
        fixture.service.execute_popup(owner, read.clone()).outcome,
        PluginOutcome::PopupState { settled: true, .. }
    ));
    let PluginAction::PopupRead { popup_id } = &read.action else {
        panic!()
    };
    let close = PluginRequest {
        action: PluginAction::PopupClose {
            popup_id: popup_id.clone(),
        },
        ..read.clone()
    };
    assert!(matches!(
        fixture.service.execute_popup(owner, close).outcome,
        PluginOutcome::PopupClosing { .. }
    ));
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if matches!(
            fixture.service.execute_popup(owner, read.clone()).outcome,
            PluginOutcome::PopupClosed { .. }
        ) {
            break;
        }
        assert!(Instant::now() < deadline, "popup disposal did not settle");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(read_snapshot(&fixture.service), snapshot);
    let effects = fixture.barrier().unwrap();
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::PopupLaunch(_)))
            .count(),
        1
    );
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::PopupForce(10)))
            .count(),
        1
    );
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::PopupInventory(10, 2)))
            .count(),
        1
    );
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::Dispose(10)))
            .count(),
        1
    );
    let reference = orchbee_client_server_protocol::RequestId::parse(popup_id.as_str()).unwrap();
    let mut connection = fixture.service.client_connection();
    let result = connection.execute(
        orchbee_client_server_protocol::ClientMessage::TerminalViewer(
            orchbee_client_server_protocol::TerminalViewerRequest::new(
                reference.clone(),
                orchbee_client_server_protocol::TerminalViewerAction::Attach {
                    terminal_create_request_id: reference,
                },
            ),
        ),
    );
    assert!(
        matches!(result,orchbee_client_server_protocol::ServerMessage::TerminalViewerResult(r)
        if !matches!(r.outcome(),orchbee_client_server_protocol::TerminalViewerOutcome::Attached { .. }))
    );
    fixture.service.disconnect_popup(owner);
    assert!(matches!(
        fixture.service.execute_popup(owner, read).outcome,
        PluginOutcome::Failed {
            reason: PluginFailure::NotFound
        }
    ));
}

#[test]
fn popup_disconnect_transfers_cleanup_without_mutating_the_source() {
    if !super::plugin_overlays::enter_isolated_catalog() {
        return;
    }
    let fixture = Fixture::new();
    fixture.service.start_close_maintenance().unwrap();
    let request = open(&fixture);
    let snapshot = read_snapshot(&fixture.service);
    assert!(matches!(
        fixture
            .service
            .execute_popup(NonZeroU64::MIN, request.clone())
            .outcome,
        PluginOutcome::PopupOpened { .. }
    ));
    fixture.service.disconnect_popup(NonZeroU64::MIN);
    let deadline = Instant::now() + Duration::from_secs(3);
    while !matches!(
        fixture
            .service
            .execute_popup(NonZeroU64::MIN, read(&request))
            .outcome,
        PluginOutcome::Failed {
            reason: PluginFailure::NotFound
        }
    ) {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(read_snapshot(&fixture.service), snapshot);
    assert_eq!(
        fixture
            .barrier()
            .unwrap()
            .iter()
            .filter(|e| matches!(e, Effect::Dispose(10)))
            .count(),
        1
    );
}

#[test]
fn popup_refuses_a_removed_source_before_launch_and_releases_admission() {
    if !super::plugin_overlays::enter_isolated_catalog() {
        return;
    }
    let fixture = Fixture::new();
    fixture.service.start_close_maintenance().unwrap();
    let mut request = open(&fixture);
    let PluginAction::PopupOpen { spec, .. } = &mut request.action else {
        panic!();
    };
    let missing = orchbee_workbench_protocol::WorkbenchPanelId::parse(&"f".repeat(32)).unwrap();
    spec.context.focused_pane_id = Some(missing.to_string());
    spec.source.panel_id = missing;
    assert!(matches!(
        fixture
            .service
            .execute_popup(NonZeroU64::MIN, request)
            .outcome,
        PluginOutcome::Failed {
            reason: PluginFailure::InvalidIntent
        }
    ));
    assert!(fixture.effects.lock().unwrap().is_empty());
    let request = open(&fixture);
    assert!(matches!(
        fixture
            .service
            .execute_popup(NonZeroU64::MIN, request)
            .outcome,
        PluginOutcome::PopupOpened { .. }
    ));
    fixture.service.disconnect_popup(NonZeroU64::MIN);
}

#[test]
fn popup_lost_launch_or_force_receipt_retains_ownership_without_replaying() {
    if !super::plugin_overlays::enter_isolated_catalog() {
        return;
    }
    for fault in [1, 2] {
        let fixture = Fixture::new();
        fixture.service.start_close_maintenance().unwrap();
        fixture
            .popup_fault
            .store(fault, std::sync::atomic::Ordering::Release);
        let request = open(&fixture);
        let snapshot = read_snapshot(&fixture.service);
        let result = fixture
            .service
            .execute_popup(NonZeroU64::MIN, request.clone());
        if fault == 1 {
            assert!(matches!(result.outcome, PluginOutcome::PopupUnknown { .. }));
        } else {
            assert!(matches!(result.outcome, PluginOutcome::PopupOpened { .. }));
            let mut close = read(&request);
            let PluginAction::PopupRead { popup_id } = close.action else {
                panic!()
            };
            close.action = PluginAction::PopupClose { popup_id };
            assert!(matches!(
                fixture
                    .service
                    .execute_popup(NonZeroU64::MIN, close)
                    .outcome,
                PluginOutcome::PopupClosing { .. }
            ));
        }
        let deadline = Instant::now() + Duration::from_secs(3);
        while !matches!(
            fixture
                .service
                .execute_popup(NonZeroU64::MIN, read(&request))
                .outcome,
            PluginOutcome::PopupUnknown { .. }
        ) {
            assert!(Instant::now() < deadline, "lost mutation was not retained");
            std::thread::sleep(Duration::from_millis(10));
        }
        fixture.service.disconnect_popup(NonZeroU64::MIN);
        assert!(matches!(
            fixture
                .service
                .execute_popup(NonZeroU64::MIN, read(&request))
                .outcome,
            PluginOutcome::PopupUnknown { .. }
        ));
        let mut replacement = request.clone();
        let PluginAction::PopupOpen { popup_id, .. } = &mut replacement.action else {
            panic!()
        };
        *popup_id = WorkbenchRequestId::parse(&format!("{:032x}", 90002)).unwrap();
        assert!(matches!(
            fixture
                .service
                .execute_popup(NonZeroU64::MIN, replacement)
                .outcome,
            PluginOutcome::Failed {
                reason: PluginFailure::Busy
            }
        ));
        let effects = fixture.effects.lock().unwrap().clone();
        assert_eq!(
            effects
                .iter()
                .filter(|e| matches!(e, Effect::PopupLaunch(_)))
                .count(),
            1
        );
        assert_eq!(
            effects
                .iter()
                .filter(|e| matches!(e, Effect::PopupForce(10)))
                .count(),
            usize::from(fault == 2)
        );
        assert!(!effects.iter().any(|e| matches!(e, Effect::Dispose(_))));
        assert_eq!(read_snapshot(&fixture.service), snapshot);
    }
}

#[test]
fn popup_known_disposal_settles_its_uncertain_observer_before_replacement_reads() {
    if !super::plugin_overlays::enter_isolated_catalog() {
        return;
    }
    let fixture = Fixture::new();
    fixture.service.start_close_maintenance().unwrap();
    let request = open(&fixture);
    assert!(matches!(
        fixture
            .service
            .execute_popup(NonZeroU64::MIN, request.clone())
            .outcome,
        PluginOutcome::PopupOpened { .. }
    ));
    fixture
        .detach_refusals
        .store(1, std::sync::atomic::Ordering::Release);
    assert!(matches!(
        fixture
            .service
            .execute_popup(NonZeroU64::MIN, read(&request))
            .outcome,
        PluginOutcome::Failed {
            reason: PluginFailure::PhysicalUnknown
        }
    ));
    let mut close = read(&request);
    let PluginAction::PopupRead { popup_id } = close.action else {
        panic!()
    };
    close.action = PluginAction::PopupClose { popup_id };
    assert!(matches!(
        fixture
            .service
            .execute_popup(NonZeroU64::MIN, close)
            .outcome,
        PluginOutcome::PopupClosing { .. }
    ));
    let deadline = Instant::now() + Duration::from_secs(3);
    while !matches!(
        fixture
            .service
            .execute_popup(NonZeroU64::MIN, read(&request))
            .outcome,
        PluginOutcome::PopupClosed { .. }
    ) {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    fixture.service.disconnect_popup(NonZeroU64::MIN);
    let mut replacement = request.clone();
    let PluginAction::PopupOpen { popup_id, .. } = &mut replacement.action else {
        panic!()
    };
    *popup_id = WorkbenchRequestId::parse(&format!("{:032x}", 90002)).unwrap();
    assert!(matches!(
        fixture
            .service
            .execute_popup(NonZeroU64::MIN, replacement.clone())
            .outcome,
        PluginOutcome::PopupOpened { .. }
    ));
    assert!(matches!(
        fixture
            .service
            .execute_popup(NonZeroU64::MIN, read(&replacement))
            .outcome,
        PluginOutcome::PopupState { settled: true, .. }
    ));
    fixture.service.disconnect_popup(NonZeroU64::MIN);
}
