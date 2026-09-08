use super::*;
use crate::site_coordinator::standalone::prepare_standalone_close_settlement;

#[test]
fn retargeted_later_create_settles_current_reference_without_changing_historical_result() {
    let mut coordinator = site();
    let first_command = tab_command(&coordinator, 1200, 0, "first");
    let first = dispatch_tab(&mut coordinator, first_command, 1201);
    let first_completion = immediate_tab_completion(&first);
    coordinator
        .complete_terminal_tab_create(first, first_completion, Some(worker(1)))
        .unwrap();
    let later_command = tab_command(&coordinator, 1202, 1, "later");
    let later = dispatch_tab(&mut coordinator, later_command.clone(), 1203);
    assert!(!later.settles_first());
    let pending_reply =
        ready(coordinator.prepare_terminal_tab_create(later_command.clone(), None, None, None));
    let before = snapshot(&mut coordinator);

    // Exercise the current-reference completion cut. The actual Close finalizer owns removal.
    coordinator.standalone = StandaloneTerminal::Creating(id(1203));
    assert!(matches!(
        coordinator.prepare_open(
            &orchbee_client_server_protocol::TerminalOpenRequest::new(id(1204), geometry()),
            None,
            Some(worker(1)),
            None,
        ),
        PreparedOpen::Wait
    ));
    assert_eq!(
        ready(coordinator.prepare_terminal_tab_create(later_command.clone(), None, None, None)),
        pending_reply
    );
    let completed = coordinator
        .complete_terminal_tab_create(later, Completion::Busy, Some(worker(1)))
        .unwrap();
    assert!(completed.standalone_changed);
    assert_eq!(
        coordinator.standalone,
        StandaloneTerminal::Settled {
            reference: id(1203),
            outcome: OpenOutcome::Unavailable {
                reason: CreateUnavailableReason::ServerBusy
            },
        }
    );
    assert_eq!(snapshot(&mut coordinator), before);
    assert_eq!(
        ready(coordinator.prepare_terminal_tab_create(later_command, None, None, None)),
        completed.result
    );
    assert!(matches!(
        pending_reply.outcome,
        WorkbenchCommandOutcome::TabCreatePending { .. }
    ));
}

#[test]
fn noncurrent_completion_preserves_newer_current_reference_and_reports_no_notification() {
    let mut coordinator = site();
    let first_command = tab_command(&coordinator, 1210, 0, "first");
    let first = dispatch_tab(&mut coordinator, first_command, 1211);
    let first_completion = immediate_tab_completion(&first);
    coordinator
        .complete_terminal_tab_create(first, first_completion, Some(worker(1)))
        .unwrap();
    let later_command = tab_command(&coordinator, 1212, 1, "pending later");
    let later = dispatch_tab(&mut coordinator, later_command.clone(), 1213);
    let newest_command = tab_command(&coordinator, 1214, 2, "new current");
    let newest = dispatch_tab(&mut coordinator, newest_command, 1215);
    let newest_completion = immediate_tab_completion(&newest);
    assert!(
        !coordinator
            .complete_terminal_tab_create(newest, newest_completion, Some(worker(1)))
            .unwrap()
            .standalone_changed
    );

    // This is a validated reference-selection cut, not a fabricated Close publication witness.
    coordinator.standalone = StandaloneTerminal::Settled {
        reference: id(1215),
        outcome: OpenOutcome::Opened {
            terminal_create_request_id: id(1215),
        },
    };
    let completed = coordinator
        .complete_terminal_tab_create(later, Completion::Unknown, Some(worker(1)))
        .unwrap();
    assert!(!completed.standalone_changed);
    assert_eq!(
        coordinator.standalone,
        StandaloneTerminal::Settled {
            reference: id(1215),
            outcome: OpenOutcome::Opened {
                terminal_create_request_id: id(1215)
            },
        }
    );
    assert_eq!(
        ready(coordinator.prepare_terminal_tab_create(later_command, None, None, None)),
        completed.result
    );
}

#[test]
fn prepared_last_member_retarget_is_effect_free_until_slot_commit() {
    let mut coordinator = site();
    let empty = coordinator
        .workbench
        .as_ref()
        .unwrap()
        .standalone_membership()
        .unwrap();
    let command = tab_command(&coordinator, 1220, 0, "last");
    let pending = dispatch_tab(&mut coordinator, command, 1221);
    let completion = immediate_tab_completion(&pending);
    coordinator
        .complete_terminal_tab_create(pending, completion, Some(worker(1)))
        .unwrap();
    let before = coordinator
        .workbench
        .as_ref()
        .unwrap()
        .standalone_membership()
        .unwrap();
    let removed = &before.members()[0];
    let prepared = prepare_standalone_close_settlement(
        &mut coordinator.standalone,
        &coordinator.terminals,
        &before,
        &empty,
        removed,
    )
    .unwrap();
    drop(prepared);
    assert!(
        matches!(&coordinator.standalone, StandaloneTerminal::Settled { reference, .. } if reference == &id(1221))
    );
    assert_eq!(
        coordinator
            .workbench
            .as_ref()
            .unwrap()
            .standalone_membership()
            .unwrap(),
        before
    );
    let changed = prepare_standalone_close_settlement(
        &mut coordinator.standalone,
        &coordinator.terminals,
        &before,
        &empty,
        removed,
    )
    .unwrap()
    .commit();
    assert!(changed);
    assert_eq!(coordinator.standalone, StandaloneTerminal::Empty);
    // The slot-only helper deliberately cannot publish removal; joint finalizer tests cover that.
    assert_eq!(
        coordinator
            .workbench
            .as_ref()
            .unwrap()
            .standalone_membership()
            .unwrap(),
        before
    );
}

#[test]
fn prepared_inactive_removal_preserves_current_reference_and_rejects_wrong_tuple() {
    let mut coordinator = site();
    let first_command = tab_command(&coordinator, 1230, 0, "survivor");
    let first = dispatch_tab(&mut coordinator, first_command, 1231);
    let first_completion = immediate_tab_completion(&first);
    coordinator
        .complete_terminal_tab_create(first, first_completion, Some(worker(1)))
        .unwrap();
    let after = coordinator
        .workbench
        .as_ref()
        .unwrap()
        .standalone_membership()
        .unwrap();
    let second_command = tab_command(&coordinator, 1232, 1, "remove");
    let second = dispatch_tab(&mut coordinator, second_command, 1233);
    let second_completion = immediate_tab_completion(&second);
    coordinator
        .complete_terminal_tab_create(second, second_completion, Some(worker(1)))
        .unwrap();
    let before = coordinator
        .workbench
        .as_ref()
        .unwrap()
        .standalone_membership()
        .unwrap();
    assert!(
        prepare_standalone_close_settlement(
            &mut coordinator.standalone,
            &coordinator.terminals,
            &before,
            &after,
            &before.members()[0],
        )
        .is_err()
    );
    let changed = prepare_standalone_close_settlement(
        &mut coordinator.standalone,
        &coordinator.terminals,
        &before,
        &after,
        &before.members()[1],
    )
    .unwrap()
    .commit();
    assert!(!changed);
    assert_eq!(
        coordinator.standalone,
        StandaloneTerminal::Settled {
            reference: id(1231),
            outcome: OpenOutcome::Opened {
                terminal_create_request_id: id(1231)
            },
        }
    );
}
