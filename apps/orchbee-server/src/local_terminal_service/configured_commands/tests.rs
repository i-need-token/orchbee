use super::*;
use crate::{
    workbench_service::{WorkbenchPolicy, WorkbenchService},
    worker_registry::WorkerRegistry,
};
use orchbee_workbench_protocol::{
    ConfiguredCommandAction, SiteId, WorkbenchRequest, WorkbenchRequestId, WorkbenchResponse,
    WorkbenchRevision,
};
use std::sync::{Arc, Mutex};

fn installed() -> Arc<LocalTerminalService> {
    let service = Arc::new(LocalTerminalService::from_identity(
        Arc::new(Mutex::new(WorkerRegistry::new())),
        Ok(SiteId::parse(&format!("{:032x}", 2)).unwrap()),
    ));
    let mut settings =
        orchbee_local_site_runtime::read_windows_user_settings_at(None, false, |_| None).unwrap();
    settings.settings.keys = orchbee_local_site_runtime::WindowsKeySettings::from_profile(
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
    let session = service.client_session(std::num::NonZeroU64::new(1).unwrap());
    let invoke = |id| {
        session.execute(WorkbenchRequest::ConfiguredCommand(
            ConfiguredCommandRequest {
                protocol_version: ProtocolVersion,
                request_id: WorkbenchRequestId::parse(&format!("{:032x}", 3)).unwrap(),
                site_id: SiteId::parse(&format!("{:032x}", 2)).unwrap(),
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
