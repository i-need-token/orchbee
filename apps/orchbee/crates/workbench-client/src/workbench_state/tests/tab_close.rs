use super::{projected, ready, snapshot, workbench_request_id};
use crate::client_view::PanelDirection;
use crate::local_client::workbench::WorkbenchTabCloseAccepted;
use crate::tui_presentation::GeometrySettlement;
use crate::workbench_state::{
    AttachmentKey, CarrierFailure, ClientNotice, CloseIntent, CloseOperation, CloseResult,
    CloseSelection, CloseState, CloseStateError, CloseStatusKind, CloseTarget, PendingCloseResult,
    WorkbenchClientState,
};
use orchbee_workbench_protocol::{
    PanelNode, PanelOrientation, PanelSlot, Presentation, PresentationBinding, TerminalId,
    TerminalPresentationId, WorkbenchPanelId, WorkbenchPanelSlotId, WorkbenchRevision,
    WorkbenchSnapshotOutcome, WorkbenchSplitId, WorkbenchTabCloseAttemptGeneration,
    WorkbenchTabCloseBlockedReason, WorkbenchTabClosePublicationReason, WorkbenchTabCloseStatus,
    WorkbenchTabCloseUnknownPhase, WorkbenchTabCloseUnknownReason, WorkerId,
};
use std::time::{Duration, Instant};
mod reconciliation;
mod set;

