use super::{
    commands::{drive_controlling_after_attach, drive_controlling_ready_from_snapshot},
    setup::*,
};
use crate::terminal_session::{
    mailbox::{QueuedCommand, Tagged, WorkerEvent},
    state::{
        CarrierFailure, MoveDecision, MoveOperation, MoveResult, RefreshDecision, SessionPhaseKind,
    },
};
use orchbee_client_server_protocol::{
    ClientMessage, ControlInput, Geometry, ServerMessage, TerminalControlAction,
    TerminalControlOutcome, TerminalControlResult, TerminalViewerAction, TerminalViewerOutcome,
    TerminalViewerResult,
};
use orchbee_workbench_protocol::{
    ProtocolVersion, Snapshot, WorkbenchAction, WorkbenchCommandOutcome, WorkbenchCommandResult,
    WorkbenchRequest, WorkbenchResponse, WorkbenchRevision, WorkbenchSnapshotOutcome,
    WorkbenchSnapshotResult, WorkbenchSnapshotUnavailableReason, WorkbenchTabMoveDirection,
};
use std::sync::atomic::Ordering;

#[test]
fn actual_move_membership_detaches_then_installs_empty_or_real_ready_and_discards_queued_old_input()
-> TestResult {
    for neighbor in [false, true] {
        drive_move_membership_with_queued_old_controls(neighbor, false)?;
    }
    Ok(())
}

#[test]
fn actual_move_empty_discards_queued_old_resize() -> TestResult {
    drive_move_membership_with_queued_old_controls(false, true)
}

#[test]
fn actual_move_successor_ready_discards_queued_old_resize() -> TestResult {
    drive_move_membership_with_queued_old_controls(true, true)
}

fn drive_move_membership_with_queued_old_controls(neighbor: bool, resize: bool) -> TestResult {
    let geometry = fixture_geometry()?;
    let latest = if resize {
        Geometry::new(
            geometry.columns(),
            geometry.rows(),
            geometry.cell_pixel_width() + 1,
            geometry.cell_pixel_height(),
        )?
    } else {
        geometry
    };
    let mut kinds = steps(&[WireKind::Workbench, WireKind::Workbench, WireKind::Obcs]);
    if neighbor {
        kinds.extend([WireKind::Obcs; 5]);
    }
    kinds.push(WireKind::Workbench); // FIFO witness after the old queued controls.
    let mut harness = Harness::start(kinds, geometry)?;
    let (old, _) = drive_controlling_ready_from_snapshot(&mut harness, snapshot(2, 1), geometry)?;
    let baseline = harness.ui.state().cache().clone();
    let operation = harness.ui.admit_move(WorkbenchTabMoveDirection::Last)?;
    harness
        .ui
        .admit_input(ControlInput::new(b"stale-after-move".to_vec())?)?;
    if resize {
        harness.ui.admit_resize(latest)?;
    }
    commit_move(&harness, &operation)?;
    let mut candidate = snapshot(if neighbor { 2 } else { 0 }, 3);
    if neighbor {
        candidate.terminal_contexts[0].tabs.remove(0);
    }
    respond_snapshot(
        &harness,
        WorkbenchSnapshotOutcome::Available {
            snapshot: candidate.clone(),
        },
    )?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Move(MoveDecision::Cleanup(_))
    ));
    assert_eq!(harness.ui.state().cache(), &baseline);
    assert_eq!(harness.ui.state().attached_key(), Some(&old));
    let ClientMessage::TerminalViewer(detach) = harness.carrier.take_obcs()? else {
        return Err(test_error("exact old Detach"));
    };
    assert_eq!(detach.action(), &TerminalViewerAction::Detach {});
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                detach.request_id().clone(),
                TerminalViewerOutcome::Detached {},
            ),
        ))?;
    let UiEvent::Membership(decision) = harness.ui.settle_next(FrameCut::None)? else {
        return Err(test_error("Move common cleanup settlement"));
    };
    if neighbor {
        let RefreshDecision::AttachRecovered(target) = decision else {
            return Err(test_error("successor Attach"));
        };
        assert_eq!(target.selected().key().generation(), old.generation() + 1);
        let next = drive_attach(&mut harness, 1)?;
        drive_controlling_after_attach(&mut harness, &next, latest)?;
        assert!(harness.ui.state().ready().is_some());
    } else {
        assert_eq!(decision, RefreshDecision::SettledWithoutAttachment);
        assert_eq!(harness.ui.state().phase_kind(), SessionPhaseKind::Empty);
        assert_eq!(harness.ui.state().desired(), None);
    }
    assert_eq!(
        harness
            .ui
            .state()
            .retained_move_for_test()
            .unwrap()
            .operation(),
        &operation
    );
    harness.ui.admit_refresh()?;
    respond_snapshot(&harness, WorkbenchSnapshotOutcome::Denied {})?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Refresh { .. }
    ));
    assert!(harness.ui.state().invariant_holds());
    harness.finish()?;
    Ok(())
}

