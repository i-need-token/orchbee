use super::{
    commands::{drive_controlling_after_attach, drive_controlling_ready_from_snapshot},
    setup::*,
};
use crate::terminal_session::state::{
    CarrierFailure, PanelDecision, PanelIntent, PanelOperation, PanelRequestKind, PanelResult,
    RefreshDecision, SessionNotice, SessionPhaseKind,
};
use orchbee_client_server_protocol::{
    ClientMessage, ServerMessage, SiteId as ObcsSiteId, TerminalId as ObcsTerminalId,
    TerminalViewerAction, TerminalViewerOutcome, TerminalViewerResult, WorkerId as ObcsWorkerId,
};
use orchbee_workbench_protocol::{
    PanelNode, PanelOrientation, ProtocolVersion, Snapshot, TerminalPresentationId,
    WorkbenchAction, WorkbenchCommandOutcome, WorkbenchCommandResult,
    WorkbenchCommandUnavailableReason, WorkbenchPanelId, WorkbenchPanelSlotId,
    WorkbenchPanelSplitDirection, WorkbenchRequest, WorkbenchResponse, WorkbenchRevision,
    WorkbenchSnapshotOutcome, WorkbenchSnapshotUnavailableReason, WorkbenchSplitId,
    WorkbenchTerminalGeometry,
};

fn steps(after: &[WireKind]) -> Vec<WireKind> {
    let mut steps = vec![
        WireKind::Obcs,
        WireKind::Workbench,
        WireKind::Obcs,
        WireKind::Obcs,
        WireKind::Obcs,
        WireKind::Obcs,
        WireKind::Obcs,
    ];
    steps.extend_from_slice(after);
    steps
}

fn id(value: u128) -> String {
    format!("{value:032x}")
}

/// Snapshot whose single Tab is a Split of the fixture leaf (ids 1001–1004, the attachable
/// binding) and a peer leaf in the 3000 id range carrying the given label.
fn split_snapshot_with_label(
    revision: u64,
    peer_label: &str,
    orientation: PanelOrientation,
) -> Snapshot {
    let mut candidate = snapshot(1, revision);
    let first = candidate.terminal_contexts[0].tabs[0].root.clone();
    candidate.terminal_contexts[0].tabs[0].root = split_node(
        first,
        3_001,
        panel_node(3_010, 3_013, 3_011, 3_012, peer_label),
        3_014,
        orientation,
        3_020,
    );
    candidate
}

/// Baseline snapshot for move/close fixtures: the peer leaf already exists, so its label is free.
fn split_snapshot(revision: u64) -> Snapshot {
    split_snapshot_with_label(revision, "peer", PanelOrientation::Horizontal)
}

/// The exact authority candidate for a Right `panel.split`: the grafted leaf carries the
/// requested presentation label ("1" in the fixtures).
fn split_candidate(revision: u64) -> Snapshot {
    split_snapshot_with_label(revision, "1", PanelOrientation::Horizontal)
}

/// The exact authority candidate for a Below `panel.split`.
fn split_candidate_below(revision: u64) -> Snapshot {
    split_snapshot_with_label(revision, "1", PanelOrientation::Vertical)
}

fn respond_snapshot(harness: &Harness, outcome: WorkbenchSnapshotOutcome) -> TestResult {
    let read = harness.carrier.take_read()?;
    harness.carrier.respond_read(&read, outcome)
}

fn respond_command(
    harness: &Harness,
    operation: &PanelOperation,
    outcome: WorkbenchCommandOutcome,
) -> TestResult {
    let WorkbenchRequest::Command(command) = harness.carrier.take_workbench()? else {
        return Err(test_error("Panel must submit one command"));
    };
    assert_eq!(&command.request_id, operation.request_id());
    assert_eq!(command.expected_revision, operation.baseline().revision());
    harness
        .carrier
        .respond_workbench(WorkbenchResponse::Command(WorkbenchCommandResult {
            protocol_version: ProtocolVersion,
            request_id: command.request_id,
            outcome,
        }))
}

fn split_pending_commit(operation: &PanelOperation, new_panel: u128) -> WorkbenchCommandOutcome {
    WorkbenchCommandOutcome::PanelSplitPending {
        revision: WorkbenchRevision::new(2),
        terminal_context_id: operation.baseline().terminal_context_id().clone(),
        tab_id: operation.target().tab_id().clone(),
        panel_id: operation.panel_id().clone(),
        new_panel_id: WorkbenchPanelId::parse(&id(new_panel)).unwrap(),
        split_id: WorkbenchSplitId::parse(&id(3_020)).unwrap(),
        first_slot_id: WorkbenchPanelSlotId::parse(&id(3_001)).unwrap(),
        second_slot_id: WorkbenchPanelSlotId::parse(&id(3_014)).unwrap(),
        presentation_id: TerminalPresentationId::parse(&id(3_013)).unwrap(),
    }
}

