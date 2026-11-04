use super::super::Origin;
use super::*;
use crate::action_menu::{ActionMenu, MenuDraw, MenuToken};
use crate::client_view::{KeyCode, KeyEvent, Modifiers};
use crate::tui_presentation::ModalPresentation;
use crate::workbench_client::ui::tests::context_selection;
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
    let addressed = ui.menus.discovery().unwrap().source().clone();
    let mut discovery = crate::workbench_state::worktree_launch::tests::discovery();
    discovery.worker = addressed.primary.as_ref().unwrap().worker_id().clone();
    let result = SidebarDiscovery {
        addressed,
        sample: crate::primary_directory::RootDirectory::Available(Some(
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

/// Clicks New Workspace ▸ in an open Project menu and returns the submenu's rows before discovery.
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
        ui.menus.intent().unwrap().entry(),
        MenuTarget::WorkspaceCreation(id) if *id == project
    ));
    let folders = project_folders(ui.state.as_ref().unwrap(), &project);
    // Without a folder row, the Project's quick path leads.
    let mut rows = if folders.is_empty() {
        vec![MenuAction::QuickWorkspace]
    } else {
        Vec::new()
    };
    rows.push(MenuAction::NewWorktree);
    rows.extend(folders.into_iter().map(MenuAction::WorkspaceInFolder));
    rows
}

