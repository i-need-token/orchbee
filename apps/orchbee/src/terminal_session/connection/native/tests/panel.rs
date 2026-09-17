use super::{
    commands::{
        drive_controlling_after_attach, drive_controlling_ready_from_snapshot,
        drive_pending_create_to_selected_without_attachment,
    },
    setup::*,
};
use crate::terminal_session::state::{
    CarrierFailure, PanelDecision, PanelIntent, PanelOperation, PanelRequestKind, PanelResult,
    RefreshDecision, SessionNotice, SessionPhaseKind,
};
use orchbee_client_server_protocol::{
    ClientMessage, ServerMessage, SiteId as ObcsSiteId, TerminalId as ObcsTerminalId,
    TerminalViewerAction, TerminalViewerOutcome, TerminalViewerResult, ViewerUnavailableReason,
    WorkerId as ObcsWorkerId,
};
use orchbee_workbench_protocol::{
    PanelNode, PanelOrientation, ProtocolVersion, Snapshot, TerminalPresentationId,
    WorkbenchAction, WorkbenchCommandOutcome, WorkbenchCommandResult,
    WorkbenchCommandUnavailableReason, WorkbenchPanelId, WorkbenchPanelSlotId,
    WorkbenchPanelSplitDirection, WorkbenchRejectionReason, WorkbenchRequest, WorkbenchResponse,
    WorkbenchRevision, WorkbenchSnapshotOutcome, WorkbenchSnapshotUnavailableReason,
    WorkbenchSplitId, WorkbenchTerminalGeometry,
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
    split_pending_commit_at(operation, new_panel, 2)
}

fn split_pending_commit_at(
    operation: &PanelOperation,
    new_panel: u128,
    revision: u64,
) -> WorkbenchCommandOutcome {
    WorkbenchCommandOutcome::PanelSplitPending {
        revision: WorkbenchRevision::new(revision),
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

#[test]
fn committed_split_whose_snapshot_drops_the_tab_repairs_membership_to_empty() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(
        steps(&[WireKind::Workbench, WireKind::Workbench, WireKind::Obcs]),
        geometry,
    )?;
    let (_key, _controller) =
        drive_controlling_ready_from_snapshot(&mut harness, split_snapshot(1), geometry)?;
    let operation = harness.ui.admit_panel(PanelRequestKind::Split {
        direction: crate::client_view::PanelDirection::Right,
    })?;
    respond_command(
        &harness,
        &operation,
        split_pending_commit(&operation, 4_010),
    )?;
    // A superseding snapshot that dropped the whole committed Tab cannot keep the leaf.
    respond_snapshot(
        &harness,
        WorkbenchSnapshotOutcome::Available {
            snapshot: snapshot(0, 3),
        },
    )?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Panel(PanelDecision::Cleanup(_))
    ));
    assert_eq!(
        harness
            .ui
            .state()
            .retained_panel_for_test()
            .unwrap()
            .operation(),
        &operation,
        "the known commit survives membership cleanup"
    );
    let ClientMessage::TerminalViewer(detach) = harness.carrier.take_obcs()? else {
        return Err(test_error("membership cleanup must Detach the old leaf"));
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
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Membership(RefreshDecision::SettledWithoutAttachment)
    ));
    let state = harness.ui.state();
    assert_eq!(state.phase_kind(), SessionPhaseKind::Empty);
    assert!(state.desired().is_none());
    assert_eq!(state.cache().revision(), WorkbenchRevision::new(3));
    assert_eq!(
        state.retained_panel_for_test().unwrap().operation(),
        &operation
    );
    assert!(state.invariant_holds());
    harness.finish()
}

