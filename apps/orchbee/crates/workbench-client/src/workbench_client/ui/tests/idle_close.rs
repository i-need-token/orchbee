//! A close of one Tab, of a set of Tabs or of a Panel asks the Server first from every route;
//! `cli_windows_idle_close` witnesses the Server's answers on a real PowerShell. A lone close stays
//! unreported for its delay.
use super::super::test_support::{controlling_ui, install_projection};
use super::super::*;
use super::normal_exit_hold::{exited, poll};
use crate::workbench_state::{CloseOperationKind, PanelIntent};
use std::sync::atomic::Ordering;

/// The next queued command that is not an ordinary Poll.
fn next(mailbox: &CommandMailbox) -> crate::workbench_client::mailbox::CommandLease {
    loop {
        let lease = mailbox.try_recv().unwrap().expect("queued command");
        if !matches!(lease.record(), QueuedCommand::Poll(_)) {
            return lease;
        }
        mailbox.finish_lease(lease).unwrap();
    }
}

/// A Controlling Client showing one terminal; `idle_close` says whether a close of one Tab or Panel
/// asks the Server first.
fn shown_ui(
    idle_close: bool,
) -> (
    WorkbenchClientUi,
    std::sync::Arc<CommandMailbox>,
    std::sync::Arc<std::sync::atomic::AtomicU64>,
) {
    let (mut ui, mailbox, _, elapsed) = controlling_ui();
    ui.idle_close = idle_close;
    install_projection(&mut ui, Some((0, false, 24)), 0);
    (ui, mailbox, elapsed)
}

fn tab_ids(ui: &WorkbenchClientUi) -> Vec<orchbee_workbench_protocol::WorkbenchTabId> {
    let state = ui.state.as_ref().unwrap();
    state
        .cache()
        .tabs()
        .iter()
        .map(|tab| tab.tab_id().clone())
        .collect()
}

/// Whether the next queued command is the conditional close of one Tab.
fn asks_the_server(mailbox: &CommandMailbox) -> bool {
    matches!(
        next(mailbox).record(),
        QueuedCommand::Close(marker)
            if matches!(marker.operation().kind(), CloseOperationKind::CloseIfIdle { .. })
    )
}

/// The key, the [x] and the Tab menus close one Tab the same way, and so does the Panel key: the
/// confirmation stays hidden while the Server decides.
#[test]
fn one_tab_or_panel_asks_the_server_first() {
    let (mut ui, mailbox, _) = shown_ui(true);
    ui.apply_client_action(ClientAction::RequestCloseTab);
    assert!(ui.view().close_confirmation().is_none());
    assert!(asks_the_server(&mailbox));

    // The [x] and the Tab menus open a set; a set of one is the same close as the key.
    let (mut ui, mailbox, _) = shown_ui(true);
    let tabs = tab_ids(&ui);
    assert!(ui.request_close_set(&tabs[..1]).is_some());
    assert!(ui.view().close_confirmation().is_none());
    assert!(asks_the_server(&mailbox));

    let (mut ui, mailbox, _) = shown_ui(true);
    ui.apply_client_action(ClientAction::ClosePanel);
    assert!(ui.view().panel_close_confirmation().is_none());
    assert!(matches!(
        next(&mailbox).record(),
        QueuedCommand::Panel(marker) if *marker.operation().intent() == PanelIntent::CloseIfIdle
    ));
}

/// A set of several Tabs asks with one conditional Start of the whole set: its confirmation stays
/// hidden while the Server decides, presents when a member runs, and goes when the Server closes
/// the set unasked.
#[test]
fn a_set_asks_the_server_first_with_one_conditional_start() {
    use crate::workbench_client::mailbox::MutationResult;
    use crate::workbench_state::CloseResult;
    use orchbee_workbench_protocol::{
        TabSetCloseAction, TabSetCloseCondition, TabSetCloseMember, TabSetCloseOutcome,
        TabSetCloseStatus, WorkbenchLabel,
    };
    for asks in [true, false] {
        let (mut ui, mailbox, _) = shown_ui(true);
        let clock = ui.clock.clone();
        let tabs = tab_ids(&ui);
        assert!(ui.request_close_set(&tabs).is_some());
        assert!(ui.view().close_confirmation().is_none());
        let lease = next(&mailbox);
        let QueuedCommand::Close(marker) = lease.record() else {
            panic!("the set Start")
        };
        let operation = marker.operation().clone();
        let CloseOperationKind::TabSet(TabSetCloseAction::Start {
            terminal_context_id,
            condition: TabSetCloseCondition::IfIdle,
            tab_ids,
            ..
        }) = operation.kind().clone()
        else {
            panic!("one conditional Start of the whole set")
        };
        assert_eq!(tab_ids, tabs);
        assert_eq!(
            mailbox.mark_close_in_flight(&operation).unwrap(),
            MutationResult::Applied
        );
        let answer = if asks {
            TabSetCloseOutcome::ConfirmationRequired {}
        } else {
            TabSetCloseOutcome::Available {
                terminal_context_id,
                condition: TabSetCloseCondition::IfIdle,
                status: TabSetCloseStatus::Pending {},
                members: tab_ids
                    .into_iter()
                    .map(|tab_id| TabSetCloseMember {
                        tab_id,
                        label: WorkbenchLabel::parse("member").unwrap(),
                        close: None,
                        absent: false,
                    })
                    .collect(),
            }
        };
        let wait = mailbox
            .publish_close_result_before(
                operation.clone(),
                CloseResult::TabSet(answer),
                operation.deadline(),
                &clock,
            )
            .unwrap();
        ui.tick();
        assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
        mailbox.finish_lease(lease).unwrap();
        assert_eq!(ui.view().close_confirmation().is_some(), asks);
        assert!(tabs.iter().all(|tab| ui.view().tab_leaving(tab) != asks));
    }
}

