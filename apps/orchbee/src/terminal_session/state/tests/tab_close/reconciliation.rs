use super::*;
use orchbee_workbench_protocol::WorkbenchTabCloseReadOutcome;

fn known_closed(handoff: bool) -> (NativeSessionState, CloseOperation) {
    let mut state = ready(2);
    let now = Instant::now();
    let close = admit_close(&mut state, 901, now);
    state
        .settle_close_exchange(
            &close,
            &CloseResult::Accepted(WorkbenchTabCloseAccepted {
                close_request_id: workbench_request_id(900),
                terminal_context_id: close.key().target().terminal_context_id().clone(),
                tab_id: close.key().target().tab_id().clone(),
                accepted_revision: close.selection().base_revision(),
            }),
        )
        .unwrap();
    let read = state
        .plan_close_read(
            workbench_request_id(902),
            now + Duration::from_secs(50),
            now,
        )
        .unwrap();
    state.begin_close_exchange(&read, now).unwrap();
    let result = CloseResult::Read(WorkbenchTabCloseReadOutcome::Available {
        close_request_id: workbench_request_id(900),
        terminal_context_id: close.key().target().terminal_context_id().clone(),
        tab_id: close.key().target().tab_id().clone(),
        attempt_generation: WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
        status: WorkbenchTabCloseStatus::Closed {
            revision: WorkbenchRevision::new(2),
        },
    });
    if handoff {
        state
            .finalize_close_worker(crate::terminal_session::state::CloseWorkerReturn::Returned(
                Some(PendingCloseResult::new(read.clone(), result)),
            ))
            .unwrap();
    } else {
        state.settle_close_exchange(&read, &result).unwrap();
    }
    (state, read)
}

#[test]
fn reconciliation_preserves_first_closed_read_and_requires_manual_retry_after_admission() {
    let (mut state, read) = known_closed(false);
    let deadline = Instant::now() + Duration::from_secs(50);
    let first = state
        .plan_close_reconciliation(workbench_request_id(903), deadline)
        .unwrap();
    assert_eq!(first.origin().key(), read.key());
    assert_eq!(first.origin().operation(), &workbench_request_id(900));
    assert_eq!(first.origin().attempt_generation().value(), 1);
    assert_eq!(
        first.origin().expectation().minimum_revision(),
        WorkbenchRevision::new(2)
    );
    assert_ne!(
        first.snapshot_request_id(),
        first.origin().key().request_id()
    );
    assert_eq!(
        state
            .plan_close_reconciliation(workbench_request_id(903), deadline)
            .unwrap(),
        first,
        "unadmitted intent does not spend an attempt"
    );
    state.begin_close_reconciliation(&first).unwrap();
    assert_eq!(
        state.plan_close_reconciliation(workbench_request_id(904), deadline),
        Err(CloseStateError::Busy)
    );
    state.close_reconciliation_failed(&first).unwrap();
    assert_eq!(
        state.plan_close_reconciliation(workbench_request_id(904), deadline),
        Err(CloseStateError::Busy)
    );
    assert_eq!(
        state.close_reconciliation_origin(read.key()),
        Some(first.origin())
    );
    state.request_live_close_reconciliation();
    let retry = state
        .plan_close_reconciliation(workbench_request_id(904), deadline)
        .unwrap();
    assert_eq!(retry.origin(), first.origin());
    state.begin_close_reconciliation(&retry).unwrap();
    assert_eq!(
        state.close_reconciliation_failed(&first),
        Err(CloseStateError::OperationMismatch),
        "late Snapshot A cannot settle retry B with the same Closed origin"
    );
    state.close_reconciliation_failed(&retry).unwrap();
}

#[test]
fn closed_cleanup_proof_requires_exact_old_generation_binding_and_target() {
    let (state, read) = known_closed(false);
    let origin = state.close_reconciliation_origin(read.key()).unwrap();
    let original_key = read.selection().attachment().unwrap();
    let proof = state
        .definitive_closed_attachment(origin, original_key, read.selection().binding())
        .unwrap();
    assert_eq!(proof.attachment_key(), original_key);
    assert_eq!(proof.closed_expectation(), origin.expectation());
    assert_eq!(
        state
            .definitive_closed_attachment_for(original_key, read.selection().binding())
            .unwrap()
            .attachment_key(),
        original_key
    );
    let next_generation =
        AttachmentKey::new(original_key.generation() + 1, original_key.tab_id().clone());
    assert!(
        state
            .definitive_closed_attachment(origin, &next_generation, read.selection().binding())
            .is_none()
    );
    assert!(
        state
            .definitive_closed_attachment_for(&next_generation, read.selection().binding())
            .is_none()
    );
    let foreign = selection(1);
    assert!(
        state
            .definitive_closed_attachment(origin, foreign.attachment().unwrap(), foreign.binding())
            .is_none()
    );
    assert!(
        state
            .definitive_closed_attachment(origin, original_key, foreign.binding())
            .is_none()
    );
}

