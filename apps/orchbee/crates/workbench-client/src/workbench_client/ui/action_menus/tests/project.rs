use super::*;
use crate::client_view::{KeyCode, Modifiers};
use crate::local_client::test_support::server::adjustable_operation_clock;
use crate::workbench_client::mailbox::EventDeadline;
use crate::workbench_client::ui::CommandMailbox;
use crate::workbench_state::workspace_setup::{SetupEdit, SetupStage};
use orchbee_workbench_protocol::WorkbenchAction;
use orchbee_workbench_protocol::{
    Directory, Project, SessionId, Snapshot, TerminalContext, WorkbenchDirectoryId,
    WorkbenchDirectoryPath, WorkbenchProjectId, WorkbenchRevision, WorkbenchTerminalContextId,
    WorkbenchWorkspaceId, WorkerId,
};
use std::sync::{Arc, atomic::AtomicU64};
use std::time::Instant;

fn snapshot() -> Snapshot {
    let mut snapshot = tests::context_selection::snapshot(true);
    let directory_id = WorkbenchDirectoryId::parse(&format!("{:032x}", 901)).unwrap();
    snapshot.navigation.directories.push(Directory {
        directory_id: directory_id.clone(),
        worker_id: WorkerId::parse(&format!("{:032x}", 902)).unwrap(),
        path: WorkbenchDirectoryPath::parse("C:/project").unwrap(),
    });
    snapshot.navigation.workspaces[1].primary_directory_id = Some(directory_id);
    let mut other = snapshot.navigation.projects[0].clone();
    other.project_id = WorkbenchProjectId::parse(&format!("{:032x}", 970)).unwrap();
    let mut workspace = snapshot.navigation.workspaces[1].clone();
    workspace.project_id = Some(other.project_id.clone());
    workspace.primary_directory_id = None;
    workspace.workspace_id = WorkbenchWorkspaceId::parse(&format!("{:032x}", 971)).unwrap();
    workspace.terminal_context_id =
        WorkbenchTerminalContextId::parse(&format!("{:032x}", 972)).unwrap();
    snapshot.terminal_contexts.push(TerminalContext {
        terminal_context_id: workspace.terminal_context_id.clone(),
        groups: Vec::new(),
        tabs: Vec::new(),
    });
    snapshot.navigation.workspaces.push(workspace);
    snapshot.navigation.projects.push(other);
    snapshot
}

#[test]
fn create_menu_omits_unresolved_worktree_and_reuses_exact_project_quick_workspace() {
    let mut snapshot = snapshot();
    snapshot.terminal_contexts[0].tabs.clear();
    let workspace = &snapshot.navigation.workspaces[1];
    let location = ClientContextLocation::Workspace {
        workspace_id: workspace.workspace_id.clone(),
        terminal_context_id: workspace.terminal_context_id.clone(),
    };
    let (mut ui, mailbox) = ready(&snapshot, &location);
    assert_eq!(ui.view().tab_count(), 0);
    let create = ui.view().tab_create_target().unwrap();
    assert_eq!(create.location, location);
    assert_eq!(ui.tab_create_action(&create), Some(ClientAction::CreateTab));
    assert!(mailbox.try_recv().unwrap().is_none());
    let project = &snapshot.navigation.projects[1];
    let source = ui.view().action_menu_source().unwrap();
    assert!(ui.open_action_menu(
        MenuTarget::Drawn {
            source,
            target: Box::new(MenuTarget::Create(Some(project.project_id.clone())))
        },
        None,
        false
    ));
    let view = ui.view();
    let menu = view.action_menu().unwrap();
    assert_eq!(
        menu.actions,
        &[MenuAction::QuickWorkspace, MenuAction::WorkspaceAtDirectory]
    );
    assert_eq!(menu.unavailable[0], None);
    assert_eq!(menu.unavailable, &[None, None]);
    let token = arm(&mut ui);
    let row = action_row(&ui, MenuAction::QuickWorkspace);
    activate_row(&mut ui, token, row);
    let action = ui.take_action_menu_action(false).unwrap();
    assert!(matches!(
        action.intent(),
        ClientAction::RequestProjectQuickWorkspace
    ));
    assert!(mailbox.try_recv().unwrap().is_none());
    assert_eq!(
        ui.apply_client_action(action),
        ClientActionDisposition::LayoutChanged
    );
    let clock = ui.clock.clone();
    crate::workbench_client::ui::tests::workspace_setup::finish_preparation(
        &mut ui, &mailbox, &clock, &snapshot, false,
    );
    let preview = ui.view().workspace_creation_preview().unwrap();
    assert!(preview.current && preview.replaces_content());
    assert_eq!(preview.project_id, Some(&project.project_id));
    let lease = mailbox.try_recv().unwrap().unwrap();
    assert!(
        matches!(lease.record(), QueuedCommand::WorkspaceSetup(marker)
        if matches!(&marker.0.command.action,
            WorkbenchAction::WorkspaceLaunch { placement: orchbee_workbench_protocol::WorkspacePlacement::Project { project_id }, .. }
            if project_id == &project.project_id))
    );
    assert!(ui.view().setup_draft().is_none());
    assert!(mailbox.try_recv().unwrap().is_none());
    mailbox.close(CarrierFailure::WorkerExited);
    assert_eq!(ui.state.as_ref().unwrap().cache().location(), &location);
}

fn workspace(snapshot: &Snapshot, project: &Project) -> ClientContextLocation {
    let workspace = snapshot
        .navigation
        .workspaces
        .iter()
        .find(|workspace| workspace.project_id.as_ref() == Some(&project.project_id))
        .unwrap();
    ClientContextLocation::Workspace {
        workspace_id: workspace.workspace_id.clone(),
        terminal_context_id: workspace.terminal_context_id.clone(),
    }
}

fn empty_project(snapshot: &mut Snapshot, project: &WorkbenchProjectId) {
    let contexts: Vec<_> = snapshot
        .navigation
        .workspaces
        .iter()
        .filter(|workspace| workspace.project_id.as_ref() == Some(project))
        .map(|workspace| workspace.terminal_context_id.clone())
        .collect();
    snapshot
        .navigation
        .workspaces
        .retain(|workspace| workspace.project_id.as_ref() != Some(project));
    snapshot
        .terminal_contexts
        .retain(|context| !contexts.contains(&context.terminal_context_id));
}

