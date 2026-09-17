//! Deterministic panel witnesses for the canonical core: admission guards, exact result
//! handling, and the repaired same-Tab selected-leaf-loss membership path.
use super::*;
use crate::client_view::PanelDirection;
#[cfg(target_os = "linux")]
use crate::local_client::workbench::WorkbenchTabCloseAccepted;
use crate::local_client::workbench::panel_close::WorkbenchPanelCloseAccepted;
use crate::local_client::workbench::panel_move::WorkbenchPanelMoveCommit;
use crate::local_client::workbench::panel_split::WorkbenchPanelSplitCommit;
use crate::terminal_session::state::panel::{
    PanelCommit, PanelDecision, PanelIntent, PanelOperation, PanelPrecommit, PanelRequestKind,
    PanelResult, PendingPanelCommit,
};
use orchbee_workbench_protocol::{
    WorkbenchCommandUnavailableReason, WorkbenchPanelSplitDirection, WorkbenchRejectionReason,
    WorkbenchSnapshotOutcome, WorkbenchSnapshotUnavailableReason, WorkbenchTabMoveDirection,
};

fn plan(state: &NativeSessionState, request: u128, kind: PanelRequestKind) -> PanelOperation {
    state
        .plan_panel(
            WorkbenchRequestId::parse(&id(request)).unwrap(),
            kind,
            Instant::now() + OPERATION_TOTAL,
        )
        .expect("ready fixture admits the specified panel action")
}

fn split_kind() -> PanelRequestKind {
    PanelRequestKind::Split {
        direction: PanelDirection::Right,
    }
}

fn move_kind() -> PanelRequestKind {
    PanelRequestKind::Move {
        direction: PanelDirection::Right,
    }
}

fn split_ready() -> NativeSessionState {
    ready_with_cache(project_native_standalone_tabs(&split_panel_baseline()).unwrap())
}

fn split_two_tab_baseline() -> Snapshot {
    let mut baseline = snapshot(2, 1, None);
    baseline.terminal_contexts[0].tabs[0].root = PanelNode::Split {
        children: Box::new([
            PanelSlot {
                node: PanelNode::Panel {
                    child_presentations: Vec::new(),
                    panel_id: panel_id(1_001),
                    primary_presentation: Presentation {
                        binding: Some(PresentationBinding {
                            terminal_id: terminal_id(1_004),
                            worker_id: worker_id(1_003),
                        }),
                        label: "1".into(),
                        presentation_id: presentation_id(1_002),
                    },
                },
                slot_id: WorkbenchPanelSlotId::parse(&id(3_001)).unwrap(),
            },
            PanelSlot {
                node: PanelNode::Panel {
                    child_presentations: Vec::new(),
                    panel_id: panel_id(3_010),
                    primary_presentation: Presentation {
                        binding: Some(PresentationBinding {
                            terminal_id: terminal_id(3_012),
                            worker_id: worker_id(3_011),
                        }),
                        label: "peer".into(),
                        presentation_id: presentation_id(3_013),
                    },
                },
                slot_id: WorkbenchPanelSlotId::parse(&id(3_014)).unwrap(),
            },
        ]),
        orientation: PanelOrientation::Horizontal,
        split_id: WorkbenchSplitId::parse(&id(3_020)).unwrap(),
    };
    baseline
}

fn split_ready_two_tabs() -> NativeSessionState {
    ready_with_cache(project_native_standalone_tabs(&split_two_tab_baseline()).unwrap())
}

/// A Retryable attachment state on the split baseline with the selection on the second
/// leaf (panel 3010): the switch was admitted, installed, and settled unattached.
fn split_retryable() -> NativeSessionState {
    let (mut state, marker, provenance) = switching_to_nonfirst_panel();
    let fresh = install_switch_candidate(&mut state, &marker);
    assert_eq!(
        state.settle_unattached_setup(&fresh, 0, &provenance),
        Some(CleanupContinuation::Stable)
    );
    state
}

/// The same split baseline admitted at an arbitrary cache revision, for results that
/// must be compared against a baseline newer than revision 1.
fn split_ready_at_revision(revision: u64) -> NativeSessionState {
    let mut baseline = split_panel_baseline();
    baseline.revision = WorkbenchRevision::new(revision);
    ready_with_cache(project_native_standalone_tabs(&baseline).unwrap())
}

/// A SelectedWithoutAttachment state built by the real create admission path: a create
/// whose terminal outcome is `OutcomeUnknown` records a non-attachable provenance, so
/// the snapshot settle lands `SettledWithoutAttachment` — the new Tab is selected and
/// the desired key stays armed, but no attachment exists.
fn selected_without_attachment_state() -> NativeSessionState {
    let mut state = ready(1);
    let operation = create_operation(&state, 2, 100, "2");
    let key = operation.key().clone();
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
        &key,
        WorkbenchTabCreateCommit::Created {
            revision: WorkbenchRevision::new(2),
            terminal_context_id: context_id(2),
            tab_id: created.tab_id.clone(),
            panel_id: panel_id.clone(),
            presentation_id: primary_presentation.presentation_id.clone(),
            terminal_outcome: WorkbenchTerminalOutcome::OutcomeUnknown {},
        },
    ));
    let decision = state.settle_create_snapshot(
        &key,
        WorkbenchSnapshotOutcome::Available {
            snapshot: candidate,
        },
    );
    assert_eq!(decision, CreateProjectionDecision::SettledWithoutAttachment);
    assert_eq!(
        state.phase_kind(),
        SessionPhaseKind::SelectedWithoutAttachment
    );
    state
}

fn split_commit(operation: &PanelOperation) -> PanelCommit {
    PanelCommit::Split(WorkbenchPanelSplitCommit::Pending {
        revision: WorkbenchRevision::new(operation.baseline().revision().value() + 1),
        terminal_context_id: operation.baseline().terminal_context_id().clone(),
        tab_id: operation.target().tab_id().clone(),
        panel_id: operation.panel_id().clone(),
        new_panel_id: WorkbenchPanelId::parse(&id(90_001)).unwrap(),
        split_id: WorkbenchSplitId::parse(&id(90_002)).unwrap(),
        first_slot_id: WorkbenchPanelSlotId::parse(&id(90_003)).unwrap(),
        second_slot_id: WorkbenchPanelSlotId::parse(&id(90_004)).unwrap(),
        presentation_id: TerminalPresentationId::parse(&id(90_005)).unwrap(),
    })
}

fn pending_split(operation: &PanelOperation) -> PendingPanelCommit {
    PendingPanelCommit::new(operation.clone(), split_commit(operation))
}

fn move_commit(operation: &PanelOperation) -> PanelCommit {
    let PanelIntent::Move {
        target_panel_id, ..
    } = operation.intent()
    else {
        panic!("move intent")
    };
    PanelCommit::Move(WorkbenchPanelMoveCommit {
        revision: WorkbenchRevision::new(operation.baseline().revision().value() + 1),
        terminal_context_id: operation.baseline().terminal_context_id().clone(),
        tab_id: operation.target().tab_id().clone(),
        panel_id: operation.panel_id().clone(),
        target_panel_id: target_panel_id.clone(),
    })
}