#[test]
#[cfg(target_os = "linux")]
fn join_retained_closed_keeps_its_first_origin_after_a_fresh_recovery_read() {
    let (mut state, first_read) = known_closed(true);
    let original = state
        .close_reconciliation_origin(first_read.key())
        .unwrap()
        .clone();
    assert!(state.finalize_retirement(
        crate::terminal_session::state::CarrierFailure::Transport,
        None
    ));
    assert!(state.begin_close_status_recovery());
    let now = Instant::now() + Duration::from_secs(1);
    let next = state
        .plan_close_read(
            workbench_request_id(911),
            now + Duration::from_secs(50),
            now,
        )
        .unwrap();
    state.begin_close_exchange(&next, now).unwrap();
    state
        .settle_close_exchange(
            &next,
            &CloseResult::Read(WorkbenchTabCloseReadOutcome::Available {
                close_request_id: original.operation().clone(),
                terminal_context_id: original.expectation().terminal_context_id().clone(),
                tab_id: original.expectation().tab_id().clone(),
                attempt_generation: original.attempt_generation(),
                status: WorkbenchTabCloseStatus::Closed {
                    revision: original.expectation().minimum_revision(),
                },
            }),
        )
        .unwrap();
    assert_eq!(
        state.close_reconciliation_origin(first_read.key()),
        Some(&original)
    );
    assert!(state.close_reconciliation_origin(next.key()).is_none());
    assert_eq!(
        state.plan_close_reconciliation(workbench_request_id(912), now + Duration::from_secs(50)),
        Err(CloseStateError::Busy),
        "status-only Closed cannot grant automatic Snapshot"
    );
    assert_eq!(
        state.request_close_membership_recovery(),
        Err(CloseStateError::NotDisplayed)
    );
    let token = state.view().close_status().unwrap().token();
    state.acknowledge_close_draw(None, Some(token));
    state.request_close_membership_recovery().unwrap();
    let first_marker = state
        .plan_close_reconciliation(workbench_request_id(912), now + Duration::from_secs(50))
        .unwrap();
    assert!(first_marker.membership_recovery());
    assert!(
        state.close_status_only(),
        "membership recovery still forbids ordinary session commands"
    );
    assert_eq!(state.open_close_confirmation(), Err(CloseStateError::Busy));
    assert!(!state.has_close_read_candidate(now + Duration::from_secs(1)));
    state.refuse_close_reconciliation_admission(first_marker.origin());
    assert_eq!(
        state.plan_close_reconciliation(workbench_request_id(913), now + Duration::from_secs(50)),
        Err(CloseStateError::Busy),
        "Full requires a new explicit gesture in the typed pending stage"
    );
    state.request_close_membership_recovery().unwrap();
    let retry = state
        .plan_close_reconciliation(workbench_request_id(913), now + Duration::from_secs(50))
        .unwrap();
    assert_eq!(retry.origin(), &original);
    state.begin_close_reconciliation(&retry).unwrap();
    state.close_reconciliation_failed(&retry).unwrap();
    assert!(state.close_status_only());
    assert_eq!(
        state.plan_close_reconciliation(workbench_request_id(914), now + Duration::from_secs(50)),
        Err(CloseStateError::Busy)
    );
}

#[test]
fn a_closed_operation_cannot_move_to_another_attempt() {
    let selected = selection(0);
    let mut state = CloseState::default();
    state.retain_submission(selected.clone()).unwrap();
    observe(
        &mut state,
        &selected,
        1,
        WorkbenchTabCloseStatus::Closed {
            revision: WorkbenchRevision::new(2),
        },
    );
    assert_eq!(
        state.observe_available(
            selected.target(),
            workbench_request_id(701),
            WorkbenchTabCloseAttemptGeneration::new(2).unwrap(),
            WorkbenchTabCloseStatus::Closed {
                revision: WorkbenchRevision::new(2)
            }
        ),
        Err(CloseStateError::FactRegression)
    );
}
