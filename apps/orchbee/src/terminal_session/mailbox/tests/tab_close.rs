use super::*;
use crate::terminal_session::state::{CloseStateError, CloseStatusKind};

#[cfg(any(target_os = "linux", windows))]
#[test]
fn recovery_copies_burned_allocator_cursors_without_copying_live_mailbox_work() {
    let geometry = Geometry::new(80, 24, 8, 16).unwrap();
    let old = CommandMailbox::new(700, geometry);
    {
        let mut state = old.lock();
        state.attachment_generation = 9;
        state.refresh_generation = 11;
        state.next_command_serial = 15;
        state.next_ui_wait_serial = 19;
        state.desired = MailboxMembership::Live {
            desired: None,
            panes: Vec::new(),
        };
    }
    old.close(CarrierFailure::Closed);
    let seed = old.retired_seed();
    let fresh = CommandMailbox::for_status_recovery(seed, geometry);
    assert_eq!(fresh.retired_seed(), seed);
    let state = fresh.lock();
    assert_eq!(
        state.desired,
        MailboxMembership::Live {
            desired: None,
            panes: Vec::new(),
        }
    );
    assert!(state.closed.is_none());
    assert!(state.queue.is_empty());
    assert!(state.active_lease.is_none());
    assert!(state.close_flight.is_none());
    assert!(state.close_deposited_result.is_none());
    assert!(matches!(state.event, EventCell::Empty));
}

fn admit(
    mailbox: &CommandMailbox,
    state: &mut NativeSessionState,
    clock: &FakeClock,
    deadline: Instant,
) -> CloseOperation {
    state.open_close_confirmation().unwrap();
    let token = state.view().close_confirmation().unwrap().token();
    state.acknowledge_close_draw(Some(token), None);
    mailbox
        .try_enqueue_close_for_using(
            state,
            deadline,
            clock,
            |state, request_id, _| state.plan_close_confirmation(request_id, deadline),
            NativeSessionState::begin_close_exchange,
        )
        .unwrap()
}

#[test]
fn missing_worker_return_does_not_turn_a_proven_queued_successor_into_submission_uncertainty() {
    let base = Instant::now();
    let clock = FakeClock::new(base);
    let mailbox = bootstrapped_mailbox(1, 900);
    let mut state = ready_state(2);
    let first = admit(&mailbox, &mut state, &clock, base + Duration::from_secs(50));
    let _first_lease = mailbox.try_recv().unwrap().unwrap();
    assert_eq!(
        mailbox.mark_close_in_flight(&first).unwrap(),
        MutationResult::Applied
    );
    let wait = mailbox
        .publish_event_before_using(
            WorkerEvent::CloseResult {
                operation: first.clone(),
                outcome: CloseResult::NotSubmitted,
            },
            EventDeadline::PlacementOnly(first.deadline()),
            &clock,
        )
        .unwrap();
    let _taken = mailbox.take_event_before_using(&clock).unwrap().unwrap();
    mailbox
        .deposit_ui_reply_before_using(&wait, &clock, |_| {
            state
                .settle_close_exchange(&first, &CloseResult::NotSubmitted)
                .map_err(|_| CarrierFailure::Protocol)?;
            Ok(UiReply::Close(first.key().clone()))
        })
        .unwrap();
    let second = admit(&mailbox, &mut state, &clock, base + Duration::from_secs(50));
    assert_ne!(first.key(), second.key());
    mailbox.close(CarrierFailure::WorkerExited);
    assert_eq!(
        state.finalize_close_worker(mailbox.close_worker_loss()),
        Ok(())
    );
    assert!(state.uncertain_close_identity().is_none());
    assert!(state.view().close_status().is_none());
    assert_eq!(state.open_close_confirmation(), Ok(()));
}

