use super::*;
use crate::terminal_session::mailbox::EventDeadline;
use crate::terminal_session::state::tab_rename::RenameResult;
use orchbee_workbench_protocol::{WorkbenchRevision, WorkbenchSnapshotOutcome};
use std::{sync::atomic::Ordering, time::Duration};

#[test]
fn rename_removed_candidate_samples_one_posttransport_pair_and_never_renews_at_cleanup() {
    for (millis, wrong_key) in [
        (0, false),
        (44_999, false),
        (45_000, false),
        (49_999, false),
        (50_000, false),
        (0, true),
    ] {
        let (mut ui, mailbox, clock, elapsed) = super::move_membership_tests::ready_ui();
        let baseline = ui.state.as_ref().unwrap().cache().clone();
        let old = ui.state.as_ref().unwrap().desired().unwrap().clone();
        let token = ui.request_rename_draft().unwrap();
        ui.edit_rename_draft(token, NativeRenameEdit::Append("changed"));
        ui.after_rename_draw(Some(token));
        assert_eq!(
            ui.submit_rename_draft(token),
            SessionActionDisposition::DismissNavigator
        );
        let lease = mailbox.try_recv().unwrap().unwrap();
        let QueuedCommand::Rename(marker) = lease.record() else {
            panic!("Rename marker");
        };
        let operation = marker.operation().clone();
        mailbox.mark_rename_in_flight(&operation).unwrap();
        let wait = mailbox
            .publish_rename_result_before(
                operation.clone(),
                RenameResult::Committed {
                    revision: WorkbenchRevision::new(2),
                    outcome: WorkbenchSnapshotOutcome::Available {
                        snapshot: super::membership_tests::snapshot(0, 3),
                    },
                },
                operation.deadline(),
                &clock,
            )
            .unwrap();
        elapsed.store(75_000, Ordering::SeqCst);
        ui.tick();
        assert!(!ui.retirement_requested());
        let state = ui.state.as_ref().unwrap();
        assert_eq!(state.cache(), &baseline);
        assert_eq!(state.attached_key(), Some(&old));
        assert_eq!(
            state.rename_receipts_for_test().1.unwrap().operation(),
            &operation
        );
        let Some(UiReply::Rename(reply)) = mailbox.take_deposited_reply(&wait).unwrap() else {
            panic!("Rename reply");
        };
        let RefreshDecision::Cleanup(cleanup) = reply.decision() else {
            panic!("common cleanup");
        };
        assert_eq!(
            cleanup.deadlines().forward_cutoff(),
            clock.now() + Duration::from_secs(45)
        );
        assert_eq!(
            cleanup.deadlines().total(),
            clock.now() + Duration::from_secs(50)
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
            state.rename_receipts_for_test().1.unwrap().operation(),
            &operation
        );
        if millis < 45_000 && !wrong_key {
            assert!(!ui.retirement_requested());
            assert_eq!(ui.view().phase(), NativeDisplayPhase::Empty);
            assert!(state.pending_rename().is_none());
            assert!(
                matches!(mailbox.take_deposited_reply(&wait).unwrap(), Some(UiReply::Rename(reply)) if reply.request_id() == operation.request_id() && reply.desired().is_none())
            );
            mailbox.finish_lease(lease).unwrap();
        } else {
            assert!(ui.retirement_requested());
            assert_eq!(state.cache(), &baseline);
        }
    }
}

#[test]
fn rename_retained_key_consumes_timely_result_after_original_cutoff_without_membership_budget() {
    let (mut ui, mailbox, clock, elapsed) = super::move_membership_tests::ready_ui();
    let old = ui.state.as_ref().unwrap().desired().unwrap().clone();
    let token = ui.request_rename_draft().unwrap();
    ui.edit_rename_draft(token, NativeRenameEdit::Clear);
    ui.edit_rename_draft(token, NativeRenameEdit::Append("new"));
    ui.after_rename_draw(Some(token));
    ui.submit_rename_draft(token);
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::Rename(marker) = lease.record() else {
        panic!("Rename marker");
    };
    let operation = marker.operation().clone();
    mailbox.mark_rename_in_flight(&operation).unwrap();
    let mut snapshot = super::membership_tests::snapshot(2, 2);
    snapshot.terminal_contexts[0].tabs[0].label = "new".into();
    let wait = mailbox
        .publish_rename_result_before(
            operation.clone(),
            RenameResult::Committed {
                revision: WorkbenchRevision::new(2),
                outcome: WorkbenchSnapshotOutcome::Available { snapshot },
            },
            operation.deadline(),
            &clock,
        )
        .unwrap();
    elapsed.store(1_000_000, Ordering::SeqCst);
    ui.tick();
    assert!(!ui.retirement_requested());
    assert_eq!(ui.status(), super::super::TerminalStatus::ReadOnly);
    assert_eq!(ui.state.as_ref().unwrap().desired(), Some(&old));
    let Some(UiReply::Rename(reply)) = mailbox.take_deposited_reply(&wait).unwrap() else {
        panic!("Rename reply");
    };
    assert_eq!(reply.decision(), &RefreshDecision::Continue);
    assert_eq!(reply.desired(), Some(&old));
    assert_eq!(operation.deadline() + Duration::from_secs(950), clock.now());
    mailbox.finish_lease(lease).unwrap();
}
