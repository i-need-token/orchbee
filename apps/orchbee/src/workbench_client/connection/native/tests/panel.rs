use super::{
    commands::{
        drive_controlling_after_attach, drive_controlling_ready_from_snapshot,
        drive_pending_create_to_selected_without_attachment,
    },
    setup::*,
};
use crate::workbench_client::{
    mailbox::{QueuedCommand, Tagged, WorkerEvent},
    state::{
        CarrierFailure, ClientNotice, ClientPhaseKind, ControllerMode, PaneFault, PaneRef,
        PanelDecision, PanelIntent, PanelOperation, PanelRequestKind, PanelResult, RefreshDecision,
    },
};
use orchbee_client_server_protocol::{
    ClientMessage, ControlInput, ControlUnavailableReason, ControllerId, Geometry, ServerMessage,
    SessionId as ObcsSessionId, TerminalControlAction, TerminalControlOutcome,
    TerminalControlResult, TerminalId as ObcsTerminalId, TerminalViewerAction,
    TerminalViewerOutcome, TerminalViewerResult, ViewerUnavailableReason, WorkerId as ObcsWorkerId,
};
use orchbee_workbench_protocol::{
    PanelNode, PanelOrientation, ProtocolVersion, Snapshot, TerminalPresentationId,
    WorkbenchAction, WorkbenchCommandOutcome, WorkbenchCommandResult,
    WorkbenchCommandUnavailableReason, WorkbenchPanelId, WorkbenchPanelSlotId,
    WorkbenchPanelSplitDirection, WorkbenchRejectionReason, WorkbenchRequest, WorkbenchResponse,
    WorkbenchRevision, WorkbenchSnapshotOutcome, WorkbenchSnapshotUnavailableReason,
    WorkbenchSplitId, WorkbenchTerminalGeometry,
};
use std::{sync::Arc, time::Instant};

