use super::super::Origin;
use super::*;
use crate::action_menu::{ActionMenu, MenuDraw, MenuToken};
use crate::client_view::{KeyCode, KeyEvent, Modifiers};
use crate::terminal_session::{ModalPresentation, ui::tests::context_selection};
use orchbee_workbench_protocol::WorkbenchDirectoryPath;

fn draw(ui: &mut NativeUi) -> MenuToken {
    let view = ui.view();
    let menu = view.action_menu().unwrap();
    let draw = MenuDraw {
        token: menu.token,
        selected: menu.selected,
        first: 0,
        visible_rows: menu.actions.len(),
        result_actions: menu.result_actions.to_vec(),
    };
    let token = draw.token;
    ui.after_action_menu_draw(Some(ModalPresentation::Presented(draw)));
    token
}

fn ready_discovery(ui: &NativeUi) -> SidebarDiscovery {
    let addressed = ui.menus.sidebar.as_ref().unwrap().source.clone();
    let mut discovery = crate::terminal_session::state::worktree_launch::tests::discovery();
    discovery.worker = addressed.primary.as_ref().unwrap().worker_id().clone();
    let result = SidebarDiscovery {
        addressed,
        sample: crate::terminal_session::primary_directory::RootDirectory::Available(Some(
            WorkbenchDirectoryPath::parse("C:/repo").unwrap(),
        )),
        source: orchbee_workbench_protocol::WorktreeSource::Directory {
            path: WorkbenchDirectoryPath::parse("C:/repo").unwrap(),
        },
        discovery,
    };
    assert!(result.valid(ui.state.as_ref().unwrap().cache().canonical_snapshot()));
    result
}

fn project_target(ui: &NativeUi) -> MenuTarget {
    MenuTarget::Project(
        ui.state
            .as_ref()
            .unwrap()
            .cache()
            .canonical_snapshot()
            .navigation
            .projects[0]
            .project_id
            .clone(),
    )
}

