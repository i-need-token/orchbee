use super::*;
use crate::client_view::ClientAction;
use orchbee_client_server_protocol::ControllerId;
use orchbee_workbench_protocol::{
    Navigation, PanelNode, Presentation, PresentationBinding, ProtocolVersion, Snapshot, Tab,
    TerminalContext, TerminalId, TerminalPresentationId, WorkbenchPanelId,
    WorkbenchTerminalFailureReason, WorkbenchTerminalGeometry, WorkbenchTerminalUnavailableReason,
    WorkerId,
};

mod create_projection;
mod generated;
mod membership;
mod move_tab;
mod rename_draft;
mod rename_text;
mod reorder;
mod reorder_history;
mod retirement_handoff;
mod switch_membership;
mod tab_close;
mod tab_rename;

fn state(tab_count: usize) -> NativeSessionState {
    NativeSessionState::starting(projected(tab_count, 1), 1, geometry(80))
        .unwrap()
        .0
}

fn ready(tab_count: usize) -> NativeSessionState {
    ready_with_cache(projected(tab_count, 1))
}

fn ready_with_cache(cache: NativeStandaloneTabs) -> NativeSessionState {
    let mut state = NativeSessionState::starting(cache, 1, geometry(80))
        .unwrap()
        .0;
    let key = state.desired().unwrap().clone();
    assert!(state.install_projection(&key));
    assert!(state.record_setup_configuration(&key, controlling(), Some(geometry(80))));
    assert!(state.stage_drawn_frame(&key, FrameIdentity::new(1)));
    assert!(state.mark_feedback_accepted(&key, FrameIdentity::new(1)));
    assert!(state.publish_ready(
        &key,
        FrameIdentity::new(1),
        controlling(),
        Some(geometry(80))
    ));
    state
}

fn retryable_state(base: Instant) -> NativeSessionState {
    let mut state = ready(2);
    let deadlines = OperationDeadlines::from_sample(base).unwrap();
    let target_key = AttachmentKey::new(2, state.tab(1).unwrap().tab_id().clone());
    let target = state
        .target_for_switch(1, target_key.clone(), deadlines)
        .unwrap();
    assert!(state.begin_switch(target));
    let target_key = install_switch_candidate(&mut state, &target_key);
    assert_eq!(
        state.settle_unattached_setup(&target_key, 1, &CreateTerminalProvenance::BindingOnly),
        Some(CleanupContinuation::Stable)
    );
    assert_eq!(
        state.phase_kind(),
        SessionPhaseKind::RetryableAttachUnavailable
    );
    state
}

fn install_switch_candidate(
    state: &mut NativeSessionState,
    marker: &AttachmentKey,
) -> AttachmentKey {
    let fresh = prepare_and_install_switch_candidate(state, marker).unwrap();
    assert_eq!(state.desired(), Some(&fresh));
    assert!(state.invariant_holds());
    fresh
}

fn prepare_and_install_switch_candidate(
    state: &mut NativeSessionState,
    marker: &AttachmentKey,
) -> Result<AttachmentKey, CarrierFailure> {
    let deadlines = state
        .switch_deadlines(marker)
        .ok_or(CarrierFailure::Protocol)?;
    let prepared = state.prepare_membership_candidate(
        state.cache().clone(),
        Some(marker.tab_id()),
        state.active().map(SelectedTab::index),
        None,
    )?;
    let fresh = AttachmentKey::new(
        marker
            .generation()
            .checked_add(1)
            .ok_or(CarrierFailure::IdentityExhausted)?,
        marker.tab_id().clone(),
    );
    if state.attached_key().is_some() {
        let cleanup = state.stage_switch_membership_cleanup(marker, prepared, deadlines)?;
        let prepared = state.prepare_cleaned_membership_candidate(&cleanup, cleanup.old())?;
        state
            .prepare_cleaned_membership_install(&cleanup, prepared, Some(fresh.clone()))?
            .commit();
    } else {
        state
            .prepare_switch_membership_install(marker, prepared, Some(fresh.clone()))?
            .commit();
    }
    Ok(fresh)
}

#[test]
fn bootstrap_uses_the_authoritative_first_tab_and_rejects_zero_generation() {
    let cache = projected(2, 1);
    assert!(NativeSessionState::starting(cache.clone(), 0, geometry(80)).is_none());

    let (state, target) = NativeSessionState::starting(cache, 1, geometry(80)).unwrap();
    assert_eq!(target.selected().key(), state.desired().unwrap());
    assert_eq!(target.selected().index(), 0);
    assert_eq!(target.site_id(), state.cache().site_id());
    assert_eq!(target.binding(), state.tab(0).unwrap().binding());
    assert_eq!(
        target.selected().key().tab_id(),
        state.tab(0).unwrap().tab_id()
    );
    assert_eq!(state.last_admitted_geometry(state.desired().unwrap()), None);
    assert_eq!(state.last_accepted_geometry(state.desired().unwrap()), None);
    assert!(state.invariant_holds());
}

#[test]
fn ready_requires_exact_projection_feedback_and_latest_controlling_geometry() {
    let mut state = state(1);
    let key = state.desired().unwrap().clone();
    assert!(!state.publish_ready(
        &key,
        FrameIdentity::new(1),
        controlling(),
        Some(geometry(80))
    ));
    assert!(state.install_projection(&key));
    assert!(state.record_setup_configuration(&key, controlling(), Some(geometry(80))));
    state.observe_geometry(geometry(81));
    assert!(state.stage_drawn_frame(&key, FrameIdentity::new(1)));
    assert!(state.mark_feedback_accepted(&key, FrameIdentity::new(1)));
    assert_eq!(
        state
            .settle_ready_candidate(
                &key,
                FrameIdentity::new(1),
                controlling(),
                Some(geometry(80))
            )
            .unwrap(),
        ReadyDecision::CatchUpResize(geometry(81))
    );
    assert_eq!(state.expected_frame_geometry(&key), Some(geometry(80)));
    assert!(state.record_setup_configuration(&key, controlling(), Some(geometry(81))));
    assert!(!state.publish_ready(
        &key,
        FrameIdentity::new(1),
        controlling(),
        Some(geometry(81))
    ));
    assert!(state.stage_drawn_frame(&key, FrameIdentity::new(2)));
    assert!(state.mark_feedback_accepted(&key, FrameIdentity::new(2)));
    assert!(state.publish_ready(
        &key,
        FrameIdentity::new(2),
        controlling(),
        Some(geometry(81))
    ));
    assert_eq!(state.phase_kind(), SessionPhaseKind::Ready);
    assert!(state.invariant_holds());
}

