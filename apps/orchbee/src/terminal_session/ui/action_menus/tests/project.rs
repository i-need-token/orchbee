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

fn confirm_quick(ui: &mut NativeUi, project: &Project, keyboard: bool) -> ClientAction {
    assert!(open(ui, project));
    let row = ui
        .view()
        .action_menu()
        .unwrap()
        .actions
        .iter()
        .position(|action| *action == MenuAction::QuickWorkspace)
        .unwrap();
    assert_eq!(ui.view().action_menu().unwrap().unavailable[row], None);
    if keyboard {
        for _ in 0..row {
            ui.capture_action_menu_key(KeyEvent::press(KeyCode::Down, Modifiers::NONE));
        }
        arm(ui);
        ui.capture_action_menu_key(KeyEvent::press(KeyCode::Enter, Modifiers::NONE));
    } else {
        let token = arm(ui);
        ui.click_action_menu(token, row);
    }
    let action = ui.take_action_menu_action(false).unwrap();
    assert!(matches!(
        action,
        ClientAction::RequestProjectQuickWorkspace(_)
    ));
    assert_eq!(ui.take_action_menu_action(false), None);
    action
}

#[test]
fn project_quick_menu_keeps_exact_target_through_preparation_and_one_confirmation() {
    for (source, target, empty) in [
        (Some(0), 0, false),
        (Some(0), 1, false),
        (Some(0), 1, true),
        (None, 1, false),
        (None, 1, true),
    ] {
        for keyboard in [false, true] {
            let mut snapshot = snapshot();
            snapshot.terminal_contexts[0].tabs.clear();
            let location = source.map_or_else(
                || {
                    NativeContextLocation::Standalone(
                        snapshot.navigation.standalone_terminal_context_id.clone(),
                    )
                },
                |index| workspace(&snapshot.navigation.projects[index]),
            );
            if empty {
                let context = snapshot.navigation.projects[target]
                    .workspaces
                    .remove(0)
                    .terminal_context_id;
                snapshot
                    .terminal_contexts
                    .retain(|candidate| candidate.terminal_context_id != context);
            }
            let project = snapshot.navigation.projects[target].clone();
            assert_eq!(project.label, snapshot.navigation.projects[0].label);
            let (mut ui, mailbox) = ready(&snapshot, &location);
            let clock = ui.clock.clone();
            let origin = Origin::capture(ui.state.as_ref());
            let cursors = mailbox.cursors();
            let action = confirm_quick(&mut ui, &project, keyboard);
            assert_eq!(Origin::capture(ui.state.as_ref()), origin);
            assert_eq!(mailbox.cursors(), cursors);
            assert!(mailbox.try_recv().unwrap().is_none());
            ui.apply_client_action(action);
            crate::terminal_session::ui::quick_workspace_tests::finish_preparation(
                &mut ui, &mailbox, &clock, &snapshot, false,
            );
            let token = ui.view().setup_draft().unwrap().token();
            assert_eq!(
                ui.view().setup_draft().unwrap().stage(),
                NativeSetupStage::QuickName
            );
            assert_eq!(
                ui.view().setup_draft().unwrap().project_label(),
                Some(project.label.as_str())
            );
            assert_eq!(Origin::capture(ui.state.as_ref()), origin);
            ui.edit_setup_draft(token, NativeSetupEdit::SelectNextProject);
            ui.edit_setup_draft(token, NativeSetupEdit::Clear);
            ui.edit_setup_draft(token, NativeSetupEdit::Append("chosen name"));
            ui.edit_setup_draft(token, NativeSetupEdit::NextField);
            let token = ui.view().setup_draft().unwrap().token();
            ui.edit_setup_draft(token, NativeSetupEdit::Clear);
            ui.edit_setup_draft(token, NativeSetupEdit::Append("child"));
            ui.submit_setup_draft(token);
            assert!(mailbox.try_recv().unwrap().is_none());
            ui.after_setup_draw(Some(ModalPresentation::Presented(token)));
            ui.submit_setup_draft(token);
            let lease = mailbox.try_recv().unwrap().unwrap();
            let QueuedCommand::WorkspaceSetup(marker) = lease.record() else {
                panic!("one launch");
            };
            assert!(matches!(&marker.0.command.action,
                WorkbenchAction::WorkspaceLaunch { project_id, label, options, launch: None, profile: None, .. }
                if project_id == &project.project_id && label.as_str() == "chosen name"
                    && options.cwd.as_ref().unwrap().as_str() == "C:/项目 space\\child"));
            assert_eq!(ui.state.as_ref().unwrap().cache().location(), &location);
            ui.submit_setup_draft(token);
            assert!(mailbox.try_recv().unwrap().is_none());
            mailbox.close(CarrierFailure::WorkerExited);
        }
    }
}

