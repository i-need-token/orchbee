use super::*;
use crate::{
    session_coordinator::PreparedTerminalTabCreate,
    terminal_coordinator::{Completion, CreateInputs, LaunchCapture, WorkerPlacement},
    workbench_service::{WorkbenchPolicy, WorkbenchService},
    worker_registry::WorkerRegistry,
};
use orchbee_client_server_protocol::RequestId;
use orchbee_workbench_protocol::{
    ConfiguredCommandAction, ConfiguredCommandSource, PanelNode, SessionId, SnapshotReadIntent,
    WorkbenchCommandOutcome, WorkbenchCommandUnavailableReason, WorkbenchRequest,
    WorkbenchRequestId, WorkbenchResponse, WorkbenchRevision, WorkbenchSnapshotOutcome,
};
use std::sync::{Arc, Mutex};

fn installed() -> Arc<LocalTerminalService> {
    install(LocalTerminalService::from_identity(
        Arc::new(Mutex::new(WorkerRegistry::new())),
        Ok(SessionId::parse(&format!("{:032x}", 2)).unwrap()),
    ))
}
fn install(service: LocalTerminalService) -> Arc<LocalTerminalService> {
    let service = Arc::new(service);
    let mut settings = orchbee_local_runtime::read_user_settings_at(None, false, |_| None).unwrap();
    settings.settings.keys = orchbee_local_runtime::KeySettings::from_profile(
        "[[keys.command]]\ncommand='echo shell'\n[[keys.command]]\ncommand='echo pane'\ntype='pane'\n[[keys.command]]\ncommand='echo popup'\ntype='popup'\nwidth='70%'\n[[keys.command]]\ncommand='tools.refresh'\ntype='plugin_action'",
    ).unwrap();
    service.configuration.initialize(&settings).unwrap();
    service
}
fn invocation(id: &str) -> ConfiguredCommandInvocation {
    ConfiguredCommandInvocation {
        command_id: id.into(),
        expected_revision: WorkbenchRevision::new(0),
        source: None,
        selection: None,
        context: Box::default(),
    }
}

#[test]
fn configured_command_installed_catalog_and_source_fences_precede_effects() {
    let service = installed();
    let (revision, entries) = service.configured_commands().unwrap();
    assert_eq!(revision, WorkbenchRevision::new(0));
    assert_eq!(entries.len(), 4);
    for entry in &entries {
        descriptor(entry).validate().unwrap();
    }
    assert_eq!(
        service.resolve_configured_command("cmd_stale").unwrap_err(),
        Failure::NotFound
    );
    for entry in &entries[1..3] {
        assert!(matches!(
            service.prepare_configured_command(&invocation(&entry.id)),
            Err(Failure::StaleSource)
        ));
    }
    let prepared = service
        .prepare_configured_command(&invocation(&entries[0].id))
        .unwrap();
    assert_eq!(prepared.command.command, "echo shell");
    assert!(
        prepared
            .environment
            .iter()
            .any(|(name, value)| name == "ORCHBEE_COMMAND_ID" && value == &entries[0].id)
    );
    let mut stale = invocation(&entries[0].id);
    stale.expected_revision = WorkbenchRevision::new(1);
    assert!(matches!(
        service.prepare_configured_command(&stale),
        Err(Failure::StaleSource)
    ));
}

#[test]
fn configured_command_display_is_the_tui_chrome_text_within_the_wire_bound() {
    let service = installed();
    let (_, entries) = service.configured_commands().unwrap();
    let displays = entries
        .iter()
        .map(|entry| descriptor(entry).display)
        .collect::<Vec<_>>();
    assert_eq!(displays[0].as_deref(), Some("echo shell"));
    assert_eq!(displays[3].as_deref(), Some("tools.refresh"));

    // Controls, separators and reordering format scalars go; an escape sequence goes whole.
    assert_eq!(
        configured_command_display("git\u{1b}[31m status\u{1b}]0;title\u{7}\n\u{202e}-s\u{2028}")
            .as_deref(),
        Some("git status-s")
    );
    assert_eq!(configured_command_display("\u{1b}[0m \t"), None);

    let bound = orchbee_workbench_protocol::MAXIMUM_CONFIGURED_COMMAND_DISPLAY_BYTES;
    let fits = "a".repeat(bound);
    assert_eq!(
        configured_command_display(&fits).as_deref(),
        Some(fits.as_str())
    );
    let cut = configured_command_display(&"界".repeat(bound)).unwrap();
    assert!(cut.len() <= bound && cut.ends_with('…'));
    assert!(cut.trim_end_matches('…').chars().all(|c| c == '界'));
}

