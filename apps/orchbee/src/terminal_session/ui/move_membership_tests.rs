use super::*;
use crate::local_client::test_server::adjustable_operation_clock;
use crate::local_client::workbench::WorkbenchTabMoveCommit;
use crate::terminal_session::mailbox::{CommandLease, EventDeadline, UiWaitKey};
use crate::terminal_session::state::{
    CleanupContinuation, MoveDecision, MoveOperation, MoveResult,
};
use orchbee_workbench_protocol::{Snapshot, WorkbenchRevision, WorkbenchSnapshotOutcome};
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) fn ready_ui() -> (
    NativeUi,
    Arc<CommandMailbox>,
    LocalOperationClock,
    Arc<AtomicU64>,
) {
    let elapsed = Arc::new(AtomicU64::new(0));
    let clock = adjustable_operation_clock(Arc::clone(&elapsed));
    let mailbox = Arc::new(CommandMailbox::new(
        10,
        Geometry::new(80, 24, 8, 16).unwrap(),
    ));
    let mut ui = NativeUi::new(Arc::clone(&mailbox), clock.clone());
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::Bootstrap {
                cache:
                    crate::terminal_session::workbench_projection::project_native_standalone_tabs(
                        &super::membership_tests::snapshot(2, 1),
                    )
                    .unwrap(),
            },
            EventDeadline::Startup(clock.now() + crate::terminal_session::state::OPERATION_TOTAL),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
    let state = ui.state.as_mut().unwrap();
    let key = state.desired().unwrap().clone();
    assert!(state.install_projection(&key));
    assert!(state.record_setup_configuration(&key, ControllerMode::ReadOnly, None));
    assert!(state.stage_drawn_frame(&key, FrameIdentity::new(1)));
    assert!(state.mark_feedback_accepted(&key, FrameIdentity::new(1)));
    assert!(state.publish_ready(&key, FrameIdentity::new(1), ControllerMode::ReadOnly, None));
    (ui, mailbox, clock, elapsed)
}

fn place_move(
    ui: &mut NativeUi,
    mailbox: &CommandMailbox,
    clock: &LocalOperationClock,
    candidate: Snapshot,
) -> (CommandLease, UiWaitKey, MoveOperation) {
    assert_eq!(
        ui.apply_client_action(ClientAction::MoveNextTab),
        SessionActionDisposition::DismissNavigator
    );
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::Move(marker) = lease.record() else {
        panic!("one Move marker");
    };
    let operation = marker.operation().clone();
    assert_eq!(
        mailbox.mark_move_in_flight(&operation).unwrap(),
        MutationResult::Applied
    );
    let wait = mailbox
        .publish_move_result_before(
            operation.clone(),
            MoveResult::Committed {
                commit: WorkbenchTabMoveCommit {
                    revision: WorkbenchRevision::new(operation.baseline().revision().value() + 1),
                    terminal_context_id: operation.baseline().terminal_context_id().clone(),
                    tab_id: operation.target().tab_id().clone(),
                },
                outcome: WorkbenchSnapshotOutcome::Available {
                    snapshot: candidate,
                },
            },
            operation.deadline(),
            clock,
        )
        .unwrap();
    (lease, wait, operation)
}

