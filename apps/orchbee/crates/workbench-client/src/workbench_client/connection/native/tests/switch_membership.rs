use super::setup::snapshot;
use super::*;
use crate::workbench_state::RefreshDecision;
use orchbee_workbench_protocol::WorkbenchSnapshotUnavailableReason;

fn startup_steps() -> Vec<WireKind> {
    vec![
        WireKind::Workbench,
        WireKind::Obcs,
        WireKind::Obcs,
        WireKind::Obcs,
        WireKind::Obcs,
        WireKind::Obcs,
    ]
}

#[test]
fn actual_switch_stages_snapshot_before_detach_and_installs_fresh_survivor_fallback_or_empty()
-> TestResult {
    for (remove_target, empty) in [(false, false), (true, false), (true, true)] {
        let geometry = fixture_geometry()?;
        let mut steps = startup_steps();
        steps.extend([WireKind::Workbench, WireKind::Obcs]);
        if !empty {
            steps.extend([WireKind::Obcs; 5]);
        }
        let mut harness = Harness::start(steps, geometry)?;
        let old = drive_controlling_startup(&mut harness, geometry, 2)?;
        let baseline = harness.ui.state().cache().clone();
        let intent = harness.ui.admit_switch(1)?;
        let read = harness.carrier.take_read()?;
        let mut candidate = snapshot(if empty { 0 } else { 2 }, 2);
        if remove_target && !empty {
            candidate.terminal_contexts[0].tabs.remove(1);
        }
        harness.carrier.respond_read(
            &read,
            WorkbenchSnapshotOutcome::Available {
                snapshot: candidate,
            },
        )?;
        assert!(
            matches!(harness.ui.settle_next(FrameCut::None)?, UiEvent::SwitchSnapshot { work, decision: RefreshDecision::Cleanup(_) } if work == intent.key)
        );
        assert_eq!(harness.ui.state().cache(), &baseline);
        assert_eq!(
            harness.ui.state().attached_key().map(|pane| pane.anchor()),
            Some(&old)
        );
        let ClientMessage::TerminalViewer(detach) = harness.carrier.take_obcs()? else {
            return Err(test_error("exact old Detach"));
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
        let settled = harness.ui.settle_next(FrameCut::None)?;
        if empty {
            assert!(matches!(
                settled,
                UiEvent::SwitchOldDetached {
                    continuation: SwitchDetachContinuation::SettledWithoutAttachment,
                    ..
                }
            ));
            assert_eq!(harness.ui.state().phase_kind(), ClientPhaseKind::Empty);
            assert_eq!(harness.mailbox.cursors().0, intent.key.generation());
            assert_eq!(harness.ui.state().desired(), None);
        } else {
            let UiEvent::SwitchOldDetached {
                continuation: SwitchDetachContinuation::Attach(target),
                ..
            } = settled
            else {
                return Err(test_error("fresh installation"));
            };
            assert_eq!(
                target.selected().key().generation(),
                intent.key.generation() + 1
            );
            assert_eq!(
                harness.ui.state().desired().unwrap().panel(),
                baseline.tabs()[if remove_target { 0 } else { 1 }]
                    .panel(0)
                    .unwrap()
                    .panel_id(),
            );
            let key = drive_attach(&mut harness, if remove_target { 0 } else { 1 })?;
            assert_eq!(&key, target.selected().key());
            super::commands::drive_controlling_after_attach(&mut harness, &key, geometry)?;
            assert!(harness.ui.state().ready().is_some());
            assert!(harness.ui.state().invariant_holds());
        }
        harness.finish()?;
    }
    Ok(())
}

#[test]
fn actual_returned_old_poll_after_marker_prevents_unavailable_anchor_restoration() -> TestResult {
    for record in [false, true] {
        let geometry = fixture_geometry()?;
        let mut steps = startup_steps();
        steps.extend([WireKind::Obcs, WireKind::Workbench]);
        let mut harness = Harness::start(steps, geometry)?;
        let old = drive_controlling_startup(&mut harness, geometry, 2)?;
        harness
            .mailbox
            .try_enqueue(QueuedCommand::Poll(Tagged::new(
                harness.ui.pane_ref(&old)?,
                (),
            )))
            .unwrap();
        let ClientMessage::TerminalViewer(poll) = harness.carrier.take_obcs()? else {
            return Err(test_error("old Poll in flight"));
        };
        assert_eq!(
            poll.action(),
            &TerminalViewerAction::Poll {
                attachment_id: attached_viewer_id()?,
            }
        );
        let intent = harness.ui.admit_switch(1)?;
        harness
            .carrier
            .respond_obcs(ServerMessage::TerminalViewerResult(
                TerminalViewerResult::new(
                    poll.request_id().clone(),
                    if record {
                        TerminalViewerOutcome::Record(full_frame_record(2, 2, geometry)?)
                    } else {
                        TerminalViewerOutcome::Empty {}
                    },
                ),
            ))?;
        let read = harness.carrier.take_read()?;
        harness.carrier.respond_read(
            &read,
            WorkbenchSnapshotOutcome::Unavailable {
                reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
            },
        )?;
        assert!(
            matches!(harness.ui.settle_next(FrameCut::None)?, UiEvent::SwitchSnapshot { work, decision: RefreshDecision::Retire } if work == intent.key)
        );
        assert_eq!(harness.ui.state().phase_kind(), ClientPhaseKind::Retired);
        assert_ne!(
            harness.ui.state().desired().map(|pane| pane.anchor()),
            Some(&old)
        );
        harness.finish_after_worker_failure(CarrierFailure::Deadline)?;
    }
    Ok(())
}

#[test]
fn actual_taken_feedback_discard_records_exact_old_proof_after_causal_cell_is_removed() -> TestResult
{
    let geometry = fixture_geometry()?;
    let mut steps = startup_steps();
    steps.extend([WireKind::Obcs, WireKind::Workbench]);
    let mut harness = Harness::start(steps, geometry)?;
    let old = drive_controlling_startup(&mut harness, geometry, 2)?;
    harness
        .mailbox
        .try_enqueue(QueuedCommand::Poll(Tagged::new(
            harness.ui.pane_ref(&old)?,
            (),
        )))
        .unwrap();
    let ClientMessage::TerminalViewer(poll) = harness.carrier.take_obcs()? else {
        return Err(test_error("old Poll in flight"));
    };
    let mailbox = Arc::clone(&harness.mailbox);
    let checkpoint = mailbox.pause_next_causal_take_resume_for_test();
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                poll.request_id().clone(),
                TerminalViewerOutcome::Record(full_frame_record(2, 2, geometry)?),
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::FrameDrawn { .. }
    ));
    assert!(checkpoint.await_reached_before(Instant::now() + Duration::from_secs(2)));
    assert_eq!(mailbox.cells(), ("empty", "empty"));
    let intent = harness.ui.admit_switch(1)?;
    assert_eq!(mailbox.switch_anchor_invalidation_for_test(), None);
    let before = mailbox.cursors();
    drop(checkpoint);
    let read = harness.carrier.take_read()?;
    assert_eq!(
        mailbox.switch_anchor_invalidation_for_test(),
        Some((intent.key.clone(), old))
    );
    let after = mailbox.cursors();
    assert_eq!(
        (after.0, after.1, after.2),
        (before.0, before.1, before.2 + 1)
    );
    harness.carrier.respond_read(
        &read,
        WorkbenchSnapshotOutcome::Unavailable {
            reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
        },
    )?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::SwitchSnapshot {
            decision: RefreshDecision::Retire,
            ..
        }
    ));
    assert_eq!(mailbox.switch_anchor_invalidation_for_test(), None);
    harness.finish_after_worker_failure(CarrierFailure::Deadline)
}