fn dissolve_project(snapshot: &mut Snapshot, project: &WorkbenchProjectId) {
    snapshot
        .navigation
        .projects
        .retain(|candidate| &candidate.project_id != project);
    for workspace in &mut snapshot.navigation.workspaces {
        if workspace.project_id.as_ref() == Some(project) {
            workspace.project_id = None;
        }
    }
}

fn install(ui: &mut WorkbenchClientUi, snapshot: &Snapshot, location: &ClientContextLocation) {
    let cache =
        crate::workbench_projection::project_native_context_tabs(snapshot, location).unwrap();
    ui.state = Some(
        WorkbenchClientState::bootstrap(cache, 10, Geometry::new(80, 24, 8, 16).unwrap())
            .unwrap()
            .0,
    );
}

fn ready(
    snapshot: &Snapshot,
    location: &ClientContextLocation,
) -> (WorkbenchClientUi, Arc<CommandMailbox>) {
    let clock = adjustable_operation_clock(Arc::new(AtomicU64::new(0)));
    let mailbox = Arc::new(CommandMailbox::new(
        10,
        Geometry::new(80, 24, 8, 16).unwrap(),
    ));
    let mut ui = WorkbenchClientUi::new(Arc::clone(&mailbox), clock.clone());
    // Draft admission requires the mailbox and UI to own the same context.
    // Bootstrap both owners instead of replacing only the ready UI's state.
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::Bootstrap {
                selection: None,
                cache: crate::workbench_projection::project_native_context_tabs(snapshot, location)
                    .unwrap(),
            },
            EventDeadline::Startup(clock.now() + crate::workbench_state::OPERATION_TOTAL),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(matches!(
        mailbox.take_deposited_reply(&wait).unwrap(),
        Some(UiReply::BootstrapApplied(None))
    ));
    assert_eq!(ui.view().phase(), DisplayPhase::Empty);
    assert_eq!(ui.state.as_ref().unwrap().cache().location(), location);
    (ui, mailbox)
}

fn open(ui: &mut WorkbenchClientUi, project: &Project) -> bool {
    ui.open_action_menu(
        MenuTarget::Drawn {
            source: ui.view().action_menu_source().unwrap(),
            target: Box::new(MenuTarget::Project(project.project_id.clone())),
        },
        None,
        false,
    )
}

/// Opens New Workspace ▸ from an open Project menu.
fn open_creation(ui: &mut WorkbenchClientUi) {
    assert_eq!(
        ui.view().action_menu().unwrap().actions,
        crate::action_menu::PROJECT_ACTIONS
    );
    let token = arm(ui);
    activate_row(ui, token, action_row(ui, MenuAction::WorkspaceCreation));
    assert_eq!(ui.take_action_menu_action(false), None);
    assert!(matches!(
        ui.menus.intent().unwrap().entry(),
        MenuTarget::WorkspaceCreation(_)
    ));
}

#[test]
fn workspace_project_submenu_uses_exact_ids_with_duplicate_labels_and_keeps_current_focus() {
    for grouped in [false, true] {
        let mut snapshot = snapshot();
        snapshot.terminal_contexts[0].tabs.clear();
        if !grouped {
            snapshot.navigation.workspaces[1].project_id = None;
        }
        assert_eq!(
            snapshot.navigation.projects[0].label,
            snapshot.navigation.projects[1].label
        );
        let addressed = &snapshot.navigation.workspaces[1];
        let target = ClientContextLocation::Workspace {
            workspace_id: addressed.workspace_id.clone(),
            terminal_context_id: addressed.terminal_context_id.clone(),
        };
        let current = {
            let fixture_context_id = snapshot.navigation.workspaces[0]
                .terminal_context_id
                .clone();
            ClientContextLocation::Workspace {
                workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                    fixture_context_id.as_str(),
                )
                .unwrap(),
                terminal_context_id: fixture_context_id,
            }
        };
        let (mut ui, mailbox) = ready(&snapshot, &current);
        let original = Origin::capture(ui.state.as_ref());
        assert!(ui.open_action_menu(MenuTarget::WorkspaceProjects(target), None, false));
        let project = snapshot.navigation.projects[1].clone();
        let chosen = MenuAction::JoinWorkspaceProject(project.project_id.clone());
        let row = {
            let view = ui.view();
            let menu = view.action_menu().unwrap();
            let mut expected = Vec::new();
            if !grouped {
                expected.push(MenuAction::JoinWorkspaceProject(
                    snapshot.navigation.projects[0].project_id.clone(),
                ));
            }
            expected.push(chosen.clone());
            expected.push(MenuAction::CreateWorkspaceProject);
            if grouped {
                expected.push(MenuAction::LeaveWorkspaceProject);
            }
            assert_eq!(menu.actions, expected);
            let create = expected.len() - 1 - usize::from(grouped);
            for position in 0..expected.len() {
                assert_eq!(
                    menu.result_separator_before(menu.result_actions, position),
                    position == create,
                    "{grouped} {position}"
                );
            }
            let row = create - 1;
            for label in &menu.labels[..create] {
                assert_eq!(label.as_deref(), Some(project.label.as_str()));
            }
            row
        };
        let token = arm(&mut ui);
        activate_row(&mut ui, token, row);
        let action = ui.take_action_menu_action(false).unwrap();
        ui.apply_client_action(action);
        let lease = mailbox
            .try_recv()
            .unwrap()
            .expect("one addressed grouping command");
        let QueuedCommand::WorkspaceSetup(marker) = lease.record() else {
            panic!("exact grouping");
        };
        assert_eq!(
            marker.0.command.action,
            WorkbenchAction::WorkspaceSetProject {
                workspace_id: addressed.workspace_id.clone(),
                project_id: Some(project.project_id),
            }
        );
        assert_eq!(Origin::capture(ui.state.as_ref()), original);
        assert!(mailbox.try_recv().unwrap().is_none());
    }
}

