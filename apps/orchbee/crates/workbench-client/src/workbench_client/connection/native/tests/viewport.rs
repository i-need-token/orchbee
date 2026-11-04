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
            let mut steps = vec![WireKind::Workbench];
            // Five remaining startup exchanges, the demand pair, and an optional reset.
            steps.extend(std::iter::repeat_n(
                WireKind::Obcs,
                7 + usize::from(refuse_projection),
            ));
            let mut harness = Harness::start(steps, geometry)?;
            super::commands::drive_controlling_ready_from_snapshot(
                &mut harness,
                super::setup::snapshot(1, 1),
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

/// A Host refusal of the last demand leg that could restore live following leaves the Server
/// slot holding a demand the Client can no longer name. The viewer is released over the wire and
/// the pane takes the typed loss path, so a focused pane parks instead of costing the Client.
#[test]
fn a_host_refused_exit_leg_releases_the_viewer_and_parks_the_focus() -> TestResult {
    let refused = || TerminalViewerOutcome::Rejected {
        reason: ViewerRejectedReason::HostRejected,
    };
    for step in [ViewportStep::Enter, ViewportStep::Recover] {
        let geometry = fixture_geometry()?;
        let legs = if step == ViewportStep::Recover {
            vec![(
                TerminalViewerAction::SetHistoryDemand {
                    attachment_id: attached_viewer_id()?,
                    demand: HistoryDemand::FollowingBottom,
                },
                refused(),
            )]
        } else {
            vec![
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
                        demand: ProjectionDemand::Paused,
                    },
                    refused(),
                ),
                (
                    TerminalViewerAction::SetHistoryDemand {
                        attachment_id: attached_viewer_id()?,
                        demand: HistoryDemand::FollowingBottom,
                    },
                    refused(),
                ),
            ]
        };
        let mut steps = vec![WireKind::Workbench];
        // Five remaining startup exchanges, the demand legs, and the release.
        steps.extend(std::iter::repeat_n(WireKind::Obcs, 6 + legs.len()));
        let mut harness = Harness::start(steps, geometry)?;
        super::commands::drive_controlling_ready_from_snapshot(
            &mut harness,
            super::setup::snapshot(1, 1),
            geometry,
        )?;
        let key = harness.ui.state().focus_pane_ref().unwrap();
        harness
            .mailbox
            .try_enqueue(QueuedCommand::Viewport(Tagged::new(key.clone(), step)))
            .unwrap();
        for (expected, outcome) in legs {
            let ClientMessage::TerminalViewer(request) = harness.carrier.take_obcs()? else {
                return Err(test_error("viewport steps must only set viewer demands"));
            };
            assert_eq!(request.action(), &expected);
            harness
                .carrier
                .respond_obcs(ServerMessage::TerminalViewerResult(
                    TerminalViewerResult::new(request.request_id().clone(), outcome),
                ))?;
        }
        let release = harness.carrier.take_obcs();
        assert_eq!(
            harness.mailbox.is_closed(),
            None,
            "a Host-refused exit leg never costs the Client: {step:?}"
        );
        let ClientMessage::TerminalViewer(release) = release? else {
            return Err(test_error("the refused viewer must be released"));
        };
        assert_eq!(
            release.action(),
            &TerminalViewerAction::Detach {
                attachment_id: attached_viewer_id()?,
            }
        );
        harness
            .carrier
            .respond_obcs(ServerMessage::TerminalViewerResult(
                TerminalViewerResult::new(
                    release.request_id().clone(),
                    TerminalViewerOutcome::Detached {},
                ),
            ))?;
        let failed = harness.ui.take_next()?;
        assert!(matches!(
            failed.event(),
            WorkerEvent::PaneFailed { key: lost, fault: PaneFault::ViewerLost } if lost == &key
        ));
        assert!(matches!(
            harness.ui.settle_taken(failed, FrameCut::None)?,
            UiEvent::PaneFailed
        ));
        assert_eq!(harness.mailbox.is_closed(), None);
        assert_eq!(
            harness.ui.state().phase_kind(),
            ClientPhaseKind::RetryableAttachUnavailable
        );
        assert!(harness.ui.state().focused_viewer_lost());
        harness.finish()?;
    }
    Ok(())
}
