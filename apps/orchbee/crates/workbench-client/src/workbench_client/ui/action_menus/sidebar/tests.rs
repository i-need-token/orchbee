use super::super::Origin;
use super::*;
use crate::action_menu::{ActionMenu, MenuDraw, MenuToken};
use crate::client_view::{KeyCode, KeyEvent, Modifiers};
use crate::workbench_client::{ModalPresentation, ui::tests::context_selection};
use orchbee_workbench_protocol::WorkbenchDirectoryPath;

fn draw(ui: &mut WorkbenchClientUi) -> MenuToken {
    let view = ui.view();
    let menu = view.action_menu().unwrap();
    let draw = MenuDraw {
        token: menu.token,
        selected: menu.selected,
        candidate_visible: !menu.candidate_hidden,
        first: 0,
        visible_rows: menu.actions.len(),
        result_actions: menu.result_actions.to_vec(),
    };
    let token = draw.token;
    ui.after_action_menu_draw(Some(ModalPresentation::Presented(draw)));
    token
}

fn ready_discovery(ui: &WorkbenchClientUi) -> SidebarDiscovery {
    let addressed = ui.menus.sidebar.as_ref().unwrap().source.clone();
    let mut discovery = crate::workbench_client::state::worktree_launch::tests::discovery();
    discovery.worker = addressed.primary.as_ref().unwrap().worker_id().clone();
    let result = SidebarDiscovery {
        addressed,
        sample: crate::workbench_client::primary_directory::RootDirectory::Available(Some(
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

fn project_target(ui: &WorkbenchClientUi) -> MenuTarget {
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

/// Clicks New Workspace ▸ in an open Project menu and returns the submenu's folder rows.
fn open_creation(ui: &mut WorkbenchClientUi) -> Vec<MenuAction> {
    let token = draw(ui);
    let source = ui.view().action_menu().unwrap().source.clone();
    let project = match &source {
        MenuTarget::Project(project) => project.clone(),
        MenuTarget::Drawn { target, .. } => match target.as_ref() {
            MenuTarget::Project(project) => project.clone(),
            _ => panic!("a Project menu is open"),
        },
        _ => panic!("a Project menu is open"),
    };
    assert_eq!(
        ui.view().action_menu().unwrap().actions[0],
        MenuAction::WorkspaceCreation
    );
    ui.activate_action_menu_row(&source, token, 0);
    assert!(
        ui.take_action_menu_action(false).is_none(),
        "a submenu has no product effect"
    );
    assert!(matches!(
        ui.menus.intent.as_ref().unwrap().entry,
        MenuTarget::WorkspaceCreation(ref id) if *id == project
    ));
    project_folders(ui.state.as_ref().unwrap(), &project)
        .into_iter()
        .map(MenuAction::WorkspaceInFolder)
        .collect()
}

#[test]
fn discovery_appends_checkouts_to_new_workspace_and_requires_a_fresh_draw_without_navigation() {
    let (mut ui, mailbox, _, _) = context_selection::ready(false);
    let original = Origin::capture(ui.state.as_ref());
    let target = project_target(&ui);
    assert!(ui.open_action_menu(target, Some((8, 5).into()), false));
    assert_eq!(
        ui.view().action_menu().unwrap().actions,
        crate::action_menu::PROJECT_ACTIONS
    );
    let folders = open_creation(&mut ui);
    let before = {
        let view = ui.view();
        assert_eq!(
            view.action_menu_cascade().len(),
            1,
            "the Project menu stays"
        );
        let menu = view.action_menu().unwrap();
        assert!(menu.candidate_hidden, "a clicked submenu has no candidate");
        assert_eq!(
            menu.unavailable[0],
            Some(crate::action_menu::MenuUnavailable::WorktreeChecking)
        );
        let mut expected = vec![MenuAction::NewWorktree];
        expected.extend(folders.iter().cloned());
        assert_eq!(menu.actions, expected);
        expected
    };
    let old = draw(&mut ui);
    let episode = ui.menus.sidebar.as_mut().unwrap();
    episode.state = DiscoveryState::Querying;
    let id = episode.id;
    let discovery = ready_discovery(&ui);
    ui.settle_sidebar_worktrees(id, Ok(Box::new(discovery)));
    let source = {
        let view = ui.view();
        let menu = view.action_menu().unwrap();
        assert_ne!(menu.token, old);
        assert!(menu.candidate_hidden, "discovery does not choose a row");
        assert_eq!(menu.actions[..before.len()], before[..]);
        assert_eq!(
            menu.actions[before.len()..],
            [
                MenuAction::OpenWorktreeAt(WorkbenchDirectoryPath::parse("C:/repo").unwrap()),
                MenuAction::OpenWorktreeAt(WorkbenchDirectoryPath::parse("C:/trees/猫").unwrap()),
            ]
        );
        assert_eq!(menu.unavailable[0], None);
        let results = menu.result_actions;
        let headings = (0..results.len())
            .map(|position| menu.result_heading_before(results, position))
            .collect::<Vec<_>>();
        let mut expected = vec![None; results.len()];
        if !folders.is_empty() {
            expected[1] = Some(crate::action_menu::MenuHeading::Folders);
        }
        expected[before.len()] = Some(crate::action_menu::MenuHeading::Worktrees);
        assert_eq!(headings, expected);
        menu.source.clone()
    };
    ui.activate_action_menu_row(&source, old, 0);
    assert!(ui.take_action_menu_action(false).is_none());
    assert!(
        ui.menus.menu.confirmed().is_none(),
        "a stale draw cannot arm"
    );
    draw(&mut ui);
    assert!(ui.action_menu_input_context().is_some());
    assert_eq!(Origin::capture(ui.state.as_ref()), original);
    assert!(mailbox.try_recv().unwrap().is_none());
}

/// The candidate a person sees: none while it is hidden.
fn candidate(ui: &WorkbenchClientUi) -> Option<MenuAction> {
    let view = ui.view();
    let menu = view.action_menu()?;
    (!menu.candidate_hidden).then(|| menu.actions[menu.selected].clone())
}

/// Presents the open menu, presses one key and settles any submenu it opened.
fn press(ui: &mut WorkbenchClientUi, code: KeyCode) {
    draw(ui);
    ui.capture_action_menu_key(KeyEvent::press(code, Modifiers::NONE));
    assert!(ui.take_action_menu_action(false).is_none(), "{code:?}");
}

#[test]
fn keyboard_cascade_starts_on_first_rows_and_left_esc_or_the_parent_frame_return() {
    let (mut ui, mailbox, _, _) = context_selection::ready(false);
    let original = Origin::capture(ui.state.as_ref());
    let target = project_target(&ui);
    let anchor = crate::action_menu::MenuAnchor::Row { column: 0, row: 4 };
    assert!(ui.open_action_menu(target, Some(anchor), false));
    assert_eq!(candidate(&ui), Some(MenuAction::WorkspaceCreation));
    for (open, back) in [
        (KeyCode::Right, Some(KeyCode::Left)),
        (KeyCode::Enter, Some(KeyCode::Esc)),
        (KeyCode::Right, None),
    ] {
        press(&mut ui, open);
        {
            let view = ui.view();
            assert_eq!(view.action_menu_cascade().len(), 1, "{open:?}");
            let menu = view.action_menu().unwrap();
            assert!(menu.has_parent);
            assert!(matches!(menu.source, MenuTarget::WorkspaceCreation(_)));
            assert_eq!(menu.anchor, Some(anchor), "a child keeps the root's anchor");
        }
        assert_eq!(
            candidate(&ui),
            Some(MenuAction::NewWorktree),
            "a keyboard submenu starts on its first row"
        );
        if let Some(back) = back {
            press(&mut ui, back);
        } else {
            // A click anywhere on the painted parent returns as Left does.
            let token = draw(&mut ui);
            let source = ui.view().action_menu().unwrap().source.clone();
            ui.activate_action_menu_control(&source, token, crate::action_menu::MenuControl::Back);
        }
        let view = ui.view();
        assert!(view.action_menu_cascade().is_empty());
        assert!(!view.action_menu().unwrap().has_parent);
        drop(view);
        assert_eq!(
            candidate(&ui),
            Some(MenuAction::WorkspaceCreation),
            "returning names the row that opened the submenu"
        );
    }
    press(&mut ui, KeyCode::Esc);
    assert!(!ui.action_menu_active(), "Esc in the root closes the menu");
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
    let folders = open_creation(&mut ui);
    assert_ne!(ui.menus.sidebar.as_ref().unwrap().id, cancelled_id);
    let token = ui.view().action_menu().unwrap().token;
    ui.menus.sidebar.as_mut().unwrap().state = DiscoveryState::Querying;
    ui.settle_sidebar_worktrees(cancelled_id, Ok(Box::new(stale)));
    let view = ui.view();
    let menu = view.action_menu().unwrap();
    assert_eq!(menu.token, token);
    let mut expected = vec![MenuAction::NewWorktree];
    expected.extend(folders);
    assert_eq!(menu.actions, expected, "a stale episode adds no checkout");
    assert!(menu.unavailable[0].is_some());
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
    let view = model.view().unwrap();
    assert!(!view.result_separator_before(view.result_actions, 0));
    assert!(!view.result_separator_before(view.result_actions, 1));
    assert_eq!(view.result_row_height(view.result_actions, 0, 1), 1);
}

#[test]
fn exact_linked_workspace_menu_opens_removal_confirmation_without_selecting_or_closing_it() {
    let (_, origin, discovery) =
        crate::workbench_client::state::worktree_launch::tests::removal::fixture();
    let (mut ui, mailbox, _, _) =
        crate::workbench_client::ui::tests::move_membership::ready_ui_with_snapshot(
            origin.cache.canonical_snapshot().clone(),
        );
    let location = discovery.addressed.cache.location().clone();
    assert!(ui.open_action_menu(MenuTarget::Workspace(location), None, false));
    assert!(
        !ui.view()
            .action_menu()
            .unwrap()
            .actions
            .contains(&MenuAction::RemoveWorktree),
        "removal waits for its proof"
    );
    let episode = ui.menus.sidebar.as_mut().unwrap();
    episode.state = DiscoveryState::Querying;
    let id = episode.id;
    ui.settle_sidebar_worktrees(id, Ok(Box::new(discovery)));
    let token = draw(&mut ui);
    let (source, index) = {
        let view = ui.view();
        let menu = view.action_menu().unwrap();
        let index = menu.actions.len() - 1;
        assert_eq!(menu.actions[index], MenuAction::RemoveWorktree);
        assert_eq!(menu.actions[index - 1], MenuAction::DeleteWorkspace);
        // Close and removal share the trailing section.
        assert!(menu.result_separator_before(menu.result_actions, index - 1));
        assert!(!menu.result_separator_before(menu.result_actions, index));
        (menu.source.clone(), index)
    };
    ui.activate_action_menu_row(&source, token, index);
    assert!(ui.take_action_menu_action(false).is_none());
    assert!(!ui.action_menu_active());
    assert_eq!(
        ui.worktrees.launcher.view().unwrap().stage(),
        crate::workbench_client::WorktreeStage::Remove
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