#[test]
fn accepted_ready_resize_invalidates_the_old_frame_proof() {
    let mut state = ready(1);
    let key = state.desired().unwrap().clone();
    state.observe_geometry(geometry(90));
    assert!(state.record_setup_configuration(&key, controlling(), Some(geometry(90))));
    assert!(!state.feedback_accepted());
    assert!(!state.publish_ready(
        &key,
        FrameIdentity::new(1),
        controlling(),
        Some(geometry(90))
    ));
    assert!(state.stage_drawn_frame(&key, FrameIdentity::new(2)));
    assert!(state.mark_feedback_accepted(&key, FrameIdentity::new(2)));
}

#[test]
fn historical_ready_admits_create_across_transient_frame_gaps() {
    let cases = [
        (false, controlling()),
        (true, controlling()),
        (false, ControllerMode::ReadOnly),
    ];
    for (case, (stage_new_frame, mode)) in cases.into_iter().enumerate() {
        let mut state = if mode == ControllerMode::ReadOnly {
            let mut state = state(1);
            let key = state.desired().unwrap().clone();
            assert!(state.install_projection(&key));
            assert!(state.record_setup_configuration(&key, mode.clone(), None));
            assert!(state.stage_drawn_frame(&key, FrameIdentity::new(1)));
            assert!(state.mark_feedback_accepted(&key, FrameIdentity::new(1)));
            assert!(state.publish_ready(&key, FrameIdentity::new(1), mode.clone(), None));
            state.provenance[0] = CreateTerminalProvenance::Started;
            state
        } else {
            ready(1)
        };
        let key = state.desired().unwrap().clone();
        let active = state.active().unwrap().clone();
        let provenance = state.provenance(0).unwrap().clone();
        if stage_new_frame {
            assert!(state.stage_drawn_frame(&key, FrameIdentity::new(2)));
        } else {
            assert!(state.invalidate_projection_frame(&key));
        }
        assert!(!state.feedback_accepted());

        let operation = create_operation(&state, 2, 20 + case as u128, "2");
        assert!(state.begin_create(operation));
        assert_eq!(state.phase_kind(), SessionPhaseKind::Creating);
        assert_eq!(state.active(), Some(&active));
        assert_eq!(state.desired().unwrap(), &key);
        assert_eq!(state.attached_key(), Some(&key));
        assert_eq!(state.projection_key(), Some(&key));
        assert_eq!(state.controller_mode(), Some(&mode));
        assert_eq!(state.provenance(0), Some(&provenance));
        assert!(state.invariant_holds());
    }
}

#[test]
fn read_only_ready_ignores_geometry_for_frames_and_never_admits_resize() {
    let mut state = state(1);
    let key = state.desired().unwrap().clone();
    assert!(state.install_projection(&key));
    assert!(state.record_setup_configuration(&key, ControllerMode::ReadOnly, None));
    state.observe_geometry(geometry(111));
    assert_eq!(state.expected_frame_geometry(&key), None);
    assert!(state.stage_drawn_frame(&key, FrameIdentity::new(1)));
    assert!(state.mark_feedback_accepted(&key, FrameIdentity::new(1)));
    assert_eq!(
        state
            .settle_ready_candidate(&key, FrameIdentity::new(1), ControllerMode::ReadOnly, None)
            .unwrap(),
        ReadyDecision::Ready
    );
    assert!(!state.record_admitted_geometry(&key, geometry(111)));
    assert_eq!(state.controller_mode(), Some(&ControllerMode::ReadOnly));
    assert!(state.invariant_holds());
}

#[test]
fn commit_keeps_old_active_until_validated_cleanup_installs_the_committed_target() {
    let mut state = ready(1);
    let old = state.active().unwrap().key().clone();
    let operation = create_operation(&state, 2, 20, "2");
    let operation_key = operation.key().clone();
    assert!(state.begin_create(operation));
    assert_eq!(state.active().unwrap().key(), &old);
    assert_eq!(state.desired().unwrap(), &old);

    let candidate = snapshot(2, 2, Some("2"));
    let created = &candidate.terminal_contexts[0].tabs[1];
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = &created.root
    else {
        unreachable!()
    };
    assert!(state.retain_create_commit(
        &operation_key,
        WorkbenchTabCreateCommit::Pending {
            revision: WorkbenchRevision::new(2),
            terminal_context_id: context_id(2),
            tab_id: created.tab_id.clone(),
            panel_id: panel_id.clone(),
            presentation_id: primary_presentation.presentation_id.clone(),
        },
    ));
    assert_eq!(
        state.phase_kind(),
        SessionPhaseKind::CreateCommittedNeedDetach
    );
    assert_eq!(state.active().unwrap().key(), &old);
    assert_eq!(state.desired().unwrap(), &old);

    let committed_key = state.committed_target().unwrap().committed_key().clone();

    assert_eq!(
        state.settle_create_snapshot(
            &operation_key,
            WorkbenchSnapshotOutcome::Available {
                snapshot: candidate
            }
        ),
        CreateProjectionDecision::SettledWithoutAttachment
    );
    assert!(state.attached_key().is_none());
    assert!(state.projection_key().is_none());
    assert_ne!(state.desired().unwrap(), &old);
    assert_eq!(state.desired(), Some(&committed_key));
    assert_eq!(state.active().unwrap().key(), &committed_key);
    assert_eq!(
        state.phase_kind(),
        SessionPhaseKind::SelectedWithoutAttachment
    );
    assert_eq!(
        state.provenance(1),
        Some(&CreateTerminalProvenance::Pending)
    );
    assert!(state.invariant_holds());
}

#[test]
fn retired_committed_create_preserves_old_active_and_known_commit_for_diagnostics() {
    let (mut state, operation_key) = committed_create_state();
    let old_active = state.active().unwrap().clone();
    let committed = state.committed_target().unwrap().clone();
    let desired = state.desired().unwrap().clone();
    let switch_key = AttachmentKey::new(3, old_active.key().tab_id().clone());
    let switch_target = state
        .target_for_switch(
            old_active.index(),
            switch_key,
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
        )
        .unwrap();

    state.retire(CarrierFailure::Transport);

    assert_eq!(state.phase_kind(), SessionPhaseKind::Retired);
    assert_eq!(state.retired_failure(), Some(CarrierFailure::Transport));
    assert_eq!(
        state.view().retired_failure(),
        Some(CarrierFailure::Transport)
    );
    assert_eq!(state.active(), Some(&old_active));
    assert_eq!(state.committed_target(), Some(&committed));
    assert_eq!(state.desired().unwrap(), &desired);
    assert!(state.attached_key().is_none());
    assert!(state.projection_key().is_none());
    assert!(!state.feedback_accepted());
    assert!(state.controller_mode().is_none());
    assert_eq!(
        state.plan_selection(ClientAction::SelectNextTab),
        SelectionPlan::Busy
    );
    assert_eq!(
        state.plan_selection(ClientAction::SelectTab(0)),
        SelectionPlan::Busy
    );
    assert_eq!(
        state.plan_selection(ClientAction::SelectTab(u8::MAX)),
        SelectionPlan::Busy
    );
    assert!(!state.begin_switch(switch_target));
    let later_create = create_operation(&state, 4, 21, "3");
    assert!(!state.begin_create(later_create));
    assert!(!state.phase_allows_refresh());
    assert!(!state.start_pending_refresh(8));
    assert!(!state.record_admitted_geometry(old_active.key(), geometry(90)));
    assert!(!state.install_projection(old_active.key()));
    assert_eq!(
        state.settle_create_snapshot(&operation_key, WorkbenchSnapshotOutcome::Denied {}),
        CreateProjectionDecision::Retire
    );

    state.retire(CarrierFailure::OutcomeUnknown);
    assert_eq!(state.retired_failure(), Some(CarrierFailure::Transport));
    assert_eq!(state.active(), Some(&old_active));
    assert_eq!(state.committed_target(), Some(&committed));
    assert!(state.invariant_holds());
}