/// While a close stays unreported, its own terminal ending is no news; the UI wakes when its delay
/// ends, and from then on the status line speaks again.
#[test]
fn an_unreported_close_withholds_the_exit_cue_and_wakes_at_the_end_of_its_delay() {
    use crate::client_view::ClientView;
    use crate::localization::Locale;
    use crate::tui_hit_map::HitMapBuilder;
    use crate::tui_theme::Theme;
    use ratatui::{Terminal, backend::TestBackend};
    // The status line alone: the Panel's own title keeps saying how its terminal ended.
    let status_line = |ui: &WorkbenchClientUi| {
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        terminal
            .draw(|frame| {
                crate::tui_render::render_terminal_shell(
                    frame,
                    &ClientView::new(120),
                    Locale::EnUs,
                    Some(ui.view()),
                    &Theme::terminal_native(),
                    &mut HitMapBuilder::new(),
                );
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        (0..120)
            .map(|x| buffer[(x, 39)].symbol())
            .collect::<String>()
    };
    let (mut ui, mailbox, elapsed) = shown_ui(false);
    let key = ui.projections[0].key.clone();
    let record = exited(&ui, &key, 1);
    let clock = ui.clock.clone();
    assert!(next_is_empty(&mailbox));
    poll(&mut ui, &mailbox, &clock, &key, record);
    let panel = key.pane().panel().clone();
    assert!(ui.view().terminal_end(&panel).is_some());
    let told = status_line(&ui);
    assert!(told.contains("exited"), "{told}");

    // A confirmed close, retained from its submission.
    ui.apply_client_action(ClientAction::RequestCloseTab);
    let token = ui.view().close_confirmation().unwrap().token();
    ui.state
        .as_mut()
        .unwrap()
        .acknowledge_close_draw(Some(token), None);
    let _ = ui.confirm_close();
    assert!(matches!(next(&mailbox).record(), QueuedCommand::Close(_)));
    assert!(ui.view().close_quiet_on_selected());
    let quiet = status_line(&ui);
    assert!(!quiet.contains("exited"), "{quiet}");
    assert!(
        ui.next_poll_delay()
            .is_some_and(|delay| delay <= std::time::Duration::from_secs(1))
    );

    elapsed.fetch_add(1000, Ordering::SeqCst);
    ui.after_draw();
    assert!(ui.tick());
    assert!(!ui.view().close_quiet_on_selected());
    assert!(
        ui.state
            .as_ref()
            .unwrap()
            .close_pending_deadline()
            .is_none()
    );
    let slow = status_line(&ui);
    assert!(slow.contains("exited"), "{slow}");
}

fn next_is_empty(mailbox: &CommandMailbox) -> bool {
    loop {
        let Some(lease) = mailbox.try_recv().unwrap() else {
            return true;
        };
        if !matches!(lease.record(), QueuedCommand::Poll(_)) {
            mailbox.finish_lease(lease).unwrap();
            return false;
        }
        mailbox.finish_lease(lease).unwrap();
    }
}

#[test]
fn second_drawn_tab_close_holds_across_close_exchange_switch_and_revision_advance() {
    use super::super::test_support::{controlling_ui_with, install_projection};
    use crate::action_menu::{DrawnMenuSource, DrawnTabCreate};
    use crate::local_client::workbench::WorkbenchTabCloseAccepted;
    use crate::workbench_client::mailbox::{EventDeadline, MutationResult};
    use crate::workbench_state::{CloseResult, RefreshDecision};
    use orchbee_workbench_protocol::{WorkbenchRevision, WorkbenchSnapshotOutcome};

    let (mut ui, mailbox, clock, _) = controlling_ui_with(3);
    ui.idle_close = true;
    let old_key = install_projection(&mut ui, Some((0, false, 24)), 0);
    let tabs = tab_ids(&ui);
    let state = ui.state.as_ref().unwrap();
    let drawn_rev1 = DrawnTabCreate {
        source: DrawnMenuSource {
            session: state.cache().session_id().clone(),
            revision: state.cache().revision(),
        },
        location: state.cache().location().clone(),
    };

    // Click [x] on the first Tab and leave its close exchange in flight.
    ui.request_drawn_tab_close(&drawn_rev1, &tabs[0]);
    let lease0 = next(&mailbox);
    let QueuedCommand::Close(marker0) = lease0.record() else {
        panic!("first close command");
    };
    let op0 = marker0.operation().clone();
    assert_eq!(
        mailbox.mark_close_in_flight(&op0).unwrap(),
        MutationResult::Applied
    );

    // Clicking a background Tab's [x] while the first close exchange is in flight is held cleanly
    // rather than dropped or forcing a modal dialog.
    ui.request_drawn_tab_close(&drawn_rev1, &tabs[2]);
    assert!(ui.view().close_confirmation().is_none());
    assert_eq!(ui.view().notice(), None);

    // First close is accepted -> active CloseRecord stays in flight and follow_leaving_selection
    // begins switching to Tab 1.
    let target0 = op0.key().target();
    let accepted = CloseResult::Accepted(WorkbenchTabCloseAccepted {
        close_request_id: op0.key().request_id().clone(),
        terminal_context_id: target0.terminal_context_id().clone(),
        tab_id: target0.tab_id().clone(),
        accepted_revision: op0.selection().base_revision(),
    });
    let wait = mailbox
        .publish_close_result_before(op0.clone(), accepted, op0.deadline(), &clock)
        .unwrap();
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
    mailbox.finish_lease(lease0).unwrap();

    // Switching is now in progress and Tab 2 remains held safely without spurious refusal.
    let switch_lease = next(&mailbox);
    let QueuedCommand::Switch(switch_marker) = switch_lease.record() else {
        panic!("Switch to successor Tab 1");
    };
    assert_eq!(switch_marker.target().selected().key().tab_id(), &tabs[1]);

    let mut snap2 = ui
        .state
        .as_ref()
        .unwrap()
        .cache()
        .canonical_snapshot()
        .clone();
    snap2.revision = WorkbenchRevision::new(2);
    let permit = mailbox
        .authorize_switch_snapshot(&switch_lease, switch_marker.target(), &clock)
        .unwrap()
        .unwrap();
    mailbox.answer_switch_snapshot(permit).unwrap();
    let switch_deadlines = switch_marker.target().deadlines();
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::SwitchSnapshotResult {
                marker: switch_marker.target().selected().key().clone(),
                outcome: WorkbenchSnapshotOutcome::Available {
                    snapshot: snap2.clone(),
                },
            },
            EventDeadline::PlacementOnly(switch_deadlines.forward_cutoff()),
            &clock,
        )
        .unwrap();
    ui.tick();
    let Some(UiReply::Switch(settlement)) = mailbox.take_deposited_reply(&wait).unwrap() else {
        panic!("Switch snapshot reply");
    };
    let RefreshDecision::Cleanup(cleanup) = settlement.decision() else {
        panic!("old viewer cleanup");
    };
    let permit = mailbox
        .authorize_switch_cleanup(&switch_lease, cleanup, old_key.anchor(), &clock)
        .unwrap()
        .unwrap();
    mailbox.answer_switch_cleanup(permit, true).unwrap();
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::MembershipDetached {
                cleanup: cleanup.clone(),
                detached: old_key.anchor().clone(),
            },
            EventDeadline::TotalCleanup(switch_deadlines.total()),
            &clock,
        )
        .unwrap();
    ui.tick();
    let Some(UiReply::Switch(settlement)) = mailbox.take_deposited_reply(&wait).unwrap() else {
        panic!("Switch successor reply");
    };
    let RefreshDecision::AttachRecovered(target) = settlement.decision() else {
        panic!("landing attach");
    };
    let landed = target.selected().key().clone();
    mailbox.finish_lease(switch_lease).unwrap();
    assert_eq!(ui.view().phase(), DisplayPhase::Switching);
    {
        use crate::workbench_state::{ControllerMode, FrameIdentity};
        use orchbee_client_server_protocol::{ControllerId, Geometry};
        let state = ui.state.as_mut().unwrap();
        let key = state.desired().unwrap().clone();
        let frame = FrameIdentity::new(2);
        let geometry = Some(Geometry::new(80, 24, 8, 16).unwrap());
        let mode =
            ControllerMode::Controlling(ControllerId::parse(&format!("{:032x}", 8)).unwrap());
        assert!(state.install_projection_t(&key));
        assert!(state.record_setup_configuration_t(&key, mode.clone(), geometry));
        assert!(state.stage_drawn_frame_t(&key, frame));
        assert!(state.mark_feedback_accepted_t(&key, frame));
        assert!(state.publish_ready_t(&key, frame, mode, geometry));
    }
    assert_eq!(ui.view().phase(), DisplayPhase::Ready);
    assert_eq!(ui.state.as_ref().unwrap().desired(), Some(&landed));

    // Once Switching settles onto Ready, the held [x] on Tab 2 replays even though Tab 0's
    // CloseRecord is still open and cache revision advanced to 2!
    ui.tick();
    let lease2 = next(&mailbox);
    let QueuedCommand::Close(marker2) = lease2.record() else {
        panic!("replayed close for Tab 2");
    };
    assert_eq!(
        marker2.operation().key().target().tab_id(),
        &tabs[2],
        "held [x] click must target exact Tab 2"
    );
    mailbox.finish_lease(lease2).unwrap();
}

