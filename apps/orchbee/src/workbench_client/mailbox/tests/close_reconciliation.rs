use super::*;
use crate::local_client::workbench::WorkbenchTabCloseAccepted;
use orchbee_workbench_protocol::{
    WorkbenchTabCloseAttemptGeneration, WorkbenchTabCloseReadOutcome, WorkbenchTabCloseStatus,
};

fn closed_state(now: Instant) -> WorkbenchClientState {
    let mut state = ready_state(2);
    state.open_close_confirmation().unwrap();
    let token = state.view().close_confirmation().unwrap().token();
    state.acknowledge_close_draw(Some(token), None);
    let close = state
        .plan_close_confirmation(
            WorkbenchRequestId::parse(&id(700)).unwrap(),
            now + Duration::from_secs(50),
        )
        .unwrap();
    state.begin_close_exchange(&close, now).unwrap();
    state
        .settle_close_exchange(
            &close,
            &CloseResult::Accepted(WorkbenchTabCloseAccepted {
                close_request_id: close.key().request_id().clone(),
                terminal_context_id: close.key().target().terminal_context_id().clone(),
                tab_id: close.key().target().tab_id().clone(),
                accepted_revision: close.selection().base_revision(),
            }),
        )
        .unwrap();
    let read = state
        .plan_close_read(
            WorkbenchRequestId::parse(&id(701)).unwrap(),
            now + Duration::from_secs(50),
            now,
        )
        .unwrap();
    state.begin_close_exchange(&read, now).unwrap();
    state
        .settle_close_exchange(
            &read,
            &CloseResult::Read(WorkbenchTabCloseReadOutcome::Available {
                close_request_id: close.key().request_id().clone(),
                terminal_context_id: close.key().target().terminal_context_id().clone(),
                tab_id: close.key().target().tab_id().clone(),
                attempt_generation: WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
                status: WorkbenchTabCloseStatus::Closed {
                    revision: WorkbenchRevision::new(2),
                },
            }),
        )
        .unwrap();
    state
}

#[test]
fn rejected_close_install_plan_preserves_both_owners_and_the_unconsumed_origin() {
    let now = Instant::now();
    let clock = FakeClock::new(now);
    let mut state = closed_state(now);
    let mailbox = bootstrapped_mailbox(1, 900);
    let marker = mailbox
        .try_enqueue_close_reconciliation_using(&mut state, now + Duration::from_secs(50), &clock)
        .unwrap();
    let before = state.cache().clone();
    let desired = state.desired().cloned();
    let candidate = state
        .prepare_close_membership_candidate(
            &marker,
            WorkbenchSnapshotOutcome::Available {
                snapshot: Snapshot {
                    protocol_version: ProtocolVersion,
                    revision: WorkbenchRevision::new(2),
                    navigation: {
                        let fixture_context_id = before.terminal_context_id().unwrap().clone();
                        let fixture_workspace_id =
                            orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                                fixture_context_id.as_str(),
                            )
                            .unwrap();
                        Navigation {
                            project_order: Vec::new(),
                            directories: Vec::new(),
                            workspaces: std::iter::once(orchbee_workbench_protocol::Workspace {
                                workspace_id: fixture_workspace_id.clone(),
                                terminal_context_id: fixture_context_id.clone(),
                                project_id: None,
                                primary_directory_id: None,
                                label: "Workspace".into(),
                            })
                            .chain(Vec::new())
                            .collect(),
                            workspace_order: Vec::new(),
                            projects: Vec::new(),
                        }
                    },
                    site_id: before.site_id().clone(),
                    terminal_contexts: vec![TerminalContext {
                        terminal_context_id: before.terminal_context_id().unwrap().clone(),
                        groups: Vec::new(),
                        tabs: Vec::new(),
                    }],
                },
            },
        )
        .unwrap()
        .unwrap();
    let cleanup = state
        .stage_close_membership_cleanup(
            &marker,
            candidate,
            OperationDeadlines::from_sample(now).unwrap(),
        )
        .unwrap();
    let mut locked = mailbox.lock();
    let generation = locked.attachment_generation;
    let mut transaction = UiReplyTransaction {
        state: &mut locked,
        observed_at: now,
    };
    let first = transaction
        .prepare_desired_install(DesiredInstall::Fresh {
            tab_id: key(1, 999).tab_id().clone(),
            panel: panel(),
        })
        .unwrap();
    let candidate = state
        .prepare_cleaned_membership_candidate(&cleanup, cleanup.old())
        .unwrap();
    assert!(matches!(
        state.prepare_cleaned_membership_install(
            &cleanup,
            candidate,
            first.desired().map(|pane| pane.anchor().clone())
        ),
        Err(CarrierFailure::Protocol)
    ));
    drop(first);
    assert_eq!(state.cache(), &before);
    assert_eq!(state.desired(), desired.as_ref());
    assert_eq!(
        state.close_reconciliation_origin(marker.origin().key()),
        Some(marker.origin())
    );
    assert_eq!(transaction.state.desired.key(), desired.as_ref());
    assert_eq!(transaction.state.attachment_generation, generation);
    assert!(state.invariant_holds());
    let first = transaction
        .prepare_desired_install(DesiredInstall::Empty)
        .unwrap();
    let candidate = state
        .prepare_cleaned_membership_candidate(&cleanup, cleanup.old())
        .unwrap();
    let second = state
        .prepare_cleaned_membership_install(
            &cleanup,
            candidate,
            first.desired().map(|pane| pane.anchor().clone()),
        )
        .unwrap();
    second.commit();
    first.commit();
    assert!(state.cache().tabs().is_empty());
    assert!(state.desired().is_none());
    assert!(transaction.state.desired.key().is_none());
    assert_eq!(transaction.state.attachment_generation, generation);
    assert!(
        state
            .close_reconciliation_origin(marker.origin().key())
            .is_none()
    );
    assert!(state.has_retained_close_records());
    assert!(state.invariant_holds());
}