fn steps(after: &[WireKind]) -> Vec<WireKind> {
    let mut steps = vec![
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
/// requested presentation label, the next ordinal after the fixture leaf ("2").
fn split_candidate(revision: u64) -> Snapshot {
    split_snapshot_with_label(revision, "2", PanelOrientation::Horizontal)
}

/// The exact authority candidate for a Below `panel.split`.
fn split_candidate_below(revision: u64) -> Snapshot {
    split_snapshot_with_label(revision, "2", PanelOrientation::Vertical)
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
        terminal_context_id: operation.baseline().terminal_context_id().unwrap().clone(),
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
    // This case covers the legacy merge that keeps the source Panel focused.
    harness.ui.state_mut().configure_panel_focus(false);
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
    assert_eq!(label.as_str(), "2", "the new Panel takes the next ordinal");
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
    assert_eq!(state.phase_kind(), ClientPhaseKind::Ready);
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
    // This case covers the legacy merge that keeps the source Panel focused.
    harness.ui.state_mut().configure_panel_focus(false);
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
        ClientPhaseKind::PanelProjectionUnavailable
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
    assert_eq!(harness.ui.state().phase_kind(), ClientPhaseKind::Ready);
    assert!(harness.ui.state().panel_recovery_for_test().is_none());
    assert_eq!(harness.ui.state().tab(0).unwrap().panel_count(), 2);
    assert!(harness.ui.state().invariant_holds());
    harness.finish()
}

#[test]
fn focused_split_after_an_unavailable_snapshot_installs_the_new_panel_on_refresh() -> TestResult {
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
        ClientPhaseKind::PanelProjectionUnavailable
    );
    harness.ui.admit_refresh()?;
    respond_snapshot(
        &harness,
        WorkbenchSnapshotOutcome::Available {
            snapshot: split_candidate_below(2),
        },
    )?;
    // The validated refresh moves focus to the created Panel through the paired cleanup.
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Refresh {
            decision: RefreshDecision::Cleanup(_),
            ..
        }
    ));
    let ClientMessage::TerminalViewer(detach) = harness.carrier.take_obcs()? else {
        return Err(test_error("the source viewer must detach first"));
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
        UiEvent::Refresh {
            decision: RefreshDecision::AttachRecovered(ref target),
            ..
        } if target.selected().panel_index() == 1
    ));
    let installed = harness.ui.state().desired().unwrap().clone();
    let ClientMessage::TerminalViewer(attach) = harness.carrier.take_obcs()? else {
        return Err(test_error("the created Panel must attach"));
    };
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(attach.request_id().clone(), attached_outcome()?),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Attached { key } if key == installed
    ));
    drive_controlling_after_attach(&mut harness, installed.anchor(), geometry)?;
    let state = harness.ui.state();
    assert_eq!(state.phase_kind(), ClientPhaseKind::Ready);
    assert!(state.panel_recovery_for_test().is_none());
    assert_eq!(state.active().unwrap().panel_index(), 1);
    assert_eq!(
        state.tab(0).unwrap().panel(1).unwrap().panel_id(),
        &WorkbenchPanelId::parse(&id(3_010))?
    );
    assert!(state.invariant_holds());
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
            terminal_context_id: operation.baseline().terminal_context_id().unwrap().clone(),
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
fn panel_close_runtime_loss_before_publication_preserves_the_survivor() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut after = vec![WireKind::Obcs; 6];
    after.extend([
        WireKind::Workbench, // Close
        WireKind::Obcs,      // already admitted input on the closed pane
        WireKind::Obcs,      // already admitted sibling Resize
        WireKind::Obcs,      // viewer Poll confirms runtime loss independently of control
        WireKind::Workbench, // publication read before removal appears
        WireKind::Workbench, // canonical removal
    ]);
    after.extend([WireKind::Obcs; 6]); // sibling cleanup, then fresh attachment
    let mut harness = Harness::start(steps(&after), geometry)?;
    drive_controlling_ready_from_snapshot(&mut harness, split_snapshot(1), geometry)?;
    let left = harness.ui.state().focus_pane_ref().unwrap();
    let (right_panel, right_viewer, _) =
        drive_reconcile_peer_first_frame(&mut harness, geometry, PeerAcquire::Controlling)?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::FeedbackAccepted { .. }
    ));
    harness.ui.move_panel_focus_to_pane(&right_panel)?;
    let right = harness.ui.state().focus_pane_ref().unwrap();
    let operation = harness.ui.admit_panel(PanelRequestKind::Close)?;
    // All commands precede any membership read. Losing the closed pane must
    // not retire the sibling or pretend the queued input was accepted.
    harness
        .ui
        .admit_input(ControlInput::new(b"queued".to_vec())?)?;
    let resized = Geometry::new(81, 24, 8, 16)?;
    assert!(
        harness
            .ui
            .state_mut()
            .record_admitted_geometry(&left, resized)
    );
    harness
        .mailbox
        .try_enqueue(QueuedCommand::Resize(Tagged::new(left.clone(), resized)))
        .map_err(|error| test_error(&format!("sibling resize: {error:?}")))?;
    harness
        .mailbox
        .try_enqueue(QueuedCommand::Poll(Tagged::new(right.clone(), ())))
        .map_err(|error| test_error(&format!("closed pane poll: {error:?}")))?;
    respond_command(
        &harness,
        &operation,
        WorkbenchCommandOutcome::PanelCloseAccepted {
            close_request_id: operation.request_id().clone(),
            terminal_context_id: operation.baseline().terminal_context_id().unwrap().clone(),
            tab_id: operation.target().tab_id().clone(),
            panel_id: operation.panel_id().clone(),
            accepted_revision: WorkbenchRevision::new(1),
        },
    )?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Panel(PanelDecision::Continue)
    ));
    let ClientMessage::TerminalControl(input) = harness.carrier.take_obcs()? else {
        return Err(test_error("queued input must reach its original viewer"));
    };
    assert!(
        matches!(input.action(), TerminalControlAction::Input { attachment_id, .. } if attachment_id == &right_viewer)
    );
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalControlResult(
            TerminalControlResult::new(
                input.request_id().clone(),
                TerminalControlOutcome::Unavailable {
                    reason: ControlUnavailableReason::TerminalRuntimeUnavailable,
                },
            ),
        ))?;
    let control_lost = harness.ui.take_next()?;
    assert!(
        matches!(control_lost.event(), WorkerEvent::PaneFailed { key, fault: PaneFault::ControlLost } if key == &right)
    );
    assert!(matches!(
        harness.ui.settle_taken(control_lost, FrameCut::None)?,
        UiEvent::PaneFailed
    ));
    assert_eq!(harness.mailbox.is_closed(), None);
    assert_eq!(harness.ui.state().phase_kind(), ClientPhaseKind::Ready);
    assert_eq!(
        harness.ui.state().controller_mode(),
        Some(&ControllerMode::ReadOnly)
    );
    assert_eq!(harness.ui.state().desired(), Some(right.pane()));
    assert!(harness.ui.state().pane_attached(&right));
    assert!(harness.ui.state().pane_projected(&right));
    assert_eq!(harness.ui.state().last_accepted_geometry(&right), None);
    assert!(harness.ui.state().pane_attached(&left));

    let ClientMessage::TerminalControl(resize) = harness.carrier.take_obcs()? else {
        return Err(test_error(
            "the surviving viewer must keep its queued Resize",
        ));
    };
    assert!(
        matches!(resize.action(), TerminalControlAction::Resize { attachment_id, geometry, .. }
        if attachment_id == &attached_viewer_id()? && *geometry == resized)
    );
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalControlResult(
            TerminalControlResult::new(
                resize.request_id().clone(),
                TerminalControlOutcome::Resized { geometry: resized },
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::SetupConfigured { .. }
    ));
    assert_eq!(
        harness.ui.state().last_accepted_geometry(&left),
        Some(resized)
    );
    assert_eq!(harness.ui.state().focus_pane_ref(), Some(right.clone()));

    // A control refusal cannot prove that final output disappeared. Only the
    // viewer's own typed runtime loss authorizes removal of this attachment.
    let ClientMessage::TerminalViewer(poll) = harness.carrier.take_obcs()? else {
        return Err(test_error("the read-only viewer must keep its queued Poll"));
    };
    assert_eq!(
        poll.action(),
        &TerminalViewerAction::Poll {
            attachment_id: right_viewer,
        }
    );
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                poll.request_id().clone(),
                TerminalViewerOutcome::Unavailable {
                    reason: ViewerUnavailableReason::TerminalRuntimeUnavailable,
                },
            ),
        ))?;
    let failed = harness.ui.take_next()?;
    assert!(
        matches!(failed.event(), WorkerEvent::PaneFailed { key, fault: PaneFault::ViewerLost } if key == &right)
    );
    assert!(matches!(
        harness.ui.settle_taken(failed, FrameCut::None)?,
        UiEvent::PaneFailed
    ));
    assert_eq!(harness.mailbox.is_closed(), None);
    assert_eq!(
        harness.ui.state().phase_kind(),
        ClientPhaseKind::RetryableAttachUnavailable
    );
    assert_eq!(harness.ui.state().desired(), Some(right.pane()));
    assert!(!harness.ui.state().pane_attached(&right));
    assert!(harness.ui.state().pane_attached(&left));

    for revision in [1, 2] {
        let mailbox = Arc::clone(&harness.mailbox);
        let state = harness.ui.state_mut();
        state.note_publication_hint();
        mailbox
            .try_enqueue_refresh_for(
                state,
                Instant::now() + crate::workbench_client::state::OPERATION_TOTAL,
                |state, generation| {
                    state
                        .start_publication_drain(generation)
                        .then_some(())
                        .ok_or("publication admission")
                },
                |_state, _result| {},
            )
            .map_err(|error| test_error(&format!("publication refresh: {error:?}")))?;
        respond_snapshot(
            &harness,
            WorkbenchSnapshotOutcome::Available {
                snapshot: if revision == 1 {
                    split_snapshot(1)
                } else {
                    snapshot(1, 2)
                },
            },
        )?;
        let event = harness.ui.settle_next(FrameCut::None)?;
        if revision == 1 {
            assert!(matches!(
                event,
                UiEvent::Refresh {
                    decision: RefreshDecision::Continue,
                    ..
                }
            ));
            assert_eq!(
                harness.ui.state().phase_kind(),
                ClientPhaseKind::RetryableAttachUnavailable
            );
            assert_eq!(harness.ui.state().focus_pane_ref(), Some(right.clone()));
            assert!(!harness.ui.state().pane_attached(&right));
            assert!(harness.ui.state().pane_attached(&left));
        } else {
            assert!(matches!(
                event,
                UiEvent::Refresh {
                    decision: RefreshDecision::Cleanup(_),
                    ..
                }
            ));
        }
    }
    let ClientMessage::TerminalViewer(detach) = harness.carrier.take_obcs()? else {
        return Err(test_error(
            "membership must clean up only the still-owned sibling",
        ));
    };
    assert_eq!(
        detach.action(),
        &TerminalViewerAction::Detach {
            attachment_id: attached_viewer_id()?
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
        UiEvent::Refresh {
            decision: RefreshDecision::AttachRecovered(_),
            ..
        }
    ));
    let ClientMessage::TerminalViewer(attach) = harness.carrier.take_obcs()? else {
        return Err(test_error(
            "canonical survivor must receive a fresh attachment",
        ));
    };
    assert!(
        matches!(attach.action(), TerminalViewerAction::AttachBinding { terminal_id, .. }
        if terminal_id == &ObcsTerminalId::parse(&id(1_004))?)
    );
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(attach.request_id().clone(), attached_outcome()?),
        ))?;
    let installed = harness.ui.state().desired().unwrap().clone();
    assert!(installed.generation() > left.anchor().generation());
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Attached { .. }
    ));
    drive_controlling_after_attach(&mut harness, installed.anchor(), geometry)?;
    assert_eq!(harness.ui.state().phase_kind(), ClientPhaseKind::Ready);
    assert_eq!(
        harness.ui.state().cache().revision(),
        WorkbenchRevision::new(2)
    );
    assert_eq!(harness.ui.state().tab(0).unwrap().panel_count(), 1);
    assert_eq!(
        harness.ui.state().desired().unwrap().panel(),
        left.pane().panel()
    );
    assert!(harness.ui.state().invariant_holds());
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
            terminal_context_id: operation.baseline().terminal_context_id().unwrap().clone(),
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
    assert_eq!(state.phase_kind(), ClientPhaseKind::Ready);
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
            session_id: ObcsSessionId::parse(&id(100))?,
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
    drive_controlling_after_attach(&mut harness, installed.anchor(), geometry)?;
    let state = harness.ui.state();
    assert_eq!(state.phase_kind(), ClientPhaseKind::Ready);
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
                crate::workbench_client::state::PanelPrecommit::Unavailable {
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
    assert_eq!(state.phase_kind(), ClientPhaseKind::Ready);
    assert_eq!(state.notice(), Some(ClientNotice::PanelNotApplied));
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
    assert_eq!(state.phase_kind(), ClientPhaseKind::Empty);
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
            assert_eq!(state.notice(), Some(ClientNotice::PanelNotApplied));
            assert_eq!(state.cache().revision(), WorkbenchRevision::new(3));
            assert_eq!(state.phase_kind(), ClientPhaseKind::Ready);
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
    assert_eq!(state.phase_kind(), ClientPhaseKind::Ready);
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
    assert_eq!(state.phase_kind(), ClientPhaseKind::Empty);
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
            terminal_context_id: operation.baseline().terminal_context_id().unwrap().clone(),
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
            session_id: ObcsSessionId::parse(&id(100))?,
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
    drive_controlling_after_attach(&mut harness, installed.anchor(), geometry)?;
    let state = harness.ui.state();
    assert_eq!(state.phase_kind(), ClientPhaseKind::Ready);
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
            terminal_context_id: operation.baseline().terminal_context_id().unwrap().clone(),
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
            session_id: ObcsSessionId::parse(&id(100))?,
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
    drive_controlling_after_attach(&mut harness, installed.anchor(), geometry)?;
    let state = harness.ui.state();
    assert_eq!(state.phase_kind(), ClientPhaseKind::Ready);
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
    // The split only builds the layout; keep the source Panel focused.
    harness.ui.state_mut().configure_panel_focus(false);
    let split = harness.ui.admit_panel(PanelRequestKind::Split {
        direction: crate::client_view::PanelDirection::Right,
    })?;
    respond_command(&harness, &split, split_pending_commit_at(&split, 3_010, 3))?;
    let mut grown = snapshot(2, 3);
    grown.terminal_contexts[0].tabs[1].root = split_node(
        panel_node(1_011, 1_012, 1_013, 1_014, "2"),
        3_001,
        panel_node(3_010, 3_013, 3_011, 3_012, "3"),
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
            terminal_context_id: operation.baseline().terminal_context_id().unwrap().clone(),
            tab_id: operation.target().tab_id().clone(),
            panel_id: operation.panel_id().clone(),
            target_panel_id: WorkbenchPanelId::parse(&id(3_010))?,
        },
    )?;
    // The unattached Tab survives but the moved selected leaf is gone at the superseding
    // revision: membership repair retargets the selection without any viewer cleanup.
    let mut contracted = snapshot(2, 5);
    contracted.terminal_contexts[0].tabs[1].root = panel_node(3_010, 3_013, 3_011, 3_012, "3");
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
        ClientPhaseKind::SelectedWithoutAttachment
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
            terminal_context_id: operation.baseline().terminal_context_id().unwrap().clone(),
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
            terminal_context_id: operation.baseline().terminal_context_id().unwrap().clone(),
            tab_id: operation.target().tab_id().clone(),
            panel_id: operation.panel_id().clone(),
            target_panel_id: WorkbenchPanelId::parse(&id(3_010))?,
        },
    )?;
    // A superseding read that folded in both our graft and a peer split keeps the selected
    // leaf: the candidate must still merge by identity with no membership cleanup. The
    // peer split wraps only leaf B's slot inside the move-created Split 3_030 — leaf
    // splits never re-root an existing Split.
    let mut superseded = snapshot(1, 3);
    superseded.terminal_contexts[0].tabs[0].root = split_node(
        split_node(
            panel_node(4_010, 4_013, 4_011, 4_012, "peer-new"),
            4_001,
            panel_node(3_010, 3_013, 3_011, 3_012, "peer"),
            4_014,
            PanelOrientation::Horizontal,
            4_020,
        ),
        3_021,
        panel_node(1_001, 1_002, 1_003, 1_004, "1"),
        3_022,
        PanelOrientation::Horizontal,
        3_030,
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
    assert_eq!(state.phase_kind(), ClientPhaseKind::Ready);
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
    // A peer left-split of the selected leaf lands at r2: a leaf split wraps only the
    // target leaf into a new inner Split, so the old root/slots survive and A moves to
    // panel index 1 in [C, A, B].
    harness.ui.admit_refresh()?;
    let mut reordered = snapshot(1, 2);
    reordered.terminal_contexts[0].tabs[0].root = split_node(
        split_node(
            panel_node(4_010, 4_013, 4_011, 4_012, "peer-new"),
            4_001,
            panel_node(1_001, 1_002, 1_003, 1_004, "1"),
            4_014,
            PanelOrientation::Horizontal,
            4_020,
        ),
        3_001,
        panel_node(3_010, 3_013, 3_011, 3_012, "peer"),
        3_014,
        PanelOrientation::Horizontal,
        3_020,
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
            terminal_context_id: operation.baseline().terminal_context_id().unwrap().clone(),
            tab_id: operation.target().tab_id().clone(),
            panel_id: operation.panel_id().clone(),
            new_panel_id: WorkbenchPanelId::parse(&id(5_010))?,
            split_id: WorkbenchSplitId::parse(&id(5_020))?,
            first_slot_id: WorkbenchPanelSlotId::parse(&id(5_001))?,
            second_slot_id: WorkbenchPanelSlotId::parse(&id(5_014))?,
            presentation_id: TerminalPresentationId::parse(&id(5_013))?,
        },
    )?;
    // The superseding read folded in exactly one peer close of the just-split selected
    // leaf at r4: the split's own Split 5_020 collapses and its new leaf 5_010 takes the
    // collapsed slot inside 4_020 — the clamped repair lands on it.
    let mut contracted = snapshot(1, 4);
    contracted.terminal_contexts[0].tabs[0].root = split_node(
        split_node(
            panel_node(4_010, 4_013, 4_011, 4_012, "peer-new"),
            4_001,
            panel_node(5_010, 5_013, 5_011, 5_012, "1"),
            4_014,
            PanelOrientation::Horizontal,
            4_020,
        ),
        3_001,
        panel_node(3_010, 3_013, 3_011, 3_012, "peer"),
        3_014,
        PanelOrientation::Horizontal,
        3_020,
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
            session_id: ObcsSessionId::parse(&id(100))?,
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
    drive_controlling_after_attach(&mut harness, installed.anchor(), geometry)?;
    let state = harness.ui.state();
    assert_eq!(state.phase_kind(), ClientPhaseKind::Ready);
    assert_eq!(state.cache().revision(), WorkbenchRevision::new(4));
    assert_eq!(state.tab(0).unwrap().panel_count(), 3);
    assert_eq!(
        state.tab(0).unwrap().panel(1).unwrap().panel_id(),
        &WorkbenchPanelId::parse(&id(5_010))?,
        "the split's own new leaf is the clamped repair target"
    );
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
    // The split only builds the layout; keep the source Panel focused.
    harness.ui.state_mut().configure_panel_focus(false);
    let grow = harness.ui.admit_panel(PanelRequestKind::Split {
        direction: crate::client_view::PanelDirection::Right,
    })?;
    respond_command(&harness, &grow, split_pending_commit_at(&grow, 3_010, 3))?;
    let mut grown = snapshot(2, 3);
    grown.terminal_contexts[0].tabs[1].root = split_node(
        panel_node(1_011, 1_012, 1_013, 1_014, "2"),
        3_001,
        panel_node(3_010, 3_013, 3_011, 3_012, "3"),
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
            terminal_context_id: operation.baseline().terminal_context_id().unwrap().clone(),
            tab_id: operation.target().tab_id().clone(),
            panel_id: operation.panel_id().clone(),
            new_panel_id: WorkbenchPanelId::parse(&id(4_010))?,
            split_id: WorkbenchSplitId::parse(&id(4_020))?,
            first_slot_id: WorkbenchPanelSlotId::parse(&id(4_001))?,
            second_slot_id: WorkbenchPanelSlotId::parse(&id(4_014))?,
            presentation_id: TerminalPresentationId::parse(&id(4_013))?,
        },
    )?;
    // One peer close of the selected source leaf collapses the split's graft: the new
    // leaf takes the collapsed inner slot first and the peer leaf survives second.
    let mut contracted = snapshot(2, 5);
    contracted.terminal_contexts[0].tabs[1].root = split_node(
        panel_node(4_010, 4_013, 4_011, 4_012, "2"),
        3_001,
        panel_node(3_010, 3_013, 3_011, 3_012, "3"),
        3_014,
        PanelOrientation::Horizontal,
        3_020,
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
        ClientPhaseKind::SelectedWithoutAttachment
    );
    assert!(!state.selected_has_viewer());
    let active = state.active().unwrap();
    assert_eq!(active.index(), 1);
    assert_eq!(active.panel_index(), 0);
    assert_eq!(state.cache().revision(), WorkbenchRevision::new(5));
    assert_eq!(state.tab(1).unwrap().panel_count(), 2);
    assert_eq!(
        state.tab(1).unwrap().panel(0).unwrap().panel_id(),
        &WorkbenchPanelId::parse(&id(4_010))?,
        "the selection repairs onto the split's own new leaf at the clamped index"
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
            terminal_context_id: operation.baseline().terminal_context_id().unwrap().clone(),
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
            terminal_context_id: operation.baseline().terminal_context_id().unwrap().clone(),
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
            session_id: ObcsSessionId::parse(&id(100))?,
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
        ClientPhaseKind::RetryableAttachUnavailable
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

// ---- m4-r4-tui independent review challenges (session foggy-lycra) ----
// Shapes chosen to be disjoint from the repair commit's own witnesses, revised
// after Astra's 03a trace review: `panel.move` rejects TabUnavailable only when
// the Tab is absent from the authority projection, and each panel.close commits
// its own revision — so refused witnesses exercise the tab-absent membership arm
// and a two-leaf fold carries the revision of the second close.

#[test]
fn refused_move_whose_superseding_snapshot_drops_the_tab_repairs_membership_to_survivor()
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
    let mut initial = snapshot(2, 1);
    initial.terminal_contexts[0].tabs[0].root = split_node(
        panel_node(1_001, 1_002, 1_003, 1_004, "1"),
        3_001,
        panel_node(3_010, 3_013, 3_011, 3_012, "peer"),
        3_014,
        PanelOrientation::Horizontal,
        3_020,
    );
    let (_key, _controller) =
        drive_controlling_ready_from_snapshot(&mut harness, initial, geometry)?;
    let operation = harness.ui.admit_panel(PanelRequestKind::Move {
        direction: crate::client_view::PanelDirection::Right,
    })?;
    // A peer removed the whole selected Tab at r2, so the authority rejects the move with
    // TabUnavailable there; the superseding r3 read confirms the Tab is still gone. The
    // refused settlement must route through membership onto the surviving Tab with
    // exactly one detach/attach pair and no fabricated commit.
    respond_command(
        &harness,
        &operation,
        WorkbenchCommandOutcome::Rejected {
            revision: WorkbenchRevision::new(2),
            reason: WorkbenchRejectionReason::TabUnavailable,
        },
    )?;
    // The candidate must drop Tab id 1_000 by identity: a same-id Tab would be a
    // resurrected leaf-repair shape that no real close sequence produces.
    let mut contracted = snapshot(2, 3);
    contracted.terminal_contexts[0].tabs.remove(0);
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
            "a refused superseding candidate must still Detach the lost attachment",
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
            if target.selected().index() == 0 && target.selected().panel_index() == 0
    ));
    let installed = harness.ui.state().desired().unwrap().clone();
    let ClientMessage::TerminalViewer(attach) = harness.carrier.take_obcs()? else {
        return Err(test_error("the surviving Tab must AttachBinding"));
    };
    assert_eq!(
        attach.action(),
        &TerminalViewerAction::AttachBinding {
            session_id: ObcsSessionId::parse(&id(100))?,
            worker_id: ObcsWorkerId::parse(&id(1_013))?,
            terminal_id: ObcsTerminalId::parse(&id(1_014))?,
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
    drive_controlling_after_attach(&mut harness, installed.anchor(), geometry)?;
    let state = harness.ui.state();
    assert_eq!(state.phase_kind(), ClientPhaseKind::Ready);
    assert_eq!(state.cache().revision(), WorkbenchRevision::new(3));
    assert_eq!(state.cache().tabs().len(), 1);
    let active = state.active().unwrap();
    assert_eq!(active.index(), 0);
    assert_eq!(active.panel_index(), 0);
    assert!(
        state.retained_panel_for_test().is_none(),
        "a refused operation has no commit to retain"
    );
    assert_eq!(
        state.notice(),
        Some(ClientNotice::PanelNotApplied),
        "the refused settlement surfaces the not-applied notice"
    );
    assert!(state.invariant_holds());
    harness.finish()
}

#[test]
fn committed_move_whose_superseding_snapshot_clamps_the_selection_down_to_a_survivor() -> TestResult
{
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
    // Two successive peer left-splits of the selected leaf (one leaf per revision, each
    // wrapping only the target leaf's slot — never re-rooting an existing Split): r2
    // lands C ahead of A, r3 lands E ahead of A again, so the read at r3 folds both and
    // the selection sits at panel index 2 in [C, E, A, B].
    harness.ui.admit_refresh()?;
    let mut grown = snapshot(1, 3);
    grown.terminal_contexts[0].tabs[0].root = split_node(
        split_node(
            panel_node(4_010, 4_013, 4_011, 4_012, "peer-c"),
            4_001,
            split_node(
                panel_node(5_010, 5_013, 5_011, 5_012, "peer-e"),
                5_001,
                panel_node(1_001, 1_002, 1_003, 1_004, "1"),
                5_014,
                PanelOrientation::Horizontal,
                5_020,
            ),
            4_014,
            PanelOrientation::Horizontal,
            4_020,
        ),
        3_001,
        panel_node(3_010, 3_013, 3_011, 3_012, "peer"),
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
        UiEvent::Refresh { .. }
    ));
    assert_eq!(harness.ui.state().active().unwrap().panel_index(), 2);
    let operation = harness.ui.admit_panel(PanelRequestKind::Move {
        direction: crate::client_view::PanelDirection::Left,
    })?;
    let PanelIntent::Move {
        target_panel_id, ..
    } = operation.intent()
    else {
        return Err(test_error("Move admission must carry the Move intent"));
    };
    let moved_to = target_panel_id.clone();
    respond_command(
        &harness,
        &operation,
        WorkbenchCommandOutcome::PanelMoved {
            revision: WorkbenchRevision::new(4),
            terminal_context_id: operation.baseline().terminal_context_id().unwrap().clone(),
            tab_id: operation.target().tab_id().clone(),
            panel_id: operation.panel_id().clone(),
            target_panel_id: moved_to,
        },
    )?;
    // Two peer closes fold in before the read, one leaf per revision: A goes at r5
    // (inner split 5_020 collapses, sibling E takes its slot) and B goes at r6 (root
    // 3_020 collapses, subtree 4_020 becomes the root). Only the two inserted leaves
    // remain at r6 and the repaired selection must clamp down from index 2 to index 1.
    let mut contracted = snapshot(1, 6);
    contracted.terminal_contexts[0].tabs[0].root = split_node(
        panel_node(4_010, 4_013, 4_011, 4_012, "peer-c"),
        4_001,
        panel_node(5_010, 5_013, 5_011, 5_012, "peer-e"),
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
            session_id: ObcsSessionId::parse(&id(100))?,
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
    drive_controlling_after_attach(&mut harness, installed.anchor(), geometry)?;
    let state = harness.ui.state();
    assert_eq!(state.phase_kind(), ClientPhaseKind::Ready);
    assert_eq!(state.cache().revision(), WorkbenchRevision::new(6));
    assert_eq!(state.tab(0).unwrap().panel_count(), 2);
    assert_eq!(
        state.active().unwrap().panel_index(),
        1,
        "the clamped selection lands on the last surviving leaf"
    );
    assert_eq!(
        state.tab(0).unwrap().panel(1).unwrap().panel_id(),
        &WorkbenchPanelId::parse(&id(5_010))?
    );
    assert_eq!(
        state.retained_panel_for_test().unwrap().operation(),
        &operation
    );
    assert!(state.invariant_holds());
    harness.finish()
}

#[test]
fn committed_move_whose_superseding_snapshot_drops_the_leaf_and_rewrites_a_member_retires_divergent()
-> TestResult {
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
            terminal_context_id: operation.baseline().terminal_context_id().unwrap().clone(),
            tab_id: operation.target().tab_id().clone(),
            panel_id: operation.panel_id().clone(),
            target_panel_id: WorkbenchPanelId::parse(&id(3_010))?,
        },
    )?;
    // A superseding read is not freed from canonical validation: the selected leaf is
    // gone AND the surviving member kept its panel_id but rewrote its terminal binding.
    // The membership-repair path must not absorb this shape; it stays divergence.
    let mut corrupted = snapshot(1, 3);
    corrupted.terminal_contexts[0].tabs[0].root = panel_node(3_010, 3_013, 3_011, 9_999, "peer");
    respond_snapshot(
        &harness,
        WorkbenchSnapshotOutcome::Available {
            snapshot: corrupted,
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
fn refused_move_superseding_tab_loss_while_unattached_reattaches_without_cleanup() -> TestResult {
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
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
        ]),
        geometry,
    )?;
    let (old_key, _controller) =
        drive_controlling_ready_from_snapshot(&mut harness, snapshot(1, 1), geometry)?;
    let (_committed_key, _candidate) =
        drive_pending_create_to_selected_without_attachment(&mut harness, &old_key)?;
    assert!(!harness.ui.state().selected_has_viewer());
    // Grow the unattached second Tab to two leaves so panel.move has a neighbor.
    // The split only builds the layout; keep the source Panel focused.
    harness.ui.state_mut().configure_panel_focus(false);
    let split = harness.ui.admit_panel(PanelRequestKind::Split {
        direction: crate::client_view::PanelDirection::Right,
    })?;
    respond_command(&harness, &split, split_pending_commit_at(&split, 3_010, 3))?;
    let mut grown = snapshot(2, 3);
    grown.terminal_contexts[0].tabs[1].root = split_node(
        panel_node(1_011, 1_012, 1_013, 1_014, "2"),
        3_001,
        panel_node(3_010, 3_013, 3_011, 3_012, "3"),
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
    // Refused at r4 because a peer removed the whole selected Tab there; the superseding
    // r5 read confirms it is still gone. With no attachment there must be no fabricated
    // cleanup: the membership path installs the selection on the surviving Tab directly
    // and settles without viewer traffic.
    respond_command(
        &harness,
        &operation,
        WorkbenchCommandOutcome::Rejected {
            revision: WorkbenchRevision::new(4),
            reason: WorkbenchRejectionReason::TabUnavailable,
        },
    )?;
    let mut contracted = snapshot(2, 5);
    contracted.terminal_contexts[0].tabs.remove(1);
    respond_snapshot(
        &harness,
        WorkbenchSnapshotOutcome::Available {
            snapshot: contracted,
        },
    )?;
    // The surviving Tab's provenance is attachable, so the membership repair retries the
    // attach on it — one AttachBinding, still no Detach since nothing was attached.
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Panel(PanelDecision::AttachRecovered(ref target))
            if target.selected().index() == 0 && target.selected().panel_index() == 0
    ));
    let installed = harness.ui.state().desired().unwrap().clone();
    let ClientMessage::TerminalViewer(attach) = harness.carrier.take_obcs()? else {
        return Err(test_error("the surviving Tab must AttachBinding"));
    };
    assert_eq!(
        attach.action(),
        &TerminalViewerAction::AttachBinding {
            session_id: ObcsSessionId::parse(&id(100))?,
            worker_id: ObcsWorkerId::parse(&id(1_003))?,
            terminal_id: ObcsTerminalId::parse(&id(1_004))?,
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
    drive_controlling_after_attach(&mut harness, installed.anchor(), geometry)?;
    let state = harness.ui.state();
    assert_eq!(state.phase_kind(), ClientPhaseKind::Ready);
    assert!(state.selected_has_viewer());
    let active = state.active().unwrap();
    assert_eq!(active.index(), 0);
    assert_eq!(active.panel_index(), 0);
    assert_eq!(state.cache().revision(), WorkbenchRevision::new(5));
    assert_eq!(state.cache().tabs().len(), 1);
    assert_eq!(
        state.tab(0).unwrap().panel(0).unwrap().panel_id(),
        &WorkbenchPanelId::parse(&id(1_001))?
    );
    assert_eq!(
        state.retained_panel_for_test().unwrap().operation(),
        &split,
        "a refused operation retains the earlier split commit, not a fabricated one"
    );
    assert!(state.invariant_holds());
    harness.finish()
}

// A Rejected outcome other than TabUnavailable is a Precommit result: the executor
// does not issue a superseding read, so the script ends at the command and the
// settlement applies the refusal notice directly.
#[test]
fn refused_move_rejected_panel_unavailable_settles_without_a_read() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(steps(&[WireKind::Workbench]), geometry)?;
    let mut initial = snapshot(2, 1);
    initial.terminal_contexts[0].tabs[0].root = split_node(
        panel_node(1_001, 1_002, 1_003, 1_004, "1"),
        3_001,
        panel_node(3_010, 3_013, 3_011, 3_012, "peer"),
        3_014,
        PanelOrientation::Horizontal,
        3_020,
    );
    let (_key, _controller) =
        drive_controlling_ready_from_snapshot(&mut harness, initial, geometry)?;
    let operation = harness.ui.admit_panel(PanelRequestKind::Move {
        direction: crate::client_view::PanelDirection::Right,
    })?;
    respond_command(
        &harness,
        &operation,
        WorkbenchCommandOutcome::Rejected {
            revision: WorkbenchRevision::new(2),
            reason: WorkbenchRejectionReason::PanelUnavailable,
        },
    )?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Panel(PanelDecision::Continue)
    ));
    let state = harness.ui.state();
    assert_eq!(state.phase_kind(), ClientPhaseKind::Ready);
    assert_eq!(state.cache().revision(), WorkbenchRevision::new(1));
    assert_eq!(
        state.notice(),
        Some(ClientNotice::PanelNotApplied),
        "the refused settlement surfaces the not-applied notice"
    );
    assert!(
        state.retained_panel_for_test().is_none(),
        "a refused operation has no commit to retain"
    );
    assert!(state.invariant_holds());
    harness.finish()
}

