use super::*;
use orchbee_workbench_protocol::WorkbenchAction;

fn choose(ui: &mut NativeUi, target: MenuTarget, action: MenuAction) -> ClientAction {
    assert!(ui.open_action_menu(
        MenuTarget::Drawn {
            source: ui.view().action_menu_source().unwrap(),
            target: Box::new(target),
        },
        None,
        false,
    ));
    let row = ui
        .view()
        .action_menu()
        .unwrap()
        .actions
        .iter()
        .position(|candidate| *candidate == action)
        .unwrap();
    assert_eq!(ui.view().action_menu().unwrap().unavailable[row], None);
    let token = arm(ui);
    activate_row(ui, token, row);
    let action = ui.take_action_menu_action(false).unwrap();
    assert_eq!(ui.take_action_menu_action(false), None);
    action
}

#[test]
fn leaving_noncurrent_workspace_submits_exact_organization_without_navigating() {
    let (mut ui, mailbox, _, _) = tests::context_selection::ready(false);
    let original = ui.state.as_ref().unwrap().cache().location().clone();
    let selected = ui.state.as_ref().unwrap().desired().cloned();
    let target = ui
        .state
        .as_ref()
        .unwrap()
        .cache()
        .navigation()
        .iter()
        .find(|context| matches!(context.location, NativeContextLocation::Workspace { .. }))
        .unwrap()
        .location
        .clone();
    let action = choose(
        &mut ui,
        MenuTarget::WorkspaceProjects(target.clone()),
        MenuAction::LeaveWorkspaceProject,
    );
    assert!(matches!(
        action,
        ClientAction::RequestWorkspaceOrganization(_)
    ));
    assert!(mailbox.try_recv().unwrap().is_none());
    ui.apply_client_action(action);
    assert!(ui.view().setup_draft().is_none());
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::WorkspaceSetup(marker) = lease.record() else {
        panic!("one exact grouping command");
    };
    let NativeContextLocation::Workspace { workspace_id, .. } = target else {
        panic!("canonical Workspace");
    };
    assert_eq!(
        marker.0.command.action,
        WorkbenchAction::WorkspaceSetProject {
            workspace_id,
            project_id: None
        }
    );
    assert_eq!(ui.state.as_ref().unwrap().cache().location(), &original);
    assert_eq!(ui.state.as_ref().unwrap().desired(), selected.as_ref());
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn right_clicking_noncurrent_ungrouped_workspace_keeps_canonical_focus_and_exact_menu_scope() {
    let (mut ui, mailbox, clock, _) = tests::context_selection::ready(false);
    let legacy = ui.state.as_ref().unwrap().cache().location().clone();
    tests::context_selection::settle_nonempty_context_selection(
        &mut ui,
        &mailbox,
        &clock,
        true,
        tests::context_selection::snapshot(false),
    );
    tests::context_selection::make_desired_ready(&mut ui, 2);
    let original = Origin::capture(ui.state.as_ref());
    assert!(matches!(
        ui.state.as_ref().unwrap().cache().location(),
        NativeContextLocation::Workspace { .. }
    ));
    let current_pane = ui.state.as_ref().unwrap().focus_pane_ref();
    assert!(ui.open_action_menu(
        MenuTarget::Drawn {
            source: ui.view().action_menu_source().unwrap(),
            target: Box::new(MenuTarget::Workspace(legacy.clone())),
        },
        Some((3, 8)),
        false,
    ));
    let view = ui.view();
    let menu = view.action_menu().unwrap();
    assert_eq!(menu.source, &MenuTarget::Workspace(legacy));
    assert_eq!(
        menu.unavailable[action_row(&ui, MenuAction::RenameWorkspace)],
        None
    );
    assert_eq!(menu.unavailable[action_row(&ui, MenuAction::NewTab)], None);
    assert_eq!(Origin::capture(ui.state.as_ref()), original);
    assert_eq!(ui.state.as_ref().unwrap().focus_pane_ref(), current_pane);
    assert!(mailbox.try_recv().unwrap().is_none());
    ui.cancel_action_menu();
    assert_eq!(Origin::capture(ui.state.as_ref()), original);
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn deleting_noncurrent_workspace_submits_only_conditional_admission_without_navigation() {
    let (mut ui, mailbox, _, _) = tests::context_selection::ready(false);
    let original = ui.state.as_ref().unwrap().cache().location().clone();
    let selected = ui.state.as_ref().unwrap().desired().cloned();
    let target = ui
        .state
        .as_ref()
        .unwrap()
        .cache()
        .navigation()
        .iter()
        .find(|context| matches!(context.location, NativeContextLocation::Workspace { .. }))
        .unwrap()
        .location
        .clone();
    let NativeContextLocation::Workspace { workspace_id, .. } = &target else {
        panic!("Workspace target");
    };
    let expected = workspace_id.clone();
    let action = choose(
        &mut ui,
        MenuTarget::Workspace(target),
        MenuAction::DeleteWorkspace,
    );
    assert!(matches!(
        action,
        ClientAction::RequestSidebarWorkspaceDelete(_)
    ));
    assert!(mailbox.try_recv().unwrap().is_none());
    ui.apply_client_action(action);
    assert!(ui.view().workspace_close_confirmation().is_none());
    let evidence = mailbox.workspace_close_worker_loss().unwrap();
    assert_eq!(
        evidence.operation.workspace_command().unwrap().action,
        WorkbenchAction::WorkspaceCloseIfIdle {
            workspace_id: expected
        }
    );
    assert_eq!(ui.state.as_ref().unwrap().cache().location(), &original);
    assert_eq!(ui.state.as_ref().unwrap().desired(), selected.as_ref());
    let lease = mailbox
        .try_recv()
        .unwrap()
        .expect("one conditional Close lease");
    assert!(matches!(lease.record(), QueuedCommand::Refresh(_)));
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn workspace_delete_menu_reviews_only_a_known_resumable_guard_refusal() {
    use crate::terminal_session::state::workspace_close::WorkspaceCloseResult;
    use crate::terminal_session::ui::tests::workspace_close as close_ui;
    use orchbee_workbench_protocol::{
        WorkbenchSnapshotOutcome, WorkbenchTabCloseBlockedReason as Reason, WorkbenchTabClosePhase,
        WorkbenchTabCloseStatus as Status, WorkbenchWorkspaceCloseReadOutcome,
    };
    for (status, conditional, recoverable) in [
        (
            Some(Status::CleanupBlocked {
                reason: Reason::HostRefused,
                resumable: true,
            }),
            true,
            true,
        ),
        (
            Some(Status::CleanupBlocked {
                reason: Reason::HostRefused,
                resumable: false,
            }),
            true,
            false,
        ),
        (
            Some(Status::CleanupBlocked {
                reason: Reason::InventoryIncomplete,
                resumable: true,
            }),
            true,
            false,
        ),
        (
            Some(Status::Closing {
                phase: WorkbenchTabClosePhase::Queued,
            }),
            true,
            false,
        ),
        (None, true, false),
        (
            Some(Status::CleanupBlocked {
                reason: Reason::HostRefused,
                resumable: true,
            }),
            false,
            false,
        ),
    ] {
        let (mut ui, mailbox, clock, snapshot) = close_ui::ready();
        let target = ui.state.as_ref().unwrap().cache().location().clone();
        let workspace = snapshot.navigation.workspaces[1].workspace_id.clone();
        let lease = if conditional {
            let action = choose(
                &mut ui,
                MenuTarget::Workspace(target.clone()),
                MenuAction::DeleteWorkspace,
            );
            ui.apply_client_action(action);
            mailbox.try_recv().unwrap().unwrap()
        } else {
            close_ui::submit(&mut ui, &mailbox)
        };
        let QueuedCommand::Refresh(marker) = lease.record() else {
            panic!("conditional Close");
        };
        let operation = marker.workspace_close().unwrap();
        mailbox
            .mark_refresh_in_flight(operation.generation)
            .unwrap();
        let mut result = close_ui::receipt(operation, false);
        if let Some(status) = status {
            let WorkspaceCloseResult::WorkspaceObserved { outcome, .. } = &mut result else {
                unreachable!()
            };
            let WorkbenchWorkspaceCloseReadOutcome::Available {
                status: retained, ..
            } = outcome.as_mut()
            else {
                unreachable!()
            };
            *retained = status;
        } else {
            result = WorkspaceCloseResult::Unknown;
        }
        assert!(mailbox.retain_workspace_close_result(operation, result));
        let wait = mailbox
            .publish_event_before(
                WorkerEvent::WorkspaceCloseResult {
                    evidence: mailbox.workspace_close_worker_loss().unwrap(),
                    outcome: Box::new(WorkbenchSnapshotOutcome::Available { snapshot }),
                },
                crate::terminal_session::mailbox::EventDeadline::PlacementOnly(
                    operation.deadlines.forward_cutoff(),
                ),
                &clock,
            )
            .unwrap();
        ui.tick();
        assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
        mailbox.finish_lease(lease).unwrap();
        assert!(mailbox.try_recv().unwrap().is_none());
        assert_eq!(
            ui.state
                .as_ref()
                .unwrap()
                .can_confirm_guarded_workspace_close(&workspace),
            recoverable
        );
        assert!(ui.open_action_menu(
            MenuTarget::Drawn {
                source: ui.view().action_menu_source().unwrap(),
                target: Box::new(MenuTarget::Workspace(target)),
            },
            None,
            false,
        ));
        let row = action_row(&ui, MenuAction::DeleteWorkspace);
        assert_eq!(
            ui.view().action_menu().unwrap().unavailable[row],
            (!recoverable).then_some(MenuUnavailable::OperationBusy)
        );
        if recoverable {
            let token = arm(&mut ui);
            activate_row(&mut ui, token, row);
            let action = ui.take_action_menu_action(false).unwrap();
            assert_eq!(
                action,
                ClientAction::RequestWorkspaceCloseAt(workspace.clone())
            );
            ui.apply_client_action(action);
            let prompt = ui.view().workspace_close_confirmation().unwrap();
            assert!(!prompt.interactive());
            assert!(
                mailbox.try_recv().unwrap().is_none(),
                "retry opens review before effects"
            );
            let token = prompt.token();
            ui.after_workspace_close_draw(Some(ModalPresentation::Presented(token)));
            ui.capture_confirmation_key(crate::client_view::ClientKey::Enter);
            let lease = mailbox.try_recv().unwrap().unwrap();
            let QueuedCommand::Refresh(marker) = lease.record() else {
                panic!("explicit Close");
            };
            assert_eq!(
                marker
                    .workspace_close()
                    .unwrap()
                    .workspace_command()
                    .unwrap()
                    .action,
                WorkbenchAction::WorkspaceClose {
                    workspace_id: workspace
                }
            );
        }
    }
}

#[test]
fn dissolving_noncurrent_project_submits_only_its_metadata_action() {
    let (mut ui, mailbox, _, _) = tests::context_selection::ready(false);
    let original = ui.state.as_ref().unwrap().cache().location().clone();
    let selected = ui.state.as_ref().unwrap().desired().cloned();
    let project = ui
        .state
        .as_ref()
        .unwrap()
        .cache()
        .canonical_snapshot()
        .navigation
        .projects[0]
        .project_id
        .clone();
    let action = choose(
        &mut ui,
        MenuTarget::Project(project.clone()),
        MenuAction::DeleteProject,
    );
    ui.apply_client_action(action);
    let lease = mailbox.try_recv().unwrap().expect("one metadata command");
    let QueuedCommand::WorkspaceSetup(marker) = lease.record() else {
        panic!("Project dissolve")
    };
    assert_eq!(
        marker.0.command.action,
        WorkbenchAction::ProjectDissolveIfIdle {
            project_id: project
        }
    );
    assert_eq!(ui.state.as_ref().unwrap().cache().location(), &original);
    assert_eq!(ui.state.as_ref().unwrap().desired(), selected.as_ref());
}
