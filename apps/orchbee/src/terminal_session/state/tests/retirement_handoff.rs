use super::*;

#[test]
fn creating_retirement_keeps_the_worker_known_commit() {
    let mut state = ready(1);
    let old_active = state.active().unwrap().clone();
    let operation = create_operation(&state, 2, 300, "2");
    let commit = pending_commit(2, 1);
    assert!(state.begin_create(operation.clone()));

    let pending = PendingCreateCommit::new(operation.clone(), commit.clone());
    assert_eq!(pending.operation(), &operation);
    assert!(state.finalize_retirement(CarrierFailure::Closed, Some(pending)));

    assert_eq!(state.phase_kind(), SessionPhaseKind::Retired);
    assert_eq!(state.active(), Some(&old_active));
    assert_eq!(state.desired().unwrap(), old_active.key());
    let retained = state
        .committed_target()
        .expect("worker-known commit retained");
    assert_eq!(retained.operation(), &operation);
    let WorkbenchTabCreateCommit::Pending { tab_id, .. } = commit else {
        unreachable!()
    };
    assert_eq!(retained.expectation().tab_id(), &tab_id);
    assert!(state.invariant_holds());
}

#[test]
fn handoff_preserves_every_terminal_provenance_and_snapshot_cache() {
    for (commit, expected_provenance) in commit_outcomes() {
        let mut state = ready(1);
        let cache = state.cache().clone();
        let active = state.active().unwrap().clone();
        let operation = create_operation(&state, 2, 300, "exact creation label");
        assert!(state.begin_create(operation.clone()));
        assert!(state.finalize_retirement(
            CarrierFailure::Deadline,
            Some(PendingCreateCommit::new(operation.clone(), commit)),
        ));
        let retained = state.committed_target().unwrap();
        assert_eq!(retained.operation(), &operation);
        assert_eq!(retained.provenance(), &expected_provenance);
        assert_eq!(
            retained.committed_key(),
            &AttachmentKey::new(2, tab_id(1010))
        );
        assert_eq!(
            retained.expectation(),
            &CreatedTabExpectation::new(
                WorkbenchRevision::new(2),
                context_id(2),
                tab_id(1010),
                panel_id(1011),
                presentation_id(1012),
                WorkbenchLabel::parse("exact creation label").unwrap(),
            ),
        );
        assert_eq!(state.cache(), &cache);
        assert_eq!(state.active(), Some(&active));
        assert_eq!(state.desired().unwrap(), active.key());
        assert_eq!(state.retired_failure(), Some(CarrierFailure::Deadline));
        assert!(state.invariant_holds());
    }
}

#[test]
fn exact_retained_duplicate_preserves_commit_cleanup_and_unavailable_shapes() {
    for (commit, _) in commit_outcomes() {
        for cut in [
            "retained",
            "cleanup pending",
            "projection unavailable",
            "retired",
        ] {
            let mut state = ready(1);
            let operation = create_operation(&state, 2, 300, "2");
            assert!(state.begin_create(operation.clone()));
            assert!(state.retain_create_commit(operation.key(), commit.clone()));
            if cut == "cleanup pending" {
                let PreparedCreateSnapshot::Available {
                    baseline,
                    committed,
                    projection,
                } = state.prepare_create_snapshot(
                    operation.key(),
                    WorkbenchSnapshotOutcome::Available {
                        snapshot: snapshot(2, 2, Some("2")),
                    },
                )
                else {
                    panic!("known Create candidate must validate")
                };
                let candidate = state
                    .prepare_create_membership_candidate(&baseline, &committed, projection)
                    .unwrap();
                let cleanup = state
                    .stage_create_membership_cleanup(
                        operation.key(),
                        candidate,
                        operation.deadlines(),
                    )
                    .unwrap();
                assert_eq!(
                    state.attached_key().map(|pane| pane.anchor()),
                    Some(cleanup.old())
                );
            }
            if cut == "projection unavailable" {
                assert_eq!(
                    state.settle_create_snapshot(
                        operation.key(),
                        WorkbenchSnapshotOutcome::Denied {}
                    ),
                    CreateProjectionDecision::ProjectionUnavailable,
                );
            }
            if cut == "retired" {
                state.retire(CarrierFailure::Transport);
            }
            let retained = state.committed_target().unwrap().clone();
            let active = state.active().cloned();
            let desired = state.desired().unwrap().clone();
            assert!(
                state.finalize_retirement(
                    CarrierFailure::Closed,
                    Some(PendingCreateCommit::new(operation.clone(), commit.clone())),
                ),
                "{cut}"
            );
            assert_eq!(state.committed_target(), Some(&retained), "{cut}");
            assert_eq!(state.active(), active.as_ref(), "{cut}");
            assert_eq!(state.desired().unwrap(), &desired, "{cut}");
            assert_eq!(
                state.retired_failure(),
                Some(if cut == "retired" {
                    CarrierFailure::Transport
                } else {
                    CarrierFailure::Closed
                })
            );
            assert!(state.invariant_holds(), "{cut}");
        }
    }
}

