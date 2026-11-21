//! The Move Tab keys under fast repeats: presses made while a Tab change is on its way run in
//! press order, one request at a time, and end with the change that did not land.
use super::*;
use crate::localization::WorkbenchClientNotice;
use crate::workbench_client::ui::tests::{membership, ready::ready_ui_with_snapshot};
use crate::workbench_state::workspace_setup::SetupResult;
use orchbee_workbench_protocol::{WorkbenchTabId, WorkbenchTabMoveDirection};
use std::sync::atomic::Ordering;

fn press_next(ui: &mut WorkbenchClientUi) {
    assert_eq!(
        ui.apply_client_action(ClientAction::MoveNextTab),
        ClientActionDisposition::DismissNavigator
    );
}

fn ids(snapshot: &Snapshot) -> Vec<WorkbenchTabId> {
    snapshot.terminal_contexts[0]
        .tabs
        .iter()
        .map(|tab| tab.tab_id.clone())
        .collect()
}

#[test]
fn move_tab_presses_during_a_tab_change_run_in_order_one_request_at_a_time() {
    let mut snapshot = membership::snapshot(4, 1);
    let (mut ui, mailbox, clock, _) = ready_ui_with_snapshot(snapshot.clone());
    let original = ids(&snapshot);
    // Three presses before the first change comes back: one request goes out, two presses wait.
    for _ in 0..3 {
        press_next(&mut ui);
    }
    for place in 1..=3 {
        // Each accepted change installs before the next press is computed, so every request
        // names one step from the order then installed and the Server's own rule accepts it.
        let operation = accept_configuration(&mut ui, &mailbox, &clock, &mut snapshot, None);
        assert_eq!(
            operation,
            TabConfiguration::MoveTab {
                tab_id: original[0].clone(),
                direction: WorkbenchTabMoveDirection::Next,
                adjacent_group_collapsed: false,
            }
        );
        assert_eq!(ids(&snapshot)[place], original[0], "press {place}");
    }
    assert!(mailbox.try_recv().unwrap().is_none(), "three presses only");
    assert_eq!(ui.view().notice(), None);
}

#[test]
fn a_refused_tab_change_ends_the_move_tab_presses_held_behind_it() {
    let (mut ui, mailbox, clock, _) = ready_ui_with_snapshot(membership::snapshot(4, 1));
    press_next(&mut ui);
    press_next(&mut ui);
    let lease = mailbox.try_recv().unwrap().expect("the first press");
    let QueuedCommand::WorkspaceSetup(marker) = lease.record() else {
        panic!("setup lane")
    };
    assert_eq!(
        mailbox.mark_setup_in_flight(&marker.0).unwrap(),
        MutationResult::Applied
    );
    assert!(mailbox.retain_setup_not_applied(&marker.0));
    let wait = mailbox
        .publish_setup_result_before(marker.0.clone(), SetupResult::NotApplied, &clock)
        .unwrap();
    ui.tick();
    mailbox.take_deposited_reply(&wait).unwrap().unwrap();
    mailbox.finish_lease(lease).unwrap();
    assert_eq!(
        ui.view().setup_status(),
        Some(crate::localization::SetupStatus::TabsNotApplied)
    );
    ui.tick();
    assert!(
        mailbox.try_recv().unwrap().is_none(),
        "the held press must not act after the refusal"
    );
    // The refusal ended only the presses already made.
    press_next(&mut ui);
    assert!(mailbox.try_recv().unwrap().is_some());
}

#[test]
fn move_tab_presses_held_past_their_bound_end_with_the_busy_notice() {
    let (mut ui, mailbox, _, elapsed) = ready_ui_with_snapshot(membership::snapshot(4, 1));
    press_next(&mut ui);
    press_next(&mut ui);
    let _unanswered = mailbox.try_recv().unwrap().expect("the first press");
    elapsed.store(4_999, Ordering::SeqCst);
    ui.tick();
    assert_eq!(ui.view().notice(), None);
    elapsed.store(5_000, Ordering::SeqCst);
    ui.tick();
    assert_eq!(ui.view().notice(), Some(WorkbenchClientNotice::LocalBusy));
    assert!(ui.held_intent_wake().is_none());
}