/// How the reconcile-owned peer's `Acquire` settles: a real request that
/// either wins a controller or is refused busy and converges read-only.
enum PeerAcquire {
    Controlling,
    BusyReadOnly,
}

/// Shared reconcile prelude: the second leaf is observed, the focus pane's
/// first-observation Resize runs first, then the reconcile lease drives the
/// peer's real first-frame setup chain — Attach, Acquire, (controlling only:
/// setup Resize + `SetupConfigured`; busy: read-only `SetupConfigured`), the
/// setup Poll, the actual first draw, and the feedback response. Returns the
/// peer panel id, its distinct viewer id, and its controller when acquired.
fn drive_reconcile_peer_first_frame(
    harness: &mut Harness,
    geometry: Geometry,
    acquire: PeerAcquire,
) -> TestResult<(
    WorkbenchPanelId,
    orchbee_client_server_protocol::ViewerId,
    Option<ControllerId>,
)> {
    let focus_panel = harness.ui.state().focus_panel_id().unwrap().clone();
    let peer_panel = WorkbenchPanelId::parse(&id(3_010))?;
    harness.ui.observe_panes(&[
        (focus_panel, 0, geometry),
        (peer_panel.clone(), 1, geometry),
    ])?;
    let ClientMessage::TerminalControl(resize) = harness.carrier.take_obcs()? else {
        return Err(test_error("first observation must Resize the focus pane"));
    };
    let TerminalControlAction::Resize {
        attachment_id,
        controller_id: resize_controller,
        sequence,
        geometry: requested,
    } = resize.action()
    else {
        return Err(test_error("first observation must Resize the focus pane"));
    };
    assert_eq!(attachment_id, &attached_viewer_id()?);
    assert_eq!(resize_controller, &controller_id()?);
    assert_eq!(sequence.get(), 2);
    assert_eq!(*requested, geometry);
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalControlResult(
            TerminalControlResult::new(
                resize.request_id().clone(),
                TerminalControlOutcome::Resized { geometry },
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::SetupConfigured {
            mode: ControllerMode::Controlling(_),
            applied_geometry: Some(actual),
            ..
        } if actual == geometry
    ));

    // The reconcile lease attaches the peer under its snapshot binding and
    // runs the same setup wire sequence as a focus attach.
    let ClientMessage::TerminalViewer(attach) = harness.carrier.take_obcs()? else {
        return Err(test_error("reconcile must AttachBinding the peer"));
    };
    assert_eq!(
        attach.action(),
        &TerminalViewerAction::AttachBinding {
            session_id: ObcsSessionId::parse(&id(100))?,
            worker_id: ObcsWorkerId::parse(&id(3_011))?,
            terminal_id: ObcsTerminalId::parse(&id(3_012))?,
        }
    );
    // A legitimate Server mints a distinct attachment for a distinct pane:
    // the peer attaches under its own viewer identity, on the same host epoch.
    let peer_viewer_id = attached_viewer_id_for(PEER_VIEWER_ID)?;
    assert_ne!(peer_viewer_id, attached_viewer_id()?);
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                attach.request_id().clone(),
                attached_outcome_for(PEER_VIEWER_ID)?,
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Attached { .. }
    ));

    let ClientMessage::TerminalControl(acquire_request) = harness.carrier.take_obcs()? else {
        return Err(test_error("peer attach must be followed by Acquire"));
    };
    assert_eq!(
        acquire_request.action(),
        &TerminalControlAction::Acquire {
            attachment_id: peer_viewer_id.clone(),
        }
    );
    let peer_controller = match acquire {
        PeerAcquire::Controlling => Some(ControllerId::parse(&id(3_015))?),
        PeerAcquire::BusyReadOnly => None,
    };
    let acquire_outcome = match &peer_controller {
        Some(controller) => TerminalControlOutcome::Acquired {
            controller_id: controller.clone(),
        },
        None => TerminalControlOutcome::Rejected {
            reason: orchbee_client_server_protocol::ControlRejectedReason::ControllerBusy,
        },
    };
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalControlResult(
            TerminalControlResult::new(acquire_request.request_id().clone(), acquire_outcome),
        ))?;
    if let Some(controller) = &peer_controller {
        let ClientMessage::TerminalControl(resize) = harness.carrier.take_obcs()? else {
            return Err(test_error("controlling peer setup must Resize"));
        };
        let TerminalControlAction::Resize {
            attachment_id,
            controller_id: resize_controller,
            sequence,
            geometry: requested,
        } = resize.action()
        else {
            return Err(test_error("controlling peer setup must Resize"));
        };
        assert_eq!(attachment_id, &peer_viewer_id);
        assert_eq!(resize_controller, controller);
        assert_eq!(sequence.get(), 1);
        assert_eq!(*requested, geometry);
        harness
            .carrier
            .respond_obcs(ServerMessage::TerminalControlResult(
                TerminalControlResult::new(
                    resize.request_id().clone(),
                    TerminalControlOutcome::Resized { geometry },
                ),
            ))?;
        assert!(matches!(
            harness.ui.settle_next(FrameCut::None)?,
            UiEvent::SetupConfigured {
                mode: ControllerMode::Controlling(ref actual),
                applied_geometry: Some(actual_geometry),
                ..
            } if actual == controller && actual_geometry == geometry
        ));
    } else {
        assert!(matches!(
            harness.ui.settle_next(FrameCut::None)?,
            UiEvent::SetupConfigured {
                mode: ControllerMode::ReadOnly,
                applied_geometry: None,
                ..
            }
        ));
    }

    // The peer's setup poll produces its actual first frame draw while still
    // the background pane; the driver's latest-installed projection routes
    // the record by exact `PaneRef`, the same routing `WorkbenchClientUi::after_draw`
    // applies for a non-focus leaf.
    let ClientMessage::TerminalViewer(poll) = harness.carrier.take_obcs()? else {
        return Err(test_error("peer setup must Poll"));
    };
    assert_eq!(
        poll.action(),
        &TerminalViewerAction::Poll {
            attachment_id: peer_viewer_id.clone(),
        }
    );
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                poll.request_id().clone(),
                TerminalViewerOutcome::Record(full_frame_record_for(
                    PEER_VIEWER_ID,
                    1,
                    1,
                    geometry,
                )?),
            ),
        ))?;
    let feedback = match harness.ui.settle_next(FrameCut::None)? {
        UiEvent::FrameDrawn {
            feedback: feedback @ TerminalViewerAction::FrameApplied { .. },
            ..
        } => feedback,
        _ => return Err(test_error("peer setup first frame must draw")),
    };
    let ClientMessage::TerminalViewer(feedback_request) = harness.carrier.take_obcs()? else {
        return Err(test_error("peer setup draw must send feedback"));
    };
    assert_eq!(feedback_request.action(), &feedback);
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                feedback_request.request_id().clone(),
                TerminalViewerOutcome::FeedbackAccepted {},
            ),
        ))?;
    Ok((peer_panel, peer_viewer_id, peer_controller))
}