#[test]
fn retired_pending_create_keeps_commit_and_historical_selection_before_cleanup() {
    for projection_unavailable in [false, true] {
        let (mut state, operation_key) = committed_create_state();
        let old = state.active().unwrap().clone();

        if projection_unavailable {
            assert_eq!(
                state.settle_create_snapshot(
                    &operation_key,
                    WorkbenchSnapshotOutcome::Unavailable {
                        reason: orchbee_workbench_protocol::WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
                    },
                ),
                CreateProjectionDecision::ProjectionUnavailable
            );
        } else {
            let PreparedCreateSnapshot::Available {
                baseline,
                committed,
                projection,
            } = state.prepare_create_snapshot(
                &operation_key,
                WorkbenchSnapshotOutcome::Available {
                    snapshot: snapshot(2, 2, Some("2")),
                },
            )
            else {
                panic!("pending cleanup requires a validated candidate")
            };
            let candidate = state
                .prepare_create_membership_candidate(&baseline, &committed, projection)
                .unwrap();
            let cleanup = state
                .stage_create_membership_cleanup(
                    &operation_key,
                    candidate,
                    committed.operation().deadlines(),
                )
                .unwrap();
            assert_eq!(cleanup.old(), old.key());
            assert!(state.pending_membership.is_some());
        }
        let expected_phase = if projection_unavailable {
            SessionPhaseKind::ProjectionUnavailable
        } else {
            SessionPhaseKind::CreateCommittedNeedDetach
        };
        assert_eq!(state.phase_kind(), expected_phase);
        assert_eq!(state.active(), Some(&old));
        let committed = state.committed_target().unwrap().clone();
        let desired = state.desired().unwrap().clone();

        state.retire(CarrierFailure::Deadline);

        assert_eq!(state.retired_failure(), Some(CarrierFailure::Deadline));
        assert!(state.pending_membership.is_none());
        assert_eq!(state.active(), Some(&old));
        assert_eq!(state.committed_target(), Some(&committed));
        assert_eq!(state.desired().unwrap(), &desired);
        assert!(state.attached_key().is_none());
        assert!(state.projection_key().is_none());
        assert!(!state.phase_allows_refresh());
        assert!(!state.start_pending_refresh(9));
        assert_eq!(
            state.settle_create_snapshot(&operation_key, WorkbenchSnapshotOutcome::Denied {}),
            CreateProjectionDecision::Retire
        );
        assert!(state.invariant_holds());
    }
}

#[test]
fn retirement_clears_pending_refresh_and_render_feedback_but_keeps_cache_diagnostics() {
    let mut state = ready(2);
    let active = state.active().unwrap().clone();
    assert!(state.feedback_accepted());
    assert!(state.start_pending_refresh(7));
    let prepared_refresh = state.prepare_refresh(
        7,
        WorkbenchSnapshotOutcome::Available {
            snapshot: snapshot(3, 2, None),
        },
    );

    state.retire(CarrierFailure::Closed);

    assert_eq!(state.active(), Some(&active));
    assert_eq!(state.pending_refresh_generation(), None);
    assert!(!state.feedback_accepted());
    assert!(state.attached_key().is_none());
    assert!(state.projection_key().is_none());
    assert_eq!(
        state.apply_prepared_refresh(prepared_refresh),
        RefreshDecision::Continue
    );
    assert_eq!(state.phase_kind(), SessionPhaseKind::Retired);
    assert_eq!(state.active(), Some(&active));
    assert!(state.invariant_holds());

    state.pending_refresh_generation = Some(8);
    assert!(!state.invariant_holds());
}

#[test]
fn precommit_reentry_uses_a_distinct_operation_and_preserves_notice_until_client_action() {
    let mut state = ready(1);
    let first = create_operation(&state, 2, 20, "2");
    let first_key = first.key().clone();
    assert!(state.begin_create(first));
    state.observe_geometry(geometry(90));
    assert!(state.restore_precommit(&first_key, Some(geometry(80))));
    assert_eq!(state.phase_kind(), SessionPhaseKind::Ready);
    assert_eq!(state.notice(), Some(SessionNotice::CreateNotCreated));
    assert_eq!(state.latest_geometry(), geometry(90));

    let second = create_operation(&state, 3, 21, "2");
    assert_ne!(first_key, *second.key());
    assert!(state.begin_create(second));
    assert_eq!(state.phase_kind(), SessionPhaseKind::Creating);
    assert_eq!(state.notice(), Some(SessionNotice::CreateNotCreated));
    state.clear_notice();
    assert_eq!(state.notice(), None);
    assert!(state.invariant_holds());
}

#[test]
fn committed_create_then_second_human_create_preserves_provenance_and_fresh_cursors() {
    let mut state = ready(1);
    let first = create_operation(&state, 2, 30, "2");
    let first_key = first.key().clone();
    assert!(state.begin_create(first));
    let committed_snapshot = snapshot(2, 2, Some("2"));
    let created = &committed_snapshot.terminal_contexts[0].tabs[1];
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = &created.root
    else {
        unreachable!()
    };
    let committed_tab_id = created.tab_id.clone();
    assert!(state.retain_create_commit(
        &first_key,
        WorkbenchTabCreateCommit::Pending {
            revision: WorkbenchRevision::new(2),
            terminal_context_id: context_id(2),
            tab_id: committed_tab_id.clone(),
            panel_id: panel_id.clone(),
            presentation_id: primary_presentation.presentation_id.clone(),
        }
    ));

    assert_eq!(
        state.settle_create_snapshot(
            &first_key,
            WorkbenchSnapshotOutcome::Available {
                snapshot: committed_snapshot
            }
        ),
        CreateProjectionDecision::SettledWithoutAttachment
    );
    assert_eq!(
        state.provenance(1),
        Some(&CreateTerminalProvenance::Pending)
    );

    let select_old_key = AttachmentKey::new(3, state.tab(0).unwrap().tab_id().clone());
    let select_old = state
        .target_for_switch(
            0,
            select_old_key.clone(),
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
        )
        .unwrap();
    assert!(state.begin_switch(select_old));
    let select_old_key = install_switch_candidate(&mut state, &select_old_key);
    assert!(state.install_projection(&select_old_key));
    assert!(state.record_setup_configuration(&select_old_key, controlling(), Some(geometry(80))));
    assert!(state.stage_drawn_frame(&select_old_key, FrameIdentity::new(4)));
    assert!(state.mark_feedback_accepted(&select_old_key, FrameIdentity::new(4)));
    assert!(state.publish_ready(
        &select_old_key,
        FrameIdentity::new(4),
        controlling(),
        Some(geometry(80))
    ));

    let second = create_operation(&state, select_old_key.generation() + 1, 31, "3");
    let second_key = second.key().clone();
    assert_ne!(second_key.request_id(), first_key.request_id());
    assert!(second_key.reserved_generation() > first_key.reserved_generation());
    assert!(state.begin_create(second));
    assert!(state.restore_precommit(&second_key, Some(geometry(80))));
    assert_eq!(state.phase_kind(), SessionPhaseKind::Ready);
    assert_eq!(
        state.provenance(1),
        Some(&CreateTerminalProvenance::Pending)
    );
    assert_eq!(state.tab(1).unwrap().tab_id(), &committed_tab_id);
    assert_ne!(state.desired().unwrap().tab_id(), &committed_tab_id);
    assert!(state.invariant_holds());
}

