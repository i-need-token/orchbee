use super::*;
use orchbee_workbench_protocol::WorkbenchTabCloseReadOutcome;

#[test]
fn inactive_closed_removal_preserves_the_survivor_attachment_and_consumes_only_its_origin() {
    use crate::terminal_session::state::{
        CloseLeaseAcknowledgement, ControllerMode, FrameIdentity, MembershipInstallDecision,
        OperationDeadlines,
    };
    let (mut state, read) = known_closed(false);
    let now = Instant::now();
    let old = state.desired().unwrap().clone();
    let switch_marker = AttachmentKey::new(2, state.tab(1).unwrap().tab_id().clone());
    let current = AttachmentKey::new(3, state.tab(1).unwrap().tab_id().clone());
    let deadlines = OperationDeadlines::from_sample(now).unwrap();
    let target = state
        .target_for_switch(1, switch_marker.clone(), deadlines)
        .unwrap();
    assert!(state.begin_switch(target));
    let prepared = state
        .prepare_switch_candidate_k(
            &switch_marker,
            orchbee_workbench_protocol::WorkbenchSnapshotOutcome::Available {
                snapshot: super::super::snapshot(2, 2, Some("2")),
            },
        )
        .unwrap();
    let cleanup = state
        .stage_switch_membership_cleanup_k(&switch_marker, prepared, deadlines)
        .unwrap();
    let prepared = state
        .prepare_cleaned_membership_candidate(&cleanup, old.anchor())
        .unwrap();
    assert!(matches!(
        state
            .prepare_cleaned_membership_install(&cleanup, prepared, Some(current.clone()),)
            .unwrap()
            .commit(),
        MembershipInstallDecision::Attach(_)
    ));
    assert!(state.install_projection_t(&current));
    assert!(state.record_setup_configuration_t(&current, ControllerMode::ReadOnly, None));
    assert!(state.stage_drawn_frame_t(&current, FrameIdentity::new(2)));
    assert!(state.mark_feedback_accepted_t(&current, FrameIdentity::new(2)));
    assert!(state.publish_ready_t(
        &current,
        FrameIdentity::new(2),
        ControllerMode::ReadOnly,
        None
    ));
    let marker = state
        .plan_close_reconciliation(workbench_request_id(925), now + Duration::from_secs(50))
        .unwrap();
    state.begin_close_reconciliation(&marker).unwrap();
    let mut snapshot = super::super::snapshot(2, 3, None);
    snapshot.terminal_contexts[0].tabs.remove(0);
    let prepared = state
        .prepare_close_membership_candidate(
            &marker,
            orchbee_workbench_protocol::WorkbenchSnapshotOutcome::Available { snapshot },
        )
        .unwrap()
        .unwrap();
    assert!(prepared.preserves_selection());
    assert_eq!(
        state
            .prepare_close_membership_install(&marker, prepared, Some(current.clone()), None)
            .unwrap()
            .commit(),
        MembershipInstallDecision::Preserved
    );
    assert_eq!(state.desired().map(|pane| pane.anchor()), Some(&current));
    assert_eq!(
        state.attached_key().map(|pane| pane.anchor()),
        Some(&current)
    );
    assert_eq!(
        state.projection_key().map(|pane| pane.anchor()),
        Some(&current)
    );
    assert!(state.feedback_accepted());
    assert_eq!(state.cache().tabs().len(), 1);
    assert!(state.invariant_holds());
    assert!(state.close_reconciliation_origin(read.key()).is_none());
    assert!(state.has_retained_close_records());
    state.acknowledge_close_lease(CloseLeaseAcknowledgement::Exchange(read.key().clone()));
    state.acknowledge_close_lease(CloseLeaseAcknowledgement::Reconciliation(Box::new(marker)));
    assert!(!state.has_retained_close_records());
    assert_eq!(state.desired().map(|pane| pane.anchor()), Some(&current));
}

