use super::*;
use crate::client_view::ClientAction;
use crate::workbench_client::mailbox::{
    CommandLease, EventDeadline, MutationResult, QueuedCommand, UiReply, WorkerEvent,
};
use crate::workbench_client::ui::tests::publication_hint::{
    attach_split_peer, ready_split_ui, split_snapshot, split_survivor_snapshot, starting_split_ui,
};
use crate::workbench_state::{ClientPhaseKind, OPERATION_TOTAL, PaneFault, RefreshDecision};
use orchbee_workbench_protocol::WorkbenchSnapshotOutcome;
use std::sync::atomic::Ordering;

fn ids(seed: u8) -> (WorkbenchTabId, WorkbenchPanelId) {
    let hex = format!("{seed:032x}");
    (
        WorkbenchTabId::parse(&hex).unwrap(),
        WorkbenchPanelId::parse(&hex).unwrap(),
    )
}

#[test]
fn attempts_for_one_pane_are_spaced_out_and_bounded_per_window() {
    let (tab, panel) = ids(1);
    let lost = Some((&tab, &panel));
    let mut recovery = ViewerRecovery::default();
    let opened = Instant::now();
    let mut lost_at = opened;
    for pause in PAUSES {
        assert!(!recovery.due(lost, lost_at));
        assert!(!recovery.due(lost, lost_at + pause - Duration::from_millis(1)));
        assert!(recovery.due(lost, lost_at + pause));
        recovery.admitted();
        // The attempt lands, and the viewer is lost again a second later.
        assert!(!recovery.due(None, lost_at + pause));
        lost_at += pause + Duration::from_secs(1);
    }
    assert!(!recovery.due(lost, lost_at));
    assert!(!recovery.due(lost, opened + WINDOW - Duration::from_millis(1)));

    // The window the first loss opened has closed: a fresh budget, after the first pause.
    let reopened = opened + WINDOW;
    assert!(!recovery.due(lost, reopened));
    assert!(recovery.due(lost, reopened + PAUSES[0]));
    // Local work holding the switch slot defers that attempt without spending it.
    let blocked = reopened + PAUSES[0];
    recovery.blocked(blocked);
    assert!(!recovery.due(lost, blocked + BLOCKED_RETRY - Duration::from_millis(1)));
    assert!(recovery.due(lost, blocked + BLOCKED_RETRY));
    recovery.admitted();
    let lost_again = blocked + BLOCKED_RETRY + Duration::from_secs(1);
    assert!(!recovery.due(lost, lost_again));
    assert!(!recovery.due(lost, lost_again + PAUSES[1] - Duration::from_millis(1)));
    assert!(recovery.due(lost, lost_again + PAUSES[1]));

    // Another pane's loss opens its own budget.
    let (tab, other) = ids(2);
    assert!(!recovery.due(Some((&tab, &other)), lost_again));
    assert!(recovery.due(Some((&tab, &other)), lost_again + PAUSES[0]));
}

/// Leases the queued commands, finishing sibling Polls as the executor finishes a stale one,
/// and answers the first Switch.
fn queued_switch(mailbox: &CommandMailbox) -> Option<CommandLease> {
    while let Some(lease) = mailbox.try_recv().unwrap() {
        match lease.record() {
            QueuedCommand::Poll(_) => mailbox.finish_lease(lease).unwrap(),
            QueuedCommand::Switch(_) => return Some(lease),
            _ => panic!("only sibling Polls and the re-attach are queued"),
        }
    }
    None
}