#[test]
fn held_close_initiation_expires_to_local_busy_and_hard_blocked_reports_busy() {
    use super::super::test_support::{controlling_ui_with, install_projection};
    use crate::workbench_client::mailbox::MutationResult;
    use crate::workbench_client::ui::WorkbenchClientNotice;
    use std::sync::atomic::Ordering;

    for action in [ClientAction::RequestCloseTab, ClientAction::ClosePanel] {
        let (mut ui, mailbox, _, elapsed) = controlling_ui_with(2);
        ui.idle_close = true;
        install_projection(&mut ui, Some((0, false, 24)), 0);
        let tabs = tab_ids(&ui);
        // Close background Tab 1 first while Tab 0 stays focused so RequestCloseTab / ClosePanel
        // target disjoint Tab 0 and hold cleanly until the busy close exchange settles or expires.
        ui.request_close_set(&tabs[1..2]).unwrap();
        let lease = next(&mailbox);
        let QueuedCommand::Close(marker) = lease.record() else {
            panic!("first close");
        };
        let op = marker.operation().clone();
        assert_eq!(
            mailbox.mark_close_in_flight(&op).unwrap(),
            MutationResult::Applied
        );

        assert_eq!(
            ui.apply_client_action(action),
            ClientActionDisposition::DismissNavigator
        );
        assert_eq!(ui.view().notice(), None);
        elapsed.fetch_add(5_000, Ordering::SeqCst);
        ui.tick();
        assert_eq!(ui.view().notice(), Some(WorkbenchClientNotice::LocalBusy));
        mailbox.finish_lease(lease).unwrap();
    }

    // Hard-blocked [x] (non-modal structural mutation in flight) refuses visibly instead of
    // dropping silently.
    let (mut ui, _mailbox, _, _) = controlling_ui_with(2);
    install_projection(&mut ui, Some((0, false, 24)), 0);
    let tabs = tab_ids(&ui);
    let drawn = drawn_tab_create(&ui);
    assert_eq!(
        ui.apply_client_action(ClientAction::MoveNextTab),
        ClientActionDisposition::DismissNavigator
    );
    ui.request_drawn_tab_close(&drawn, &tabs[1]);
    assert_eq!(ui.view().notice(), Some(WorkbenchClientNotice::LocalBusy));
}