#[test]
fn created_terminal_outcomes_map_to_exact_selection_provenance() {
    let cases = [
        (
            WorkbenchTerminalOutcome::Started {
                geometry: workbench_geometry(80),
            },
            CreateTerminalProvenance::Started,
            true,
        ),
        (
            WorkbenchTerminalOutcome::Unavailable {
                reason: WorkbenchTerminalUnavailableReason::TerminalRuntimeUnavailable,
            },
            CreateTerminalProvenance::Unavailable(
                WorkbenchTerminalUnavailableReason::TerminalRuntimeUnavailable,
            ),
            false,
        ),
        (
            WorkbenchTerminalOutcome::Failed {
                reason: WorkbenchTerminalFailureReason::TerminalCreateFailed,
            },
            CreateTerminalProvenance::Failed(WorkbenchTerminalFailureReason::TerminalCreateFailed),
            false,
        ),
        (
            WorkbenchTerminalOutcome::OutcomeUnknown {},
            CreateTerminalProvenance::OutcomeUnknown,
            false,
        ),
    ];

    for (case, (terminal_outcome, provenance, attachable)) in cases.into_iter().enumerate() {
        let mut state = ready(1);
        let operation = create_operation(&state, 2, 100 + case as u128, "2");
        let operation_key = operation.key().clone();
        assert!(state.begin_create(operation));
        let candidate = snapshot(2, 2, Some("2"));
        let created = &candidate.terminal_contexts[0].tabs[1];
        let PanelNode::Panel {
            panel_id,
            primary_presentation,
            ..
        } = &created.root
        else {
            unreachable!()
        };
        assert!(state.retain_create_commit(
            &operation_key,
            WorkbenchTabCreateCommit::Created {
                revision: WorkbenchRevision::new(2),
                terminal_context_id: context_id(2),
                tab_id: created.tab_id.clone(),
                panel_id: panel_id.clone(),
                presentation_id: primary_presentation.presentation_id.clone(),
                terminal_outcome,
            },
        ));

        let decision = state.settle_create_snapshot(
            &operation_key,
            WorkbenchSnapshotOutcome::Available {
                snapshot: candidate,
            },
        );
        assert_eq!(state.provenance(1), Some(&provenance));
        if attachable {
            let CreateProjectionDecision::Attach(target) = decision else {
                panic!("Started must select an attachable target")
            };
            assert_eq!(target.provenance(), &provenance);
            assert_eq!(state.phase_kind(), SessionPhaseKind::Switching);
        } else {
            assert_eq!(decision, CreateProjectionDecision::SettledWithoutAttachment);
            assert_eq!(
                state.phase_kind(),
                SessionPhaseKind::SelectedWithoutAttachment
            );
        }
        assert!(state.invariant_holds());
    }
}

#[test]
fn append_only_refresh_preserves_a_nonfirst_active_id_and_known_provenance() {
    let mut baseline = snapshot(2, 1, None);
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &mut baseline.terminal_contexts[0].tabs[1].root
    else {
        unreachable!()
    };
    primary_presentation.label = "presentation sentinel".into();
    let mut state = ready_with_cache(project_native_standalone_tabs(&baseline).unwrap());
    let deadlines = OperationDeadlines::from_sample(Instant::now()).unwrap();
    let key = AttachmentKey::new(2, state.tab(1).unwrap().tab_id().clone());
    let target = state.target_for_switch(1, key.clone(), deadlines).unwrap();
    assert!(state.begin_switch(target));
    let key = install_switch_candidate(&mut state, &key);
    assert!(state.install_projection(&key));
    assert!(state.record_setup_configuration(&key, controlling(), Some(geometry(80))));
    assert!(state.stage_drawn_frame(&key, FrameIdentity::new(2)));
    assert!(state.mark_feedback_accepted(&key, FrameIdentity::new(2)));
    assert!(state.publish_ready(
        &key,
        FrameIdentity::new(2),
        controlling(),
        Some(geometry(80))
    ));
    state.provenance[1] = CreateTerminalProvenance::Started;
    let active = state.active().unwrap().clone();
    let attached = state.attached_key().cloned();
    let projection = state.projection_key().cloned();
    let mode = state.controller_mode().cloned();
    let mut refreshed = snapshot(3, 2, None);
    refreshed.terminal_contexts[0].tabs[1].label = "renamed tab".into();
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &mut refreshed.terminal_contexts[0].tabs[1].root
    else {
        unreachable!()
    };
    primary_presentation.label = "presentation sentinel".into();
    assert!(state.start_pending_refresh(1));
    assert_eq!(
        finish_refresh(
            &mut state,
            1,
            WorkbenchSnapshotOutcome::Available {
                snapshot: refreshed,
            },
            None,
        ),
        RefreshDecision::Continue
    );
    assert_eq!(state.active().unwrap().index(), 1);
    assert_eq!(state.active(), Some(&active));
    assert_eq!(state.attached_key(), attached.as_ref());
    assert_eq!(state.projection_key(), projection.as_ref());
    assert_eq!(state.controller_mode(), mode.as_ref());
    assert!(state.feedback_accepted());
    assert_eq!(state.view().tab(1).unwrap().tab_label(), "renamed tab");
    assert_eq!(
        state.view().tab(1).unwrap().presentation_label(),
        "presentation sentinel"
    );
    assert_eq!(
        state.provenance(1),
        Some(&CreateTerminalProvenance::Started)
    );
    assert_eq!(
        state.provenance(2),
        Some(&CreateTerminalProvenance::BindingOnly)
    );
    assert_eq!(
        state.refresh_status(),
        Some(NavigatorRefreshStatus::Success(WorkbenchRevision::new(2)))
    );
    assert!(state.invariant_holds());
}