/// The lost pane's split sibling keeps its viewer, so the re-attach first cleans up that live
/// sibling under the old anchor, then attaches the lost panel under the new one.
#[test]
fn a_lost_focused_viewer_reattaches_its_own_panel_after_a_pause() {
    let (mut ui, mailbox, clock, elapsed) = ready_split_ui();
    attach_split_peer(&mut ui, &mailbox, &clock);
    let peer = ui.state.as_ref().unwrap().cache().tabs()[0]
        .panel(1)
        .unwrap()
        .panel_id()
        .clone();
    ui.apply_client_action(ClientAction::FocusPanel(peer.clone()));
    let focus = ui.state.as_ref().unwrap().focus_pane_ref().unwrap();
    assert_eq!(focus.pane().panel(), &peer);
    assert!(queued_switch(&mailbox).is_none());

    mailbox
        .publish_notice_before(
            WorkerEvent::PaneFailed {
                key: focus.clone(),
                fault: PaneFault::ViewerLost,
            },
            clock.now() + OPERATION_TOTAL,
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(ui.state.as_ref().unwrap().focused_viewer_lost());
    let lost_at = elapsed.load(Ordering::SeqCst);
    elapsed.store(lost_at + 299, Ordering::SeqCst);
    ui.tick();
    assert!(
        queued_switch(&mailbox).is_none(),
        "the re-attach waits out its pause"
    );

    elapsed.store(lost_at + 300, Ordering::SeqCst);
    ui.tick();
    let lease = queued_switch(&mailbox).expect("the automatic re-attach");
    let QueuedCommand::Switch(marker) = lease.record() else {
        unreachable!("queued_switch answers a Switch");
    };
    let selected = marker.target().selected().key();
    assert_eq!(selected.tab_id(), focus.pane().tab_id());
    assert_eq!(selected.panel(), &peer, "the lost panel stays focused");
    assert!(selected.generation() > focus.pane().generation());
    let state = ui.state.as_ref().unwrap();
    assert_eq!(state.desired(), Some(selected));
    assert!(!state.focused_viewer_lost());
    assert!(state.invariant_holds());

    let target = marker.target().clone();
    let permit = mailbox
        .authorize_switch_snapshot(&lease, &target, &clock)
        .unwrap()
        .unwrap();
    mailbox.answer_switch_snapshot(permit).unwrap();
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::SwitchSnapshotResult {
                marker: target.selected().key().clone(),
                outcome: WorkbenchSnapshotOutcome::Available {
                    snapshot: split_snapshot(1),
                },
            },
            EventDeadline::PlacementOnly(target.deadlines().forward_cutoff()),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(
        mailbox.is_closed().is_none(),
        "the live sibling never costs the Client"
    );
    let UiReply::Switch(settlement) = mailbox.take_deposited_reply(&wait).unwrap().unwrap() else {
        panic!("switch settlement");
    };
    let RefreshDecision::Cleanup(cleanup) = settlement.decision() else {
        panic!("the live sibling is cleaned up before the attach");
    };
    assert_eq!(cleanup.old(), focus.pane().anchor());
    let permit = mailbox
        .authorize_switch_cleanup(&lease, cleanup, cleanup.old(), &clock)
        .unwrap()
        .expect("the re-attach owns its cleanup");
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
    let UiReply::Switch(settlement) = mailbox.take_deposited_reply(&wait).unwrap().unwrap() else {
        panic!("membership settlement");
    };
    let RefreshDecision::AttachRecovered(attach) = settlement.decision() else {
        panic!("the lost panel attaches again");
    };
    let attached = attach.selected().key();
    assert_eq!(attached.tab_id(), focus.pane().tab_id());
    assert_eq!(attached.panel(), &peer, "the lost panel stays focused");
    assert!(attached.generation() >= target.selected().key().generation());
    mailbox.finish_lease(lease).unwrap();
    assert!(mailbox.is_closed().is_none());
    assert!(!ui.retirement_requested());
    assert!(ui.state.as_ref().unwrap().invariant_holds());

    // One attempt per loss: the landing switch is never doubled.
    elapsed.store(lost_at + 10_000, Ordering::SeqCst);
    ui.tick();
    assert!(queued_switch(&mailbox).is_none());
    assert!(mailbox.is_closed().is_none());
}

/// A focused terminal can be gone before its removal is published, so its Attach fails and the
/// focus parks while its split sibling attaches and keeps polling. The publication that removes
/// the parked panel cleans up that live sibling and lands on the surviving panel.
#[test]
fn removing_a_focus_parked_by_a_failed_attach_keeps_the_client() {
    let (mut ui, mailbox, clock, _elapsed) = starting_split_ui();
    let state = ui.state.as_ref().unwrap();
    let focus = state.focus_pane_ref().unwrap();
    let provenance = state.focus_record().unwrap().provenance().clone();
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::SetupWithoutAttachment {
                key: focus.clone(),
                target_index: 0,
                target_provenance: provenance,
            },
            EventDeadline::Forward(clock.now() + OPERATION_TOTAL),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
    let state = ui.state.as_ref().unwrap();
    assert_eq!(
        state.phase_kind(),
        ClientPhaseKind::RetryableAttachUnavailable
    );
    assert_eq!(
        state.focus_record().unwrap().fault(),
        Some(PaneFault::AttachUnavailable)
    );
    attach_split_peer(&mut ui, &mailbox, &clock);
    let peer = ui.state.as_ref().unwrap().cache().tabs()[0]
        .panel(1)
        .unwrap()
        .panel_id()
        .clone();
    ui.tick();
    let mut polled = None;
    while let Some(lease) = mailbox.try_recv().unwrap() {
        if let QueuedCommand::Poll(poll) = lease.record() {
            polled = Some(poll.key().pane().panel().clone());
        }
        mailbox.finish_lease(lease).unwrap();
    }
    assert_eq!(polled.as_ref(), Some(&peer), "the sibling keeps polling");

    ui.state.as_mut().unwrap().note_publication_hint();
    ui.tick();
    let lease = mailbox
        .try_recv()
        .unwrap()
        .expect("the publication's Refresh");
    let QueuedCommand::Refresh(marker) = lease.record() else {
        panic!("one Refresh marker");
    };
    assert_eq!(
        mailbox.mark_refresh_in_flight(marker.generation()).unwrap(),
        MutationResult::Applied
    );
    let wait = mailbox
        .publish_refresh_result_before(
            marker.generation(),
            WorkbenchSnapshotOutcome::Available {
                snapshot: split_survivor_snapshot(2, 1),
            },
            marker.deadline(),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert_eq!(
        mailbox.is_closed(),
        None,
        "a parked focus never costs the Client"
    );
    let Some(UiReply::Refresh(settlement)) = mailbox.take_deposited_reply(&wait).unwrap() else {
        panic!("refresh settlement");
    };
    let RefreshDecision::Cleanup(cleanup) = settlement.decision() else {
        panic!("the live sibling is cleaned up before the install");
    };
    assert_eq!(cleanup.old(), focus.pane().anchor());
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
    let Some(UiReply::Membership(settlement)) = mailbox.take_deposited_reply(&wait).unwrap() else {
        panic!("membership settlement");
    };
    let RefreshDecision::AttachRecovered(attach) = settlement.decision() else {
        panic!("the surviving panel attaches");
    };
    assert_eq!(attach.selected().key().panel(), &peer);
    mailbox.finish_lease(lease).unwrap();
    assert_eq!(mailbox.is_closed(), None);
    assert!(!ui.retirement_requested());
    assert!(ui.state.as_ref().unwrap().invariant_holds());
}

/// A background Tab close keeps its record while the Server closes that Tab. A focused viewer
/// lost meanwhile parks beside the close instead of costing the Client, and its automatic
/// re-attach waits for the close.
#[test]
fn a_focused_viewer_lost_beside_a_background_close_parks() {
    use crate::local_client::workbench::WorkbenchTabCloseAccepted;
    use crate::workbench_client::ui::test_support::{controlling_ui, install_projection};
    use crate::workbench_state::CloseResult;
    let (mut ui, mailbox, clock, elapsed) = controlling_ui();
    ui.idle_close = false;
    let focus = install_projection(&mut ui, Some((0, false, 24)), 0);
    let background = ui.state.as_ref().unwrap().cache().tabs()[1]
        .tab_id()
        .clone();
    assert!(
        ui.request_close_set(std::slice::from_ref(&background))
            .is_some()
    );
    let token = ui.view().close_confirmation().unwrap().token();
    ui.state
        .as_mut()
        .unwrap()
        .acknowledge_close_draw(Some(token), None);
    let _ = ui.confirm_close();
    let lease = loop {
        let lease = mailbox.try_recv().unwrap().expect("the background close");
        if matches!(lease.record(), QueuedCommand::Close(_)) {
            break lease;
        }
        mailbox.finish_lease(lease).unwrap();
    };
    let QueuedCommand::Close(marker) = lease.record() else {
        unreachable!("the loop answers a Close")
    };
    let operation = marker.operation().clone();
    assert_eq!(
        mailbox.mark_close_in_flight(&operation).unwrap(),
        MutationResult::Applied
    );
    let wait = mailbox
        .publish_close_result_before(
            operation.clone(),
            CloseResult::Accepted(WorkbenchTabCloseAccepted {
                close_request_id: orchbee_workbench_protocol::WorkbenchRequestId::parse(&format!(
                    "{:032x}",
                    61_001
                ))
                .unwrap(),
                terminal_context_id: operation.key().target().terminal_context_id().clone(),
                tab_id: background.clone(),
                accepted_revision: operation.selection().base_revision(),
            }),
            operation.deadline(),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
    mailbox.finish_lease(lease).unwrap();
    assert!(ui.view().tab_leaving(&background));

    mailbox
        .publish_notice_before(
            WorkerEvent::PaneFailed {
                key: focus.clone(),
                fault: PaneFault::ViewerLost,
            },
            clock.now() + OPERATION_TOTAL,
            &clock,
        )
        .unwrap();
    ui.tick();
    assert_eq!(
        mailbox.is_closed(),
        None,
        "a background close never costs the Client"
    );
    let state = ui.state.as_ref().unwrap();
    assert!(state.focused_viewer_lost());
    assert_eq!(state.focus_pane_ref(), Some(focus));
    assert!(
        state.view().tab_leaving(&background),
        "the close keeps its record"
    );

    elapsed.fetch_add(300, Ordering::SeqCst);
    ui.tick();
    assert!(
        ui.state.as_ref().unwrap().focused_viewer_lost(),
        "the re-attach waits for the close"
    );
    assert_eq!(mailbox.is_closed(), None);
    assert!(ui.state.as_ref().unwrap().invariant_holds());
}

/// A rename draft can stay open for a long time. A focused viewer lost meanwhile parks beside the
/// draft instead of costing the Client, and its automatic re-attach waits for the draft.
#[test]
fn a_focused_viewer_lost_under_a_rename_draft_parks() {
    use crate::workbench_client::ui::test_support::{controlling_ui, install_projection};
    let (mut ui, mailbox, clock, elapsed) = controlling_ui();
    let focus = install_projection(&mut ui, Some((0, false, 24)), 0);
    assert!(ui.request_rename_draft().is_some());

    mailbox
        .publish_notice_before(
            WorkerEvent::PaneFailed {
                key: focus.clone(),
                fault: PaneFault::ViewerLost,
            },
            clock.now() + OPERATION_TOTAL,
            &clock,
        )
        .unwrap();
    ui.tick();
    assert_eq!(
        mailbox.is_closed(),
        None,
        "a rename draft never costs the Client"
    );
    let state = ui.state.as_ref().unwrap();
    assert!(state.focused_viewer_lost());
    assert_eq!(state.focus_pane_ref(), Some(focus));
    assert!(state.rename_draft().is_some(), "the draft stays open");

    elapsed.fetch_add(300, Ordering::SeqCst);
    ui.tick();
    assert!(
        ui.state.as_ref().unwrap().focused_viewer_lost(),
        "the re-attach waits for the draft"
    );

    ui.state.as_mut().unwrap().cancel_rename_draft();
    elapsed.fetch_add(100, Ordering::SeqCst);
    ui.tick();
    let mut switched = false;
    while let Some(lease) = mailbox.try_recv().unwrap() {
        if matches!(lease.record(), QueuedCommand::Switch(_)) {
            switched = true;
            break;
        }
        mailbox.finish_lease(lease).unwrap();
    }
    assert!(switched, "the re-attach follows the cancelled draft");
}