/// The interleave window the ReadyCandidate defect needed: the executor is
/// parked between the deposited `FeedbackSettled` reply and its own Ready
/// publication check while the caller's in-window actions land. A correct
/// executor then treats the reconcile setup as record-only completion — no
/// `ReadyCandidate` — and moves on to ordinary command work.
fn settle_peer_feedback_with_window(
    harness: &mut Harness,
    pane_b_ref: &PaneRef,
    in_window: impl FnOnce(&mut Harness) -> TestResult,
) -> TestResult {
    let mailbox = Arc::clone(&harness.mailbox);
    let checkpoint = mailbox.pause_next_ui_reply_resume_for_test();
    let taken = harness.ui.take_next()?;
    let WorkerEvent::FeedbackAccepted { key, .. } = taken.event() else {
        return Err(test_error("peer setup must publish FeedbackAccepted"));
    };
    assert_eq!(key, pane_b_ref);
    assert!(matches!(
        harness.ui.settle_taken(taken, FrameCut::None)?,
        UiEvent::FeedbackAccepted { .. }
    ));
    if !checkpoint.await_reached_before(Instant::now() + WALL_GUARD) {
        return Err(test_error(
            "worker did not pause after receiving the FeedbackSettled reply",
        ));
    }
    in_window(harness)?;
    drop(checkpoint);
    Ok(())
}