#[test]
fn workspace_project_choice_reorder_retires_the_frozen_submenu_before_dispatch() {
    let mut snapshot = snapshot();
    snapshot.terminal_contexts[0].tabs.clear();
    let current = {
        let fixture_context_id = snapshot.navigation.workspaces[0]
            .terminal_context_id
            .clone();
        ClientContextLocation::Workspace {
            workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                fixture_context_id.as_str(),
            )
            .unwrap(),
            terminal_context_id: fixture_context_id,
        }
    };
    let (mut ui, mailbox) = ready(&snapshot, &current);
    let target = workspace(&snapshot, &snapshot.navigation.projects[0]);
    assert!(ui.open_action_menu(MenuTarget::WorkspaceProjects(target), None, false));
    let token = arm(&mut ui);
    let source = ui.view().action_menu().unwrap().source.clone();
    snapshot.navigation.project_order = snapshot
        .navigation
        .projects
        .iter()
        .rev()
        .map(|project| project.project_id.clone())
        .collect();
    snapshot.revision = WorkbenchRevision::new(snapshot.revision.value() + 1);
    install(&mut ui, &snapshot, &current);
    ui.activate_action_menu_row(&source, token, 1);
    assert!(ui.take_action_menu_action(false).is_none());
    assert!(!ui.action_menu_active());
    assert!(ui.view().setup_draft().is_none());
    assert!(mailbox.try_recv().unwrap().is_none());
}

fn open_create(ui: &mut WorkbenchClientUi, project: &Project) -> bool {
    ui.open_action_menu(
        MenuTarget::Drawn {
            source: ui.view().action_menu_source().unwrap(),
            target: Box::new(MenuTarget::Create(Some(project.project_id.clone()))),
        },
        None,
        false,
    )
}

fn confirm_quick(
    ui: &mut WorkbenchClientUi,
    project: &Project,
    keyboard: bool,
) -> crate::workbench_client::InputAction {
    assert!(open_create(ui, project));
    let action = confirm_open_action(ui, keyboard, MenuAction::QuickWorkspace);
    assert!(matches!(
        action.intent(),
        ClientAction::RequestProjectQuickWorkspace
    ));
    action
}

/// Confirms New Workspace ▸ New Workspace, which leads the submenu of a Project without folders.
fn confirm_creation_quick(
    ui: &mut WorkbenchClientUi,
    project: &Project,
) -> crate::workbench_client::InputAction {
    assert!(open(ui, project));
    open_creation(ui);
    let action = confirm_open_action(ui, false, MenuAction::QuickWorkspace);
    assert!(matches!(
        action.intent(),
        ClientAction::RequestProjectQuickWorkspace
    ));
    action
}

fn confirm_project_action(
    ui: &mut WorkbenchClientUi,
    project: &Project,
    keyboard: bool,
    requested: MenuAction,
) -> crate::workbench_client::InputAction {
    assert!(open(ui, project));
    confirm_open_action(ui, keyboard, requested)
}

fn confirm_open_action(
    ui: &mut WorkbenchClientUi,
    keyboard: bool,
    requested: MenuAction,
) -> crate::workbench_client::InputAction {
    let row = ui
        .view()
        .action_menu()
        .unwrap()
        .actions
        .iter()
        .position(|action| *action == requested)
        .unwrap();
    assert_eq!(ui.view().action_menu().unwrap().unavailable[row], None);
    if keyboard {
        arm(ui);
        for _ in 0..row {
            ui.capture_action_menu_key(KeyEvent::press(KeyCode::Down, Modifiers::NONE));
        }
        arm(ui);
        ui.capture_action_menu_key(KeyEvent::press(KeyCode::Enter, Modifiers::NONE));
    } else {
        let token = arm(ui);
        activate_row(ui, token, row);
    }
    let action = ui.take_action_menu_action(false).unwrap();
    assert_eq!(ui.take_action_menu_action(false), None);
    action
}

#[test]
fn project_quick_menu_keeps_exact_target_through_preparation_and_one_launch() {
    for (source, target, empty) in [
        (Some(0), 0, false),
        (Some(0), 1, false),
        (Some(0), 1, true),
        (None, 1, false),
        (None, 1, true),
    ] {
        // An empty Project's New Workspace ▸ leads with the same quick path as its [+] menu.
        for (keyboard, submenu) in [(false, false), (true, false), (false, true)] {
            if submenu && !empty {
                continue;
            }
            let mut snapshot = snapshot();
            snapshot.terminal_contexts[0].tabs.clear();
            let location = source.map_or_else(
                || {
                    let fixture_context_id = snapshot.navigation.workspaces[0]
                        .terminal_context_id
                        .clone();
                    ClientContextLocation::Workspace {
                        workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                            fixture_context_id.as_str(),
                        )
                        .unwrap(),
                        terminal_context_id: fixture_context_id,
                    }
                },
                |index| workspace(&snapshot, &snapshot.navigation.projects[index]),
            );
            if empty {
                let project_id = snapshot.navigation.projects[target].project_id.clone();
                empty_project(&mut snapshot, &project_id);
            }
            let project = snapshot.navigation.projects[target].clone();
            assert_eq!(project.label, snapshot.navigation.projects[0].label);
            let (mut ui, mailbox) = ready(&snapshot, &location);
            let clock = ui.clock.clone();
            let origin = Origin::capture(ui.state.as_ref());
            let cursors = mailbox.cursors();
            let action = if submenu {
                confirm_creation_quick(&mut ui, &project)
            } else {
                confirm_quick(&mut ui, &project, keyboard)
            };
            assert_eq!(Origin::capture(ui.state.as_ref()), origin);
            assert_eq!(mailbox.cursors(), cursors);
            assert!(mailbox.try_recv().unwrap().is_none());
            ui.apply_client_action(action);
            crate::workbench_client::ui::tests::workspace_setup::finish_preparation(
                &mut ui, &mailbox, &clock, &snapshot, false,
            );
            assert_eq!(Origin::capture(ui.state.as_ref()), origin);
            assert!(ui.view().setup_draft().is_none());
            let lease = mailbox.try_recv().unwrap().unwrap();
            let QueuedCommand::WorkspaceSetup(marker) = lease.record() else {
                panic!("one launch");
            };
            assert!(matches!(&marker.0.command.action,
                WorkbenchAction::WorkspaceLaunch { placement: orchbee_workbench_protocol::WorkspacePlacement::Project { project_id }, label, options, launch: None, profile: None, .. }
                if project_id == &project.project_id && label.as_str() == "项目 space"
                    && options.cwd.as_ref().unwrap().as_str() == "C:/项目 space"));
            assert_eq!(ui.state.as_ref().unwrap().cache().location(), &location);
            assert!(!ui.request_quick_workspace());
            assert!(mailbox.try_recv().unwrap().is_none());
            mailbox.close(CarrierFailure::WorkerExited);
        }
    }
}

