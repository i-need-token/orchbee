use super::super::{context_selection_tests, move_membership_tests, publication_hint_tests};
use super::*;
use crate::{localization::Locale, tui_pointer::HitMapBuilder, tui_theme::Theme};
use ratatui::{Terminal, backend::TestBackend};

fn arm(ui: &mut NativeUi) -> MenuToken {
    let token = ui.view().action_menu().unwrap().token;
    ui.after_action_menu_draw(Some(ModalPresentation::Presented(MenuDraw {
        token,
        rows: 64,
    })));
    token
}

#[test]
fn pane_open_and_cancel_preserve_focus_then_one_confirmed_action_focuses_exact_peer() {
    let (mut ui, mailbox, clock, _) = publication_hint_tests::ready_split_ui();
    publication_hint_tests::attach_split_peer(&mut ui, &mailbox, &clock);
    let state = ui.state.as_ref().unwrap();
    let old = state.focus_pane_ref().unwrap();
    let peer = state.cache().tabs()[0].panel(1).unwrap().panel_id();
    let peer = state.pane_by_panel(peer).unwrap().pane_ref();
    assert!(ui.open_action_menu(MenuTarget::Pane(peer.clone()), Some((9, 9)), false));
    assert_eq!(
        ui.state.as_ref().unwrap().focus_pane_ref(),
        Some(old.clone())
    );
    arm(&mut ui);
    ui.cancel_action_menu();
    assert_eq!(ui.state.as_ref().unwrap().focus_pane_ref(), Some(old));
    assert_eq!(ui.take_action_menu_action(false), None);
    assert!(ui.open_action_menu(MenuTarget::Pane(peer.clone()), None, false));
    let token = arm(&mut ui);
    ui.click_action_menu(token, 6);
    assert_eq!(ui.state.as_ref().unwrap().focus_pane_ref(), Some(peer));
    assert_eq!(
        ui.take_action_menu_action(false),
        Some(ClientAction::ClosePanel)
    );
    assert_eq!(ui.take_action_menu_action(false), None);
    assert!(!ui.action_menu_active());
    // Handoff only selected the existing close action, never confirmed or submitted it.
    assert!(ui.view().panel_close_confirmation().is_none());
    ui.apply_client_action(ClientAction::ClosePanel);
    assert!(ui.view().panel_close_confirmation().is_some());
}