/// Asserts the admitted input reaches the carrier as a terminal `Input` on
/// exactly `viewer_id`/`controller`/`sequence` — the routing proof that a
/// surviving executor delivered it to the right incarnation.
fn assert_wire_input(
    request: &orchbee_client_server_protocol::TerminalControlRequest,
    viewer_id: &orchbee_client_server_protocol::ViewerId,
    controller: &ControllerId,
    expected_sequence: u64,
    expected_input: &ControlInput,
) -> TestResult {
    let TerminalControlAction::Input {
        attachment_id,
        controller_id,
        sequence,
        bytes,
    } = request.action()
    else {
        return Err(test_error("expected terminal Input"));
    };
    assert_eq!(attachment_id, viewer_id);
    assert_eq!(controller_id, controller);
    assert_eq!(sequence.get(), expected_sequence);
    assert_eq!(bytes, expected_input);
    Ok(())
}

fn respond_input_accepted(
    harness: &Harness,
    request: &orchbee_client_server_protocol::TerminalControlRequest,
) -> TestResult {
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalControlResult(
            TerminalControlResult::new(
                request.request_id().clone(),
                TerminalControlOutcome::InputAccepted {},
            ),
        ))
}

/// A focus move to a converged reconcile peer inside the feedback window
/// must not push a second `ReadyCandidate` into the already-`Ready` phase:
/// the reconcile setup completes its own record silently, and the very next
/// wire request is the queued input on the peer's own attachment. Under the
/// defect the executor parked on the doomed `ReadyCandidate` publish and the
/// input could never reach the carrier.
#[test]
fn reconcile_setup_focus_move_during_ready_check_completes_without_ready() -> TestResult {
    let geometry = fixture_geometry()?;
    // 7 post-Ready steps: focus Resize + the peer's
    // Attach/Acquire/Resize/Poll/feedback + the queued Input.
    let mut harness = Harness::start(steps(&[WireKind::Obcs; 7]), geometry)?;
    let (_anchor_a, controller_a) =
        drive_controlling_ready_from_snapshot(&mut harness, split_snapshot(1), geometry)?;
    let pane_a = harness.ui.state().desired().cloned().unwrap();
    let pane_a_ref = harness
        .ui
        .state()
        .pane_ref_for(&pane_a)
        .ok_or_else(|| test_error("focus pane record must be live"))?;
    let (peer_panel, peer_viewer_id, peer_controller) =
        drive_reconcile_peer_first_frame(&mut harness, geometry, PeerAcquire::Controlling)?;
    let peer_controller = peer_controller.ok_or_else(|| test_error("controlling peer"))?;
    let pane_b = harness
        .ui
        .state()
        .plan_panel_focus_local_at(&peer_panel)
        .ok_or_else(|| test_error("configured peer must pass the local focus plan"))?;
    let pane_b_ref = harness
        .ui
        .state()
        .pane_ref_for(&pane_b)
        .ok_or_else(|| test_error("peer pane record must be live"))?;

    let input = ControlInput::new(vec![b'x'])?;
    settle_peer_feedback_with_window(&mut harness, &pane_b_ref, |harness| {
        // A real local focus move to the peer — plan, install, and desired
        // republish — plus the queued input land inside the window.
        let moved = harness.ui.move_panel_focus_to_pane(&peer_panel)?;
        assert_eq!(moved, pane_b);
        assert_eq!(harness.ui.state().desired(), Some(&pane_b));
        assert_eq!(harness.ui.state().phase_kind(), ClientPhaseKind::Ready);
        harness.ui.admit_input(input.clone())?;
        Ok(())
    })?;

    // Survival is proven by the wire, not by probing an empty event queue:
    // the next actual request must be the admitted Input on the peer's own
    // attachment and controller — impossible had a doomed `ReadyCandidate`
    // publish parked the worker first.
    let ClientMessage::TerminalControl(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("queued input must reach the carrier"));
    };
    assert_wire_input(&request, &peer_viewer_id, &peer_controller, 2, &input)?;
    respond_input_accepted(&harness, &request)?;

    assert_eq!(harness.mailbox.is_closed(), None);
    assert_eq!(harness.ui.state().phase_kind(), ClientPhaseKind::Ready);
    assert!(harness.ui.state().pane_attached(&pane_b_ref));
    assert!(harness.ui.state().pollable_panes().contains(&pane_b_ref));
    // Pane A kept its exact live incarnation and attachment.
    assert_eq!(
        harness.ui.state().pane_ref_for(&pane_a).as_ref(),
        Some(&pane_a_ref)
    );
    assert!(harness.ui.state().pane_attached(&pane_a_ref));
    let _ = controller_a;
    harness.finish()
}