#[test]
fn project_quick_cancel_menu_or_preparation_never_creates_or_navigates() {
    for during_preparation in [false, true] {
        let mut snapshot = snapshot();
        let project_id = snapshot.navigation.projects[1].project_id.clone();
        empty_project(&mut snapshot, &project_id);
        let location = workspace(&snapshot, &snapshot.navigation.projects[0]);
        let (mut ui, mailbox) = ready(&snapshot, &location);
        let clock = ui.clock.clone();
        let origin = Origin::capture(ui.state.as_ref());
        if during_preparation {
            let action = confirm_quick(&mut ui, &snapshot.navigation.projects[1], false);
            ui.apply_client_action(action);
            crate::workbench_client::ui::tests::workspace_setup::finish_preparation(
                &mut ui, &mailbox, &clock, &snapshot, true,
            );
        } else {
            assert!(open(&mut ui, &snapshot.navigation.projects[1]));
            ui.capture_action_menu_key(KeyEvent::press(KeyCode::Esc, Modifiers::NONE));
            assert!(!ui.action_menu_active());
            assert_eq!(ui.take_action_menu_action(false), None);
        }
        assert!(ui.view().setup_draft().is_none());
        assert_eq!(Origin::capture(ui.state.as_ref()), origin);
        assert!(mailbox.try_recv().unwrap().is_none());
    }
}

#[test]
fn project_quick_handoff_expiry_refuses_and_accepted_launch_keeps_its_budget() {
    use std::sync::atomic::Ordering;
    for expired_before_acceptance in [false, true] {
        let snapshot = snapshot();
        let location = workspace(&snapshot, &snapshot.navigation.projects[0]);
        let (mut ui, mailbox) = ready(&snapshot, &location);
        let elapsed = Arc::new(AtomicU64::new(0));
        ui.clock = adjustable_operation_clock(Arc::clone(&elapsed));
        let clock = ui.clock.clone();
        let action = confirm_quick(&mut ui, &snapshot.navigation.projects[1], true);
        if expired_before_acceptance {
            elapsed.store(30_000, Ordering::Relaxed);
            ui.apply_client_action(action);
            assert!(mailbox.try_recv().unwrap().is_none());
            assert!(ui.view().setup_draft().is_none());
        } else {
            ui.apply_client_action(action);
            crate::workbench_client::ui::tests::workspace_setup::finish_preparation(
                &mut ui, &mailbox, &clock, &snapshot, false,
            );
            assert!(ui.view().setup_draft().is_none());
            let lease = mailbox.try_recv().unwrap().unwrap();
            let QueuedCommand::WorkspaceSetup(marker) = lease.record() else {
                panic!("launch after the accepted review");
            };
            assert!(marker.0.deadlines.forward_cutoff() > clock.now());
            mailbox.close(CarrierFailure::WorkerExited);
        }
    }
}

#[test]
fn project_quick_menu_rejects_noncurrent_target_replacement_even_after_taking_the_action() {
    for stage in 0..3 {
        for removed in [false, true] {
            let mut snapshot = snapshot();
            let location = workspace(&snapshot, &snapshot.navigation.projects[0]);
            let target = snapshot.navigation.projects[1].clone();
            let (mut ui, mailbox) = ready(&snapshot, &location);
            assert!(open_create(&mut ui, &target));
            let row = ui
                .view()
                .action_menu()
                .unwrap()
                .actions
                .iter()
                .position(|action| *action == MenuAction::QuickWorkspace)
                .unwrap();
            let token = arm(&mut ui);
            if stage > 0 {
                activate_row(&mut ui, token, row);
            }
            let action = (stage == 2).then(|| ui.take_action_menu_action(false).unwrap());
            if removed {
                let project_id = snapshot
                    .navigation
                    .projects
                    .last()
                    .unwrap()
                    .project_id
                    .clone();
                dissolve_project(&mut snapshot, &project_id);
            } else {
                snapshot.navigation.projects[1].label = "Replacement".into();
            }
            install(&mut ui, &snapshot, &location);
            if let Some(action) = action {
                ui.apply_client_action(action);
            } else {
                if stage == 0 {
                    activate_row(&mut ui, token, row);
                }
                assert_eq!(ui.take_action_menu_action(false), None);
            }
            assert!(!ui.action_menu_active());
            assert!(ui.view().setup_draft().is_none());
            assert_eq!(ui.state.as_ref().unwrap().cache().location(), &location);
            assert!(mailbox.try_recv().unwrap().is_none());
        }
    }
}

#[test]
fn project_menu_requires_addressed_git_proof_even_when_its_workspace_is_current() {
    for current in 0..2 {
        let snapshot = snapshot();
        let project = &snapshot.navigation.projects[current];
        let location = workspace(&snapshot, project);
        let (mut ui, mailbox) = ready(&snapshot, &location);
        let origin = Origin::capture(ui.state.as_ref());
        let cursors = mailbox.cursors();
        assert!(open(&mut ui, project));
        open_creation(&mut ui);
        assert_eq!(ui.take_action_menu_action(false), None);
        assert!(matches!(ui.menus.intent().unwrap().stage(), Stage::Open));
        assert_eq!(Origin::capture(ui.state.as_ref()), origin);
        assert_eq!(mailbox.cursors(), cursors);
        assert!(mailbox.try_recv().unwrap().is_none());
        ui.cancel_action_menu();
        assert_eq!(Origin::capture(ui.state.as_ref()), origin);
    }
}

