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
            associate_directory: false,
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
            launch: None,
            profile: None,
        },
    };

    let navigation_only = WorkbenchService::new(
        WorkbenchPolicy {
            read: true,
            manage_navigation: true,
            associate_directory: false,
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
            associate_directory: false,
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
            associate_directory: false,
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

#[test]
fn directory_association_requires_its_own_grant_and_navigation_without_launch_permission() {
    let terminals = Arc::new(LocalTerminalService::new(Arc::new(Mutex::new(
        WorkerRegistry::new(),
    ))));
    let reader = WorkbenchService::new(
        WorkbenchPolicy {
            read: true,
            manage_navigation: true,
            associate_directory: false,
            create_terminal: false,
            manage_terminal_lifecycle: false,
        },
        Arc::clone(&terminals),
    );
    let read = WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 401)).unwrap(),
    });
    let WorkbenchResponse::Snapshot(before) = response(reader.execute(read.clone())) else {
        panic!("snapshot expected")
    };
    let WorkbenchSnapshotOutcome::Available { snapshot } = &before.outcome else {
        panic!("available snapshot expected")
    };
    let command = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 402)).unwrap(),
        site_id: snapshot.site_id.clone(),
        expected_revision: snapshot.revision,
        action: WorkbenchAction::WorkspacePrimaryDirectoryAccept {
            project_id: orchbee_workbench_protocol::WorkbenchProjectId::parse(&format!(
                "{:032x}",
                403
            ))
            .unwrap(),
            workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(&format!(
                "{:032x}",
                404
            ))
            .unwrap(),
            path: orchbee_workbench_protocol::WorkbenchDirectoryPath::parse("/tmp/workspace")
                .unwrap(),
        },
    };
    for (manage_navigation, associate_directory) in
        [(false, false), (true, false), (false, true), (true, true)]
    {
        let service = WorkbenchService::new(
            WorkbenchPolicy {
                read: true,
                manage_navigation,
                associate_directory,
                create_terminal: false,
                manage_terminal_lifecycle: false,
            },
            Arc::clone(&terminals),
        );
        let WorkbenchResponse::Command(result) =
            response(service.execute(WorkbenchRequest::Command(command.clone())))
        else {
            panic!("command result expected")
        };
        if manage_navigation && associate_directory {
            assert!(matches!(
                result.outcome,
                WorkbenchCommandOutcome::Rejected {
                    reason:
                        orchbee_workbench_protocol::WorkbenchRejectionReason::ProjectUnavailable,
                    ..
                }
            ));
        } else {
            assert_eq!(result.outcome, WorkbenchCommandOutcome::Denied {});
        }
        assert_eq!(
            response(reader.execute(read.clone())),
            WorkbenchResponse::Snapshot(before.clone())
        );
    }
}