/// The same reconcile setup completes with focus never leaving pane A: no
/// `ReadyCandidate` is published and ordinary input to pane A flows on its
/// own attachment afterwards.
#[test]
fn reconcile_setup_focus_stays_completes_without_ready() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(steps(&[WireKind::Obcs; 7]), geometry)?;
    let (_anchor_a, controller_a) =
        drive_controlling_ready_from_snapshot(&mut harness, split_snapshot(1), geometry)?;
    let pane_a = harness.ui.state().desired().cloned().unwrap();
    let pane_a_ref = harness
        .ui
        .state()
        .pane_ref_for(&pane_a)
        .ok_or_else(|| test_error("focus pane record must be live"))?;
    let (peer_panel, _peer_viewer_id, _peer_controller) =
        drive_reconcile_peer_first_frame(&mut harness, geometry, PeerAcquire::Controlling)?;
    let pane_b = harness
        .ui
        .state()
        .plan_panel_focus_local_at(&peer_panel)
        .ok_or_else(|| test_error("configured peer must pass the local focus plan"))?;
    let pane_b_ref = harness
        .ui
        .state()
        .pane_ref_for(&pane_b)
        .ok_or_else(|| test_error("peer pane record must be live"))?;

    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::FeedbackAccepted { .. }
    ));
    assert_eq!(harness.ui.state().desired(), Some(&pane_a));
    assert_eq!(harness.ui.state().phase_kind(), ClientPhaseKind::Ready);

    let input = ControlInput::new(vec![b'x'])?;
    harness.ui.admit_input(input.clone())?;
    let ClientMessage::TerminalControl(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("queued input must reach the carrier"));
    };
    assert_wire_input(&request, &attached_viewer_id()?, &controller_a, 3, &input)?;
    respond_input_accepted(&harness, &request)?;

    assert_eq!(harness.mailbox.is_closed(), None);
    assert!(harness.ui.state().pane_attached(&pane_b_ref));
    assert!(harness.ui.state().pollable_panes().contains(&pane_b_ref));
    assert_eq!(
        harness.ui.state().pane_ref_for(&pane_a).as_ref(),
        Some(&pane_a_ref)
    );
    harness.finish()
}