#[test]
fn project_menu_empty_and_noncurrent_targets_do_not_invent_a_worktree_or_change_focus() {
    for initial_workspace in [true, false] {
        for empty in [true, false] {
            let mut snapshot = snapshot();
            snapshot.terminal_contexts[0].tabs.clear();
            let target = usize::from(!initial_workspace);
            let location = if initial_workspace {
                {
                    let fixture_context_id = snapshot.navigation.workspaces[0]
                        .terminal_context_id
                        .clone();
                    ClientContextLocation::Workspace {
                        workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                            fixture_context_id.as_str(),
                        )
                        .unwrap(),
                        terminal_context_id: fixture_context_id,
                    }
                }
            } else {
                workspace(&snapshot, &snapshot.navigation.projects[0])
            };
            if empty {
                let project = snapshot.navigation.projects[target].project_id.clone();
                empty_project(&mut snapshot, &project);
            }
            let (mut ui, mailbox) = ready(&snapshot, &location);
            let origin = Origin::capture(ui.state.as_ref());
            let cursors = mailbox.cursors();
            assert!(open(&mut ui, &snapshot.navigation.projects[target]));
            open_creation(&mut ui);
            assert_eq!(ui.take_action_menu_action(false), None);
            assert!(matches!(ui.menus.intent().unwrap().stage(), Stage::Open));
            assert_eq!(Origin::capture(ui.state.as_ref()), origin);
            assert_eq!(mailbox.cursors(), cursors);
            assert!(mailbox.try_recv().unwrap().is_none());
            ui.cancel_action_menu();
            assert_eq!(Origin::capture(ui.state.as_ref()), origin);
        }
    }
}

#[test]
fn project_menu_rejects_stale_drawn_and_replaced_sources_before_confirmation_or_handoff() {
    let (mut ui, _, _, _) = tests::context_selection::ready(true);
    let original = snapshot();
    let project = &original.navigation.projects[0];
    install(&mut ui, &original, &workspace(&original, project));
    for wrong_session in [false, true] {
        let mut source = ui.view().action_menu_source().unwrap();
        if wrong_session {
            source.session = SessionId::parse(&format!("{:032x}", 999)).unwrap();
        } else {
            source.revision = WorkbenchRevision::new(source.revision.value() + 1);
        }
        assert!(!ui.open_action_menu(
            MenuTarget::Drawn {
                source,
                target: Box::new(MenuTarget::Project(project.project_id.clone())),
            },
            None,
            false,
        ));
    }
    assert!(!ui.open_action_menu(
        MenuTarget::Project(WorkbenchProjectId::parse(&format!("{:032x}", 999)).unwrap()),
        None,
        false,
    ));
    for confirmed in [false, true] {
        for change in 0..6 {
            let (mut ui, mailbox, _, _) = tests::context_selection::ready(true);
            install(&mut ui, &original, &workspace(&original, project));
            assert!(open(&mut ui, project));
            let row = ui
                .view()
                .action_menu()
                .unwrap()
                .actions
                .iter()
                .position(|action| *action == MenuAction::RenameProject)
                .unwrap();
            let token = arm(&mut ui);
            if confirmed {
                activate_row(&mut ui, token, row);
                assert!(matches!(
                    ui.menus.intent().unwrap().stage(),
                    Stage::Offered(_, _)
                ));
            }
            let mut replacement = original.clone();
            replacement.revision = WorkbenchRevision::new(original.revision.value() + 1);
            let mut location = workspace(&original, project);
            match change {
                0 => replacement.navigation.projects[0].label = "Renamed".into(),
                1 => {
                    replacement.navigation.directories[0].path =
                        WorkbenchDirectoryPath::parse("C:/replacement").unwrap();
                }
                2 => {
                    replacement.navigation.directories[0].worker_id =
                        WorkerId::parse(&format!("{:032x}", 903)).unwrap();
                }
                3 => {
                    let id = WorkbenchTerminalContextId::parse(&format!("{:032x}", 904)).unwrap();
                    replacement.terminal_contexts[1].terminal_context_id = id.clone();
                    replacement.navigation.workspaces[1].terminal_context_id = id;
                    location = workspace(&replacement, &replacement.navigation.projects[0]);
                }
                4 => {
                    dissolve_project(&mut replacement, &project.project_id);
                    location = {
                        let fixture_context_id = replacement.navigation.workspaces[0]
                            .terminal_context_id
                            .clone();
                        ClientContextLocation::Workspace {
                            workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                                fixture_context_id.as_str(),
                            )
                            .unwrap(),
                            terminal_context_id: fixture_context_id,
                        }
                    };
                }
                _ => location = workspace(&replacement, &replacement.navigation.projects[1]),
            }
            install(&mut ui, &replacement, &location);
            let origin = Origin::capture(ui.state.as_ref());
            if !confirmed {
                activate_row(&mut ui, token, row);
            }
            assert_eq!(ui.take_action_menu_action(false), None);
            assert!(
                !ui.action_menu_active(),
                "confirmed={confirmed} change={change}"
            );
            assert_eq!(Origin::capture(ui.state.as_ref()), origin);
            assert!(mailbox.try_recv().unwrap().is_none());
        }
    }
}