#[test]
fn same_revision_label_change_retires_as_projection_divergence() {
    let mut state = ready(1);
    let diagnostic_active = state.active().unwrap().clone();
    let mut conflicting = snapshot(1, 1, None);
    conflicting.terminal_contexts[0].tabs[0].label = "same revision rename".into();
    assert!(state.start_pending_refresh(1));
    assert_eq!(
        finish_refresh(
            &mut state,
            1,
            WorkbenchSnapshotOutcome::Available {
                snapshot: conflicting,
            },
            None,
        ),
        RefreshDecision::Retire
    );
    assert_eq!(state.phase_kind(), SessionPhaseKind::Retired);
    assert_eq!(state.active(), Some(&diagnostic_active));
    assert_eq!(
        state.retired_failure(),
        Some(CarrierFailure::AuthorityDivergence)
    );
    assert!(state.invariant_holds());
}

#[test]
fn pending_create_accepts_later_revision_tab_rename_with_exact_presentation_anchor() {
    let mut state = ready(1);
    let operation = create_operation(&state, 2, 32, "2");
    let operation_key = operation.key().clone();
    assert!(state.begin_create(operation));
    let committed_snapshot = snapshot(2, 2, Some("2"));
    let created = &committed_snapshot.terminal_contexts[0].tabs[1];
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = &created.root
    else {
        unreachable!()
    };
    let committed_tab_id = created.tab_id.clone();
    assert!(state.retain_create_commit(
        &operation_key,
        WorkbenchTabCreateCommit::Pending {
            revision: WorkbenchRevision::new(2),
            terminal_context_id: context_id(2),
            tab_id: committed_tab_id.clone(),
            panel_id: panel_id.clone(),
            presentation_id: primary_presentation.presentation_id.clone(),
        },
    ));

    let mut later = snapshot(2, 3, Some("2"));
    later.terminal_contexts[0].tabs[1].label = "renamed after commit".into();
    assert_eq!(
        state.settle_create_snapshot(
            &operation_key,
            WorkbenchSnapshotOutcome::Available { snapshot: later },
        ),
        CreateProjectionDecision::SettledWithoutAttachment
    );
    assert_eq!(state.desired().unwrap().tab_id(), &committed_tab_id);
    assert_eq!(
        state.view().tab(1).unwrap().tab_label(),
        "renamed after commit"
    );
    assert_eq!(state.view().tab(1).unwrap().presentation_label(), "2");
    assert_eq!(
        state.provenance(1),
        Some(&CreateTerminalProvenance::Pending)
    );
    assert_eq!(
        state.phase_kind(),
        SessionPhaseKind::SelectedWithoutAttachment
    );
    assert!(state.invariant_holds());
}

#[test]
fn projection_unavailable_moved_started_refresh_needs_a_fresh_setup_budget() {
    let mut state = ready(1);
    let operation = create_operation(&state, 2, 30, "2");
    let original_deadlines = operation.deadlines();
    let key = operation.key().clone();
    assert!(state.begin_create(operation));
    let mut candidate = snapshot(2, 2, Some("2"));
    let created = &candidate.terminal_contexts[0].tabs[1];
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = &created.root
    else {
        unreachable!()
    };
    assert!(state.retain_create_commit(
        &key,
        WorkbenchTabCreateCommit::Created {
            revision: WorkbenchRevision::new(2),
            terminal_context_id: context_id(2),
            tab_id: created.tab_id.clone(),
            panel_id: panel_id.clone(),
            presentation_id: primary_presentation.presentation_id.clone(),
            terminal_outcome: WorkbenchTerminalOutcome::Started {
                geometry: workbench_geometry(80),
            },
        },
    ));
    candidate.revision = WorkbenchRevision::new(3);
    candidate.terminal_contexts[0].tabs.swap(0, 1);

    assert_eq!(
        state.settle_create_snapshot(&key, WorkbenchSnapshotOutcome::Denied {}),
        CreateProjectionDecision::ProjectionUnavailable
    );

    assert!(state.start_pending_refresh(1));
    let prepared = state.prepare_refresh(
        1,
        WorkbenchSnapshotOutcome::Available {
            snapshot: candidate.clone(),
        },
    );
    assert!(prepared.needs_recovery_deadlines());
    // Preparing recovery cannot install or detach before a fresh budget is supplied.
    assert_eq!(state.phase_kind(), SessionPhaseKind::ProjectionUnavailable);
    assert_eq!(state.pending_refresh_generation(), Some(1));
    assert_eq!(state.attached_key(), state.desired());
    assert_eq!(state.cache().revision(), WorkbenchRevision::new(1));
    let PreparedRefresh::CreateMembership {
        generation,
        operation,
        prepared,
    } = prepared
    else {
        panic!("Create recovery must use the common membership candidate")
    };
    let recovery =
        OperationDeadlines::from_sample(original_deadlines.total() + Duration::from_secs(70))
            .unwrap();
    assert!(recovery.forward_cutoff() > original_deadlines.total());
    let decision = create_projection::finish_create_refresh(
        &mut state, generation, &operation, prepared, recovery,
    );
    let RefreshDecision::AttachRecovered(target) = decision else {
        panic!("expected recovered Attach")
    };
    assert_eq!(target.deadlines(), recovery);
    assert_eq!(state.pending_refresh_generation(), None);
    assert_eq!(target.provenance(), &CreateTerminalProvenance::Started);
    assert_eq!(target.selected().index(), 0);
    assert_eq!(target.selected().key(), state.desired().unwrap());
    assert_eq!(target.binding(), state.tab(0).unwrap().binding());
    assert_eq!(state.phase_kind(), SessionPhaseKind::Switching);
    assert!(state.invariant_holds());
}

#[test]
fn cleanup_never_publishes_a_stale_generation_and_selects_by_provenance() {
    let mut state = ready(2);
    let deadlines = OperationDeadlines::from_sample(Instant::now()).unwrap();
    let old_key = AttachmentKey::new(2, state.tab(1).unwrap().tab_id().clone());
    let target = state
        .target_for_switch(1, old_key.clone(), deadlines)
        .unwrap();
    assert!(state.begin_switch(target));
    let old_key = install_switch_candidate(&mut state, &old_key);
    assert!(state.install_projection(&old_key));
    let newer_key = AttachmentKey::new(
        old_key.generation() + 1,
        state.tab(0).unwrap().tab_id().clone(),
    );
    let newer = state
        .target_for_switch(0, newer_key.clone(), deadlines)
        .unwrap();
    assert!(state.begin_switch(newer));
    assert_eq!(
        state.settle_cleanup(&old_key, 1, &CreateTerminalProvenance::BindingOnly),
        Some(CleanupContinuation::Superseded {
            desired: newer_key.clone(),
        })
    );
    assert_eq!(state.phase_kind(), SessionPhaseKind::Switching);
    assert_eq!(state.desired().unwrap(), &newer_key);

    let newer_key = install_switch_candidate(&mut state, &newer_key);
    assert_eq!(
        state.settle_unattached_setup(&newer_key, 0, &CreateTerminalProvenance::BindingOnly),
        Some(CleanupContinuation::Stable)
    );
    assert_eq!(
        state.phase_kind(),
        SessionPhaseKind::RetryableAttachUnavailable
    );
    assert!(state.projection_key().is_none());
    assert!(state.invariant_holds());
}

