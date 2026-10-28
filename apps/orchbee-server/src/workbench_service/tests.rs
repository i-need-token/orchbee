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
fn workspace_git_requires_only_read_grant_and_never_changes_canonical_state() {
    use orchbee_workbench_protocol::{
        WorkbenchWorkspaceId, WorkspaceGitFailure, WorkspaceGitOutcome, WorkspaceGitRequest,
    };
    let terminals = Arc::new(LocalTerminalService::new(Arc::new(Mutex::new(
        WorkerRegistry::new(),
    ))));
    let service = |read| {
        WorkbenchService::new(
            WorkbenchPolicy {
                read,
                manage_navigation: false,
                associate_directory: false,
                create_terminal: false,
                manage_terminal_lifecycle: false,
            },
            terminals.clone(),
        )
    };
    let reader = service(true);
    let snapshot = || {
        let WorkbenchResponse::Snapshot(reply) = response(reader.execute(
            WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
                protocol_version: ProtocolVersion,
                request_id: WorkbenchRequestId::parse(&format!("{:032x}", 1)).unwrap(),
            }),
        )) else {
            panic!("snapshot")
        };
        let WorkbenchSnapshotOutcome::Available { snapshot } = reply.outcome else {
            panic!("available snapshot")
        };
        snapshot
    };
    let before = snapshot();
    for (read, reason) in [
        (false, WorkspaceGitFailure::Denied),
        (true, WorkspaceGitFailure::Unavailable),
    ] {
        let request = WorkspaceGitRequest {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{:032x}", 2)).unwrap(),
            session_id: before.session_id.clone(),
            workspace_id: WorkbenchWorkspaceId::parse(&format!("{:032x}", 3)).unwrap(),
        };
        let WorkbenchResponse::WorkspaceGit(reply) =
            response(service(read).execute(WorkbenchRequest::WorkspaceGit(request.clone())))
        else {
            panic!("typed Git result")
        };
        assert!(reply.answers(&request));
        assert_eq!(reply.outcome, WorkspaceGitOutcome::Failed { reason });
        assert_eq!(snapshot(), before);
    }
}

#[cfg(windows)]
#[test]
fn configuration_reads_and_reloads_require_their_own_grant_before_owner_admission() {
    use orchbee_workbench_protocol::{
        ConfigurationAction, ConfigurationFailure, ConfigurationOutcome, ConfigurationRequest,
    };
    let terminals = Arc::new(LocalTerminalService::new(Arc::new(Mutex::new(
        WorkerRegistry::new(),
    ))));
    for (action, read, manage_navigation, denied) in [
        (ConfigurationAction::Current, false, true, true),
        (ConfigurationAction::Reload, true, false, true),
        (ConfigurationAction::Current, true, false, false),
        (ConfigurationAction::Reload, false, true, false),
    ] {
        let service = WorkbenchService::new(
            WorkbenchPolicy {
                read,
                manage_navigation,
                associate_directory: false,
                create_terminal: false,
                manage_terminal_lifecycle: false,
            },
            Arc::clone(&terminals),
        );
        let WorkbenchResponse::Configuration(result) = response(service.execute(
            WorkbenchRequest::Configuration(ConfigurationRequest {
                protocol_version: ProtocolVersion,
                request_id: WorkbenchRequestId::parse(&format!("{:032x}", 1)).unwrap(),
                session_id:
                    orchbee_workbench_protocol::SessionId::parse(&format!("{:032x}", 2)).unwrap(),
                action,
            }),
        )) else {
            panic!("typed configuration result");
        };
        assert_eq!(
            matches!(
                result.outcome,
                ConfigurationOutcome::Failed {
                    reason: ConfigurationFailure::Denied,
                    ..
                }
            ),
            denied
        );
        assert!(
            matches!(result.outcome, ConfigurationOutcome::Failed { .. }),
            "uninitialized owner never fabricates publication"
        );
    }
}