fn selection(index: usize) -> CloseSelection {
    let cache = projected(2, 1);
    let tab = &cache.tabs()[index];
    CloseSelection::new(
        CloseTarget::new(
            cache.session_id().clone(),
            cache.terminal_context_id().unwrap().clone(),
            tab.tab_id().clone(),
        ),
        tab.tab_label().clone(),
        tab.binding().unwrap().clone(),
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
    assert!(state.status(selected.target()).is_none());
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
fn disposed_auto_admissions_enforce_31_32_33_without_same_attempt_renewal() {
    let selected = selection(0);
    let mut state = CloseState::default();
    state
        .retain_submission(selected.clone(), Instant::now())
        .unwrap();
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
    state
        .retain_submission(selected.clone(), Instant::now())
        .unwrap();
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
fn host_unknown_never_becomes_a_query_failure_class() {
    let selected = selection(0);
    let mut state = CloseState::default();
    state
        .retain_submission(selected.clone(), Instant::now())
        .unwrap();
    observe(
        &mut state,
        &selected,
        1,
        WorkbenchTabCloseStatus::OutcomeUnknown {
            phase: WorkbenchTabCloseUnknownPhase::Dispose,
            reason: WorkbenchTabCloseUnknownReason::TransportLost,
        },
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
    assert!(state.view().close_confirmation().is_some());
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
    assert!(state.install_projection_t(&key));
    assert!(state.record_setup_configuration_t(&key, super::ControllerMode::ReadOnly, None));
    assert!(state.stage_drawn_frame_t(&key, super::FrameIdentity::new(1)));
    assert!(state.mark_feedback_accepted_t(&key, super::FrameIdentity::new(1)));
    assert!(state.publish_ready_t(
        &key,
        super::FrameIdentity::new(1),
        super::ControllerMode::ReadOnly,
        None
    ));
    state.open_close_confirmation().unwrap();
    assert!(state.view().close_confirmation().is_some());
    assert_eq!(
        state.controller_mode(),
        Some(&super::ControllerMode::ReadOnly)
    );
    assert_eq!(state.desired(), Some(&key));
}

#[test]
fn split_close_uses_the_attached_leaf_binding_for_confirmation_and_submission() {
    let mut source = super::snapshot(1, 1, None);
    let original = source.terminal_contexts[0].tabs[0].root.clone();
    let peer_binding = PresentationBinding {
        terminal_id: TerminalId::parse(&super::id(2_012)).unwrap(),
        worker_id: WorkerId::parse(&super::id(2_011)).unwrap(),
    };
    source.terminal_contexts[0].tabs[0].root = PanelNode::Split {
        ratio: Default::default(),
        children: Box::new([
            PanelSlot {
                node: original,
                slot_id: WorkbenchPanelSlotId::parse(&super::id(2_001)).unwrap(),
            },
            PanelSlot {
                node: PanelNode::Panel {
                    right_click: Default::default(),
                    child_presentations: Vec::new(),
                    panel_id: WorkbenchPanelId::parse(&super::id(2_010)).unwrap(),
                    primary_presentation: Presentation {
                        custom_label: None,
                        binding: Some(peer_binding.clone()),
                        label: "peer".into(),
                        presentation_id: TerminalPresentationId::parse(&super::id(2_013)).unwrap(),
                        launch_failure: None,
                        agent_not_resumed: None,
                        agent_start_failure: None,
                        launch_program: None,
                    },
                },
                slot_id: WorkbenchPanelSlotId::parse(&super::id(2_014)).unwrap(),
            },
        ]),
        orientation: PanelOrientation::Horizontal,
        split_id: WorkbenchSplitId::parse(&super::id(2_020)).unwrap(),
    };
    let mut state = super::ready_with_cache(super::project_preferred_workspace(&source).unwrap());
    let fresh = super::attach_nonfirst_leaf(&mut state);
    assert!(state.stage_drawn_frame_t(&fresh, super::FrameIdentity::new(2)));
    assert!(state.mark_feedback_accepted_t(&fresh, super::FrameIdentity::new(2)));
    let pane = state.plan_panel_focus_local(PanelDirection::Right).unwrap();
    assert!(state.move_panel_focus_local(pane));
    assert_eq!(state.active().unwrap().panel_index(), 1);

    state.open_close_confirmation().unwrap();
    let token = state.view().close_confirmation().unwrap().token();
    state.acknowledge_close_draw(Some(token), None);
    let operation = state
        .plan_close_confirmation(
            super::workbench_request_id(2_100),
            Instant::now() + Duration::from_secs(50),
        )
        .unwrap();

    assert_eq!(
        operation.selection().binding().unwrap().terminal_id(),
        &peer_binding.terminal_id
    );
    assert_eq!(
        operation.selection().binding().unwrap().worker_id(),
        &peer_binding.worker_id
    );
    assert_ne!(
        operation.selection().binding(),
        state.tab(0).unwrap().binding()
    );
}

#[test]
fn canonical_receipt_and_closed_revision_are_retained_independently_of_reads() {
    let selected = selection(0);
    let mut state = CloseState::default();
    state
        .retain_submission(selected.clone(), Instant::now())
        .unwrap();
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

#[test]
fn a_progressing_close_leaves_the_strip_and_a_blocked_one_returns() {
    let selected = selection(0);
    let tab = selected.target().tab_id().clone();
    let mut state = CloseState::default();
    assert!(!state.tab_leaving(&tab));
    state
        .retain_submission(selected.clone(), Instant::now())
        .unwrap();
    assert!(state.tab_leaving(&tab));
    observe(
        &mut state,
        &selected,
        1,
        WorkbenchTabCloseStatus::Closing {
            phase: orchbee_workbench_protocol::WorkbenchTabClosePhase::WaitingGrace,
        },
    );
    assert!(state.tab_leaving(&tab));
    assert!(!state.tab_leaving(selection(1).target().tab_id()));
    observe(&mut state, &selected, 1, parked());
    assert!(!state.tab_leaving(&tab));
}

/// The Server retries and renews a close by itself, raising its attempt generation while the close
/// reads `closing`: the follow keeps reading and the Tab stays out. Only a falling generation is a
/// regression.
#[test]
fn the_follow_continues_across_a_generation_that_rises_by_itself() {
    let now = Instant::now();
    let mut state = ready(2);
    let selected = selection(0);
    let tab = selected.target().tab_id().clone();
    let closing = || WorkbenchTabCloseStatus::Closing {
        phase: orchbee_workbench_protocol::WorkbenchTabClosePhase::WaitingGrace,
    };
    state
        .close
        .retain_submission(selected.clone(), now)
        .unwrap();
    for generation in 1..=3 {
        observe(&mut state.close, &selected, generation, closing());
        assert!(!state.close.status(selected.target()).unwrap().stopped());
        assert!(state.close.tab_leaving(&tab));
        assert!(state.has_close_read_candidate(now));
    }
    assert_eq!(
        state.close.observe_available(
            selected.target(),
            workbench_request_id(701),
            WorkbenchTabCloseAttemptGeneration::new(2).unwrap(),
            closing(),
        ),
        Err(CloseStateError::GenerationRegression)
    );
    assert!(state.close.tab_leaving(&tab));
}

/// A close that finally fails, whatever its status, shows its Tab again and reports one sentence
/// with no reason.
#[test]
fn a_finally_failed_close_shows_the_tab_again_with_the_one_sentence() {
    use crate::localization::{CloseStatusText, Locale};
    let unknown = WorkbenchTabCloseStatus::OutcomeUnknown {
        phase: WorkbenchTabCloseUnknownPhase::Dispose,
        reason: WorkbenchTabCloseUnknownReason::TransportLost,
    };
    for failure in [parked(), disposed(true), unknown] {
        let mut state = ready(2);
        let selected = selection(0);
        let tab = selected.target().tab_id().clone();
        state
            .close
            .retain_submission(selected.clone(), Instant::now())
            .unwrap();
        assert!(state.close.tab_leaving(&tab));
        observe(&mut state.close, &selected, 1, failure);
        assert!(!state.close.tab_leaving(&tab));
        let report = state.view().announced_close_status().unwrap();
        assert!(report.stopped());
        let label = report.target_label();
        for (locale, sentence) in [
            (Locale::EnUs, format!("Couldn't close {label}.")),
            (Locale::ZhCn, format!("未能关闭 {label}。")),
        ] {
            let told = locale.native_close_status(CloseStatusText {
                label,
                kind: report.status(),
                stopped: report.stopped(),
            });
            assert_eq!(told, sentence);
        }
    }
}

#[test]
fn navigation_skips_a_leaving_tab_and_an_accepted_close_moves_the_selection() {
    use crate::client_view::ClientAction;
    use crate::workbench_state::SelectionPlan;
    let mut state = ready(3);
    let now = Instant::now();
    let operation = admit_close(&mut state, 821, now);
    assert_eq!(state.plan_selection_index(0), SelectionPlan::Busy);
    for (action, plan) in [
        (
            ClientAction::SelectNextTab,
            SelectionPlan::Switch { index: 1 },
        ),
        (
            ClientAction::SelectPreviousTab,
            SelectionPlan::Switch { index: 2 },
        ),
        (
            ClientAction::SelectTab(1),
            SelectionPlan::Switch { index: 1 },
        ),
        (ClientAction::SelectTab(3), SelectionPlan::OutOfRange(3)),
    ] {
        assert_eq!(state.plan_selection(action), plan);
    }
    // The selection waits for the Server to accept the close before it moves.
    assert_eq!(state.leaving_selection_successor(), None);
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
    assert_eq!(state.leaving_selection_successor(), Some(1));
    assert!(!state.view().last_tab_leaving());
}

fn admit_close(state: &mut WorkbenchClientState, identity: u128, now: Instant) -> CloseOperation {
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
                cache.session_id().clone(),
                cache.terminal_context_id().unwrap().clone(),
                super::tab_id(20_000 + index as u128),
            ),
            tab.tab_label().clone(),
            tab.binding().unwrap().clone(),
            None,
            cache.revision(),
        )
    };
    let mut state = CloseState::default();
    for index in 0..255 {
        state
            .retain_submission(selection_at(index), Instant::now())
            .unwrap();
    }
    state.open_confirmation(selection_at(255)).unwrap();
    let token = state.confirmation().unwrap().token();
    assert_eq!(
        state.open_confirmation(selection_at(256)),
        Err(CloseStateError::Busy)
    );
    assert_eq!(state.confirmation().unwrap().token(), token);
    state.cancel_confirmation();
    state
        .retain_submission(selection_at(255), Instant::now())
        .unwrap();
    assert_eq!(
        state.retain_submission(selection_at(256), Instant::now()),
        Err(CloseStateError::Busy)
    );
    assert_eq!(
        state.open_confirmation(selection_at(256)),
        Err(CloseStateError::Busy)
    );
    assert!(state.confirmation().is_none());
    state
        .retain_submission(selection_at(0), Instant::now())
        .unwrap();
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
    assert!(state.close_suppresses_attachment(state.desired().unwrap().anchor()));
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

/// A real refresh mutation bumps the baseline revision the confirmation
/// froze; the settle pass cancels the armed capture and posts the bounded
/// notice instead of leaving it armed-but-stale.
#[test]
fn drifted_close_confirmation_settles_as_cancelled_with_notice() {
    let mut state = ready(2);
    state.open_close_confirmation().unwrap();
    let token = state.view().close_confirmation().unwrap().token();
    state.acknowledge_close_draw(Some(token), None);

    assert!(state.start_pending_refresh(2));
    let prepared = state.prepare_refresh(
        2,
        WorkbenchSnapshotOutcome::Available {
            snapshot: snapshot(2, 2, None),
        },
    );
    state.apply_prepared_refresh(prepared, std::time::Instant::now());

    assert!(state.settle_stale_close_confirmations());
    assert!(state.view().close_confirmation().is_none());
    assert_eq!(state.notice, Some(ClientNotice::ConfirmationCancelled));
    assert!(!state.settle_stale_close_confirmations());
    assert!(state.invariant_holds());
}

/// A new confirmation that cannot complete its first paint is refused with the geometry notice,
/// not mislabeled as an explicit cancellation.
#[test]
fn first_unpresentable_close_draw_is_refused() {
    let mut state = ready(2);
    state.open_close_confirmation().unwrap();
    assert_eq!(
        state.settle_close_confirmation_not_presented(),
        Some(GeometrySettlement::Refused)
    );
    assert!(state.view().close_confirmation().is_none());
    assert_eq!(state.notice, Some(ClientNotice::DialogRefused));
    assert_eq!(state.settle_close_confirmation_not_presented(), None);
    assert!(state.invariant_holds());
}

/// Once presented, geometry loss retains the same frozen confirmation while
/// revoking its exact-draw authority; a later complete paint resumes it.
#[test]
fn presented_close_confirmation_suspends_and_resumes() {
    let mut state = ready(2);
    state.open_close_confirmation().unwrap();
    let token = state.view().close_confirmation().unwrap().token();
    state.acknowledge_close_draw(Some(token), None);
    assert!(state.view().close_confirmation().unwrap().interactive());
    assert_eq!(
        state.settle_close_confirmation_not_presented(),
        Some(GeometrySettlement::Suspended)
    );
    let suspended = state.view().close_confirmation().unwrap();
    assert_eq!(suspended.token(), token);
    assert!(!suspended.interactive());
    assert_eq!(
        state.close.confirmed_intent(token),
        Err(CloseStateError::NotDisplayed)
    );
    state.acknowledge_close_draw(Some(token), None);
    assert!(state.view().close_confirmation().unwrap().interactive());
    assert!(state.invariant_holds());
}

/// Client retirement drops the armed confirmation outright.
#[test]
fn retirement_drops_the_armed_close_confirmation() {
    let mut state = ready(2);
    state.open_close_confirmation().unwrap();
    state.retire(CarrierFailure::Closed);
    assert!(state.view().close_confirmation().is_none());
    assert!(state.invariant_holds());
}

/// Close-carrier retirement drops the armed confirmation outright — no prompt
/// survives to keep swallowing keys after the carrier is gone.
#[test]
fn close_carrier_retirement_drops_the_armed_confirmation() {
    let mut state = ready(2);
    state.open_close_confirmation().unwrap();
    state.close_carrier_retired();
    assert!(state.view().close_confirmation().is_none());
    assert!(state.invariant_holds());
}