#[test]
fn full_close_reconciliation_admission_keeps_the_initial_intent_and_all_allocators() {
    let now = Instant::now();
    let clock = FakeClock::new(now);
    let mut state = closed_state(now);
    let mailbox = bootstrapped_mailbox(1, 900);
    for _ in 0..COMMAND_CAPACITY {
        mailbox
            .try_enqueue(QueuedCommand::Input(Tagged::new(
                pane_ref(&key(1, 1)),
                ControlInput::new(vec![b'a'; MAXIMUM_CONTROL_INPUT_BYTES]).unwrap(),
            )))
            .unwrap();
    }
    let before = mailbox.lock().identity_cursor;
    assert!(matches!(
        mailbox.try_enqueue_close_reconciliation_using(
            &mut state,
            now + Duration::from_secs(50),
            &clock
        ),
        Err(CloseAdmissionError::Full)
    ));
    assert_eq!(mailbox.lock().identity_cursor, before);
    assert!(mailbox.lock().close_reconciliation.is_none());
    let lease = mailbox.try_recv().unwrap().unwrap();
    mailbox.finish_lease(lease).unwrap();
    let marker = mailbox
        .try_enqueue_close_reconciliation_using(&mut state, now + Duration::from_secs(50), &clock)
        .unwrap();
    assert_eq!(marker.snapshot_request_id().as_str(), id(901));
    assert!(matches!(
        mailbox.try_enqueue_close_reconciliation_using(
            &mut state,
            now + Duration::from_secs(50),
            &clock
        ),
        Err(CloseAdmissionError::Busy)
    ));
}