#[test]
fn panel_split_commits_validates_and_merges_the_exact_candidate() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(steps(&[WireKind::Workbench, WireKind::Workbench]), geometry)?;
    let (_key, _controller) =
        drive_controlling_ready_from_snapshot(&mut harness, snapshot(1, 1), geometry)?;
    let operation = harness.ui.admit_panel(PanelRequestKind::Split {
        direction: crate::client_view::PanelDirection::Right,
    })?;
    let PanelIntent::Split {
        direction,
        label,
        geometry: split_geometry,
    } = operation.intent()
    else {
        return Err(test_error("Split admission must carry the Split intent"));
    };
    assert_eq!(*direction, WorkbenchPanelSplitDirection::Right);
    assert_eq!(label.as_str(), "1");
    assert_eq!(
        *split_geometry,
        WorkbenchTerminalGeometry::new(
            geometry.columns(),
            geometry.rows(),
            geometry.cell_pixel_width(),
            geometry.cell_pixel_height(),
        )?
    );
    respond_command(
        &harness,
        &operation,
        split_pending_commit(&operation, 3_010),
    )?;
    respond_snapshot(
        &harness,
        WorkbenchSnapshotOutcome::Available {
            snapshot: split_candidate(2),
        },
    )?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Panel(PanelDecision::Continue)
    ));
    let state = harness.ui.state();
    assert_eq!(state.cache().revision(), WorkbenchRevision::new(2));
    assert_eq!(state.tab(0).unwrap().panel_count(), 2);
    assert_eq!(state.phase_kind(), SessionPhaseKind::Ready);
    assert_eq!(
        state.retained_panel_for_test().unwrap().operation(),
        &operation
    );
    harness.finish()
}

#[test]
fn panel_split_snapshot_unavailable_retains_commit_until_refresh_validates() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(
        steps(&[
            WireKind::Workbench,
            WireKind::Workbench,
            WireKind::Workbench,
        ]),
        geometry,
    )?;
    let (_key, _controller) =
        drive_controlling_ready_from_snapshot(&mut harness, snapshot(1, 1), geometry)?;
    let operation = harness.ui.admit_panel(PanelRequestKind::Split {
        direction: crate::client_view::PanelDirection::Down,
    })?;
    respond_command(
        &harness,
        &operation,
        split_pending_commit(&operation, 3_010),
    )?;
    respond_snapshot(
        &harness,
        WorkbenchSnapshotOutcome::Unavailable {
            reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
        },
    )?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Panel(PanelDecision::Continue)
    ));
    assert_eq!(
        harness.ui.state().phase_kind(),
        SessionPhaseKind::PanelProjectionUnavailable
    );
    assert_eq!(
        harness
            .ui
            .state()
            .panel_recovery_for_test()
            .unwrap()
            .operation(),
        &operation
    );
    harness.ui.admit_refresh()?;
    respond_snapshot(
        &harness,
        WorkbenchSnapshotOutcome::Available {
            snapshot: split_candidate_below(2),
        },
    )?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Refresh { .. }
    ));
    assert_eq!(harness.ui.state().phase_kind(), SessionPhaseKind::Ready);
    assert!(harness.ui.state().panel_recovery_for_test().is_none());
    assert_eq!(harness.ui.state().tab(0).unwrap().panel_count(), 2);
    assert!(harness.ui.state().invariant_holds());
    harness.finish()
}

#[test]
fn panel_move_commits_and_merges_the_relocated_leaf() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(steps(&[WireKind::Workbench, WireKind::Workbench]), geometry)?;
    let (_key, _controller) =
        drive_controlling_ready_from_snapshot(&mut harness, split_snapshot(1), geometry)?;
    let operation = harness.ui.admit_panel(PanelRequestKind::Move {
        direction: crate::client_view::PanelDirection::Right,
    })?;
    let PanelIntent::Move {
        target_panel_id,
        direction,
    } = operation.intent()
    else {
        return Err(test_error("Move admission must carry the Move intent"));
    };
    assert_eq!(*direction, WorkbenchPanelSplitDirection::Right);
    assert_eq!(
        target_panel_id,
        &WorkbenchPanelId::parse(&id(3_010))?,
        "Right targets the neighboring leaf"
    );
    respond_command(
        &harness,
        &operation,
        WorkbenchCommandOutcome::PanelMoved {
            revision: WorkbenchRevision::new(2),
            terminal_context_id: operation.baseline().terminal_context_id().clone(),
            tab_id: operation.target().tab_id().clone(),
            panel_id: operation.panel_id().clone(),
            target_panel_id: target_panel_id.clone(),
        },
    )?;
    // Moving the selected (first) leaf Right grafts it after the target leaf.
    let mut moved = split_snapshot(2);
    let root = moved.terminal_contexts[0].tabs[0].root.clone();
    let PanelNode::Split { children, .. } = root else {
        panic!("split baseline must keep a Split root");
    };
    let [_, target] = *children;
    moved.terminal_contexts[0].tabs[0].root = split_node(
        target.node,
        3_021,
        panel_node(1_001, 1_002, 1_003, 1_004, "1"),
        3_022,
        PanelOrientation::Horizontal,
        3_030,
    );
    respond_snapshot(
        &harness,
        WorkbenchSnapshotOutcome::Available { snapshot: moved },
    )?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Panel(PanelDecision::Continue)
    ));
    let state = harness.ui.state();
    assert_eq!(state.cache().revision(), WorkbenchRevision::new(2));
    assert_eq!(state.tab(0).unwrap().panel_count(), 2);
    assert_eq!(
        state.active().unwrap().panel_index(),
        1,
        "the moved leaf keeps its identity as selected"
    );
    assert_eq!(
        state.retained_panel_for_test().unwrap().operation(),
        &operation
    );
    harness.finish()
}