/// Focus may move A→B inside the window and back B→A afterwards: the
/// reconcile setup still completes record-only, the phase stays `Ready`,
/// and pane A's control and exact incarnation keep working.
#[test]
fn reconcile_setup_focus_round_trip_completes_without_ready() -> TestResult {
    let geometry = fixture_geometry()?;
    // 8 post-Ready steps: focus Resize + peer Attach/Acquire/Resize/Poll/
    // feedback + Input to B + Input back to A.
    let mut harness = Harness::start(steps(&[WireKind::Obcs; 8]), geometry)?;
    let (_anchor_a, controller_a) =
        drive_controlling_ready_from_snapshot(&mut harness, split_snapshot(1), geometry)?;
    let pane_a = harness.ui.state().desired().cloned().unwrap();
    let pane_a_ref = harness
        .ui
        .state()
        .pane_ref_for(&pane_a)
        .ok_or_else(|| test_error("focus pane record must be live"))?;
    let focus_panel = harness.ui.state().focus_panel_id().unwrap().clone();
    let (peer_panel, peer_viewer_id, peer_controller) =
        drive_reconcile_peer_first_frame(&mut harness, geometry, PeerAcquire::Controlling)?;
    let peer_controller = peer_controller.ok_or_else(|| test_error("controlling peer"))?;
    let pane_b = harness
        .ui
        .state()
        .plan_panel_focus_local_at(&peer_panel)
        .ok_or_else(|| test_error("configured peer must pass the local focus plan"))?;
    let pane_b_ref = harness
        .ui
        .state()
        .pane_ref_for(&pane_b)
        .ok_or_else(|| test_error("peer pane record must be live"))?;

    let input_b = ControlInput::new(vec![b'b'])?;
    settle_peer_feedback_with_window(&mut harness, &pane_b_ref, |harness| {
        let moved = harness.ui.move_panel_focus_to_pane(&peer_panel)?;
        assert_eq!(moved, pane_b);
        harness.ui.admit_input(input_b.clone())?;
        Ok(())
    })?;
    let ClientMessage::TerminalControl(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("queued input to B must reach the carrier"));
    };
    assert_wire_input(&request, &peer_viewer_id, &peer_controller, 2, &input_b)?;
    respond_input_accepted(&harness, &request)?;

    // The return leg moves focus back to A by direct panel identity and an
    // input lands on A's own attachment — its incarnation and controller
    // were preserved through the round trip.
    let input_a = ControlInput::new(vec![b'a'])?;
    let moved = harness.ui.move_panel_focus_to_pane(&focus_panel)?;
    assert_eq!(moved, pane_a);
    assert_eq!(harness.ui.state().desired(), Some(&pane_a));
    harness.ui.admit_input(input_a.clone())?;
    let ClientMessage::TerminalControl(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("queued input back to A must reach the carrier"));
    };
    assert_wire_input(&request, &attached_viewer_id()?, &controller_a, 3, &input_a)?;
    respond_input_accepted(&harness, &request)?;

    assert_eq!(harness.mailbox.is_closed(), None);
    assert_eq!(harness.ui.state().phase_kind(), ClientPhaseKind::Ready);
    assert_eq!(
        harness.ui.state().pane_ref_for(&pane_a).as_ref(),
        Some(&pane_a_ref)
    );
    assert!(harness.ui.state().pane_attached(&pane_a_ref));
    assert!(harness.ui.state().pane_attached(&pane_b_ref));
    harness.finish()
}

/// A peer whose `Acquire` is refused `ControllerBusy` converges read-only:
/// its record still completes the same first-frame chain without a second
/// `ReadyCandidate`, joins the ordinary poll rotation, and never takes
/// control — an admitted input to it is consumed without a wire request,
/// while pane A keeps working on its own attachment.
#[test]
fn reconcile_readonly_setup_focus_move_completes_without_ready() -> TestResult {
    let geometry = fixture_geometry()?;
    // 6 post-Ready steps: focus Resize + peer Attach/Acquire(busy)/Poll/
    // feedback + Input back to A. An input to the read-only B produces no
    // wire request, so the carrier's next observed request must be A's.
    let mut harness = Harness::start(steps(&[WireKind::Obcs; 6]), geometry)?;
    let (_anchor_a, controller_a) =
        drive_controlling_ready_from_snapshot(&mut harness, split_snapshot(1), geometry)?;
    let pane_a = harness.ui.state().desired().cloned().unwrap();
    let pane_a_ref = harness
        .ui
        .state()
        .pane_ref_for(&pane_a)
        .ok_or_else(|| test_error("focus pane record must be live"))?;
    let focus_panel = harness.ui.state().focus_panel_id().unwrap().clone();
    let (peer_panel, _peer_viewer_id, peer_controller) =
        drive_reconcile_peer_first_frame(&mut harness, geometry, PeerAcquire::BusyReadOnly)?;
    assert!(peer_controller.is_none());
    let pane_b = harness
        .ui
        .state()
        .plan_panel_focus_local_at(&peer_panel)
        .ok_or_else(|| test_error("configured read-only peer must pass the focus plan"))?;
    let pane_b_ref = harness
        .ui
        .state()
        .pane_ref_for(&pane_b)
        .ok_or_else(|| test_error("peer pane record must be live"))?;

    let input_b = ControlInput::new(vec![b'b'])?;
    let input_a = ControlInput::new(vec![b'a'])?;
    settle_peer_feedback_with_window(&mut harness, &pane_b_ref, |harness| {
        // Focus may land on the read-only peer — it is a settled mode — but
        // it never gains control, so its admitted input is consumed with no
        // wire request; the return leg queues A's input behind it.
        let moved = harness.ui.move_panel_focus_to_pane(&peer_panel)?;
        assert_eq!(moved, pane_b);
        harness.ui.admit_input(input_b.clone())?;
        let moved = harness.ui.move_panel_focus_to_pane(&focus_panel)?;
        assert_eq!(moved, pane_a);
        harness.ui.admit_input(input_a.clone())?;
        Ok(())
    })?;

    // Command order is deterministic: the carrier's next observed request is
    // A's Input — had the read-only peer's input wrongly reached the wire it
    // would arrive first on the peer attachment and fail this assertion.
    let ClientMessage::TerminalControl(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("queued input back to A must reach the carrier"));
    };
    assert_wire_input(&request, &attached_viewer_id()?, &controller_a, 3, &input_a)?;
    respond_input_accepted(&harness, &request)?;

    assert_eq!(harness.mailbox.is_closed(), None);
    assert_eq!(harness.ui.state().phase_kind(), ClientPhaseKind::Ready);
    // The read-only peer is a settled record: attached, projected, and
    // inside the ordinary poll admission without ever taking control.
    assert!(harness.ui.state().pane_attached(&pane_b_ref));
    assert!(harness.ui.state().pollable_panes().contains(&pane_b_ref));
    assert_eq!(
        harness.ui.state().pane_ref_for(&pane_a).as_ref(),
        Some(&pane_a_ref)
    );
    harness.finish()
}

