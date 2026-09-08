use super::*;
use crate::local_client::workbench::WorkbenchTabMoveCommit;
use orchbee_workbench_protocol::WorkbenchTabMoveDirection;

fn operation(state: &NativeSessionState, request: u128) -> MoveOperation {
    state
        .plan_move(
            WorkbenchRequestId::parse(&id(request)).unwrap(),
            WorkbenchTabMoveDirection::Next,
            Instant::now() + OPERATION_TOTAL,
        )
        .unwrap()
}

fn commit(operation: &MoveOperation) -> WorkbenchTabMoveCommit {
    WorkbenchTabMoveCommit {
        revision: WorkbenchRevision::new(operation.baseline().revision().value() + 1),
        terminal_context_id: operation.baseline().terminal_context_id().clone(),
        tab_id: operation.target().tab_id().clone(),
    }
}

#[test]
fn move_keeps_historical_ready_and_read_only_viewer_through_recovery() {
    for read_only in [false, true] {
        let mut state = ready(3);
        if read_only {
            let SessionPhase::Ready(ready) = &mut state.phase else {
                unreachable!()
            };
            ready.mode = ControllerMode::ReadOnly;
        }
        let key = state.desired().unwrap().clone();
        assert!(state.invalidate_projection_frame(&key));
        let original_mode = state.controller_mode().cloned();
        let operation = operation(&state, 100);
        assert!(state.begin_move(operation.clone()));
        assert_eq!(
            state.plan_selection(ClientAction::SelectNextTab),
            SelectionPlan::Busy
        );
        assert!(!state.start_pending_refresh(1));
        let prepared = state.prepare_move(
            &operation,
            MoveResult::Committed {
                commit: commit(&operation),
                outcome: WorkbenchSnapshotOutcome::Denied {},
            },
        );
        assert_eq!(state.apply_prepared_move(prepared), MoveDecision::Continue);
        assert_eq!(
            state.phase_kind(),
            SessionPhaseKind::MoveProjectionUnavailable
        );
        assert_eq!(state.desired().unwrap(), &key);
        assert_eq!(state.projection_key(), Some(&key));
        assert_eq!(state.controller_mode(), original_mode.as_ref());
        assert!(!state.feedback_accepted());
        assert!(
            state
                .plan_move(
                    WorkbenchRequestId::parse(&id(101)).unwrap(),
                    WorkbenchTabMoveDirection::Next,
                    Instant::now() + OPERATION_TOTAL
                )
                .is_none()
        );
        assert_eq!(
            state.plan_selection(ClientAction::SelectTab(1)),
            SelectionPlan::Busy
        );
        assert!(state.start_pending_refresh(2));
        let mut candidate = snapshot(3, 2, None);
        candidate.terminal_contexts[0].tabs.swap(0, 1);
        assert_eq!(
            finish_refresh(
                &mut state,
                2,
                WorkbenchSnapshotOutcome::Available {
                    snapshot: candidate
                },
                None
            ),
            RefreshDecision::Continue
        );
        assert_eq!(state.phase_kind(), SessionPhaseKind::Ready);
        assert_eq!(state.active().unwrap().index(), 1);
        assert_eq!(state.desired().unwrap(), &key);
        assert_eq!(state.projection_key(), Some(&key));
        assert_eq!(state.controller_mode(), original_mode.as_ref());
        assert!(!state.feedback_accepted());
        assert!(state.invariant_holds());
    }
}

#[test]
fn settled_move_a_then_queued_b_retirement_recognizes_only_exact_a_commit() {
    let mut state = ready(3);
    let first = operation(&state, 100);
    let first_commit = commit(&first);
    assert!(state.begin_move(first.clone()));
    let mut candidate = snapshot(3, 2, None);
    candidate.terminal_contexts[0].tabs.swap(0, 1);
    let prepared = state.prepare_move(
        &first,
        MoveResult::Committed {
            commit: first_commit.clone(),
            outcome: WorkbenchSnapshotOutcome::Available {
                snapshot: candidate,
            },
        },
    );
    assert_eq!(state.apply_prepared_move(prepared), MoveDecision::Continue);
    let second = operation(&state, 101);
    assert!(state.begin_move(second.clone()));
    assert!(state.retain_pending_move(PendingMoveCommit::new(first.clone(), first_commit.clone())));
    assert_eq!(state.pending_move(), Some(&second));
    let mut wrong = first_commit.clone();
    wrong.tab_id = state.tab(0).unwrap().tab_id().clone();
    assert!(!state.retain_pending_move(PendingMoveCommit::new(first.clone(), wrong)));
    state.retire(CarrierFailure::Closed);
    assert!(state.retain_pending_move(PendingMoveCommit::new(first, first_commit)));
    assert_eq!(state.pending_move(), Some(&second));
    assert!(state.invariant_holds());
}