#[test]
fn full_operation_mismatch_cannot_inject_or_replace_retirement_facts() {
    for retained in [false, true] {
        for field in [
            "stale request",
            "future request",
            "stale generation",
            "future generation",
            "site",
            "revision",
            "context",
            "label",
            "geometry",
            "forward cutoff",
            "total",
        ] {
            let mut state = ready(1);
            let operation = create_operation(&state, 2, 300, "2");
            let commit = pending_commit(2, 1);
            assert!(state.begin_create(operation.clone()));
            if retained {
                assert!(state.retain_create_commit(operation.key(), commit.clone()));
            }
            let expected = state.committed_target().cloned();
            let mut wrong = operation;
            match field {
                "stale request" => wrong.key.request_id = workbench_request_id(299),
                "future request" => wrong.key.request_id = workbench_request_id(301),
                "stale generation" => wrong.key.reserved_generation = 1,
                "future generation" => wrong.key.reserved_generation = 3,
                "site" => wrong.site_id = site_id(99),
                "revision" => wrong.expected_revision = WorkbenchRevision::new(9),
                "context" => wrong.terminal_context_id = context_id(99),
                "label" => wrong.label = WorkbenchLabel::parse("other").unwrap(),
                "geometry" => wrong.geometry = geometry(81),
                "forward cutoff" => wrong.deadlines.forward_cutoff += Duration::from_nanos(1),
                "total" => wrong.deadlines.total += Duration::from_nanos(1),
                _ => unreachable!(),
            }
            assert!(
                !state.finalize_retirement(
                    CarrierFailure::OutcomeUnknown,
                    Some(PendingCreateCommit::new(wrong, commit)),
                ),
                "{retained}/{field}"
            );
            assert_eq!(
                state.committed_target(),
                expected.as_ref(),
                "{retained}/{field}"
            );
            assert_eq!(
                state.retired_failure(),
                Some(CarrierFailure::OutcomeUnknown)
            );
            assert_eq!(state.desired().unwrap(), state.active().unwrap().key());
            assert!(state.invariant_holds(), "{retained}/{field}");
        }
    }
}

#[test]
fn invalid_adapter_facts_are_rejected_before_retaining_a_commit() {
    let mut invalid = Vec::new();
    for revision in [0, 1, 3, u64::MAX] {
        invalid.push(pending_commit(revision, 1));
    }
    let mut wrong_context = pending_commit(2, 1);
    let WorkbenchTabCreateCommit::Pending {
        terminal_context_id,
        ..
    } = &mut wrong_context
    else {
        unreachable!()
    };
    *terminal_context_id = context_id(99);
    invalid.push(wrong_context);
    for dimensions in [
        (81, 24, 8, 16),
        (80, 25, 8, 16),
        (80, 24, 9, 16),
        (80, 24, 8, 17),
    ] {
        invalid.push(created_commit(WorkbenchTerminalOutcome::Started {
            geometry: WorkbenchTerminalGeometry::new(
                dimensions.0,
                dimensions.1,
                dimensions.2,
                dimensions.3,
            )
            .unwrap(),
        }));
    }
    for (case, commit) in invalid.into_iter().enumerate() {
        for from_handoff in [false, true] {
            let mut state = ready(1);
            let operation = create_operation(&state, 2, 300, "2");
            assert!(state.begin_create(operation.clone()));
            if from_handoff {
                assert!(
                    !state.finalize_retirement(
                        CarrierFailure::Closed,
                        Some(PendingCreateCommit::new(operation, commit.clone())),
                    ),
                    "{case}"
                );
                assert_eq!(state.retired_failure(), Some(CarrierFailure::Closed));
            } else {
                assert!(
                    !state.retain_create_commit(operation.key(), commit.clone()),
                    "{case}"
                );
                assert_eq!(state.retired_failure(), Some(CarrierFailure::Protocol));
            }
            assert!(state.committed_target().is_none(), "{case}/{from_handoff}");
            assert!(state.invariant_holds(), "{case}/{from_handoff}");
        }
    }
}