/// The `Ctrl+B z` trace end to end: the volatile flag flip emits no wire
/// request, both viewers stay attached with their exact incarnations, and
/// resize admits only focus while zoomed, then restores both visible peers.
#[test]
fn zoom_trace_keeps_peers_alive_and_resizes_only_visible_controlling_panes() -> TestResult {
    let canonical = fixture_geometry()?;
    let zoomed_wide = Geometry::new(
        canonical.columns().checked_add(8).unwrap(),
        canonical.rows(),
        canonical.cell_pixel_width(),
        canonical.cell_pixel_height(),
    )?;
    // 10 post-Ready steps: focus Resize + the peer chain's five + two zoom
    // resizes + both visible leaves restored. A flag flip emits nothing, so the
    // script itself proves the toggle is wire-free.
    let mut harness = Harness::start(steps(&[WireKind::Obcs; 10]), canonical)?;
    let (_anchor_a, _controller_a) =
        drive_controlling_ready_from_snapshot(&mut harness, split_snapshot(1), canonical)?;
    let pane_a = harness.ui.state().desired().cloned().unwrap();
    let pane_a_ref = harness
        .ui
        .state()
        .pane_ref_for(&pane_a)
        .ok_or_else(|| test_error("focus pane record must be live"))?;
    let focus_panel = harness.ui.state().focus_panel_id().unwrap().clone();
    let (peer_panel, peer_viewer_id, peer_controller) =
        drive_reconcile_peer_first_frame(&mut harness, canonical, PeerAcquire::Controlling)?;
    let peer_controller = peer_controller.ok_or_else(|| test_error("controlling peer"))?;
    let pane_b = harness
        .ui
        .state()
        .plan_panel_focus_local_at(&peer_panel)
        .ok_or_else(|| test_error("configured peer must pass the local focus plan"))?;
    let pane_b_ref = harness
        .ui
        .state()
        .pane_ref_for(&pane_b)
        .ok_or_else(|| test_error("peer pane record must be live"))?;
    // The helper leaves the peer's feedback event unsettled; consume it so
    // the mailbox admits the resizes the zoom legs queue.
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::FeedbackAccepted { .. }
    ));

    // Zoom: local flag only — every incarnation stays live and attached.
    assert!(harness.ui.toggle_zoom());
    assert!(harness.ui.state().zoom_active());
    assert!(harness.ui.state().pane_attached(&pane_a_ref));
    assert!(harness.ui.state().pane_attached(&pane_b_ref));
    assert!(harness.ui.state().pollable_panes().contains(&pane_b_ref));

    // The zoomed observation still reports both leaves; only the focused
    // pane's whole-area report is admitted — one Resize on pane A's own
    // attachment, nothing on the hidden peer's.
    harness.ui.observe_panes(&[
        (focus_panel.clone(), 0, zoomed_wide),
        (peer_panel.clone(), 1, canonical),
    ])?;
    let ClientMessage::TerminalControl(resize) = harness.carrier.take_obcs()? else {
        return Err(test_error("zoomed observation must Resize the focus pane"));
    };
    let TerminalControlAction::Resize {
        attachment_id,
        geometry: requested,
        ..
    } = resize.action()
    else {
        return Err(test_error("zoomed observation must Resize the focus pane"));
    };
    assert_eq!(attachment_id, &attached_viewer_id()?);
    assert_eq!(*requested, zoomed_wide);
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalControlResult(
            TerminalControlResult::new(
                resize.request_id().clone(),
                TerminalControlOutcome::Resized {
                    geometry: zoomed_wide,
                },
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::SetupConfigured { .. }
    ));

    // Focus follows the hidden peer: the move is local, and the next
    // observation admits the peer's whole-area report on ITS attachment.
    let moved = harness.ui.move_panel_focus_to_pane(&peer_panel)?;
    assert_eq!(moved, pane_b);
    assert!(harness.ui.state().zoom_active());
    harness.ui.observe_panes(&[
        (focus_panel.clone(), 0, canonical),
        (peer_panel.clone(), 1, zoomed_wide),
    ])?;
    let ClientMessage::TerminalControl(resize) = harness.carrier.take_obcs()? else {
        return Err(test_error("focus-moved observation must Resize the peer"));
    };
    let TerminalControlAction::Resize {
        attachment_id,
        controller_id: resize_controller,
        geometry: requested,
        ..
    } = resize.action()
    else {
        return Err(test_error("focus-moved observation must Resize the peer"));
    };
    assert_eq!(attachment_id, &peer_viewer_id);
    assert_eq!(resize_controller, &peer_controller);
    assert_eq!(*requested, zoomed_wide);
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalControlResult(
            TerminalControlResult::new(
                resize.request_id().clone(),
                TerminalControlOutcome::Resized {
                    geometry: zoomed_wide,
                },
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::SetupConfigured { .. }
    ));

    // Restore both visible controlling leaves, each on its own frozen attachment.
    assert!(harness.ui.toggle_zoom());
    assert!(!harness.ui.state().zoom_active());
    harness.ui.observe_panes(&[
        (focus_panel.clone(), 0, canonical),
        (peer_panel.clone(), 1, canonical),
    ])?;
    for expected_viewer in [attached_viewer_id()?, peer_viewer_id.clone()] {
        let ClientMessage::TerminalControl(resize) = harness.carrier.take_obcs()? else {
            return Err(test_error(
                "restored observation must resize each visible leaf",
            ));
        };
        let TerminalControlAction::Resize {
            attachment_id,
            geometry: requested,
            ..
        } = resize.action()
        else {
            return Err(test_error("restore must be Resize"));
        };
        assert_eq!(attachment_id, &expected_viewer);
        assert_eq!(*requested, canonical);
        harness
            .carrier
            .respond_obcs(ServerMessage::TerminalControlResult(
                TerminalControlResult::new(
                    resize.request_id().clone(),
                    TerminalControlOutcome::Resized {
                        geometry: canonical,
                    },
                ),
            ))?;
        assert!(matches!(
            harness.ui.settle_next(FrameCut::None)?,
            UiEvent::SetupConfigured { .. }
        ));
    }

    assert_eq!(harness.mailbox.is_closed(), None);
    assert_eq!(harness.ui.state().phase_kind(), ClientPhaseKind::Ready);
    // Both incarnations survived the whole zoom session bit for bit.
    assert!(harness.ui.state().pane_attached(&pane_a_ref));
    assert!(harness.ui.state().pane_attached(&pane_b_ref));
    assert_eq!(
        harness.ui.state().pane_ref_for(&pane_a).as_ref(),
        Some(&pane_a_ref)
    );
    assert_eq!(
        harness.ui.state().pane_ref_for(&pane_b).as_ref(),
        Some(&pane_b_ref)
    );
    harness.finish()
}