#[test]
fn actual_move_ui_stages_later_removal_after_timely_result_without_destroying_old_viewer() {
    for (neighbor, millis, wrong_key) in [
        (false, 0, false),
        (true, 44_999, false),
        (false, 45_000, false),
        (false, 49_999, false),
        (false, 50_000, false),
        (false, 0, true),
    ] {
        let (mut ui, mailbox, clock, elapsed) = ready_ui();
        let baseline = ui.state.as_ref().unwrap().cache().clone();
        let old = ui.state.as_ref().unwrap().desired().unwrap().clone();
        let mut candidate = super::membership_tests::snapshot(if neighbor { 2 } else { 0 }, 3);
        if neighbor {
            candidate.terminal_contexts[0].tabs.remove(0);
        }
        let (lease, wait, operation) = place_move(&mut ui, &mailbox, &clock, candidate);
        elapsed.store(75_000, Ordering::SeqCst);
        ui.tick();
        assert!(
            !ui.retirement_requested(),
            "later removal must stage common exact cleanup"
        );
        let state = ui.state.as_ref().unwrap();
        assert_eq!(state.cache(), &baseline);
        assert_eq!(state.attached_key(), Some(&old));
        assert!(state.ready().is_none());
        assert_eq!(
            state.retained_move_for_test().unwrap().operation(),
            &operation
        );
        assert!(state.invariant_holds());
        let Some(UiReply::Move(settled)) = mailbox.take_deposited_reply(&wait).unwrap() else {
            panic!("Move settlement");
        };
        let MoveDecision::Cleanup(cleanup) = settled.decision() else {
            panic!("common cleanup");
        };
        assert_eq!(
            cleanup.deadlines().forward_cutoff(),
            clock.now() + std::time::Duration::from_secs(45)
        );
        let detached = if wrong_key {
            AttachmentKey::new(old.generation() + 1, old.tab_id().clone())
        } else {
            old.clone()
        };
        let wait = mailbox
            .publish_event_before(
                WorkerEvent::MembershipDetached {
                    cleanup: cleanup.clone(),
                    detached,
                },
                EventDeadline::TotalCleanup(cleanup.deadlines().total()),
                &clock,
            )
            .unwrap();
        elapsed.store(75_000 + millis, Ordering::SeqCst);
        ui.tick();
        let state = ui.state.as_ref().unwrap();
        assert_eq!(
            state.retained_move_for_test().unwrap().operation(),
            &operation
        );
        if millis < 45_000 && !wrong_key {
            assert!(!ui.retirement_requested());
            assert!(!state.move_blocks_structure());
            assert!(state.invariant_holds());
            let Some(UiReply::Membership(settlement)) =
                mailbox.take_deposited_reply(&wait).unwrap()
            else {
                panic!("exact cleanup settlement");
            };
            if neighbor {
                assert!(
                    matches!(settlement.decision(), RefreshDecision::AttachRecovered(target) if target.selected().key().generation() == old.generation() + 1)
                );
                assert_eq!(state.cache().tabs().len(), 1);
            } else {
                assert_eq!(ui.view().phase(), NativeDisplayPhase::Empty);
                assert!(settlement.desired().is_none());
            }
            mailbox.finish_lease(lease).unwrap();
        } else {
            assert!(ui.retirement_requested());
            assert_eq!(state.cache(), &baseline);
        }
    }
}

#[test]
fn actual_tab_unavailable_ui_keeps_ready_and_allows_an_ordinary_explicit_refresh() {
    let (mut ui, mailbox, clock, elapsed) = ready_ui();
    let state = ui.state.as_ref().unwrap();
    let baseline = state.cache().clone();
    let old_ready = state.ready().cloned();
    let old = state.desired().cloned();
    ui.apply_client_action(ClientAction::MoveNextTab);
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::Move(marker) = lease.record() else {
        panic!("Move marker");
    };
    let operation = marker.operation().clone();
    assert_eq!(
        mailbox.mark_move_in_flight(&operation).unwrap(),
        MutationResult::Applied
    );
    let wait = mailbox
        .publish_move_result_before(
            operation.clone(),
            MoveResult::TabUnavailable {
                revision: WorkbenchRevision::new(3),
                outcome: WorkbenchSnapshotOutcome::Denied {},
            },
            operation.deadline(),
            &clock,
        )
        .unwrap();
    elapsed.store(75_000, Ordering::SeqCst);
    ui.tick();
    assert!(matches!(
        mailbox.take_deposited_reply(&wait).unwrap(),
        Some(UiReply::Move(_))
    ));
    mailbox.finish_lease(lease).unwrap();
    let state = ui.state.as_ref().unwrap();
    assert_eq!(state.cache(), &baseline);
    assert_eq!(state.ready(), old_ready.as_ref());
    assert_eq!(state.desired(), old.as_ref());
    assert_eq!(state.notice(), Some(SessionNotice::MoveNotMoved));
    assert!(state.retained_move_for_test().is_none());
    assert!(!state.move_blocks_structure());
    assert!(!ui.retirement_requested());
    ui.apply_client_action(ClientAction::OpenWorkbenchNavigator);
    let refresh = mailbox.try_recv().unwrap().unwrap();
    assert!(
        matches!(refresh.record(), QueuedCommand::Refresh(_)),
        "recovery is an ordinary explicit Snapshot marker"
    );
}

