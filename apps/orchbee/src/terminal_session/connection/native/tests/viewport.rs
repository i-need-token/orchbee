//! Demand selection at the real executor/carrier boundary, including partial-entry cleanup.
use super::*;

#[test]
fn automatic_tail_keeps_live_frames_active_and_manual_history_pauses_them() -> TestResult {
    for (step, demand) in [
        (ViewportStep::EnterTail, ProjectionDemand::Active),
        (ViewportStep::Enter, ProjectionDemand::Paused),
    ] {
        for refuse_projection in [false, true] {
            let geometry = fixture_geometry()?;
            let mut steps = vec![WireKind::Obcs, WireKind::Workbench];
            // Five remaining startup exchanges, the demand pair, and an optional reset.
            steps.extend(std::iter::repeat_n(
                WireKind::Obcs,
                7 + usize::from(refuse_projection),
            ));
            let mut harness = Harness::start(steps, geometry)?;
            super::commands::drive_controlling_ready_from_snapshot(
                &mut harness,
                snapshot(1, 1),
                geometry,
            )?;
            let key = harness.ui.state().focus_pane_ref().unwrap();
            harness
                .mailbox
                .try_enqueue(QueuedCommand::Viewport(Tagged::new(key.clone(), step)))
                .unwrap();
            let mut actions = vec![
                (
                    TerminalViewerAction::SetHistoryDemand {
                        attachment_id: attached_viewer_id()?,
                        demand: HistoryDemand::ViewingHistory,
                    },
                    TerminalViewerOutcome::FeedbackAccepted {},
                ),
                (
                    TerminalViewerAction::SetProjectionDemand {
                        attachment_id: attached_viewer_id()?,
                        demand,
                    },
                    if refuse_projection {
                        TerminalViewerOutcome::Rejected {
                            reason: ViewerRejectedReason::HostRejected,
                        }
                    } else {
                        TerminalViewerOutcome::FeedbackAccepted {}
                    },
                ),
            ];
            if refuse_projection {
                actions.push((
                    TerminalViewerAction::SetHistoryDemand {
                        attachment_id: attached_viewer_id()?,
                        demand: HistoryDemand::FollowingBottom,
                    },
                    TerminalViewerOutcome::FeedbackAccepted {},
                ));
            }
            for (expected, outcome) in actions {
                let ClientMessage::TerminalViewer(request) = harness.carrier.take_obcs()? else {
                    return Err(test_error("viewport entry must only set viewer demands"));
                };
                assert_eq!(request.action(), &expected);
                harness
                    .carrier
                    .respond_obcs(ServerMessage::TerminalViewerResult(
                        TerminalViewerResult::new(request.request_id().clone(), outcome),
                    ))?;
            }
            let (wait, event) = harness.ui.take_next()?.into_parts();
            assert!(wait.is_none());
            assert!(matches!(
                event,
                WorkerEvent::ViewportOutcome {
                    key: actual,
                    outcome: ViewportOutcome::Enter { applied },
                } if actual == key && applied != refuse_projection
            ));
            harness.finish()?;
        }
    }
    Ok(())
}