/// The exact committed projection of a Right split of panel 1001: the fresh graft split
/// replaces leaf 1001 inside the original slot and carries the declared slot/split ids.
fn split_grafted_snapshot() -> Snapshot {
    let mut candidate = split_panel_baseline();
    candidate.revision = WorkbenchRevision::new(2);
    candidate.terminal_contexts[0].tabs[0].root = PanelNode::Split {
        children: Box::new([
            PanelSlot {
                node: PanelNode::Split {
                    children: Box::new([
                        PanelSlot {
                            node: PanelNode::Panel {
                                child_presentations: Vec::new(),
                                panel_id: panel_id(1_001),
                                primary_presentation: Presentation {
                                    binding: Some(PresentationBinding {
                                        terminal_id: terminal_id(1_004),
                                        worker_id: worker_id(1_003),
                                    }),
                                    label: "1".into(),
                                    presentation_id: presentation_id(1_002),
                                },
                            },
                            slot_id: WorkbenchPanelSlotId::parse(&id(90_003)).unwrap(),
                        },
                        PanelSlot {
                            node: PanelNode::Panel {
                                child_presentations: Vec::new(),
                                panel_id: panel_id(90_001),
                                primary_presentation: Presentation {
                                    binding: Some(PresentationBinding {
                                        terminal_id: terminal_id(90_007),
                                        worker_id: worker_id(90_006),
                                    }),
                                    label: "1".into(),
                                    presentation_id: presentation_id(90_005),
                                },
                            },
                            slot_id: WorkbenchPanelSlotId::parse(&id(90_004)).unwrap(),
                        },
                    ]),
                    orientation: PanelOrientation::Horizontal,
                    split_id: WorkbenchSplitId::parse(&id(90_002)).unwrap(),
                },
                slot_id: WorkbenchPanelSlotId::parse(&id(3_001)).unwrap(),
            },
            PanelSlot {
                node: PanelNode::Panel {
                    child_presentations: Vec::new(),
                    panel_id: panel_id(3_010),
                    primary_presentation: Presentation {
                        binding: Some(PresentationBinding {
                            terminal_id: terminal_id(3_012),
                            worker_id: worker_id(3_011),
                        }),
                        label: "peer".into(),
                        presentation_id: presentation_id(3_013),
                    },
                },
                slot_id: WorkbenchPanelSlotId::parse(&id(3_014)).unwrap(),
            },
        ]),
        orientation: PanelOrientation::Horizontal,
        split_id: WorkbenchSplitId::parse(&id(3_020)).unwrap(),
    };
    candidate
}

/// The exact committed projection of a Right move of leaf 1001 past leaf 3010: the two
/// leaves are re-grafted under a fresh horizontal split in target-then-moved order.
fn move_grafted_snapshot() -> Snapshot {
    let mut candidate = split_panel_baseline();
    candidate.revision = WorkbenchRevision::new(2);
    candidate.terminal_contexts[0].tabs[0].root = PanelNode::Split {
        children: Box::new([
            PanelSlot {
                node: PanelNode::Panel {
                    child_presentations: Vec::new(),
                    panel_id: panel_id(3_010),
                    primary_presentation: Presentation {
                        binding: Some(PresentationBinding {
                            terminal_id: terminal_id(3_012),
                            worker_id: worker_id(3_011),
                        }),
                        label: "peer".into(),
                        presentation_id: presentation_id(3_013),
                    },
                },
                slot_id: WorkbenchPanelSlotId::parse(&id(90_031)).unwrap(),
            },
            PanelSlot {
                node: PanelNode::Panel {
                    child_presentations: Vec::new(),
                    panel_id: panel_id(1_001),
                    primary_presentation: Presentation {
                        binding: Some(PresentationBinding {
                            terminal_id: terminal_id(1_004),
                            worker_id: worker_id(1_003),
                        }),
                        label: "1".into(),
                        presentation_id: presentation_id(1_002),
                    },
                },
                slot_id: WorkbenchPanelSlotId::parse(&id(90_032)).unwrap(),
            },
        ]),
        orientation: PanelOrientation::Horizontal,
        split_id: WorkbenchSplitId::parse(&id(90_030)).unwrap(),
    };
    candidate
}

/// A superseding truth in which the selected leaf 1001 was closed by a peer while its
/// sibling 3010 survived inside the same Tab.
fn selected_leaf_lost_snapshot(revision: u64) -> Snapshot {
    let mut candidate = split_panel_baseline();
    candidate.revision = WorkbenchRevision::new(revision);
    candidate.terminal_contexts[0].tabs[0].root = PanelNode::Panel {
        child_presentations: Vec::new(),
        panel_id: panel_id(3_010),
        primary_presentation: Presentation {
            binding: Some(PresentationBinding {
                terminal_id: terminal_id(3_012),
                worker_id: worker_id(3_011),
            }),
            label: "peer".into(),
            presentation_id: presentation_id(3_013),
        },
    };
    candidate
}

/// A superseding truth in which the selected leaf 3010 was closed while leaf 1001 survived.
fn second_leaf_lost_snapshot(revision: u64) -> Snapshot {
    let mut candidate = split_panel_baseline();
    candidate.revision = WorkbenchRevision::new(revision);
    candidate.terminal_contexts[0].tabs[0].root = PanelNode::Panel {
        child_presentations: Vec::new(),
        panel_id: panel_id(1_001),
        primary_presentation: Presentation {
            binding: Some(PresentationBinding {
                terminal_id: terminal_id(1_004),
                worker_id: worker_id(1_003),
            }),
            label: "1".into(),
            presentation_id: presentation_id(1_002),
        },
    };
    candidate
}

/// A superseding truth in which Tab 1000 no longer exists.
fn selected_tab_gone_snapshot(revision: u64) -> Snapshot {
    let mut candidate = split_two_tab_baseline();
    candidate.revision = WorkbenchRevision::new(revision);
    candidate.terminal_contexts[0].tabs.remove(0);
    candidate
}

/// The unattached membership fixture: Retryable selected the second Tab, whose removal
/// leaves Tab 1000 as the surviving selection fallback.
fn tab_gone_snapshot_for_retryable() -> Snapshot {
    let mut candidate = snapshot(2, 2, None);
    candidate.terminal_contexts[0].tabs.remove(1);
    candidate
}

fn deadlines() -> OperationDeadlines {
    OperationDeadlines::from_sample(Instant::now()).unwrap()
}

#[test]
fn panel_split_plans_the_selected_identity_latest_geometry_and_direction() {
    for (direction, wire) in [
        (PanelDirection::Left, WorkbenchPanelSplitDirection::Left),
        (PanelDirection::Right, WorkbenchPanelSplitDirection::Right),
        (PanelDirection::Up, WorkbenchPanelSplitDirection::Above),
        (PanelDirection::Down, WorkbenchPanelSplitDirection::Below),
    ] {
        let mut state = ready(1);
        state.observe_geometry(geometry(91));
        let operation = plan(&state, 70_001, PanelRequestKind::Split { direction });
        let active = state.active().unwrap();
        let selected = state.cache().tabs()[active.index()]
            .panel(active.panel_index())
            .unwrap();
        assert_eq!(operation.panel_id(), selected.panel_id());
        assert_eq!(operation.target(), state.desired().unwrap());
        assert_eq!(operation.baseline(), state.cache());
        let PanelIntent::Split {
            direction,
            label,
            geometry: planned_geometry,
        } = operation.intent()
        else {
            panic!("split intent")
        };
        assert_eq!(*direction, wire);
        assert_eq!(label, selected.presentation_label());
        assert_eq!(planned_geometry.columns(), 91);
        assert!(
            state.pending_panel().is_none(),
            "planning alone is not admission"
        );
        assert!(state.invariant_holds());
    }
}

#[test]
fn panel_move_without_a_geometric_neighbor_admits_nothing() {
    let state = ready(1);
    let original_cache = state.cache().clone();
    for direction in [
        PanelDirection::Left,
        PanelDirection::Right,
        PanelDirection::Up,
        PanelDirection::Down,
    ] {
        assert!(
            state
                .plan_panel(
                    WorkbenchRequestId::parse(&id(70_002)).unwrap(),
                    PanelRequestKind::Move { direction },
                    Instant::now() + OPERATION_TOTAL,
                )
                .is_none()
        );
    }
    assert!(state.pending_panel().is_none());
    assert_eq!(state.cache(), &original_cache);
    assert!(state.invariant_holds());
}

#[test]
fn panel_move_plans_the_geometric_neighbor_target() {
    let state = split_ready();
    for direction in [
        PanelDirection::Left,
        PanelDirection::Up,
        PanelDirection::Down,
    ] {
        assert!(
            state
                .plan_panel(
                    WorkbenchRequestId::parse(&id(70_020)).unwrap(),
                    PanelRequestKind::Move { direction },
                    Instant::now() + OPERATION_TOTAL,
                )
                .is_none()
        );
    }
    let operation = plan(
        &state,
        70_021,
        PanelRequestKind::Move {
            direction: PanelDirection::Right,
        },
    );
    let PanelIntent::Move {
        target_panel_id,
        direction,
    } = operation.intent()
    else {
        panic!("move intent")
    };
    assert_eq!(*target_panel_id, panel_id(3_010));
    assert_eq!(*direction, WorkbenchPanelSplitDirection::Right);
    assert_eq!(operation.panel_id(), &panel_id(1_001));
    assert!(state.invariant_holds());
}

