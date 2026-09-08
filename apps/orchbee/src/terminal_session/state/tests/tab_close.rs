use super::{projected, ready, workbench_request_id};
use crate::local_client::workbench::WorkbenchTabCloseAccepted;
use crate::terminal_session::state::{
    AttachmentKey, CloseConfirmationKind, CloseIntent, CloseOperation, CloseResult, CloseSelection,
    CloseState, CloseStateError, CloseStatusKind, CloseTarget, NativeSessionState,
    PendingCloseResult,
};
use orchbee_workbench_protocol::{
    WorkbenchRevision, WorkbenchTabCloseAttemptGeneration, WorkbenchTabCloseBlockedReason,
    WorkbenchTabClosePublicationReason, WorkbenchTabCloseStatus, WorkbenchTabCloseUnknownPhase,
    WorkbenchTabCloseUnknownReason,
};
use std::time::{Duration, Instant};
mod reconciliation;

fn selection(index: usize) -> CloseSelection {
    let cache = projected(2, 1);
    let tab = &cache.tabs()[index];
    CloseSelection::new(
        CloseTarget::new(
            cache.site_id().clone(),
            cache.terminal_context_id().clone(),
            tab.tab_id().clone(),
        ),
        tab.tab_label().clone(),
        tab.binding().clone(),
        Some(AttachmentKey::new(1, tab.tab_id().clone())),
        cache.revision(),
    )
}

fn observe(
    state: &mut CloseState,
    selected: &CloseSelection,
    generation: u64,
    status: WorkbenchTabCloseStatus,
) {
    state
        .observe_available(
            selected.target(),
            workbench_request_id(701),
            WorkbenchTabCloseAttemptGeneration::new(generation).unwrap(),
            status,
        )
        .unwrap();
}

fn parked() -> WorkbenchTabCloseStatus {
    WorkbenchTabCloseStatus::CleanupBlocked {
        reason: WorkbenchTabCloseBlockedReason::HostRefused,
        resumable: true,
    }
}

fn disposed(resumable: bool) -> WorkbenchTabCloseStatus {
    WorkbenchTabCloseStatus::DisposedAwaitingPublication {
        reason: WorkbenchTabClosePublicationReason::Preparing,
        resumable,
    }
}

#[test]
fn unseen_cancelled_and_replaced_confirmation_tokens_cannot_authorize_close() {
    let selected = selection(0);
    let mut state = CloseState::default();
    state.open_confirmation(selected.clone()).unwrap();
    let first = state.confirmation().unwrap().token();
    assert_eq!(
        state.confirmed_intent(first),
        Err(CloseStateError::NotDisplayed)
    );
    state.acknowledge_draw(Some(first), None);
    assert!(matches!(
        state.confirmed_intent(first),
        Ok(CloseIntent::Close(_))
    ));
    state.cancel_confirmation();
    assert!(state.confirmation().is_none());
    assert!(state.latest_status().is_none());
    state.open_confirmation(selected).unwrap();
    let next = state.confirmation().unwrap().token();
    assert_ne!(first, next);
    state.acknowledge_draw(Some(first), None);
    assert_eq!(
        state.confirmed_intent(next),
        Err(CloseStateError::NotDisplayed)
    );
}

#[test]
fn changed_display_revokes_prompt_before_input_and_complete_draw_rearms_same_intent() {
    let mut state = CloseState::default();
    state.open_confirmation(selection(0)).unwrap();
    let token = state.confirmation().unwrap().token();
    state.acknowledge_draw(Some(token), None);
    assert!(state.confirmed_intent(token).is_ok());
    state.invalidate_display();
    assert_eq!(state.confirmation().unwrap().token(), token);
    assert_eq!(
        state.confirmed_intent(token),
        Err(CloseStateError::NotDisplayed)
    );
    state.acknowledge_draw(Some(token), None);
    assert!(state.confirmed_intent(token).is_ok());
    state.acknowledge_draw(None, None);
    assert_eq!(
        state.confirmed_intent(token),
        Err(CloseStateError::NotDisplayed)
    );
}