#[test]
fn project_menu_disabled_reason_renders_in_both_locales_at_small_geometry() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [Theme::terminal_native(), Theme::rgb_test_dark()] {
            for (width, height) in [
                (120, 40),
                (120, 30),
                (80, 24),
                (80, 20),
                (50, 10),
                (20, 7),
                (12, 3),
            ] {
                let (mut ui, _, _, _) = tests::context_selection::ready(true);
                let mut snapshot = snapshot();
                snapshot.terminal_contexts[0].tabs.clear();
                let location = {
                    let fixture_context_id = snapshot.navigation.workspaces[0]
                        .terminal_context_id
                        .clone();
                    ClientContextLocation::Workspace {
                        workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                            fixture_context_id.as_str(),
                        )
                        .unwrap(),
                        terminal_context_id: fixture_context_id,
                    }
                };
                install(&mut ui, &snapshot, &location);
                ui.observe_geometry(Geometry::new(width, height, 8, 16).unwrap());
                assert!(open(&mut ui, &snapshot.navigation.projects[0]));
                assert!(ui.state.as_mut().unwrap().start_pending_refresh(1));
                ui.drive_action_menu();
                let selected_row = action_row(&ui, MenuAction::DeleteProject);
                let action_count = ui.view().action_menu().unwrap().actions.len();
                ui.menus.menu.select_action(MenuAction::DeleteProject);
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                let mut receipt = DrawReceipt::default();
                let mut hits = HitMapBuilder::new();
                terminal
                    .draw(|frame| {
                        receipt = crate::tui_render::render_terminal_shell(
                            frame,
                            &crate::client_view::ClientView::new(width),
                            locale,
                            Some(ui.view()),
                            &theme,
                            &mut hits,
                        );
                    })
                    .unwrap();
                ui.after_close_draw(receipt);
                if width == 12 {
                    assert!(!ui.action_menu_active());
                    continue;
                }
                assert!(ui.action_menu_active());
                // Wide symbols own their continuation cells; retain actual
                // spaces without inserting the TestBackend's blank slots.
                let text: String = terminal
                    .backend()
                    .buffer()
                    .content()
                    .chunks(usize::from(width))
                    .map(|row| {
                        let mut text = String::new();
                        let mut column = 0;
                        while let Some(cell) = row.get(column) {
                            text.push_str(cell.symbol());
                            column += crate::terminal_text::display_width(cell.symbol()).max(1);
                        }
                        text.push('\n');
                        text
                    })
                    .collect();
                let reasons = [false, true].map(|compact| {
                    locale.menu_unavailable(MenuUnavailable::OperationBusy, compact)
                });
                assert!(
                    reasons
                        .iter()
                        .any(|reason| text.replace(' ', "").contains(&reason.replace(' ', ""))),
                    "{locale:?} {width}x{height}, selected={selected_row}, unavailable={:?}:\n{text}",
                    ui.view().action_menu().unwrap().unavailable[selected_row]
                );
                assert!(!text.contains("Esc"), "context menu has no external footer");
                assert!(!text.contains(&format!("{}/{}", selected_row + 1, action_count)));
                assert!(!text.contains("Enter"));
                if width >= 80 {
                    assert!(
                        text.contains(&locale.menu_object_action(&MenuAction::DeleteProject, None))
                    );
                }
                if width == 20 {
                    let hits = hits.finish();
                    let row = (0..height)
                        .find(|y| {
                            (0..width).any(|x| {
                                matches!(
                                    hits.hit_target_at(x, *y),
                                    Some(crate::tui_hit_map::HitTarget::ActionMenuRow {
                                        row,
                                        ..
                                    }) if *row == selected_row
                                )
                            })
                        })
                        .unwrap();
                    let selected = text.lines().nth(usize::from(row)).unwrap();
                    assert!(
                        selected
                            .contains(&locale.menu_object_action(&MenuAction::DeleteProject, None))
                            || selected.contains(theme.glyphs.ellipsis)
                    );
                }
            }
        }
    }
}

#[test]
fn project_rename_menu_opens_exact_target_without_navigation_and_submits_once_after_draw() {
    for (source, target, empty) in [
        (Some(0), 0, false),
        (Some(0), 1, false),
        (Some(0), 1, true),
        (None, 1, true),
    ] {
        for keyboard in [false, true] {
            for cancel in [false, true] {
                let mut snapshot = snapshot();
                snapshot.terminal_contexts[0].tabs.clear();
                let location = source.map_or_else(
                    || {
                        let fixture_context_id = snapshot.navigation.workspaces[0]
                            .terminal_context_id
                            .clone();
                        ClientContextLocation::Workspace {
                            workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                                fixture_context_id.as_str(),
                            )
                            .unwrap(),
                            terminal_context_id: fixture_context_id,
                        }
                    },
                    |index| workspace(&snapshot, &snapshot.navigation.projects[index]),
                );
                if empty {
                    let project_id = snapshot.navigation.projects[target].project_id.clone();
                    empty_project(&mut snapshot, &project_id);
                }
                let project = snapshot.navigation.projects[target].clone();
                let (mut ui, mailbox) = ready(&snapshot, &location);
                let origin = Origin::capture(ui.state.as_ref());
                let cursors = mailbox.cursors();
                let action =
                    confirm_project_action(&mut ui, &project, keyboard, MenuAction::RenameProject);
                assert!(matches!(
                    action.intent(),
                    ClientAction::RequestTargetedProjectRename
                ));
                ui.apply_client_action(action);
                let draft = ui.view().setup_draft().unwrap();
                let token = draft.token();
                assert_eq!(draft.stage(), SetupStage::ProjectRename);
                assert_eq!(draft.text(), project.label);
                assert_eq!(draft.project_label(), Some(project.label.as_str()));
                assert_eq!(Origin::capture(ui.state.as_ref()), origin);
                assert_eq!(mailbox.cursors(), cursors);
                if cancel {
                    ui.cancel_setup_draft(token);
                    assert!(ui.view().setup_draft().is_none());
                    assert!(!ui.state.as_ref().unwrap().setup_blocks_structure());
                    assert_eq!(mailbox.cursors(), cursors);
                } else {
                    ui.submit_setup_draft(token);
                    assert!(mailbox.try_recv().unwrap().is_none());
                    ui.after_setup_draw(Some(ModalPresentation::Presented(token)));
                    ui.edit_setup_draft(token, SetupEdit::Clear);
                    ui.edit_setup_draft(token, SetupEdit::Append("Exact 雪"));
                    ui.submit_setup_draft(token);
                    let lease = mailbox.try_recv().unwrap().unwrap();
                    let QueuedCommand::WorkspaceSetup(marker) = lease.record() else {
                        panic!("one rename");
                    };
                    assert!(
                        matches!(&marker.0.command.action, WorkbenchAction::ProjectRename { project_id, label } if project_id == &project.project_id && label.as_str() == "Exact 雪")
                    );
                    ui.submit_setup_draft(token);
                }
                assert!(mailbox.try_recv().unwrap().is_none());
                assert_eq!(Origin::capture(ui.state.as_ref()), origin);
                mailbox.close(CarrierFailure::WorkerExited);
            }
        }
    }
}