#[test]
fn panel_move_plans_in_retryable_phase_against_the_selected_leaf() {
    let state = split_retryable();
    assert_eq!(
        state.phase_kind(),
        SessionPhaseKind::RetryableAttachUnavailable
    );
    let operation = plan(
        &state,
        70_022,
        PanelRequestKind::Move {
            direction: PanelDirection::Left,
        },
    );
    let PanelIntent::Move {
        target_panel_id,
        direction,
    } = operation.intent()
    else {
        panic!("move intent")
    };
    assert_eq!(*target_panel_id, panel_id(1_001));
    assert_eq!(*direction, WorkbenchPanelSplitDirection::Left);
    assert_eq!(operation.panel_id(), &panel_id(3_010));
    assert!(
        state
            .plan_panel(
                WorkbenchRequestId::parse(&id(70_023)).unwrap(),
                PanelRequestKind::Move {
                    direction: PanelDirection::Right,
                },
                Instant::now() + OPERATION_TOTAL,
            )
            .is_none()
    );
    assert!(state.invariant_holds());
}

#[test]
fn panel_admission_excludes_other_structure_until_definite_non_submission() {
    let mut state = ready(2);
    let original_cache = state.cache().clone();
    let original_ready = state.ready().cloned();
    let operation = plan(&state, 70_003, split_kind());
    assert!(state.begin_panel(operation.clone()));
    assert!(
        !state.begin_panel(operation.clone()),
        "cannot admit the same operation twice"
    );
    assert!(state.panel_blocks_structure());
    assert!(!state.start_pending_refresh(1));
    assert_eq!(
        state.plan_selection(ClientAction::SelectNextTab),
        SelectionPlan::Busy
    );
    assert!(
        state
            .plan_panel(
                WorkbenchRequestId::parse(&id(70_004)).unwrap(),
                PanelRequestKind::Close,
                Instant::now() + OPERATION_TOTAL,
            )
            .is_none()
    );
    let prepared = state.prepare_panel(&operation, PanelResult::NotSubmitted);
    assert_eq!(
        state.apply_prepared_panel(prepared),
        PanelDecision::Continue
    );
    assert!(!state.panel_blocks_structure());
    assert!(state.pending_panel().is_none());
    assert_eq!(state.cache(), &original_cache);
    assert_eq!(state.ready(), original_ready.as_ref());
    assert!(state.retained_panel_for_test().is_none());
    assert!(state.invariant_holds());
}

#[test]
fn panel_planning_is_refused_while_any_structure_guard_is_active() {
    // Pending refresh blocks panel planning.
    let mut state = ready(1);
    assert!(state.start_pending_refresh(1));
    assert!(
        state
            .plan_panel(
                WorkbenchRequestId::parse(&id(70_030)).unwrap(),
                split_kind(),
                Instant::now() + OPERATION_TOTAL,
            )
            .is_none()
    );

    // A pending Tab move blocks panel planning (structural mutual exclusion).
    let mut state = ready(2);
    let move_operation = state
        .plan_move(
            workbench_request_id(70_031),
            WorkbenchTabMoveDirection::Next,
            Instant::now() + OPERATION_TOTAL,
        )
        .unwrap();
    assert!(state.begin_move(move_operation));
    assert!(
        state
            .plan_panel(
                WorkbenchRequestId::parse(&id(70_032)).unwrap(),
                split_kind(),
                Instant::now() + OPERATION_TOTAL,
            )
            .is_none()
    );

    // An open rename draft blocks panel planning.
    let mut state = ready(1);
    assert!(state.open_rename_draft().is_some());
    assert!(
        state
            .plan_panel(
                WorkbenchRequestId::parse(&id(70_033)).unwrap(),
                split_kind(),
                Instant::now() + OPERATION_TOTAL,
            )
            .is_none()
    );

    // Switching phase does not allow panel planning.
    let mut state = ready(2);
    let target_key = AttachmentKey::new(2, state.tab(1).unwrap().tab_id().clone());
    let target = state.target_for_switch(1, target_key, deadlines()).unwrap();
    assert!(state.begin_switch(target));
    assert!(
        state
            .plan_panel(
                WorkbenchRequestId::parse(&id(70_034)).unwrap(),
                split_kind(),
                Instant::now() + OPERATION_TOTAL,
            )
            .is_none()
    );

    // Retired phase does not allow panel planning.
    let mut state = ready(1);
    state.retire(CarrierFailure::Closed);
    assert!(
        state
            .plan_panel(
                WorkbenchRequestId::parse(&id(70_035)).unwrap(),
                split_kind(),
                Instant::now() + OPERATION_TOTAL,
            )
            .is_none()
    );

    // A broken invariant refuses planning fail-closed.
    let mut state = ready(1);
    state.attached_key = None;
    assert!(!state.invariant_holds());
    assert!(
        state
            .plan_panel(
                WorkbenchRequestId::parse(&id(70_036)).unwrap(),
                split_kind(),
                Instant::now() + OPERATION_TOTAL,
            )
            .is_none()
    );
}

#[cfg(target_os = "linux")]
#[test]
fn close_status_only_mode_refuses_panel_planning() {
    // Drive a complete close confirmation + accepted result so a close record with a
    // known operation survives retirement, then enter the status-only recovery carrier.
    let mut state = ready(1);
    let now = Instant::now();
    state.open_close_confirmation().unwrap();
    let token = state.view().close_confirmation().unwrap().token();
    state.acknowledge_close_draw(Some(token), None);
    let close = state
        .plan_close_confirmation(workbench_request_id(70_040), now + Duration::from_secs(50))
        .unwrap();
    state.begin_close_exchange(&close, now).unwrap();
    state
        .settle_close_exchange(
            &close,
            &CloseResult::Accepted(WorkbenchTabCloseAccepted {
                close_request_id: workbench_request_id(70_900),
                terminal_context_id: close.key().target().terminal_context_id().clone(),
                tab_id: close.key().target().tab_id().clone(),
                accepted_revision: close.selection().base_revision(),
            }),
        )
        .unwrap();
    assert!(state.finalize_retirement(CarrierFailure::Transport, None));
    assert!(state.begin_close_status_recovery());
    assert!(state.close_status_only());
    assert!(
        state
            .plan_panel(
                WorkbenchRequestId::parse(&id(70_041)).unwrap(),
                split_kind(),
                Instant::now() + OPERATION_TOTAL,
            )
            .is_none()
    );
    assert!(state.invariant_holds());
}

#[test]
fn begin_panel_rejects_operations_that_do_not_match_a_fresh_plan() {
    let mut state = split_ready();
    let operation = plan(&state, 70_050, split_kind());

    // Wrong leaf identity: the fresh plan always carries the selected leaf.
    let forged = PanelOperation::new(
        operation.request_id().clone(),
        operation.target().clone(),
        panel_id(9_999),
        operation.baseline().clone(),
        split_kind(),
        operation.intent().clone(),
        operation.deadline(),
    );
    assert!(!state.begin_panel(forged));

    // Wrong baseline snapshot.
    let forged = PanelOperation::new(
        operation.request_id().clone(),
        operation.target().clone(),
        operation.panel_id().clone(),
        projected(1, 2),
        split_kind(),
        operation.intent().clone(),
        operation.deadline(),
    );
    assert!(!state.begin_panel(forged));

    // An intent that disagrees with the request kind can never be the fresh plan.
    let forged = PanelOperation::new(
        operation.request_id().clone(),
        operation.target().clone(),
        operation.panel_id().clone(),
        operation.baseline().clone(),
        split_kind(),
        PanelIntent::Close,
        operation.deadline(),
    );
    assert!(!state.begin_panel(forged));

    // Wrong target key for the same identity.
    let forged = PanelOperation::new(
        operation.request_id().clone(),
        AttachmentKey::new(9, operation.target().tab_id().clone()),
        operation.panel_id().clone(),
        operation.baseline().clone(),
        split_kind(),
        operation.intent().clone(),
        operation.deadline(),
    );
    assert!(!state.begin_panel(forged));
    assert!(state.begin_panel(operation));
    assert!(state.pending_panel().is_some());
    assert!(state.invariant_holds());
}

