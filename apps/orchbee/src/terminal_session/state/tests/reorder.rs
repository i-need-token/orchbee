use super::*;

#[test]
fn refresh_remaps_every_historical_provenance_and_preserves_ready_identity() {
    let mut state = ready(6);
    let history = vec![
        CreateTerminalProvenance::Started,
        CreateTerminalProvenance::Pending,
        CreateTerminalProvenance::Unavailable(
            WorkbenchTerminalUnavailableReason::TerminalRuntimeUnavailable,
        ),
        CreateTerminalProvenance::Failed(WorkbenchTerminalFailureReason::TerminalCreateFailed),
        CreateTerminalProvenance::OutcomeUnknown,
        CreateTerminalProvenance::BindingOnly,
    ];
    state.provenance = history.clone();
    let key = state.desired().unwrap().clone();
    assert!(state.invalidate_projection_frame_t(&key));
    let mode = state.controller_mode().cloned();
    let mut candidate = snapshot(7, 2, None);
    candidate.terminal_contexts[0].tabs.reverse();
    assert!(state.start_pending_refresh(1));
    assert_eq!(
        finish_refresh(
            &mut state,
            1,
            WorkbenchSnapshotOutcome::Available {
                snapshot: candidate
            },
            None
        ),
        RefreshDecision::Continue
    );
    assert_eq!(state.active().unwrap().index(), 6);
    assert_eq!(state.desired().unwrap(), &key);
    assert_eq!(state.attached_key(), Some(&key));
    assert_eq!(state.projection_key(), Some(&key));
    assert_eq!(state.controller_mode(), mode.as_ref());
    assert!(!state.feedback_accepted());
    assert_eq!(
        state.provenance(0),
        Some(&CreateTerminalProvenance::BindingOnly)
    );
    for (index, provenance) in history.iter().rev().enumerate() {
        assert_eq!(state.provenance(index + 1), Some(provenance));
    }
    assert!(state.invariant_holds());
    let operation = create_operation(&state, 2, 99, "new");
    assert!(state.begin_create(operation));
}

#[test]
fn reorder_preserves_both_stable_unattached_phases_with_nonfirst_selection() {
    for retryable in [false, true] {
        let mut state = retryable_state(Instant::now());
        if !retryable {
            state.provenance[1] = CreateTerminalProvenance::Pending;
            state.phase = SessionPhase::SelectedWithoutAttachment(state.active().unwrap().clone());
        }
        let phase = state.phase_kind();
        let key = state.desired().unwrap().clone();
        let provenance = state.provenance(1).cloned();
        let mut candidate = snapshot(3, 2, None);
        candidate.terminal_contexts[0].tabs.swap(0, 1);
        assert!(state.start_pending_refresh(1));
        assert_eq!(
            finish_refresh(
                &mut state,
                1,
                WorkbenchSnapshotOutcome::Available {
                    snapshot: candidate
                },
                None
            ),
            RefreshDecision::Continue
        );
        assert_eq!(state.phase_kind(), phase);
        assert_eq!(state.active().unwrap().index(), 0);
        assert_eq!(state.desired().unwrap(), &key);
        assert_eq!(state.provenance(0), provenance.as_ref());
        assert_eq!(state.attached_key(), None);
        assert_eq!(state.projection_key(), None);
        assert!(state.invariant_holds());
    }
}

#[test]
fn correlated_create_and_explicit_recovery_overlay_the_moved_created_id_only() {
    for recover in [false, true] {
        let (mut state, operation) = committed_create_state();
        state.provenance[0] = CreateTerminalProvenance::Started;
        assert!(!state.start_pending_refresh(9));
        assert_eq!(state.attached_key(), state.desired());
        let mut candidate = snapshot(3, 3, None);
        candidate.terminal_contexts[0].tabs.rotate_left(1);
        candidate.terminal_contexts[0].tabs[0].label = "renamed created".into();
        if recover {
            assert_eq!(
                state.settle_create_snapshot(&operation, WorkbenchSnapshotOutcome::Denied {}),
                CreateProjectionDecision::ProjectionUnavailable
            );
            assert!(state.start_pending_refresh(9));
            assert_eq!(
                finish_refresh(
                    &mut state,
                    9,
                    WorkbenchSnapshotOutcome::Available {
                        snapshot: candidate
                    },
                    Some(OperationDeadlines::from_sample(Instant::now()).unwrap())
                ),
                RefreshDecision::SettledWithoutAttachment
            );
        } else {
            assert_eq!(
                state.settle_create_snapshot(
                    &operation,
                    WorkbenchSnapshotOutcome::Available {
                        snapshot: candidate
                    }
                ),
                CreateProjectionDecision::SettledWithoutAttachment
            );
        }
        assert_eq!(state.active().unwrap().index(), 0);
        assert_eq!(
            state.provenance(0),
            Some(&CreateTerminalProvenance::Pending)
        );
        assert_eq!(
            state.provenance(1),
            Some(&CreateTerminalProvenance::BindingOnly)
        );
        assert_eq!(
            state.provenance(2),
            Some(&CreateTerminalProvenance::Started)
        );
        assert_eq!(state.view().tab(0).unwrap().tab_label(), "renamed created");
        assert!(state.invariant_holds());
    }
}

#[test]
fn reused_created_id_at_a_later_revision_cannot_overlay_provenance_during_recovery() {
    for recover in [false, true] {
        let mut state = ready(2);
        state.provenance[0] = CreateTerminalProvenance::Started;
        let baseline = state.cache().clone();
        let history = state.provenance.clone();
        let operation = create_operation(&state, 2, 20, "1");
        let key = operation.key().clone();
        let existing = state.tab(0).unwrap().clone();
        assert!(state.begin_create(operation));
        assert!(state.retain_create_commit(
            &key,
            WorkbenchTabCreateCommit::Pending {
                revision: WorkbenchRevision::new(2),
                terminal_context_id: context_id(2),
                tab_id: existing.tab_id().clone(),
                panel_id: existing.panel_id().clone(),
                presentation_id: existing.presentation_id().clone(),
            }
        ));

        let mut candidate = snapshot(3, 3, None);
        candidate.terminal_contexts[0].tabs.reverse();
        if recover {
            assert_eq!(
                state.settle_create_snapshot(&key, WorkbenchSnapshotOutcome::Denied {}),
                CreateProjectionDecision::ProjectionUnavailable
            );
            assert!(state.start_pending_refresh(9));
            assert_eq!(
                finish_refresh(
                    &mut state,
                    9,
                    WorkbenchSnapshotOutcome::Available {
                        snapshot: candidate
                    },
                    None
                ),
                RefreshDecision::Retire
            );
        } else {
            assert_eq!(
                state.settle_create_snapshot(
                    &key,
                    WorkbenchSnapshotOutcome::Available {
                        snapshot: candidate
                    }
                ),
                CreateProjectionDecision::Retire
            );
        }
        assert_eq!(state.cache(), &baseline);
        assert_eq!(state.provenance, history);
        assert_eq!(
            state.retired_failure(),
            Some(CarrierFailure::AuthorityDivergence)
        );
        assert!(state.invariant_holds());
    }
}