#[test]
fn queued_input_before_switch_still_uses_its_owned_controller_after_desired_advances() -> TestResult
{
    drive_queued_control_before_switch(false)
}

#[test]
fn queued_resize_before_switch_still_uses_its_owned_controller_after_desired_advances() -> TestResult
{
    drive_queued_control_before_switch(true)
}

fn drive_queued_control_before_switch(resize: bool) -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(steps(&[WireKind::Obcs, WireKind::Obcs]), geometry)?;
    let (old, controller) =
        drive_controlling_ready_from_snapshot(&mut harness, snapshot(2, 1), geometry)?;
    harness
        .mailbox
        .try_enqueue(QueuedCommand::Poll(Tagged::new(old.clone(), ())))
        .map_err(|error| test_error(&format!("Poll admission: {error:?}")))?;
    let ClientMessage::TerminalViewer(poll) = harness.carrier.take_obcs()? else {
        return Err(test_error("blocked old Poll"));
    };
    assert_eq!(poll.action(), &TerminalViewerAction::Poll {});
    let latest = Geometry::new(
        geometry.columns(),
        geometry.rows(),
        geometry.cell_pixel_width() + 1,
        geometry.cell_pixel_height(),
    )?;
    if resize {
        harness.ui.admit_resize(latest)?;
    } else {
        harness
            .ui
            .admit_input(ControlInput::new(b"before-switch".to_vec())?)?;
    }
    let next = harness.ui.admit_switch(1)?;
    assert_ne!(next.key, old);
    assert_eq!(harness.ui.state().attached_key(), Some(&old));
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(poll.request_id().clone(), TerminalViewerOutcome::Empty {}),
        ))?;
    let ClientMessage::TerminalControl(control) = harness.carrier.take_obcs()? else {
        return Err(test_error("pre-marker control must drain"));
    };
    let outcome = if resize {
        assert!(
            matches!(control.action(), TerminalControlAction::Resize {controller_id,sequence,geometry}
            if controller_id == &controller && sequence.get() == 2 && *geometry == latest)
        );
        TerminalControlOutcome::Resized { geometry: latest }
    } else {
        assert!(
            matches!(control.action(), TerminalControlAction::Input {controller_id,sequence,bytes}
            if controller_id == &controller && sequence.get() == 2 && bytes.as_bytes() == b"before-switch")
        );
        TerminalControlOutcome::InputAccepted {}
    };
    // End this bounded witness before the independently owned Switch continuation.
    harness.mailbox.close(CarrierFailure::Closed);
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalControlResult(
            TerminalControlResult::new(control.request_id().clone(), outcome),
        ))?;
    harness.finish()
}