#[test]
fn panel_close_of_the_attached_leaf_repairs_membership_to_the_survivor() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(
        steps(&[
            WireKind::Workbench,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
        ]),
        geometry,
    )?;
    let (old_key, _controller) =
        drive_controlling_ready_from_snapshot(&mut harness, split_snapshot(1), geometry)?;
    let operation = harness.ui.admit_panel(PanelRequestKind::Close)?;
    assert!(matches!(operation.intent(), PanelIntent::Close));
    respond_command(
        &harness,
        &operation,
        WorkbenchCommandOutcome::PanelCloseAccepted {
            close_request_id: operation.request_id().clone(),
            terminal_context_id: operation.baseline().terminal_context_id().clone(),
            tab_id: operation.target().tab_id().clone(),
            panel_id: operation.panel_id().clone(),
            accepted_revision: WorkbenchRevision::new(1),
        },
    )?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Panel(PanelDecision::Continue)
    ));
    let state = harness.ui.state();
    assert_eq!(state.phase_kind(), SessionPhaseKind::Ready);
    assert!(state.retained_panel_for_test().is_none());
    assert_eq!(state.cache().revision(), WorkbenchRevision::new(1));
    harness.ui.admit_refresh()?;
    let mut contracted = snapshot(1, 2);
    contracted.terminal_contexts[0].tabs[0].root = panel_node(3_010, 3_013, 3_011, 3_012, "peer");
    respond_snapshot(
        &harness,
        WorkbenchSnapshotOutcome::Available {
            snapshot: contracted,
        },
    )?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Refresh {
            decision: RefreshDecision::Cleanup(_),
            ..
        }
    ));
    let ClientMessage::TerminalViewer(detach) = harness.carrier.take_obcs()? else {
        return Err(test_error(
            "the membership cleanup must Detach the closed leaf",
        ));
    };
    assert_eq!(
        detach.action(),
        &TerminalViewerAction::Detach {
            attachment_id: attached_viewer_id()?,
        }
    );
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                detach.request_id().clone(),
                TerminalViewerOutcome::Detached {},
            ),
        ))?;
    let UiEvent::Refresh {
        decision: RefreshDecision::AttachRecovered(target),
        ..
    } = harness.ui.settle_next(FrameCut::None)?
    else {
        return Err(test_error("surviving leaf must recover with Attach"));
    };
    assert_eq!(target.selected().panel_index(), 0);
    let installed = harness.ui.state().desired().unwrap().clone();
    assert_eq!(installed.generation(), old_key.generation() + 1);
    let ClientMessage::TerminalViewer(attach) = harness.carrier.take_obcs()? else {
        return Err(test_error("the survivor leaf must AttachBinding"));
    };
    assert_eq!(
        attach.action(),
        &TerminalViewerAction::AttachBinding {
            site_id: ObcsSiteId::parse(&id(100))?,
            worker_id: ObcsWorkerId::parse(&id(3_011))?,
            terminal_id: ObcsTerminalId::parse(&id(3_012))?,
        }
    );
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(attach.request_id().clone(), attached_outcome()?),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Attached { key } if key == installed
    ));
    drive_controlling_after_attach(&mut harness, &installed, geometry)?;
    let state = harness.ui.state();
    assert_eq!(state.phase_kind(), SessionPhaseKind::Ready);
    assert_eq!(state.cache().revision(), WorkbenchRevision::new(2));
    assert_eq!(state.tab(0).unwrap().panel_count(), 1);
    assert_eq!(
        state.tab(0).unwrap().panel(0).unwrap().panel_id(),
        &WorkbenchPanelId::parse(&id(3_010))?
    );
    assert!(state.invariant_holds());
    harness.finish()
}

