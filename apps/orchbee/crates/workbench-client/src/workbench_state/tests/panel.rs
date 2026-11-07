//! Deterministic panel witnesses for the canonical core: admission guards, exact result
//! handling, and the repaired same-Tab selected-leaf-loss membership path.
use super::*;
use crate::client_view::PanelDirection;
use crate::local_client::workbench::WorkbenchTabCloseAccepted;
use crate::local_client::workbench::panel_close::WorkbenchPanelCloseAccepted;
use crate::local_client::workbench::panel_placement::WorkbenchPanelPlacementCommit;
use crate::local_client::workbench::panel_split::WorkbenchPanelSplitCommit;
use crate::tui_presentation::GeometrySettlement;
use crate::workbench_state::panel::{
    PanelCommit, PanelDecision, PanelIntent, PanelOperation, PanelPrecommit, PanelRequestKind,
    PanelResult, PendingPanelCommit,
};
use crate::workbench_state::panel_close::PanelCloseStateError;
use crate::workbench_state::{CloseOperationKind, CloseReadMode, CloseResult, CloseStatusKind};
use orchbee_workbench_protocol::{
    WorkbenchCommandUnavailableReason, WorkbenchPanelSplitDirection, WorkbenchRejectionReason,
    WorkbenchSnapshotOutcome, WorkbenchSnapshotUnavailableReason,
    WorkbenchTabCloseAttemptGeneration, WorkbenchTabCloseBlockedReason, WorkbenchTabCloseStatus,
    WorkbenchTabMoveDirection,
};