#[test]
fn definite_panel_denial_releases_admission_without_inventing_a_commit() {
    let mut state = ready(1);
    let operation = plan(&state, 70_005, split_kind());
    let cache = state.cache().clone();
    let viewer = state.ready().cloned();
    assert!(state.begin_panel(operation.clone()));
    let prepared = state.prepare_panel(&operation, PanelResult::Precommit(PanelPrecommit::Denied));
    assert_eq!(
        state.apply_prepared_panel(prepared),
        PanelDecision::Continue
    );
    assert_eq!(state.cache(), &cache);
    assert_eq!(state.ready(), viewer.as_ref());
    assert!(state.pending_panel().is_none());
    assert!(state.retained_panel_for_test().is_none());
    assert_eq!(state.notice, Some(SessionNotice::PanelNotApplied));
    assert!(state.invariant_holds());
}

#[test]
fn definite_precommit_refusals_release_admission_for_every_reason_class() {
    for result in [
        PanelResult::Precommit(PanelPrecommit::Rejected {
            revision: WorkbenchRevision::new(1),
            reason: WorkbenchRejectionReason::RevisionMismatch,
        }),
        PanelResult::Precommit(PanelPrecommit::Unavailable {
            reason: WorkbenchCommandUnavailableReason::TerminalBusy,
        }),
    ] {
        let mut state = ready(1);
        let operation = plan(&state, 70_060, split_kind());
        assert!(state.begin_panel(operation.clone()));
        let prepared = state.prepare_panel(&operation, result);
        assert_eq!(
            state.apply_prepared_panel(prepared),
            PanelDecision::Continue
        );
        assert!(state.pending_panel().is_none());
        assert_eq!(state.notice, Some(SessionNotice::PanelNotApplied));
        assert!(state.invariant_holds());
    }
}

#[test]
fn unchanged_result_at_the_baseline_revision_releases_without_recovery() {
    let mut state = ready(1);
    let operation = plan(&state, 70_061, split_kind());
    let cache = state.cache().clone();
    assert!(state.begin_panel(operation.clone()));
    let prepared =
        state.prepare_panel(&operation, PanelResult::Unchanged(state.cache().revision()));
    assert_eq!(
        state.apply_prepared_panel(prepared),
        PanelDecision::Continue
    );
    assert_eq!(state.cache(), &cache);
    assert!(state.pending_panel().is_none());
    assert!(state.retained_panel_for_test().is_none());
    assert!(state.panel_recovery_for_test().is_none());
    assert_eq!(state.phase_kind(), SessionPhaseKind::Ready);
    assert!(state.invariant_holds());
}

#[test]
fn unchanged_result_at_a_divergent_revision_retires() {
    let mut state = ready(1);
    let operation = plan(&state, 70_062, split_kind());
    assert!(state.begin_panel(operation.clone()));
    let prepared = state.prepare_panel(
        &operation,
        PanelResult::Unchanged(WorkbenchRevision::new(9)),
    );
    assert_eq!(state.apply_prepared_panel(prepared), PanelDecision::Retire);
    assert_eq!(state.phase_kind(), SessionPhaseKind::Retired);
    assert!(state.retained_panel_for_test().is_none());
    assert!(state.invariant_holds());
}

#[test]
fn committed_split_merges_the_exact_projection_in_place() {
    let mut state = split_ready();
    let operation = plan(&state, 70_070, split_kind());
    let known = pending_split(&operation);
    assert!(state.begin_panel(operation.clone()));
    let prepared = state.prepare_panel(
        &operation,
        PanelResult::Committed {
            commit: Box::new(split_commit(&operation)),
            outcome: WorkbenchSnapshotOutcome::Available {
                snapshot: split_grafted_snapshot(),
            },
        },
    );
    assert_eq!(
        state.apply_prepared_panel(prepared),
        PanelDecision::Continue
    );
    assert_eq!(state.cache().revision(), WorkbenchRevision::new(2));
    assert_eq!(state.cache().tabs()[0].panel_count(), 3);
    assert!(
        state.cache().tabs()[0]
            .panel_index(&panel_id(90_001))
            .is_some()
    );
    assert_eq!(
        state.active(),
        Some(&SelectedTab::new_panel(operation.target().clone(), 0, 0))
    );
    assert_eq!(state.phase_kind(), SessionPhaseKind::Ready);
    assert!(state.pending_panel().is_none());
    assert_eq!(state.retained_panel_for_test(), Some(&known));
    assert!(state.invariant_holds());
}

#[test]
fn committed_move_merges_the_exact_projection_and_retargets_the_selection() {
    let mut state = split_ready();
    let operation = plan(
        &state,
        70_071,
        PanelRequestKind::Move {
            direction: PanelDirection::Right,
        },
    );
    let known = PendingPanelCommit::new(operation.clone(), move_commit(&operation));
    assert!(state.begin_panel(operation.clone()));
    let prepared = state.prepare_panel(
        &operation,
        PanelResult::Committed {
            commit: Box::new(move_commit(&operation)),
            outcome: WorkbenchSnapshotOutcome::Available {
                snapshot: move_grafted_snapshot(),
            },
        },
    );
    assert_eq!(
        state.apply_prepared_panel(prepared),
        PanelDecision::Continue
    );
    assert_eq!(state.cache().revision(), WorkbenchRevision::new(2));
    assert_eq!(state.cache().tabs()[0].panel_count(), 2);
    assert_eq!(
        state.active(),
        Some(&SelectedTab::new_panel(operation.target().clone(), 0, 1)),
        "the selected leaf follows its new geometric position"
    );
    assert_eq!(state.phase_kind(), SessionPhaseKind::Ready);
    assert_eq!(state.retained_panel_for_test(), Some(&known));
    assert!(state.invariant_holds());
}

#[test]
fn committed_split_without_a_projection_enters_recovery_and_keeps_blocking() {
    let mut state = split_ready();
    let operation = plan(&state, 70_072, split_kind());
    let known = pending_split(&operation);
    assert!(state.begin_panel(operation.clone()));
    let prepared = state.prepare_panel(
        &operation,
        PanelResult::Committed {
            commit: Box::new(split_commit(&operation)),
            outcome: WorkbenchSnapshotOutcome::Unavailable {
                reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
            },
        },
    );
    assert_eq!(
        state.apply_prepared_panel(prepared),
        PanelDecision::Continue
    );
    assert_eq!(
        state.phase_kind(),
        SessionPhaseKind::PanelProjectionUnavailable
    );
    assert_eq!(state.panel_recovery_for_test(), Some(&known));
    assert_eq!(state.retained_panel_for_test(), Some(&known));
    assert!(state.pending_panel().is_none());
    assert!(
        state
            .plan_panel(
                WorkbenchRequestId::parse(&id(70_073)).unwrap(),
                split_kind(),
                Instant::now() + OPERATION_TOTAL,
            )
            .is_none(),
        "the unresolved committed fact keeps blocking panel structure"
    );
    assert!(
        state.retain_pending_panel(known.clone()),
        "the known commit stays idempotently retainable during recovery"
    );
    assert!(state.invariant_holds());
}

#[test]
fn committed_split_with_a_divergent_projection_retires_but_keeps_the_known_commit() {
    let mut state = split_ready();
    let operation = plan(&state, 70_074, split_kind());
    let known = pending_split(&operation);
    assert!(state.begin_panel(operation.clone()));
    let prepared = state.prepare_panel(
        &operation,
        PanelResult::Committed {
            commit: Box::new(split_commit(&operation)),
            outcome: WorkbenchSnapshotOutcome::Available {
                snapshot: snapshot(1, 2, None),
            },
        },
    );
    assert_eq!(state.apply_prepared_panel(prepared), PanelDecision::Retire);
    assert_eq!(state.phase_kind(), SessionPhaseKind::Retired);
    assert_eq!(
        state.retained_panel_for_test(),
        Some(&known),
        "a definite committed fact is retained even on divergence"
    );
    assert!(state.invariant_holds());
}