#[test]
fn move_rejected_tab_unavailable_reads_a_snapshot_before_settling() -> TestResult {
    for (snapshot, extra) in [
        // The Tab survives past the rejection revision: ordinary refresh result.
        (split_snapshot(3), 0usize),
        // The Tab is gone at a later revision: membership repair to Empty.
        (snapshot(0, 3), 1usize),
    ] {
        let geometry = fixture_geometry()?;
        let mut wire = vec![WireKind::Workbench, WireKind::Workbench];
        wire.extend(std::iter::repeat_n(WireKind::Obcs, extra));
        let mut harness = Harness::start(steps(&wire), geometry)?;
        drive_controlling_ready_from_snapshot(&mut harness, split_snapshot(1), geometry)?;
        let operation = harness.ui.admit_panel(PanelRequestKind::Move {
            direction: crate::client_view::PanelDirection::Right,
        })?;
        respond_command(
            &harness,
            &operation,
            WorkbenchCommandOutcome::Rejected {
                revision: WorkbenchRevision::new(2),
                reason: WorkbenchRejectionReason::TabUnavailable,
            },
        )?;
        respond_snapshot(&harness, WorkbenchSnapshotOutcome::Available { snapshot })?;
        if extra == 0 {
            assert!(matches!(
                harness.ui.settle_next(FrameCut::None)?,
                UiEvent::Panel(PanelDecision::Continue)
            ));
            let state = harness.ui.state();
            assert_eq!(state.notice(), Some(SessionNotice::PanelNotApplied));
            assert_eq!(state.cache().revision(), WorkbenchRevision::new(3));
            assert_eq!(state.phase_kind(), SessionPhaseKind::Ready);
            assert!(state.selected_has_viewer());
        } else {
            assert!(matches!(
                harness.ui.settle_next(FrameCut::None)?,
                UiEvent::Panel(PanelDecision::Cleanup(_))
            ));
            let ClientMessage::TerminalViewer(detach) = harness.carrier.take_obcs()? else {
                return Err(test_error("membership cleanup must Detach the old leaf"));
            };
            harness
                .carrier
                .respond_obcs(ServerMessage::TerminalViewerResult(
                    TerminalViewerResult::new(
                        detach.request_id().clone(),
                        TerminalViewerOutcome::Detached {},
                    ),
                ))?;
            assert!(matches!(
                harness.ui.settle_next(FrameCut::None)?,
                UiEvent::Membership(RefreshDecision::SettledWithoutAttachment)
            ));
        }
        assert!(harness.ui.state().invariant_holds());
        harness.finish()?;
    }
    Ok(())
}

#[test]
fn move_unchanged_settles_at_baseline_and_retires_on_a_divergent_revision() -> TestResult {
    // Unchanged at the baseline revision is a definite no-op.
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(steps(&[WireKind::Workbench]), geometry)?;
    drive_controlling_ready_from_snapshot(&mut harness, split_snapshot(1), geometry)?;
    let operation = harness.ui.admit_panel(PanelRequestKind::Move {
        direction: crate::client_view::PanelDirection::Right,
    })?;
    respond_command(
        &harness,
        &operation,
        WorkbenchCommandOutcome::Unchanged {
            revision: WorkbenchRevision::new(1),
        },
    )?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Panel(PanelDecision::Continue)
    ));
    let state = harness.ui.state();
    assert_eq!(state.notice(), None);
    assert_eq!(state.cache().revision(), WorkbenchRevision::new(1));
    assert_eq!(state.phase_kind(), SessionPhaseKind::Ready);
    assert!(state.retained_panel_for_test().is_none());
    harness.finish()?;

    // Unchanged carrying any other revision is an incompatible outcome; the adapter
    // retires it as outcome-unknown before the state machine sees a result.
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(steps(&[WireKind::Workbench]), geometry)?;
    drive_controlling_ready_from_snapshot(&mut harness, split_snapshot(1), geometry)?;
    let operation = harness.ui.admit_panel_transport(PanelRequestKind::Move {
        direction: crate::client_view::PanelDirection::Right,
    })?;
    respond_command(
        &harness,
        &operation,
        WorkbenchCommandOutcome::Unchanged {
            revision: WorkbenchRevision::new(2),
        },
    )?;
    assert!(harness.ui.take_next().is_err());
    assert_eq!(
        harness.mailbox.is_closed(),
        Some(CarrierFailure::OutcomeUnknown)
    );
    harness.finish_after_worker_failure(CarrierFailure::OutcomeUnknown)
}

#[test]
fn committed_split_while_unattached_repairs_membership_without_a_cleanup() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(
        steps(&[
            WireKind::Workbench,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Workbench,
        ]),
        geometry,
    )?;
    let (old_key, _controller) =
        drive_controlling_ready_from_snapshot(&mut harness, snapshot(1, 1), geometry)?;
    let (_committed_key, _candidate) =
        drive_pending_create_to_selected_without_attachment(&mut harness, &old_key)?;
    assert!(!harness.ui.state().selected_has_viewer());
    let operation = harness.ui.admit_panel(PanelRequestKind::Split {
        direction: crate::client_view::PanelDirection::Right,
    })?;
    respond_command(
        &harness,
        &operation,
        split_pending_commit_at(&operation, 3_010, 3),
    )?;
    respond_snapshot(
        &harness,
        WorkbenchSnapshotOutcome::Available {
            snapshot: snapshot(0, 4),
        },
    )?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Panel(PanelDecision::SettledWithoutAttachment)
    ));
    let state = harness.ui.state();
    assert_eq!(state.phase_kind(), SessionPhaseKind::Empty);
    assert!(state.desired().is_none());
    assert_eq!(
        state.retained_panel_for_test().unwrap().operation(),
        &operation,
        "the commit is retained through the unattached membership install"
    );
    assert!(state.invariant_holds());
    harness.finish()
}

