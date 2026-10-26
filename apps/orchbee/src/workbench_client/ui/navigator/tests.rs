use super::super::tests::context_selection::ready;
use super::*;
use crate::workbench_client::mailbox::QueuedCommand;
use orchbee_workbench_protocol::WorkbenchRevision;

fn row(ui: &NativeUi, target: Target) -> Row {
    Row {
        episode: 1,
        source: ui.navigator_catalog().unwrap().source,
        target,
    }
}

#[test]
fn exact_current_is_distinct_from_busy_and_consumes_no_request() {
    let (mut ui, mailbox, _, _) = ready(false);
    let catalog = ui.navigator_catalog().unwrap();
    for target in [
        Target::Location(catalog.source.location.clone()),
        Target::Tab(catalog.current_tab.clone().unwrap()),
    ] {
        let proof = row(&ui, target);
        assert_eq!(ui.activate_navigator(&proof), Admission::ExactCurrent);
        assert!(mailbox.try_recv().unwrap().is_none());
    }
    ui.apply_client_action(ClientAction::OpenWorkbenchNavigator);
    let proof = row(&ui, Target::Tab(catalog.current_tab.unwrap()));
    assert_eq!(ui.activate_navigator(&proof), Admission::Refused);
    let lease = mailbox.try_recv().unwrap().unwrap();
    assert!(matches!(lease.record(), QueuedCommand::Refresh(_)));
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn an_exact_current_row_does_not_bypass_closed_mailbox_admission() {
    let (mut ui, mailbox, _, _) = ready(false);
    let catalog = ui.navigator_catalog().unwrap();
    let proof = row(&ui, Target::Location(catalog.source.location));
    mailbox.close(super::super::CarrierFailure::Closed);
    assert_eq!(ui.activate_navigator(&proof), Admission::Refused);
}

#[test]
fn stale_source_refuses_and_exact_location_admits_only_one_original_refresh() {
    let (mut ui, mailbox, _, _) = ready(false);
    let target = Target::Location(ui.navigator_catalog().unwrap().locations[1].clone());
    let mut stale = row(&ui, target.clone());
    stale.source.revision = WorkbenchRevision::new(999);
    assert_eq!(ui.activate_navigator(&stale), Admission::Refused);
    assert!(mailbox.try_recv().unwrap().is_none());
    let proof = row(&ui, target);
    assert!(matches!(
        ui.activate_navigator(&proof),
        Admission::Admitted(_)
    ));
    assert_eq!(ui.activate_navigator(&proof), Admission::Refused);
    let lease = mailbox.try_recv().unwrap().unwrap();
    assert!(matches!(lease.record(), QueuedCommand::Refresh(_)));
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn admitted_tab_generation_is_distinct_from_a_highlighted_pending_target() {
    let (mut ui, mailbox, _, _) = ready(false);
    let target = Target::Tab(ui.navigator_catalog().unwrap().tabs[1].clone());
    let proof = row(&ui, target);
    assert!(matches!(
        ui.activate_navigator(&proof),
        Admission::Admitted(_)
    ));
    assert_eq!(ui.activate_navigator(&proof), Admission::Refused);
    assert!(mailbox.try_recv().unwrap().is_some());
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn unavailable_current_tab_is_a_guarded_noop_but_ordinary_selection_can_retry() {
    use super::super::{ClientActionDisposition, RefreshDecision, SelectionPlan};
    use crate::workbench_client::mailbox::{EventDeadline, UiReply, WorkerEvent};
    use crate::workbench_client::state::{
        CleanupContinuation, ClientPhaseKind, CreateTerminalProvenance,
    };
    use orchbee_workbench_protocol::WorkbenchSnapshotOutcome;

    let (mut ui, mailbox, clock, _) = ready(false);
    assert_eq!(
        ui.select_tab(ClientAction::SelectTab(2)),
        ClientActionDisposition::DismissNavigator
    );
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::Switch(marker) = lease.record() else {
        panic!("Switch lease");
    };
    let target = marker.target();
    let permit = mailbox
        .authorize_switch_snapshot(&lease, target, &clock)
        .unwrap()
        .unwrap();
    mailbox.answer_switch_snapshot(permit).unwrap();
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::SwitchSnapshotResult {
                marker: target.selected().key().clone(),
                outcome: WorkbenchSnapshotOutcome::Available {
                    snapshot: super::super::tests::context_selection::snapshot(false),
                },
            },
            EventDeadline::PlacementOnly(target.deadlines().forward_cutoff()),
            &clock,
        )
        .unwrap();
    ui.tick();
    let UiReply::Switch(settlement) = mailbox.take_deposited_reply(&wait).unwrap().unwrap() else {
        panic!("Switch reply");
    };
    let RefreshDecision::Cleanup(cleanup) = settlement.decision() else {
        panic!("Switch cleanup");
    };
    let permit = mailbox
        .authorize_switch_cleanup(&lease, cleanup, cleanup.old(), &clock)
        .unwrap()
        .unwrap();
    assert!(mailbox.answer_switch_cleanup(permit, true).unwrap());
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::MembershipDetached {
                cleanup: cleanup.clone(),
                detached: cleanup.old().clone(),
            },
            EventDeadline::TotalCleanup(cleanup.deadlines().total()),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(matches!(
        mailbox.take_deposited_reply(&wait).unwrap(),
        Some(UiReply::Switch(_))
    ));
    let state = ui.state.as_mut().unwrap();
    let key = state.desired().unwrap().clone();
    assert_eq!(
        mailbox.apply_local_for(state, |state| state.settle_unattached_setup_t(
            &key,
            1,
            &CreateTerminalProvenance::BindingOnly
        )),
        Some(CleanupContinuation::Stable)
    );
    assert!(state.invariant_holds());
    assert_eq!(
        state.phase_kind(),
        ClientPhaseKind::RetryableAttachUnavailable
    );
    assert_eq!(
        state.plan_selection(ClientAction::SelectTab(2)),
        SelectionPlan::Switch { index: 1 }
    );
    mailbox.finish_lease(lease).unwrap();

    // The navigator's accepted entry refresh still owns admission before any no-op.
    ui.apply_client_action(ClientAction::OpenWorkbenchNavigator);
    let proof = row(
        &ui,
        Target::Tab(ui.navigator_catalog().unwrap().current_tab.unwrap()),
    );
    let during_refresh = mailbox.cursors();
    assert_eq!(ui.activate_navigator(&proof), Admission::Refused);
    assert_eq!(mailbox.cursors(), during_refresh);
    super::super::tests::publication_hint::settle_queued_refresh(
        &mut ui,
        &mailbox,
        &clock,
        WorkbenchSnapshotOutcome::Available {
            snapshot: super::super::tests::context_selection::snapshot(false),
        },
    );
    assert_eq!(
        ui.state.as_ref().unwrap().phase_kind(),
        ClientPhaseKind::RetryableAttachUnavailable
    );

    let before = mailbox.cursors();
    let proof = row(
        &ui,
        Target::Tab(ui.navigator_catalog().unwrap().current_tab.unwrap()),
    );
    assert_eq!(ui.activate_navigator(&proof), Admission::ExactCurrent);
    assert_eq!(mailbox.cursors(), before);
    assert!(mailbox.try_recv().unwrap().is_none());
    assert_eq!(
        ui.select_tab(ClientAction::SelectTab(2)),
        ClientActionDisposition::DismissNavigator
    );
    assert_eq!(mailbox.cursors().0, before.0 + 1);
    assert!(matches!(
        mailbox.try_recv().unwrap().unwrap().record(),
        QueuedCommand::Switch(_)
    ));
}

#[test]
fn long_navigator_lists_paint_only_complete_scrolled_rows_in_both_locales_and_sizes() {
    use crate::client_view::{ClientActionDisposition, ClientView, navigator::Command};
    use crate::localization::Locale;
    use crate::tui_pointer::{HitMapBuilder, HitTarget};

    use orchbee_local_site_runtime::{WindowsKeyChord, WindowsKeySettings};
    use orchbee_workbench_protocol::*;
    use ratatui::{Terminal, backend::TestBackend};
    let mut source = super::super::tests::membership::snapshot(20, 1);
    let mut workspaces = Vec::new();
    for index in 0..20 {
        let context_id =
            WorkbenchTerminalContextId::parse(&format!("{:032x}", 700 + index)).unwrap();
        source.terminal_contexts.push(TerminalContext {
            groups: Vec::new(),
            tabs: Vec::new(),
            terminal_context_id: context_id.clone(),
        });
        workspaces.push(Workspace {
            project_id: Some(WorkbenchProjectId::parse(&format!("{:032x}", 900)).unwrap()),
            label: format!("Workspace 工作区 {index} with a long label"),
            primary_directory_id: None,
            workspace_id: WorkbenchWorkspaceId::parse(&format!("{:032x}", 800 + index)).unwrap(),
            terminal_context_id: context_id,
        });
    }
    source.navigation.workspaces.extend(workspaces);
    source.navigation.projects.push(Project {
        project_id: WorkbenchProjectId::parse(&format!("{:032x}", 900)).unwrap(),
        label: "Project 项目".into(),
    });
    let (ui, _, _, _) = super::super::tests::move_membership::ready_ui_with_snapshot(source);
    for (width, height) in [(120, 41), (120, 40), (80, 24)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for remapped in [false, true] {
                let mut view = ClientView::new(width);
                if remapped {
                    view.configure_keys(&WindowsKeySettings {
                        commands: Vec::new(),
                        overrides: Vec::new(),
                        prefix: WindowsKeyChord {
                            key: "a".into(),
                            ctrl: true,
                            alt: false,
                            shift: false,
                            super_key: false,
                        },
                    });
                }
                let transition = view.decide_action(ClientAction::OpenWorkbenchNavigator);
                view.commit(transition, ClientActionDisposition::KeepNavigator);
                view.navigator.reconcile(ui.navigator_catalog());
                for column in [Command::Tabs, Command::Locations] {
                    view.navigator.move_focus(column, 5);
                    view.navigator.move_focus(Command::Last, 5);
                    let capture = width == 120
                        && height == 40
                        && locale == Locale::EnUs
                        && !remapped
                        && column == Command::Locations;
                    let theme = if capture {
                        crate::tui_theme::Theme::terminal_native()
                    } else {
                        crate::tui_theme::Theme::rgb_test_dark()
                    };
                    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                    let mut hits = HitMapBuilder::new();
                    terminal
                        .draw(|frame| {
                            crate::tui_render::render_terminal_shell(
                                frame,
                                &view,
                                locale,
                                Some(ui.view()),
                                &theme,
                                &mut hits,
                            );
                        })
                        .unwrap();
                    if capture {
                        crate::tui_render::test_capture::capture_screen(
                            &mut terminal,
                            "alignment-navigator-locations-en-us-120x40-terminal-native",
                            locale,
                            &theme,
                        );
                    }
                    let map = hits.finish();
                    let paint = map.navigator_paint().unwrap();
                    assert!(paint.rows.len() < 40);
                    assert!(
                        paint
                            .rows
                            .iter()
                            .any(|row| Some(&row.target) == view.navigator.focus.as_ref())
                    );
                    for row in &paint.rows {
                        assert!(view.navigator.accepts(row, false));
                    }
                    let mut hit_count = 0;
                    let mut close_hit_count = 0;
                    for y in 0..height {
                        for x in 0..width {
                            if let Some(HitTarget::NavigatorRow(row)) = map.hit_target_at(x, y) {
                                assert!(paint.rows.contains(row));
                                hit_count += 1;
                            }
                            if matches!(map.hit_target_at(x, y), Some(HitTarget::NavigatorClose(_)))
                            {
                                assert_eq!(
                                    terminal.backend().buffer()[(x, y)].bg,
                                    theme.color(crate::tui_theme::Role::SurfaceOverlay)
                                );
                                close_hit_count += 1;
                            }
                        }
                    }
                    assert!(hit_count > paint.rows.len());
                    assert_eq!(close_hit_count, 3);
                    let text: String = terminal
                        .backend()
                        .buffer()
                        .content()
                        .iter()
                        .map(|cell| cell.symbol())
                        .collect();
                    assert!(text.contains("Esc"));
                    assert!(text.contains("Enter"));
                    assert!(text.contains(if remapped { "Ctrl+a w" } else { "Ctrl+b w" }));
                    assert!(text.contains(if column == Command::Tabs {
                        "20/20"
                    } else {
                        "21/21"
                    }));
                }
            }
        }
    }
}