#[test]
fn exact_cleanup_settles_a_viewer_attached_on_wire_before_ui_install() {
    let deadlines = OperationDeadlines::from_sample(Instant::now()).unwrap();

    let mut current = ready(2);
    let current_key = AttachmentKey::new(2, current.tab(1).unwrap().tab_id().clone());
    let target = current
        .target_for_switch(1, current_key.clone(), deadlines)
        .unwrap();
    assert!(current.begin_switch(target));
    let current_key = install_switch_candidate(&mut current, &current_key);
    assert!(current.attached_key().is_none());
    assert_eq!(
        current.settle_cleanup(&current_key, 1, &CreateTerminalProvenance::BindingOnly),
        Some(CleanupContinuation::Stable)
    );
    assert_eq!(
        current.phase_kind(),
        SessionPhaseKind::RetryableAttachUnavailable
    );
    assert!(current.invariant_holds());

    let mut superseded = ready(2);
    let old_key = AttachmentKey::new(2, superseded.tab(1).unwrap().tab_id().clone());
    let old_target = superseded
        .target_for_switch(1, old_key.clone(), deadlines)
        .unwrap();
    assert!(superseded.begin_switch(old_target));
    let old_key = install_switch_candidate(&mut superseded, &old_key);
    let newer_key = AttachmentKey::new(
        old_key.generation() + 1,
        superseded.tab(0).unwrap().tab_id().clone(),
    );
    let newer_target = superseded
        .target_for_switch(0, newer_key.clone(), deadlines)
        .unwrap();
    assert!(superseded.begin_switch(newer_target));
    assert_eq!(
        superseded.settle_cleanup(&old_key, 1, &CreateTerminalProvenance::BindingOnly),
        Some(CleanupContinuation::Superseded {
            desired: newer_key.clone(),
        })
    );
    assert_eq!(superseded.desired().unwrap(), &newer_key);
    assert_eq!(superseded.phase_kind(), SessionPhaseKind::Switching);
    assert!(superseded.invariant_holds());
}

#[test]
fn switch_candidate_cleanup_preserves_deadlines_and_superseded_detach_cannot_install() {
    let base = Instant::now();
    let deadlines = OperationDeadlines::from_sample(base).unwrap();
    for attachable in [true, false] {
        let mut state = ready(2);
        if !attachable {
            state.provenance[1] = CreateTerminalProvenance::Pending;
        }
        let marker = AttachmentKey::new(2, state.tab(1).unwrap().tab_id().clone());
        let target = state
            .target_for_switch(1, marker.clone(), deadlines)
            .unwrap();
        assert!(state.begin_switch(target));
        let fresh = install_switch_candidate(&mut state, &marker);
        assert_eq!(fresh.generation(), marker.generation() + 1);
        if attachable {
            assert_eq!(state.switch_deadlines(&fresh), Some(deadlines));
            assert!(state.install_projection(&fresh));
        } else {
            assert_eq!(
                state.phase_kind(),
                SessionPhaseKind::SelectedWithoutAttachment
            );
            assert!(!state.install_projection(&fresh));
        }
        assert!(state.invariant_holds());
    }

    let mut state = ready(2);
    let old = state.desired().unwrap().clone();
    let marker = AttachmentKey::new(2, state.tab(1).unwrap().tab_id().clone());
    assert!(
        state.begin_switch(
            state
                .target_for_switch(1, marker.clone(), deadlines)
                .unwrap()
        )
    );
    let cleanup = state.stage_switch_expiry_cleanup(&marker).unwrap();
    assert_eq!(cleanup.deadlines(), deadlines);
    let newest = AttachmentKey::new(3, state.tab(0).unwrap().tab_id().clone());
    assert!(
        state.begin_switch(
            state
                .target_for_switch(0, newest.clone(), deadlines)
                .unwrap()
        )
    );
    state.finish_switch_membership_admission(None, None, true);
    assert!(
        state
            .prepare_cleaned_membership_candidate(&cleanup, &old)
            .is_err()
    );
    state
        .settle_switch_membership_cleanup(&cleanup, true)
        .unwrap();
    assert_eq!(state.desired(), Some(&newest));
    assert_eq!(state.attached_key(), None);
    assert!(!state.install_projection(&newest));
    assert!(state.invariant_holds());
}