#[test]
fn committed_move_whose_superseding_snapshot_drops_the_selected_leaf_repairs_membership()
-> TestResult {
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
    let operation = harness.ui.admit_panel(PanelRequestKind::Move {
        direction: crate::client_view::PanelDirection::Right,
    })?;
    respond_command(
        &harness,
        &operation,
        WorkbenchCommandOutcome::PanelMoved {
            revision: WorkbenchRevision::new(2),
            terminal_context_id: operation.baseline().terminal_context_id().clone(),
            tab_id: operation.target().tab_id().clone(),
            panel_id: operation.panel_id().clone(),
            target_panel_id: WorkbenchPanelId::parse(&id(3_010))?,
        },
    )?;
    // A peer panel.close folded into the authority between the commit and the Snapshot
    // read: the read legitimately supersedes the committed revision, the Tab survives, and
    // the just-moved selected leaf is gone. The selection must repair onto the surviving
    // leaf through the paired membership path rather than retiring as divergence.
    let mut contracted = snapshot(1, 3);
    contracted.terminal_contexts[0].tabs[0].root = panel_node(3_010, 3_013, 3_011, 3_012, "peer");
    respond_snapshot(
        &harness,
        WorkbenchSnapshotOutcome::Available {
            snapshot: contracted,
        },
    )?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Panel(PanelDecision::Cleanup(_))
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
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Membership(RefreshDecision::AttachRecovered(ref target))
            if target.selected().panel_index() == 0
    ));
    let installed = harness.ui.state().desired().unwrap().clone();
    assert_eq!(installed.generation(), old_key.generation() + 1);
    let ClientMessage::TerminalViewer(attach) = harness.carrier.take_obcs()? else {
        return Err(test_error("the surviving leaf must AttachBinding"));
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
    assert_eq!(state.cache().revision(), WorkbenchRevision::new(3));
    assert_eq!(state.tab(0).unwrap().panel_count(), 1);
    assert_eq!(
        state.tab(0).unwrap().panel(0).unwrap().panel_id(),
        &WorkbenchPanelId::parse(&id(3_010))?
    );
    assert_eq!(
        state.retained_panel_for_test().unwrap().operation(),
        &operation,
        "the known move commit survives membership repair"
    );
    assert!(state.invariant_holds());
    harness.finish()
}