#[test]
fn exact_incarnation_modal_priority_geometry_and_timeout_revoke_unhanded_intent() {
    let (mut ui, _, _, elapsed) = move_membership_tests::ready_ui();
    let focus = ui.state.as_ref().unwrap().focus_pane_ref().unwrap();
    let stale = PaneRef::new(focus.pane().clone(), focus.incarnation() + 1);
    assert!(!ui.open_action_menu(MenuTarget::Pane(stale), None, false));
    ui.state.as_mut().unwrap().open_rename_draft().unwrap();
    assert!(!ui.open_action_menu(MenuTarget::Global, None, false));
    ui.apply_client_action(ClientAction::CancelClose);
    // Use an independently ready UI: the original rename owner retains cancellation semantics.
    let (mut ui, _, _, _) = move_membership_tests::ready_ui();
    assert!(ui.open_action_menu(MenuTarget::Global, None, false));
    arm(&mut ui);
    ui.observe_geometry(Geometry::new(120, 40, 8, 16).unwrap());
    assert!(!ui.action_menu_active());
    assert_eq!(ui.take_action_menu_action(false), None);
    assert!(ui.open_action_menu(MenuTarget::Global, None, false));
    arm(&mut ui);
    ui.cancel_action_menu();
    assert!(ui.open_action_menu(MenuTarget::Global, None, false));
    // Deadline tests use the fixture's injected clock, never wall-clock sleep.
    let (mut timed, _, _, time) = move_membership_tests::ready_ui();
    assert!(timed.open_action_menu(MenuTarget::Global, None, false));
    arm(&mut timed);
    time.store(31_000, std::sync::atomic::Ordering::SeqCst);
    assert!(timed.drive_action_menu());
    assert!(!timed.action_menu_active());
    assert_eq!(elapsed.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn empty_workspace_navigation_uses_owned_refresh_and_exact_cleanup_then_original_rename() {
    let (mut ui, mailbox, clock, _) = context_selection_tests::ready(true);
    let location = ui.state.as_ref().unwrap().cache().navigation()[1]
        .location
        .clone();
    let old = Origin::capture(ui.state.as_ref());
    assert!(ui.open_action_menu(MenuTarget::Workspace(location.clone()), None, false));
    assert_eq!(Origin::capture(ui.state.as_ref()), old);
    let token = arm(&mut ui);
    ui.click_action_menu(token, 3);
    let Stage::Context { generation, .. } = ui.menus.intent.as_ref().unwrap().stage else {
        panic!("owned refresh");
    };
    ui.menu_navigation_prepared(NavigationKey::Context(generation + 1), true);
    ui.menu_navigation_installed(NavigationKey::Context(generation + 1));
    assert_eq!(ui.take_action_menu_action(false), None);
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::Refresh(marker) = lease.record() else {
        panic!("one refresh");
    };
    assert_eq!(marker.generation(), generation);
    mailbox.mark_refresh_in_flight(generation).unwrap();
    let wait = mailbox
        .publish_refresh_result_before(
            generation,
            orchbee_workbench_protocol::WorkbenchSnapshotOutcome::Available {
                snapshot: context_selection_tests::snapshot(true),
            },
            marker.deadline(),
            &clock,
        )
        .unwrap();
    ui.tick();
    let Some(UiReply::Refresh(reply)) = mailbox.take_deposited_reply(&wait).unwrap() else {
        panic!("refresh reply");
    };
    let RefreshDecision::Cleanup(cleanup) = reply.decision() else {
        panic!("old viewer cleanup");
    };
    assert_eq!(ui.take_action_menu_action(false), None);
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::MembershipDetached {
                cleanup: cleanup.clone(),
                detached: cleanup.old().clone(),
            },
            super::super::super::mailbox::EventDeadline::TotalCleanup(cleanup.deadlines().total()),
            &clock,
        )
        .unwrap();
    ui.tick();
    mailbox.take_deposited_reply(&wait).unwrap().unwrap();
    mailbox.finish_lease(lease).unwrap();
    assert_eq!(ui.state.as_ref().unwrap().cache().location(), &location);
    assert_eq!(ui.view().phase(), NativeDisplayPhase::Empty);
    let action = ui
        .take_action_menu_action(false)
        .expect("original action does not require Ready");
    assert_eq!(action, ClientAction::RequestWorkspaceRename);
    ui.apply_client_action(action);
    assert!(ui.view().setup_draft().is_some());
}

#[test]
fn toggles_recheck_frozen_prerequisites_and_foreign_navigation_drops_the_intent() {
    let (mut ui, _, _, _) = move_membership_tests::ready_ui();
    assert!(ui.open_action_menu(MenuTarget::Global, None, false));
    let token = arm(&mut ui);
    let release = ui
        .view()
        .action_menu()
        .unwrap()
        .actions
        .iter()
        .position(|action| *action == MenuAction::ReleasePointer)
        .unwrap();
    ui.click_action_menu(token, release);
    assert_eq!(ui.take_action_menu_action(true), None);
    assert!(!ui.action_menu_active());
    assert!(ui.open_action_menu(MenuTarget::Global, None, false));
    arm(&mut ui);
    ui.apply_client_action(ClientAction::SelectTab(2));
    assert!(!ui.action_menu_active());
    assert_eq!(ui.take_action_menu_action(false), None);
}

#[test]
fn action_menu_renders_and_hits_only_presented_rows_in_both_locales_themes_and_sizes() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [
            Theme::terminal_native(),
            Theme::rgb_test_dark(),
            Theme::one_light(),
            Theme::one_dark(),
        ] {
            for (width, height) in [(120, 40), (80, 24), (80, 20), (50, 10), (12, 3)] {
                let (mut ui, _, _, _) = move_membership_tests::ready_ui();
                let geometry = Geometry::new(width, height, 8, 16).unwrap();
                let mut snapshot = context_selection_tests::snapshot(true);
                snapshot.navigation.projects[0].workspaces[0].label = "猫 🐝 workspace".into();
                let cache =
                    crate::terminal_session::workbench_projection::project_native_standalone_tabs(
                        &snapshot,
                    )
                    .unwrap();
                let location = cache.navigation()[1].location.clone();
                ui.state = Some(
                    NativeSessionState::bootstrap(cache, 10, geometry)
                        .unwrap()
                        .0,
                );
                ui.observe_geometry(geometry);
                assert!(ui.open_action_menu(
                    MenuTarget::Workspace(location),
                    Some((width - 1, height - 1)),
                    false
                ));
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                let mut hits = HitMapBuilder::new();
                let mut receipt = NativeDrawReceipt::default();
                let mut view = crate::client_view::ClientView::new(width);
                view.commit(
                    view.decide_action(ClientAction::OpenShortcutHelp),
                    SessionActionDisposition::KeepNavigator,
                );
                terminal
                    .draw(|frame| {
                        receipt = crate::tui_render::render_terminal_shell(
                            frame,
                            &view,
                            locale,
                            Some(crate::terminal_session::TerminalSessionView::Native(
                                ui.view(),
                            )),
                            &theme,
                            &mut hits,
                        );
                    })
                    .unwrap();
                let text = terminal
                    .backend()
                    .buffer()
                    .content()
                    .iter()
                    .map(|c| c.symbol())
                    .collect::<String>();
                ui.after_close_draw(receipt);
                if width == 12 {
                    assert!(!ui.action_menu_active());
                } else {
                    assert!(
                        text.replace(' ', "")
                            .contains(&locale.menu_title().replace(' ', "")),
                        "{locale:?} {width}x{height}: {text}"
                    );
                    assert!(text.contains("Esc"));
                    assert!(
                        text.contains('猫'),
                        "wide title {locale:?} {width}x{height}"
                    );
                    assert!(ui.menus.menu.confirm());
                    let map = hits.finish();
                    let rows = (0..height)
                        .flat_map(|y| (0..width).map(move |x| (x, y)))
                        .filter(|(x, y)| {
                            matches!(
                                map.hit_target_at(*x, *y),
                                Some(crate::tui_pointer::HitTarget::ActionMenuRow { .. })
                            )
                        })
                        .count();
                    assert!(rows > 0);
                }
                ui.cancel_action_menu();
                assert_eq!(
                    view.overlay(),
                    Some(crate::client_view::Overlay::ShortcutHelp)
                );
            }
        }
    }
}