fn drawn_tab_create(ui: &WorkbenchClientUi) -> crate::action_menu::DrawnTabCreate {
    let state = ui.state.as_ref().unwrap();
    crate::action_menu::DrawnTabCreate {
        source: crate::action_menu::DrawnMenuSource {
            session: state.cache().session_id().clone(),
            revision: state.cache().revision(),
        },
        location: state.cache().location().clone(),
    }
}

fn settle_accepted_tab_close(
    ui: &mut WorkbenchClientUi,
    mailbox: &CommandMailbox,
    clock: &LocalOperationClock,
    expected_tab: &orchbee_workbench_protocol::WorkbenchTabId,
) {
    use crate::local_client::workbench::WorkbenchTabCloseAccepted;
    use crate::workbench_client::mailbox::MutationResult;
    use crate::workbench_state::CloseResult;

    let lease = next(mailbox);
    let QueuedCommand::Close(marker) = lease.record() else {
        panic!("expected close command for {expected_tab:?}");
    };
    let op = marker.operation().clone();
    assert_eq!(op.key().target().tab_id(), expected_tab);
    assert_eq!(
        mailbox.mark_close_in_flight(&op).unwrap(),
        MutationResult::Applied
    );
    let target = op.key().target();
    let wait = mailbox
        .publish_close_result_before(
            op.clone(),
            CloseResult::Accepted(WorkbenchTabCloseAccepted {
                close_request_id: op.key().request_id().clone(),
                terminal_context_id: target.terminal_context_id().clone(),
                tab_id: target.tab_id().clone(),
                accepted_revision: op.selection().base_revision(),
            }),
            op.deadline(),
            clock,
        )
        .unwrap();
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
    mailbox.finish_lease(lease).unwrap();
}

fn land_switch_with_snapshot(
    ui: &mut WorkbenchClientUi,
    mailbox: &CommandMailbox,
    clock: &LocalOperationClock,
    old_anchor: &crate::workbench_state::AttachmentKey,
    revision: u64,
    frame_id: u64,
    mutate: impl FnOnce(&mut orchbee_workbench_protocol::Snapshot),
) -> crate::workbench_state::PaneKey {
    use crate::workbench_client::mailbox::EventDeadline;
    use crate::workbench_state::{ControllerMode, FrameIdentity, RefreshDecision};
    use orchbee_client_server_protocol::{ControllerId, Geometry};
    use orchbee_workbench_protocol::{WorkbenchRevision, WorkbenchSnapshotOutcome};

    let lease = next(mailbox);
    let QueuedCommand::Switch(marker) = lease.record() else {
        panic!("Switch command");
    };
    let mut snap = ui
        .state
        .as_ref()
        .unwrap()
        .cache()
        .canonical_snapshot()
        .clone();
    snap.revision = WorkbenchRevision::new(revision);
    mutate(&mut snap);
    let permit = mailbox
        .authorize_switch_snapshot(&lease, marker.target(), clock)
        .unwrap()
        .unwrap();
    mailbox.answer_switch_snapshot(permit).unwrap();
    let deadlines = marker.target().deadlines();
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::SwitchSnapshotResult {
                marker: marker.target().selected().key().clone(),
                outcome: WorkbenchSnapshotOutcome::Available { snapshot: snap },
            },
            EventDeadline::PlacementOnly(deadlines.forward_cutoff()),
            clock,
        )
        .unwrap();
    ui.tick();
    let Some(UiReply::Switch(settlement)) = mailbox.take_deposited_reply(&wait).unwrap() else {
        panic!("Switch snapshot reply");
    };
    let RefreshDecision::Cleanup(cleanup) = settlement.decision() else {
        panic!("old viewer cleanup");
    };
    let permit = mailbox
        .authorize_switch_cleanup(&lease, cleanup, old_anchor, clock)
        .unwrap()
        .unwrap();
    mailbox.answer_switch_cleanup(permit, true).unwrap();
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::MembershipDetached {
                cleanup: cleanup.clone(),
                detached: old_anchor.clone(),
            },
            EventDeadline::TotalCleanup(deadlines.total()),
            clock,
        )
        .unwrap();
    ui.tick();
    assert!(matches!(
        mailbox.take_deposited_reply(&wait).unwrap(),
        Some(UiReply::Switch(_))
    ));
    mailbox.finish_lease(lease).unwrap();
    let state = ui.state.as_mut().unwrap();
    let key = state.desired().unwrap().clone();
    let frame = FrameIdentity::new(frame_id);
    let geometry = Some(Geometry::new(80, 24, 8, 16).unwrap());
    let mode = ControllerMode::Controlling(ControllerId::parse(&format!("{:032x}", 8)).unwrap());
    assert!(state.install_projection_t(&key));
    assert!(state.record_setup_configuration_t(&key, mode.clone(), geometry));
    assert!(state.stage_drawn_frame_t(&key, frame));
    assert!(state.mark_feedback_accepted_t(&key, frame));
    assert!(state.publish_ready_t(&key, frame, mode, geometry));
    key
}