#[test]
fn timely_typed_close_deposit_survives_mailbox_retirement_without_a_worker_return() {
    let base = Instant::now();
    let clock = FakeClock::new(base);
    let mailbox = bootstrapped_mailbox(1, 920);
    let mut state = ready_state(2);
    let operation = admit(&mailbox, &mut state, &clock, base + Duration::from_secs(50));
    let _lease = mailbox.try_recv().unwrap().unwrap();
    assert_eq!(
        mailbox.mark_close_in_flight(&operation).unwrap(),
        MutationResult::Applied
    );
    let target = operation.key().target();
    let accepted = crate::local_client::workbench::WorkbenchTabCloseAccepted {
        close_request_id: operation.key().request_id().clone(),
        terminal_context_id: target.terminal_context_id().clone(),
        tab_id: target.tab_id().clone(),
        accepted_revision: operation.selection().base_revision(),
    };
    mailbox
        .publish_event_before_using(
            WorkerEvent::CloseResult {
                operation: operation.clone(),
                outcome: CloseResult::Accepted(accepted),
            },
            EventDeadline::PlacementOnly(operation.deadline()),
            &clock,
        )
        .unwrap();
    mailbox.close(CarrierFailure::WorkerExited);
    assert_eq!(
        state.finalize_close_worker(mailbox.close_worker_loss()),
        Ok(())
    );
    assert!(state.uncertain_close_identity().is_none());
    assert_eq!(
        state.view().close_status().unwrap().status(),
        CloseStatusKind::AwaitingStatus
    );
    assert!(state.close_suppresses_attachment(state.desired().unwrap().anchor()));
    assert_eq!(
        state.open_close_confirmation(),
        Err(CloseStateError::NotResumable)
    );
}

#[test]
fn a_deposited_closed_read_survives_missing_worker_return_with_its_revision() {
    let base = Instant::now();
    let clock = FakeClock::new(base);
    let mailbox = bootstrapped_mailbox(1, 940);
    let mut state = ready_state(2);
    let operation = admit(&mailbox, &mut state, &clock, base + Duration::from_secs(50));
    let lease = mailbox.try_recv().unwrap().unwrap();
    mailbox.mark_close_in_flight(&operation).unwrap();
    let target = operation.key().target();
    let accepted = crate::local_client::workbench::WorkbenchTabCloseAccepted {
        close_request_id: operation.key().request_id().clone(),
        terminal_context_id: target.terminal_context_id().clone(),
        tab_id: target.tab_id().clone(),
        accepted_revision: operation.selection().base_revision(),
    };
    let result = CloseResult::Accepted(accepted);
    let wait = mailbox
        .publish_event_before_using(
            WorkerEvent::CloseResult {
                operation: operation.clone(),
                outcome: result.clone(),
            },
            EventDeadline::PlacementOnly(operation.deadline()),
            &clock,
        )
        .unwrap();
    mailbox.take_event_before_using(&clock).unwrap().unwrap();
    mailbox
        .deposit_ui_reply_before_using(&wait, &clock, |_| {
            state
                .settle_close_exchange(&operation, &result)
                .map_err(|_| CarrierFailure::Protocol)?;
            Ok(UiReply::Close(operation.key().clone()))
        })
        .unwrap();
    mailbox.await_ui_reply(&wait).unwrap().unwrap();
    mailbox.finish_lease(lease).unwrap();
    let deadline = base + Duration::from_secs(50);
    let read = mailbox
        .try_enqueue_close_for_using(
            &mut state,
            deadline,
            &clock,
            |state, id, now| state.plan_close_read(id, deadline, now),
            NativeSessionState::begin_close_exchange,
        )
        .unwrap();
    let _lease = mailbox.try_recv().unwrap().unwrap();
    mailbox.mark_close_in_flight(&read).unwrap();
    let closed = orchbee_workbench_protocol::WorkbenchTabCloseReadOutcome::Available {
        close_request_id: operation.key().request_id().clone(),
        terminal_context_id: target.terminal_context_id().clone(),
        tab_id: target.tab_id().clone(),
        attempt_generation: orchbee_workbench_protocol::WorkbenchTabCloseAttemptGeneration::new(1)
            .unwrap(),
        status: orchbee_workbench_protocol::WorkbenchTabCloseStatus::Closed {
            revision: WorkbenchRevision::new(2),
        },
    };
    mailbox
        .publish_event_before_using(
            WorkerEvent::CloseResult {
                operation: read,
                outcome: CloseResult::Read(closed),
            },
            EventDeadline::PlacementOnly(deadline),
            &clock,
        )
        .unwrap();
    mailbox.close(CarrierFailure::WorkerExited);
    assert_eq!(
        state.finalize_close_worker(mailbox.close_worker_loss()),
        Ok(())
    );
    assert_eq!(
        state.view().close_status().unwrap().status(),
        CloseStatusKind::Closed
    );
    assert!(state.uncertain_close_identity().is_none());
}

