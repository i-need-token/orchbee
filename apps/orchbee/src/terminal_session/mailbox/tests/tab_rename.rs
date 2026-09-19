use super::*;
use crate::terminal_session::state::rename_draft::RenameDraftEdit;

#[test]
fn rename_real_admission_refuses_unseen_and_full_without_burning_identity_or_draft() {
    let base = Instant::now();
    let deadline = base + Duration::from_secs(50);
    let clock = FakeClock::new(base);
    let mailbox = bootstrapped_mailbox(1, 5);
    let mut state = ready_state(3);
    let token = state.open_rename_draft().unwrap();
    state.edit_rename_draft(token, RenameDraftEdit::Clear);
    state.edit_rename_draft(token, RenameDraftEdit::Append(" exact 雪 "));
    let before = mailbox.cursors();
    assert!(
        mailbox
            .try_enqueue_rename_for_using(&mut state, token, deadline, &clock)
            .is_err()
    );
    assert_eq!(mailbox.cursors(), before);
    assert_eq!(state.rename_draft().unwrap().text(), " exact 雪 ");
    state.acknowledge_rename_draw(Some(token));
    for _ in 0..COMMAND_CAPACITY {
        mailbox.try_enqueue(input(&mailbox, 0)).unwrap();
    }
    let before = mailbox.cursors();
    assert!(
        mailbox
            .try_enqueue_rename_for_using(&mut state, token, deadline, &clock)
            .is_err()
    );
    assert_eq!(mailbox.cursors(), before);
    assert!(state.plan_rename_draft(token).is_some());
}

#[test]
fn rename_actual_admission_captures_once_and_closes_draft_with_shared_fence() {
    let base = Instant::now();
    let deadline = base + Duration::from_secs(50);
    let mailbox = bootstrapped_mailbox(1, 5);
    let mut state = ready_state(3);
    let token = state.open_rename_draft().unwrap();
    state.acknowledge_rename_draw(Some(token));
    let operation = mailbox
        .try_enqueue_rename_for_using(&mut state, token, deadline, &FakeClock::new(base))
        .unwrap();
    assert_eq!(mailbox.cursors(), (1, 0, 6));
    assert!(state.rename_draft().is_none());
    assert_eq!(operation.target(), &key(1, 1));
    assert_eq!(operation.deadline(), deadline);
    assert!(state.open_rename_draft().is_none());
    assert!(
        state
            .plan_move(
                WorkbenchRequestId::parse(&id(200)).unwrap(),
                orchbee_workbench_protocol::WorkbenchTabMoveDirection::Next,
                deadline
            )
            .is_none()
    );
    let lease = mailbox.try_recv().unwrap().unwrap();
    assert!(matches!(lease.record(), QueuedCommand::Rename(_)));
    assert_eq!(
        mailbox.mark_rename_in_flight(&operation).unwrap(),
        MutationResult::Applied
    );
    assert!(matches!(
        mailbox.try_enqueue_refresh(deadline, |_| Ok::<_, ()>(())),
        Err(AdmissionError::Full)
    ));
}

#[test]
fn rename_admission_boundary_refusals_leave_all_cursors_and_exact_text_unchanged() {
    let base = Instant::now();
    let deadline = base + Duration::from_secs(50);
    for cut in 0..7 {
        let mailbox = bootstrapped_mailbox(1, if cut == 0 { u128::MAX } else { 5 });
        let mut state = ready_state(3);
        let token = state.open_rename_draft().unwrap();
        state.acknowledge_rename_draw(Some(token));
        if cut == 1 {
            mailbox.lock().next_command_serial = u64::MAX;
        }
        if cut == 2 {
            state.edit_rename_draft(token, RenameDraftEdit::Fault);
        }
        if cut == 3 {
            state.edit_rename_draft(token, RenameDraftEdit::Completeness(false));
        }
        if cut == 4 {
            mailbox.lock().desired = MailboxMembership::Live {
                desired: Some(pane_key(&key(2, 1))),
                panes: vec![pane_ref(&key(2, 1))],
            };
        }
        let before = mailbox.cursors();
        let serial = mailbox.lock().next_command_serial;
        let text = state.rename_draft().unwrap().text().to_owned();
        let now = match cut {
            5 => deadline,
            6 => deadline + Duration::from_nanos(1),
            _ => base,
        };
        assert!(
            mailbox
                .try_enqueue_rename_for_using(&mut state, token, deadline, &FakeClock::new(now))
                .is_err(),
            "cut {cut}"
        );
        assert_eq!(mailbox.cursors(), before);
        assert_eq!(mailbox.lock().next_command_serial, serial);
        assert!(mailbox.lock().queue.is_empty());
        assert_eq!(state.rename_draft().unwrap().text(), text);
        assert!(state.pending_rename().is_none());
    }
}

