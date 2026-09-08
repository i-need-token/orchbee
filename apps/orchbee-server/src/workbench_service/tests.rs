use super::*;
use crate::{local_terminal_service::LocalTerminalService, worker_registry::WorkerRegistry};
use orchbee_workbench_protocol::{
    SnapshotReadIntent, WorkbenchCommand, WorkbenchCommandUnavailableReason, WorkbenchLabel,
    WorkbenchRequestId, WorkbenchTerminalGeometry,
};
use std::sync::Mutex;

fn response(execution: WorkbenchExecution) -> WorkbenchResponse {
    let WorkbenchExecution::Response(response) = execution else {
        panic!("semantic Workbench response expected")
    };
    response
}

#[test]
fn navigation_permission_alone_denies_tab_create_before_terminal_or_ledger_effects() {
    let terminals = Arc::new(LocalTerminalService::new(Arc::new(Mutex::new(
        WorkerRegistry::new(),
    ))));
    let reader = WorkbenchService::new(
        WorkbenchPolicy {
            read: true,
            manage_navigation: false,
            create_terminal: false,
            manage_terminal_lifecycle: false,
        },
        Arc::clone(&terminals),
    );
    let WorkbenchResponse::Snapshot(snapshot) = response(reader.execute(
        WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{:032x}", 1)).unwrap(),
        }),
    )) else {
        panic!("snapshot expected")
    };
    let WorkbenchSnapshotOutcome::Available { snapshot } = snapshot.outcome else {
        panic!("available snapshot expected")
    };
    let command = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 2)).unwrap(),
        site_id: snapshot.site_id,
        expected_revision: snapshot.revision,
        action: WorkbenchAction::TabCreate {
            terminal_context_id: snapshot.navigation.standalone_terminal_context_id,
            label: WorkbenchLabel::parse("denied").unwrap(),
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
        },
    };

    let navigation_only = WorkbenchService::new(
        WorkbenchPolicy {
            read: true,
            manage_navigation: true,
            create_terminal: false,
            manage_terminal_lifecycle: false,
        },
        Arc::clone(&terminals),
    );
    assert!(matches!(
        response(navigation_only.execute(WorkbenchRequest::Command(command.clone()))),
        WorkbenchResponse::Command(WorkbenchCommandResult {
            outcome: WorkbenchCommandOutcome::Denied {},
            ..
        })
    ));

    let combined = WorkbenchService::new(
        WorkbenchPolicy {
            read: true,
            manage_navigation: true,
            create_terminal: true,
            manage_terminal_lifecycle: false,
        },
        terminals,
    );
    assert!(matches!(
        response(combined.execute(WorkbenchRequest::Command(command))),
        WorkbenchResponse::Command(WorkbenchCommandResult {
            outcome: WorkbenchCommandOutcome::Unavailable {
                reason: WorkbenchCommandUnavailableReason::NoCompatibleWorker,
            },
            ..
        })
    ));
}

#[test]
fn navigation_permission_admits_move_without_terminal_create_permission() {
    let terminals = Arc::new(LocalTerminalService::new(Arc::new(Mutex::new(
        WorkerRegistry::new(),
    ))));
    let service = WorkbenchService::new(
        WorkbenchPolicy {
            read: true,
            manage_navigation: true,
            create_terminal: false,
            manage_terminal_lifecycle: false,
        },
        terminals,
    );
    let WorkbenchResponse::Snapshot(snapshot) = response(service.execute(
        WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{:032x}", 20)).unwrap(),
        }),
    )) else {
        panic!("snapshot expected")
    };
    let WorkbenchSnapshotOutcome::Available { snapshot } = snapshot.outcome else {
        panic!("available snapshot expected")
    };
    let command = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 21)).unwrap(),
        site_id: snapshot.site_id,
        expected_revision: snapshot.revision,
        action: WorkbenchAction::TabMove {
            terminal_context_id: snapshot.navigation.standalone_terminal_context_id,
            tab_id: orchbee_workbench_protocol::WorkbenchTabId::parse(&format!("{:032x}", 22))
                .unwrap(),
            direction: orchbee_workbench_protocol::WorkbenchTabMoveDirection::First,
        },
    };
    assert!(matches!(
        response(service.execute(WorkbenchRequest::Command(command))),
        WorkbenchResponse::Command(WorkbenchCommandResult {
            outcome: WorkbenchCommandOutcome::Rejected {
                reason: orchbee_workbench_protocol::WorkbenchRejectionReason::TabUnavailable,
                ..
            },
            ..
        })
    ));
}

mod tab_close;