#[test]
fn timely_snapshot_placement_is_untimed_but_a_stale_retry_reply_cannot_enter_the_new_ui_transaction()
 {
    let now = Instant::now();
    let clock = FakeClock::new(now);
    let mut state = closed_state(now);
    let mailbox = bootstrapped_mailbox(1, 900);
    let first = mailbox
        .try_enqueue_close_reconciliation_using(&mut state, now + Duration::from_secs(50), &clock)
        .unwrap();
    let lease = mailbox.try_recv().unwrap().unwrap();
    assert_eq!(
        mailbox.mark_close_reconciliation_in_flight(&first).unwrap(),
        MutationResult::Applied
    );
    let wait = mailbox
        .publish_event_before_using(
            WorkerEvent::CloseReconciliationSnapshot {
                marker: first.clone(),
                outcome: WorkbenchSnapshotOutcome::Denied {},
            },
            EventDeadline::PlacementOnly(first.deadline()),
            &clock,
        )
        .unwrap();
    clock.set(now + Duration::from_secs(60));
    let _taken = mailbox.take_event_before_using(&clock).unwrap().unwrap();
    mailbox
        .deposit_ui_reply_before_using(&wait, &clock, |_| {
            state
                .close_reconciliation_failed(&first)
                .map_err(|_| CarrierFailure::Protocol)?;
            Ok(UiReply::CloseReconciliation(Box::new(
                CloseReconciliationSettlement::new(
                    first.clone(),
                    RefreshDecision::Continue,
                    state.desired().cloned(),
                ),
            )))
        })
        .unwrap();
    mailbox.await_ui_reply(&wait).unwrap().unwrap();
    mailbox.finish_lease(lease).unwrap();
    state.acknowledge_close_lease(mailbox.take_close_acknowledgement().unwrap());
    state.request_live_close_reconciliation();
    let retry = mailbox
        .try_enqueue_close_reconciliation_using(&mut state, now + Duration::from_secs(110), &clock)
        .unwrap();
    assert_eq!(retry.snapshot_request_id().as_str(), id(902));
    let retry_lease = mailbox.try_recv().unwrap().unwrap();
    assert_eq!(
        mailbox.mark_close_reconciliation_in_flight(&retry).unwrap(),
        MutationResult::Applied
    );
    let newer = mailbox
        .publish_event_before_using(
            WorkerEvent::CloseReconciliationSnapshot {
                marker: retry.clone(),
                outcome: WorkbenchSnapshotOutcome::Denied {},
            },
            EventDeadline::PlacementOnly(retry.deadline()),
            &clock,
        )
        .unwrap();
    let _taken = mailbox.take_event_before_using(&clock).unwrap().unwrap();
    let identity_cursor = mailbox.lock().identity_cursor;
    let cache = state.cache().clone();
    let desired = state.desired().cloned();
    let mut called = false;
    assert_eq!(
        mailbox
            .deposit_ui_reply_before_using(&wait, &clock, |_| {
                called = true;
                Ok(UiReply::CloseReconciliation(Box::new(
                    CloseReconciliationSettlement::new(
                        first.clone(),
                        RefreshDecision::Continue,
                        state.desired().cloned(),
                    ),
                )))
            })
            .unwrap(),
        MutationResult::Stale
    );
    assert!(!called);
    assert_eq!(mailbox.lock().identity_cursor, identity_cursor);
    assert_eq!(state.cache(), &cache);
    assert_eq!(state.desired(), desired.as_ref());
    assert_eq!(
        state.close_reconciliation_origin(first.origin().key()),
        Some(first.origin())
    );
    assert!(
        mailbox
            .lock()
            .exact_close_reconciliation(&newer.correlation)
    );
    assert_eq!(mailbox.lock().event_wait_key.as_ref(), Some(&newer));
    assert_eq!(
        mailbox.lock().active_lease.unwrap().serial,
        retry_lease.serial
    );
    assert!(mailbox.take_close_acknowledgement().is_none());
    mailbox
        .deposit_ui_reply_before_using(&newer, &clock, |_| {
            state
                .close_reconciliation_failed(&retry)
                .map_err(|_| CarrierFailure::Protocol)?;
            Ok(UiReply::CloseReconciliation(Box::new(
                CloseReconciliationSettlement::new(
                    retry.clone(),
                    RefreshDecision::Continue,
                    state.desired().cloned(),
                ),
            )))
        })
        .unwrap();
    assert!(
        mailbox.take_close_acknowledgement().is_none(),
        "deposit does not release the worker lease"
    );
    mailbox.await_ui_reply(&newer).unwrap().unwrap();
    assert!(
        mailbox.take_close_acknowledgement().is_none(),
        "taking a reply still leaves the worker lease live"
    );
    mailbox.finish_lease(retry_lease).unwrap();
    state.acknowledge_close_lease(mailbox.take_close_acknowledgement().unwrap());
    assert_eq!(mailbox.is_closed(), None);
}