#[test]
fn async_git_availability_preserves_action_slots_and_requires_a_fresh_draw_without_navigation() {
    let (mut ui, mailbox, _, _) = context_selection::ready(false);
    let original = Origin::capture(ui.state.as_ref());
    let target = project_target(&ui);
    assert!(ui.open_action_menu(target, Some((8, 5)), false));
    assert!(
        ui.view()
            .action_menu()
            .unwrap()
            .actions
            .contains(&MenuAction::WorkspaceWorktrees)
    );
    let before = ui.view().action_menu().unwrap().actions.to_vec();
    let worktrees = before
        .iter()
        .position(|action| action == &MenuAction::WorkspaceWorktrees)
        .unwrap();
    assert_eq!(
        ui.view().action_menu().unwrap().unavailable[worktrees],
        Some(crate::action_menu::MenuUnavailable::WorktreeChecking)
    );
    let old = draw(&mut ui);
    assert!(!ui.drive_action_menu());
    assert_eq!(ui.view().action_menu().unwrap().token, old);
    assert!(ui.action_menu_input_context().unwrap().activation_ready());
    let episode = ui.menus.sidebar.as_mut().unwrap();
    episode.state = DiscoveryState::Querying;
    let id = episode.id;
    let discovery = ready_discovery(&ui);
    ui.settle_sidebar_worktrees(id, Ok(Box::new(discovery)));
    let (source, current, row) = {
        let view = ui.view();
        let menu = view.action_menu().unwrap();
        (
            menu.source.clone(),
            menu.token,
            menu.actions
                .iter()
                .position(|action| action == &MenuAction::WorkspaceWorktrees)
                .unwrap(),
        )
    };
    assert_ne!(current, old);
    assert_eq!(ui.view().action_menu().unwrap().actions, before);
    assert!(!ui.action_menu_input_context().unwrap().activation_ready());
    ui.activate_action_menu_row(&source, old, row);
    assert!(ui.take_action_menu_action(false).is_none());
    assert!(ui.menus.menu.confirmed().is_none());
    let current = draw(&mut ui);
    ui.activate_action_menu_row(&source, current, row);
    assert!(
        ui.take_action_menu_action(false).is_none(),
        "submenu has no product effect"
    );
    let view = ui.view();
    let menu = view.action_menu().unwrap();
    assert!(matches!(menu.source, MenuTarget::Worktrees(_)));
    assert_eq!(
        menu.actions,
        &[
            MenuAction::NewWorktree,
            MenuAction::OpenWorktreeAt(WorkbenchDirectoryPath::parse("C:/repo").unwrap()),
            MenuAction::OpenWorktreeAt(WorkbenchDirectoryPath::parse("C:/trees/猫").unwrap()),
        ]
    );
    assert!(!menu.separator_before(0));
    assert!(menu.separator_before(1));
    assert!(!menu.separator_before(2));
    assert_eq!(Origin::capture(ui.state.as_ref()), original);
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn cancelled_or_replaced_episode_cannot_reopen_or_populate_a_new_menu() {
    let (mut ui, mailbox, _, _) = context_selection::ready(false);
    let target = project_target(&ui);
    assert!(ui.open_action_menu(target.clone(), None, false));
    ui.menus.sidebar.as_mut().unwrap().state = DiscoveryState::Querying;
    let cancelled_id = ui.menus.sidebar.as_ref().unwrap().id;
    let stale = ready_discovery(&ui);
    ui.capture_action_menu_key(KeyEvent::press(KeyCode::Esc, Modifiers::NONE));
    assert!(!ui.action_menu_active());
    assert!(ui.menus.sidebar.is_none());
    ui.settle_sidebar_worktrees(cancelled_id, Ok(Box::new(stale.clone())));
    assert!(!ui.action_menu_active());
    assert!(ui.open_action_menu(target, None, false));
    let token = ui.view().action_menu().unwrap().token;
    assert_ne!(ui.menus.sidebar.as_ref().unwrap().id, cancelled_id);
    ui.menus.sidebar.as_mut().unwrap().state = DiscoveryState::Querying;
    ui.settle_sidebar_worktrees(cancelled_id, Ok(Box::new(stale)));
    assert_eq!(ui.view().action_menu().unwrap().token, token);
    assert!(
        ui.view()
            .action_menu()
            .unwrap()
            .actions
            .contains(&MenuAction::WorkspaceWorktrees)
    );
    let view = ui.view();
    let menu = view.action_menu().unwrap();
    let row = menu
        .actions
        .iter()
        .position(|action| action == &MenuAction::WorkspaceWorktrees)
        .unwrap();
    assert!(menu.unavailable[row].is_some());
    assert!(matches!(
        ui.menus.sidebar.as_ref().unwrap().state,
        DiscoveryState::Querying
    ));
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn creation_menu_has_no_orphan_worktree_separator_when_the_checkout_list_is_empty() {
    let (mut ui, _, _, _) = context_selection::ready(false);
    let target = project_target(&ui);
    assert!(ui.open_action_menu(target, None, false));
    let mut discovery = ready_discovery(&ui);
    discovery.discovery.entries.clear();
    let episode = ui.menus.sidebar.as_mut().unwrap();
    episode.state = DiscoveryState::Ready(Box::new(discovery));
    let SidebarWorktreeTarget::Project(project) = episode.source.target.clone() else {
        unreachable!()
    };
    let entry = MenuTarget::Create(Some(project));
    let composed = actions(&entry, Vec::new(), ui.menus.sidebar.as_ref());
    assert_eq!(
        composed,
        vec![MenuAction::QuickWorkspace, MenuAction::WorkspaceAtDirectory]
    );
    let mut model = ActionMenu::default();
    assert!(model.open_for(entry, String::new(), composed, None));
    assert!(!model.view().unwrap().separator_before(0));
    assert!(!model.view().unwrap().separator_before(1));
    assert_eq!(model.view().unwrap().row_height(0, 1), 1);
}

#[test]
fn exact_linked_workspace_menu_opens_removal_confirmation_without_selecting_or_closing_it() {
    let (_, origin, discovery) =
        crate::terminal_session::state::worktree_launch::tests::removal::fixture();
    let (mut ui, mailbox, _, _) =
        crate::terminal_session::ui::tests::move_membership::ready_ui_with_snapshot(
            origin.cache.canonical_snapshot().clone(),
        );
    let location = discovery.addressed.cache.location().clone();
    assert!(ui.open_action_menu(MenuTarget::Workspace(location), None, false));
    let episode = ui.menus.sidebar.as_mut().unwrap();
    episode.state = DiscoveryState::Querying;
    let id = episode.id;
    ui.settle_sidebar_worktrees(id, Ok(Box::new(discovery)));
    let token = draw(&mut ui);
    let (source, index) = {
        let view = ui.view();
        let menu = view.action_menu().unwrap();
        (
            menu.source.clone(),
            menu.actions
                .iter()
                .position(|action| action == &MenuAction::WorkspaceWorktrees)
                .unwrap(),
        )
    };
    ui.activate_action_menu_row(&source, token, index);
    assert!(ui.take_action_menu_action(false).is_none());
    let token = draw(&mut ui);
    let (source, index) = {
        let view = ui.view();
        let menu = view.action_menu().unwrap();
        let index = menu
            .actions
            .iter()
            .position(|action| action == &MenuAction::RemoveWorktree)
            .unwrap();
        assert!(menu.separator_before(index));
        (menu.source.clone(), index)
    };
    ui.activate_action_menu_row(&source, token, index);
    assert!(ui.take_action_menu_action(false).is_none());
    assert!(!ui.action_menu_active());
    assert_eq!(
        ui.worktrees.launcher.view().unwrap().stage(),
        crate::terminal_session::WorktreeStage::Remove
    );
    assert_eq!(
        ui.worktrees
            .launcher
            .view()
            .unwrap()
            .removal_target()
            .unwrap()
            .1,
        "C:/trees/猫"
    );
    assert!(ui.worktrees.launcher.operation().is_none());
    assert_eq!(
        ui.state.as_ref().unwrap().worktree_origin().unwrap(),
        origin
    );
    assert!(mailbox.try_recv().unwrap().is_none());
}
