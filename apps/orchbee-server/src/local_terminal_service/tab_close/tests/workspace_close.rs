use super::*;
use crate::site_coordinator::PendingTerminalTabCreate;
use orchbee_client_server_protocol::RequestId;
use orchbee_server::WorkbenchGrant;
use orchbee_workbench_protocol::{
    WorkbenchDirectoryPath, WorkbenchProjectId, WorkbenchTerminalContextId,
    WorkbenchWorkspaceCloseRead, WorkbenchWorkspaceCloseReadOutcome, WorkbenchWorkspaceCloseResume,
    WorkbenchWorkspaceCloseResumeOutcome, WorkbenchWorkspaceId,
};

fn worker() -> WorkerPlacement {
    WorkerPlacement {
        worker_id: orchbee_capability_node_protocol::WorkerId::from_bytes([9; 16]).unwrap(),
        generation: WorkerGeneration::new(NonZeroU64::MIN),
    }
}

fn command(
    service: &LocalTerminalService,
    request: u128,
    action: WorkbenchAction,
) -> WorkbenchCommand {
    let snapshot = read_snapshot(service);
    WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: id(request),
        site_id: snapshot.site_id,
        expected_revision: snapshot.revision,
        action,
    }
}

fn navigation(service: &LocalTerminalService, command: WorkbenchCommand) -> WorkbenchCommandResult {
    let WorkbenchResponse::Command(result) = service
        .execute_workbench(WorkbenchRequest::Command(command).try_into().unwrap(), true)
        .unwrap()
    else {
        panic!("navigation result")
    };
    result
}

fn workspace(service: &LocalTerminalService) -> WorkbenchTerminalContextId {
    let project = navigation(
        service,
        command(
            service,
            2000,
            WorkbenchAction::ProjectCreate {
                label: WorkbenchLabel::parse("project").unwrap(),
            },
        ),
    );
    let WorkbenchCommandOutcome::ProjectCreated { project_id, .. } = project.outcome else {
        panic!("project")
    };
    let created = navigation(
        service,
        command(
            service,
            2001,
            WorkbenchAction::WorkspaceCreate {
                project_id: project_id.clone(),
                label: WorkbenchLabel::parse("workspace").unwrap(),
            },
        ),
    );
    let WorkbenchCommandOutcome::WorkspaceCreated {
        workspace_id,
        terminal_context_id,
        ..
    } = created.outcome
    else {
        panic!("workspace")
    };
    let directory = command(
        service,
        2002,
        WorkbenchAction::WorkspacePrimaryDirectoryAccept {
            project_id,
            workspace_id,
            path: WorkbenchDirectoryPath::parse("/tmp").unwrap(),
        },
    );
    let mut site = service.coordinator.lock().unwrap();
    let authority = site.close_authority().unwrap();
    let grant =
        WorkbenchGrant::manage_navigation(directory.site_id.clone()).with_directory_association();
    assert!(matches!(
        authority
            .accept_workspace_directory(
                Some(&grant),
                directory,
                orchbee_workbench_protocol::WorkerId::parse(&format!(
                    "{:032x}",
                    u128::from_be_bytes(worker().worker_id.as_bytes())
                ))
                .unwrap()
            )
            .outcome,
        WorkbenchCommandOutcome::WorkspacePrimaryDirectoryAccepted { .. }
    ));
    terminal_context_id
}

fn scope(
    service: &LocalTerminalService,
    context: &WorkbenchTerminalContextId,
) -> (WorkbenchProjectId, WorkbenchWorkspaceId) {
    read_snapshot(service)
        .navigation
        .projects
        .iter()
        .find_map(|project| {
            project
                .workspaces
                .iter()
                .find(|workspace| workspace.terminal_context_id == *context)
                .map(|workspace| (project.project_id.clone(), workspace.workspace_id.clone()))
        })
        .unwrap()
}