#[test]
fn rename_typed_placement_has_one_cutoff_but_ui_ack_can_be_late_with_b_queued() {
    let base = Instant::now();
    let deadline = base + Duration::from_secs(50);
    let mailbox = bootstrapped_mailbox(1, 5);
    let mut state = ready_state(3);
    let token = state.open_rename_draft().unwrap();
    state.acknowledge_rename_draw(Some(token));
    let clock = FakeClock::new(base);
    let first = mailbox
        .try_enqueue_rename_for_using(&mut state, token, deadline, &clock)
        .unwrap();
    let lease = mailbox.try_recv().unwrap().unwrap();
    mailbox.mark_rename_in_flight(&first).unwrap();
    let wait = mailbox
        .publish_event_before_using(
            WorkerEvent::RenameResult {
                operation: first.clone(),
                outcome: RenameResult::Unchanged(WorkbenchRevision::new(1)),
            },
            EventDeadline::PlacementOnly(deadline),
            &clock,
        )
        .unwrap();
    clock.set(deadline + Duration::from_secs(100));
    mailbox.take_event_before_using(&clock).unwrap().unwrap();
    mailbox
        .deposit_ui_reply_before_using(&wait, &clock, |_| {
            state.settle_rename_without_membership(
                &first,
                crate::terminal_session::state::tab_rename::RenamePreparation::Unchanged,
            )?;
            Ok(UiReply::Rename(RenameSettlement::new(
                first.request_id().clone(),
                RefreshDecision::Continue,
                state.desired().cloned(),
            )))
        })
        .unwrap();
    let token = state.open_rename_draft().unwrap();
    state.acknowledge_rename_draw(Some(token));
    let second = mailbox
        .try_enqueue_rename_for_using(
            &mut state,
            token,
            deadline + Duration::from_secs(200),
            &clock,
        )
        .unwrap();
    assert_ne!(first.request_id(), second.request_id());
    assert!(mailbox.try_recv().unwrap().is_none());
    assert!(
        matches!(mailbox.take_deposited_reply(&wait).unwrap(), Some(UiReply::Rename(reply)) if reply.request_id() == first.request_id())
    );
    assert_eq!(state.pending_rename(), Some(&second));
    mailbox.finish_lease(lease).unwrap();
    assert!(mailbox.try_recv().unwrap().is_some());
}

#[test]
fn rename_missing_worker_preserves_exact_handoff_evidence_through_mailbox_close() {
    use crate::terminal_session::state::tab_rename::RenameWorkerEvidence;
    for cut in 0..3 {
        let base = Instant::now();
        let deadline = base + Duration::from_secs(50);
        let mailbox = bootstrapped_mailbox(1, 5);
        let mut state = ready_state(3);
        let token = state.open_rename_draft().unwrap();
        state.edit_rename_draft(token, RenameDraftEdit::Append("changed"));
        state.acknowledge_rename_draw(Some(token));
        let clock = FakeClock::new(base);
        let operation = mailbox
            .try_enqueue_rename_for_using(&mut state, token, deadline, &clock)
            .unwrap();
        let _lease = if cut > 0 {
            let lease = mailbox.try_recv().unwrap().unwrap();
            mailbox.mark_rename_in_flight(&operation).unwrap();
            Some(lease)
        } else {
            None
        };
        if cut == 2 {
            mailbox
                .publish_event_before_using(
                    WorkerEvent::RenameResult {
                        operation: operation.clone(),
                        outcome: RenameResult::Committed {
                            revision: WorkbenchRevision::new(2),
                            outcome: WorkbenchSnapshotOutcome::Denied {},
                        },
                    },
                    EventDeadline::PlacementOnly(deadline),
                    &clock,
                )
                .unwrap();
        }
        mailbox.close(CarrierFailure::WorkerExited);
        let evidence = mailbox.rename_worker_loss();
        match (cut, evidence) {
            (0, None) => {}
            (1, Some(RenameWorkerEvidence::Unknown(current))) => assert_eq!(current, operation),
            (2, Some(RenameWorkerEvidence::Known(known))) => {
                assert_eq!(known.operation(), &operation);
                assert_eq!(known.revision(), WorkbenchRevision::new(2));
            }
            _ => panic!("incorrect handoff evidence at cut {cut}"),
        }
    }
}
