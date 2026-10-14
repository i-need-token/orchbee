use super::*;
use crate::terminal_session::NativeSetupStage;
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
fn moving_noncurrent_workspace_opens_organization_without_navigating_or_mutating() {
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
        MenuTarget::Workspace(target),
        MenuAction::MoveWorkspace,
    );
    assert!(matches!(
        action,
        ClientAction::RequestWorkspaceOrganization(_)
    ));
    assert!(mailbox.try_recv().unwrap().is_none());
    ui.apply_client_action(action);
    assert_eq!(
        ui.view().setup_draft().unwrap().stage(),
        NativeSetupStage::ChooseWorkspaceProject
    );
    assert_eq!(ui.state.as_ref().unwrap().cache().location(), &original);
    assert_eq!(ui.state.as_ref().unwrap().desired(), selected.as_ref());
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn closing_noncurrent_workspace_opens_exact_confirmation_without_navigation() {
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
        MenuAction::CloseWorkspace,
    );
    assert_eq!(action, ClientAction::RequestWorkspaceCloseAt(expected));
    assert!(mailbox.try_recv().unwrap().is_none());
    ui.apply_client_action(action);
    assert!(ui.view().workspace_close_confirmation().is_some());
    assert_eq!(ui.state.as_ref().unwrap().cache().location(), &original);
    assert_eq!(ui.state.as_ref().unwrap().desired(), selected.as_ref());
    assert!(mailbox.try_recv().unwrap().is_none());
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
        MenuAction::DissolveProject,
    );
    ui.apply_client_action(action);
    let lease = mailbox.try_recv().unwrap().expect("one metadata command");
    let QueuedCommand::WorkspaceSetup(marker) = lease.record() else {
        panic!("Project dissolve")
    };
    assert_eq!(
        marker.0.command.action,
        WorkbenchAction::ProjectDissolve {
            project_id: project
        }
    );
    assert_eq!(ui.state.as_ref().unwrap().cache().location(), &original);
    assert_eq!(ui.state.as_ref().unwrap().desired(), selected.as_ref());
}