#[test]
fn panel_terminal_busy_is_a_definite_not_applied_notice() -> TestResult {
    for (kind, baseline) in [
        (
            PanelRequestKind::Split {
                direction: crate::client_view::PanelDirection::Right,
            },
            snapshot(1, 1),
        ),
        (
            PanelRequestKind::Move {
                direction: crate::client_view::PanelDirection::Right,
            },
            split_snapshot(1),
        ),
    ] {
        let geometry = fixture_geometry()?;
        let mut harness = Harness::start(steps(&[WireKind::Workbench]), geometry)?;
        drive_controlling_ready_from_snapshot(&mut harness, baseline, geometry)?;
        let operation = harness.ui.admit_panel_transport(kind)?;
        respond_command(
            &harness,
            &operation,
            WorkbenchCommandOutcome::Unavailable {
                reason: WorkbenchCommandUnavailableReason::TerminalBusy,
            },
        )?;
        let taken = harness.ui.take_next()?;
        let (settled, outcome) = harness.ui.settle_panel_transport(taken)?;
        assert_eq!(settled, operation);
        assert!(matches!(
            outcome,
            PanelResult::Precommit(
                crate::terminal_session::state::PanelPrecommit::Unavailable {
                    reason: WorkbenchCommandUnavailableReason::TerminalBusy,
                }
            )
        ));
        assert!(harness.ui.state().selected_has_viewer());
        harness.finish()?;
    }
    Ok(())
}

#[test]
fn panel_precommit_rejection_is_a_definite_not_applied_notice() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(steps(&[WireKind::Workbench]), geometry)?;
    drive_controlling_ready_from_snapshot(&mut harness, snapshot(1, 1), geometry)?;
    let operation = harness.ui.admit_panel(PanelRequestKind::Split {
        direction: crate::client_view::PanelDirection::Right,
    })?;
    respond_command(&harness, &operation, WorkbenchCommandOutcome::Denied {})?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Panel(PanelDecision::Continue)
    ));
    let state = harness.ui.state();
    assert_eq!(state.phase_kind(), SessionPhaseKind::Ready);
    assert_eq!(state.notice(), Some(SessionNotice::PanelNotApplied));
    assert!(state.pending_panel().is_none());
    harness.finish()
}

#[test]
fn lost_split_response_retires_unknown_and_never_replays() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(steps(&[WireKind::Workbench]), geometry)?;
    drive_controlling_ready_from_snapshot(&mut harness, snapshot(1, 1), geometry)?;
    let operation = harness.ui.admit_panel_transport(PanelRequestKind::Split {
        direction: crate::client_view::PanelDirection::Right,
    })?;
    let WorkbenchRequest::Command(command) = harness.carrier.take_workbench()? else {
        return Err(test_error("Split admission must submit panel.split"));
    };
    assert_eq!(command.request_id, *operation.request_id());
    assert!(matches!(command.action, WorkbenchAction::PanelSplit { .. }));
    harness.carrier.disconnect()?;
    assert!(harness.ui.take_next().is_err());
    assert_eq!(
        harness.mailbox.is_closed(),
        Some(CarrierFailure::OutcomeUnknown)
    );
    harness.finish_after_worker_failure(CarrierFailure::OutcomeUnknown)
}

#[test]
fn split_commit_worker_exit_retains_the_pending_commit() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(steps(&[WireKind::Workbench, WireKind::Workbench]), geometry)?;
    let (_key, _controller) =
        drive_controlling_ready_from_snapshot(&mut harness, snapshot(1, 1), geometry)?;
    let operation = harness.ui.admit_panel_transport(PanelRequestKind::Split {
        direction: crate::client_view::PanelDirection::Right,
    })?;
    respond_command(
        &harness,
        &operation,
        split_pending_commit(&operation, 3_010),
    )?;
    respond_snapshot(&harness, WorkbenchSnapshotOutcome::Denied {})?;
    let retained = harness.close_with_pending_panel()?;
    let retained = retained.ok_or_else(|| test_error("worker must retain the Split commit"))?;
    assert_eq!(retained.operation(), &operation);
    Ok(())
}

#[test]
fn move_without_a_neighbor_leaf_is_a_noop_admission() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(steps(&[]), geometry)?;
    drive_controlling_ready_from_snapshot(&mut harness, snapshot(1, 1), geometry)?;
    // A single-leaf Tab has no Right neighbor: the plan refuses before admission.
    let refused = harness.ui.admit_panel(PanelRequestKind::Move {
        direction: crate::client_view::PanelDirection::Right,
    });
    assert!(refused.is_err());
    harness.finish()
}