#[test]
fn third_close_click_and_held_tab_switch_preserve_all_pending_close_targets() {
    use super::super::test_support::{controlling_ui_with, install_projection};
    use crate::local_client::workbench::WorkbenchTabCloseAccepted;
    use crate::workbench_client::mailbox::MutationResult;
    use crate::workbench_state::CloseResult;

    // Part A: clicking [x] on Tabs 2, 2 (duplicate), and 3 while Tab 1's close exchange is in
    // flight keeps Tabs 2 and 3 in FIFO order and deduplicates the repeat click on Tab 2.
    let (mut ui, mailbox, clock, _) = controlling_ui_with(4);
    ui.idle_close = true;
    install_projection(&mut ui, Some((0, false, 24)), 0);
    let tabs = tab_ids(&ui);
    let drawn = drawn_tab_create(&ui);

    ui.request_drawn_tab_close(&drawn, &tabs[1]);
    let lease1 = next(&mailbox);
    let QueuedCommand::Close(marker1) = lease1.record() else {
        panic!("first close");
    };
    let op1 = marker1.operation().clone();
    assert_eq!(
        mailbox.mark_close_in_flight(&op1).unwrap(),
        MutationResult::Applied
    );

    ui.request_drawn_tab_close(&drawn, &tabs[2]);
    ui.request_drawn_tab_close(&drawn, &tabs[2]);
    ui.request_drawn_tab_close(&drawn, &tabs[3]);
    assert_eq!(ui.view().notice(), None);

    let target1 = op1.key().target();
    let wait1 = mailbox
        .publish_close_result_before(
            op1.clone(),
            CloseResult::Accepted(WorkbenchTabCloseAccepted {
                close_request_id: op1.key().request_id().clone(),
                terminal_context_id: target1.terminal_context_id().clone(),
                tab_id: target1.tab_id().clone(),
                accepted_revision: op1.selection().base_revision(),
            }),
            op1.deadline(),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait1).unwrap().is_some());
    mailbox.finish_lease(lease1).unwrap();

    // Tick replays queued Tab 2 first, then Tab 3 once Tab 2 settles; duplicate Tab 2 is gone
    // (only status Reads for the already-accepted closes may follow, never another CloseIfIdle).
    ui.tick();
    settle_accepted_tab_close(&mut ui, &mailbox, &clock, &tabs[2]);
    ui.tick();
    settle_accepted_tab_close(&mut ui, &mailbox, &clock, &tabs[3]);
    ui.tick();
    while let Some(lease) = mailbox.try_recv().unwrap() {
        assert!(!matches!(
            lease.record(),
            QueuedCommand::Close(marker)
                if matches!(marker.operation().kind(), CloseOperationKind::CloseIfIdle { .. })
        ));
        mailbox.finish_lease(lease).unwrap();
    }

    // Part B: pressing a held navigation key during Switching does not drop a queued [x] close.
    let (mut ui, mailbox, clock, _) = controlling_ui_with(4);
    ui.idle_close = true;
    let old_key = install_projection(&mut ui, Some((0, false, 24)), 0);
    let tabs = tab_ids(&ui);
    let drawn = drawn_tab_create(&ui);
    ui.request_drawn_tab_close(&drawn, &tabs[0]);
    settle_accepted_tab_close(&mut ui, &mailbox, &clock, &tabs[0]);

    ui.request_drawn_tab_close(&drawn, &tabs[3]);
    assert_eq!(
        ui.apply_client_action(ClientAction::SelectNextTab),
        ClientActionDisposition::DismissNavigator
    );
    let key1 = land_switch_with_snapshot(&mut ui, &mailbox, &clock, old_key.anchor(), 2, 2, |_| {});
    ui.tick();
    assert_eq!(ui.view().phase(), DisplayPhase::Switching);
    land_switch_with_snapshot(&mut ui, &mailbox, &clock, key1.anchor(), 3, 3, |_| {});
    assert_eq!(ui.state.as_ref().unwrap().active().unwrap().index(), 2);

    ui.tick();
    let lease3 = next(&mailbox);
    let QueuedCommand::Close(marker3) = lease3.record() else {
        panic!("held [x] Tab 3 must still replay after SelectNextTab finishes");
    };
    assert_eq!(marker3.operation().key().target().tab_id(), &tabs[3]);
    mailbox.finish_lease(lease3).unwrap();
}