#[test]
fn original_carrier_resume_requires_displayed_status_and_both_current_modal_tokens() {
    let selected = selection(0);
    let mut state = CloseState::default();
    state.retain_submission(selected.clone()).unwrap();
    observe(&mut state, &selected, 1, parked());
    assert_eq!(
        state.open_confirmation(selected.clone()),
        Err(CloseStateError::NotDisplayed)
    );
    let status = state.status(selected.target()).unwrap().token();
    state.acknowledge_draw(None, Some(status));
    state.open_confirmation(selected.clone()).unwrap();
    let prompt = state.confirmation().unwrap();
    assert_eq!(prompt.kind(), CloseConfirmationKind::Resume);
    assert_eq!(prompt.resume_status().unwrap().token(), status);
    let token = prompt.token();
    state.acknowledge_draw(Some(token), None);
    assert_eq!(
        state.confirmed_intent(token),
        Err(CloseStateError::NotDisplayed)
    );
    state.acknowledge_draw(Some(token), Some(status));
    assert!(matches!(
        state.confirmed_intent(token),
        Ok(CloseIntent::Resume { .. })
    ));
    observe(&mut state, &selected, 2, parked());
    state.acknowledge_draw(Some(token), Some(status));
    assert_eq!(
        state.confirmed_intent(token),
        Err(CloseStateError::NotDisplayed)
    );
}

#[test]
fn another_targets_status_receipt_does_not_arm_resume() {
    let first = selection(0);
    let second = selection(1);
    let mut state = CloseState::default();
    for selected in [&first, &second] {
        state.retain_submission(selected.clone()).unwrap();
        observe(&mut state, selected, 1, parked());
    }
    let first_status = state.status(first.target()).unwrap().token();
    let second_status = state.status(second.target()).unwrap().token();
    state.acknowledge_draw(None, Some(first_status));
    state.open_confirmation(first).unwrap();
    let token = state.confirmation().unwrap().token();
    state.acknowledge_draw(Some(token), Some(second_status));
    assert_eq!(
        state.confirmed_intent(token),
        Err(CloseStateError::NotDisplayed)
    );
}

#[test]
fn disposed_auto_admissions_enforce_31_32_33_without_same_attempt_renewal() {
    let selected = selection(0);
    let mut state = CloseState::default();
    state.retain_submission(selected.clone()).unwrap();
    observe(&mut state, &selected, 1, disposed(false));
    for _ in 0..31 {
        state
            .charge_disposed_auto_admission(selected.target())
            .unwrap();
    }
    assert!(
        !state
            .status(selected.target())
            .unwrap()
            .observation_paused()
    );
    state
        .charge_disposed_auto_admission(selected.target())
        .unwrap();
    assert!(
        state
            .status(selected.target())
            .unwrap()
            .observation_paused()
    );
    assert_eq!(
        state.charge_disposed_auto_admission(selected.target()),
        Err(CloseStateError::Busy)
    );
    state.observation_failed(selected.target());
    state.invalidate_display();
    observe(&mut state, &selected, 1, disposed(true));
    observe(&mut state, &selected, 1, disposed(false));
    assert!(
        state
            .status(selected.target())
            .unwrap()
            .observation_paused()
    );
    observe(&mut state, &selected, 2, disposed(false));
    assert!(
        !state
            .status(selected.target())
            .unwrap()
            .observation_paused()
    );
    state
        .charge_disposed_auto_admission(selected.target())
        .unwrap();
}

