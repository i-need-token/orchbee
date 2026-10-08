use super::*;
use crate::client_view::{KeyCode, Modifiers};
use crate::local_client::test_server::adjustable_operation_clock;
use crate::terminal_session::mailbox::EventDeadline;
use orchbee_workbench_protocol::{
    Directory, Project, SiteId, Snapshot, TerminalContext, WorkbenchDirectoryId,
    WorkbenchDirectoryPath, WorkbenchProjectId, WorkbenchRevision, WorkbenchTerminalContextId,
    WorkbenchWorkspaceId, WorkerId,
};
use std::sync::{Arc, atomic::AtomicU64};

fn snapshot() -> Snapshot {
    let mut snapshot = context_selection_tests::snapshot(true);
    let project = &mut snapshot.navigation.projects[0];
    let directory_id = WorkbenchDirectoryId::parse(&format!("{:032x}", 901)).unwrap();
    project.directories.push(Directory {
        directory_id: directory_id.clone(),
        worker_id: WorkerId::parse(&format!("{:032x}", 902)).unwrap(),
        path: WorkbenchDirectoryPath::parse("C:/project").unwrap(),
    });
    project.workspaces[0].primary_directory_id = Some(directory_id);
    let mut other = project.clone();
    other.project_id = WorkbenchProjectId::parse(&format!("{:032x}", 970)).unwrap();
    other.directories.clear();
    other.workspaces[0].primary_directory_id = None;
    other.workspaces[0].workspace_id =
        WorkbenchWorkspaceId::parse(&format!("{:032x}", 971)).unwrap();
    other.workspaces[0].terminal_context_id =
        WorkbenchTerminalContextId::parse(&format!("{:032x}", 972)).unwrap();
    snapshot.terminal_contexts.push(TerminalContext {
        terminal_context_id: other.workspaces[0].terminal_context_id.clone(),
        groups: Vec::new(),
        tabs: Vec::new(),
    });
    snapshot.navigation.projects.push(other);
    snapshot
}

fn workspace(project: &Project) -> NativeContextLocation {
    NativeContextLocation::Workspace {
        project_id: project.project_id.clone(),
        workspace_id: project.workspaces[0].workspace_id.clone(),
        terminal_context_id: project.workspaces[0].terminal_context_id.clone(),
    }
}

fn install(ui: &mut NativeUi, snapshot: &Snapshot, location: &NativeContextLocation) {
    let cache = crate::terminal_session::workbench_projection::project_native_context_tabs(
        snapshot, location,
    )
    .unwrap();
    ui.state = Some(
        NativeSessionState::bootstrap(cache, 10, Geometry::new(80, 24, 8, 16).unwrap())
            .unwrap()
            .0,
    );
}

fn ready(snapshot: &Snapshot, location: &NativeContextLocation) -> (NativeUi, Arc<CommandMailbox>) {
    let clock = adjustable_operation_clock(Arc::new(AtomicU64::new(0)));
    let mailbox = Arc::new(CommandMailbox::new(
        10,
        Geometry::new(80, 24, 8, 16).unwrap(),
    ));
    let mut ui = NativeUi::new(Arc::clone(&mailbox), clock.clone());
    // Draft admission requires the mailbox and UI to own the same context.
    // Bootstrap both owners instead of replacing only the ready UI's state.
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::Bootstrap {
                selection: None,
                cache: crate::terminal_session::workbench_projection::project_native_context_tabs(
                    snapshot, location,
                )
                .unwrap(),
            },
            EventDeadline::Startup(clock.now() + crate::terminal_session::state::OPERATION_TOTAL),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(matches!(
        mailbox.take_deposited_reply(&wait).unwrap(),
        Some(UiReply::BootstrapApplied(None))
    ));
    assert_eq!(ui.view().phase(), NativeDisplayPhase::Empty);
    assert_eq!(ui.state.as_ref().unwrap().cache().location(), location);
    (ui, mailbox)
}

fn open(ui: &mut NativeUi, project: &Project) -> bool {
    ui.open_action_menu(
        MenuTarget::Drawn {
            source: ui.view().action_menu_source().unwrap(),
            target: Box::new(MenuTarget::Project(project.project_id.clone())),
        },
        None,
        false,
    )
}