/// Settles queued Polls, snapshot Refreshes, and Close status Reads (answering `Closing` so active
/// `CloseRecord`s remain open) and returns the first remaining command.
fn serve_background_commands(
    ui: &mut WorkbenchClientUi,
    mailbox: &CommandMailbox,
    clock: &LocalOperationClock,
    snapshot: &orchbee_workbench_protocol::Snapshot,
) -> Option<crate::workbench_client::mailbox::CommandLease> {
    use crate::workbench_client::mailbox::MutationResult;
    use crate::workbench_state::CloseResult;
    use orchbee_workbench_protocol::{
        WorkbenchSnapshotOutcome, WorkbenchTabCloseAttemptGeneration, WorkbenchTabClosePhase,
        WorkbenchTabCloseReadOutcome, WorkbenchTabCloseStatus,
    };

    while let Some(lease) = mailbox.try_recv().unwrap() {
        match lease.record() {
            QueuedCommand::Poll(_) => mailbox.finish_lease(lease).unwrap(),
            QueuedCommand::Refresh(marker) => {
                let (generation, deadline) = (marker.generation(), marker.deadline());
                assert_eq!(
                    mailbox.mark_refresh_in_flight(generation).unwrap(),
                    MutationResult::Applied
                );
                let wait = mailbox
                    .publish_refresh_result_before(
                        generation,
                        WorkbenchSnapshotOutcome::Available {
                            snapshot: snapshot.clone(),
                        },
                        deadline,
                        clock,
                    )
                    .unwrap();
                ui.tick();
                let _ = mailbox.take_deposited_reply(&wait);
                mailbox.finish_lease(lease).unwrap();
            }
            QueuedCommand::Close(marker)
                if matches!(marker.operation().kind(), CloseOperationKind::Read { .. }) =>
            {
                let read = marker.operation().clone();
                let CloseOperationKind::Read {
                    expected_operation, ..
                } = read.kind().clone()
                else {
                    unreachable!();
                };
                assert_eq!(
                    mailbox.mark_close_in_flight(&read).unwrap(),
                    MutationResult::Applied
                );
                let close_request_id =
                    expected_operation.unwrap_or_else(|| read.key().request_id().clone());
                let wait = mailbox
                    .publish_close_result_before(
                        read.clone(),
                        CloseResult::Read(WorkbenchTabCloseReadOutcome::Available {
                            close_request_id,
                            terminal_context_id: read.key().target().terminal_context_id().clone(),
                            tab_id: read.key().target().tab_id().clone(),
                            attempt_generation: WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
                            status: WorkbenchTabCloseStatus::Closing {
                                phase: WorkbenchTabClosePhase::WaitingGrace,
                            },
                        }),
                        read.deadline(),
                        clock,
                    )
                    .unwrap();
                ui.tick();
                let _ = mailbox.take_deposited_reply(&wait);
                mailbox.finish_lease(lease).unwrap();
                ui.tick();
            }
            _ => return Some(lease),
        }
    }
    None
}