#[test]
fn actual_unattached_move_installs_a_fresh_successor_or_empty_without_cleanup() {
    for neighbor in [false, true] {
        let (mut ui, mailbox, clock, _) = ready_ui();
        let state = ui.state.as_mut().unwrap();
        let old = state.desired().unwrap().clone();
        let deadlines = OperationDeadlines::from_sample(clock.now()).unwrap();
        mailbox
            .try_enqueue_switch_for(
                state,
                |state, generation| {
                    let key =
                        AttachmentKey::new(generation, state.tab(1).unwrap().tab_id().clone());
                    let target = state.target_for_switch(1, key, deadlines).unwrap();
                    assert!(state.begin_switch(target.clone()));
                    Ok::<_, ()>(SwitchMarker::new(target))
                },
                |state, result, facts| {
                    if result.is_ok() {
                        state.finish_switch_membership_admission(
                            facts.discarded(),
                            facts.inherited(),
                            facts.authorized_cleanup(),
                        );
                    }
                },
            )
            .unwrap();
        let lease = mailbox.try_recv().unwrap().unwrap();
        let QueuedCommand::Switch(marker) = lease.record() else {
            panic!("Switch lease")
        };
        let permit = mailbox
            .authorize_switch_snapshot(&lease, marker.target(), &clock)
            .unwrap()
            .unwrap();
        mailbox.answer_switch_snapshot(permit).unwrap();
        let wait = mailbox
            .publish_event_before(
                WorkerEvent::SwitchSnapshotResult {
                    marker: marker.target().selected().key().clone(),
                    outcome: WorkbenchSnapshotOutcome::Available {
                        snapshot: super::membership_tests::snapshot(2, 2),
                    },
                },
                EventDeadline::PlacementOnly(deadlines.forward_cutoff()),
                &clock,
            )
            .unwrap();
        ui.tick();
        let Some(UiReply::Switch(settlement)) = mailbox.take_deposited_reply(&wait).unwrap() else {
            panic!("Switch candidate reply")
        };
        let RefreshDecision::Cleanup(cleanup) = settlement.decision() else {
            panic!("old viewer cleanup")
        };
        let permit = mailbox
            .authorize_switch_cleanup(&lease, cleanup, &old, &clock)
            .unwrap()
            .unwrap();
        mailbox.answer_switch_cleanup(permit, true).unwrap();
        let wait = mailbox
            .publish_event_before(
                WorkerEvent::MembershipDetached {
                    cleanup: cleanup.clone(),
                    detached: old,
                },
                EventDeadline::TotalCleanup(deadlines.total()),
                &clock,
            )
            .unwrap();
        ui.tick();
        let Some(UiReply::Switch(settlement)) = mailbox.take_deposited_reply(&wait).unwrap() else {
            panic!("Switch successor reply")
        };
        let RefreshDecision::AttachRecovered(target) = settlement.decision() else {
            panic!("fresh successor")
        };
        let selected = target.selected().key().clone();
        let state = ui.state.as_mut().unwrap();
        assert_eq!(
            state.settle_unattached_setup(
                &selected,
                target.selected().index(),
                target.provenance()
            ),
            Some(CleanupContinuation::Stable)
        );
        mailbox.finish_lease(lease).unwrap();
        assert!(state.invariant_holds());
        assert!(state.attached_key().is_none());
        let mut candidate = super::membership_tests::snapshot(if neighbor { 2 } else { 0 }, 4);
        if neighbor {
            candidate.terminal_contexts[0].tabs.remove(1);
        }
        let (lease, wait, operation) = place_move(&mut ui, &mailbox, &clock, candidate);
        ui.tick();
        assert!(!ui.retirement_requested());
        let Some(UiReply::Move(settlement)) = mailbox.take_deposited_reply(&wait).unwrap() else {
            panic!("direct Move installation");
        };
        if neighbor {
            assert!(
                matches!(settlement.decision(), MoveDecision::AttachRecovered(target)
                if target.selected().key().generation() == selected.generation() + 1)
            );
        } else {
            assert_eq!(
                settlement.decision(),
                &MoveDecision::SettledWithoutAttachment
            );
            assert!(settlement.desired().is_none());
            assert_eq!(mailbox.cursors().0, selected.generation());
        }
        let state = ui.state.as_ref().unwrap();
        assert!(state.attached_key().is_none());
        assert!(!state.move_blocks_structure());
        assert_eq!(
            state.retained_move_for_test().unwrap().operation(),
            &operation
        );
        assert!(state.invariant_holds());
        mailbox.finish_lease(lease).unwrap();
    }
}