#[test]
fn project_rename_handoff_expires_or_rejects_replacement_but_editor_does_not_expire() {
    use std::sync::atomic::Ordering;
    for refusal in ["none", "expired", "replaced", "removed"] {
        let mut snapshot = snapshot();
        snapshot.navigation.projects[1].label = "Requested Project".into();
        let location = workspace(&snapshot, &snapshot.navigation.projects[0]);
        let (mut ui, mailbox) = ready(&snapshot, &location);
        let elapsed = Arc::new(AtomicU64::new(0));
        ui.clock = adjustable_operation_clock(Arc::clone(&elapsed));
        let action = confirm_project_action(
            &mut ui,
            &snapshot.navigation.projects[1],
            true,
            MenuAction::RenameProject,
        );
        if refusal == "expired" {
            elapsed.store(30_000, Ordering::Relaxed);
        }
        if refusal == "replaced" || refusal == "removed" {
            if refusal == "removed" {
                let project_id = snapshot
                    .navigation
                    .projects
                    .last()
                    .unwrap()
                    .project_id
                    .clone();
                dissolve_project(&mut snapshot, &project_id);
            } else {
                snapshot.navigation.projects[1].label = "Replacement".into();
            }
            install(&mut ui, &snapshot, &location);
        }
        let cursors = mailbox.cursors();
        ui.apply_client_action(action);
        assert_eq!(mailbox.cursors(), cursors);
        if refusal == "none" {
            assert_eq!(
                ui.view().setup_draft().unwrap().project_label(),
                Some("Requested Project")
            );
            assert_eq!(ui.view().setup_draft().unwrap().text(), "Requested Project");
            elapsed.store(120_000, Ordering::Relaxed);
            let token = ui.view().setup_draft().unwrap().token();
            ui.after_setup_draw(Some(ModalPresentation::Presented(token)));
            ui.submit_setup_draft(token);
            let lease = mailbox.try_recv().unwrap().unwrap();
            let QueuedCommand::WorkspaceSetup(marker) = lease.record() else {
                panic!("rename after untimed editing");
            };
            assert!(marker.0.deadlines.forward_cutoff() > ui.clock.now());
            mailbox.close(CarrierFailure::WorkerExited);
        } else {
            assert!(ui.view().setup_draft().is_none());
            assert!(!ui.state.as_ref().unwrap().setup_blocks_structure());
            assert!(mailbox.try_recv().unwrap().is_none());
        }
        assert_eq!(ui.state.as_ref().unwrap().cache().location(), &location);
    }
}

struct RenameInput<'a> {
    ui: &'a mut WorkbenchClientUi,
    expire: Option<Arc<AtomicU64>>,
    forwarded: Vec<u8>,
}

impl crate::terminal_input::TerminalInputPort for RenameInput<'_> {
    fn native_capture_claims(&self) -> crate::tui_composition::CaptureClaims {
        self.ui.capture_claims()
    }
    fn native_capture_context(
        &self,
        owner: crate::tui_composition::WorkbenchCaptureOwner,
    ) -> Option<crate::workbench_client::CaptureContext> {
        self.ui.capture_context(owner)
    }
    fn forward(
        &mut self,
        bytes: &[u8],
    ) -> Result<(), crate::terminal_input::TerminalInputUnavailable> {
        self.forwarded.extend_from_slice(bytes);
        Ok(())
    }
    fn surface_input(&mut self, input: crate::terminal_input::SurfaceInput<'_>) -> bool {
        use crate::terminal_input::SurfaceInput;
        match input {
            SurfaceInput::ActionMenu { .. } | SurfaceInput::Setup { .. } => {
                self.ui.surface_input(input)
            }
            _ => false,
        }
    }
    fn take_action_menu_action(
        &mut self,
        released: bool,
    ) -> Option<crate::workbench_client::InputAction> {
        self.ui.take_action_menu_action(released)
    }
    fn apply_client_action(
        &mut self,
        command: crate::workbench_client::InputAction,
    ) -> Option<ClientActionDisposition> {
        assert!(matches!(
            command.intent(),
            ClientAction::RequestTargetedProjectRename
        ));
        if let Some(elapsed) = &self.expire {
            elapsed.store(30_000, std::sync::atomic::Ordering::Relaxed);
        }
        Some(self.ui.apply_client_action(command))
    }
    fn surface_action(
        &mut self,
        action: crate::terminal_input::SurfaceAction<'_>,
    ) -> Option<crate::client_view::ClientActionDisposition> {
        use crate::terminal_input::{SetupAction, SurfaceAction};
        match action {
            SurfaceAction::Setup(SetupAction::Cancel(_))
            | SurfaceAction::Setup(SetupAction::Submit(_)) => self.ui.surface_action(action),
            _ => None,
        }
    }
}