#[test]
fn drawn_tab_close_opens_while_a_panel_of_that_tab_is_slowly_closing() {
    use super::publication_hint::ready_split_ui_with_mode;
    use crate::exchange_values::workbench::panel_close::WorkbenchPanelCloseAccepted;
    use crate::workbench_client::mailbox::MutationResult;
    use crate::workbench_state::{ControllerMode, PanelResult};
    use orchbee_client_server_protocol::ControllerId;

    let (mut ui, mailbox, clock, _) = ready_split_ui_with_mode(ControllerMode::Controlling(
        ControllerId::parse(&format!("{:032x}", 1)).unwrap(),
    ));
    let source = ui
        .state
        .as_ref()
        .unwrap()
        .cache()
        .canonical_snapshot()
        .clone();
    assert!(serve_background_commands(&mut ui, &mailbox, &clock, &source).is_none());
    ui.idle_close = true;

    // Close Panel 0 of the split Tab and settle it as Accepted so a live Panel-scoped
    // CloseRecord stays active inside that Tab while Panel 1 takes focus locally.
    assert_eq!(
        ui.apply_client_action(ClientAction::ClosePanel),
        ClientActionDisposition::DismissNavigator
    );
    let lease = serve_background_commands(&mut ui, &mailbox, &clock, &source)
        .expect("expected Panel CloseIfIdle");
    let QueuedCommand::Panel(panel_marker) = lease.record() else {
        panic!("expected Panel CloseIfIdle");
    };
    let panel_op = panel_marker.operation().clone();
    assert_eq!(
        mailbox.mark_panel_in_flight(&panel_op).unwrap(),
        MutationResult::Applied
    );
    let wait = mailbox
        .publish_panel_result_before(
            panel_op.clone(),
            PanelResult::CloseAccepted(WorkbenchPanelCloseAccepted {
                close_request_id: panel_op.request_id().clone(),
                terminal_context_id: panel_op.baseline().terminal_context_id().unwrap().clone(),
                tab_id: panel_op.target().tab_id().clone(),
                panel_id: panel_op.panel_id().clone(),
                accepted_revision: panel_op.baseline().revision(),
            }),
            panel_op.deadline(),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
    mailbox.finish_lease(lease).unwrap();
    assert!(serve_background_commands(&mut ui, &mailbox, &clock, &source).is_none());

    // Clicking [x] on the Tab itself must open cleanly rather than refusing with LocalBusy.
    let tab0 = ui.state.as_ref().unwrap().cache().tabs()[0]
        .tab_id()
        .clone();
    let drawn = drawn_tab_create(&ui);
    ui.request_drawn_tab_close(&drawn, &tab0);
    assert_eq!(ui.view().notice(), None);
    let tab_lease = serve_background_commands(&mut ui, &mailbox, &clock, &source)
        .expect("Tab close must be admitted");
    let QueuedCommand::Close(tab_marker) = tab_lease.record() else {
        panic!("Tab close must be admitted while only its single Panel is closing");
    };
    assert!(matches!(
        tab_marker.operation().kind(),
        CloseOperationKind::CloseIfIdle { .. }
    ));
    assert_eq!(tab_marker.operation().key().target().tab_id(), &tab0);
    mailbox.finish_lease(tab_lease).unwrap();
}

#[test]
fn held_tab_menu_close_verifies_frozen_target_set_and_closes_behind_active_tab_set() {
    use super::super::test_support::{controlling_ui_with, install_projection};
    use crate::workbench_client::mailbox::MutationResult;
    use crate::workbench_client::ui::WorkbenchClientNotice;
    use crate::workbench_state::CloseResult;
    use orchbee_workbench_protocol::{
        TabSetCloseAction, TabSetCloseCondition, TabSetCloseMember, TabSetCloseOutcome,
        TabSetCloseStatus, WorkbenchLabel,
    };
    use std::sync::atomic::Ordering;

    // Part A: held TabMenuClose refuses with WorkspaceChanged if its target set changed before
    // replay (e.g., a trailing Tab departed while the held CloseTrailingTabs waited for a switch).
    let (mut ui, mailbox, clock, _) = controlling_ui_with(4);
    ui.idle_close = true;
    let old_key = install_projection(&mut ui, Some((0, false, 24)), 0);
    let tabs = tab_ids(&ui);
    let drawn = drawn_tab_create(&ui);
    ui.request_drawn_tab_close(&drawn, &tabs[0]);
    settle_accepted_tab_close(&mut ui, &mailbox, &clock, &tabs[0]);

    // Switching to Tab 1 is in progress; ask CloseTrailingTabs on Tab 1 (frozen set [Tab 2, Tab 3]).
    ui.apply_client_action(ClientAction::TabMenu {
        action: crate::action_menu::MenuAction::CloseTrailingTabs,
        tab_id: Some(tabs[1].clone()),
    });
    assert_eq!(ui.view().notice(), None);

    // Finish landing on Tab 1 with Tab 3 removed by a peer revision so trailing set becomes [Tab 2].
    let _landed =
        land_switch_with_snapshot(&mut ui, &mailbox, &clock, old_key.anchor(), 2, 2, |snap| {
            snap.terminal_contexts[0]
                .tabs
                .retain(|tab| tab.tab_id != tabs[3]);
        });

    let landed_snap = ui
        .state
        .as_ref()
        .unwrap()
        .cache()
        .canonical_snapshot()
        .clone();
    assert!(serve_background_commands(&mut ui, &mailbox, &clock, &landed_snap).is_none());
    ui.tick();
    assert_eq!(
        ui.view().notice(),
        Some(WorkbenchClientNotice::WorkspaceChanged)
    );
    while let Some(lease) = mailbox.try_recv().unwrap() {
        assert!(!matches!(
            lease.record(),
            QueuedCommand::Close(m)
                if matches!(m.operation().kind(), CloseOperationKind::TabSet(_))
        ));
        mailbox.finish_lease(lease).unwrap();
    }

    // Part B: clicking [x] on Tab 3 while a multi-Tab set [Tab 1, Tab 2] is actively closing holds
    // behind the set instead of failing with LocalBusy, then closes Tab 3 once the set ends.
    let (mut ui, mailbox, clock, elapsed) = controlling_ui_with(4);
    ui.idle_close = true;
    install_projection(&mut ui, Some((0, false, 24)), 0);
    let tabs = tab_ids(&ui);
    let drawn = drawn_tab_create(&ui);
    assert!(ui.request_close_set(&tabs[1..3]).is_some());
    let set_lease = next(&mailbox);
    let QueuedCommand::Close(set_marker) = set_lease.record() else {
        panic!("TabSet Start");
    };
    let set_op = set_marker.operation().clone();
    let CloseOperationKind::TabSet(TabSetCloseAction::Start {
        terminal_context_id,
        tab_ids,
        ..
    }) = set_op.kind().clone()
    else {
        panic!("TabSet Start operation");
    };
    assert_eq!(
        mailbox.mark_close_in_flight(&set_op).unwrap(),
        MutationResult::Applied
    );
    let members: Vec<_> = tab_ids
        .into_iter()
        .map(|tab_id| TabSetCloseMember {
            tab_id,
            label: WorkbenchLabel::parse("member").unwrap(),
            close: None,
            absent: false,
        })
        .collect();
    let wait = mailbox
        .publish_close_result_before(
            set_op.clone(),
            CloseResult::TabSet(TabSetCloseOutcome::Available {
                terminal_context_id: terminal_context_id.clone(),
                condition: TabSetCloseCondition::IfIdle,
                status: TabSetCloseStatus::Pending {},
                members: members.clone(),
            }),
            set_op.deadline(),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
    mailbox.finish_lease(set_lease).unwrap();
    assert!(ui.state.as_ref().unwrap().close_set_status().is_some());

    // [x] on Tab 3 must hold behind the active multi-Tab set rather than fail with LocalBusy.
    ui.request_drawn_tab_close(&drawn, &tabs[3]);
    assert_eq!(ui.view().notice(), None);

    elapsed.fetch_add(250, Ordering::SeqCst);
    ui.tick();
    let status_lease = next(&mailbox);
    let QueuedCommand::Close(status_marker) = status_lease.record() else {
        panic!("TabSet Status read");
    };
    let status_op = status_marker.operation().clone();
    assert_eq!(
        mailbox.mark_close_in_flight(&status_op).unwrap(),
        MutationResult::Applied
    );
    let wait = mailbox
        .publish_close_result_before(
            status_op.clone(),
            CloseResult::TabSet(TabSetCloseOutcome::Available {
                terminal_context_id,
                condition: TabSetCloseCondition::IfIdle,
                status: TabSetCloseStatus::Closed {},
                members,
            }),
            status_op.deadline(),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
    mailbox.finish_lease(status_lease).unwrap();
    assert!(ui.state.as_ref().unwrap().close_set_status().is_none());

    ui.tick();
    let lease3 = next(&mailbox);
    let QueuedCommand::Close(marker3) = lease3.record() else {
        panic!("held [x] Tab 3 must replay once multi-Tab set finishes");
    };
    assert_eq!(marker3.operation().key().target().tab_id(), &tabs[3]);
    mailbox.finish_lease(lease3).unwrap();
}

#[test]
fn drawn_tab_close_and_held_close_replay_respect_modal_overlay_and_session_guards() {
    use super::super::test_support::{controlling_ui_with, install_projection};
    use crate::workbench_client::ui::WorkbenchClientNotice;
    use orchbee_workbench_protocol::SessionId;

    let (mut ui, mailbox, clock, elapsed) = controlling_ui_with(3);
    ui.idle_close = true;
    install_projection(&mut ui, Some((0, false, 24)), 0);
    let tabs = tab_ids(&ui);

    // A stale DrawnTabCreate session never initiates a close.
    let mut wrong_session = drawn_tab_create(&ui);
    wrong_session.source.session = SessionId::parse("ffffffffffffffffffffffffffffffff").unwrap();
    ui.request_drawn_tab_close(&wrong_session, &tabs[0]);
    assert!(mailbox.try_recv().unwrap().is_none());

    // Queue a valid [x] click behind an in-flight close of Tab 0, then open Settings before
    // replay runs.
    let drawn = drawn_tab_create(&ui);
    ui.request_drawn_tab_close(&drawn, &tabs[0]);
    ui.request_drawn_tab_close(&drawn, &tabs[1]);
    assert!(ui.settings.editor.open().is_some());
    assert!(ui.settings_active());

    // Immediate [x] behind open Settings must be ignored too.
    ui.request_drawn_tab_close(&drawn, &tabs[2]);
    assert!(ui.background_reads_yield());

    settle_accepted_tab_close(&mut ui, &mailbox, &clock, &tabs[0]);

    // Even after Tab 0's close acceptance settles, only status reads for Tab 0 may run — the
    // queued close on Tab 1 must NOT replay underneath Settings.
    ui.tick();
    while let Some(lease) = mailbox.try_recv().unwrap() {
        assert!(
            !matches!(
                lease.record(),
                QueuedCommand::Close(marker)
                    if marker.operation().key().target().tab_id() != &tabs[0]
            ),
            "queued close on Tab 1 must not replay underneath open Settings"
        );
        mailbox.finish_lease(lease).unwrap();
    }
    assert!(ui.background_reads_yield());

    // If Settings stays open past the hold bound, the held close expires cleanly to LocalBusy.
    elapsed.fetch_add(5_000, Ordering::SeqCst);
    ui.tick();
    while let Some(lease) = mailbox.try_recv().unwrap() {
        assert!(!matches!(
            lease.record(),
            QueuedCommand::Close(marker)
                if marker.operation().key().target().tab_id() != &tabs[0]
        ));
        mailbox.finish_lease(lease).unwrap();
    }
    assert!(!ui.background_reads_yield());
    assert_eq!(ui.view().notice(), Some(WorkbenchClientNotice::LocalBusy));
}