#[test]
fn project_quick_cancel_preparation_or_form_never_creates_or_navigates() {
    for during_preparation in [false, true] {
        let mut snapshot = snapshot();
        let context = snapshot.navigation.projects[1]
            .workspaces
            .remove(0)
            .terminal_context_id;
        snapshot
            .terminal_contexts
            .retain(|candidate| candidate.terminal_context_id != context);
        let location = workspace(&snapshot.navigation.projects[0]);
        let (mut ui, mailbox) = ready(&snapshot, &location);
        let clock = ui.clock.clone();
        let origin = Origin::capture(ui.state.as_ref());
        let action = confirm_quick(&mut ui, &snapshot.navigation.projects[1], false);
        ui.apply_client_action(action);
        crate::terminal_session::ui::quick_workspace_tests::finish_preparation(
            &mut ui,
            &mailbox,
            &clock,
            &snapshot,
            during_preparation,
        );
        if !during_preparation {
            let token = ui.view().setup_draft().unwrap().token();
            ui.cancel_setup_draft(token);
        }
        assert!(ui.view().setup_draft().is_none());
        assert_eq!(Origin::capture(ui.state.as_ref()), origin);
        assert!(mailbox.try_recv().unwrap().is_none());
    }
}

#[test]
fn project_quick_handoff_expiry_refuses_but_form_editing_does_not_expire() {
    use std::sync::atomic::Ordering;
    for expired_before_acceptance in [false, true] {
        let snapshot = snapshot();
        let location = workspace(&snapshot.navigation.projects[0]);
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
            crate::terminal_session::ui::quick_workspace_tests::finish_preparation(
                &mut ui, &mailbox, &clock, &snapshot, false,
            );
            elapsed.store(120_000, Ordering::Relaxed);
            let token = ui.view().setup_draft().unwrap().token();
            ui.after_setup_draw(Some(ModalPresentation::Presented(token)));
            ui.submit_setup_draft(token);
            let lease = mailbox.try_recv().unwrap().unwrap();
            let QueuedCommand::WorkspaceSetup(marker) = lease.record() else {
                panic!("launch after editing");
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
            let location = workspace(&snapshot.navigation.projects[0]);
            let target = snapshot.navigation.projects[1].clone();
            let (mut ui, mailbox) = ready(&snapshot, &location);
            assert!(open(&mut ui, &target));
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
                ui.click_action_menu(token, row);
            }
            let action = (stage == 2).then(|| ui.take_action_menu_action(false).unwrap());
            if removed {
                let project = snapshot.navigation.projects.pop().unwrap();
                snapshot.terminal_contexts.retain(|context| {
                    !project.workspaces.iter().any(|workspace| {
                        workspace.terminal_context_id == context.terminal_context_id
                    })
                });
            } else {
                snapshot.navigation.projects[1].label = "Replacement".into();
            }
            install(&mut ui, &snapshot, &location);
            if let Some(action) = action {
                ui.apply_client_action(action);
            } else {
                if stage == 0 {
                    ui.click_action_menu(token, row);
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
fn project_menu_uses_current_workspace_and_hands_original_actions_once() {
    for current in 0..2 {
        for (menu_action, expected) in [
            (
                MenuAction::NewWorkspace,
                ClientAction::RequestWorkspaceCreate,
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
            let actions = ui.view().action_menu().unwrap().actions.to_vec();
            for (row, action) in actions.iter().enumerate() {
                if *action == MenuAction::QuickWorkspace {
                    continue;
                }
                assert_eq!(
                    ui.view().action_menu().unwrap().unavailable[row],
                    Some(MenuUnavailable::ProjectWorkspaceRequired)
                );
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