#[test]
fn committed_membership_for_a_removed_tab_walks_cleanup_and_install() {
    let mut state = split_ready_two_tabs();
    let operation = plan(&state, 70_080, split_kind());
    let known = pending_split(&operation);
    let old = state.attached_key().unwrap().clone();
    assert!(state.begin_panel(operation.clone()));
    let prepared = state.prepare_panel(
        &operation,
        PanelResult::Committed {
            commit: Box::new(split_commit(&operation)),
            outcome: WorkbenchSnapshotOutcome::Available {
                snapshot: selected_tab_gone_snapshot(3),
            },
        },
    );
    let (admitted, candidate) = state
        .retain_prepared_panel_membership(prepared)
        .expect("current membership result is retained");
    assert_eq!(&admitted, &operation);
    assert_eq!(state.retained_panel_for_test(), Some(&known));
    let cleanup = state
        .stage_panel_membership_cleanup(operation.request_id(), candidate, deadlines())
        .expect("attached membership stages cleanup");
    assert_eq!(
        cleanup.result(),
        &MembershipResultKey::Panel(operation.request_id().clone())
    );
    assert_eq!(cleanup.old(), &old);
    assert_eq!(state.phase_kind(), SessionPhaseKind::Switching);
    assert!(
        state.panel_blocks_structure(),
        "the pending panel fact survives membership staging"
    );
    assert!(
        state
            .plan_panel(
                WorkbenchRequestId::parse(&id(70_081)).unwrap(),
                split_kind(),
                Instant::now() + OPERATION_TOTAL,
            )
            .is_none()
    );
    let cleaned = state
        .prepare_cleaned_membership_candidate(&cleanup, cleanup.old())
        .expect("cleaned candidate re-prepares");
    let fresh = AttachmentKey::new(
        state.desired().unwrap().generation() + 1,
        cleaned.selected_tab_id().unwrap().clone(),
    );
    let install = state
        .prepare_cleaned_membership_install(&cleanup, cleaned, Some(fresh.clone()))
        .expect("cleaned membership install is prepared");
    let MembershipInstallDecision::Attach(target) = install.commit() else {
        panic!("attached membership installs a fresh attachment")
    };
    assert_eq!(target.selected().key(), &fresh);
    assert_eq!(target.selected().index(), 0);
    assert_eq!(target.selected().panel_index(), 0);
    assert_eq!(state.phase_kind(), SessionPhaseKind::Switching);
    assert_eq!(state.desired(), Some(&fresh));
    assert_eq!(state.attached_key(), None);
    assert!(state.pending_panel().is_none());
    assert_eq!(state.retained_panel_for_test(), Some(&known));
    assert!(state.invariant_holds());
}

#[test]
fn tab_unavailable_membership_installs_without_cleanup_when_unattached() {
    // Input tier: `TabUnavailable` is adapter-producible only for Move intents
    // (`connection/native/panel.rs:152-159`); bound to a Split operation it is a
    // deliberate reducer-defense fixture — no honest adapter returns it this way.
    let mut state = retryable_state(Instant::now());
    let operation = plan(&state, 70_090, split_kind());
    assert!(state.begin_panel(operation.clone()));
    let result = PanelResult::TabUnavailable {
        revision: WorkbenchRevision::new(2),
        outcome: WorkbenchSnapshotOutcome::Available {
            snapshot: tab_gone_snapshot_for_retryable(),
        },
    };
    let prepared = state.prepare_panel(&operation, result.clone());
    let (admitted, candidate) = state
        .retain_prepared_panel_membership(prepared)
        .expect("current membership result is retained");
    assert_eq!(&admitted, &operation);
    let selected_tab = candidate.selected_tab_id().unwrap().clone();
    // The firing guard is the Ready-phase gate (`membership.rs:1067`): this fixture is
    // `RetryableAttachUnavailable`, so staging is refused before the unattached arm
    // (`membership.rs:1071`) is even reached — a Ready+unattached state cannot exist.
    assert!(
        state
            .stage_panel_membership_cleanup(operation.request_id(), candidate, deadlines())
            .is_err(),
        "a non-Ready membership stage must not fabricate a cleanup"
    );
    assert!(state.pending_membership.is_none());
    // Re-prepare an identical membership candidate for the install path; the consumed
    // staging attempt left the pending fact untouched.
    let prepared = state.prepare_panel(&operation, result);
    let (_, candidate) = state
        .retain_prepared_panel_membership(prepared)
        .expect("the identical membership result is retained again");
    let fresh = AttachmentKey::new(state.desired().unwrap().generation() + 1, selected_tab);
    let install = state
        .prepare_panel_membership_install(
            operation.request_id(),
            candidate,
            Some(fresh.clone()),
            Some(deadlines()),
        )
        .expect("unattached membership install is prepared");
    let MembershipInstallDecision::Attach(target) = install.commit() else {
        panic!("unattached membership installs a fresh attachment")
    };
    assert_eq!(target.selected().key(), &fresh);
    assert_eq!(target.selected().index(), 0);
    assert_eq!(target.selected().panel_index(), 0);
    assert_eq!(state.phase_kind(), SessionPhaseKind::Switching);
    assert_eq!(state.desired(), Some(&fresh));
    assert!(state.pending_panel().is_none());
    assert!(state.invariant_holds());
}

#[test]
fn superseding_snapshot_missing_the_selected_leaf_repairs_through_paired_membership() {
    // The repaired contract: a same-Tab selected-leaf loss inside a superseding truth is
    // answered by a paired membership cleanup/install that re-targets the selection,
    // never by a direct merge that fabricates the missing leaf.
    // Input tier: the Split-bound `TabUnavailable` carrier is a deliberate
    // reducer-defense fixture — the adapter produces `TabUnavailable` only for Move
    // (`native/panel.rs:152-159`); the membership path it feeds is real.
    let mut state = split_ready();
    let operation = plan(&state, 70_100, split_kind());
    let old = state.attached_key().unwrap().clone();
    assert!(state.begin_panel(operation.clone()));
    let prepared = state.prepare_panel(
        &operation,
        PanelResult::TabUnavailable {
            revision: WorkbenchRevision::new(2),
            outcome: WorkbenchSnapshotOutcome::Available {
                snapshot: selected_leaf_lost_snapshot(3),
            },
        },
    );
    let (admitted, candidate) = state
        .retain_prepared_panel_membership(prepared)
        .expect("the leaf-loss truth enters the paired membership path");
    assert_eq!(&admitted, &operation);
    assert_eq!(state.notice, Some(SessionNotice::PanelNotApplied));
    let cleanup = state
        .stage_panel_membership_cleanup(operation.request_id(), candidate, deadlines())
        .expect("attached leaf-loss membership stages cleanup");
    assert_eq!(
        cleanup.result(),
        &MembershipResultKey::Panel(operation.request_id().clone())
    );
    assert_eq!(cleanup.old(), &old);
    let cleaned = state
        .prepare_cleaned_membership_candidate(&cleanup, cleanup.old())
        .expect("cleaned candidate re-prepares");
    let fresh = AttachmentKey::new(2, operation.target().tab_id().clone());
    let install = state
        .prepare_cleaned_membership_install(&cleanup, cleaned, Some(fresh.clone()))
        .expect("the repaired leaf installs a fresh attachment");
    let MembershipInstallDecision::Attach(target) = install.commit() else {
        panic!("leaf-loss repair installs a fresh attachment")
    };
    assert_eq!(target.selected().key(), &fresh);
    assert_eq!(target.selected().index(), 0);
    assert_eq!(
        target.selected().panel_index(),
        0,
        "the selection is retargeted to the clamped surviving leaf"
    );
    assert_eq!(state.cache().tabs()[0].panel_count(), 1);
    assert_eq!(
        state.cache().tabs()[0].panel(0).unwrap().panel_id(),
        &panel_id(3_010)
    );
    assert_eq!(state.phase_kind(), SessionPhaseKind::Switching);
    assert_eq!(state.desired(), Some(&fresh));
    assert!(state.pending_panel().is_none());
    assert!(state.invariant_holds());
}

