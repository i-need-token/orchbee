use super::*;
use crate::client_view::ClientAction;
use crate::workbench_client::mailbox::{
    CommandLease, EventDeadline, QueuedCommand, UiReply, WorkerEvent,
};
use crate::workbench_client::ui::tests::publication_hint::{
    attach_split_peer, ready_split_ui, split_snapshot,
};
use crate::workbench_state::{OPERATION_TOTAL, PaneFault, RefreshDecision};
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