#[cfg(any(target_os = "linux", windows))]
#[test]
fn retired_recovery_seed_never_resets_any_exhausted_allocator() {
    for exhausted in 0..5 {
        let old = bootstrapped_mailbox(9, 700);
        {
            let mut state = old.lock();
            match exhausted {
                0 => state.identity_cursor = u128::MAX,
                1 => state.refresh_generation = u64::MAX,
                2 => state.next_command_serial = u64::MAX,
                3 => state.next_ui_wait_serial = u64::MAX,
                _ => state.attachment_generation = u64::MAX,
            }
        }
        old.close(CarrierFailure::Closed);
        let seed = old.retired_seed();
        let fresh = CommandMailbox::for_status_recovery(seed, geometry(80));
        let clock = crate::local_client::test_server::adjustable_operation_clock(Arc::new(
            std::sync::atomic::AtomicU64::new(0),
        ));
        let deadline = clock.now() + Duration::from_secs(50);
        if exhausted == 0 {
            assert!(fresh.allocate_workbench_request_id().is_err());
        } else if exhausted == 1 || exhausted == 2 {
            assert!(
                fresh
                    .try_enqueue_refresh(deadline, |_| Ok::<_, ()>(()))
                    .is_err()
            );
        } else if exhausted == 4 {
            let mut state = fresh.lock();
            let mut transaction = UiReplyTransaction {
                state: &mut state,
                observed_at: clock.now(),
            };
            assert!(matches!(
                transaction.prepare_desired_install(DesiredInstall::Fresh {
                    tab_id: key(1, 2).tab_id().clone(),
                    panel: panel()
                }),
                Err(CarrierFailure::IdentityExhausted)
            ));
            assert!(transaction.state.desired.key().is_none());
            transaction
                .prepare_desired_install(DesiredInstall::Empty)
                .unwrap()
                .commit();
        } else {
            let generation = fresh
                .try_enqueue_refresh(deadline, |_| Ok::<_, ()>(()))
                .unwrap();
            let _lease = fresh.try_recv().unwrap().unwrap();
            fresh.mark_refresh_in_flight(generation).unwrap();
            assert!(
                fresh
                    .publish_refresh_result_before(
                        generation,
                        WorkbenchSnapshotOutcome::Denied {},
                        deadline,
                        &clock
                    )
                    .is_err()
            );
        }
        assert_eq!(
            fresh.cursors().0,
            if exhausted == 4 { u64::MAX } else { 9 },
            "no fabricated attachment generation"
        );
        let state = fresh.lock();
        match exhausted {
            0 => assert_eq!(state.identity_cursor, u128::MAX),
            1 => assert_eq!(state.refresh_generation, u64::MAX),
            2 => assert_eq!(state.next_command_serial, u64::MAX),
            3 => assert_eq!(state.next_ui_wait_serial, u64::MAX),
            _ => assert_eq!(state.attachment_generation, u64::MAX),
        }
    }
}