#[test]
fn superseding_leaf_loss_installs_fresh_attachment_without_cleanup_when_unattached() {
    // Input tier: `TabUnavailable` is adapter-producible only for Move intents
    // (`connection/native/panel.rs:152-159`); bound to a Split operation it is a
    // deliberate reducer-defense fixture — no honest adapter returns it this way.
    let mut state = split_retryable();
    assert!(state.attached_key().is_none());
    let operation = plan(
        &state,
        70_110,
        PanelRequestKind::Split {
            direction: PanelDirection::Left,
        },
    );
    assert!(state.begin_panel(operation.clone()));
    let result = PanelResult::TabUnavailable {
        revision: WorkbenchRevision::new(2),
        outcome: WorkbenchSnapshotOutcome::Available {
            snapshot: second_leaf_lost_snapshot(3),
        },
    };
    let prepared = state.prepare_panel(&operation, result.clone());
    let (admitted, candidate) = state
        .retain_prepared_panel_membership(prepared)
        .expect("the unattached leaf-loss truth enters membership");
    assert_eq!(&admitted, &operation);
    // Same guard attribution as above: the `!Ready` phase gate fires in this Retryable
    // fixture; the unattached arm would also refuse, but is not what is exercised.
    assert!(
        state
            .stage_panel_membership_cleanup(operation.request_id(), candidate, deadlines())
            .is_err()
    );
    assert!(state.pending_membership.is_none());
    let prepared = state.prepare_panel(&operation, result);
    let (_, candidate) = state
        .retain_prepared_panel_membership(prepared)
        .expect("the identical membership result is retained again");
    let fresh = AttachmentKey::new(
        state.desired().unwrap().generation() + 1,
        operation.target().tab_id().clone(),
    );
    let install = state
        .prepare_panel_membership_install(
            operation.request_id(),
            candidate,
            Some(fresh.clone()),
            Some(deadlines()),
        )
        .expect("unattached leaf-loss membership installs fresh");
    let MembershipInstallDecision::Attach(target) = install.commit() else {
        panic!("unattached leaf-loss repair installs a fresh attachment")
    };
    assert_eq!(target.selected().key(), &fresh);
    assert_eq!(target.selected().index(), 0);
    assert_eq!(target.selected().panel_index(), 0);
    assert_eq!(state.phase_kind(), SessionPhaseKind::Switching);
    assert_eq!(state.desired(), Some(&fresh));
    assert!(state.invariant_holds());
}

#[test]
fn membership_prepared_panel_consumed_directly_is_a_protocol_violation() {
    // The common consumer must route a Membership result through the paired
    // cleanup/install path; applying it as a plain result fails closed.
    // Input tier: the Split-bound `TabUnavailable` carrier is a deliberate
    // reducer-defense fixture — the adapter produces `TabUnavailable` only for Move
    // (`native/panel.rs:152-159`); the protocol-violation path it feeds is real.
    let mut state = split_ready();
    let operation = plan(&state, 70_120, split_kind());
    assert!(state.begin_panel(operation.clone()));
    let prepared = state.prepare_panel(
        &operation,
        PanelResult::TabUnavailable {
            revision: WorkbenchRevision::new(2),
            outcome: WorkbenchSnapshotOutcome::Available {
                snapshot: selected_leaf_lost_snapshot(3),
            },
        },
    );
    assert!(prepared.membership_candidate().is_some());
    assert_eq!(state.apply_prepared_panel(prepared), PanelDecision::Retire);
    assert_eq!(state.phase_kind(), SessionPhaseKind::Retired);
    assert!(state.invariant_holds());
}

#[test]
fn tab_unavailable_with_a_surviving_leaf_truth_merges_the_superseding_projection() {
    // The defensive fixture presents a refusal accompanied by a newer truth that still
    // contains the selected leaf: the core merges it in place and marks the panel not
    // applied. Input tier: the Split-bound `TabUnavailable` carrier is a deliberate
    // reducer-defense fixture — the adapter produces `TabUnavailable` only for Move
    // (`native/panel.rs:152-159`); the merge path it feeds is real.
    let mut state = split_ready();
    let operation = plan(&state, 70_130, split_kind());
    assert!(state.begin_panel(operation.clone()));
    let mut truth = split_panel_baseline();
    truth.revision = WorkbenchRevision::new(3);
    truth.terminal_contexts[0].tabs[0].label = "peer renamed".into();
    let prepared = state.prepare_panel(
        &operation,
        PanelResult::TabUnavailable {
            revision: WorkbenchRevision::new(2),
            outcome: WorkbenchSnapshotOutcome::Available { snapshot: truth },
        },
    );
    assert_eq!(
        state.apply_prepared_panel(prepared),
        PanelDecision::Continue
    );
    assert_eq!(state.cache().revision(), WorkbenchRevision::new(3));
    assert_eq!(state.cache().tabs()[0].tab_label().as_str(), "peer renamed");
    assert_eq!(state.notice, Some(SessionNotice::PanelNotApplied));
    assert_eq!(state.phase_kind(), SessionPhaseKind::Ready);
    assert!(state.pending_panel().is_none());
    assert!(state.invariant_holds());
}

#[test]
fn panel_close_acceptance_is_a_receipt_not_synchronous_leaf_deletion() {
    let mut state = ready(1);
    let operation = plan(&state, 70_006, PanelRequestKind::Close);
    let cache = state.cache().clone();
    let viewer = state.ready().cloned();
    let accepted = WorkbenchPanelCloseAccepted {
        close_request_id: WorkbenchRequestId::parse(&id(70_007)).unwrap(),
        terminal_context_id: operation.baseline().terminal_context_id().clone(),
        tab_id: operation.target().tab_id().clone(),
        panel_id: operation.panel_id().clone(),
        accepted_revision: operation.baseline().revision(),
    };
    assert!(state.begin_panel(operation.clone()));
    let prepared = state.prepare_panel(&operation, PanelResult::CloseAccepted(accepted));
    assert_eq!(
        state.apply_prepared_panel(prepared),
        PanelDecision::Continue
    );
    assert_eq!(
        state.cache(),
        &cache,
        "receipt does not delete the leaf or Tab"
    );
    assert_eq!(
        state.ready(),
        viewer.as_ref(),
        "receipt does not detach the viewer"
    );
    assert!(state.pending_panel().is_none());
    assert!(
        state.retained_panel_for_test().is_none(),
        "a close receipt is not a split/move commit"
    );
    assert!(
        state.publication_debt_pending(),
        "an accepted close receipt owes a refresh drain to observe the removal"
    );
    assert!(state.invariant_holds());
}

#[test]
fn close_acceptance_with_mismatched_identity_or_revision_diverges() {
    for (panel, revision) in [
        (panel_id(9_999), 1), // wrong leaf identity
        (panel_id(1_001), 2), // revision beyond the baseline
    ] {
        let mut state = ready(1);
        let operation = plan(&state, 70_140, PanelRequestKind::Close);
        assert!(state.begin_panel(operation.clone()));
        let accepted = WorkbenchPanelCloseAccepted {
            close_request_id: WorkbenchRequestId::parse(&id(70_140)).unwrap(),
            terminal_context_id: operation.baseline().terminal_context_id().clone(),
            tab_id: operation.target().tab_id().clone(),
            panel_id: panel,
            accepted_revision: WorkbenchRevision::new(revision),
        };
        let prepared = state.prepare_panel(&operation, PanelResult::CloseAccepted(accepted));
        assert_eq!(state.apply_prepared_panel(prepared), PanelDecision::Retire);
        assert_eq!(state.phase_kind(), SessionPhaseKind::Retired);
        assert!(state.retained_panel_for_test().is_none());
        assert!(state.invariant_holds());
    }
}

#[test]
fn stale_prepared_panel_is_rejected_by_the_core_without_fabricating_success() {
    // Deliberately faulty consumer input, not an honest Server operation trace.
    // The mailbox must reject a stale reply before calling this consumer; the
    // core's second line of defence retires on a non-current operation.
    let mut state = ready(1);
    let current = plan(&state, 70_008, split_kind());
    let stale = plan(&state, 70_009, split_kind());
    assert!(state.begin_panel(current));
    let cache = state.cache().clone();
    let prepared = state.prepare_panel(&stale, PanelResult::NotSubmitted);
    assert_eq!(state.apply_prepared_panel(prepared), PanelDecision::Retire);
    assert_eq!(state.phase_kind(), SessionPhaseKind::Retired);
    assert_eq!(state.cache(), &cache);
    assert!(state.retained_panel_for_test().is_none());
    assert!(state.invariant_holds());
}

#[test]
fn an_exact_pending_panel_commit_remains_known_after_transport_retirement() {
    let mut state = ready(1);
    let operation = plan(&state, 70_010, split_kind());
    let known = pending_split(&operation);
    assert!(state.begin_panel(operation.clone()));
    assert!(state.retain_pending_panel(known.clone()));
    assert_eq!(state.retained_panel_for_test(), Some(&known));
    state.retire(CarrierFailure::Closed);
    assert_eq!(state.retained_panel_for_test(), Some(&known));
    assert!(
        state.retain_pending_panel(known.clone()),
        "the identical known fact is idempotent"
    );
    assert!(state.invariant_holds());
}