#[test]
fn committed_split_whose_superseding_snapshot_drops_the_source_leaf_repairs_membership()
-> TestResult {
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
    let (_key, _controller) =
        drive_controlling_ready_from_snapshot(&mut harness, split_snapshot(1), geometry)?;
    let operation = harness.ui.admit_panel(PanelRequestKind::Split {
        direction: crate::client_view::PanelDirection::Right,
    })?;
    respond_command(
        &harness,
        &operation,
        WorkbenchCommandOutcome::PanelSplitPending {
            revision: WorkbenchRevision::new(2),
            terminal_context_id: operation.baseline().terminal_context_id().clone(),
            tab_id: operation.target().tab_id().clone(),
            panel_id: operation.panel_id().clone(),
            new_panel_id: WorkbenchPanelId::parse(&id(5_010))?,
            split_id: WorkbenchSplitId::parse(&id(5_020))?,
            first_slot_id: WorkbenchPanelSlotId::parse(&id(5_001))?,
            second_slot_id: WorkbenchPanelSlotId::parse(&id(5_014))?,
            presentation_id: TerminalPresentationId::parse(&id(5_013))?,
        },
    )?;
    // The same peer close folded past our commit: the superseding read keeps the leaf our
    // split created plus the untouched peer leaf, and drops the selected source leaf.
    let mut superseded = snapshot(1, 3);
    superseded.terminal_contexts[0].tabs[0].root = split_node(
        panel_node(5_010, 5_013, 5_011, 5_012, "1"),
        3_001,
        panel_node(3_010, 3_013, 3_011, 3_012, "peer"),
        3_014,
        PanelOrientation::Horizontal,
        3_020,
    );
    respond_snapshot(
        &harness,
        WorkbenchSnapshotOutcome::Available {
            snapshot: superseded,
        },
    )?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Panel(PanelDecision::Cleanup(_))
    ));
    let ClientMessage::TerminalViewer(detach) = harness.carrier.take_obcs()? else {
        return Err(test_error(
            "the membership cleanup must Detach the closed leaf",
        ));
    };
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                detach.request_id().clone(),
                TerminalViewerOutcome::Detached {},
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Membership(RefreshDecision::AttachRecovered(ref target))
            if target.selected().panel_index() == 0
    ));
    let installed = harness.ui.state().desired().unwrap().clone();
    let ClientMessage::TerminalViewer(attach) = harness.carrier.take_obcs()? else {
        return Err(test_error("the surviving split leaf must AttachBinding"));
    };
    assert_eq!(
        attach.action(),
        &TerminalViewerAction::AttachBinding {
            site_id: ObcsSiteId::parse(&id(100))?,
            worker_id: ObcsWorkerId::parse(&id(5_011))?,
            terminal_id: ObcsTerminalId::parse(&id(5_012))?,
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
    assert_eq!(state.cache().revision(), WorkbenchRevision::new(3));
    assert_eq!(state.tab(0).unwrap().panel_count(), 2);
    assert_eq!(
        state.active().unwrap().panel_index(),
        0,
        "selection repairs onto the leaf our split created"
    );
    assert_eq!(
        state.retained_panel_for_test().unwrap().operation(),
        &operation
    );
    assert!(state.invariant_holds());
    harness.finish()
}

#[test]
fn committed_move_superseding_leaf_loss_while_unattached_repairs_without_cleanup() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(
        steps(&[
            WireKind::Workbench,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Workbench,
            WireKind::Workbench,
            WireKind::Workbench,
        ]),
        geometry,
    )?;
    let (old_key, _controller) =
        drive_controlling_ready_from_snapshot(&mut harness, snapshot(1, 1), geometry)?;
    let (_committed_key, _candidate) =
        drive_pending_create_to_selected_without_attachment(&mut harness, &old_key)?;
    assert!(!harness.ui.state().selected_has_viewer());
    // Grow the unattached second Tab to two leaves so panel.move has a neighbor.
    let split = harness.ui.admit_panel(PanelRequestKind::Split {
        direction: crate::client_view::PanelDirection::Right,
    })?;
    respond_command(&harness, &split, split_pending_commit_at(&split, 3_010, 3))?;
    let mut grown = snapshot(2, 3);
    grown.terminal_contexts[0].tabs[1].root = split_node(
        panel_node(1_011, 1_012, 1_013, 1_014, "2"),
        3_001,
        panel_node(3_010, 3_013, 3_011, 3_012, "2"),
        3_014,
        PanelOrientation::Horizontal,
        3_020,
    );
    respond_snapshot(
        &harness,
        WorkbenchSnapshotOutcome::Available { snapshot: grown },
    )?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Panel(PanelDecision::Continue)
    ));
    assert_eq!(harness.ui.state().tab(1).unwrap().panel_count(), 2);
    let operation = harness.ui.admit_panel(PanelRequestKind::Move {
        direction: crate::client_view::PanelDirection::Right,
    })?;
    respond_command(
        &harness,
        &operation,
        WorkbenchCommandOutcome::PanelMoved {
            revision: WorkbenchRevision::new(4),
            terminal_context_id: operation.baseline().terminal_context_id().clone(),
            tab_id: operation.target().tab_id().clone(),
            panel_id: operation.panel_id().clone(),
            target_panel_id: WorkbenchPanelId::parse(&id(3_010))?,
        },
    )?;
    // The unattached Tab survives but the moved selected leaf is gone at the superseding
    // revision: membership repair retargets the selection without any viewer cleanup.
    let mut contracted = snapshot(2, 5);
    contracted.terminal_contexts[0].tabs[1].root = panel_node(3_010, 3_013, 3_011, 3_012, "2");
    respond_snapshot(
        &harness,
        WorkbenchSnapshotOutcome::Available {
            snapshot: contracted,
        },
    )?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Panel(PanelDecision::SettledWithoutAttachment)
    ));
    let state = harness.ui.state();
    assert_eq!(
        state.phase_kind(),
        SessionPhaseKind::SelectedWithoutAttachment
    );
    assert!(!state.selected_has_viewer());
    let active = state.active().unwrap();
    assert_eq!(active.index(), 1);
    assert_eq!(active.panel_index(), 0);
    assert_eq!(state.cache().revision(), WorkbenchRevision::new(5));
    assert_eq!(state.tab(1).unwrap().panel_count(), 1);
    assert_eq!(
        state.tab(1).unwrap().panel(0).unwrap().panel_id(),
        &WorkbenchPanelId::parse(&id(3_010))?
    );
    assert_eq!(
        state.retained_panel_for_test().unwrap().operation(),
        &operation,
        "the commit is retained through the unattached membership install"
    );
    assert!(state.invariant_holds());
    harness.finish()
}