#[test]
fn project_menu_uses_current_workspace_and_hands_original_actions_once() {
    for current in 0..2 {
        for (menu_action, expected) in [
            (
                MenuAction::NewWorkspace,
                ClientAction::RequestWorkspaceCreate,
            ),
            (
                MenuAction::QuickWorkspace,
                ClientAction::RequestQuickWorkspace,
            ),
            (
                MenuAction::RenameProject,
                ClientAction::RequestProjectRename,
            ),
            (MenuAction::NewWorktree, ClientAction::RequestWorktreeCreate),
            (MenuAction::OpenWorktree, ClientAction::RequestWorktreeOpen),
        ] {
            let snapshot = snapshot();
            let project = &snapshot.navigation.projects[current];
            let location = workspace(project);
            let (mut ui, mailbox) = ready(&snapshot, &location);
            let origin = Origin::capture(ui.state.as_ref());
            let cursors = mailbox.cursors();
            assert!(open(&mut ui, project));
            ui.capture_action_menu_key(KeyEvent::press(KeyCode::Esc, Modifiers::NONE));
            assert!(!ui.action_menu_active());
            assert_eq!(Origin::capture(ui.state.as_ref()), origin);
            assert!(open(&mut ui, project));
            let row = ui
                .view()
                .action_menu()
                .unwrap()
                .actions
                .iter()
                .position(|action| *action == menu_action)
                .unwrap();
            let token = arm(&mut ui);
            ui.click_action_menu(token, row);
            assert_eq!(ui.take_action_menu_action(false), Some(expected.clone()));
            assert_eq!(ui.take_action_menu_action(false), None);
            assert_eq!(Origin::capture(ui.state.as_ref()), origin);
            assert_eq!(mailbox.cursors(), cursors);
            assert!(mailbox.try_recv().unwrap().is_none());
            if matches!(
                menu_action,
                MenuAction::NewWorkspace | MenuAction::RenameProject
            ) {
                ui.apply_client_action(expected);
                let view = ui.view();
                let draft = view.setup_draft().unwrap();
                if menu_action == MenuAction::RenameProject {
                    assert_eq!(draft.stage(), NativeSetupStage::ProjectRename);
                    assert_eq!(draft.text(), project.label);
                } else {
                    assert_eq!(draft.stage(), NativeSetupStage::ChooseProject);
                    assert!(draft.project_at(current).unwrap().selected());
                }
                assert_eq!(ui.state.as_ref().unwrap().cache().location(), &location);
                assert!(mailbox.try_recv().unwrap().is_none());
            }
        }
    }
}

#[test]
fn project_menu_never_guesses_context_for_empty_other_or_standalone_targets() {
    for standalone in [true, false] {
        for empty in [true, false] {
            let (mut ui, mailbox, _, _) = context_selection_tests::ready(true);
            let mut snapshot = snapshot();
            let target = usize::from(!standalone);
            let location = if standalone {
                NativeContextLocation::Standalone(
                    snapshot.navigation.standalone_terminal_context_id.clone(),
                )
            } else {
                workspace(&snapshot.navigation.projects[0])
            };
            if empty {
                let project = &mut snapshot.navigation.projects[target];
                let context = project.workspaces.remove(0).terminal_context_id;
                snapshot
                    .terminal_contexts
                    .retain(|candidate| candidate.terminal_context_id != context);
            }
            install(&mut ui, &snapshot, &location);
            let origin = Origin::capture(ui.state.as_ref());
            let cursors = mailbox.cursors();
            assert!(open(&mut ui, &snapshot.navigation.projects[target]));
            let count = ui.view().action_menu().unwrap().actions.len();
            assert!(
                ui.view()
                    .action_menu()
                    .unwrap()
                    .unavailable
                    .iter()
                    .all(|reason| *reason == Some(MenuUnavailable::ProjectWorkspaceRequired))
            );
            for row in 0..count {
                let token = arm(&mut ui);
                ui.click_action_menu(token, row);
                arm(&mut ui);
                ui.capture_action_menu_key(KeyEvent::press(KeyCode::Enter, Modifiers::NONE));
                assert!(ui.action_menu_active());
                assert_eq!(ui.menus.menu.confirmed(), None);
                assert_eq!(ui.take_action_menu_action(false), None);
                assert_eq!(Origin::capture(ui.state.as_ref()), origin);
                assert_eq!(mailbox.cursors(), cursors);
                assert!(mailbox.try_recv().unwrap().is_none());
            }
            ui.cancel_action_menu();
            assert_eq!(Origin::capture(ui.state.as_ref()), origin);
        }
    }
}