#[test]
fn committed_revision_overflow_is_not_a_valid_handoff() {
    let mut state = ready(1);
    state.cache = projected(1, u64::MAX);
    let operation = create_operation(&state, 2, 300, "2");
    assert!(state.begin_create(operation.clone()));
    assert!(!state.finalize_retirement(
        CarrierFailure::Closed,
        Some(PendingCreateCommit::new(operation, pending_commit(0, 1))),
    ));
    assert!(state.committed_target().is_none());
    assert!(state.invariant_holds());
}

#[test]
fn conflicting_retained_tuple_or_provenance_never_replaces_the_first_commit() {
    for retired in [false, true] {
        for field in [
            "tab",
            "panel",
            "presentation",
            "pending",
            "unavailable reason",
        ] {
            let mut state = ready(1);
            let operation = create_operation(&state, 2, 300, "2");
            let commit = created_commit(WorkbenchTerminalOutcome::Unavailable {
                reason: WorkbenchTerminalUnavailableReason::TerminalRuntimeUnavailable,
            });
            assert!(state.begin_create(operation.clone()));
            assert!(state.retain_create_commit(operation.key(), commit.clone()));
            let expected = state.committed_target().cloned();
            if retired {
                state.retire(CarrierFailure::Transport);
            }
            let mut conflict = commit;
            let WorkbenchTabCreateCommit::Created {
                tab_id: tab,
                panel_id: panel,
                presentation_id: presentation,
                terminal_outcome,
                ..
            } = &mut conflict
            else {
                unreachable!()
            };
            match field {
                "tab" => *tab = tab_id(99),
                "panel" => *panel = panel_id(99),
                "presentation" => *presentation = presentation_id(99),
                "pending" => conflict = pending_commit(2, 1),
                "unavailable reason" => {
                    *terminal_outcome = WorkbenchTerminalOutcome::Unavailable {
                        reason: WorkbenchTerminalUnavailableReason::ServerBusy,
                    }
                }
                _ => unreachable!(),
            }
            assert!(
                !state.finalize_retirement(
                    CarrierFailure::Deadline,
                    Some(PendingCreateCommit::new(operation, conflict)),
                ),
                "{retired}/{field}"
            );
            assert_eq!(state.committed_target(), expected.as_ref());
            assert_eq!(
                state.retired_failure(),
                Some(if retired {
                    CarrierFailure::Transport
                } else {
                    CarrierFailure::Deadline
                })
            );
            assert!(state.invariant_holds());
        }
    }
}

#[test]
fn a_missing_pending_value_retires_without_inventing_a_commit() {
    let mut state = ready(1);
    let operation = create_operation(&state, 2, 300, "2");
    assert!(state.begin_create(operation));
    assert!(state.finalize_retirement(CarrierFailure::WorkerExited, None));
    assert!(state.committed_target().is_none());
    assert_eq!(state.retired_failure(), Some(CarrierFailure::WorkerExited));
    assert!(state.invariant_holds());
}

#[test]
fn a_handoff_without_a_corresponding_create_phase_is_rejected() {
    for phase in ["starting", "ready", "restored", "retired"] {
        let mut state = if phase == "starting" {
            state(1)
        } else {
            ready(1)
        };
        let operation = create_operation(&state, 2, 300, "2");
        if phase == "restored" {
            assert!(state.begin_create(operation.clone()));
            assert!(state.restore_precommit(operation.key(), Some(geometry(80))));
        }
        if phase == "retired" {
            assert!(state.begin_create(operation.clone()));
            state.retire(CarrierFailure::Transport);
        }
        assert!(
            !state.finalize_retirement(
                CarrierFailure::Closed,
                Some(PendingCreateCommit::new(operation, pending_commit(2, 1))),
            ),
            "{phase}"
        );
        assert!(state.committed_target().is_none());
        assert_eq!(
            state.retired_failure(),
            Some(if phase == "retired" {
                CarrierFailure::Transport
            } else {
                CarrierFailure::Closed
            })
        );
        assert!(state.invariant_holds());
    }
}