#[test]
fn drawn_snapshot_source_and_owned_workspace_directory_replacement_are_fenced() {
    use orchbee_workbench_protocol::{
        Directory, WorkbenchDirectoryId, WorkbenchDirectoryPath, WorkbenchRevision, WorkerId,
    };
    let (mut ui, mailbox, clock, _) = context_selection_tests::ready(true);
    let location = ui.state.as_ref().unwrap().cache().navigation()[1]
        .location
        .clone();
    let mut source = ui.view().action_menu_source().unwrap();
    source.revision = WorkbenchRevision::new(source.revision.value() + 1);
    assert!(!ui.open_action_menu(
        MenuTarget::Drawn {
            source,
            target: Box::new(MenuTarget::Workspace(location.clone()))
        },
        None,
        false
    ));
    assert!(ui.open_action_menu(MenuTarget::Workspace(location), None, false));
    let token = arm(&mut ui);
    ui.click_action_menu(token, 4);
    let Stage::Context { generation, .. } = ui.menus.intent.as_ref().unwrap().stage else {
        panic!("owned generation");
    };
    let mut replacement = context_selection_tests::snapshot(true);
    replacement.revision = WorkbenchRevision::new(2);
    let directory_id = WorkbenchDirectoryId::parse("00000000000000000000000000000901").unwrap();
    let project = &mut replacement.navigation.projects[0];
    project.directories.push(Directory {
        directory_id: directory_id.clone(),
        worker_id: WorkerId::parse("00000000000000000000000000000902").unwrap(),
        path: WorkbenchDirectoryPath::parse("C:/other-project").unwrap(),
    });
    project.workspaces[0].primary_directory_id = Some(directory_id);
    let baseline = Origin::capture(ui.state.as_ref());
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::Refresh(marker) = lease.record() else {
        panic!("refresh");
    };
    mailbox.mark_refresh_in_flight(generation).unwrap();
    let wait = mailbox
        .publish_refresh_result_before(
            generation,
            orchbee_workbench_protocol::WorkbenchSnapshotOutcome::Available {
                snapshot: replacement,
            },
            marker.deadline(),
            &clock,
        )
        .unwrap();
    ui.tick();
    let Some(UiReply::Refresh(reply)) = mailbox.take_deposited_reply(&wait).unwrap() else {
        panic!("refresh reply");
    };
    assert!(
        !matches!(reply.decision(), RefreshDecision::Cleanup(_)),
        "replacement refused before old viewer cleanup"
    );
    assert_eq!(Origin::capture(ui.state.as_ref()), baseline);
    mailbox.finish_lease(lease).unwrap();
    assert!(!ui.action_menu_active());
    assert_eq!(ui.take_action_menu_action(false), None);
}