fn pending_create(
    service: &LocalTerminalService,
    context: &WorkbenchTerminalContextId,
    request: u128,
) -> (WorkbenchCommand, PendingTerminalTabCreate) {
    let command = command(
        service,
        request,
        WorkbenchAction::TabCreate {
            terminal_context_id: context.clone(),
            label: WorkbenchLabel::parse("workspace terminal").unwrap(),
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
            launch: None,
            profile: None,
        },
    );
    let PreparedTerminalTabCreate::Dispatch(pending) = service
        .coordinator
        .lock()
        .unwrap()
        .prepare_terminal_tab_create(
            command.clone(),
            Some(RequestId::parse(id(request + 1).as_str()).unwrap()),
            lease_expiry(),
            Some(worker()),
        )
    else {
        panic!("pending Create")
    };
    (command, pending)
}

fn complete_create(
    service: &LocalTerminalService,
    pending: PendingTerminalTabCreate,
    identity: u8,
) -> WorkbenchCommandResult {
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = pending.command() else {
        panic!("launch")
    };
    let TerminalLaunchMode::Immediate(geometry) = launch.mode() else {
        panic!("immediate")
    };
    let result = TerminalLaunchResult::new(
        launch.authorization(),
        TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
            HostTerminalIdentity::new(
                HostCreationId::from_bytes([identity; 16]).unwrap(),
                HostEpoch::from_bytes([4; 16]).unwrap(),
                HostTerminalId::from_bytes([identity; 16]).unwrap(),
            ),
            HostLaunchState::Started(geometry),
        )),
    )
    .unwrap();
    service
        .coordinator
        .lock()
        .unwrap()
        .complete_terminal_tab_create(pending, Completion::Launch(result), Some(worker()))
        .unwrap()
        .result
}

fn close_command(
    service: &LocalTerminalService,
    context: &WorkbenchTerminalContextId,
    request: u128,
) -> WorkbenchCommand {
    let (project_id, workspace_id) = scope(service, context);
    command(
        service,
        request,
        WorkbenchAction::WorkspaceClose {
            project_id,
            workspace_id,
        },
    )
}

fn accept(service: &LocalTerminalService, close: &WorkbenchCommand) -> WorkbenchCommandResult {
    let result = service
        .coordinator
        .lock()
        .unwrap()
        .execute_workspace_close(close.clone(), Some(worker()), true, Instant::now())
        .unwrap();
    assert!(matches!(
        result.outcome,
        WorkbenchCommandOutcome::WorkspaceCloseAccepted { .. }
    ));
    result
}

fn read(
    service: &LocalTerminalService,
    close: &WorkbenchCommand,
) -> WorkbenchWorkspaceCloseReadOutcome {
    let WorkbenchAction::WorkspaceClose {
        project_id,
        workspace_id,
    } = &close.action
    else {
        panic!("Workspace Close")
    };
    service
        .read_workspace_close(WorkbenchWorkspaceCloseRead {
            protocol_version: ProtocolVersion,
            request_id: id(40),
            site_id: close.site_id.clone(),
            project_id: project_id.clone(),
            workspace_id: workspace_id.clone(),
        })
        .unwrap()
        .outcome
}

// These exact replies prove Site/carrier refinement, not real Worker or Host disposal.
fn dispose_all(service: &LocalTerminalService, peer: &mut UnixStream, close: &WorkbenchCommand) {
    let handle = service.workers.lock().unwrap().request_handle().unwrap();
    loop {
        let token = match service
            .coordinator
            .lock()
            .unwrap()
            .reserve_close_lifecycle_step(
                &close.request_id,
                Instant::now(),
                lease_expiry().unwrap(),
            )
            .unwrap()
        {
            Some(token) => token,
            None => break,
        };
        let request = token.request();
        let receipt = handle.submit_lifecycle(request).unwrap();
        assert_eq!(lifecycle(peer), request);
        service
            .coordinator
            .lock()
            .unwrap()
            .close_authority()
            .unwrap()
            .confirm_terminal_tab_close_step_admitted(
                &close.request_id,
                &token,
                receipt.admitted_observed_at(),
            )
            .unwrap();
        let outcome = match &request.action {
            TerminalLifecycleAction::InventoryPage { .. } => settled_page(),
            _ => TerminalLifecycleOutcome::Disposed,
        };
        reply(peer, request, outcome);
        let response = receipt.wait().unwrap();
        service
            .coordinator
            .lock()
            .unwrap()
            .close_authority()
            .unwrap()
            .settle_terminal_tab_close_step(
                &close.request_id,
                token,
                Instant::now(),
                CloseStepObservation::Result(&response),
            )
            .unwrap();
    }
}