#[test]
fn switch_entry_expiry_without_a_viewer_requires_an_installed_candidate() {
    let base = Instant::now();
    let deadlines = OperationDeadlines::from_sample(base).unwrap();
    for at_cut in [false, true] {
        let mut state = retryable_state(base);
        let marker = AttachmentKey::new(
            state.desired().unwrap().generation() + 1,
            state.tab(0).unwrap().tab_id().clone(),
        );
        assert!(
            state.begin_switch(
                state
                    .target_for_switch(0, marker.clone(), deadlines)
                    .unwrap()
            )
        );
        assert_eq!(
            state.settle_switch_entry_expired(
                &marker,
                0,
                &CreateTerminalProvenance::BindingOnly,
                deadlines.forward_cutoff()
            ),
            None
        );
        let fresh = install_switch_candidate(&mut state, &marker);
        let at = if at_cut {
            deadlines.forward_cutoff()
        } else {
            deadlines.forward_cutoff() - Duration::from_nanos(1)
        };
        assert_eq!(
            state.settle_switch_entry_expired(
                &fresh,
                0,
                &CreateTerminalProvenance::BindingOnly,
                at
            ),
            at_cut.then_some(CleanupContinuation::Stable)
        );
        assert_eq!(
            state.phase_kind(),
            if at_cut {
                SessionPhaseKind::RetryableAttachUnavailable
            } else {
                SessionPhaseKind::Switching
            }
        );
        assert!(state.invariant_holds());
    }
    let mut state = retryable_state(base);
    state.provenance[1] = CreateTerminalProvenance::Pending;
    let marker = AttachmentKey::new(
        state.desired().unwrap().generation() + 1,
        state.tab(1).unwrap().tab_id().clone(),
    );
    assert!(
        state.begin_switch(
            state
                .target_for_switch(1, marker.clone(), deadlines)
                .unwrap()
        )
    );
    let fresh = install_switch_candidate(&mut state, &marker);
    assert_eq!(
        state.phase_kind(),
        SessionPhaseKind::SelectedWithoutAttachment
    );
    assert_eq!(
        state.settle_switch_entry_expired(
            &fresh,
            1,
            &CreateTerminalProvenance::Pending,
            deadlines.forward_cutoff()
        ),
        None
    );
    assert!(state.invariant_holds());
}
#[test]
fn exact_attached_and_feedback_events_acknowledge_supersession_without_new_proof() {
    let base = Instant::now();
    let deadlines = OperationDeadlines::from_sample(base).unwrap();

    let mut attached = ready(2);
    let stale_key = AttachmentKey::new(2, attached.tab(1).unwrap().tab_id().clone());
    let stale_target = attached
        .target_for_switch(1, stale_key.clone(), deadlines)
        .unwrap();
    assert!(attached.begin_switch(stale_target));
    let stale_key = install_switch_candidate(&mut attached, &stale_key);
    let desired = AttachmentKey::new(
        stale_key.generation() + 1,
        attached.tab(0).unwrap().tab_id().clone(),
    );
    let target = attached
        .target_for_switch(0, desired.clone(), deadlines)
        .unwrap();
    assert!(attached.begin_switch(target));
    assert_eq!(
        attached.settle_attached_event(&stale_key),
        Some(SetupEventDisposition::Superseded {
            desired: desired.clone(),
        })
    );
    assert!(attached.attached_key().is_none());
    assert!(attached.projection_key().is_none());
    assert_eq!(attached.desired().unwrap(), &desired);
    let wrong = AttachmentKey::new(99, tab_id(99));
    assert!(attached.settle_attached_event(&wrong).is_none());
    assert!(attached.invariant_holds());

    let mut configured = ready(2);
    let stale_key = AttachmentKey::new(2, configured.tab(1).unwrap().tab_id().clone());
    let stale_target = configured
        .target_for_switch(1, stale_key.clone(), deadlines)
        .unwrap();
    assert!(configured.begin_switch(stale_target));
    let stale_key = install_switch_candidate(&mut configured, &stale_key);
    assert_eq!(
        configured.settle_attached_event(&stale_key),
        Some(SetupEventDisposition::Applied)
    );
    let desired = AttachmentKey::new(
        stale_key.generation() + 1,
        configured.tab(0).unwrap().tab_id().clone(),
    );
    let target = configured
        .target_for_switch(0, desired.clone(), deadlines)
        .unwrap();
    assert!(configured.begin_switch(target));
    assert_eq!(
        configured.settle_setup_configuration_event(&stale_key, controlling(), Some(geometry(80)),),
        Some(SetupEventDisposition::Superseded {
            desired: desired.clone(),
        })
    );
    assert_eq!(
        configured.last_accepted_geometry(&stale_key),
        Some(geometry(80))
    );
    assert_eq!(configured.desired().unwrap(), &desired);
    assert!(configured.invariant_holds());

    let mut feedback = ready(2);
    let stale_key = AttachmentKey::new(2, feedback.tab(1).unwrap().tab_id().clone());
    let stale_target = feedback
        .target_for_switch(1, stale_key.clone(), deadlines)
        .unwrap();
    assert!(feedback.begin_switch(stale_target));
    let stale_key = install_switch_candidate(&mut feedback, &stale_key);
    assert_eq!(
        feedback.settle_attached_event(&stale_key),
        Some(SetupEventDisposition::Applied)
    );
    assert!(feedback.record_setup_configuration(&stale_key, controlling(), Some(geometry(80)),));
    let frame = FrameIdentity::new(7);
    assert!(feedback.stage_drawn_frame(&stale_key, frame));
    let desired = AttachmentKey::new(
        stale_key.generation() + 1,
        feedback.tab(0).unwrap().tab_id().clone(),
    );
    let target = feedback
        .target_for_switch(0, desired.clone(), deadlines)
        .unwrap();
    assert!(feedback.begin_switch(target));
    assert_eq!(
        feedback.settle_feedback_event(&stale_key, frame),
        Some(SetupEventDisposition::Superseded {
            desired: desired.clone(),
        })
    );
    assert!(!feedback.feedback_accepted());
    assert_eq!(feedback.desired().unwrap(), &desired);
    let wrong = AttachmentKey::new(99, tab_id(99));
    assert!(feedback.settle_feedback_event(&wrong, frame).is_none());
    assert!(feedback.invariant_holds());
}

#[test]
fn selection_wrap_bounds_and_same_target_retry_are_exact() {
    let mut state = ready(2);
    assert_eq!(
        state.plan_selection(ClientAction::SelectPreviousTab),
        SelectionPlan::Switch { index: 1 }
    );
    assert_eq!(
        state.plan_selection(ClientAction::SelectTab(3)),
        SelectionPlan::OutOfRange(3)
    );
    assert_eq!(
        state.plan_selection(ClientAction::SelectTab(1)),
        SelectionPlan::NoOp
    );

    let deadlines = OperationDeadlines::from_sample(Instant::now()).unwrap();
    let key = AttachmentKey::new(2, state.tab(1).unwrap().tab_id().clone());
    let target = state.target_for_switch(1, key.clone(), deadlines).unwrap();
    assert!(state.begin_switch(target));
    let key = install_switch_candidate(&mut state, &key);
    assert_eq!(
        state.settle_unattached_setup(&key, 1, &CreateTerminalProvenance::BindingOnly),
        Some(CleanupContinuation::Stable)
    );
    assert_eq!(
        state.plan_selection(ClientAction::SelectTab(2)),
        SelectionPlan::Switch { index: 1 }
    );
    assert_eq!(
        state.plan_selection(ClientAction::SelectNextTab),
        SelectionPlan::Switch { index: 0 }
    );

    let one = ready(1);
    assert_eq!(
        one.plan_selection(ClientAction::SelectNextTab),
        SelectionPlan::NoOp
    );
    assert_eq!(
        one.plan_selection(ClientAction::SelectPreviousTab),
        SelectionPlan::NoOp
    );
}

#[test]
fn stale_refresh_is_inert_and_divergence_retires() {
    let mut state = ready(1);
    let diagnostic_active = state.active().unwrap().clone();
    assert!(state.start_pending_refresh(4));
    assert_eq!(
        finish_refresh(&mut state, 3, WorkbenchSnapshotOutcome::Denied {}, None),
        RefreshDecision::Continue
    );
    assert_eq!(state.pending_refresh_generation(), Some(4));
    let mut divergent = snapshot(1, 2, None);
    divergent.site_id = site_id(99);
    assert_eq!(
        finish_refresh(
            &mut state,
            4,
            WorkbenchSnapshotOutcome::Available {
                snapshot: divergent
            },
            None,
        ),
        RefreshDecision::Retire
    );
    assert_eq!(state.phase_kind(), SessionPhaseKind::Retired);
    assert_eq!(state.active(), Some(&diagnostic_active));
    assert_eq!(
        state.retired_failure(),
        Some(CarrierFailure::AuthorityDivergence)
    );
    assert!(state.invariant_holds());
}