#[test]
fn carrier_retirement_revokes_members_and_stateless_global_menu_observes_geometry() {
    for row in [3, 4, 5] {
        let (mut ui, _, _, _) = move_membership_tests::ready_ui();
        let pane = ui.state.as_ref().unwrap().focus_pane_ref().unwrap();
        assert!(ui.open_action_menu(MenuTarget::Pane(pane.clone()), None, false));
        let token = arm(&mut ui);
        ui.click_action_menu(token, row);
        ui.retire(CarrierFailure::Closed);
        assert!(!ui.action_menu_active());
        assert_eq!(ui.take_action_menu_action(false), None);
        assert!(!ui.open_action_menu(MenuTarget::Pane(pane), None, false));
        assert!(ui.open_action_menu(MenuTarget::Global, None, false));
        assert_eq!(
            ui.view().action_menu().unwrap().actions,
            &[
                MenuAction::Help,
                MenuAction::Sidebar,
                MenuAction::ReleasePointer,
                MenuAction::Detach
            ]
        );
    }
    let (mut ui, _, _, _) = move_membership_tests::ready_ui();
    ui.state = None;
    ui.menus.geometry = None;
    assert!(!ui.open_action_menu(MenuTarget::Global, None, false));
    ui.observe_geometry(Geometry::new(80, 24, 8, 16).unwrap());
    assert!(ui.open_action_menu(MenuTarget::Global, None, false));
    arm(&mut ui);
    ui.observe_geometry(Geometry::new(120, 40, 8, 16).unwrap());
    assert!(!ui.action_menu_active());
    assert_eq!(ui.take_action_menu_action(false), None);
}

#[test]
fn equal_numeric_navigation_generations_cannot_cross_context_and_tab_domains() {
    for context in [false, true] {
        let (mut ui, _, _, _) = context_selection_tests::ready(true);
        let target = if context {
            MenuTarget::Workspace(
                ui.state.as_ref().unwrap().cache().navigation()[1]
                    .location
                    .clone(),
            )
        } else {
            MenuTarget::Tab(
                ui.state.as_ref().unwrap().cache().tabs()[1]
                    .tab_id()
                    .clone(),
            )
        };
        assert!(ui.open_action_menu(target, None, false));
        let token = arm(&mut ui);
        ui.click_action_menu(token, 0);
        let generation = match ui.menus.intent.as_ref().unwrap().stage {
            Stage::Context { generation, .. } | Stage::Tab { generation, .. } => generation,
            _ => panic!("navigation pending"),
        };
        let foreign = if context {
            NavigationKey::Tab(generation)
        } else {
            NavigationKey::Context(generation)
        };
        ui.menu_navigation_prepared(foreign, true);
        ui.menu_navigation_installed(foreign);
        assert!(matches!(
            ui.menus.intent.as_ref().unwrap().stage,
            Stage::Context {
                prepared: false,
                ..
            } | Stage::Tab {
                prepared: false,
                ..
            }
        ));
        assert_eq!(ui.take_action_menu_action(false), None);
        let own = if context {
            NavigationKey::Context(generation)
        } else {
            NavigationKey::Tab(generation)
        };
        ui.menu_navigation_prepared(own, true);
        assert!(matches!(
            ui.menus.intent.as_ref().unwrap().stage,
            Stage::Context { prepared: true, .. } | Stage::Tab { prepared: true, .. }
        ));
        ui.menu_navigation_installed(foreign);
        assert!(matches!(
            ui.menus.intent.as_ref().unwrap().stage,
            Stage::Context { prepared: true, .. } | Stage::Tab { prepared: true, .. }
        ));
        assert_eq!(ui.take_action_menu_action(false), None);
    }
}