#[test]
fn project_rename_dispatcher_captures_fragmented_handoff_and_paste_tail_even_when_refused() {
    for refused in [false, true] {
        let snapshot = snapshot();
        let location = workspace(&snapshot, &snapshot.navigation.projects[0]);
        let (mut ui, mailbox) = ready(&snapshot, &location);
        let elapsed = Arc::new(AtomicU64::new(0));
        ui.clock = adjustable_operation_clock(Arc::clone(&elapsed));
        assert!(open(&mut ui, &snapshot.navigation.projects[1]));
        let row = ui
            .view()
            .action_menu()
            .unwrap()
            .actions
            .iter()
            .position(|a| *a == MenuAction::RenameProject)
            .unwrap();
        arm(&mut ui);
        for _ in 0..row {
            ui.capture_action_menu_key(KeyEvent::press(KeyCode::Down, Modifiers::NONE));
        }
        arm(&mut ui);
        let mut input = RenameInput {
            ui: &mut ui,
            expire: refused.then_some(elapsed),
            forwarded: Vec::new(),
        };
        let mut dispatcher = crate::terminal_input::InputDispatcher::default();
        let mut view = crate::client_view::ClientView::new(80);
        let mut now = Instant::now();
        let mut modes = Vec::new();
        for bytes in [b"\x1b[13".as_slice(), b"uignored\x1b[200~paste\n\x1b[201~"] {
            dispatcher
                .dispatch(
                    &mut view,
                    bytes,
                    &mut input,
                    now,
                    |_, _| Ok::<bool, std::io::Error>(false),
                    &mut |mode| {
                        modes.push(mode);
                        Ok(())
                    },
                )
                .unwrap();
        }
        assert!(modes.contains(&crate::terminal_input::ModeRequest::BracketedPaste(true)));
        assert!(input.forwarded.is_empty());
        assert!(!input.ui.action_menu_active());
        assert_eq!(input.ui.view().setup_draft().is_some(), !refused);
        dispatcher.publish_hit_map(HitMapBuilder::new().finish());
        if !refused {
            assert_eq!(
                input.ui.view().setup_draft().unwrap().text(),
                snapshot.navigation.projects[1].label
            );
            dispatcher
                .dispatch(
                    &mut view,
                    b"\rignored",
                    &mut input,
                    now,
                    |_, _| Ok::<bool, std::io::Error>(false),
                    &mut |_| Ok(()),
                )
                .unwrap();
            assert!(mailbox.try_recv().unwrap().is_none());
            dispatcher.publish_hit_map(HitMapBuilder::new().finish());
            let token = input.ui.view().setup_draft().unwrap().token();
            input
                .ui
                .after_setup_draw(Some(ModalPresentation::Presented(token)));
            dispatcher
                .dispatch(
                    &mut view,
                    b"\x1b",
                    &mut input,
                    now,
                    |_, _| Ok::<bool, std::io::Error>(false),
                    &mut |_| Ok(()),
                )
                .unwrap();
            assert!(input.ui.view().setup_draft().is_some());
            now += Duration::from_millis(50);
            dispatcher
                .synchronize(&mut view, &mut input, now, &mut |_| {
                    Ok::<(), std::io::Error>(())
                })
                .unwrap();
            assert!(input.ui.view().setup_draft().is_none());
        }
        assert!(!input.ui.state.as_ref().unwrap().setup_blocks_structure());
        assert!(input.forwarded.is_empty());
        dispatcher.publish_hit_map(HitMapBuilder::new().finish());
        dispatcher
            .dispatch(
                &mut view,
                b"terminal",
                &mut input,
                now + std::time::Duration::from_millis(52),
                |_, _| Ok::<bool, std::io::Error>(false),
                &mut |_| Ok(()),
            )
            .unwrap();
        assert_eq!(input.forwarded, b"terminal");
        assert!(mailbox.try_recv().unwrap().is_none());
    }
}

#[test]
fn project_quick_return_consumes_once_and_preserves_newer_surface_intent() {
    use crate::client_view::{ClientView, Overlay, SidebarSurface};
    fn apply(view: &mut ClientView, action: ClientAction) {
        let transition = view.decide_action(action);
        view.commit(transition, ClientActionDisposition::KeepNavigator);
    }

    let snapshot = snapshot();
    let location = workspace(&snapshot, &snapshot.navigation.projects[0]);
    let (mut ui, mailbox) = ready(&snapshot, &location);
    let action = confirm_quick(&mut ui, &snapshot.navigation.projects[1], true);
    for newer in [
        "none",
        "hide_reopen",
        "help_return",
        "refused_focus",
        "pointer",
        "machine",
    ] {
        let mut view = ClientView::new(80);
        apply(&mut view, ClientAction::ToggleBusinessSidebar);
        apply(&mut view, action.intent());
        assert!(view.project_quick_entry_pending());
        match newer {
            "hide_reopen" => {
                apply(&mut view, ClientAction::ToggleBusinessSidebar);
                apply(&mut view, ClientAction::ToggleBusinessSidebar);
            }
            "help_return" => {
                apply(&mut view, ClientAction::OpenShortcutHelp);
                apply(&mut view, ClientAction::ToggleBusinessSidebar);
            }
            "refused_focus" => {
                let focus = ClientAction::FocusPanel(
                    orchbee_workbench_protocol::WorkbenchPanelId::parse(&format!("{:032x}", 9_999))
                        .unwrap(),
                );
                apply(&mut view, focus.clone());
                ui.apply_client_action(focus);
            }
            "pointer" => view.commit_pointer(ClientActionDisposition::KeepNavigator),
            "machine" => {
                // Pointer machine-list and Agent routes dismiss the overlay first.
                view.dismiss_overlay();
                apply(&mut view, ClientAction::ToggleBusinessSidebar);
            }
            _ => {}
        }
        assert_eq!(view.overlay(), Some(Overlay::BusinessSidebar));
        assert_eq!(view.project_quick_entry_pending(), newer == "none");
        // The same checks apply when Ready was queued before the new input.
        assert_eq!(view.complete_project_quick_entry(), newer == "none");
        assert!(!view.project_quick_entry_pending());
        if newer == "none" {
            assert_eq!(view.sidebar_surface(), SidebarSurface::Hidden);
            assert!(!view.machines.sidebar_navigation_active());
            apply(&mut view, ClientAction::ToggleBusinessSidebar);
        }
        assert!(!view.complete_project_quick_entry());
        assert_eq!(view.overlay(), Some(Overlay::BusinessSidebar));
    }
    for surface in ["wide", "hidden", "help", "navigator"] {
        let mut view = ClientView::new(if surface == "wide" { 160 } else { 80 });
        if surface == "help" || surface == "navigator" {
            apply(&mut view, ClientAction::ToggleBusinessSidebar);
        }
        apply(&mut view, action.intent());
        if surface == "help" {
            apply(&mut view, ClientAction::OpenShortcutHelp);
        } else if surface == "navigator" {
            apply(&mut view, ClientAction::OpenWorkbenchNavigator);
        }
        let overlay = view.overlay();
        let sidebar = view.sidebar_surface();
        assert!(!view.complete_project_quick_entry());
        assert_eq!(view.overlay(), overlay);
        assert_eq!(view.sidebar_surface(), sidebar);
        assert!(!view.project_quick_entry_pending());
        view.resize(80);
        apply(&mut view, ClientAction::ToggleBusinessSidebar);
        assert!(!view.complete_project_quick_entry());
        assert_eq!(view.overlay(), Some(Overlay::BusinessSidebar));
    }
    assert!(mailbox.try_recv().unwrap().is_none());
}