#[test]
fn committed_move_exact_revision_snapshot_without_the_moved_leaf_retires_divergent() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(steps(&[WireKind::Workbench, WireKind::Workbench]), geometry)?;
    drive_controlling_ready_from_snapshot(&mut harness, split_snapshot(1), geometry)?;
    let operation = harness.ui.admit_panel(PanelRequestKind::Move {
        direction: crate::client_view::PanelDirection::Right,
    })?;
    respond_command(
        &harness,
        &operation,
        WorkbenchCommandOutcome::PanelMoved {
            revision: WorkbenchRevision::new(2),
            terminal_context_id: operation.baseline().terminal_context_id().clone(),
            tab_id: operation.target().tab_id().clone(),
            panel_id: operation.panel_id().clone(),
            target_panel_id: WorkbenchPanelId::parse(&id(3_010))?,
        },
    )?;
    // At the exact committed revision the graft proof still runs: a Snapshot that claims
    // our revision yet lacks the moved leaf is authoritative divergence, not repair.
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
        UiEvent::Panel(PanelDecision::Retire)
    ));
    assert_eq!(
        harness
            .ui
            .state()
            .retained_panel_for_test()
            .unwrap()
            .operation(),
        &operation,
        "the known commit is retained through the divergence retire"
    );
    harness.finish_after_worker_failure(CarrierFailure::AuthorityDivergence)
}

#[test]
fn committed_move_whose_superseding_snapshot_keeps_the_selected_leaf_merges() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(steps(&[WireKind::Workbench, WireKind::Workbench]), geometry)?;
    drive_controlling_ready_from_snapshot(&mut harness, split_snapshot(1), geometry)?;
    let operation = harness.ui.admit_panel(PanelRequestKind::Move {
        direction: crate::client_view::PanelDirection::Right,
    })?;
    respond_command(
        &harness,
        &operation,
        WorkbenchCommandOutcome::PanelMoved {
            revision: WorkbenchRevision::new(2),
            terminal_context_id: operation.baseline().terminal_context_id().clone(),
            tab_id: operation.target().tab_id().clone(),
            panel_id: operation.panel_id().clone(),
            target_panel_id: WorkbenchPanelId::parse(&id(3_010))?,
        },
    )?;
    // A superseding read that folded in both our graft and a peer split keeps the selected
    // leaf: the candidate must still merge by identity with no membership cleanup.
    let mut superseded = snapshot(1, 3);
    superseded.terminal_contexts[0].tabs[0].root = split_node(
        panel_node(4_010, 4_013, 4_011, 4_012, "peer-new"),
        4_001,
        split_node(
            panel_node(3_010, 3_013, 3_011, 3_012, "peer"),
            3_021,
            panel_node(1_001, 1_002, 1_003, 1_004, "1"),
            3_022,
            PanelOrientation::Horizontal,
            3_030,
        ),
        4_014,
        PanelOrientation::Horizontal,
        4_020,
    );
    respond_snapshot(
        &harness,
        WorkbenchSnapshotOutcome::Available {
            snapshot: superseded,
        },
    )?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Panel(PanelDecision::Continue)
    ));
    let state = harness.ui.state();
    assert_eq!(state.phase_kind(), SessionPhaseKind::Ready);
    assert_eq!(state.cache().revision(), WorkbenchRevision::new(3));
    assert_eq!(state.tab(0).unwrap().panel_count(), 3);
    assert_eq!(
        state.active().unwrap().panel_index(),
        2,
        "the moved leaf keeps its identity as selected inside the peer-grown tree"
    );
    assert!(state.selected_has_viewer());
    assert_eq!(
        state.retained_panel_for_test().unwrap().operation(),
        &operation
    );
    assert!(state.invariant_holds());
    harness.finish()
}