#[test]
fn an_earlier_completed_create_cannot_diagnose_a_later_operation() {
    for later_phase in ["ready", "creating", "retained", "restored"] {
        let (mut state, earlier, earlier_commit) = completed_create();
        let later = create_operation(&state, 3, 301, "3");
        if later_phase != "ready" {
            assert!(state.begin_create(later.clone()));
        }
        if later_phase == "retained" {
            assert!(state.retain_create_commit(later.key(), pending_commit(3, 2)));
        }
        if later_phase == "restored" {
            assert!(state.restore_precommit(later.key(), Some(geometry(80))));
        }
        let expected = state.committed_target().cloned();
        let active = state.active().cloned();
        assert!(
            !state.finalize_retirement(
                CarrierFailure::Deadline,
                Some(PendingCreateCommit::new(earlier, earlier_commit)),
            ),
            "{later_phase}"
        );
        assert_eq!(state.committed_target(), expected.as_ref());
        assert_eq!(state.active(), active.as_ref());
        assert_eq!(state.cache().revision(), WorkbenchRevision::new(2));
        assert!(state.invariant_holds());
    }
}

#[test]
fn repeated_finalization_preserves_the_first_cause_and_rejects_post_retirement_effects() {
    for first in [
        CarrierFailure::OutcomeUnknown,
        CarrierFailure::Deadline,
        CarrierFailure::Transport,
    ] {
        let mut state = ready(2);
        let operation = create_operation(&state, 2, 300, "3");
        let commit = pending_commit(2, 2);
        let active = state.active().unwrap().clone();
        let switch = state
            .target_for_switch(
                1,
                AttachmentKey::new(3, state.tab(1).unwrap().tab_id().clone()),
                operation.deadlines(),
            )
            .unwrap();
        assert!(state.begin_create(operation.clone()));
        assert!(state.finalize_retirement(
            first,
            Some(PendingCreateCommit::new(operation.clone(), commit.clone()))
        ));
        let expected = state.committed_target().cloned();
        assert!(state.finalize_retirement(CarrierFailure::Closed, None));
        assert!(state.finalize_retirement(
            CarrierFailure::Protocol,
            Some(PendingCreateCommit::new(operation.clone(), commit.clone())),
        ));
        assert!(!state.finalize_retirement(
            CarrierFailure::WorkerExited,
            Some(PendingCreateCommit::new(
                operation.clone(),
                pending_commit(2, 9)
            )),
        ));
        assert_eq!(state.retired_failure(), Some(first));
        assert_eq!(state.committed_target(), expected.as_ref());
        assert!(!state.begin_create(operation.clone()));
        assert!(!state.begin_switch(switch));
        assert!(!state.restore_precommit(operation.key(), Some(geometry(80))));
        assert!(!state.retain_create_commit(operation.key(), commit));
        assert!(matches!(
            state.prepare_create_snapshot(
                operation.key(),
                WorkbenchSnapshotOutcome::Available {
                    snapshot: snapshot(3, 3, Some("3")),
                }
            ),
            PreparedCreateSnapshot::Invalid
        ));
        assert_eq!(
            state.settle_create_snapshot(operation.key(), WorkbenchSnapshotOutcome::Denied {}),
            CreateProjectionDecision::Retire
        );
        assert_eq!(
            state.plan_selection(ClientAction::SelectNextTab),
            SelectionPlan::Busy
        );
        assert!(!state.phase_allows_refresh());
        assert!(!state.start_pending_refresh(9));
        assert!(!state.record_admitted_geometry_t(active.key(), geometry(90)));
        assert!(!state.install_projection_t(active.key()));
        assert!(!state.stage_drawn_frame_t(active.key(), FrameIdentity::new(9)));
        assert!(!state.mark_feedback_accepted_t(active.key(), FrameIdentity::new(9)));
        assert!(state.attached_key().is_none());
        assert!(state.projection_key().is_none());
        assert!(state.controller_mode().is_none());
        assert!(!state.feedback_accepted());
        assert_eq!(state.active(), Some(&active));
        assert_eq!(state.desired().unwrap(), active.key());
        assert_eq!(state.committed_target(), expected.as_ref());
        assert!(state.invariant_holds());
    }
}