#[test]
fn view_borrows_canonical_duplicate_labels_and_marks_one_active_row() {
    let mut snapshot = snapshot(2, 1, None);
    snapshot.terminal_contexts[0].tabs[0].label = "same".into();
    snapshot.terminal_contexts[0].tabs[1].label = "same".into();
    for tab in &mut snapshot.terminal_contexts[0].tabs {
        let PanelNode::Panel {
            primary_presentation,
            ..
        } = &mut tab.root
        else {
            unreachable!()
        };
        primary_presentation.label = "same".into();
    }
    let (state, target) = NativeSessionState::starting(
        project_native_standalone_tabs(&snapshot).unwrap(),
        1,
        geometry(80),
    )
    .unwrap();
    let view = state.view();
    assert_eq!(view.phase(), SessionPhaseKind::Starting);
    assert_eq!(view.tab_count(), 2);
    assert_eq!(view.notice(), None);
    assert_eq!(view.refresh_status(), None);
    assert_eq!(view.tab(0).unwrap().tab_label(), "same");
    assert_eq!(view.tab(1).unwrap().presentation_label(), "same");
    assert_eq!(view.tab(0).unwrap().position(), 1);
    assert!(view.tab(0).unwrap().active());
    assert!(!view.tab(1).unwrap().active());
    assert_eq!(target.selected().key(), state.desired().unwrap());
    assert_eq!(target.site_id(), state.cache().site_id());
    assert_eq!(target.binding(), state.tab(0).unwrap().binding());
}

fn create_operation(
    state: &NativeSessionState,
    generation: u64,
    request: u128,
    label: &str,
) -> CreateOperation {
    CreateOperation::new(
        CreateOperationKey::new(workbench_request_id(request), generation),
        state.cache().site_id().clone(),
        state.cache().revision(),
        state.cache().terminal_context_id().clone(),
        WorkbenchLabel::parse(label).unwrap(),
        state.latest_geometry(),
        OperationDeadlines::from_sample(Instant::now()).unwrap(),
    )
}

fn committed_create_state() -> (NativeSessionState, CreateOperationKey) {
    let mut state = ready(1);
    let operation = create_operation(&state, 2, 20, "2");
    let operation_key = operation.key().clone();
    assert!(state.begin_create(operation));
    let candidate = snapshot(2, 2, Some("2"));
    let created = &candidate.terminal_contexts[0].tabs[1];
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = &created.root
    else {
        unreachable!()
    };
    assert!(state.retain_create_commit(
        &operation_key,
        WorkbenchTabCreateCommit::Pending {
            revision: WorkbenchRevision::new(2),
            terminal_context_id: context_id(2),
            tab_id: created.tab_id.clone(),
            panel_id: panel_id.clone(),
            presentation_id: primary_presentation.presentation_id.clone(),
        },
    ));
    (state, operation_key)
}

fn finish_refresh(
    state: &mut NativeSessionState,
    generation: u64,
    outcome: WorkbenchSnapshotOutcome,
    recovery_deadlines: Option<OperationDeadlines>,
) -> RefreshDecision {
    let prepared = state.prepare_refresh(generation, outcome);
    if let PreparedRefresh::CreateMembership {
        generation,
        operation,
        prepared,
    } = prepared
    {
        create_projection::finish_create_refresh(
            state,
            generation,
            &operation,
            prepared,
            recovery_deadlines.expect("Create recovery must sample its own budget"),
        )
    } else {
        state.apply_prepared_refresh(prepared)
    }
}

fn controlling() -> ControllerMode {
    ControllerMode::Controlling(ControllerId::parse(&id(700)).unwrap())
}

fn projected(tab_count: usize, revision: u64) -> NativeStandaloneTabs {
    project_native_standalone_tabs(&snapshot(tab_count, revision, None)).unwrap()
}

fn snapshot(tab_count: usize, revision: u64, created_label: Option<&str>) -> Snapshot {
    let mut tabs: Vec<_> = (0..tab_count).map(tab).collect();
    if let Some(label) = created_label {
        let created = tabs.last_mut().unwrap();
        created.label = label.into();
        let PanelNode::Panel {
            primary_presentation,
            ..
        } = &mut created.root
        else {
            unreachable!()
        };
        primary_presentation.label = label.into();
    }
    Snapshot {
        navigation: Navigation {
            projects: Vec::new(),
            standalone_terminal_context_id: context_id(2),
        },
        protocol_version: ProtocolVersion,
        revision: WorkbenchRevision::new(revision),
        site_id: site_id(1),
        terminal_contexts: vec![TerminalContext {
            groups: Vec::new(),
            tabs,
            terminal_context_id: context_id(2),
        }],
    }
}

fn tab(index: usize) -> Tab {
    let seed = 1_000 + u128::try_from(index).unwrap() * 10;
    let label = (index + 1).to_string();
    Tab {
        group_id: None,
        label: label.clone(),
        pinned: false,
        root: PanelNode::Panel {
            child_presentations: Vec::new(),
            panel_id: panel_id(seed + 1),
            primary_presentation: Presentation {
                binding: Some(PresentationBinding {
                    terminal_id: terminal_id(seed + 4),
                    worker_id: worker_id(seed + 3),
                }),
                label,
                presentation_id: presentation_id(seed + 2),
            },
        },
        tab_id: tab_id(seed),
    }
}

fn geometry(columns: u16) -> Geometry {
    Geometry::new(columns, 24, 8, 16).unwrap()
}

fn workbench_geometry(columns: u16) -> WorkbenchTerminalGeometry {
    WorkbenchTerminalGeometry::new(columns, 24, 8, 16).unwrap()
}

fn site_id(value: u128) -> SiteId {
    SiteId::parse(&id(value)).unwrap()
}

fn context_id(value: u128) -> WorkbenchTerminalContextId {
    WorkbenchTerminalContextId::parse(&id(value)).unwrap()
}

fn workbench_request_id(value: u128) -> WorkbenchRequestId {
    WorkbenchRequestId::parse(&id(value)).unwrap()
}

fn tab_id(value: u128) -> WorkbenchTabId {
    WorkbenchTabId::parse(&id(value)).unwrap()
}

fn panel_id(value: u128) -> WorkbenchPanelId {
    WorkbenchPanelId::parse(&id(value)).unwrap()
}

fn presentation_id(value: u128) -> TerminalPresentationId {
    TerminalPresentationId::parse(&id(value)).unwrap()
}

fn worker_id(value: u128) -> WorkerId {
    WorkerId::parse(&id(value)).unwrap()
}

fn terminal_id(value: u128) -> TerminalId {
    TerminalId::parse(&id(value)).unwrap()
}

fn id(value: u128) -> String {
    format!("{value:032x}")
}