#[test]
fn explicit_move_recovery_reads_snapshot_only_and_retains_commit_through_empty_cleanup()
-> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(
        steps(&[
            WireKind::Workbench,
            WireKind::Workbench,
            WireKind::Workbench,
            WireKind::Obcs,
        ]),
        geometry,
    )?;
    let (old, _) = drive_controlling_ready_from_snapshot(&mut harness, snapshot(2, 1), geometry)?;
    let operation = harness.ui.admit_move(WorkbenchTabMoveDirection::Last)?;
    commit_move(&harness, &operation)?;
    respond_snapshot(&harness, WorkbenchSnapshotOutcome::Denied {})?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Move(MoveDecision::Continue)
    ));
    assert_eq!(
        harness.ui.state().phase_kind(),
        SessionPhaseKind::MoveProjectionUnavailable
    );
    assert_eq!(harness.ui.state().attached_key(), Some(&old));
    harness.ui.admit_refresh()?;
    respond_snapshot(
        &harness,
        WorkbenchSnapshotOutcome::Available {
            snapshot: snapshot(0, 3),
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
        return Err(test_error("recovery exact Detach"));
    };
    assert_eq!(detach.action(), &TerminalViewerAction::Detach {});
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
            decision: RefreshDecision::SettledWithoutAttachment,
            ..
        }
    ));
    assert_eq!(harness.ui.state().phase_kind(), SessionPhaseKind::Empty);
    assert_eq!(
        harness
            .ui
            .state()
            .retained_move_for_test()
            .unwrap()
            .operation(),
        &operation
    );
    harness.finish()
}

#[test]
fn snapshot_failure_taxonomy_preserves_io_and_protocol_causes() -> TestResult {
    use crate::local_client::workbench::{SnapshotReadError, SnapshotReadFailure};
    use orchbee_workbench_protocol::{WorkbenchRequestId, WorkbenchWireError};
    let request_id = WorkbenchRequestId::parse("00000000000000000000000000000001")?;
    for (reason, expected) in [
        (
            SnapshotReadFailure::Response(WorkbenchWireError::Io(std::io::Error::from(
                std::io::ErrorKind::TimedOut,
            ))),
            CarrierFailure::Deadline,
        ),
        (
            SnapshotReadFailure::Response(WorkbenchWireError::Io(std::io::Error::from(
                std::io::ErrorKind::BrokenPipe,
            ))),
            CarrierFailure::Transport,
        ),
        (
            SnapshotReadFailure::Response(WorkbenchWireError::InvalidHeader),
            CarrierFailure::Protocol,
        ),
        (
            SnapshotReadFailure::CorrelationMismatch {
                expected: request_id.clone(),
                received: WorkbenchRequestId::parse("00000000000000000000000000000002")?,
            },
            CarrierFailure::Protocol,
        ),
    ] {
        assert_eq!(
            super::super::snapshot(SnapshotReadError::Failed {
                request_id: request_id.clone(),
                reason
            }),
            expected
        );
    }
    Ok(())
}

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

fn commit_move(harness: &Harness, operation: &MoveOperation) -> TestResult {
    let WorkbenchRequest::Command(command) = harness.carrier.take_workbench()? else {
        return Err(test_error("Move must submit one command"));
    };
    assert_eq!(&command.request_id, operation.request_id());
    assert_eq!(command.expected_revision, operation.baseline().revision());
    assert_eq!(
        command.action,
        WorkbenchAction::TabMove {
            terminal_context_id: operation.baseline().terminal_context_id().clone(),
            tab_id: operation.target().tab_id().clone(),
            direction: operation.direction(),
        }
    );
    harness
        .carrier
        .respond_workbench(WorkbenchResponse::Command(WorkbenchCommandResult {
            protocol_version: ProtocolVersion,
            request_id: command.request_id,
            outcome: WorkbenchCommandOutcome::TabMoved {
                revision: WorkbenchRevision::new(2),
                terminal_context_id: operation.baseline().terminal_context_id().clone(),
                tab_id: operation.target().tab_id().clone(),
            },
        }))
}

fn respond_snapshot(harness: &Harness, outcome: WorkbenchSnapshotOutcome) -> TestResult {
    let WorkbenchRequest::SnapshotRead(read) = harness.carrier.take_workbench()? else {
        return Err(test_error(
            "known Move must read one Snapshot without OBCS effects",
        ));
    };
    harness
        .carrier
        .respond_workbench(WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
            protocol_version: ProtocolVersion,
            request_id: read.request_id,
            outcome,
        }))
}

fn reordered_snapshot() -> Snapshot {
    let mut result = snapshot(2, 2);
    result.terminal_contexts[0].tabs.swap(0, 1);
    result
}