#[test]
fn committed_split_after_reorder_dropping_the_selected_leaf_repairs_membership() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(
        steps(&[
            WireKind::Workbench,
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
    let (_key, _controller) =
        drive_controlling_ready_from_snapshot(&mut harness, split_snapshot(1), geometry)?;
    // A peer split lands before the selected leaf so the selection sits at panel index 1.
    harness.ui.admit_refresh()?;
    let mut reordered = snapshot(1, 2);
    reordered.terminal_contexts[0].tabs[0].root = split_node(
        panel_node(4_010, 4_013, 4_011, 4_012, "peer-new"),
        4_001,
        split_node(
            panel_node(1_001, 1_002, 1_003, 1_004, "1"),
            3_001,
            panel_node(3_010, 3_013, 3_011, 3_012, "peer"),
            3_014,
            PanelOrientation::Horizontal,
            3_020,
        ),
        4_014,
        PanelOrientation::Horizontal,
        4_020,
    );
    respond_snapshot(
        &harness,
        WorkbenchSnapshotOutcome::Available {
            snapshot: reordered,
        },
    )?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Refresh { .. }
    ));
    assert_eq!(harness.ui.state().active().unwrap().panel_index(), 1);
    let operation = harness.ui.admit_panel(PanelRequestKind::Split {
        direction: crate::client_view::PanelDirection::Right,
    })?;
    respond_command(
        &harness,
        &operation,
        WorkbenchCommandOutcome::PanelSplitPending {
            revision: WorkbenchRevision::new(3),
            terminal_context_id: operation.baseline().terminal_context_id().clone(),
            tab_id: operation.target().tab_id().clone(),
            panel_id: operation.panel_id().clone(),
            new_panel_id: WorkbenchPanelId::parse(&id(5_010))?,
            split_id: WorkbenchSplitId::parse(&id(5_020))?,
            first_slot_id: WorkbenchPanelSlotId::parse(&id(5_001))?,
            second_slot_id: WorkbenchPanelSlotId::parse(&id(5_014))?,
            presentation_id: TerminalPresentationId::parse(&id(5_013))?,
        },
    )?;
    // The superseding read folded in a peer close of the just-split selected leaf; the
    // surviving tree keeps the outer split and contracts the inner one to the peer leaf.
    let mut contracted = snapshot(1, 4);
    contracted.terminal_contexts[0].tabs[0].root = split_node(
        panel_node(4_010, 4_013, 4_011, 4_012, "peer-new"),
        4_001,
        panel_node(3_010, 3_013, 3_011, 3_012, "peer"),
        4_014,
        PanelOrientation::Horizontal,
        4_020,
    );
    respond_snapshot(
        &harness,
        WorkbenchSnapshotOutcome::Available {
            snapshot: contracted,
        },
    )?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Panel(PanelDecision::Cleanup(_))
    ));
    let ClientMessage::TerminalViewer(detach) = harness.carrier.take_obcs()? else {
        return Err(test_error(
            "the membership cleanup must Detach the closed leaf",
        ));
    };
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                detach.request_id().clone(),
                TerminalViewerOutcome::Detached {},
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Membership(RefreshDecision::AttachRecovered(ref target))
            if target.selected().panel_index() == 1
    ));
    let installed = harness.ui.state().desired().unwrap().clone();
    let ClientMessage::TerminalViewer(attach) = harness.carrier.take_obcs()? else {
        return Err(test_error("the surviving leaf must AttachBinding"));
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
    assert_eq!(state.cache().revision(), WorkbenchRevision::new(4));
    assert_eq!(state.tab(0).unwrap().panel_count(), 2);
    assert_eq!(
        state.active().unwrap().panel_index(),
        1,
        "selection repairs onto the surviving leaf at the clamped index"
    );
    assert_eq!(
        state.retained_panel_for_test().unwrap().operation(),
        &operation
    );
    assert!(state.invariant_holds());
    harness.finish()
}