#[test]
fn fabricated_current_serial_with_a_stale_reconciliation_correlation_closes_as_protocol() {
    let now = Instant::now();
    let clock = FakeClock::new(now);
    let mut state = closed_state(now);
    let mailbox = bootstrapped_mailbox(1, 900);
    let first = mailbox
        .try_enqueue_close_reconciliation_using(&mut state, now + Duration::from_secs(50), &clock)
        .unwrap();
    let lease = mailbox.try_recv().unwrap().unwrap();
    assert_eq!(
        mailbox.mark_close_reconciliation_in_flight(&first).unwrap(),
        MutationResult::Applied
    );
    let wait = mailbox
        .publish_event_before_using(
            WorkerEvent::CloseReconciliationSnapshot {
                marker: first.clone(),
                outcome: WorkbenchSnapshotOutcome::Denied {},
            },
            EventDeadline::PlacementOnly(first.deadline()),
            &clock,
        )
        .unwrap();
    clock.set(now + Duration::from_secs(60));
    let _taken = mailbox.take_event_before_using(&clock).unwrap().unwrap();
    mailbox
        .deposit_ui_reply_before_using(&wait, &clock, |_| {
            state
                .close_reconciliation_failed(&first)
                .map_err(|_| CarrierFailure::Protocol)?;
            Ok(UiReply::CloseReconciliation(Box::new(
                CloseReconciliationSettlement::new(
                    first.clone(),
                    RefreshDecision::Continue,
                    state.desired().cloned(),
                ),
            )))
        })
        .unwrap();
    mailbox.await_ui_reply(&wait).unwrap().unwrap();
    mailbox.finish_lease(lease).unwrap();
    state.acknowledge_close_lease(mailbox.take_close_acknowledgement().unwrap());
    state.request_live_close_reconciliation();
    let retry = mailbox
        .try_enqueue_close_reconciliation_using(&mut state, now + Duration::from_secs(110), &clock)
        .unwrap();
    let _retry_lease = mailbox.try_recv().unwrap().unwrap();
    assert_eq!(
        mailbox.mark_close_reconciliation_in_flight(&retry).unwrap(),
        MutationResult::Applied
    );
    let newer = mailbox
        .publish_event_before_using(
            WorkerEvent::CloseReconciliationSnapshot {
                marker: retry.clone(),
                outcome: WorkbenchSnapshotOutcome::Denied {},
            },
            EventDeadline::PlacementOnly(retry.deadline()),
            &clock,
        )
        .unwrap();
    let _taken = mailbox.take_event_before_using(&clock).unwrap().unwrap();
    let fabricated = UiWaitKey {
        correlation: wait.correlation,
        ..newer.clone()
    };
    let cache = state.cache().clone();
    let desired = state.desired().cloned();
    let identity_cursor = mailbox.lock().identity_cursor;
    let mut called = false;
    assert_eq!(
        mailbox
            .deposit_ui_reply_before_using(&fabricated, &clock, |_| {
                called = true;
                Ok(UiReply::CloseReconciliation(Box::new(
                    CloseReconciliationSettlement::new(
                        first.clone(),
                        RefreshDecision::Continue,
                        state.desired().cloned(),
                    ),
                )))
            })
            .unwrap_err()
            .closed_failure(),
        Some(CarrierFailure::Protocol)
    );
    assert!(!called);
    assert_eq!(mailbox.is_closed(), Some(CarrierFailure::Protocol));
    assert_eq!(mailbox.lock().identity_cursor, identity_cursor);
    assert_eq!(state.cache(), &cache);
    assert_eq!(state.desired(), desired.as_ref());
    assert_eq!(
        state.close_reconciliation_origin(retry.origin().key()),
        Some(retry.origin())
    );
    assert!(mailbox.take_close_acknowledgement().is_none());
    state.close_reconciliation_failed(&retry).unwrap();
}