fn completed_create() -> (
    NativeSessionState,
    CreateOperation,
    WorkbenchTabCreateCommit,
) {
    let mut state = ready(1);
    let operation = create_operation(&state, 2, 300, "2");
    let commit = created_commit(WorkbenchTerminalOutcome::Started {
        geometry: workbench_geometry(80),
    });
    assert!(state.begin_create(operation.clone()));
    assert!(state.retain_create_commit(operation.key(), commit.clone()));

    let CreateProjectionDecision::Attach(target) = state.settle_create_snapshot(
        operation.key(),
        WorkbenchSnapshotOutcome::Available {
            snapshot: snapshot(2, 2, Some("2")),
        },
    ) else {
        panic!("created snapshot must license Attach");
    };
    let key = target.selected().key();
    assert!(state.install_projection_t(key));
    assert!(state.record_setup_configuration_t(key, controlling(), Some(geometry(80))));
    assert!(state.stage_drawn_frame_t(key, FrameIdentity::new(2)));
    assert!(state.mark_feedback_accepted_t(key, FrameIdentity::new(2)));
    assert!(state.publish_ready_t(
        key,
        FrameIdentity::new(2),
        controlling(),
        Some(geometry(80))
    ));
    assert!(state.invariant_holds());
    (state, operation, commit)
}

fn commit_outcomes() -> Vec<(WorkbenchTabCreateCommit, CreateTerminalProvenance)> {
    vec![
        (pending_commit(2, 1), CreateTerminalProvenance::Pending),
        (
            created_commit(WorkbenchTerminalOutcome::Started {
                geometry: workbench_geometry(80),
            }),
            CreateTerminalProvenance::Started,
        ),
        (
            created_commit(WorkbenchTerminalOutcome::Unavailable {
                reason: WorkbenchTerminalUnavailableReason::TerminalRuntimeUnavailable,
            }),
            CreateTerminalProvenance::Unavailable(
                WorkbenchTerminalUnavailableReason::TerminalRuntimeUnavailable,
            ),
        ),
        (
            created_commit(WorkbenchTerminalOutcome::Unavailable {
                reason: WorkbenchTerminalUnavailableReason::ServerBusy,
            }),
            CreateTerminalProvenance::Unavailable(WorkbenchTerminalUnavailableReason::ServerBusy),
        ),
        (
            created_commit(WorkbenchTerminalOutcome::Failed {
                reason: WorkbenchTerminalFailureReason::TerminalCreateFailed,
            }),
            CreateTerminalProvenance::Failed(WorkbenchTerminalFailureReason::TerminalCreateFailed),
        ),
        (
            created_commit(WorkbenchTerminalOutcome::OutcomeUnknown {}),
            CreateTerminalProvenance::OutcomeUnknown,
        ),
    ]
}

fn created_commit(terminal_outcome: WorkbenchTerminalOutcome) -> WorkbenchTabCreateCommit {
    let WorkbenchTabCreateCommit::Pending {
        revision,
        terminal_context_id,
        tab_id,
        panel_id,
        presentation_id,
    } = pending_commit(2, 1)
    else {
        unreachable!()
    };
    WorkbenchTabCreateCommit::Created {
        revision,
        terminal_context_id,
        tab_id,
        panel_id,
        presentation_id,
        terminal_outcome,
    }
}

fn pending_commit(revision: u64, index: usize) -> WorkbenchTabCreateCommit {
    let tab = tab(index);
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = tab.root
    else {
        unreachable!()
    };
    WorkbenchTabCreateCommit::Pending {
        revision: WorkbenchRevision::new(revision),
        terminal_context_id: context_id(2),
        tab_id: tab.tab_id,
        panel_id,
        presentation_id: primary_presentation.presentation_id,
    }
}