#[test]
fn actual_busy_old_poll_without_frame_preserves_exact_unavailable_restoration() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut steps = startup_steps();
    steps.extend([WireKind::Obcs, WireKind::Workbench]);
    let mut harness = Harness::start(steps, geometry)?;
    let old = drive_controlling_startup(&mut harness, geometry, 2)?;
    harness
        .mailbox
        .try_enqueue(QueuedCommand::Poll(Tagged::new(
            harness.ui.pane_ref(&old)?,
            (),
        )))
        .unwrap();
    let ClientMessage::TerminalViewer(poll) = harness.carrier.take_obcs()? else {
        return Err(test_error("old Poll in flight"));
    };
    let intent = harness.ui.admit_switch(1)?;
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                poll.request_id().clone(),
                TerminalViewerOutcome::Unavailable {
                    reason: ViewerUnavailableReason::ServerBusy,
                },
            ),
        ))?;
    let read = harness.carrier.take_read()?;
    assert_eq!(harness.mailbox.switch_anchor_invalidation_for_test(), None);
    let before = harness.mailbox.cursors();
    harness.carrier.respond_read(
        &read,
        WorkbenchSnapshotOutcome::Unavailable {
            reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
        },
    )?;
    assert!(
        matches!(harness.ui.settle_next(FrameCut::None)?, UiEvent::SwitchSnapshot { work, decision: RefreshDecision::Continue } if work == intent.key)
    );
    assert_eq!(
        harness.ui.state().desired().map(|pane| pane.anchor()),
        Some(&old)
    );
    assert_eq!(harness.ui.state().phase_kind(), ClientPhaseKind::Ready);
    assert_eq!(harness.mailbox.cursors(), before);
    harness.finish()
}

#[test]
fn timely_switch_unavailable_waits_for_exact_ui_restore_after_original_total_deadline() -> TestResult
{
    let geometry = fixture_geometry()?;
    let mut steps = startup_steps();
    steps.push(WireKind::Workbench);
    let mut harness = Harness::start(steps, geometry)?;
    let old = drive_controlling_startup(&mut harness, geometry, 2)?;
    let mailbox = Arc::clone(&harness.mailbox);
    let checkpoint = mailbox.pause_next_switch_snapshot_wait_for_test();
    let intent = harness.ui.admit_switch(1)?;
    let read = harness.carrier.take_read()?;
    harness.carrier.respond_read(
        &read,
        WorkbenchSnapshotOutcome::Unavailable {
            reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
        },
    )?;
    let taken = harness.ui.take_next()?;
    assert!(checkpoint.await_reached_before(Instant::now() + Duration::from_secs(2)));
    let cursors = mailbox.cursors();
    harness.elapsed_millis.store(75_000, Ordering::SeqCst);
    drop(checkpoint);
    assert!(
        mailbox.await_condition_waiter_before(Instant::now() + Duration::from_secs(2)),
        "timely Snapshot must wait for semantic UI settlement: {:?}",
        mailbox.is_closed()
    );
    assert!(
        matches!(harness.ui.settle_taken(taken, FrameCut::None)?, UiEvent::SwitchSnapshot { work, decision: RefreshDecision::Continue } if work == intent.key)
    );
    assert_eq!(
        harness.ui.state().desired().map(|pane| pane.anchor()),
        Some(&old)
    );
    assert_eq!(harness.ui.state().phase_kind(), ClientPhaseKind::Ready);
    assert_eq!(mailbox.cursors(), cursors);
    harness.finish()
}