fn publish(service: &LocalTerminalService, close: &WorkbenchCommand) -> bool {
    let mut site = service.coordinator.lock().unwrap();
    loop {
        let step = site
            .close_authority()
            .unwrap()
            .next_terminal_tab_close_step_target(&close.request_id, Instant::now())
            .unwrap();
        match step.next() {
            CloseContinuationNext::TargetCleanup => {
                let runtime = step.target().unwrap().runtime();
                service
                    .complete_disposed_target_cleanup(&site, &close.request_id, runtime)
                    .unwrap();
                site.close_authority()
                    .unwrap()
                    .complete_terminal_tab_close_target_cleanup(&close.request_id, runtime)
                    .unwrap();
            }
            CloseContinuationNext::Publication => {
                return site
                    .publish_terminal_tab_close(&close.request_id, Instant::now())
                    .unwrap();
            }
            next => panic!("unexpected Workspace Close step {next:?}"),
        }
    }
}

#[test]
fn empty_workspace_close_publishes_without_lifecycle_and_preserves_project_and_directory() {
    let Fixture {
        service,
        peer,
        worker_thread,
        ..
    } = fixture();
    let context = workspace(&service);
    let (project_id, workspace_id) = scope(&service, &context);
    let before = read_snapshot(&service);
    let close = close_command(&service, &context, 2300);
    let accepted = accept(&service, &close);

    // The empty scope asks for no Worker step: the first scheduled step is
    // the publication itself.
    assert!(matches!(
        read(&service, &close),
        WorkbenchWorkspaceCloseReadOutcome::Available {
            status: WorkbenchTabCloseStatus::Closing { .. },
            ref members,
            ..
        } if members.is_empty()
    ));
    assert!(!publish(&service, &close));

    let after = read_snapshot(&service);
    assert!(
        !after
            .terminal_contexts
            .iter()
            .any(|item| item.terminal_context_id == context)
    );
    let project = after
        .navigation
        .projects
        .iter()
        .find(|project| project.project_id == project_id)
        .unwrap();
    assert!(project.workspaces.is_empty());
    assert_eq!(
        project.directories,
        before.navigation.projects[0].directories
    );
    assert_eq!(
        after.navigation.projects.len(),
        before.navigation.projects.len()
    );
    assert!(matches!(
        read(&service, &close),
        WorkbenchWorkspaceCloseReadOutcome::Available {
            status: WorkbenchTabCloseStatus::Closed { .. },
            ..
        }
    ));
    // Identical request replays; the removed scope rejects fresh closes.
    assert_eq!(accept(&service, &close), accepted);
    let stale = command(
        &service,
        2301,
        WorkbenchAction::WorkspaceClose {
            project_id,
            workspace_id,
        },
    );
    assert!(matches!(
        service
            .coordinator
            .lock()
            .unwrap()
            .execute_workspace_close(stale, Some(worker()), true, Instant::now())
            .unwrap()
            .outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: orchbee_workbench_protocol::WorkbenchRejectionReason::WorkspaceUnavailable,
            ..
        }
    ));
    drop(peer);
    let _exit = worker_thread.join().unwrap();
}