#[test]
fn committed_split_superseding_leaf_loss_while_unattached_repairs_without_cleanup() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(
        steps(&[
            WireKind::Workbench,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Workbench,
            WireKind::Workbench,
            WireKind::Workbench,
        ]),
        geometry,
    )?;
    let (old_key, _controller) =
        drive_controlling_ready_from_snapshot(&mut harness, snapshot(1, 1), geometry)?;
    let (_committed_key, _candidate) =
        drive_pending_create_to_selected_without_attachment(&mut harness, &old_key)?;
    assert!(!harness.ui.state().selected_has_viewer());
    // Grow the unattached second Tab to two leaves so a peer close of the selected source
    // still leaves one retained baseline member for the canonical check.
    let grow = harness.ui.admit_panel(PanelRequestKind::Split {
        direction: crate::client_view::PanelDirection::Right,
    })?;
    respond_command(&harness, &grow, split_pending_commit_at(&grow, 3_010, 3))?;
    let mut grown = snapshot(2, 3);
    grown.terminal_contexts[0].tabs[1].root = split_node(
        panel_node(1_011, 1_012, 1_013, 1_014, "2"),
        3_001,
        panel_node(3_010, 3_013, 3_011, 3_012, "2"),
        3_014,
        PanelOrientation::Horizontal,
        3_020,
    );
    respond_snapshot(
        &harness,
        WorkbenchSnapshotOutcome::Available { snapshot: grown },
    )?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Panel(PanelDecision::Continue)
    ));
    assert_eq!(harness.ui.state().tab(1).unwrap().panel_count(), 2);
    // The tested split commits on the selected leaf; the superseding Snapshot folds in a
    // peer close of that source while the new leaf and the peer leaf survive.
    let operation = harness.ui.admit_panel(PanelRequestKind::Split {
        direction: crate::client_view::PanelDirection::Right,
    })?;
    respond_command(
        &harness,
        &operation,
        WorkbenchCommandOutcome::PanelSplitPending {
            revision: WorkbenchRevision::new(4),
            terminal_context_id: operation.baseline().terminal_context_id().clone(),
            tab_id: operation.target().tab_id().clone(),
            panel_id: operation.panel_id().clone(),
            new_panel_id: WorkbenchPanelId::parse(&id(4_010))?,
            split_id: WorkbenchSplitId::parse(&id(4_020))?,
            first_slot_id: WorkbenchPanelSlotId::parse(&id(4_001))?,
            second_slot_id: WorkbenchPanelSlotId::parse(&id(4_014))?,
            presentation_id: TerminalPresentationId::parse(&id(4_013))?,
        },
    )?;
    let mut contracted = snapshot(2, 5);
    contracted.terminal_contexts[0].tabs[1].root = split_node(
        panel_node(3_010, 3_013, 3_011, 3_012, "2"),
        4_001,
        panel_node(4_010, 4_013, 4_011, 4_012, "2"),
        4_014,
        PanelOrientation::Horizontal,
        4_020,
    );
    respond_snapshot(
        &harness,
        WorkbenchSnapshotOutcome::Available {
            snapshot: contracted,
        },
    )?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Panel(PanelDecision::SettledWithoutAttachment)
    ));
    let state = harness.ui.state();
    assert_eq!(
        state.phase_kind(),
        SessionPhaseKind::SelectedWithoutAttachment
    );
    assert!(!state.selected_has_viewer());
    let active = state.active().unwrap();
    assert_eq!(active.index(), 1);
    assert_eq!(active.panel_index(), 0);
    assert_eq!(state.cache().revision(), WorkbenchRevision::new(5));
    assert_eq!(state.tab(1).unwrap().panel_count(), 2);
    assert_eq!(
        state.tab(1).unwrap().panel(0).unwrap().panel_id(),
        &WorkbenchPanelId::parse(&id(3_010))?,
        "the selection repairs onto the surviving peer leaf"
    );
    assert_eq!(
        state.retained_panel_for_test().unwrap().operation(),
        &operation,
        "the split commit is retained through the unattached membership install"
    );
    assert!(state.invariant_holds());
    harness.finish()
}

