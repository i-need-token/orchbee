use super::*;
use orchbee_workbench_protocol::{
    PluginAction, PluginFailure, PluginInvocationContext, PluginOutcome, PluginRequest, SessionId,
};
#[test]
fn plugin_execution_requires_read_create_and_lifecycle_authority_before_resolution() {
    for read in [false, true] {
        for create in [false, true] {
            for lifecycle in [false, true] {
                let terminals = Arc::new(LocalTerminalService::new(Arc::new(Mutex::new(
                    WorkerRegistry::new(),
                ))));
                let service = WorkbenchService::new(
                    WorkbenchPolicy {
                        read,
                        create_terminal: create,
                        manage_terminal_lifecycle: lifecycle,
                        manage_navigation: true,
                        associate_directory: true,
                    },
                    terminals,
                );
                let request = PluginRequest {
                    protocol_version: ProtocolVersion,
                    request_id: WorkbenchRequestId::parse(&"1".repeat(32)).unwrap(),
                    session_id: SessionId::parse(&"2".repeat(32)).unwrap(),
                    action: PluginAction::Invoke {
                        operation_id: "4".repeat(32),
                        plugin_id: None,
                        action_id: "hello".into(),
                        context: Box::new(PluginInvocationContext::default()),
                        link_catalog: None,
                    },
                };
                let WorkbenchResponse::Plugin(result) =
                    response(service.execute(WorkbenchRequest::Plugin(request)))
                else {
                    panic!()
                };
                assert_eq!(
                    result.outcome,
                    PluginOutcome::Failed {
                        reason: if read && create && lifecycle {
                            PluginFailure::SessionMismatch
                        } else {
                            PluginFailure::Denied
                        }
                    }
                );
            }
        }
    }
}

#[test]
fn popup_session_applies_read_create_and_close_permissions_per_operation() {
    let spec: orchbee_workbench_protocol::PluginPopupSpec = serde_json::from_value(serde_json::json!({
        "selector":{"pluginId":"tools","entrypointId":"dashboard"},
        "source":{"terminalContextId":"33333333333333333333333333333333",
            "tabId":"44444444444444444444444444444444","panelId":"55555555555555555555555555555555",
            "member":{"presentationId":"66666666666666666666666666666666","selectionGeneration":"1"}},
        "context":{"tabId":"44444444444444444444444444444444","focusedPaneId":"55555555555555555555555555555555"},
        "cwd":null,"environment":[],"width":null,"height":null,
    })).unwrap();
    let popup_id = WorkbenchRequestId::parse(&"7".repeat(32)).unwrap();
    for read in [false, true] {
        for create in [false, true] {
            for lifecycle in [false, true] {
                let terminals = Arc::new(LocalTerminalService::new(Arc::new(Mutex::new(
                    WorkerRegistry::new(),
                ))));
                let service = WorkbenchService::new(
                    WorkbenchPolicy {
                        read,
                        create_terminal: create,
                        manage_terminal_lifecycle: lifecycle,
                        manage_navigation: true,
                        associate_directory: true,
                    },
                    terminals,
                );
                let session = service.client_connection(std::num::NonZeroU64::MIN);
                for (action, allowed) in [
                    (
                        PluginAction::PopupOpen {
                            popup_id: popup_id.clone(),
                            spec: Box::new(spec.clone()),
                            geometry: orchbee_workbench_protocol::WorkbenchTerminalGeometry::new(
                                60, 12, 0, 0,
                            )
                            .unwrap(),
                        },
                        read && create && lifecycle,
                    ),
                    (
                        PluginAction::PopupRead {
                            popup_id: popup_id.clone(),
                        },
                        read,
                    ),
                    (
                        PluginAction::PopupClose {
                            popup_id: popup_id.clone(),
                        },
                        read && lifecycle,
                    ),
                ] {
                    let request = PluginRequest {
                        protocol_version: ProtocolVersion,
                        request_id: WorkbenchRequestId::parse(&"1".repeat(32)).unwrap(),
                        session_id: SessionId::parse(&"2".repeat(32)).unwrap(),
                        action,
                    };
                    let WorkbenchResponse::Plugin(result) =
                        response(session.execute(WorkbenchRequest::Plugin(request)))
                    else {
                        panic!()
                    };
                    assert_eq!(
                        result.outcome,
                        PluginOutcome::Failed {
                            reason: if allowed {
                                PluginFailure::SessionMismatch
                            } else {
                                PluginFailure::Denied
                            }
                        }
                    );
                }
            }
        }
    }
}