#[test]
fn move_deposits_then_waits_past_deadline_before_same_controller_input() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(
        steps(&[WireKind::Workbench, WireKind::Workbench, WireKind::Obcs]),
        geometry,
    )?;
    let (key, controller) =
        drive_controlling_ready_from_snapshot(&mut harness, snapshot(2, 1), geometry)?;
    let frame = harness
        .ui
        .projection_frame()
        .map(|frame| (frame.route, frame.geometry, frame.rows.clone()));
    let operation = harness
        .ui
        .admit_move_transport(WorkbenchTabMoveDirection::Next)?;
    harness
        .ui
        .admit_input(ControlInput::new(b"after-move".to_vec())?)?;
    commit_move(&harness, &operation)?;
    respond_snapshot(
        &harness,
        WorkbenchSnapshotOutcome::Available {
            snapshot: reordered_snapshot(),
        },
    )?;
    let taken = harness.ui.take_next()?;
    assert!(taken.wait_key().is_some());
    harness.elapsed_millis.store(70_000, Ordering::SeqCst);
    harness.mailbox.wake_waiters();
    let (settled, outcome) = harness.ui.settle_move_transport(taken)?;
    assert_eq!(settled, operation);
    assert!(
        matches!(outcome, MoveResult::Committed {commit, outcome: WorkbenchSnapshotOutcome::Available {..}} if commit.revision == WorkbenchRevision::new(2) && &commit.tab_id == key.tab_id())
    );
    assert_eq!(
        harness.ui.projection_frame().map(|frame| (
            frame.route,
            frame.geometry,
            frame.rows.clone()
        )),
        frame
    );
    assert_eq!(harness.ui.state().desired().unwrap(), &key);
    let ClientMessage::TerminalControl(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("settled Move must resume Input"));
    };
    assert!(
        matches!(request.action(), TerminalControlAction::Input {controller_id, sequence, bytes} if controller_id == &controller && sequence.get() == 2 && bytes.as_bytes() == b"after-move")
    );
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalControlResult(
            TerminalControlResult::new(
                request.request_id().clone(),
                TerminalControlOutcome::InputAccepted {},
            ),
        ))?;
    harness.finish()
}

#[test]
fn move_noop_submits_no_snapshot_and_keeps_viewer() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(steps(&[WireKind::Workbench]), geometry)?;
    let (key, _controller) =
        drive_controlling_ready_from_snapshot(&mut harness, snapshot(2, 1), geometry)?;
    let operation = harness
        .ui
        .admit_move_transport(WorkbenchTabMoveDirection::First)?;
    let WorkbenchRequest::Command(command) = harness.carrier.take_workbench()? else {
        return Err(test_error("expected Move command"));
    };
    assert_eq!(&command.request_id, operation.request_id());
    harness
        .carrier
        .respond_workbench(WorkbenchResponse::Command(WorkbenchCommandResult {
            protocol_version: ProtocolVersion,
            request_id: command.request_id,
            outcome: WorkbenchCommandOutcome::Unchanged {
                revision: WorkbenchRevision::new(1),
            },
        }))?;
    let taken = harness.ui.take_next()?;
    let (_, outcome) = harness.ui.settle_move_transport(taken)?;
    assert!(
        matches!(outcome, MoveResult::Unchanged(revision) if revision == WorkbenchRevision::new(1))
    );
    assert!(harness.ui.state().selected_has_viewer());
    assert_eq!(harness.ui.state().desired().unwrap(), &key);
    harness.finish()
}

#[test]
fn move_definite_snapshot_unavailable_retains_commit_in_typed_result() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(steps(&[WireKind::Workbench, WireKind::Workbench]), geometry)?;
    let (key, _controller) =
        drive_controlling_ready_from_snapshot(&mut harness, snapshot(2, 1), geometry)?;
    let operation = harness
        .ui
        .admit_move_transport(WorkbenchTabMoveDirection::Last)?;
    commit_move(&harness, &operation)?;
    respond_snapshot(
        &harness,
        WorkbenchSnapshotOutcome::Unavailable {
            reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
        },
    )?;
    let taken = harness.ui.take_next()?;
    let (_, outcome) = harness.ui.settle_move_transport(taken)?;
    assert!(
        matches!(outcome, MoveResult::Committed {commit, outcome: WorkbenchSnapshotOutcome::Unavailable {..}} if &commit.tab_id == key.tab_id())
    );
    assert!(harness.ui.state().selected_has_viewer());
    harness.finish()
}