#[test]
fn move_reorders_both_unattached_phases_and_preserves_nonfirst_identity_and_provenance() {
    for retryable in [false, true] {
        let mut state = retryable_state(Instant::now());
        if !retryable {
            state.provenance[1] = CreateTerminalProvenance::Pending;
            state.phase = SessionPhase::SelectedWithoutAttachment(state.active().unwrap().clone());
        }
        let phase = state.phase_kind();
        let key = state.desired().unwrap().clone();
        let provenance = state.provenance(1).cloned();
        let operation = operation(&state, 100);
        assert_eq!(operation.direction(), WorkbenchTabMoveDirection::First);
        assert!(state.begin_move(operation.clone()));
        let mut candidate = snapshot(2, 2, None);
        candidate.terminal_contexts[0].tabs.swap(0, 1);
        let prepared = state.prepare_move(
            &operation,
            MoveResult::Committed {
                commit: commit(&operation),
                outcome: WorkbenchSnapshotOutcome::Available {
                    snapshot: candidate,
                },
            },
        );
        assert_eq!(state.apply_prepared_move(prepared), MoveDecision::Continue);
        assert_eq!(state.phase_kind(), phase);
        assert_eq!(state.desired().unwrap(), &key);
        assert_eq!(state.active().unwrap().index(), 0);
        assert_eq!(state.provenance(0), provenance.as_ref());
        assert_eq!(state.attached_key(), None);
        assert!(state.invariant_holds());
    }
}

#[test]
fn move_noop_and_definite_precommit_release_without_cache_or_viewer_change() {
    for result in [
        MoveResult::NotSubmitted,
        MoveResult::Precommit(crate::local_client::workbench::WorkbenchTabMovePrecommit::Denied),
        MoveResult::Unchanged(WorkbenchRevision::new(1)),
    ] {
        let mut state = ready(1);
        let baseline = state.cache().clone();
        let ready = state.ready().cloned();
        let operation = operation(&state, 100);
        assert_eq!(operation.direction(), WorkbenchTabMoveDirection::First);
        assert!(state.begin_move(operation.clone()));
        let prepared = state.prepare_move(&operation, result.clone());
        assert_eq!(state.apply_prepared_move(prepared), MoveDecision::Continue);
        assert_eq!(state.cache(), &baseline);
        assert_eq!(state.ready(), ready.as_ref());
        assert!(state.feedback_accepted());
        assert!(!state.move_blocks_structure());
        assert_eq!(
            state.notice(),
            if matches!(result, MoveResult::Unchanged(_)) {
                None
            } else {
                Some(SessionNotice::MoveNotMoved)
            }
        );
        assert!(state.invariant_holds());
    }
}

#[test]
fn move_corruption_retains_only_valid_known_commit_and_never_installs_candidate() {
    for corrupt_commit in [false, true] {
        let mut state = ready(3);
        let baseline = state.cache().clone();
        let operation = operation(&state, 100);
        let mut returned = commit(&operation);
        if corrupt_commit {
            returned.tab_id = state.tab(1).unwrap().tab_id().clone();
        }
        assert!(state.begin_move(operation.clone()));
        // This valid schema is the wrong exact-commit order.
        let prepared = state.prepare_move(
            &operation,
            MoveResult::Committed {
                commit: returned,
                outcome: WorkbenchSnapshotOutcome::Available {
                    snapshot: snapshot(3, 2, None),
                },
            },
        );
        assert_eq!(state.apply_prepared_move(prepared), MoveDecision::Retire);
        assert_eq!(state.cache(), &baseline);
        assert_eq!(state.settled_move.is_some(), !corrupt_commit);
        assert!(state.invariant_holds());
    }
}