#[test]
fn populated_workspace_close_disposes_every_member_then_removes_scope() {
    let Fixture {
        service,
        mut peer,
        worker_thread,
        ..
    } = fixture();
    let context = workspace(&service);
    let (project_id, workspace_id) = scope(&service, &context);
    let (_, first) = pending_create(&service, &context, 2200);
    complete_create(&service, first, 30);
    let (_, second) = pending_create(&service, &context, 2210);
    complete_create(&service, second, 31);
    let close = close_command(&service, &context, 2300);
    let accepted = accept(&service, &close);
    assert!(matches!(
        read(&service, &close),
        WorkbenchWorkspaceCloseReadOutcome::Available {
            ref members,
            ref counts,
            ..
        } if members.len() == 2 && counts.closing == 2
    ));

    dispose_all(&service, &mut peer, &close);
    assert!(!publish(&service, &close));
    let after = read_snapshot(&service);
    assert!(
        !after
            .terminal_contexts
            .iter()
            .any(|item| item.terminal_context_id == context)
    );
    assert!(!after.navigation.projects.iter().any(|project| {
        project
            .workspaces
            .iter()
            .any(|workspace| workspace.workspace_id == workspace_id)
    }));
    assert_eq!(after.navigation.projects.len(), 1);
    assert_eq!(after.navigation.projects[0].project_id, project_id);
    assert_eq!(after.navigation.projects[0].directories.len(), 1);
    assert!(matches!(
        read(&service, &close),
        WorkbenchWorkspaceCloseReadOutcome::Available {
            status: WorkbenchTabCloseStatus::Closed { .. },
            ..
        }
    ));
    assert_eq!(accept(&service, &close), accepted);
    drop(peer);
    let _exit = worker_thread.join().unwrap();
}

#[test]
fn workspace_close_read_reports_members_and_service_resume_recovers_parked_publication() {
    use orchbee_workbench_protocol::WorkbenchTabCloseAttemptGeneration;
    let Fixture {
        service,
        mut peer,
        worker_thread,
        ..
    } = fixture();
    let context = workspace(&service);
    let (project_id, workspace_id) = scope(&service, &context);
    let (_, first) = pending_create(&service, &context, 2200);
    complete_create(&service, first, 30);
    let close = close_command(&service, &context, 2300);
    accept(&service, &close);
    dispose_all(&service, &mut peer, &close);

    // A failed target cleanup parks the member resumably — the same
    // `disposed-awaiting-publication` truth the Workspace read reports.
    {
        let mut site = service.coordinator.lock().unwrap();
        let step = site
            .close_authority()
            .unwrap()
            .next_terminal_tab_close_step_target(&close.request_id, Instant::now())
            .unwrap();
        assert_eq!(step.next(), CloseContinuationNext::TargetCleanup);
        let runtime = step.target().unwrap().runtime();
        site.close_authority()
            .unwrap()
            .park_terminal_tab_close_target_cleanup(&close.request_id, runtime)
            .unwrap();
    }
    assert!(matches!(
        read(&service, &close),
        WorkbenchWorkspaceCloseReadOutcome::Available {
            status: WorkbenchTabCloseStatus::DisposedAwaitingPublication {
                resumable: true,
                ..
            },
            ref members,
            ref counts,
            ..
        } if members.len() == 1 && counts.disposed_awaiting_publication == 1
    ));
    let resume = WorkbenchWorkspaceCloseResume {
        protocol_version: ProtocolVersion,
        request_id: id(60),
        site_id: close.site_id.clone(),
        project_id: project_id.clone(),
        workspace_id: workspace_id.clone(),
        close_request_id: close.request_id.clone(),
        expected_attempt_generation: WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
    };
    let resumed = service.resume_workspace_close(resume.clone()).unwrap();
    assert!(matches!(
        resumed.outcome,
        WorkbenchWorkspaceCloseResumeOutcome::Resumed { .. }
    ));
    assert_eq!(service.resume_workspace_close(resume), Some(resumed));
    assert!(!publish(&service, &close));
    assert!(matches!(
        read(&service, &close),
        WorkbenchWorkspaceCloseReadOutcome::Available {
            status: WorkbenchTabCloseStatus::Closed { .. },
            ..
        }
    ));
    drop(peer);
    let _exit = worker_thread.join().unwrap();
}