#[test]
fn move_tab_unavailable_reads_one_snapshot_without_retaining_a_commit() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(steps(&[WireKind::Workbench, WireKind::Workbench]), geometry)?;
    let (key, _) = drive_controlling_ready_from_snapshot(&mut harness, snapshot(2, 1), geometry)?;
    let operation = harness
        .ui
        .admit_move_transport(WorkbenchTabMoveDirection::Last)?;
    let WorkbenchRequest::Command(command) = harness.carrier.take_workbench()? else {
        return Err(test_error("one Move command"));
    };
    harness
        .carrier
        .respond_workbench(WorkbenchResponse::Command(WorkbenchCommandResult {
            protocol_version: ProtocolVersion,
            request_id: command.request_id,
            outcome: WorkbenchCommandOutcome::Rejected {
                revision: WorkbenchRevision::new(3),
                reason: orchbee_workbench_protocol::WorkbenchRejectionReason::TabUnavailable,
            },
        }))?;
    respond_snapshot(&harness, WorkbenchSnapshotOutcome::Denied {})?;
    let taken = harness.ui.take_next()?;
    let (actual, outcome) = harness.ui.settle_move_transport(taken)?;
    assert_eq!(actual, operation);
    assert!(matches!(outcome, MoveResult::TabUnavailable {
        revision, outcome: WorkbenchSnapshotOutcome::Denied {},
    } if revision == WorkbenchRevision::new(3)));
    assert_eq!(harness.ui.state().desired(), Some(&key));
    let moved = harness.close_with_pending_move()?;
    assert!(
        moved.is_none(),
        "a definite refusal is not a committed Move"
    );
    Ok(())
}

#[test]
fn move_snapshot_corruption_retires_with_worker_owned_commit() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(steps(&[WireKind::Workbench, WireKind::Workbench]), geometry)?;
    drive_controlling_ready_from_snapshot(&mut harness, snapshot(2, 1), geometry)?;
    let operation = harness
        .ui
        .admit_move_transport(WorkbenchTabMoveDirection::Next)?;
    commit_move(&harness, &operation)?;
    assert!(matches!(
        harness.carrier.take_workbench()?,
        WorkbenchRequest::SnapshotRead(_)
    ));
    harness.carrier.respond_malformed_workbench()?;
    let (create, moved) =
        harness.finish_after_worker_failure_with_pending_commits(CarrierFailure::Protocol)?;
    assert!(create.is_none());
    let moved = moved.ok_or_else(|| test_error("corrupt Snapshot lost known Move commit"))?;
    assert_eq!(moved.operation(), &operation);
    assert_eq!(moved.commit().revision, WorkbenchRevision::new(2));
    Ok(())
}

#[test]
fn close_while_move_result_awaits_ui_joins_with_known_commit() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(steps(&[WireKind::Workbench, WireKind::Workbench]), geometry)?;
    drive_controlling_ready_from_snapshot(&mut harness, snapshot(2, 1), geometry)?;
    let operation = harness
        .ui
        .admit_move_transport(WorkbenchTabMoveDirection::Next)?;
    commit_move(&harness, &operation)?;
    respond_snapshot(
        &harness,
        WorkbenchSnapshotOutcome::Available {
            snapshot: reordered_snapshot(),
        },
    )?;
    let taken = harness.ui.take_next()?;
    assert!(matches!(taken.into_event(), WorkerEvent::MoveResult { .. }));
    let pending = harness
        .close_with_pending_move()?
        .ok_or_else(|| test_error("close lost pending Move commit"))?;
    assert_eq!(pending.operation(), &operation);
    assert_eq!(pending.commit().revision, WorkbenchRevision::new(2));
    Ok(())
}