#[test]
fn retain_pending_panel_rejects_foreign_or_invalid_facts() {
    let mut state = split_ready();
    let operation = plan(&state, 70_150, split_kind());
    let known = pending_split(&operation);
    assert!(state.begin_panel(operation.clone()));

    // A commit whose revision does not follow the baseline is invalid.
    let invalid = PendingPanelCommit::new(
        operation.clone(),
        PanelCommit::Split(WorkbenchPanelSplitCommit::Pending {
            revision: WorkbenchRevision::new(9),
            terminal_context_id: operation.baseline().terminal_context_id().clone(),
            tab_id: operation.target().tab_id().clone(),
            panel_id: operation.panel_id().clone(),
            new_panel_id: WorkbenchPanelId::parse(&id(90_001)).unwrap(),
            split_id: WorkbenchSplitId::parse(&id(90_002)).unwrap(),
            first_slot_id: WorkbenchPanelSlotId::parse(&id(90_003)).unwrap(),
            second_slot_id: WorkbenchPanelSlotId::parse(&id(90_004)).unwrap(),
            presentation_id: TerminalPresentationId::parse(&id(90_005)).unwrap(),
        }),
    );
    assert!(!state.retain_pending_panel(invalid));

    // A valid commit for an operation the core never admitted is foreign.
    let foreign_operation = PanelOperation::new(
        WorkbenchRequestId::parse(&id(70_151)).unwrap(),
        operation.target().clone(),
        operation.panel_id().clone(),
        operation.baseline().clone(),
        split_kind(),
        operation.intent().clone(),
        operation.deadline(),
    );
    let foreign = pending_split(&foreign_operation);
    assert!(!state.retain_pending_panel(foreign));

    assert!(state.retain_pending_panel(known.clone()));
    assert_eq!(state.retained_panel_for_test(), Some(&known));
    assert!(state.invariant_holds());
}

#[test]
fn stale_baseline_move_receives_reachable_revision_mismatch_refusal() {
    // Reachable positive control for "a peer raced my baseline", constructed per the
    // proven server control flow (R7 report-server-panel-outcomes-18 `955a72ec`
    // §3/§7.1): the client holds a baseline containing the target; a legal commit by
    // another client advances the authority to a newer revision; the original Move is
    // then answered `Rejected{RevisionMismatch, revision: current}` at `begin` —
    // before admission — never `TabUnavailable`. This is a reducer-level injection of
    // the production-reachable `PanelResult::Precommit` shape; no network or real
    // server ran here, and R7's parallel witnesses provide the server-side binding.
    let mut state = split_ready_two_tabs();
    let operation = plan(&state, 70_166, move_kind());
    let cache = state.cache().clone();
    assert!(state.begin_panel(operation.clone()));
    let prepared = state.prepare_panel(
        &operation,
        PanelResult::Precommit(PanelPrecommit::Rejected {
            revision: WorkbenchRevision::new(5),
            reason: WorkbenchRejectionReason::RevisionMismatch,
        }),
    );
    assert_eq!(
        state.apply_prepared_panel(prepared),
        PanelDecision::Continue
    );
    assert_eq!(state.notice, Some(SessionNotice::PanelNotApplied));
    assert_eq!(state.phase_kind(), SessionPhaseKind::Ready);
    assert!(state.pending_panel().is_none());
    assert_eq!(state.cache(), &cache);
    assert!(state.retained_panel_for_test().is_none());
    assert!(state.invariant_holds());
}

#[test]
fn tab_unavailable_sub_arms_cover_only_defensive_tiers() {
    // Reachability tiers (R7 server-order review `955a72ec` §7.1/§7.6):
    // `TabUnavailable` is producible on the wire ONLY for Move intents
    // (`connection/native/panel.rs:152-159`), and a real `Rejected{TabUnavailable}`
    // always carries `revision == expected == baseline` — the server checks
    // `expected != current` FIRST (`terminal_panel_move.rs:224-225`) and answers
    // `Rejected{RevisionMismatch, revision: current}` before `plan()` could emit
    // anything else (`workbench_authority.rs:446-450`). Even `revision == baseline`
    // requires degenerate input: the client's own baseline would already lack the Tab,
    // which `plan_panel` could never have admitted. Every arm below is therefore a
    // defensive input, tiered by how close it sits to the adapter-constructible
    // boundary; none is a normal server history. The reachable positive control for
    // "a peer raced my baseline" lives in
    // `stale_baseline_move_receives_reachable_revision_mismatch_refusal`.

    // Defensive — adapter-constructible shape carrying an impossible authority claim:
    // no honest authority reports the Tab unavailable at a revision older than the
    // baseline we already hold.
    let mut state = split_ready_at_revision(2);
    let operation = plan(&state, 70_160, move_kind());
    assert!(state.begin_panel(operation.clone()));
    let prepared = state.prepare_panel(
        &operation,
        PanelResult::TabUnavailable {
            revision: WorkbenchRevision::new(1),
            outcome: WorkbenchSnapshotOutcome::Unavailable {
                reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
            },
        },
    );
    assert_eq!(state.apply_prepared_panel(prepared), PanelDecision::Retire);
    assert_eq!(state.phase_kind(), SessionPhaseKind::Retired);
    assert_eq!(
        state.retired_failure(),
        Some(CarrierFailure::AuthorityDivergence)
    );
    assert!(state.retained_panel_for_test().is_none());
    assert!(state.invariant_holds());

    // Defensive — adapter-constructible shape with server-impossible field values:
    // `revision: 5` above baseline 1 can never be emitted for this reason, since
    // `RevisionMismatch` intercepts the request first. The reducer's
    // rev ≥ baseline + non-Available → NotApplied leniency is real and stays pinned;
    // the input itself is not a reachable history.
    let mut state = split_ready();
    let operation = plan(&state, 70_161, move_kind());
    let cache = state.cache().clone();
    assert!(state.begin_panel(operation.clone()));
    let prepared = state.prepare_panel(
        &operation,
        PanelResult::TabUnavailable {
            revision: WorkbenchRevision::new(5),
            outcome: WorkbenchSnapshotOutcome::Unavailable {
                reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
            },
        },
    );
    assert_eq!(
        state.apply_prepared_panel(prepared),
        PanelDecision::Continue
    );
    assert_eq!(state.notice, Some(SessionNotice::PanelNotApplied));
    assert_eq!(state.phase_kind(), SessionPhaseKind::Ready);
    assert!(state.pending_panel().is_none());
    assert_eq!(state.cache(), &cache);
    assert!(state.retained_panel_for_test().is_none());
    assert!(state.invariant_holds());

    // Defensive leniency boundary — the only revision shape a real server could emit
    // (`revision == baseline`), and even then only under degenerate input: the
    // baseline still visibly contains the Tab, which no well-formed client's admitted
    // operation allows. The reducer tolerates this as NotApplied; recorded so the
    // leniency boundary stays pinned, not as a reachable-history example.
    let mut state = split_ready();
    let operation = plan(&state, 70_164, move_kind());
    let cache = state.cache().clone();
    assert!(state.begin_panel(operation.clone()));
    let prepared = state.prepare_panel(
        &operation,
        PanelResult::TabUnavailable {
            revision: WorkbenchRevision::new(1),
            outcome: WorkbenchSnapshotOutcome::Unavailable {
                reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
            },
        },
    );
    assert_eq!(
        state.apply_prepared_panel(prepared),
        PanelDecision::Continue
    );
    assert_eq!(state.notice, Some(SessionNotice::PanelNotApplied));
    assert_eq!(state.cache(), &cache);
    assert!(state.invariant_holds());

    // Defensive — adapter-constructible shape, self-contradictory evidence (and the
    // claimed revision 2 > baseline 1 is server-impossible on its own): a Move refusal
    // at rev 2 while the authority's own served snapshot at rev 2 still contains the
    // Tab — the evidence disproves the claim. The fail-closed edge of the check.
    let mut state = split_ready();
    let operation = plan(&state, 70_162, move_kind());
    assert!(state.begin_panel(operation.clone()));
    let mut still_present = split_panel_baseline();
    still_present.revision = WorkbenchRevision::new(2);
    let prepared = state.prepare_panel(
        &operation,
        PanelResult::TabUnavailable {
            revision: WorkbenchRevision::new(2),
            outcome: WorkbenchSnapshotOutcome::Available {
                snapshot: still_present,
            },
        },
    );
    assert_eq!(state.apply_prepared_panel(prepared), PanelDecision::Retire);
    assert_eq!(state.phase_kind(), SessionPhaseKind::Retired);
    assert_eq!(
        state.retired_failure(),
        Some(CarrierFailure::AuthorityDivergence)
    );
    assert!(state.retained_panel_for_test().is_none());
    assert!(state.invariant_holds());

    // Defensive — adapter-constructible shape, unverifiable (and rev 3 > baseline 1 is
    // server-impossible): refusal whose snapshot evidence only reaches rev 2 — the
    // evidence is older than the claim.
    let mut state = split_ready();
    let operation = plan(&state, 70_163, move_kind());
    assert!(state.begin_panel(operation.clone()));
    let prepared = state.prepare_panel(
        &operation,
        PanelResult::TabUnavailable {
            revision: WorkbenchRevision::new(3),
            outcome: WorkbenchSnapshotOutcome::Available {
                snapshot: selected_leaf_lost_snapshot(2),
            },
        },
    );
    assert_eq!(state.apply_prepared_panel(prepared), PanelDecision::Retire);
    assert_eq!(state.phase_kind(), SessionPhaseKind::Retired);
    assert_eq!(
        state.retired_failure(),
        Some(CarrierFailure::AuthorityDivergence)
    );
    assert!(state.retained_panel_for_test().is_none());
    assert!(state.invariant_holds());

    // Deliberately adapter-unreachable forge: `TabUnavailable` bound to a Split
    // operation cannot be produced by any honest wire peer (`native/panel.rs:81-83`
    // unconditionally maps Split `DefinitePrecommit` to `Precommit`). Retained to pin
    // that the reducer's TabUnavailable classification is intent-agnostic
    // (`prepare_panel` does not dispatch on the operation kind) — a forged
    // kind-mismatched result is tolerated with the same leniency as the other
    // defensive arms.
    let mut state = split_ready();
    let operation = plan(&state, 70_165, split_kind());
    let cache = state.cache().clone();
    assert!(state.begin_panel(operation.clone()));
    let prepared = state.prepare_panel(
        &operation,
        PanelResult::TabUnavailable {
            revision: WorkbenchRevision::new(5),
            outcome: WorkbenchSnapshotOutcome::Unavailable {
                reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
            },
        },
    );
    assert_eq!(
        state.apply_prepared_panel(prepared),
        PanelDecision::Continue
    );
    assert_eq!(state.notice, Some(SessionNotice::PanelNotApplied));
    assert_eq!(state.cache(), &cache);
    assert!(state.invariant_holds());
}