#[test]
fn move_recovery_keeps_failure_fenced_and_accepts_later_supported_membership() {
    let mut state = ready(3);
    let operation = operation(&state, 100);
    assert!(state.begin_move(operation.clone()));
    let prepared = state.prepare_move(
        &operation,
        MoveResult::Committed {
            commit: commit(&operation),
            outcome: WorkbenchSnapshotOutcome::Denied {},
        },
    );
    assert_eq!(state.apply_prepared_move(prepared), MoveDecision::Continue);
    assert!(state.start_pending_refresh(1));
    assert_eq!(
        finish_refresh(&mut state, 1, WorkbenchSnapshotOutcome::Denied {}, None),
        RefreshDecision::Continue
    );
    assert_eq!(
        state.phase_kind(),
        SessionPhaseKind::MoveProjectionUnavailable
    );
    assert!(!state.begin_create(create_operation(&state, 2, 101, "new")));
    assert!(state.start_pending_refresh(2));
    let mut candidate = snapshot(4, 4, None);
    candidate.terminal_contexts[0].tabs.reverse();
    assert_eq!(
        finish_refresh(
            &mut state,
            2,
            WorkbenchSnapshotOutcome::Available {
                snapshot: candidate
            },
            None
        ),
        RefreshDecision::Continue
    );
    assert_eq!(state.active().unwrap().index(), 3);
    assert_eq!(state.cache().tabs().len(), 4);
    assert_eq!(state.phase_kind(), SessionPhaseKind::Ready);
    assert!(state.invariant_holds());
}

#[test]
fn move_recovery_keeps_exact_commit_through_common_cleanup_and_empty_install() {
    let mut state = ready(2);
    let operation = operation(&state, 110);
    let pending = PendingMoveCommit::new(operation.clone(), commit(&operation));
    let old = operation.target().clone();
    let baseline = state.cache().clone();
    assert!(state.begin_move(operation.clone()));
    let prepared = state.prepare_move(
        &operation,
        MoveResult::Committed {
            commit: pending.commit().clone(),
            outcome: WorkbenchSnapshotOutcome::Denied {},
        },
    );
    assert_eq!(state.apply_prepared_move(prepared), MoveDecision::Continue);
    assert!(state.start_pending_refresh(7));
    let PreparedRefresh::Membership { prepared, .. } = state.prepare_refresh(
        7,
        WorkbenchSnapshotOutcome::Available {
            snapshot: snapshot(0, 3, None),
        },
    ) else {
        panic!("later removal is a membership candidate");
    };
    let cleanup = state
        .stage_refresh_membership_cleanup(
            7,
            prepared,
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
        )
        .unwrap();
    assert!(
        state.invariant_holds(),
        "known Move recovery must remain exact while cleanup is pending"
    );
    assert_eq!(state.cache(), &baseline);
    assert_eq!(state.attached_key(), Some(&old));
    assert_eq!(state.retained_move_for_test(), Some(&pending));
    let candidate = state
        .prepare_cleaned_membership_candidate(&cleanup, &old)
        .unwrap();
    drop(
        state
            .prepare_cleaned_membership_install(&cleanup, candidate, None)
            .unwrap(),
    );
    assert_eq!(state.move_recovery.as_ref(), Some(&pending));
    assert_eq!(state.cache(), &baseline);
    let candidate = state
        .prepare_cleaned_membership_candidate(&cleanup, &old)
        .unwrap();
    assert_eq!(
        state
            .prepare_cleaned_membership_install(&cleanup, candidate, None)
            .unwrap()
            .commit(),
        MembershipInstallDecision::Empty
    );
    assert!(state.move_recovery.is_none());
    assert_eq!(state.retained_move_for_test(), Some(&pending));
    assert_eq!(state.phase_kind(), SessionPhaseKind::Empty);
    assert!(state.invariant_holds());
}

#[test]
fn move_preparation_distinguishes_later_removal_from_exact_commit_corruption() {
    for revision in [2, 3] {
        let mut state = ready(2);
        let operation = operation(&state, 120);
        assert!(state.begin_move(operation.clone()));
        let prepared = state.prepare_move(
            &operation,
            MoveResult::Committed {
                commit: commit(&operation),
                outcome: WorkbenchSnapshotOutcome::Available {
                    snapshot: snapshot(0, revision, None),
                },
            },
        );
        assert_eq!(prepared.membership_candidate().is_some(), revision == 3);
        assert_eq!(state.cache(), operation.baseline());
        assert!(
            state.retained_move_for_test().is_none(),
            "pure preparation does not retain or install"
        );
        if revision == 2 {
            assert_eq!(state.apply_prepared_move(prepared), MoveDecision::Retire);
            assert!(state.retained_move_for_test().is_some());
        }
    }
}