#[test]
fn configured_command_permissions_follow_execution_owner_and_preserve_failure() {
    let terminals = installed();
    let entries = terminals.configured_commands().unwrap().1;
    let service = WorkbenchService::new(
        WorkbenchPolicy {
            read: true,
            manage_navigation: false,
            associate_directory: false,
            create_terminal: true,
            manage_terminal_lifecycle: true,
        },
        terminals,
    );
    let session = service.client_connection(std::num::NonZeroU64::new(1).unwrap());
    let invoke = |id| {
        session.execute(WorkbenchRequest::ConfiguredCommand(
            ConfiguredCommandRequest {
                protocol_version: ProtocolVersion,
                request_id: WorkbenchRequestId::parse(&format!("{:032x}", 3)).unwrap(),
                session_id: SessionId::parse(&format!("{:032x}", 2)).unwrap(),
                action: ConfiguredCommandAction::Invoke {
                    invocation: Box::new(invocation(id)),
                },
            },
        ))
    };
    for (index, expected) in [
        (0, Failure::Unavailable),
        (1, Failure::Denied),
        (2, Failure::Denied),
    ] {
        let crate::workbench_service::WorkbenchExecution::Response(
            WorkbenchResponse::ConfiguredCommand(result),
        ) = invoke(&entries[index].id)
        else {
            panic!()
        };
        assert_eq!(result.outcome, Outcome::Failed { reason: expected });
    }
    let crate::workbench_service::WorkbenchExecution::Response(
        WorkbenchResponse::ConfiguredCommand(result),
    ) = invoke(&entries[3].id)
    else {
        panic!()
    };
    assert!(
        matches!(result.outcome, Outcome::Plugin { result, .. } if matches!(result.outcome, orchbee_workbench_protocol::PluginOutcome::Failed { .. }))
    );
}

fn snapshot(service: &LocalTerminalService) -> Snapshot {
    let request = WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 9_999)).unwrap(),
    });
    let WorkbenchResponse::Snapshot(result) = service
        .execute_workbench(request.try_into().unwrap(), false)
        .unwrap()
    else {
        panic!("Snapshot result expected")
    };
    let WorkbenchSnapshotOutcome::Available { snapshot } = result.outcome else {
        panic!("available Snapshot expected")
    };
    snapshot
}

/// Splitting the source Panel is a Panel creation. Routed through Tab creation, which admits only new
/// Tabs, every Pane command ended `denied`; admitted as a split, it reaches Worker placement, which
/// this Session lacks.
#[test]
fn configured_pane_command_is_admitted_as_a_panel_split() {
    let service = install(LocalTerminalService::workspace_fixture(Arc::new(
        Mutex::new(WorkerRegistry::new()),
    )));
    let pane = service.configured_commands().unwrap().1.swap_remove(1);
    let worker = WorkerPlacement {
        worker_id: node::WorkerId::from_bytes([9; 16]).unwrap(),
        generation: node::WorkerGeneration::new(std::num::NonZeroU64::MIN),
    };
    // A Tab whose terminal start stays unknown still holds the Panel a command starts from.
    let before = snapshot(&service);
    let workspace = before.navigation.workspaces[0].terminal_context_id.clone();
    let create = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 11)).unwrap(),
        session_id: before.session_id.clone(),
        expected_revision: before.revision,
        action: WorkbenchAction::TabCreate {
            placement: None,
            automatic_label: false,
            terminal_context_id: workspace.clone(),
            label: WorkbenchLabel::parse("source").unwrap(),
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
            launch: None,
            profile: None,
            options: Default::default(),
        },
    };
    {
        let mut coordinator = service.coordinator.lock().unwrap();
        let PreparedTerminalTabCreate::Dispatch(pending) = coordinator.prepare_terminal_tab_create(
            create,
            CreateInputs::new(
                Some(RequestId::parse(&format!("{:032x}", 12)).unwrap()),
                crate::local_terminal_service::lease_expiry(),
                Some(worker),
                LaunchCapture::absent(),
            ),
        ) else {
            panic!("tab.create dispatch expected")
        };
        coordinator
            .complete_terminal_tab_create(pending, Completion::Unknown, Some(worker))
            .unwrap();
    }
    let current = snapshot(&service);
    let tab = current
        .terminal_contexts
        .iter()
        .find(|context| context.terminal_context_id == workspace)
        .and_then(|context| context.tabs.first())
        .unwrap();
    let PanelNode::Panel { panel_id, .. } = &tab.root else {
        panic!("one Panel expected")
    };
    // The source as a Client reports it: no frame, so the split takes the default size.
    let source: ConfiguredCommandSource = serde_json::from_value(serde_json::json!({
        "clientId": format!("{:032x}", 3),
        "viewRevision": "1",
        "presentation": {
            "terminalContextId": workspace.as_str(),
            "tabId": tab.tab_id.as_str(),
            "panelId": panel_id.as_str(),
            "member": {
                "presentationId": format!("{:032x}", 7),
                "binding": { "terminalId": format!("{:032x}", 1), "workerId": "09".repeat(16) },
                "attachmentGeneration": "0",
                "incarnation": 0,
                "viewport": { "state": "unavailable", "reason": "frame-unavailable" },
            },
        },
    }))
    .unwrap();
    let invocation = ConfiguredCommandInvocation {
        command_id: pane.id.clone(),
        expected_revision: current.revision,
        source: Some(source),
        selection: None,
        context: Box::default(),
    };
    let request = ConfiguredCommandRequest {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 13)).unwrap(),
        session_id: current.session_id.clone(),
        action: ConfiguredCommandAction::Invoke {
            invocation: Box::new(invocation.clone()),
        },
    };
    let prepared = Prepared {
        command: pane.definition,
        snapshot: current,
        context: Default::default(),
        directory: WorkbenchDirectoryPath::parse("C:\\").unwrap(),
        environment: Vec::new(),
        placement: None,
    };
    let Ok(Outcome::Pane { result, .. }) =
        service.dispatch_configured_command(&request, &invocation, prepared)
    else {
        panic!("Pane receipt expected")
    };
    assert!(matches!(
        result.outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::NoCompatibleWorker
        }
    ));
}
