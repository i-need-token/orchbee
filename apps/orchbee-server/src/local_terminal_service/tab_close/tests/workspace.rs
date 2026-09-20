use super::*;
use crate::site_coordinator::{PendingTerminalTabCreate, PreparedOpen};
use orchbee_client_server_protocol::{Geometry, OpenOutcome, RequestId, TerminalOpenRequest};
use orchbee_server::WorkbenchGrant;
use orchbee_workbench_protocol::{
    WorkbenchCommandResult, WorkbenchDirectoryPath, WorkbenchTabMoveDirection,
    WorkbenchTerminalContextId,
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

fn tabs(
    service: &LocalTerminalService,
    context: &WorkbenchTerminalContextId,
) -> Vec<orchbee_workbench_protocol::Tab> {
    read_snapshot(service)
        .terminal_contexts
        .into_iter()
        .find(|item| &item.terminal_context_id == context)
        .unwrap()
        .tabs
}

fn close_command(
    service: &LocalTerminalService,
    context: &WorkbenchTerminalContextId,
    request: u128,
) -> WorkbenchCommand {
    command(
        service,
        request,
        WorkbenchAction::TabClose {
            terminal_context_id: context.clone(),
            tab_id: tabs(service, context)[0].tab_id.clone(),
        },
    )
}

fn accept(service: &LocalTerminalService, close: &WorkbenchCommand) -> WorkbenchCommandResult {
    let result = service
        .coordinator
        .lock()
        .unwrap()
        .execute_terminal_tab_close(close.clone(), Some(worker()), true, Instant::now())
        .unwrap();
    assert!(matches!(
        result.outcome,
        WorkbenchCommandOutcome::TabCloseAccepted { .. }
    ));
    result
}

// These exact replies prove Site/carrier refinement, not real Worker or Host disposal.
fn dispose(service: &LocalTerminalService, peer: &mut UnixStream, close: &WorkbenchCommand) {
    let handle = service.workers.lock().unwrap().request_handle().unwrap();
    for outcome in [settled_page(), TerminalLifecycleOutcome::Disposed] {
        let token = service
            .coordinator
            .lock()
            .unwrap()
            .reserve_close_lifecycle_step(
                &close.request_id,
                Instant::now(),
                lease_expiry().unwrap(),
            )
            .unwrap()
            .unwrap();
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
    let runtime = site
        .close_authority()
        .unwrap()
        .terminal_tab_close_target(&close.request_id)
        .unwrap()
        .runtime();
    service
        .complete_disposed_target_cleanup(&site, &close.request_id, runtime)
        .unwrap();
    site.close_authority()
        .unwrap()
        .complete_terminal_tab_close_target_cleanup(&close.request_id, runtime)
        .unwrap();
    site.publish_terminal_tab_close(&close.request_id, Instant::now())
        .unwrap()
}

#[derive(Debug, PartialEq, Eq)]
enum OpenProbe {
    Wait,
    Ready(OpenOutcome),
}

fn open_probe(service: &LocalTerminalService) -> OpenProbe {
    match service.coordinator.lock().unwrap().prepare_open(
        &TerminalOpenRequest::new(
            RequestId::parse(id(2900).as_str()).unwrap(),
            Geometry::new(80, 24, 8, 16).unwrap(),
        ),
        None,
        Some(worker()),
        None,
    ) {
        PreparedOpen::Wait => OpenProbe::Wait,
        PreparedOpen::Ready(result) => OpenProbe::Ready(result),
        PreparedOpen::Dispatch(_) => panic!("probe has no candidate and must not create"),
    }
}

#[test]
fn workspace_publication_preserves_empty_creating_and_settled_standalone_slots_without_notification()
 {
    for state in ["empty", "creating", "settled"] {
        let Fixture {
            service,
            mut peer,
            worker_thread,
            close,
        } = fixture();
        if state != "settled" {
            accept(&service, &close);
            dispose(&service, &mut peer, &close);
            assert!(publish(&service, &close));
        }
        let standalone = read_snapshot(&service)
            .navigation
            .standalone_terminal_context_id;
        let pending = (state == "creating").then(|| pending_create(&service, &standalone, 2100).1);
        let before_slot = open_probe(&service);
        let expected_slot = match state {
            "creating" => OpenProbe::Wait,
            "settled" => OpenProbe::Ready(OpenOutcome::Opened {
                terminal_create_request_id: RequestId::parse(id(11).as_str()).unwrap(),
            }),
            _ => OpenProbe::Ready(OpenOutcome::Unavailable {
                reason: orchbee_client_server_protocol::CreateUnavailableReason::ServerBusy,
            }),
        };
        assert_eq!(before_slot, expected_slot);
        let before_tabs = tabs(&service, &standalone);
        let context = workspace(&service);
        let (_, target) = pending_create(&service, &context, 2200);
        complete_create(&service, target, 30);
        let close = close_command(&service, &context, 2300);
        let accepted = accept(&service, &close);
        dispose(&service, &mut peer, &close);
        assert!(
            !publish(&service, &close),
            "Workspace must not notify standalone waiters: {state}"
        );
        assert_eq!(open_probe(&service), before_slot, "{state}");
        assert_eq!(tabs(&service, &standalone), before_tabs);
        assert!(tabs(&service, &context).is_empty());
        assert_eq!(accept(&service, &close), accepted);
        if let Some(pending) = pending {
            complete_create(&service, pending, 31);
            assert_eq!(
                open_probe(&service),
                OpenProbe::Ready(OpenOutcome::Opened {
                    terminal_create_request_id: RequestId::parse(id(2101).as_str()).unwrap(),
                })
            );
        }
        drop(peer);
        let _exit = worker_thread.join().unwrap();
    }
}

#[test]
fn moved_pending_workspace_create_settles_after_close_at_its_original_revision_and_replays() {
    let Fixture {
        service,
        mut peer,
        worker_thread,
        ..
    } = fixture();
    let context = workspace(&service);
    let (_, target) = pending_create(&service, &context, 2200);
    complete_create(&service, target, 30);
    let (create, pending) = pending_create(&service, &context, 2210);
    let original = tabs(&service, &context);
    let fixed_revision = read_snapshot(&service).revision;
    let mut close = close_command(&service, &context, 2300);
    let accepted = accept(&service, &close);
    let moved = command(
        &service,
        2310,
        WorkbenchAction::TabMove {
            terminal_context_id: context.clone(),
            tab_id: original[1].tab_id.clone(),
            direction: WorkbenchTabMoveDirection::First,
        },
    );
    let moved_result = service
        .coordinator
        .lock()
        .unwrap()
        .execute_terminal_tab_move(moved.clone())
        .unwrap();
    assert!(matches!(
        moved_result.outcome,
        WorkbenchCommandOutcome::TabMoved { .. }
    ));
    assert_eq!(
        tabs(&service, &context),
        vec![original[1].clone(), original[0].clone()]
    );
    dispose(&service, &mut peer, &close);
    assert!(!publish(&service, &close));
    assert_eq!(tabs(&service, &context), vec![original[1].clone()]);
    let published_revision = read_snapshot(&service).revision;
    assert_eq!(
        published_revision,
        WorkbenchRevision::new(fixed_revision.value() + 2)
    );
    let result = complete_create(&service, pending, 31);
    assert!(
        matches!(&result.outcome, WorkbenchCommandOutcome::TabCreated { revision, tab_id, .. }
        if revision == &fixed_revision && tab_id == &original[1].tab_id)
    );
    assert_eq!(read_snapshot(&service).revision, published_revision);
    assert_eq!(tabs(&service, &context)[0].tab_id, original[1].tab_id);
    let PreparedTerminalTabCreate::Ready(replay) = service
        .coordinator
        .lock()
        .unwrap()
        .prepare_terminal_tab_create(create, None, None, None)
    else {
        panic!("Create replay")
    };
    assert_eq!(replay, result);
    assert_eq!(accept(&service, &close), accepted);
    assert_eq!(
        service
            .coordinator
            .lock()
            .unwrap()
            .execute_terminal_tab_move(moved)
            .unwrap(),
        moved_result
    );
    close.request_id = id(2320);
    close.expected_revision = published_revision;
    assert!(matches!(
        service
            .coordinator
            .lock()
            .unwrap()
            .execute_terminal_tab_close(close, Some(worker()), true, Instant::now())
            .unwrap()
            .outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: orchbee_workbench_protocol::WorkbenchRejectionReason::TabUnavailable,
            ..
        }
    ));
    drop(peer);
    let _exit = worker_thread.join().unwrap();
}

#[test]
fn last_workspace_close_keeps_directory_recreates_fresh_ids_and_cleans_only_its_context() {
    use crate::local_terminal_service::controller_session::ControlTarget;
    use orchbee_capability_node_protocol::OperationId;
    let Fixture {
        service,
        mut peer,
        worker_thread,
        mut close,
    } = fixture();
    let context = workspace(&service);
    let (_, target) = pending_create(&service, &context, 2200);
    complete_create(&service, target, 30);
    let old_tab = tabs(&service, &context)[0].clone();
    let before = read_snapshot(&service);
    close.expected_revision = before.revision;
    let workspace_close = close_command(&service, &context, 2300);
    let mut owners = Vec::new();
    for (index, command) in [&close, &workspace_close].into_iter().enumerate() {
        accept(&service, command);
        let runtime = service
            .coordinator
            .lock()
            .unwrap()
            .close_authority()
            .unwrap()
            .terminal_tab_close_target(&command.request_id)
            .unwrap()
            .runtime();
        let key = ControlTarget {
            worker: runtime.worker_id().as_bytes(),
            generation: runtime.worker_generation().get(),
            host_epoch: runtime.host_identity().host_epoch().as_bytes(),
            terminal: runtime.host_identity().terminal_id().as_bytes(),
        };
        let owner = OperationId::from_bytes([40 + index as u8; 16]).unwrap();
        // Seed the real local controller registry; this test covers exact cleanup, not acquisition.
        assert!(
            service
                .controllers
                .lock()
                .unwrap()
                .insert(key, owner)
                .is_none()
        );
        owners.push((key, owner));
    }
    dispose(&service, &mut peer, &workspace_close);
    assert!(!publish(&service, &workspace_close));
    assert_eq!(
        service
            .controllers
            .lock()
            .unwrap()
            .iter()
            .map(|(key, owner)| (*key, *owner))
            .collect::<Vec<_>>(),
        vec![owners[0]]
    );
    let empty = read_snapshot(&service);
    assert_eq!(empty.navigation, before.navigation);
    assert!(tabs(&service, &context).is_empty());
    let (_, fresh) = pending_create(&service, &context, 2400);
    complete_create(&service, fresh, 32);
    let recreated = tabs(&service, &context)[0].clone();
    assert_ne!(recreated.tab_id, old_tab.tab_id);
    let [
        orchbee_workbench_protocol::PanelNode::Panel {
            panel_id: old_panel,
            primary_presentation: old_presentation,
            ..
        },
        orchbee_workbench_protocol::PanelNode::Panel {
            panel_id: new_panel,
            primary_presentation: new_presentation,
            ..
        },
    ] = [&old_tab.root, &recreated.root]
    else {
        panic!("simple panels")
    };
    assert_ne!(old_panel, new_panel);
    assert_ne!(
        old_presentation.presentation_id,
        new_presentation.presentation_id
    );
    assert_ne!(old_presentation.binding, new_presentation.binding);
    dispose(&service, &mut peer, &close);
    assert!(publish(&service, &close));
    assert!(service.controllers.lock().unwrap().is_empty());
    assert_eq!(tabs(&service, &context), vec![recreated]);
    assert_eq!(read_snapshot(&service).navigation, before.navigation);
    assert!(matches!(
        read(&service, &workspace_close),
        WorkbenchTabCloseReadOutcome::Available {
            status: WorkbenchTabCloseStatus::Closed { .. },
            ..
        }
    ));
    drop(peer);
    let _exit = worker_thread.join().unwrap();
}