#[test]
fn committed_with_an_invalid_commit_retires_without_retaining_the_fact() {
    // `PendingPanelCommit::valid()` is the apply-level gate for committed results: a
    // commit that does not follow the baseline exactly is divergent with no pending
    // fact — unlike the valid-commit-divergent arm above, nothing may be retained.
    for invalid in [
        // The committed revision skips a step: baseline + 2 is not the successor.
        (|operation: &PanelOperation| {
            let mut commit = split_commit(operation);
            let PanelCommit::Split(WorkbenchPanelSplitCommit::Pending { revision, .. }) =
                &mut commit
            else {
                unreachable!()
            };
            *revision = WorkbenchRevision::new(revision.value() + 1);
            commit
        }) as fn(&PanelOperation) -> PanelCommit,
        // A foreign panel identity: the commit names a leaf the operation never named.
        (|operation: &PanelOperation| {
            let mut commit = split_commit(operation);
            let PanelCommit::Split(WorkbenchPanelSplitCommit::Pending {
                panel_id: commit_panel,
                ..
            }) = &mut commit
            else {
                unreachable!()
            };
            *commit_panel = panel_id(9_999);
            commit
        }) as fn(&PanelOperation) -> PanelCommit,
    ] {
        let mut state = split_ready();
        let operation = plan(&state, 70_170, split_kind());
        assert!(state.begin_panel(operation.clone()));
        let prepared = state.prepare_panel(
            &operation,
            PanelResult::Committed {
                commit: Box::new(invalid(&operation)),
                outcome: WorkbenchSnapshotOutcome::Unavailable {
                    reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
                },
            },
        );
        assert_eq!(state.apply_prepared_panel(prepared), PanelDecision::Retire);
        assert_eq!(state.phase_kind(), SessionPhaseKind::Retired);
        assert_eq!(
            state.retired_failure(),
            Some(CarrierFailure::AuthorityDivergence)
        );
        assert!(
            state.retained_panel_for_test().is_none(),
            "an invalid commit is never retained as a known fact"
        );
        assert!(state.invariant_holds());
    }
}

#[test]
fn stale_membership_result_is_refused_by_the_second_line_of_defence() {
    // Carrier (a) is a genuinely reachable history: the operation's own legal
    // `Committed` (a Split commits at baseline+1) and the adapter's post-commit
    // `read_panel_snapshot` legitimately reflects a peer's intervening close — a
    // superseding truth in which the selected Tab is gone. That is exactly the
    // membership carrier the honest pipeline can deliver (see also
    // `committed_membership_for_a_removed_tab_walks_cleanup_and_install`).
    //   (b) the release step is white-box: no honest server sends a second result for
    //       the same operation — it stands in for the consumer freeing the operation
    //       after the membership candidate was handed off;
    //   (c) the re-presented stale carrier is a deliberately forged reducer-defense
    //       input: no honest server returns two results for one operation.
    // `retain_prepared_panel_membership` is the second line of defence (after the
    // mailbox's stale-reply filter): a membership-prepared result whose operation is no
    // longer current must be refused with Protocol before any cleanup/install is staged.
    let mut state = split_ready_two_tabs();
    let operation = plan(&state, 70_180, split_kind());
    let known = pending_split(&operation);
    assert!(state.begin_panel(operation.clone()));
    let result = PanelResult::Committed {
        commit: Box::new(split_commit(&operation)),
        outcome: WorkbenchSnapshotOutcome::Available {
            snapshot: selected_tab_gone_snapshot(3),
        },
    };
    // Positive control: while the operation is current, the reachable (a) result is
    // genuinely classified as membership and retained — the refusal below is not
    // vacuous, and the committed fact is recorded as the known outcome.
    let prepared = state.prepare_panel(&operation, result.clone());
    assert!(prepared.membership_candidate().is_some());
    let (admitted, _candidate) = state
        .retain_prepared_panel_membership(prepared)
        .expect("the current membership result retains");
    assert_eq!(&admitted, &operation);
    assert_eq!(state.retained_panel_for_test(), Some(&known));
    // The white-box (b) step releases the pending operation through the ordinary
    // consumer, leaving a second prepared carrier for the same operation stale.
    let release = state.prepare_panel(&operation, PanelResult::NotSubmitted);
    assert_eq!(state.apply_prepared_panel(release), PanelDecision::Continue);
    assert!(state.pending_panel().is_none());
    // The forged (c) carrier: same content as the once-retained (a), re-presented after
    // release — refused by the currentness check, not by content validation.
    let stale = state.prepare_panel(&operation, result);
    assert!(stale.membership_candidate().is_some());
    assert!(matches!(
        state.retain_prepared_panel_membership(stale),
        Err(CarrierFailure::Protocol)
    ));
    assert_eq!(state.notice, Some(SessionNotice::PanelNotApplied));
    assert!(state.pending_membership.is_none());
    // The known committed fact survives both the release and the forged re-present.
    assert_eq!(state.retained_panel_for_test(), Some(&known));
    assert!(state.invariant_holds());
}

#[test]
fn selected_without_attachment_phase_admits_panel_planning() {
    // `SelectedWithoutAttachment` is an allowed planning phase: the selection exists,
    // the desired key is armed, and no attachment is held — planning must still target
    // the selected leaf, and the admitted operation begins normally.
    let mut state = selected_without_attachment_state();
    assert_eq!(state.attached_key(), None);
    let selected = state.active().unwrap().clone();
    assert_eq!(selected.index(), 1, "the created tab is selected");
    let selected_panel = state.cache().tabs()[selected.index()]
        .panel(selected.panel_index())
        .unwrap()
        .panel_id()
        .clone();
    let operation = plan(&state, 70_190, split_kind());
    assert_eq!(operation.panel_id(), &selected_panel);
    assert_eq!(operation.target(), state.desired().unwrap());
    assert!(state.begin_panel(operation));
    assert!(state.pending_panel().is_some());
    assert!(state.invariant_holds());
}