fn worktree_row(rows: &[MenuAction]) -> usize {
    rows.iter()
        .position(|action| *action == MenuAction::NewWorktree)
        .unwrap()
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
    let rows = open_creation(&mut ui);
    let worktree = worktree_row(&rows);
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
            menu.unavailable[worktree],
            Some(crate::action_menu::MenuUnavailable::WorktreeChecking)
        );
        assert_eq!(menu.actions, rows);
        rows
    };
    let old = draw(&mut ui);
    ui.menus.mark_discovery_querying();
    let id = ui.menus.discovery().unwrap().id();
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
        assert_eq!(menu.unavailable[worktree], None);
        let results = menu.result_actions;
        let headings = (0..results.len())
            .map(|position| menu.result_heading_before(results, position))
            .collect::<Vec<_>>();
        let mut expected = vec![None; results.len()];
        if worktree + 1 < before.len() {
            expected[worktree + 1] = Some(crate::action_menu::MenuHeading::Folders);
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
            Some(MenuAction::QuickWorkspace),
            "a keyboard submenu starts on its first row, the folderless Project's quick path"
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
fn pointer_rest_opens_only_a_cascading_submenu_and_a_parent_row_takes_it_back() {
    use crate::action_menu::MenuControl;
    use crate::terminal_input::{MenuAction as Gesture, SurfaceAction};
    let (mut ui, mailbox, _, _) = context_selection::ready(false);
    let original = Origin::capture(ui.state.as_ref());
    let rest = |ui: &mut WorkbenchClientUi, row| {
        let token = draw(ui);
        let source = ui.view().action_menu().unwrap().source.clone();
        let opened = ui
            .surface_action(SurfaceAction::ActionMenu(Gesture::Rest {
                source: &source,
                token,
                row,
            }))
            .is_some();
        assert!(ui.take_action_menu_action(false).is_none());
        opened
    };

    // The global menu's new-Tab submenu replaces its page, so it waits for a click or key.
    assert!(ui.open_action_menu(MenuTarget::Global, Some((7, 4).into()), false));
    let row = ui
        .view()
        .action_menu()
        .unwrap()
        .actions
        .iter()
        .position(|action| *action == MenuAction::TabCreation)
        .unwrap();
    assert!(!rest(&mut ui, row));
    assert_eq!(ui.menus.parent_count(), 0);
    ui.cancel_action_menu();

    let anchor = crate::action_menu::MenuAnchor::Pointer { column: 0, row: 4 };
    assert!(ui.open_action_menu(project_target(&ui), Some(anchor), false));
    assert!(!rest(&mut ui, 1), "resting on Rename does not rename");
    assert_eq!(ui.menus.parent_count(), 0);
    assert!(rest(&mut ui, 0), "resting on New Workspace ▸ opens it");
    assert!(matches!(
        ui.menus.intent().unwrap().entry(),
        MenuTarget::WorkspaceCreation(_)
    ));
    assert_eq!(candidate(&ui), None, "the pointer chooses in the child");

    // Another parent row takes the menu back with itself as the candidate.
    let token = draw(&mut ui);
    let source = ui.view().action_menu().unwrap().source.clone();
    ui.activate_action_menu_control(&source, token, MenuControl::ParentRow(1));
    assert_eq!(ui.menus.parent_count(), 0);
    assert_eq!(candidate(&ui), Some(MenuAction::RenameProject));
    assert_eq!(Origin::capture(ui.state.as_ref()), original);
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn cancelled_or_replaced_episode_cannot_reopen_or_populate_a_new_menu() {
    let (mut ui, mailbox, _, _) = context_selection::ready(false);
    let target = project_target(&ui);
    assert!(ui.open_action_menu(target.clone(), None, false));
    ui.menus.mark_discovery_querying();
    let cancelled_id = ui.menus.discovery().unwrap().id();
    let stale = ready_discovery(&ui);
    ui.capture_action_menu_key(KeyEvent::press(KeyCode::Esc, Modifiers::NONE));
    assert!(!ui.action_menu_active());
    assert!(ui.menus.discovery().is_none());
    ui.settle_sidebar_worktrees(cancelled_id, Ok(Box::new(stale.clone())));
    assert!(!ui.action_menu_active());
    assert!(ui.open_action_menu(target, None, false));
    let rows = open_creation(&mut ui);
    assert_ne!(ui.menus.discovery().unwrap().id(), cancelled_id);
    let token = ui.view().action_menu().unwrap().token;
    ui.menus.mark_discovery_querying();
    ui.settle_sidebar_worktrees(cancelled_id, Ok(Box::new(stale)));
    let view = ui.view();
    let menu = view.action_menu().unwrap();
    assert_eq!(menu.token, token);
    assert_eq!(menu.actions, rows, "a stale episode adds no checkout");
    assert!(menu.unavailable[worktree_row(&rows)].is_some());
    assert!(matches!(
        ui.menus.discovery().unwrap().state(),
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
    ui.menus.mark_discovery_querying();
    let id = ui.menus.discovery().unwrap().id();
    assert!(ui.menus.settle_discovery(
        id,
        super::super::episode::Answer::Found(Box::new(discovery)),
        || ui.clock.now(),
        |_| true,
        |_| true,
    ));
    let SidebarWorktreeTarget::Project(project) =
        ui.menus.discovery().unwrap().source().target.clone()
    else {
        unreachable!()
    };
    let entry = MenuTarget::Create(Some(project));
    let composed = actions(&entry, Vec::new(), ui.menus.discovery());
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
    let (_, origin, discovery) = crate::workbench_state::worktree_launch::tests::removal::fixture();
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
    ui.menus.mark_discovery_querying();
    let id = ui.menus.discovery().unwrap().id();
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
        crate::workbench_state::worktree_launch::WorktreeStage::Remove
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

/// An ungrouped Workspace whose first shell sits in its own Git directory, with its row menu open.
fn ungrouped_git_workspace() -> (
    WorkbenchClientUi,
    std::sync::Arc<crate::workbench_client::mailbox::CommandMailbox>,
    MenuTarget,
) {
    let (mut ui, mailbox, _, target) = closed_ungrouped_git_workspace();
    assert!(ui.open_action_menu(target.clone(), None, false));
    (ui, mailbox, target)
}

/// That Workspace's menu, opened from a sidebar row that shows Git facts for `directory`.
fn opened_on_sidebar_git(directory: &str) -> WorkbenchClientUi {
    let (mut ui, _, _, target) = closed_ungrouped_git_workspace();
    ui.observe_sidebar_git(Some(WorkbenchDirectoryPath::parse(directory).unwrap()));
    assert!(ui.open_action_menu(target, None, false));
    ui
}

/// Chooses the row that `action` names.
fn choose(ui: &mut WorkbenchClientUi, action: &MenuAction) {
    let token = draw(ui);
    let source = ui.view().action_menu().unwrap().source.clone();
    let index = row(ui, action);
    ui.activate_action_menu_row(&source, token, index);
}

fn closed_ungrouped_git_workspace() -> (
    WorkbenchClientUi,
    std::sync::Arc<crate::workbench_client::mailbox::CommandMailbox>,
    crate::local_client::LocalOperationClock,
    MenuTarget,
) {
    let mut snapshot = crate::workbench_state::worktree_launch::addressed::tests::fixture();
    let member = snapshot
        .navigation
        .workspaces
        .iter_mut()
        .find(|workspace| workspace.workspace_id.as_str() == format!("{:032x}", 80))
        .unwrap();
    member.project_id = None;
    member.automatic_label = true;
    let location = ClientContextLocation::Workspace {
        workspace_id: member.workspace_id.clone(),
        terminal_context_id: member.terminal_context_id.clone(),
    };
    let (ui, mailbox, clock, _) =
        crate::workbench_client::ui::tests::move_membership::ready_ui_with_snapshot(snapshot);
    (ui, mailbox, clock, MenuTarget::Workspace(location))
}

/// The discovery the lane reports for that shell's cwd: the main checkout and one linked one.
fn workspace_discovery(ui: &WorkbenchClientUi) -> SidebarDiscovery {
    use orchbee_workbench_protocol::{WorktreeEntryText, WorktreeQuerySource, WorktreeSource};
    let addressed = ui.menus.discovery().unwrap().source().clone();
    let directory = addressed.cache.directory().unwrap().clone();
    // The shell reports its cwd in its own spelling.
    let sample = crate::primary_directory::RootDirectory::Available(Some(
        WorkbenchDirectoryPath::parse("c:\\First-Repository\\").unwrap(),
    ));
    let source = addressed.resolve_sample(sample.clone()).unwrap();
    let WorktreeSource::Workspace { workspace_id } = &source else {
        panic!("a cwd in the Workspace's directory is that Workspace: {source:?}");
    };
    let mut discovery = crate::workbench_state::worktree_launch::tests::discovery();
    discovery.worker = directory.worker_id.clone();
    discovery.source = WorktreeQuerySource {
        requested_path: directory.path.clone(),
        repository_root: directory.path.clone(),
        common_directory: WorkbenchDirectoryPath::parse("C:/first-repository/.git").unwrap(),
        checkout_path: directory.path.clone(),
        workspace_id: Some(workspace_id.clone()),
    };
    discovery.entries[0].checkout.path = WorktreeEntryText::Utf8(directory.path.as_str().into());
    let result = SidebarDiscovery {
        addressed,
        sample,
        source,
        discovery,
    };
    assert!(result.valid(ui.state.as_ref().unwrap().cache().canonical_snapshot()));
    result
}

/// Publishes what the Server commits when the first terminal moves: the followed directory, and
/// the automatic name it carries.
fn first_terminal_moves(ui: &mut WorkbenchClientUi, label: &str, directory: &str) {
    use orchbee_workbench_protocol::{WorkbenchRevision, WorkbenchSnapshotOutcome};
    let now = ui.clock.now();
    let state = ui.state.as_mut().unwrap();
    let mut snapshot = state.cache().canonical_snapshot().clone();
    snapshot.revision = WorkbenchRevision::new(snapshot.revision.value() + 1);
    let member = snapshot
        .navigation
        .workspaces
        .iter_mut()
        .find(|workspace| workspace.workspace_id.as_str() == format!("{:032x}", 80))
        .unwrap();
    member.label = label.into();
    member.terminal_directory = Some(WorkbenchDirectoryPath::parse(directory).unwrap());
    assert!(state.start_pending_refresh(44));
    let prepared = state.prepare_refresh(44, WorkbenchSnapshotOutcome::Available { snapshot });
    state.apply_prepared_refresh(prepared, now);
    ui.drive_sidebar_worktrees();
    ui.drive_action_menu();
}

/// The answer the lane reports about where the first terminal stands now: the checkout it stands
/// in and, with `entries` of two, one linked checkout.
fn answer_where_it_stands(ui: &WorkbenchClientUi, entries: usize) -> SidebarDiscovery {
    use orchbee_workbench_protocol::{WorktreeEntryText, WorktreeQuerySource, WorktreeSource};
    let addressed = ui.menus.discovery().unwrap().source().clone();
    let sample = crate::primary_directory::RootDirectory::Unavailable;
    let source = addressed.resolve_sample(sample.clone()).unwrap();
    let WorktreeSource::WorkspaceDirectory { workspace_id, path } = &source else {
        panic!("a followed directory outside the Workspace's own: {source:?}");
    };
    let mut discovery = crate::workbench_state::worktree_launch::tests::discovery();
    discovery.worker = addressed.cache.directory().unwrap().worker_id.clone();
    discovery.source = WorktreeQuerySource {
        requested_path: path.clone(),
        repository_root: WorkbenchDirectoryPath::parse("C:/first-repository").unwrap(),
        common_directory: WorkbenchDirectoryPath::parse("C:/first-repository/.git").unwrap(),
        checkout_path: WorkbenchDirectoryPath::parse("C:/first-repository").unwrap(),
        workspace_id: Some(workspace_id.clone()),
    };
    discovery.entries[0].checkout.path = WorktreeEntryText::Utf8(path.as_str().into());
    discovery.entries.truncate(entries);
    let result = SidebarDiscovery {
        addressed,
        sample,
        source,
        discovery,
    };
    assert!(result.valid(ui.state.as_ref().unwrap().cache().canonical_snapshot()));
    result
}

fn settle(ui: &mut WorkbenchClientUi, result: Result<Box<SidebarDiscovery>, WorktreeNotice>) {
    ui.menus.mark_discovery_querying();
    let id = ui.menus.discovery().unwrap().id();
    ui.settle_sidebar_worktrees(id, result);
}

fn row(ui: &WorkbenchClientUi, action: &MenuAction) -> usize {
    let view = ui.view();
    let menu = view.action_menu().unwrap();
    menu.actions.iter().position(|row| row == action).unwrap()
}

fn unavailable(
    ui: &WorkbenchClientUi,
    action: &MenuAction,
) -> Option<crate::action_menu::MenuUnavailable> {
    let index = row(ui, action);
    ui.view().action_menu().unwrap().unavailable[index]
}

#[test]
fn a_move_within_the_answered_checkout_keeps_the_worktree_rows_and_any_other_drops_them() {
    use crate::action_menu::MenuUnavailable;
    let rows = [MenuAction::NewWorktree, MenuAction::WorkspaceWorktrees];
    let (mut ui, _, _) = ungrouped_git_workspace();
    let discovery = workspace_discovery(&ui);
    settle(&mut ui, Ok(Box::new(discovery)));
    let token = draw(&mut ui);
    let label = ui.view().action_menu().unwrap().title.to_owned();

    // Nothing the person sees changes while Git is asked again, or when it says the same.
    first_terminal_moves(&mut ui, &label, "C:/first-repository/sub");
    for action in &rows {
        assert_eq!(unavailable(&ui, action), None, "{action:?} during the read");
    }
    assert_eq!(ui.view().action_menu().unwrap().token, token);
    let same = answer_where_it_stands(&ui, 2);
    settle(&mut ui, Ok(Box::new(same)));
    for action in &rows {
        assert_eq!(unavailable(&ui, action), None, "{action:?} after the read");
    }
    assert_eq!(ui.view().action_menu().unwrap().token, token);

    // A different answer changes the rows once, with the read.
    first_terminal_moves(&mut ui, &label, "C:/first-repository/other");
    let fewer = answer_where_it_stands(&ui, 1);
    settle(&mut ui, Ok(Box::new(fewer)));
    assert_eq!(unavailable(&ui, &MenuAction::NewWorktree), None);
    assert_eq!(
        unavailable(&ui, &MenuAction::WorkspaceWorktrees),
        Some(MenuUnavailable::NoOtherWorktrees)
    );

    // A path that only starts like the checkout's, or climbs out of it, stands elsewhere.
    for elsewhere in ["C:/first-repository-two", "C:/first-repository/../plain"] {
        first_terminal_moves(&mut ui, &label, elsewhere);
        for action in &rows {
            assert_eq!(
                unavailable(&ui, action),
                Some(MenuUnavailable::WorktreeChecking),
                "{action:?} in {elsewhere}"
            );
        }
        let answer = answer_where_it_stands(&ui, 2);
        settle(&mut ui, Ok(Box::new(answer)));
    }
    // The shell's own spelling of a directory in the checkout stands in it.
    first_terminal_moves(&mut ui, &label, "c:\\First-Repository\\sub");
    assert_eq!(unavailable(&ui, &MenuAction::NewWorktree), None);
    let answer = answer_where_it_stands(&ui, 2);
    settle(&mut ui, Ok(Box::new(answer)));

    // Leaving for a folder outside Git leaves nothing of what Git said about the repository.
    first_terminal_moves(&mut ui, &label, "C:/plain");
    for action in &rows {
        assert_eq!(
            unavailable(&ui, action),
            Some(MenuUnavailable::WorktreeChecking),
            "{action:?} during the read"
        );
    }
    settle(&mut ui, Err(WorktreeNotice::ListingUnavailable));
    for action in &rows {
        assert_eq!(
            unavailable(&ui, action),
            Some(MenuUnavailable::WorktreeUnavailable),
            "{action:?} after the read"
        );
    }

    // No answer about one folder speaks for another, in or out of Git.
    first_terminal_moves(&mut ui, &label, "C:/plain/deeper");
    assert_eq!(
        unavailable(&ui, &MenuAction::NewWorktree),
        Some(MenuUnavailable::WorktreeChecking)
    );
    settle(&mut ui, Err(WorktreeNotice::ListingUnavailable));
    first_terminal_moves(&mut ui, &label, "C:/first-repository/back");
    assert_eq!(
        unavailable(&ui, &MenuAction::NewWorktree),
        Some(MenuUnavailable::WorktreeChecking)
    );
    let answer = answer_where_it_stands(&ui, 2);
    settle(&mut ui, Ok(Box::new(answer)));
    for action in &rows {
        assert_eq!(unavailable(&ui, action), None, "{action:?} back in Git");
    }
}

/// Open Worktree ▸ depends on the checkout list, so a menu starts it from the last list Git gave
/// for the same Workspace in the same checkout, and from nothing else.
#[test]
fn open_worktree_starts_from_the_last_list_for_the_same_checkout() {
    use crate::action_menu::MenuUnavailable;
    let (mut ui, _, _, target) = closed_ungrouped_git_workspace();
    let open = |ui: &mut WorkbenchClientUi| {
        ui.observe_sidebar_git(Some(
            WorkbenchDirectoryPath::parse(SIDEBAR_DIRECTORY).unwrap(),
        ));
        assert!(ui.open_action_menu(target.clone(), None, false));
    };
    let close = |ui: &mut WorkbenchClientUi| {
        for _ in 0..3 {
            if ui.action_menu_active() {
                press(ui, KeyCode::Esc);
            }
        }
        assert!(!ui.action_menu_active());
    };
    let worktrees = |ui: &WorkbenchClientUi| unavailable(ui, &MenuAction::WorkspaceWorktrees);
    open(&mut ui);
    let label = ui.view().action_menu().unwrap().title.to_owned();
    assert_eq!(worktrees(&ui), Some(MenuUnavailable::WorktreeChecking));

    // The only checkout: every later menu says so from its first paint.
    let mut only = workspace_discovery(&ui);
    only.discovery.entries.truncate(1);
    settle(&mut ui, Ok(Box::new(only)));
    close(&mut ui);
    open(&mut ui);
    assert_eq!(worktrees(&ui), Some(MenuUnavailable::NoOtherWorktrees));

    // A linked checkout: the next menu offers the list at once.
    let answer = workspace_discovery(&ui);
    settle(&mut ui, Ok(Box::new(answer)));
    close(&mut ui);
    open(&mut ui);
    assert_eq!(worktrees(&ui), None);
    choose(&mut ui, &MenuAction::WorkspaceWorktrees);
    assert!(ui.take_action_menu_action(false).is_none());
    assert!(matches!(
        ui.view().action_menu().unwrap().source,
        MenuTarget::WorkspaceWorktrees(_)
    ));
    close(&mut ui);

    // Nothing is kept for a checkout the Workspace left, or after Git stopped answering there.
    first_terminal_moves(&mut ui, &label, "C:/first-repository-two");
    open(&mut ui);
    assert_eq!(worktrees(&ui), Some(MenuUnavailable::WorktreeChecking));
    close(&mut ui);
    first_terminal_moves(&mut ui, &label, "C:/first-repository/sub");
    open(&mut ui);
    assert_eq!(worktrees(&ui), None);
    settle(&mut ui, Err(WorktreeNotice::ListingUnavailable));
    close(&mut ui);
    open(&mut ui);
    assert_eq!(worktrees(&ui), Some(MenuUnavailable::WorktreeChecking));
}

/// Git-only actions follow the current addressed result, including the first open menu.
#[test]
fn non_git_discovery_removes_worktree_actions_and_a_new_repository_restores_them() {
    let rows = [MenuAction::NewWorktree, MenuAction::WorkspaceWorktrees];
    let (mut ui, _, _, target) = closed_ungrouped_git_workspace();
    assert!(ui.open_action_menu(target.clone(), None, false));
    let stale = draw(&mut ui);
    settle(&mut ui, Err(WorktreeNotice::NotRepository));
    assert!(
        rows.iter()
            .all(|row| !ui.view().action_menu().unwrap().actions.contains(row))
    );
    assert_ne!(
        ui.view().action_menu().unwrap().token,
        stale,
        "old hit proofs must expire"
    );
    press(&mut ui, KeyCode::Esc);
    assert!(ui.open_action_menu(target, None, false));
    assert!(
        rows.iter()
            .all(|row| !ui.view().action_menu().unwrap().actions.contains(row))
    );
    let answer = workspace_discovery(&ui);
    settle(&mut ui, Ok(Box::new(answer)));
    assert!(
        rows.iter()
            .all(|row| ui.view().action_menu().unwrap().actions.contains(row))
    );
}

/// A row chosen on an answer the Workspace then left, or that Git no longer repeats, closes the menu
/// with the notice at once; it neither runs nor waits out the menu's deadline.
#[test]
fn a_chosen_row_never_acts_on_what_git_said_about_another_directory() {
    let closed_with_the_notice = |ui: &mut WorkbenchClientUi| {
        assert!(ui.take_action_menu_action(false).is_none());
        assert!(!ui.action_menu_active());
        assert!(ui.worktrees.launcher.view().is_none());
        assert_eq!(
            ui.command_notice.current(ui.clock.now()),
            Some(crate::localization::WorkbenchClientNotice::MenuUnavailable)
        );
    };
    let on_the_checkout_list = || {
        let (mut ui, _, _) = ungrouped_git_workspace();
        let discovery = workspace_discovery(&ui);
        settle(&mut ui, Ok(Box::new(discovery)));
        choose(&mut ui, &MenuAction::WorkspaceWorktrees);
        assert!(ui.take_action_menu_action(false).is_none(), "a submenu");
        ui
    };
    let linked = MenuAction::OpenWorktreeAt(WorkbenchDirectoryPath::parse("C:/trees/猫").unwrap());

    // Chosen on what the sidebar showed, then the Workspace leaves before Git answers.
    let mut ui = opened_on_sidebar_git(SIDEBAR_DIRECTORY);
    let label = ui.view().action_menu().unwrap().title.to_owned();
    choose(&mut ui, &MenuAction::NewWorktree);
    assert!(ui.take_action_menu_action(false).is_none());
    assert!(ui.action_menu_active(), "it waits for the answer");
    first_terminal_moves(&mut ui, &label, "C:/plain");
    closed_with_the_notice(&mut ui);

    // An open list of the repository's checkouts ends when the Workspace leaves it.
    let mut ui = on_the_checkout_list();
    first_terminal_moves(&mut ui, "plain", "C:/plain");
    assert!(!ui.action_menu_active());
    assert!(ui.worktrees.launcher.view().is_none());

    // A listed checkout chosen after a move within the checkout waits for the new answer.
    let mut ui = on_the_checkout_list();
    first_terminal_moves(&mut ui, "sub", "C:/first-repository/sub");
    choose(&mut ui, &linked);
    assert!(ui.take_action_menu_action(false).is_none());
    assert!(ui.action_menu_active(), "it waits for the answer");
    settle(&mut ui, Err(WorktreeNotice::ListingUnavailable));
    closed_with_the_notice(&mut ui);

    let mut ui = on_the_checkout_list();
    first_terminal_moves(&mut ui, "sub", "C:/first-repository/sub");
    choose(&mut ui, &linked);
    assert!(ui.take_action_menu_action(false).is_none());
    assert!(ui.action_menu_active(), "it waits for the answer");
    first_terminal_moves(&mut ui, "plain", "C:/plain");
    closed_with_the_notice(&mut ui);
}

#[test]
fn a_row_chosen_during_the_read_waits_for_the_answer_and_acts_on_it() {
    use crate::workbench_state::worktree_launch::WorktreeMode;
    let (mut ui, _, _) = ungrouped_git_workspace();
    let discovery = workspace_discovery(&ui);
    settle(&mut ui, Ok(Box::new(discovery)));
    first_terminal_moves(&mut ui, "sub", "C:/first-repository/sub");
    let token = draw(&mut ui);
    let source = ui.view().action_menu().unwrap().source.clone();
    let index = row(&ui, &MenuAction::NewWorktree);
    ui.activate_action_menu_row(&source, token, index);
    assert!(ui.take_action_menu_action(false).is_none());
    assert!(ui.action_menu_active(), "it waits for the answer");
    assert!(ui.worktrees.launcher.view().is_none());

    let answer = answer_where_it_stands(&ui, 2);
    settle(&mut ui, Ok(Box::new(answer)));
    assert!(ui.take_action_menu_action(false).is_none());
    assert!(!ui.action_menu_active());
    let origin = ui.state.as_ref().unwrap().worktree_origin().unwrap();
    let launcher = &mut ui.worktrees.launcher;
    assert_eq!(launcher.view().unwrap().mode(), WorktreeMode::Create);
    let token = launcher.token().unwrap();
    launcher.draw(Some(token));
    let plan = launcher
        .create_with_defaults(
            orchbee_workbench_protocol::WorkbenchRequestId::parse(&format!("{:032x}", 903))
                .unwrap(),
            &origin,
            crate::localization::Locale::EnUs,
        )
        .unwrap();
    assert!(matches!(
        &plan.intent,
        orchbee_workbench_protocol::WorktreeIntent::Create {
            source: orchbee_workbench_protocol::WorktreeSource::WorkspaceDirectory { path, .. },
            ..
        } if path.as_str() == "C:/first-repository/sub"
    ));
}

#[test]
fn a_chosen_checkout_the_new_answer_no_longer_lists_is_reported() {
    let (mut ui, _, _) = ungrouped_git_workspace();
    let discovery = workspace_discovery(&ui);
    settle(&mut ui, Ok(Box::new(discovery)));
    let token = draw(&mut ui);
    let source = ui.view().action_menu().unwrap().source.clone();
    let index = row(&ui, &MenuAction::WorkspaceWorktrees);
    ui.activate_action_menu_row(&source, token, index);
    assert!(ui.take_action_menu_action(false).is_none(), "a submenu");

    first_terminal_moves(&mut ui, "sub", "C:/first-repository/sub");
    let token = draw(&mut ui);
    let source = ui.view().action_menu().unwrap().source.clone();
    let index = row(
        &ui,
        &MenuAction::OpenWorktreeAt(WorkbenchDirectoryPath::parse("C:/trees/猫").unwrap()),
    );
    ui.activate_action_menu_row(&source, token, index);
    assert!(ui.take_action_menu_action(false).is_none());
    assert!(ui.action_menu_active(), "it waits for the answer");

    let answer = answer_where_it_stands(&ui, 1);
    settle(&mut ui, Ok(Box::new(answer)));
    assert!(ui.take_action_menu_action(false).is_none());
    assert!(!ui.action_menu_active());
    assert!(ui.worktrees.launcher.view().is_none());
    assert_eq!(
        ui.command_notice.current(ui.clock.now()),
        Some(crate::localization::WorkbenchClientNotice::MenuUnavailable)
    );
}

const SIDEBAR_DIRECTORY: &str = "C:/first-repository";

#[test]
fn new_worktree_starts_enabled_where_the_sidebar_shows_a_branch_and_waits_when_chosen() {
    use crate::action_menu::MenuUnavailable;
    let rows = [MenuAction::NewWorktree, MenuAction::WorkspaceWorktrees];

    // Git facts prove New Worktree…, and Open Worktree ▸ waits for the checkouts it lists.
    let mut ui = opened_on_sidebar_git(SIDEBAR_DIRECTORY);
    assert_eq!(unavailable(&ui, &MenuAction::NewWorktree), None);
    assert_eq!(
        unavailable(&ui, &MenuAction::WorkspaceWorktrees),
        Some(MenuUnavailable::WorktreeChecking)
    );
    let answer = workspace_discovery(&ui);
    settle(&mut ui, Ok(Box::new(answer.clone())));
    for action in &rows {
        assert_eq!(unavailable(&ui, action), None, "{action:?} after the read");
    }

    let mut ui = opened_on_sidebar_git(SIDEBAR_DIRECTORY);
    choose(&mut ui, &MenuAction::NewWorktree);
    assert!(ui.take_action_menu_action(false).is_none());
    assert!(ui.action_menu_active(), "it waits");
    assert!(ui.worktrees.launcher.view().is_none());
    settle(&mut ui, Ok(Box::new(answer.clone())));
    assert!(ui.take_action_menu_action(false).is_none());
    assert!(!ui.action_menu_active());
    assert!(ui.worktrees.launcher.view().is_some());

    // An answer that no longer offers the chosen row closes the menu and says so.
    let mut only = answer;
    only.discovery.entries.truncate(1);
    let mut ui = opened_on_sidebar_git(SIDEBAR_DIRECTORY);
    choose(&mut ui, &MenuAction::NewWorktree);
    assert!(ui.take_action_menu_action(false).is_none());
    settle(&mut ui, Err(WorktreeNotice::ListingUnavailable));
    assert!(ui.take_action_menu_action(false).is_none());
    assert!(!ui.action_menu_active());
    assert_eq!(
        ui.command_notice.current(ui.clock.now()),
        Some(crate::localization::WorkbenchClientNotice::MenuUnavailable)
    );

    // An answer that differs changes the rows it concerns, and the rows it leaves stay.
    let mut ui = opened_on_sidebar_git(SIDEBAR_DIRECTORY);
    settle(&mut ui, Ok(Box::new(only)));
    assert_eq!(unavailable(&ui, &MenuAction::NewWorktree), None);
    assert_eq!(
        unavailable(&ui, &MenuAction::WorkspaceWorktrees),
        Some(MenuUnavailable::NoOtherWorktrees)
    );
    let mut ui = opened_on_sidebar_git(SIDEBAR_DIRECTORY);
    settle(&mut ui, Err(WorktreeNotice::ListingUnavailable));
    assert_eq!(
        unavailable(&ui, &MenuAction::NewWorktree),
        Some(MenuUnavailable::WorktreeUnavailable)
    );

    // A `cd` below that directory before the first answer leaves the rows as they are, and any
    // other leaves nothing of what the sidebar showed.
    let mut ui = opened_on_sidebar_git(SIDEBAR_DIRECTORY);
    let label = ui.view().action_menu().unwrap().title.to_owned();
    first_terminal_moves(&mut ui, &label, "C:/first-repository/sub");
    assert_eq!(unavailable(&ui, &MenuAction::NewWorktree), None);
    first_terminal_moves(&mut ui, &label, "C:/plain");
    for action in &rows {
        assert_eq!(
            unavailable(&ui, action),
            Some(MenuUnavailable::WorktreeChecking),
            "{action:?} after leaving"
        );
    }

    // Facts about another directory say nothing about this one.
    let ui = opened_on_sidebar_git("C:/elsewhere");
    for action in &rows {
        assert_eq!(
            unavailable(&ui, action),
            Some(MenuUnavailable::WorktreeChecking)
        );
    }
}

#[test]
fn a_background_read_greys_no_setup_row_and_a_chosen_one_waits_for_it() {
    use crate::action_menu::MenuUnavailable;
    use crate::workbench_client::ui::tests::refresh_gating::{
        background_read, settle_background_read,
    };
    let (mut ui, mailbox, clock, target) = closed_ungrouped_git_workspace();
    assert!(ui.open_action_menu(target, None, false));
    let token = draw(&mut ui);
    let mut snapshot = ui
        .state
        .as_ref()
        .unwrap()
        .cache()
        .canonical_snapshot()
        .clone();

    let read = background_read(&mut ui, &mailbox);
    assert!(ui.state.as_ref().unwrap().background_refresh_only());
    ui.drive_action_menu();
    assert_eq!(unavailable(&ui, &MenuAction::WorkspaceProjects), None);
    assert_eq!(
        ui.view().action_menu().unwrap().token,
        token,
        "nothing to repaint"
    );

    // Move to Project opens at once. The row with an effect holds what it needs until the read
    // settled, and no further read overtakes it.
    choose(&mut ui, &MenuAction::WorkspaceProjects);
    assert!(ui.take_action_menu_action(false).is_none());
    assert!(matches!(
        ui.view().action_menu().unwrap().source,
        MenuTarget::WorkspaceProjects(_)
    ));
    assert_eq!(unavailable(&ui, &MenuAction::CreateWorkspaceProject), None);
    choose(&mut ui, &MenuAction::CreateWorkspaceProject);
    assert!(ui.take_action_menu_action(false).is_none(), "it waits");
    assert!(ui.action_menu_active());
    assert!(ui.background_reads_yield());
    snapshot.revision =
        orchbee_workbench_protocol::WorkbenchRevision::new(snapshot.revision.value() + 1);
    settle_background_read(&mut ui, &mailbox, &clock, read, snapshot, true);
    assert!(
        ui.take_action_menu_action(false).is_some(),
        "the request runs against what the read installed"
    );

    // The Client's own read still makes the row busy.
    let (mut ui, _, _) = ungrouped_git_workspace();
    assert!(ui.state.as_mut().unwrap().start_pending_refresh(44));
    ui.drive_action_menu();
    assert_eq!(
        unavailable(&ui, &MenuAction::WorkspaceProjects),
        Some(MenuUnavailable::OperationBusy)
    );
}

#[test]
fn the_menu_heading_follows_the_workspace_name_beside_an_open_submenu() {
    let (mut ui, _, _) = ungrouped_git_workspace();
    let discovery = workspace_discovery(&ui);
    settle(&mut ui, Ok(Box::new(discovery)));
    first_terminal_moves(&mut ui, "sub (2)", "C:/first-repository/sub");
    assert_eq!(ui.view().action_menu().unwrap().title, "sub (2)");

    let answer = answer_where_it_stands(&ui, 2);
    settle(&mut ui, Ok(Box::new(answer)));
    let token = draw(&mut ui);
    let source = ui.view().action_menu().unwrap().source.clone();
    let index = row(&ui, &MenuAction::WorkspaceWorktrees);
    ui.activate_action_menu_row(&source, token, index);
    assert!(ui.take_action_menu_action(false).is_none(), "a submenu");
    first_terminal_moves(&mut ui, "deeper", "C:/first-repository/sub/deeper");
    let view = ui.view();
    assert_eq!(view.action_menu_cascade()[0].title, "deeper");
}

#[test]
fn ungrouped_git_workspace_menu_creates_a_worktree_from_the_workspace_so_the_server_groups_both() {
    use crate::action_menu::MenuUnavailable;
    use crate::workbench_state::worktree_launch::WorktreeMode;
    let (mut ui, _, target) = ungrouped_git_workspace();
    // Both Worktree rows form a section of their own and wait for Git discovery.
    {
        let view = ui.view();
        let menu = view.action_menu().unwrap();
        assert_eq!(
            menu.actions,
            [
                MenuAction::WorkspaceProjects,
                MenuAction::RenameWorkspace,
                MenuAction::NewWorktree,
                MenuAction::WorkspaceWorktrees,
                MenuAction::DeleteWorkspace,
            ]
        );
        let separators = (0..menu.result_actions.len())
            .map(|position| menu.result_separator_before(menu.result_actions, position))
            .collect::<Vec<_>>();
        assert_eq!(separators, [false, false, true, false, true]);
    }
    for action in [MenuAction::NewWorktree, MenuAction::WorkspaceWorktrees] {
        assert_eq!(
            unavailable(&ui, &action),
            Some(MenuUnavailable::WorktreeChecking)
        );
    }
    // A source without Git keeps them disabled.
    settle(&mut ui, Err(WorktreeNotice::ListingUnavailable));
    for action in [MenuAction::NewWorktree, MenuAction::WorkspaceWorktrees] {
        assert_eq!(
            unavailable(&ui, &action),
            Some(MenuUnavailable::WorktreeUnavailable)
        );
    }
    ui.cancel_action_menu();

    assert!(ui.open_action_menu(target, None, false));
    let original = Origin::capture(ui.state.as_ref());
    let discovery = workspace_discovery(&ui);
    settle(&mut ui, Ok(Box::new(discovery)));
    assert_eq!(unavailable(&ui, &MenuAction::NewWorktree), None);
    let token = draw(&mut ui);
    let source = ui.view().action_menu().unwrap().source.clone();
    let index = row(&ui, &MenuAction::NewWorktree);
    ui.activate_action_menu_row(&source, token, index);
    assert!(ui.take_action_menu_action(false).is_none());
    assert!(!ui.action_menu_active());
    assert_eq!(
        Origin::capture(ui.state.as_ref()),
        original,
        "no navigation"
    );
    let origin = ui.state.as_ref().unwrap().worktree_origin().unwrap();
    let launcher = &mut ui.worktrees.launcher;
    assert_eq!(launcher.view().unwrap().mode(), WorktreeMode::Create);
    let token = launcher.token().unwrap();
    launcher.draw(Some(token));
    let plan = launcher
        .create_with_defaults(
            orchbee_workbench_protocol::WorkbenchRequestId::parse(&format!("{:032x}", 901))
                .unwrap(),
            &origin,
            crate::localization::Locale::EnUs,
        )
        .unwrap();
    // A Workspace source with no Project lets the Server's Alongside placement create the
    // Project holding this Workspace and the new one in the same commit.
    assert_eq!(plan.project, None);
    assert!(matches!(
        &plan.intent,
        orchbee_workbench_protocol::WorktreeIntent::Create {
            source: orchbee_workbench_protocol::WorktreeSource::Workspace { workspace_id },
            ..
        } if workspace_id.as_str() == format!("{:032x}", 80)
    ));
}

#[test]
fn open_worktree_submenu_lists_the_other_checkouts_and_opens_the_chosen_one_alongside() {
    use crate::workbench_state::worktree_launch::WorktreeMode;
    let (mut ui, mailbox, _) = ungrouped_git_workspace();
    let original = Origin::capture(ui.state.as_ref());
    let discovery = workspace_discovery(&ui);
    settle(&mut ui, Ok(Box::new(discovery.clone())));
    assert_eq!(unavailable(&ui, &MenuAction::WorkspaceWorktrees), None);
    let token = draw(&mut ui);
    let source = ui.view().action_menu().unwrap().source.clone();
    let index = row(&ui, &MenuAction::WorkspaceWorktrees);
    ui.activate_action_menu_row(&source, token, index);
    assert!(ui.take_action_menu_action(false).is_none(), "a submenu");
    let linked = WorkbenchDirectoryPath::parse("C:/trees/猫").unwrap();
    {
        let view = ui.view();
        assert_eq!(
            view.action_menu_cascade().len(),
            1,
            "the Workspace menu stays"
        );
        let menu = view.action_menu().unwrap();
        assert!(matches!(menu.source, MenuTarget::WorkspaceWorktrees(_)));
        // The Workspace's own checkout would only reuse it, so the list leaves it out.
        assert_eq!(menu.actions, [MenuAction::OpenWorktreeAt(linked.clone())]);
        assert_eq!(menu.labels[0].as_deref(), Some("same"));
    }
    let token = draw(&mut ui);
    let source = ui.view().action_menu().unwrap().source.clone();
    ui.activate_action_menu_row(&source, token, 0);
    assert!(ui.take_action_menu_action(false).is_none());
    assert!(!ui.action_menu_active());
    assert_eq!(
        Origin::capture(ui.state.as_ref()),
        original,
        "no navigation"
    );
    let origin = ui.state.as_ref().unwrap().worktree_origin().unwrap();
    let launcher = &mut ui.worktrees.launcher;
    assert_eq!(launcher.view().unwrap().mode(), WorktreeMode::Open);
    let token = launcher.token().unwrap();
    launcher.draw(Some(token));
    let plan = launcher
        .confirm(
            token,
            orchbee_workbench_protocol::WorkbenchRequestId::parse(&format!("{:032x}", 902))
                .unwrap(),
            &origin,
        )
        .unwrap();
    assert_eq!(plan.project, None);
    assert!(matches!(
        &plan.intent,
        orchbee_workbench_protocol::WorktreeIntent::Open {
            source: orchbee_workbench_protocol::WorktreeSource::Workspace { .. },
            selector: orchbee_workbench_protocol::WorktreeSelector::Path { path },
            ..
        } if *path == linked
    ));
    assert!(mailbox.try_recv().unwrap().is_none());

    // Without another checkout the submenu has nothing to offer.
    let mut only = discovery;
    only.discovery.entries.truncate(1);
    let (mut ui, _, _) = ungrouped_git_workspace();
    settle(&mut ui, Ok(Box::new(only)));
    assert_eq!(
        unavailable(&ui, &MenuAction::WorkspaceWorktrees),
        Some(crate::action_menu::MenuUnavailable::NoOtherWorktrees)
    );
}