#[test]
fn committed_move_superseding_leaf_loss_detach_failure_retires_protocol_keeping_commit()
-> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(
        steps(&[WireKind::Workbench, WireKind::Workbench, WireKind::Obcs]),
        geometry,
    )?;
    drive_controlling_ready_from_snapshot(&mut harness, split_snapshot(1), geometry)?;
    let operation = harness.ui.admit_panel(PanelRequestKind::Move {
        direction: crate::client_view::PanelDirection::Right,
    })?;
    respond_command(
        &harness,
        &operation,
        WorkbenchCommandOutcome::PanelMoved {
            revision: WorkbenchRevision::new(2),
            terminal_context_id: operation.baseline().terminal_context_id().clone(),
            tab_id: operation.target().tab_id().clone(),
            panel_id: operation.panel_id().clone(),
            target_panel_id: WorkbenchPanelId::parse(&id(3_010))?,
        },
    )?;
    let mut contracted = snapshot(1, 3);
    contracted.terminal_contexts[0].tabs[0].root = panel_node(3_010, 3_013, 3_011, 3_012, "peer");
    respond_snapshot(
        &harness,
        WorkbenchSnapshotOutcome::Available {
            snapshot: contracted,
        },
    )?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Panel(PanelDecision::Cleanup(_))
    ));
    let ClientMessage::TerminalViewer(detach) = harness.carrier.take_obcs()? else {
        return Err(test_error(
            "the membership cleanup must Detach the closed leaf",
        ));
    };
    // A non-Detached Detach outcome is a protocol failure, not an outcome: the worker
    // retires while the already-known move commit stays retained for the failure proof.
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                detach.request_id().clone(),
                TerminalViewerOutcome::Unavailable {
                    reason: ViewerUnavailableReason::TerminalRuntimeUnavailable,
                },
            ),
        ))?;
    assert_eq!(
        harness
            .ui
            .state()
            .retained_panel_for_test()
            .unwrap()
            .operation(),
        &operation,
        "the known commit survives the failed cleanup"
    );
    harness.finish_after_worker_failure(CarrierFailure::Protocol)
}

#[test]
fn committed_move_superseding_leaf_loss_attach_unavailable_parks_retryable_keeping_commit()
-> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(
        steps(&[
            WireKind::Workbench,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
        ]),
        geometry,
    )?;
    drive_controlling_ready_from_snapshot(&mut harness, split_snapshot(1), geometry)?;
    let operation = harness.ui.admit_panel(PanelRequestKind::Move {
        direction: crate::client_view::PanelDirection::Right,
    })?;
    respond_command(
        &harness,
        &operation,
        WorkbenchCommandOutcome::PanelMoved {
            revision: WorkbenchRevision::new(2),
            terminal_context_id: operation.baseline().terminal_context_id().clone(),
            tab_id: operation.target().tab_id().clone(),
            panel_id: operation.panel_id().clone(),
            target_panel_id: WorkbenchPanelId::parse(&id(3_010))?,
        },
    )?;
    let mut contracted = snapshot(1, 3);
    contracted.terminal_contexts[0].tabs[0].root = panel_node(3_010, 3_013, 3_011, 3_012, "peer");
    respond_snapshot(
        &harness,
        WorkbenchSnapshotOutcome::Available {
            snapshot: contracted,
        },
    )?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Panel(PanelDecision::Cleanup(_))
    ));
    let ClientMessage::TerminalViewer(detach) = harness.carrier.take_obcs()? else {
        return Err(test_error(
            "the membership cleanup must Detach the closed leaf",
        ));
    };
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                detach.request_id().clone(),
                TerminalViewerOutcome::Detached {},
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Membership(RefreshDecision::AttachRecovered(ref target))
            if target.selected().index() == 0 && target.selected().panel_index() == 0
    ));
    let installed = harness.ui.state().desired().unwrap().clone();
    let ClientMessage::TerminalViewer(attach) = harness.carrier.take_obcs()? else {
        return Err(test_error("the surviving leaf must AttachBinding"));
    };
    assert_eq!(
        attach.action(),
        &TerminalViewerAction::AttachBinding {
            site_id: ObcsSiteId::parse(&id(100))?,
            worker_id: ObcsWorkerId::parse(&id(3_011))?,
            terminal_id: ObcsTerminalId::parse(&id(3_012))?,
        }
    );
    // The recovered attach answers Unavailable: the session parks on the retargeted leaf
    // retryable, the committed move fact stays, and no second attach or replay runs.
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                attach.request_id().clone(),
                TerminalViewerOutcome::Unavailable {
                    reason: ViewerUnavailableReason::TerminalRuntimeUnavailable,
                },
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::SetupWithoutAttachment { key, .. } if key == installed
    ));
    let state = harness.ui.state();
    assert_eq!(
        state.phase_kind(),
        SessionPhaseKind::RetryableAttachUnavailable
    );
    assert!(!state.selected_has_viewer());
    let active = state.active().unwrap();
    assert_eq!(active.index(), 0);
    assert_eq!(active.panel_index(), 0);
    assert_eq!(
        state.tab(0).unwrap().panel(0).unwrap().panel_id(),
        &WorkbenchPanelId::parse(&id(3_010))?
    );
    assert_eq!(state.cache().revision(), WorkbenchRevision::new(3));
    assert_eq!(
        state.retained_panel_for_test().unwrap().operation(),
        &operation,
        "the known move commit survives the failed reattach"
    );
    assert!(state.invariant_holds());
    harness.finish()
}