fn plan(state: &WorkbenchClientState, request: u128, kind: PanelRequestKind) -> PanelOperation {
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

fn split_ready() -> WorkbenchClientState {
    ready_with_cache(project_preferred_workspace(&split_panel_baseline()).unwrap())
}

fn expired_refresh_pane(now: Instant) -> (WorkbenchClientState, WorkbenchPanelId) {
    let mut state = split_ready();
    let leaves: Vec<_> = state
        .tab(0)
        .unwrap()
        .leaf_panel_ids()
        .into_iter()
        .enumerate()
        .map(|(index, panel)| (panel, index, geometry(80)))
        .collect();
    let peer = leaves[1].0.clone();
    assert!(state.sync_pane_set(&leaves, now));
    assert!(state.reconcile_targets(now + OPERATION_TOTAL).is_none());
    assert_eq!(
        state.pane_by_panel(&peer).unwrap().fault,
        Some(PaneFault::ConvergenceExpired)
    );
    assert!(state.invariant_holds());
    (state, peer)
}

fn refresh_same_panels(
    state: &WorkbenchClientState,
    generation: u64,
    revision: u64,
) -> PreparedRefresh {
    let mut candidate = split_panel_baseline();
    candidate.revision = WorkbenchRevision::new(revision);
    state.prepare_refresh(
        generation,
        WorkbenchSnapshotOutcome::Available {
            snapshot: candidate,
        },
    )
}

#[test]
fn explicit_refresh_install_reopens_expired_pane_without_membership_change() {
    let now = Instant::now();
    let installed_at = now + OPERATION_TOTAL + Duration::from_millis(1);
    let (mut state, peer) = expired_refresh_pane(now);
    let focus = state.desired().unwrap().clone();
    let before = state.pane_record(&focus).unwrap().clone();
    assert!(state.start_pending_refresh(41));
    assert_eq!(
        state.pane_by_panel(&peer).unwrap().fault,
        Some(PaneFault::ConvergenceExpired)
    );
    let prepared = refresh_same_panels(&state, 41, 1);
    assert!(matches!(prepared, PreparedRefresh::Append { .. }));
    assert_eq!(
        state.apply_prepared_refresh(prepared, installed_at),
        RefreshDecision::Continue
    );
    assert_eq!(state.pane_by_panel(&peer).unwrap().fault, None);
    let after = state.pane_record(&focus).unwrap();
    assert_eq!(before.pane_ref(), after.pane_ref());
    assert_eq!(before.mode(), after.mode());
    assert_eq!(before.accepted_frame(), after.accepted_frame());
    assert_eq!(
        before.last_accepted_geometry(),
        after.last_accepted_geometry()
    );
    assert!(state.pending_refresh_generation().is_none());
    assert!(state.invariant_holds());
}

#[test]
fn failed_and_automatic_refreshes_keep_expired_panes_parked() {
    let now = Instant::now();
    let installed_at = now + OPERATION_TOTAL + Duration::from_millis(1);
    let (mut state, peer) = expired_refresh_pane(now);
    assert!(state.start_pending_refresh(41));
    let failed = state.prepare_refresh(41, WorkbenchSnapshotOutcome::Denied {});
    assert_eq!(
        state.apply_prepared_refresh(failed, installed_at),
        RefreshDecision::Continue
    );
    assert_eq!(
        state.pane_by_panel(&peer).unwrap().fault,
        Some(PaneFault::ConvergenceExpired)
    );
    state.note_publication_hint();
    assert!(state.start_publication_drain(42));
    let automatic = refresh_same_panels(&state, 42, 2);
    assert_eq!(
        state.apply_prepared_refresh(automatic, installed_at),
        RefreshDecision::Continue
    );
    assert_eq!(
        state.pane_by_panel(&peer).unwrap().fault,
        Some(PaneFault::ConvergenceExpired)
    );
    assert!(state.invariant_holds());
}

#[test]
fn explicit_refresh_result_reopens_once_and_live_episode_is_not_extended() {
    let now = Instant::now();
    let installed_at = now + OPERATION_TOTAL + Duration::from_millis(1);
    let (mut state, peer) = expired_refresh_pane(now);
    assert!(state.start_pending_refresh(41));
    let first = refresh_same_panels(&state, 41, 2);
    let duplicate = refresh_same_panels(&state, 41, 2);
    assert_eq!(
        state.apply_prepared_refresh(first, installed_at),
        RefreshDecision::Continue
    );
    let (_, _, budget) = state.reconcile_targets(installed_at).unwrap();
    assert_eq!(budget.total(), installed_at + OPERATION_TOTAL);

    // Another genuine successful user refresh is not an extension of live work.
    assert!(state.start_pending_refresh(42));
    let fresh = refresh_same_panels(&state, 42, 3);
    assert_eq!(
        state.apply_prepared_refresh(fresh, installed_at + Duration::from_secs(1)),
        RefreshDecision::Continue
    );
    assert_eq!(
        state.reconcile_targets(installed_at).unwrap().2.total(),
        budget.total()
    );

    assert!(state.reconcile_targets(budget.total()).is_none());
    assert_eq!(
        state.pane_by_panel(&peer).unwrap().fault,
        Some(PaneFault::ConvergenceExpired)
    );
    assert_eq!(
        state.apply_prepared_refresh(duplicate, budget.total() + Duration::from_secs(1)),
        RefreshDecision::Continue
    );
    assert_eq!(
        state.pane_by_panel(&peer).unwrap().fault,
        Some(PaneFault::ConvergenceExpired)
    );
    assert!(state.invariant_holds());
}

#[test]
fn stale_user_result_cannot_reopen_an_automatic_flight_or_retired_client() {
    let now = Instant::now();
    let installed_at = now + OPERATION_TOTAL + Duration::from_millis(1);
    let (mut state, peer) = expired_refresh_pane(now);
    assert!(state.start_pending_refresh(41));
    let late_success = refresh_same_panels(&state, 41, 2);
    let failed = state.prepare_refresh(41, WorkbenchSnapshotOutcome::Denied {});
    assert_eq!(
        state.apply_prepared_refresh(failed, installed_at),
        RefreshDecision::Continue
    );
    state.note_publication_hint();
    assert!(state.start_publication_drain(42));
    assert_eq!(
        state.apply_prepared_refresh(late_success, installed_at),
        RefreshDecision::Continue
    );
    assert_eq!(state.pending_refresh_generation(), Some(42));
    assert_eq!(
        state.pane_by_panel(&peer).unwrap().fault,
        Some(PaneFault::ConvergenceExpired)
    );
    let automatic = refresh_same_panels(&state, 42, 2);
    state.retire(CarrierFailure::Transport);
    assert_eq!(
        state.apply_prepared_refresh(automatic, installed_at),
        RefreshDecision::Continue
    );
    assert!(state.pending_refresh_generation().is_none());
    assert_eq!(state.phase_kind(), ClientPhaseKind::Retired);
    assert!(state.invariant_holds());
}

#[test]
fn user_refresh_preserves_definite_faults_and_departing_viewer_ownership() {
    let now = Instant::now();
    let installed_at = now + OPERATION_TOTAL + Duration::from_millis(1);
    for fault in [
        PaneFault::AttachRejected,
        PaneFault::CapacityExceeded,
        PaneFault::ViewerLost,
        PaneFault::ControlLost,
    ] {
        let (mut state, peer) = expired_refresh_pane(now);
        let reference = state.pane_by_panel(&peer).unwrap().pane_ref();
        // Loss is a result about an existing attachment, unlike an attach
        // rejection. Establish that real precondition through the setup path.
        if matches!(fault, PaneFault::ViewerLost | PaneFault::ControlLost) {
            state
                .panes
                .iter_mut()
                .find(|record| record.key.panel() == &peer)
                .unwrap()
                .fault = None;
            assert!(state.settle_attached_event_t(&reference).is_some());
            assert!(state.record_setup_configuration_t(
                &reference,
                controlling(),
                Some(geometry(80))
            ));
        }
        assert!(state.settle_pane_fault(&reference, fault));
        assert!(
            state.invariant_holds(),
            "fault {fault:?} must preserve the record shape before refresh"
        );
        assert!(state.reconcile_targets(installed_at).is_none());
        assert_eq!(
            state.pane_by_panel(&peer).unwrap().fault,
            Some(fault),
            "a fair tick cannot turn a known fault into an expired attach"
        );
        assert!(state.start_pending_refresh(41));
        let prepared = refresh_same_panels(&state, 41, 2);
        assert_eq!(
            state.apply_prepared_refresh(prepared, installed_at),
            RefreshDecision::Continue
        );
        assert_eq!(state.pane_by_panel(&peer).unwrap().fault, Some(fault));
        assert!(state.invariant_holds());
    }

    let (mut state, peer) = expired_refresh_pane(now);
    let reference = state.pane_by_panel(&peer).unwrap().pane_ref();
    state
        .panes
        .iter_mut()
        .find(|record| record.key.panel() == &peer)
        .unwrap()
        .fault = None;
    assert!(state.settle_attached_event_t(&reference).is_some());
    assert!(state.record_setup_configuration_t(&reference, controlling(), Some(geometry(80))));
    // No disposal proof exists: an in-flight departure is never a free slot.
    let record = state
        .panes
        .iter_mut()
        .find(|record| record.key.panel() == &peer)
        .unwrap();
    record.mark_departing();
    assert!(state.start_pending_refresh(41));
    let prepared = refresh_same_panels(&state, 41, 2);
    assert_eq!(
        state.apply_prepared_refresh(prepared, installed_at),
        RefreshDecision::Continue
    );
    let record = state.pane_record(reference.pane()).unwrap();
    assert_eq!(record.pane_ref(), reference);
    assert!(record.attached() && record.departing);
    assert_eq!(record.fault, None);
    assert!(state.invariant_holds());
}

fn split_two_tab_baseline() -> Snapshot {
    let mut baseline = snapshot(2, 1, None);
    baseline.terminal_contexts[0].tabs[0].root = PanelNode::Split {
        ratio: Default::default(),
        children: Box::new([
            PanelSlot {
                node: PanelNode::Panel {
                    right_click: Default::default(),
                    child_presentations: Vec::new(),
                    panel_id: panel_id(1_001),
                    primary_presentation: Presentation {
                        custom_label: None,
                        binding: Some(PresentationBinding {
                            terminal_id: terminal_id(1_004),
                            worker_id: worker_id(1_003),
                        }),
                        label: "1".into(),
                        presentation_id: presentation_id(1_002),
                        launch_failure: None,
                        launch_program: None,
                    },
                },
                slot_id: WorkbenchPanelSlotId::parse(&id(3_001)).unwrap(),
            },
            PanelSlot {
                node: PanelNode::Panel {
                    right_click: Default::default(),
                    child_presentations: Vec::new(),
                    panel_id: panel_id(3_010),
                    primary_presentation: Presentation {
                        custom_label: None,
                        binding: Some(PresentationBinding {
                            terminal_id: terminal_id(3_012),
                            worker_id: worker_id(3_011),
                        }),
                        label: "peer".into(),
                        presentation_id: presentation_id(3_013),
                        launch_failure: None,
                        launch_program: None,
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

fn split_ready_two_tabs() -> WorkbenchClientState {
    ready_with_cache(project_preferred_workspace(&split_two_tab_baseline()).unwrap())
}

/// A Retryable attachment state on the split baseline with the selection on the second
/// leaf (panel 3010): the switch was admitted, installed, and settled unattached.
fn split_retryable() -> WorkbenchClientState {
    let (mut state, marker, provenance) = switching_to_nonfirst_panel();
    assert_eq!(
        state.settle_unattached_setup_t(&marker, 0, &provenance),
        Some(CleanupContinuation::Stable)
    );
    state
}

/// The same split baseline admitted at an arbitrary cache revision, for results that
/// must be compared against a baseline newer than revision 1.
fn split_ready_at_revision(revision: u64) -> WorkbenchClientState {
    let mut baseline = split_panel_baseline();
    baseline.revision = WorkbenchRevision::new(revision);
    ready_with_cache(project_preferred_workspace(&baseline).unwrap())
}

/// A SelectedWithoutAttachment state built by the real create admission path: a create
/// whose terminal outcome is `OutcomeUnknown` records a non-attachable provenance, so
/// the snapshot settle lands `SettledWithoutAttachment` — the new Tab is selected and
/// the desired key stays armed, but no attachment exists.
fn selected_without_attachment_state() -> WorkbenchClientState {
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
        ClientPhaseKind::SelectedWithoutAttachment
    );
    state
}

fn split_commit(operation: &PanelOperation) -> PanelCommit {
    PanelCommit::Split(WorkbenchPanelSplitCommit::Pending {
        revision: WorkbenchRevision::new(operation.baseline().revision().value() + 1),
        terminal_context_id: operation.baseline().terminal_context_id().unwrap().clone(),
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
    PanelCommit::Move(WorkbenchPanelPlacementCommit {
        revision: WorkbenchRevision::new(operation.baseline().revision().value() + 1),
        terminal_context_id: operation.baseline().terminal_context_id().unwrap().clone(),
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
        ratio: Default::default(),
        children: Box::new([
            PanelSlot {
                node: PanelNode::Split {
                    ratio: Default::default(),
                    children: Box::new([
                        PanelSlot {
                            node: PanelNode::Panel {
                                right_click: Default::default(),
                                child_presentations: Vec::new(),
                                panel_id: panel_id(1_001),
                                primary_presentation: Presentation {
                                    custom_label: None,
                                    binding: Some(PresentationBinding {
                                        terminal_id: terminal_id(1_004),
                                        worker_id: worker_id(1_003),
                                    }),
                                    label: "1".into(),
                                    presentation_id: presentation_id(1_002),
                                    launch_failure: None,
                                    launch_program: None,
                                },
                            },
                            slot_id: WorkbenchPanelSlotId::parse(&id(90_003)).unwrap(),
                        },
                        PanelSlot {
                            node: PanelNode::Panel {
                                right_click: Default::default(),
                                child_presentations: Vec::new(),
                                panel_id: panel_id(90_001),
                                primary_presentation: Presentation {
                                    custom_label: None,
                                    binding: Some(PresentationBinding {
                                        terminal_id: terminal_id(90_007),
                                        worker_id: worker_id(90_006),
                                    }),
                                    label: "2".into(),
                                    presentation_id: presentation_id(90_005),
                                    launch_failure: None,
                                    launch_program: None,
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
                    right_click: Default::default(),
                    child_presentations: Vec::new(),
                    panel_id: panel_id(3_010),
                    primary_presentation: Presentation {
                        custom_label: None,
                        binding: Some(PresentationBinding {
                            terminal_id: terminal_id(3_012),
                            worker_id: worker_id(3_011),
                        }),
                        label: "peer".into(),
                        presentation_id: presentation_id(3_013),
                        launch_failure: None,
                        launch_program: None,
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
        ratio: Default::default(),
        children: Box::new([
            PanelSlot {
                node: PanelNode::Panel {
                    right_click: Default::default(),
                    child_presentations: Vec::new(),
                    panel_id: panel_id(3_010),
                    primary_presentation: Presentation {
                        custom_label: None,
                        binding: Some(PresentationBinding {
                            terminal_id: terminal_id(3_012),
                            worker_id: worker_id(3_011),
                        }),
                        label: "peer".into(),
                        presentation_id: presentation_id(3_013),
                        launch_failure: None,
                        launch_program: None,
                    },
                },
                slot_id: WorkbenchPanelSlotId::parse(&id(90_031)).unwrap(),
            },
            PanelSlot {
                node: PanelNode::Panel {
                    right_click: Default::default(),
                    child_presentations: Vec::new(),
                    panel_id: panel_id(1_001),
                    primary_presentation: Presentation {
                        custom_label: None,
                        binding: Some(PresentationBinding {
                            terminal_id: terminal_id(1_004),
                            worker_id: worker_id(1_003),
                        }),
                        label: "1".into(),
                        presentation_id: presentation_id(1_002),
                        launch_failure: None,
                        launch_program: None,
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
        right_click: Default::default(),
        child_presentations: Vec::new(),
        panel_id: panel_id(3_010),
        primary_presentation: Presentation {
            custom_label: None,
            binding: Some(PresentationBinding {
                terminal_id: terminal_id(3_012),
                worker_id: worker_id(3_011),
            }),
            label: "peer".into(),
            presentation_id: presentation_id(3_013),
            launch_failure: None,
            launch_program: None,
        },
    };
    candidate
}

/// A superseding truth in which the selected leaf 3010 was closed while leaf 1001 survived.
fn second_leaf_lost_snapshot(revision: u64) -> Snapshot {
    let mut candidate = split_panel_baseline();
    candidate.revision = WorkbenchRevision::new(revision);
    candidate.terminal_contexts[0].tabs[0].root = PanelNode::Panel {
        right_click: Default::default(),
        child_presentations: Vec::new(),
        panel_id: panel_id(1_001),
        primary_presentation: Presentation {
            custom_label: None,
            binding: Some(PresentationBinding {
                terminal_id: terminal_id(1_004),
                worker_id: worker_id(1_003),
            }),
            label: "1".into(),
            presentation_id: presentation_id(1_002),
            launch_failure: None,
            launch_program: None,
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
        assert_eq!(label.as_str(), "2");
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
        ClientPhaseKind::RetryableAttachUnavailable
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
    if let Some(record) = state.focus_record_mut() {
        record.attachment = None;
    }
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
    assert_eq!(state.notice, Some(ClientNotice::PanelNotApplied));
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
        assert_eq!(state.notice, Some(ClientNotice::PanelNotApplied));
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
    assert_eq!(state.phase_kind(), ClientPhaseKind::Ready);
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
    assert_eq!(state.phase_kind(), ClientPhaseKind::Retired);
    assert!(state.retained_panel_for_test().is_none());
    assert!(state.invariant_holds());
}

#[test]
fn committed_split_can_preserve_focus_with_the_exact_projection() {
    let mut state = split_ready();
    state.configure_panel_focus(false);
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
        Some(&SelectedTab::new_panel(
            state.pane_key_for_anchor(operation.target()).unwrap(),
            0,
            0
        ))
    );
    assert_eq!(state.phase_kind(), ClientPhaseKind::Ready);
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
        Some(&SelectedTab::new_panel(
            state.pane_key_for_anchor(operation.target()).unwrap(),
            0,
            1
        )),
        "the selected leaf follows its new geometric position"
    );
    assert_eq!(state.phase_kind(), ClientPhaseKind::Ready);
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
        ClientPhaseKind::PanelProjectionUnavailable
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
    assert_eq!(state.phase_kind(), ClientPhaseKind::Retired);
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
    assert_eq!(state.phase_kind(), ClientPhaseKind::Switching);
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
    assert_eq!(state.phase_kind(), ClientPhaseKind::Switching);
    assert_eq!(state.desired().map(|pane| pane.anchor()), Some(&fresh));
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
    assert_eq!(state.phase_kind(), ClientPhaseKind::Switching);
    assert_eq!(state.desired().map(|pane| pane.anchor()), Some(&fresh));
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
    assert_eq!(state.notice, Some(ClientNotice::PanelNotApplied));
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
    assert_eq!(state.phase_kind(), ClientPhaseKind::Switching);
    assert_eq!(state.desired().map(|pane| pane.anchor()), Some(&fresh));
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
    assert_eq!(state.phase_kind(), ClientPhaseKind::Switching);
    assert_eq!(state.desired().map(|pane| pane.anchor()), Some(&fresh));
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
    assert_eq!(state.phase_kind(), ClientPhaseKind::Retired);
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
    assert_eq!(state.notice, Some(ClientNotice::PanelNotApplied));
    assert_eq!(state.phase_kind(), ClientPhaseKind::Ready);
    assert!(state.pending_panel().is_none());
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
            terminal_context_id: operation.baseline().terminal_context_id().unwrap().clone(),
            tab_id: operation.target().tab_id().clone(),
            panel_id: panel,
            accepted_revision: WorkbenchRevision::new(revision),
        };
        let prepared = state.prepare_panel(&operation, PanelResult::CloseAccepted(accepted));
        assert_eq!(state.apply_prepared_panel(prepared), PanelDecision::Retire);
        assert_eq!(state.phase_kind(), ClientPhaseKind::Retired);
        assert!(state.retained_panel_for_test().is_none());
        assert!(state.invariant_holds());
    }
}

#[test]
fn accepted_panel_close_hides_the_leaf_and_moves_focus_to_the_survivor() {
    let mut state = split_ready();
    // The survivor needs a live viewer for the local focus move, as a real acceptance has.
    super::attach_nonfirst_leaf(&mut state);
    let operation = plan(&state, 70_150, PanelRequestKind::Close);
    let closing = operation.panel_id().clone();
    assert_eq!(state.focus_panel_id(), Some(&closing));
    assert!(state.begin_panel(operation.clone()));
    let accepted = WorkbenchPanelCloseAccepted {
        close_request_id: WorkbenchRequestId::parse(&id(70_150)).unwrap(),
        terminal_context_id: operation.baseline().terminal_context_id().unwrap().clone(),
        tab_id: operation.target().tab_id().clone(),
        panel_id: closing.clone(),
        accepted_revision: WorkbenchRevision::new(1),
    };
    let prepared = state.prepare_panel(&operation, PanelResult::CloseAccepted(accepted));
    assert_eq!(
        state.apply_prepared_panel(prepared),
        PanelDecision::Continue
    );

    // The cache keeps the leaf until publication; the display and the focus follow the survivor.
    let tab = &state.cache().tabs()[0];
    assert_eq!(tab.panel_tree().leaf_count(), 2);
    assert_eq!(state.display_panel_tree(tab).leaf_count(), 1);
    assert_ne!(state.focus_panel_id(), Some(&closing));
}

/// Regression: an accepted close of a Tab's last Panel left the strip only when the Server
/// published the Tab's removal, five seconds later for a busy terminal on Windows. That Panel
/// takes its Tab, so it leaves at acceptance as a Tab close does, and comes back with its report
/// when the close stops.
#[test]
fn accepted_last_panel_close_leaves_with_its_tab_at_acceptance() {
    let mut state = ready(2);
    let now = Instant::now();
    let operation = plan(&state, 70_155, PanelRequestKind::Close);
    let closing = state.active().unwrap().index();
    assert!(state.begin_panel(operation.clone()));
    let close_request_id = WorkbenchRequestId::parse(&id(70_155)).unwrap();
    let terminal_context_id = operation.baseline().terminal_context_id().unwrap().clone();
    let tab_id = operation.target().tab_id().clone();
    let accepted = WorkbenchPanelCloseAccepted {
        close_request_id: close_request_id.clone(),
        terminal_context_id: terminal_context_id.clone(),
        tab_id: tab_id.clone(),
        panel_id: operation.panel_id().clone(),
        accepted_revision: WorkbenchRevision::new(1),
    };
    let prepared = state.prepare_panel(&operation, PanelResult::CloseAccepted(accepted));
    assert_eq!(
        state.apply_prepared_panel(prepared),
        PanelDecision::Continue
    );

    // No publication arrived: the canonical list still holds the Tab, the Client already does not.
    assert_eq!(state.cache().tabs().len(), 2);
    assert!(!state.tab_shown(closing));
    let neighbour = state.leaving_selection_successor();
    assert!(neighbour.is_some_and(|index| index != closing && state.tab_shown(index)));
    assert!(state.invariant_holds());

    // The close is followed as the Tab's own; one that stops shows the Tab again with its report.
    let read = state
        .plan_close_read(
            WorkbenchRequestId::parse(&id(70_156)).unwrap(),
            now + OPERATION_TOTAL,
            now,
        )
        .unwrap();
    state.begin_close_exchange(&read, now).unwrap();
    state
        .settle_close_exchange(
            &read,
            &CloseResult::Read(
                orchbee_workbench_protocol::WorkbenchTabCloseReadOutcome::Available {
                    close_request_id,
                    terminal_context_id,
                    tab_id,
                    attempt_generation: WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
                    status: WorkbenchTabCloseStatus::CleanupBlocked {
                        reason: WorkbenchTabCloseBlockedReason::HostRefused,
                        resumable: true,
                    },
                },
            ),
        )
        .unwrap();
    assert!(state.tab_shown(closing));
    assert!(state.view().announced_close_status().is_some());
}

/// Regression: an accepted close of one Panel of several hid the leaf until a publication removed
/// it, with nothing following the close, so a close that stopped after its acceptance hid the
/// Panel for good. The close is now read by its operation, never sent again: a close that stops
/// shows the Panel again with its report, and one that completes ends the follow silently.
#[test]
fn accepted_panel_close_that_stops_shows_the_panel_again_with_its_report() {
    let blocked = WorkbenchTabCloseStatus::CleanupBlocked {
        reason: WorkbenchTabCloseBlockedReason::HostRefused,
        resumable: true,
    };
    let closed = WorkbenchTabCloseStatus::Closed {
        revision: WorkbenchRevision::new(2),
    };
    for status in [blocked, closed] {
        let now = Instant::now();
        let mut state = split_ready();
        super::attach_nonfirst_leaf(&mut state);
        let operation = plan(&state, 70_160, PanelRequestKind::Close);
        let closing = operation.panel_id().clone();
        assert!(state.begin_panel(operation.clone()));
        let close_request_id = WorkbenchRequestId::parse(&id(70_160)).unwrap();
        let terminal_context_id = operation.baseline().terminal_context_id().unwrap().clone();
        let tab_id = operation.target().tab_id().clone();
        let accepted = WorkbenchPanelCloseAccepted {
            close_request_id: close_request_id.clone(),
            terminal_context_id: terminal_context_id.clone(),
            tab_id: tab_id.clone(),
            panel_id: closing.clone(),
            accepted_revision: WorkbenchRevision::new(1),
        };
        let prepared = state.prepare_panel(&operation, PanelResult::CloseAccepted(accepted));
        assert_eq!(
            state.apply_prepared_panel(prepared),
            PanelDecision::Continue
        );
        assert_eq!(
            state
                .display_panel_tree(&state.cache().tabs()[0])
                .leaf_count(),
            1
        );

        let read = state
            .plan_close_read(
                WorkbenchRequestId::parse(&id(70_161)).unwrap(),
                now + OPERATION_TOTAL,
                now,
            )
            .unwrap();
        assert_eq!(
            read.kind(),
            &CloseOperationKind::Read {
                expected_operation: Some(close_request_id.clone()),
                mode: CloseReadMode::Initial,
            }
        );
        state.begin_close_exchange(&read, now).unwrap();
        let stops = !matches!(status, WorkbenchTabCloseStatus::Closed { .. });
        state
            .settle_close_exchange(
                &read,
                &CloseResult::Read(
                    orchbee_workbench_protocol::WorkbenchTabCloseReadOutcome::Available {
                        close_request_id,
                        terminal_context_id,
                        tab_id,
                        attempt_generation: WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
                        status,
                    },
                ),
            )
            .unwrap();

        let drawn = state
            .display_panel_tree(&state.cache().tabs()[0])
            .leaf_count();
        let report = state.view().announced_close_status();
        if stops {
            assert_eq!(drawn, 2);
            let report = report.unwrap();
            assert_eq!(report.status(), CloseStatusKind::CleanupBlocked);
            assert!(report.panel() && !report.resumable());
        } else {
            // The leaf stays hidden until the publication removes it.
            assert_eq!(drawn, 1);
            assert!(report.is_none());
            assert!(!state.has_retained_close_records());
        }
        assert_ne!(state.focus_panel_id(), Some(&closing));
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
    assert_eq!(state.phase_kind(), ClientPhaseKind::Retired);
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
            terminal_context_id: operation.baseline().terminal_context_id().unwrap().clone(),
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
    assert_eq!(state.notice, Some(ClientNotice::PanelNotApplied));
    assert_eq!(state.phase_kind(), ClientPhaseKind::Ready);
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
    // `expected != current` FIRST (the shared placement authority preflight) and answers
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
    assert_eq!(state.phase_kind(), ClientPhaseKind::Retired);
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
    assert_eq!(state.notice, Some(ClientNotice::PanelNotApplied));
    assert_eq!(state.phase_kind(), ClientPhaseKind::Ready);
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
    assert_eq!(state.notice, Some(ClientNotice::PanelNotApplied));
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
    assert_eq!(state.phase_kind(), ClientPhaseKind::Retired);
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
    assert_eq!(state.phase_kind(), ClientPhaseKind::Retired);
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
    assert_eq!(state.notice, Some(ClientNotice::PanelNotApplied));
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
        assert_eq!(state.phase_kind(), ClientPhaseKind::Retired);
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
    assert_eq!(state.notice, Some(ClientNotice::PanelNotApplied));
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

#[test]
fn panel_close_confirmation_requires_the_matching_draw_receipt() {
    let mut state = split_ready();
    let selected = state.active().unwrap().clone();
    let leaf = state.cache().tabs()[selected.index()]
        .panel(selected.panel_index())
        .unwrap()
        .panel_id()
        .clone();
    let request = workbench_request_id(70_900);
    let deadline = Instant::now() + OPERATION_TOTAL;

    assert!(state.open_panel_close_confirmation().is_ok());
    let view = state.view().panel_close_confirmation().unwrap();
    assert!(!view.dissolves_tab(), "one of two leaves keeps the Tab");
    assert!(!view.panel_label().is_empty());
    assert!(!view.tab_label().is_empty());
    assert_eq!(view.panel_position(), 1);
    assert_eq!(view.tab_position(), 1);
    let token = view.token();

    // Undrawn and unmatched-token receipts both refuse to plan the close.
    assert!(matches!(
        state.plan_panel_close_confirmation(request.clone(), deadline),
        Err(PanelCloseStateError::NotDisplayed)
    ));
    state.acknowledge_panel_close_draw(None);
    assert!(matches!(
        state.plan_panel_close_confirmation(request.clone(), deadline),
        Err(PanelCloseStateError::NotDisplayed)
    ));

    state.acknowledge_panel_close_draw(Some(token));
    let operation = state
        .plan_panel_close_confirmation(request.clone(), deadline)
        .expect("the drawn prompt admits the confirmed close");
    assert_eq!(operation.intent(), &PanelIntent::Close);
    assert_eq!(operation.panel_id(), &leaf);
    assert_eq!(operation.target(), selected.key());

    // A dropped receipt blocks execution until the modal draws again.
    state.invalidate_panel_close_display();
    assert!(matches!(
        state.plan_panel_close_confirmation(request, deadline),
        Err(PanelCloseStateError::NotDisplayed)
    ));
    assert!(state.invariant_holds());
}

#[test]
fn moved_panel_selection_stales_a_drawn_close_confirmation() {
    let mut state = split_ready();
    assert!(state.open_panel_close_confirmation().is_ok());
    let token = state.view().panel_close_confirmation().unwrap().token();
    state.acknowledge_panel_close_draw(Some(token));

    // The real focus move the UI drives: the selected leaf moves to the
    // second panel without any membership change.
    super::attach_nonfirst_leaf(&mut state);
    let pane = state.plan_panel_focus_local(PanelDirection::Right).unwrap();
    assert!(state.move_panel_focus_local(pane));
    assert_eq!(state.active().unwrap().panel_index(), 1);

    // The frozen leaf is no longer the selection: the drawn token cannot execute.
    assert!(matches!(
        state.plan_panel_close_confirmation(
            workbench_request_id(70_901),
            Instant::now() + OPERATION_TOTAL
        ),
        Err(PanelCloseStateError::StaleConfirmation)
    ));
    state.cancel_panel_close_confirmation();
    assert!(state.view().panel_close_confirmation().is_none());
    state.open_panel_close_confirmation().unwrap();
    let next = state.view().panel_close_confirmation().unwrap();
    assert_eq!(next.panel_position(), 2);
    assert_eq!(next.tab_position(), 1);
    assert_ne!(next.token(), token);
    assert!(state.invariant_holds());
}

/// A local focus move or ordinary poll may only land on a leaf whose own
/// setup has fully converged: between `Attached` and `SetupConfigured` the
/// record carries no controller mode yet, and focusing it would write a
/// guessed `ReadOnly` into Ready that the real `Controlling` configuration
/// then contradicts — the receipt's `ready.mode == mode` check rejects it as
/// a protocol violation. Polling it would draw the leaf's shell while that
/// same unconfigured leaf still refuses a single chord. The leaf is refused
/// as a poll slot and focus target in both windows — before the record
/// exists and while it is attached but unconfigured — and admitted once the
/// leaf is live.
#[test]
fn panel_focus_move_and_poll_wait_for_the_target_setup_configuration() {
    let mut state = split_ready();
    let active = state.active().unwrap();
    let panel_id = state
        .tab(active.index())
        .unwrap()
        .leaf_panel_ids()
        .into_iter()
        .nth(1)
        .unwrap();
    let pane = PaneKey::new(state.desired().unwrap().anchor().clone(), panel_id);
    state.install_unattached_pane(&SelectedTab::new_panel(pane.clone(), 0, 1));

    // The record exists but nothing has attached yet: no focus target and no
    // ordinary-poll slot.
    assert!(
        state
            .plan_panel_focus_local(PanelDirection::Right)
            .is_none()
    );
    assert!(state.pollable_panes().iter().all(|key| key.pane() != &pane));
    let pane_ref = state.pane_ref_for(&pane).unwrap();
    assert!(state.settle_attached_event_t(&pane_ref).is_some());

    // Attached but still unconfigured: polling it would draw a shell the
    // user still cannot reach, and the move would have to invent a mode for
    // Ready — both refused, and the state stays valid.
    assert!(state.pollable_panes().iter().all(|key| key != &pane_ref));
    assert!(
        state
            .plan_panel_focus_local(PanelDirection::Right)
            .is_none()
    );
    assert!(!state.move_panel_focus_local(pane.clone()));
    assert!(state.invariant_holds());

    // Once the real configuration lands, the same `PaneRef` becomes pollable
    // and the same move succeeds carrying the leaf's own mode — nothing for
    // the receipt to contradict, and the focus pane's identity is unchanged.
    assert!(state.record_setup_configuration_t(&pane_ref, controlling(), Some(geometry(80))));
    assert!(state.pollable_panes().iter().any(|key| key == &pane_ref));
    let pane = state.plan_panel_focus_local(PanelDirection::Right).unwrap();
    assert!(state.move_panel_focus_local(pane.clone()));
    assert_eq!(state.active().unwrap().panel_index(), 1);
    assert_eq!(state.focus_pane_ref().unwrap().pane(), &pane);
    assert!(state.invariant_holds());
}

/// A leaf the server configured `ReadOnly` is not refused from the ordinary
/// poll rotation or from focus: it displays its shell and takes focus
/// without ever claiming input control — `ready.mode` carries the granted
/// `ReadOnly`, never an invented `Controlling`.
#[test]
fn readonly_leaf_polls_and_focuses_without_claiming_control() {
    let mut state = split_ready();
    let active = state.active().unwrap();
    let panel_id = state
        .tab(active.index())
        .unwrap()
        .leaf_panel_ids()
        .into_iter()
        .nth(1)
        .unwrap();
    let pane = PaneKey::new(state.desired().unwrap().anchor().clone(), panel_id);
    state.install_unattached_pane(&SelectedTab::new_panel(pane.clone(), 0, 1));
    let pane_ref = state.pane_ref_for(&pane).unwrap();
    assert!(state.settle_attached_event_t(&pane_ref).is_some());
    assert!(state.record_setup_configuration_t(&pane_ref, ControllerMode::ReadOnly, None));

    assert!(state.pollable_panes().iter().any(|key| key == &pane_ref));
    let pane = state.plan_panel_focus_local(PanelDirection::Right).unwrap();
    assert!(state.move_panel_focus_local(pane));
    assert_eq!(state.controller_mode(), Some(&ControllerMode::ReadOnly));
    assert!(state.invariant_holds());
}

/// The pointer's direct-click planner [`plan_panel_focus_local_at`] resolves a
/// stable `WorkbenchPanelId` instead of a direction, so it must apply the same
/// readiness boundary as [`plan_panel_focus_local`]: a still-configuring leaf
/// (attached but not yet `SetupConfigured`, no settled mode) is refused at
/// plan time — never a `Some` whose matching move then has to fail or invent
/// a mode.
#[test]
fn direct_panel_click_refuses_an_unconfigured_leaf() {
    let mut state = split_ready();
    let active = state.active().unwrap();
    let panel_id = state
        .tab(active.index())
        .unwrap()
        .leaf_panel_ids()
        .into_iter()
        .nth(1)
        .unwrap();
    let pane = PaneKey::new(state.desired().unwrap().anchor().clone(), panel_id.clone());
    state.install_unattached_pane(&SelectedTab::new_panel(pane.clone(), 0, 1));

    assert!(state.plan_panel_focus_local_at(&panel_id).is_none());
    let pane_ref = state.pane_ref_for(&pane).unwrap();
    assert!(state.settle_attached_event_t(&pane_ref).is_some());

    // Attached but still unconfigured: the direct click is refused the same
    // way, and no guessed mode reaches Ready.
    assert!(state.plan_panel_focus_local_at(&panel_id).is_none());
    assert!(!state.move_panel_focus_local(pane));
    assert!(state.invariant_holds());
}

/// A `ReadOnly`-configured leaf may be focused by direct click: it displays
/// and takes focus under its real `PaneRef` while `ready.mode` carries the
/// granted `ReadOnly` — never an invented `Controlling`.
#[test]
fn direct_panel_click_focuses_a_readonly_leaf_without_claiming_control() {
    let mut state = split_ready();
    let active = state.active().unwrap();
    let panel_id = state
        .tab(active.index())
        .unwrap()
        .leaf_panel_ids()
        .into_iter()
        .nth(1)
        .unwrap();
    let pane = PaneKey::new(state.desired().unwrap().anchor().clone(), panel_id.clone());
    state.install_unattached_pane(&SelectedTab::new_panel(pane.clone(), 0, 1));
    let pane_ref = state.pane_ref_for(&pane).unwrap();
    assert!(state.settle_attached_event_t(&pane_ref).is_some());
    assert!(state.record_setup_configuration_t(&pane_ref, ControllerMode::ReadOnly, None));

    assert_eq!(
        state.plan_panel_focus_local_at(&panel_id),
        Some(pane.clone())
    );
    assert!(state.move_panel_focus_local(pane));
    assert_eq!(state.focus_pane_ref(), Some(pane_ref));
    assert_eq!(state.controller_mode(), Some(&ControllerMode::ReadOnly));
    assert!(state.invariant_holds());
}

/// A normally configured leaf takes a direct-click focus move by its exact
/// `PaneRef` incarnation: the plan names the same pane, the move carries the
/// leaf's own `Controlling` mode, and the settled focus identity is the real
/// reference — not an anchor-level match that a reincarnated pane could also
/// satisfy.
#[test]
fn direct_panel_click_focuses_a_controlling_leaf_by_exact_pane_ref() {
    let mut state = split_ready();
    let active = state.active().unwrap();
    let panel_id = state
        .tab(active.index())
        .unwrap()
        .leaf_panel_ids()
        .into_iter()
        .nth(1)
        .unwrap();
    let pane = PaneKey::new(state.desired().unwrap().anchor().clone(), panel_id.clone());
    state.install_unattached_pane(&SelectedTab::new_panel(pane.clone(), 0, 1));
    let pane_ref = state.pane_ref_for(&pane).unwrap();
    assert!(state.settle_attached_event_t(&pane_ref).is_some());
    assert!(state.record_setup_configuration_t(&pane_ref, controlling(), Some(geometry(80))));

    assert_eq!(
        state.plan_panel_focus_local_at(&panel_id),
        Some(pane.clone())
    );
    assert!(state.move_panel_focus_local(pane));
    assert_eq!(state.active().unwrap().panel_index(), 1);
    assert_eq!(state.focus_pane_ref(), Some(pane_ref));
    assert!(state.invariant_holds());
}

#[test]
fn panel_close_confirmation_owns_the_modal_slot_exclusively() {
    let mut state = split_ready();
    assert!(state.open_panel_close_confirmation().is_ok());
    assert!(matches!(
        state.open_panel_close_confirmation(),
        Err(PanelCloseStateError::Busy)
    ));
    assert!(
        state.open_close_confirmation().is_err(),
        "a Tab close cannot arm over a panel confirmation"
    );
    state.cancel_panel_close_confirmation();
    assert!(state.open_close_confirmation().is_ok());
}

/// A real refresh mutation bumps the baseline revision the confirmation
/// froze; the settle pass cancels the armed capture and posts the bounded
/// notice instead of leaving it armed-but-stale.
#[test]
fn drifted_panel_close_confirmation_settles_as_cancelled_with_notice() {
    let mut state = split_ready();
    assert!(state.open_panel_close_confirmation().is_ok());
    let token = state.view().panel_close_confirmation().unwrap().token();
    state.acknowledge_panel_close_draw(Some(token));

    assert!(state.start_pending_refresh(2));
    let prepared = state.prepare_refresh(
        2,
        WorkbenchSnapshotOutcome::Available {
            snapshot: snapshot(2, 2, None),
        },
    );
    state.apply_prepared_refresh(prepared, std::time::Instant::now());

    assert!(state.settle_stale_close_confirmations());
    assert!(state.view().panel_close_confirmation().is_none());
    assert_eq!(state.notice, Some(ClientNotice::ConfirmationCancelled));
    assert!(!state.settle_stale_close_confirmations());
    assert!(state.invariant_holds());
}

/// The same target changing its last-leaf nature — a sibling leaf appears —
/// is identity drift: the frozen `dissolves_tab` premise no longer holds and
/// the armed capture settles as cancelled.
#[test]
fn last_leaf_nature_change_stales_the_armed_panel_close() {
    let mut state = ready(2);
    assert!(state.open_panel_close_confirmation().is_ok());
    let view = state.view().panel_close_confirmation().unwrap();
    assert!(view.dissolves_tab(), "a single-leaf Tab dissolves on close");
    let token = view.token();
    state.acknowledge_panel_close_draw(Some(token));

    let mut refresh = split_panel_baseline();
    refresh.revision = WorkbenchRevision::new(2);
    assert!(state.start_pending_refresh(2));
    let prepared =
        state.prepare_refresh(2, WorkbenchSnapshotOutcome::Available { snapshot: refresh });
    state.apply_prepared_refresh(prepared, std::time::Instant::now());

    assert!(state.settle_stale_close_confirmations());
    assert!(state.view().panel_close_confirmation().is_none());
    assert_eq!(state.notice, Some(ClientNotice::ConfirmationCancelled));
    assert!(state.invariant_holds());
}

/// A new confirmation that cannot complete its first paint is refused with the geometry notice,
/// not mislabeled as an explicit cancellation.
#[test]
fn first_unpresentable_panel_close_draw_is_refused() {
    let mut state = split_ready();
    assert!(state.open_panel_close_confirmation().is_ok());
    assert_eq!(
        state.settle_panel_close_not_presented(),
        Some(GeometrySettlement::Refused)
    );
    assert!(state.view().panel_close_confirmation().is_none());
    assert_eq!(state.notice, Some(ClientNotice::DialogRefused));
    assert_eq!(state.settle_panel_close_not_presented(), None);
    assert!(state.invariant_holds());
}

#[test]
fn presented_panel_close_confirmation_suspends_and_resumes() {
    let mut state = split_ready();
    assert!(state.open_panel_close_confirmation().is_ok());
    let token = state.view().panel_close_confirmation().unwrap().token();
    state.acknowledge_panel_close_draw(Some(token));
    assert!(
        state
            .view()
            .panel_close_confirmation()
            .unwrap()
            .interactive()
    );
    assert_eq!(
        state.settle_panel_close_not_presented(),
        Some(GeometrySettlement::Suspended)
    );
    let suspended = state.view().panel_close_confirmation().unwrap();
    assert_eq!(suspended.token(), token);
    assert!(!suspended.interactive());
    state.acknowledge_panel_close_draw(Some(token));
    assert!(
        state
            .view()
            .panel_close_confirmation()
            .unwrap()
            .interactive()
    );
    assert!(state.invariant_holds());
}

/// Client retirement drops the armed confirmation outright.
#[test]
fn retirement_drops_the_armed_panel_close_confirmation() {
    let mut state = split_ready();
    assert!(state.open_panel_close_confirmation().is_ok());
    state.retire(CarrierFailure::Closed);
    assert!(state.view().panel_close_confirmation().is_none());
    assert!(state.invariant_holds());
}

#[test]
fn committed_split_focuses_the_created_identity_through_membership_install() {
    let mut state = split_ready();
    let operation = plan(&state, 70_999, split_kind());
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
    let candidate = prepared
        .membership_candidate()
        .expect("new focus requires paired installation");
    assert_eq!(candidate.focus_panel_id(), Some(&panel_id(90_001)));
    assert!(
        state.invariant_holds(),
        "preparing never moves the old input owner"
    );
}