#[test]
fn move_and_snapshot_share_admission_budget_and_expire_at_equality() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(steps(&[WireKind::Workbench, WireKind::Workbench]), geometry)?;
    drive_controlling_ready_from_snapshot(&mut harness, snapshot(2, 1), geometry)?;
    let operation = harness
        .ui
        .admit_move_transport(WorkbenchTabMoveDirection::Next)?;
    let WorkbenchRequest::Command(command) = harness.carrier.take_workbench()? else {
        return Err(test_error("expected Move command"));
    };
    harness.elapsed_millis.store(49_000, Ordering::SeqCst);
    harness
        .carrier
        .respond_workbench(WorkbenchResponse::Command(WorkbenchCommandResult {
            protocol_version: ProtocolVersion,
            request_id: command.request_id,
            outcome: WorkbenchCommandOutcome::TabMoved {
                revision: WorkbenchRevision::new(2),
                terminal_context_id: operation.baseline().terminal_context_id().clone(),
                tab_id: operation.target().tab_id().clone(),
            },
        }))?;
    let WorkbenchRequest::SnapshotRead(read) = harness.carrier.take_workbench()? else {
        return Err(test_error("Move commit must use remaining Snapshot budget"));
    };
    assert_eq!(
        u128::from_str_radix(read.request_id.as_str(), 16)?,
        u128::from_str_radix(operation.request_id().as_str(), 16)? + 1
    );
    harness.elapsed_millis.store(50_000, Ordering::SeqCst);
    harness
        .carrier
        .respond_workbench(WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
            protocol_version: ProtocolVersion,
            request_id: read.request_id,
            outcome: WorkbenchSnapshotOutcome::Available {
                snapshot: reordered_snapshot(),
            },
        }))?;
    let (create, moved) =
        harness.finish_after_worker_failure_with_pending_commits(CarrierFailure::Deadline)?;
    assert!(create.is_none());
    assert_eq!(
        moved
            .ok_or_else(|| test_error("Snapshot expiry lost known commit"))?
            .operation(),
        &operation
    );
    Ok(())
}

#[test]
fn move_ui_retirement_preserves_known_commit_until_worker_join() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(steps(&[WireKind::Workbench, WireKind::Workbench]), geometry)?;
    drive_controlling_ready_from_snapshot(&mut harness, snapshot(2, 1), geometry)?;
    let operation = harness
        .ui
        .admit_move_transport(WorkbenchTabMoveDirection::Next)?;
    commit_move(&harness, &operation)?;
    respond_snapshot(
        &harness,
        WorkbenchSnapshotOutcome::Available {
            snapshot: reordered_snapshot(),
        },
    )?;
    let taken = harness.ui.take_next()?;
    harness
        .ui
        .settle_move_transport_decision(taken, MoveDecision::Retire)?;
    let (_, pending) = harness
        .finish_after_worker_failure_with_pending_commits(CarrierFailure::AuthorityDivergence)?;
    assert_eq!(
        pending
            .ok_or_else(|| test_error("UI retirement lost known commit"))?
            .operation(),
        &operation
    );
    Ok(())
}

#[test]
fn move_commit_survives_shared_snapshot_identity_exhaustion_without_replay() -> TestResult {
    let geometry = fixture_geometry()?;
    // Seven startup exchanges consume IDs through MAX-1; Move reserves the last one.
    let mut harness =
        Harness::start_with_identity_seed(steps(&[WireKind::Workbench]), geometry, u128::MAX - 8)?;
    drive_controlling_ready_from_snapshot(&mut harness, snapshot(2, 1), geometry)?;
    let operation = harness
        .ui
        .admit_move_transport(WorkbenchTabMoveDirection::Next)?;
    assert_eq!(
        u128::from_str_radix(operation.request_id().as_str(), 16)?,
        u128::MAX
    );
    commit_move(&harness, &operation)?;
    let (create, moved) = harness
        .finish_after_worker_failure_with_pending_commits(CarrierFailure::IdentityExhausted)?;
    assert!(create.is_none());
    assert_eq!(
        moved
            .ok_or_else(|| test_error("identity exhaustion lost Move commit"))?
            .operation(),
        &operation
    );
    Ok(())
}