#[test]
fn project_menu_rejects_stale_drawn_and_replaced_sources_before_confirmation_or_handoff() {
    let (mut ui, _, _, _) = context_selection_tests::ready(true);
    let original = snapshot();
    let project = &original.navigation.projects[0];
    install(&mut ui, &original, &workspace(project));
    for wrong_site in [false, true] {
        let mut source = ui.view().action_menu_source().unwrap();
        if wrong_site {
            source.site = SiteId::parse(&format!("{:032x}", 999)).unwrap();
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
            let (mut ui, mailbox, _, _) = context_selection_tests::ready(true);
            install(&mut ui, &original, &workspace(project));
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
                ui.click_action_menu(token, row);
                assert!(matches!(
                    ui.menus.intent.as_ref().unwrap().stage,
                    Stage::Offered(_, _)
                ));
            }
            let mut replacement = original.clone();
            replacement.revision = WorkbenchRevision::new(original.revision.value() + 1);
            let mut location = workspace(project);
            match change {
                0 => replacement.navigation.projects[0].label = "Renamed".into(),
                1 => {
                    replacement.navigation.projects[0].directories[0].path =
                        WorkbenchDirectoryPath::parse("C:/replacement").unwrap();
                }
                2 => {
                    replacement.navigation.projects[0].directories[0].worker_id =
                        WorkerId::parse(&format!("{:032x}", 903)).unwrap();
                }
                3 => {
                    let id = WorkbenchTerminalContextId::parse(&format!("{:032x}", 904)).unwrap();
                    replacement.terminal_contexts[1].terminal_context_id = id.clone();
                    replacement.navigation.projects[0].workspaces[0].terminal_context_id = id;
                    location = workspace(&replacement.navigation.projects[0]);
                }
                4 => {
                    replacement.navigation.projects.remove(0);
                    location = NativeContextLocation::Standalone(
                        replacement
                            .navigation
                            .standalone_terminal_context_id
                            .clone(),
                    );
                }
                _ => location = workspace(&replacement.navigation.projects[1]),
            }
            install(&mut ui, &replacement, &location);
            let origin = Origin::capture(ui.state.as_ref());
            if !confirmed {
                ui.click_action_menu(token, row);
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
                (16, 7),
                (12, 3),
            ] {
                let (mut ui, _, _, _) = context_selection_tests::ready(true);
                let snapshot = snapshot();
                let location = NativeContextLocation::Standalone(
                    snapshot.navigation.standalone_terminal_context_id.clone(),
                );
                install(&mut ui, &snapshot, &location);
                ui.observe_geometry(Geometry::new(width, height, 8, 16).unwrap());
                assert!(open(&mut ui, &snapshot.navigation.projects[0]));
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                let mut receipt = NativeDrawReceipt::default();
                terminal
                    .draw(|frame| {
                        receipt = crate::tui_render::render_terminal_shell(
                            frame,
                            &crate::client_view::ClientView::new(width),
                            locale,
                            Some(crate::terminal_session::TerminalSessionView::Native(
                                ui.view(),
                            )),
                            &theme,
                            &mut HitMapBuilder::new(),
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
                let reason =
                    locale.menu_unavailable(MenuUnavailable::ProjectWorkspaceRequired, true);
                assert!(text.replace(' ', "").contains(&reason.replace(' ', "")));
                assert!(text.contains("Esc"));
                assert!(text.contains("1/5"));
                assert!(!text.contains("Enter"));
                if width >= 80 {
                    assert!(text.contains(locale.menu_action(MenuAction::NewWorkspace)));
                }
                if width == 16 {
                    let selected = text.lines().find(|line| line.contains("> ")).unwrap();
                    assert!(selected.contains(theme.glyphs.ellipsis));
                }
            }
        }
    }
}