#[test]
fn tab_unavailable_snapshot_obeys_refusal_floor_without_inventing_a_move_commit() {
    for revision in [2, 3, 4] {
        let mut state = ready(2);
        let operation = operation(&state, 121);
        assert!(state.begin_move(operation.clone()));
        let prepared = state.prepare_move(
            &operation,
            MoveResult::TabUnavailable {
                revision: WorkbenchRevision::new(3),
                outcome: WorkbenchSnapshotOutcome::Available {
                    snapshot: snapshot(0, revision, None),
                },
            },
        );
        assert_eq!(prepared.membership_candidate().is_some(), revision >= 3);
        if revision < 3 {
            assert_eq!(state.apply_prepared_move(prepared), MoveDecision::Retire);
        }
        assert!(state.retained_move_for_test().is_none());
        assert!(state.move_recovery.is_none());
    }
    let mut state = ready(2);
    let operation = operation(&state, 122);
    let old_ready = state.ready().cloned();
    assert!(state.begin_move(operation.clone()));
    let prepared = state.prepare_move(
        &operation,
        MoveResult::TabUnavailable {
            revision: WorkbenchRevision::new(3),
            outcome: WorkbenchSnapshotOutcome::Denied {},
        },
    );
    assert_eq!(state.apply_prepared_move(prepared), MoveDecision::Continue);
    assert_eq!(state.ready(), old_ready.as_ref());
    assert_eq!(state.notice(), Some(SessionNotice::MoveNotMoved));
    assert!(!state.move_blocks_structure());
    assert!(state.retained_move_for_test().is_none());
    assert!(state.start_pending_refresh(8));
}

#[test]
fn tab_unavailable_same_revision_cannot_reintroduce_the_refused_target() {
    for revision in [3, 4] {
        let mut state = ready(2);
        let operation = operation(&state, 123);
        let old = state.cache().clone();
        assert!(state.begin_move(operation.clone()));
        let prepared = state.prepare_move(
            &operation,
            MoveResult::TabUnavailable {
                revision: WorkbenchRevision::new(3),
                outcome: WorkbenchSnapshotOutcome::Available {
                    snapshot: snapshot(2, revision, None),
                },
            },
        );
        assert_eq!(
            state.apply_prepared_move(prepared),
            if revision == 3 {
                MoveDecision::Retire
            } else {
                MoveDecision::Continue
            }
        );
        assert!(state.retained_move_for_test().is_none());
        assert!(state.move_recovery.is_none());
        if revision == 3 {
            assert_eq!(
                state.retired_failure(),
                Some(CarrierFailure::AuthorityDivergence)
            );
            assert_eq!(state.cache(), &old);
        } else {
            assert!(state.ready().is_some());
            assert_eq!(state.cache().revision(), WorkbenchRevision::new(4));
        }
    }
}

#[test]
fn known_move_retention_precedes_a_failed_membership_install_without_changing_old_ownership() {
    let mut state = ready(2);
    let operation = operation(&state, 124);
    let old = state.desired().cloned();
    let baseline = state.cache().clone();
    assert!(state.begin_move(operation.clone()));
    let prepared = state.prepare_move(
        &operation,
        MoveResult::Committed {
            commit: commit(&operation),
            outcome: WorkbenchSnapshotOutcome::Available {
                snapshot: snapshot(0, 3, None),
            },
        },
    );
    let (retained, candidate) = state.retain_prepared_move_membership(prepared).unwrap();
    assert_eq!(retained, operation);
    assert_eq!(
        state.retained_move_for_test().unwrap().operation(),
        &operation
    );
    // Direct installation may not erase a still-owned Viewer, even though the commit is known.
    assert!(
        state
            .prepare_membership_install(candidate, None, None)
            .is_err()
    );
    assert_eq!(state.cache(), &baseline);
    assert_eq!(state.desired(), old.as_ref());
    assert_eq!(state.attached_key(), old.as_ref());
    assert_eq!(state.pending_move(), Some(&operation));
    state.retire(CarrierFailure::Protocol);
    assert_eq!(
        state.retained_move_for_test().unwrap().operation(),
        &operation
    );
    assert!(state.invariant_holds());
}