#[test]
fn navigation_permission_alone_denies_tab_create_before_terminal_or_ledger_effects() {
    let terminals = Arc::new(LocalTerminalService::workspace_fixture(Arc::new(
        Mutex::new(WorkerRegistry::new()),
    )));
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
        session_id: snapshot.session_id,
        expected_revision: snapshot.revision,
        action: WorkbenchAction::TabCreate {
            placement: None,
            automatic_label: false,
            terminal_context_id: snapshot.navigation.workspaces[0]
                .terminal_context_id
                .clone(),
            label: WorkbenchLabel::parse("denied").unwrap(),
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
            launch: None,
            profile: None,
            options: Default::default(),
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
    let terminals = Arc::new(LocalTerminalService::workspace_fixture(Arc::new(
        Mutex::new(WorkerRegistry::new()),
    )));
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
        session_id: snapshot.session_id,
        expected_revision: snapshot.revision,
        action: WorkbenchAction::TabMove {
            terminal_context_id: snapshot.navigation.workspaces[0]
                .terminal_context_id
                .clone(),
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

#[test]
fn panel_terminal_creation_requires_both_grants_and_reaches_the_lifecycle_owner() {
    use orchbee_workbench_protocol::{
        WorkbenchPanelId, WorkbenchPanelTerminalTarget, WorkbenchRejectionReason, WorkbenchTabId,
    };
    let terminals = Arc::new(LocalTerminalService::workspace_fixture(Arc::new(
        Mutex::new(WorkerRegistry::new()),
    )));
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
    let WorkbenchResponse::Snapshot(result) = response(reader.execute(
        WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{:032x}", 9800)).unwrap(),
        }),
    )) else {
        panic!()
    };
    let WorkbenchSnapshotOutcome::Available { snapshot } = result.outcome else {
        panic!()
    };
    let command = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 9801)).unwrap(),
        session_id: snapshot.session_id,
        expected_revision: snapshot.revision,
        action: WorkbenchAction::PanelCreateTerminal {
            terminal_context_id: snapshot.navigation.workspaces[0]
                .terminal_context_id
                .clone(),
            tab_id: WorkbenchTabId::parse(&format!("{:032x}", 9802)).unwrap(),
            panel_id: WorkbenchPanelId::parse(&format!("{:032x}", 9803)).unwrap(),
            target: WorkbenchPanelTerminalTarget::Child {
                label: WorkbenchLabel::parse("child").unwrap(),
            },
            geometry: WorkbenchTerminalGeometry::new(80, 24, 0, 0).unwrap(),
            launch: None,
            profile: None,
            options: Default::default(),
        },
    };
    for (navigation, create) in [(false, false), (true, false), (false, true), (true, true)] {
        let service = WorkbenchService::new(
            WorkbenchPolicy {
                read: true,
                manage_navigation: navigation,
                associate_directory: false,
                create_terminal: create,
                manage_terminal_lifecycle: false,
            },
            Arc::clone(&terminals),
        );
        let WorkbenchResponse::Command(result) =
            response(service.execute(WorkbenchRequest::Command(command.clone())))
        else {
            panic!()
        };
        if navigation && create {
            assert!(matches!(
                result.outcome,
                WorkbenchCommandOutcome::Rejected {
                    reason: WorkbenchRejectionReason::TabUnavailable,
                    ..
                }
            ));
        } else {
            assert!(matches!(result.outcome, WorkbenchCommandOutcome::Denied {}));
        }
    }
}

mod tab_close;
mod workspace_close;

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
        session_id: snapshot.session_id.clone(),
        expected_revision: snapshot.revision,
        action: WorkbenchAction::WorkspacePrimaryDirectoryAccept {
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
                        orchbee_workbench_protocol::WorkbenchRejectionReason::WorkspaceUnavailable,
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

#[test]
fn workspace_launch_requires_navigation_directory_and_terminal_grants() {
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
        session_id: snapshot.session_id.clone(),
        expected_revision: snapshot.revision,
        action: WorkbenchAction::WorkspaceLaunch {
            placement: orchbee_workbench_protocol::WorkspacePlacement::Project {
                project_id: orchbee_workbench_protocol::WorkbenchProjectId::parse(&format!(
                    "{:032x}",
                    403
                ))
                .unwrap(),
            },
            label: WorkbenchLabel::parse("launch").unwrap(),
            geometry: WorkbenchTerminalGeometry::new(80, 24, 0, 0).unwrap(),
            launch: None,
            profile: None,
            options: Box::new(orchbee_workbench_protocol::LaunchOptions {
                plugin_pane: None,
                plugin_context: None,
                cwd: Some(
                    orchbee_workbench_protocol::WorkbenchDirectoryPath::parse(r"C:\workspace")
                        .unwrap(),
                ),
                environment: Default::default(),
            }),
        },
    };
    for (manage_navigation, associate_directory, create_terminal) in [
        (false, false, false),
        (false, false, true),
        (false, true, false),
        (false, true, true),
        (true, false, false),
        (true, false, true),
        (true, true, false),
        (true, true, true),
    ] {
        let service = WorkbenchService::new(
            WorkbenchPolicy {
                read: true,
                manage_navigation,
                associate_directory,
                create_terminal,
                manage_terminal_lifecycle: false,
            },
            Arc::clone(&terminals),
        );
        let WorkbenchResponse::Command(result) =
            response(service.execute(WorkbenchRequest::Command(command.clone())))
        else {
            panic!("command result expected")
        };
        if manage_navigation && associate_directory && create_terminal {
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

#[cfg(windows)]
mod client_view;

#[cfg(windows)]
mod workspace_metadata;

#[cfg(windows)]
mod panel_metadata;

mod panel_agent;

#[cfg(windows)]
mod notifications;

#[test]
fn layout_structure_uses_navigation_only_without_a_worker() {
    let terminals = Arc::new(LocalTerminalService::workspace_fixture(Arc::new(
        Mutex::new(WorkerRegistry::new()),
    )));
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
    let WorkbenchResponse::Snapshot(before) = response(service.execute(
        WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{:032x}", 9809)).unwrap(),
        }),
    )) else {
        panic!()
    };
    let WorkbenchSnapshotOutcome::Available { snapshot } = before.outcome else {
        panic!()
    };
    let command = WorkbenchCommand { protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 9810)).unwrap(),
        session_id: snapshot.session_id, expected_revision: snapshot.revision,
        action: WorkbenchAction::TabCreateLayout { terminal_context_id: snapshot.navigation.workspaces[0].terminal_context_id.clone(),
            label: WorkbenchLabel::parse("Layout").unwrap(), automatic_label: false,
            layout: serde_json::from_value(serde_json::json!({"kind":"panel","primary":{"label":"Empty","customLabel":null},"children":[],"rightClick":"orchbee"})).unwrap() } };
    let response = response(service.execute(WorkbenchRequest::Command(command)));
    assert!(matches!(
        response,
        WorkbenchResponse::Command(orchbee_workbench_protocol::WorkbenchCommandResult {
            outcome: WorkbenchCommandOutcome::TabLayoutCreated { .. },
            ..
        })
    ));
}

#[cfg(windows)]
mod window_title;

#[cfg(all(test, windows))]
mod plugins;