#[test]
fn disposed_and_closed_facts_survive_failed_or_contradictory_observation() {
    let selected = selection(0);
    let mut state = CloseState::default();
    state.retain_submission(selected.clone()).unwrap();
    observe(&mut state, &selected, 1, disposed(false));
    let operation = workbench_request_id(701);
    let generation = WorkbenchTabCloseAttemptGeneration::new(1).unwrap();
    assert_eq!(
        state.observe_available(selected.target(), operation.clone(), generation, parked()),
        Err(CloseStateError::FactRegression)
    );
    observe(
        &mut state,
        &selected,
        1,
        WorkbenchTabCloseStatus::Closed {
            revision: WorkbenchRevision::new(2),
        },
    );
    state.observation_failed(selected.target());
    assert_eq!(
        state.status(selected.target()).unwrap().status(),
        CloseStatusKind::Closed
    );
    assert_eq!(
        state.observe_available(
            selected.target(),
            operation,
            generation,
            WorkbenchTabCloseStatus::Closed {
                revision: WorkbenchRevision::new(3)
            }
        ),
        Err(CloseStateError::RevisionMismatch)
    );
    assert_eq!(
        state.status(selected.target()).unwrap().status(),
        CloseStatusKind::Closed
    );
}

#[test]
fn host_unknown_never_becomes_resumable_or_a_query_failure_class() {
    let selected = selection(0);
    let mut state = CloseState::default();
    state.retain_submission(selected.clone()).unwrap();
    observe(
        &mut state,
        &selected,
        1,
        WorkbenchTabCloseStatus::OutcomeUnknown {
            phase: WorkbenchTabCloseUnknownPhase::Dispose,
            reason: WorkbenchTabCloseUnknownReason::TransportLost,
        },
    );
    let status = state.status(selected.target()).unwrap().token();
    state.acknowledge_draw(None, Some(status));
    assert_eq!(
        state.open_confirmation(selected.clone()),
        Err(CloseStateError::NotResumable)
    );
    state.observation_failed(selected.target());
    assert_eq!(
        state.status(selected.target()).unwrap().status(),
        CloseStatusKind::OutcomeUnknown
    );
    assert!(state.status(selected.target()).unwrap().unavailable());
}

#[test]
fn real_native_state_opens_confirmation_without_changing_selection() {
    let mut state = ready(2);
    let original = state.desired().unwrap().clone();
    state.open_close_confirmation().unwrap();
    assert_eq!(
        state.view().close_confirmation().unwrap().kind(),
        CloseConfirmationKind::Close
    );
    assert_eq!(state.desired(), Some(&original));
    state.cancel_close_confirmation();
    assert!(state.view().close_confirmation().is_none());
    assert_eq!(state.desired(), Some(&original));
    assert!(state.invariant_holds());
}

#[test]
fn readonly_attachment_can_request_close_without_controller_authority() {
    let mut state = super::state(2);
    let key = state.desired().unwrap().clone();
    assert!(state.install_projection(&key));
    assert!(state.record_setup_configuration(&key, super::ControllerMode::ReadOnly, None));
    assert!(state.stage_drawn_frame(&key, super::FrameIdentity::new(1)));
    assert!(state.mark_feedback_accepted(&key, super::FrameIdentity::new(1)));
    assert!(state.publish_ready(
        &key,
        super::FrameIdentity::new(1),
        super::ControllerMode::ReadOnly,
        None
    ));
    state.open_close_confirmation().unwrap();
    assert_eq!(
        state.view().close_confirmation().unwrap().kind(),
        CloseConfirmationKind::Close
    );
    assert_eq!(
        state.controller_mode(),
        Some(&super::ControllerMode::ReadOnly)
    );
    assert_eq!(state.desired(), Some(&key));
}

#[test]
fn canonical_receipt_and_closed_revision_are_retained_independently_of_reads() {
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
    let mut receipt = WorkbenchTabCloseAccepted {
        close_request_id: workbench_request_id(701),
        terminal_context_id: selected.target().terminal_context_id().clone(),
        tab_id: selected.target().tab_id().clone(),
        accepted_revision: WorkbenchRevision::new(1),
    };
    state
        .record_acceptance(selected.target(), &receipt)
        .unwrap();
    receipt.accepted_revision = WorkbenchRevision::new(2);
    assert_eq!(
        state.record_acceptance(selected.target(), &receipt),
        Err(CloseStateError::RevisionMismatch)
    );
    receipt.accepted_revision = WorkbenchRevision::new(1);
    receipt.close_request_id = workbench_request_id(702);
    assert_eq!(
        state.record_acceptance(selected.target(), &receipt),
        Err(CloseStateError::OperationMismatch)
    );
    assert_eq!(
        state.status(selected.target()).unwrap().status(),
        CloseStatusKind::Closed
    );
}