#[test]
fn close_membership_install_and_prune_require_exact_plan_commit_and_finished_snapshot_lease() {
    use crate::terminal_session::state::{CloseLeaseAcknowledgement, OperationDeadlines};
    let (mut state, read) = known_closed(false);
    let now = Instant::now();
    let first = state
        .plan_close_reconciliation(workbench_request_id(920), now + Duration::from_secs(50))
        .unwrap();
    state.begin_close_reconciliation(&first).unwrap();
    state.close_reconciliation_failed(&first).unwrap();
    state.request_live_close_reconciliation();
    let retry = state
        .plan_close_reconciliation(workbench_request_id(921), now + Duration::from_secs(50))
        .unwrap();
    state.begin_close_reconciliation(&retry).unwrap();
    let candidate = state
        .prepare_close_membership_candidate(
            &retry,
            orchbee_workbench_protocol::WorkbenchSnapshotOutcome::Available {
                snapshot: super::super::snapshot(0, 2, None),
            },
        )
        .unwrap()
        .unwrap();
    let before = state.cache().clone();
    let cleanup = state
        .stage_close_membership_cleanup(
            &retry,
            candidate,
            OperationDeadlines::from_sample(now).unwrap(),
        )
        .unwrap();
    assert!(state.validate_membership_gone(&cleanup, cleanup.closed_proof().unwrap()));
    let prepared = state
        .prepare_cleaned_membership_candidate(&cleanup, cleanup.old())
        .unwrap();
    drop(
        state
            .prepare_cleaned_membership_install(&cleanup, prepared, None)
            .unwrap(),
    );
    assert_eq!(state.cache(), &before);
    assert_eq!(
        state.close_reconciliation_origin(read.key()),
        Some(retry.origin())
    );
    let prepared = state
        .prepare_cleaned_membership_candidate(&cleanup, cleanup.old())
        .unwrap();
    state
        .prepare_cleaned_membership_install(&cleanup, prepared, None)
        .unwrap()
        .commit();
    assert!(state.invariant_holds());
    assert!(state.cache().tabs().is_empty());
    assert!(state.close_reconciliation_origin(read.key()).is_none());
    assert!(
        state.has_retained_close_records(),
        "UI commit alone cannot prune a worker-owned marker"
    );
    state.acknowledge_close_lease(CloseLeaseAcknowledgement::Exchange(read.key().clone()));
    state.acknowledge_close_lease(CloseLeaseAcknowledgement::Reconciliation(Box::new(first)));
    assert!(
        state.has_retained_close_records(),
        "old Snapshot retry acknowledgement cannot free the current record"
    );
    state.acknowledge_close_lease(CloseLeaseAcknowledgement::Reconciliation(Box::new(retry)));
    assert!(!state.has_retained_close_records());
}

fn known_closed(handoff: bool) -> (NativeSessionState, CloseOperation) {
    known_closed_from(ready(2), handoff)
}

fn known_closed_from(
    mut state: NativeSessionState,
    handoff: bool,
) -> (NativeSessionState, CloseOperation) {
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
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
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

#[test]
fn unbound_selected_panel_uses_exact_closed_tab_proof_to_retire_bound_sibling_viewers() {
    use crate::terminal_session::state::{MembershipInstallDecision, OperationDeadlines};
    let mut source = super::super::split_panel_baseline();
    source.terminal_contexts[0]
        .tabs
        .push(snapshot(2, 1, None).terminal_contexts[0].tabs[1].clone());
    let PanelNode::Split { children, .. } = &mut source.terminal_contexts[0].tabs[0].root else {
        unreachable!()
    };
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &mut children[1].node
    else {
        unreachable!()
    };
    primary_presentation.binding = None;
    let cache =
        crate::terminal_session::workbench_projection::project_native_standalone_tabs(&source)
            .unwrap();
    let mut state = super::super::ready_with_cache(cache);
    let old = state.desired().unwrap().anchor().clone();
    let empty_panel = state.tab(0).unwrap().panel(1).unwrap().panel_id().clone();
    assert!(state.move_panel_focus_local(state.plan_panel_focus_local_at(&empty_panel).unwrap()));
    let (mut state, read) = known_closed_from(state, false);
    assert!(read.selection().binding().is_none());
    assert!(state.has_attachments() && state.selected_is_unbound());
    let marker = state
        .plan_close_reconciliation(
            workbench_request_id(903),
            Instant::now() + Duration::from_secs(50),
        )
        .unwrap();
    state.begin_close_reconciliation(&marker).unwrap();
    source.revision = WorkbenchRevision::new(2);
    source.terminal_contexts[0].tabs.remove(0);
    let prepared = state
        .prepare_close_membership_candidate(
            &marker,
            WorkbenchSnapshotOutcome::Available { snapshot: source },
        )
        .unwrap()
        .unwrap();
    let deadlines = OperationDeadlines::from_sample(Instant::now()).unwrap();
    let cleanup = state
        .stage_close_membership_cleanup(&marker, prepared, deadlines)
        .unwrap();
    let proof = cleanup.closed_proof().unwrap();
    assert!(state.validate_membership_gone(&cleanup, proof));
    assert_eq!(proof.attachment_key(), &old);
    assert!(
        state
            .definitive_closed_attachment(
                marker.origin(),
                &AttachmentKey::new(old.generation() + 1, old.tab_id().clone()),
                None
            )
            .is_none()
    );
    let candidate = state
        .prepare_cleaned_membership_candidate(&cleanup, &old)
        .unwrap();
    let desired = AttachmentKey::new(2, candidate.selected_tab_id().unwrap().clone());
    assert!(matches!(
        state
            .prepare_cleaned_membership_install(&cleanup, candidate, Some(desired))
            .unwrap()
            .commit(),
        MembershipInstallDecision::Attach(_)
    ));
    assert!(state.invariant_holds());
    assert!(state.close_reconciliation_origin(read.key()).is_none());
}