fn admit_close(state: &mut NativeSessionState, identity: u128, now: Instant) -> CloseOperation {
    state.open_close_confirmation().unwrap();
    let token = state.view().close_confirmation().unwrap().token();
    state.acknowledge_close_draw(Some(token), None);
    let operation = state
        .plan_close_confirmation(
            workbench_request_id(identity),
            now + Duration::from_secs(50),
        )
        .unwrap();
    state.begin_close_exchange(&operation, now).unwrap();
    operation
}

#[test]
fn retained_record_capacity_rejects_257_before_opening_or_replacing_an_intent() {
    let cache = projected(1, 1);
    let selection_at = |index: usize| {
        let tab = &cache.tabs()[0];
        CloseSelection::new(
            CloseTarget::new(
                cache.site_id().clone(),
                cache.terminal_context_id().clone(),
                super::tab_id(20_000 + index as u128),
            ),
            tab.tab_label().clone(),
            tab.binding().clone(),
            None,
            cache.revision(),
        )
    };
    let mut state = CloseState::default();
    for index in 0..255 {
        state.retain_submission(selection_at(index)).unwrap();
    }
    state.open_confirmation(selection_at(255)).unwrap();
    let token = state.confirmation().unwrap().token();
    assert_eq!(
        state.open_confirmation(selection_at(256)),
        Err(CloseStateError::Busy)
    );
    assert_eq!(state.confirmation().unwrap().token(), token);
    state.cancel_confirmation();
    state.retain_submission(selection_at(255)).unwrap();
    assert_eq!(
        state.retain_submission(selection_at(256)),
        Err(CloseStateError::Busy)
    );
    assert_eq!(
        state.open_confirmation(selection_at(256)),
        Err(CloseStateError::Busy)
    );
    assert!(state.confirmation().is_none());
    state.retain_submission(selection_at(0)).unwrap();
}

#[test]
fn known_operation_not_closing_is_authority_divergence_and_keeps_receipt() {
    let mut state = ready(2);
    let now = Instant::now();
    let operation = admit_close(&mut state, 801, now);
    let target = operation.key().target();
    let accepted = WorkbenchTabCloseAccepted {
        close_request_id: operation.key().request_id().clone(),
        terminal_context_id: target.terminal_context_id().clone(),
        tab_id: target.tab_id().clone(),
        accepted_revision: operation.selection().base_revision(),
    };
    state
        .settle_close_exchange(&operation, &CloseResult::Accepted(accepted))
        .unwrap();
    let read = state
        .plan_close_read(
            workbench_request_id(802),
            now + Duration::from_secs(50),
            now,
        )
        .unwrap();
    state.begin_close_exchange(&read, now).unwrap();
    assert_eq!(
        state.settle_close_exchange(
            &read,
            &CloseResult::Read(
                orchbee_workbench_protocol::WorkbenchTabCloseReadOutcome::NotClosing {}
            )
        ),
        Err(CloseStateError::FactRegression)
    );
    assert!(state.close_suppresses_attachment(state.desired().unwrap()));
}

#[test]
fn late_settled_refusal_cannot_reapply_to_new_queued_close_or_skip_its_settlement() {
    let mut state = ready(2);
    let now = Instant::now();
    let first = admit_close(&mut state, 811, now);
    state
        .settle_close_exchange(&first, &CloseResult::NotSubmitted)
        .unwrap();
    let second = admit_close(&mut state, 812, now);
    assert_ne!(first.key(), second.key());
    assert_eq!(
        state.finalize_close_worker(super::CloseWorkerReturn::Returned(Some(
            PendingCloseResult::new(first, CloseResult::NotSubmitted)
        ))),
        Ok(())
    );
    assert!(state.view().close_status().is_none());
    state.open_close_confirmation().unwrap();
}
