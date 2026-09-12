use super::setup::*;
use crate::terminal_session::{
    mailbox::{QueuedCommand, Tagged},
    state::{
        CarrierFailure, ControllerMode, CreateProjectionDecision, CreateTerminalProvenance,
        NavigatorRefreshStatus, PendingRefreshStage, RefreshDecision, SessionNotice,
        SessionPhaseKind,
    },
};
use orchbee_client_server_protocol::{
    ClientMessage, ControlInput, ControlUnavailableReason, Geometry, ServerMessage,
    TerminalControlAction, TerminalControlOutcome, TerminalControlResult, TerminalViewerAction,
    TerminalViewerOutcome, TerminalViewerResult,
};
use orchbee_workbench_protocol::{
    Delta, DeltaChange, PanelNode, PanelOrientation, ProtocolVersion, TerminalContext,
    WorkbenchAction, WorkbenchCommandOutcome, WorkbenchCommandResult, WorkbenchPublicationDocument,
    WorkbenchPublicationOutcome, WorkbenchPublicationResult, WorkbenchRejectionReason,
    WorkbenchRequest, WorkbenchResponse, WorkbenchRevision, WorkbenchSnapshotOutcome,
    WorkbenchTerminalFailureReason, WorkbenchTerminalOutcome, WorkbenchTerminalUnavailableReason,
};
use std::sync::atomic::Ordering;

#[test]
fn empty_create_started_reads_candidate_without_detach_then_attaches_reserved_generation()
-> TestResult {
    let geometry = fixture_geometry()?;
    let mut kinds = vec![WireKind::Obcs, WireKind::Workbench];
    kinds.extend([WireKind::Obcs; 5]);
    kinds.extend([
        WireKind::Workbench,
        WireKind::Obcs,
        WireKind::Workbench,
        WireKind::Workbench,
    ]);
    kinds.extend([WireKind::Obcs; 5]);
    let mut harness = Harness::start(kinds, geometry)?;
    drive_controlling_ready(&mut harness, geometry)?;
    let generation = harness.ui.admit_refresh()?;
    let read = harness.carrier.take_read()?;
    harness.carrier.respond_read(
        &read,
        WorkbenchSnapshotOutcome::Available {
            snapshot: snapshot(0, 2),
        },
    )?;
    assert!(
        matches!(harness.ui.settle_next(FrameCut::None)?, UiEvent::Refresh { generation: actual, decision: RefreshDecision::Cleanup(_) } if actual == generation)
    );
    let ClientMessage::TerminalViewer(detach) = harness.carrier.take_obcs()? else {
        return Err(test_error("Refresh exact Detach"));
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
        UiEvent::Refresh {
            decision: RefreshDecision::SettledWithoutAttachment,
            ..
        }
    ));
    assert_eq!(harness.ui.state().phase_kind(), SessionPhaseKind::Empty);
    let operation = harness.ui.admit_create()?;
    let WorkbenchRequest::Command(command) = harness.carrier.take_workbench()? else {
        return Err(test_error("manual Empty Create"));
    };
    let WorkbenchAction::TabCreate {
        geometry: captured, ..
    } = command.action
    else {
        return Err(test_error("tab.create"));
    };
    let mut candidate = snapshot(2, 3);
    candidate.terminal_contexts[0].tabs.remove(0);
    set_tab_labels(&mut candidate, 0, "1", "1")?;
    let created = &candidate.terminal_contexts[0].tabs[0];
    let expected = crate::terminal_session::state::AttachmentKey::new(
        operation.reserved_generation(),
        created.tab_id.clone(),
    );
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = &created.root
    else {
        return Err(test_error("Panel"));
    };
    harness
        .carrier
        .respond_workbench(WorkbenchResponse::Command(WorkbenchCommandResult {
            protocol_version: ProtocolVersion,
            request_id: command.request_id,
            outcome: WorkbenchCommandOutcome::TabCreated {
                revision: candidate.revision,
                terminal_context_id: candidate.terminal_contexts[0].terminal_context_id.clone(),
                tab_id: created.tab_id.clone(),
                panel_id: panel_id.clone(),
                presentation_id: primary_presentation.presentation_id.clone(),
                terminal_outcome: WorkbenchTerminalOutcome::Started { geometry: captured },
            },
        }))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::CreateCommittedKnown { .. }
    ));
    let read = harness.carrier.take_read()?;
    harness.carrier.respond_read(
        &read,
        WorkbenchSnapshotOutcome::Available {
            snapshot: candidate,
        },
    )?;
    assert!(
        matches!(harness.ui.settle_next(FrameCut::None)?, UiEvent::CreateProjection { decision: CreateProjectionDecision::Attach(ref target), .. } if target.selected().key() == &expected)
    );
    assert_eq!(drive_attach(&mut harness, 1)?, expected);
    drive_controlling_after_attach(&mut harness, &expected, geometry)?;
    assert!(harness.ui.state().ready().is_some());
    assert_eq!(harness.ui.state().desired(), Some(&expected));
    assert_eq!(
        harness
            .ui
            .state()
            .settled_create_for_test()
            .unwrap()
            .operation()
            .key(),
        &operation
    );
    harness.finish()
}

#[test]
fn candidate_first_create_unavailable_never_detaches_or_publishes_the_reserved_target() -> TestResult
{
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(
        vec![
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Workbench,
        ],
        geometry,
    )?;
    let (old, _) = drive_controlling_ready(&mut harness, geometry)?;
    let baseline = harness.ui.state().cache().clone();
    let operation = harness.ui.admit_create()?;
    let committed = snapshot(2, 2);
    let created = &committed.terminal_contexts[0].tabs[1];
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = &created.root
    else {
        return Err(test_error("simple created tab"));
    };
    let WorkbenchRequest::Command(command) = harness.carrier.take_workbench()? else {
        return Err(test_error("Create"));
    };
    harness
        .carrier
        .respond_workbench(WorkbenchResponse::Command(WorkbenchCommandResult {
            protocol_version: ProtocolVersion,
            request_id: command.request_id,
            outcome: WorkbenchCommandOutcome::TabCreatePending {
                revision: WorkbenchRevision::new(2),
                terminal_context_id: committed.terminal_contexts[0].terminal_context_id.clone(),
                tab_id: created.tab_id.clone(),
                panel_id: panel_id.clone(),
                presentation_id: primary_presentation.presentation_id.clone(),
            },
        }))?;
    assert!(
        matches!(harness.ui.settle_next(FrameCut::None)?, UiEvent::CreateCommittedKnown { operation: actual } if actual == operation)
    );
    let read = harness.carrier.take_read()?;
    harness.carrier.respond_read(
        &read,
        WorkbenchSnapshotOutcome::Unavailable {
            reason:
                orchbee_workbench_protocol::WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
        },
    )?;
    let taken = harness.ui.take_next()?;
    harness.elapsed_millis.store(51_000, Ordering::SeqCst);
    assert!(matches!(
        harness.ui.settle_taken(taken, FrameCut::None)?,
        UiEvent::CreateProjection {
            decision: CreateProjectionDecision::ProjectionUnavailable,
            ..
        }
    ));
    assert_eq!(harness.ui.state().cache(), &baseline);
    assert_eq!(harness.ui.state().desired(), Some(&old));
    assert_eq!(harness.ui.state().projection_key(), Some(&old));
    assert!(harness.ui.state().ready().is_none());
    assert!(harness.ui.state().committed_target().is_some());
    harness.finish()
}

#[test]
fn attached_refresh_membership_runs_real_detach_before_empty_or_successor_ready() -> TestResult {
    for (neighbor, expires, wrong_response) in [
        (false, false, false),
        (true, false, false),
        (true, true, false),
        (false, false, true),
    ] {
        let geometry = fixture_geometry()?;
        let mut kinds = vec![
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Obcs,
        ];
        if neighbor && !expires {
            kinds.extend([WireKind::Obcs; 5]);
        }
        if !neighbor && !expires && !wrong_response {
            kinds.push(WireKind::Workbench);
        }
        let mut harness = Harness::start(kinds, geometry)?;
        let (old, _) = drive_controlling_ready(&mut harness, geometry)?;
        let baseline = harness.ui.state().cache().clone();
        let generation = harness.ui.admit_refresh()?;
        let read = harness.carrier.take_read()?;
        let mut candidate = snapshot(if neighbor { 2 } else { 0 }, 2);
        if neighbor {
            candidate.terminal_contexts[0].tabs.remove(0);
        }
        harness.carrier.respond_read(
            &read,
            WorkbenchSnapshotOutcome::Available {
                snapshot: candidate,
            },
        )?;
        assert!(
            matches!(harness.ui.settle_next(FrameCut::None)?, UiEvent::Refresh { generation: actual, decision: RefreshDecision::Cleanup(_) } if actual == generation)
        );
        assert_eq!(harness.ui.state().cache(), &baseline);
        assert_eq!(harness.ui.state().desired(), Some(&old));
        assert!(harness.ui.state().ready().is_none());
        let ClientMessage::TerminalViewer(detach) = harness.carrier.take_obcs()? else {
            return Err(test_error("exact old Detach before installation"));
        };
        assert_eq!(
            detach.action(),
            &TerminalViewerAction::Detach {
                attachment_id: attached_viewer_id()?,
            }
        );
        let detached = TerminalViewerResult::new(
            detach.request_id().clone(),
            if wrong_response {
                TerminalViewerOutcome::Empty {}
            } else {
                TerminalViewerOutcome::Detached {}
            },
        );
        harness
            .carrier
            .respond_obcs(ServerMessage::TerminalViewerResult(detached))?;
        if wrong_response {
            assert_eq!(harness.ui.state().cache(), &baseline);
            // The carrier rejects an action-incompatible reply after submission as ambiguous.
            harness.finish_after_worker_failure(CarrierFailure::OutcomeUnknown)?;
            continue;
        }
        let taken = harness.ui.take_next()?;
        if expires {
            harness.elapsed_millis.store(45_000, Ordering::SeqCst);
        }
        let event = harness.ui.settle_taken(taken, FrameCut::None)?;
        if expires {
            assert!(matches!(
                event,
                UiEvent::Refresh {
                    decision: RefreshDecision::Retire,
                    ..
                }
            ));
            assert_eq!(harness.ui.state().cache(), &baseline);
            harness.finish_after_worker_failure(CarrierFailure::Deadline)?;
        } else if neighbor {
            let UiEvent::Refresh {
                decision: RefreshDecision::AttachRecovered(target),
                ..
            } = event
            else {
                return Err(test_error("successor attach decision"));
            };
            assert_eq!(target.selected().key().generation(), old.generation() + 1);
            let next = drive_attach(&mut harness, 1)?;
            assert_eq!(&next, target.selected().key());
            drive_controlling_after_attach(&mut harness, &next, geometry)?;
            assert!(harness.ui.state().ready().is_some());
            assert_eq!(harness.ui.state().active().unwrap().index(), 0);
            harness.finish()?;
        } else {
            assert!(matches!(
                event,
                UiEvent::Refresh {
                    decision: RefreshDecision::SettledWithoutAttachment,
                    ..
                }
            ));
            assert_eq!(harness.ui.state().phase_kind(), SessionPhaseKind::Empty);
            assert_eq!(harness.ui.state().desired(), None);
            let create = harness.ui.admit_create()?;
            let WorkbenchRequest::Command(command) = harness.carrier.take_workbench()? else {
                return Err(test_error("manual Empty Create only"));
            };
            assert_eq!(command.expected_revision, WorkbenchRevision::new(2));
            let WorkbenchAction::TabCreate {
                geometry: captured, ..
            } = command.action
            else {
                return Err(test_error("tab.create"));
            };
            assert_eq!(captured.columns(), geometry.columns());
            harness
                .carrier
                .respond_workbench(WorkbenchResponse::Command(WorkbenchCommandResult {
                    protocol_version: ProtocolVersion,
                    request_id: command.request_id,
                    outcome: WorkbenchCommandOutcome::Denied {},
                }))?;
            assert!(
                matches!(harness.ui.settle_next(FrameCut::None)?, UiEvent::CreatePrecommit { operation } if operation == create)
            );
            assert_eq!(harness.ui.state().phase_kind(), SessionPhaseKind::Empty);
            assert_eq!(harness.ui.state().desired(), None);
            harness.finish()?;
        }
    }
    Ok(())
}

#[test]
fn create_drains_busy_input_resize_and_drawn_feedback_before_one_submission() -> TestResult {
    let geometry = fixture_geometry()?;
    let resized = Geometry::new(
        geometry.columns(),
        geometry.rows(),
        geometry
            .cell_pixel_width()
            .checked_add(1)
            .ok_or_else(|| test_error("cell-pixel width overflow"))?,
        geometry
            .cell_pixel_height()
            .checked_add(1)
            .ok_or_else(|| test_error("cell-pixel height overflow"))?,
    )?;
    let mut harness = Harness::start(
        vec![
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Workbench,
        ],
        geometry,
    )?;
    let (old_key, controller) = drive_controlling_ready(&mut harness, geometry)?;

    let input = ControlInput::new(vec![b'x'])?;
    harness
        .mailbox
        .try_enqueue(QueuedCommand::Input(Tagged::new(
            old_key.clone(),
            input.clone(),
        )))
        .map_err(|error| test_error(&format!("failed to enqueue Input: {error:?}")))?;
    let ClientMessage::TerminalControl(first_input) = harness.carrier.take_obcs()? else {
        return Err(test_error("accepted Input must reach the carrier"));
    };
    assert_control_input(&first_input, &controller, 2, &input)?;
    assert_eq!(first_input.request_id().as_str(), identity(0x108));

    harness.ui.admit_resize(resized)?;
    harness
        .mailbox
        .try_enqueue(QueuedCommand::Poll(Tagged::new(old_key.clone(), ())))
        .map_err(|error| test_error(&format!("failed to enqueue Poll: {error:?}")))?;
    let create = harness.ui.admit_create()?;
    assert_eq!(create.reserved_generation(), 2);
    assert_eq!(create.request_id().as_str(), identity(0x109));
    assert_eq!(harness.ui.state().phase_kind(), SessionPhaseKind::Creating);

    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalControlResult(
            TerminalControlResult::new(
                first_input.request_id().clone(),
                TerminalControlOutcome::Unavailable {
                    reason: ControlUnavailableReason::ServerBusy,
                },
            ),
        ))?;
    let ClientMessage::TerminalControl(retried_input) = harness.take_obcs_with_advancing_clock()?
    else {
        return Err(test_error("ServerBusy Input must retry on the same lease"));
    };
    assert_control_input(&retried_input, &controller, 3, &input)?;
    assert_eq!(retried_input.request_id().as_str(), identity(0x10a));
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalControlResult(
            TerminalControlResult::new(
                retried_input.request_id().clone(),
                TerminalControlOutcome::InputAccepted {},
            ),
        ))?;

    let ClientMessage::TerminalControl(resize) = harness.carrier.take_obcs()? else {
        return Err(test_error("pre-Create Resize must drain after Input"));
    };
    let TerminalControlAction::Resize {
        attachment_id,
        controller_id,
        sequence,
        geometry: requested_geometry,
    } = resize.action()
    else {
        return Err(test_error("expected pre-Create Resize"));
    };
    assert_eq!(attachment_id, &attached_viewer_id()?);
    assert_eq!(controller_id, &controller);
    assert_eq!(sequence.get(), 4);
    assert_eq!(*requested_geometry, resized);
    assert_eq!(resize.request_id().as_str(), identity(0x10b));
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
        UiEvent::SetupConfigured {
            ref key,
            mode: ControllerMode::Controlling(ref actual),
            applied_geometry: Some(actual_geometry),
        } if key == &old_key && actual == &controller && actual_geometry == resized
    ));

    let ClientMessage::TerminalViewer(poll) = harness.carrier.take_obcs()? else {
        return Err(test_error("pre-Create Poll must drain after Resize"));
    };
    assert_eq!(
        poll.action(),
        &TerminalViewerAction::Poll {
            attachment_id: attached_viewer_id()?,
        }
    );
    assert_eq!(poll.request_id().as_str(), identity(0x10c));
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                poll.request_id().clone(),
                TerminalViewerOutcome::Record(full_frame_record(2, 2, resized)?),
            ),
        ))?;
    let feedback = match harness.ui.settle_next(FrameCut::None)? {
        UiEvent::FrameDrawn { key, feedback } if key == old_key => feedback,
        _ => return Err(test_error("old Poll frame must cross the draw checkpoint")),
    };
    assert!(matches!(
        feedback,
        TerminalViewerAction::FrameApplied { .. }
    ));

    let ClientMessage::TerminalViewer(feedback_request) = harness.carrier.take_obcs()? else {
        return Err(test_error("drawn old frame must send causal feedback"));
    };
    assert_eq!(feedback_request.action(), &feedback);
    assert_eq!(feedback_request.request_id().as_str(), identity(0x10d));
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                feedback_request.request_id().clone(),
                TerminalViewerOutcome::FeedbackAccepted {},
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::FeedbackAccepted { ref key } if key == &old_key
    ));

    let WorkbenchRequest::Command(command) = harness.carrier.take_workbench()? else {
        return Err(test_error(
            "tab.create must be the first exchange after the causal drain",
        ));
    };
    assert_eq!(command.request_id, *create.request_id());
    assert_eq!(command.expected_revision, WorkbenchRevision::new(1));
    let WorkbenchAction::TabCreate {
        terminal_context_id,
        label,
        geometry: requested_geometry,
    } = &command.action
    else {
        return Err(test_error("expected one tab.create action"));
    };
    assert_eq!(
        terminal_context_id,
        harness.ui.state().cache().terminal_context_id()
    );
    assert_eq!(label.as_str(), "2");
    assert_eq!(requested_geometry.columns(), resized.columns());
    assert_eq!(requested_geometry.rows(), resized.rows());
    assert_eq!(
        requested_geometry.cell_pixel_width(),
        resized.cell_pixel_width()
    );
    assert_eq!(
        requested_geometry.cell_pixel_height(),
        resized.cell_pixel_height()
    );
    harness
        .carrier
        .respond_workbench(WorkbenchResponse::Command(WorkbenchCommandResult {
            protocol_version: ProtocolVersion,
            request_id: command.request_id,
            outcome: WorkbenchCommandOutcome::Rejected {
                revision: WorkbenchRevision::new(1),
                reason: WorkbenchRejectionReason::ResourceLimit,
            },
        }))?;

    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::CreatePrecommit { ref operation } if operation == &create
    ));
    assert_eq!(harness.ui.state().phase_kind(), SessionPhaseKind::Ready);
    assert_eq!(
        harness.ui.state().active().map(|tab| tab.key()),
        Some(&old_key)
    );
    assert_eq!(harness.ui.state().desired().unwrap(), &old_key);
    assert_eq!(harness.ui.state().projection_key(), Some(&old_key));
    assert_eq!(
        harness.ui.state().notice(),
        Some(SessionNotice::CreateNotCreated)
    );

    harness.finish()
}

#[test]
fn historical_ready_allows_create_after_blocked_frame_and_drains_a_second_blocked_poll()
-> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(
        vec![
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Workbench,
        ],
        geometry,
    )?;
    let (old_key, _controller) = drive_controlling_ready(&mut harness, geometry)?;
    let baseline_no_frame_redraws = harness.ui.no_frame_redraws();

    harness
        .mailbox
        .try_enqueue(QueuedCommand::Poll(Tagged::new(old_key.clone(), ())))
        .map_err(|error| test_error(&format!("failed to enqueue first blocked Poll: {error:?}")))?;
    let ClientMessage::TerminalViewer(first_poll) = harness.carrier.take_obcs()? else {
        return Err(test_error("first ordinary Poll must reach the carrier"));
    };
    assert_eq!(
        first_poll.action(),
        &TerminalViewerAction::Poll {
            attachment_id: attached_viewer_id()?,
        }
    );
    assert_eq!(first_poll.request_id().as_str(), identity(0x108));
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                first_poll.request_id().clone(),
                TerminalViewerOutcome::Record(blocked_record(2)?),
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::NoFrameRedraw {
            ref key,
            feedback: None,
        } if key == &old_key
    ));
    assert_eq!(harness.ui.no_frame_redraws(), baseline_no_frame_redraws + 1);
    assert_eq!(harness.ui.state().phase_kind(), SessionPhaseKind::Ready);
    assert!(!harness.ui.state().feedback_accepted());

    // Keep a second old-target Poll in flight across the marker. Historical Ready must admit the
    // Create immediately after the prior no-frame redraw; it must not wait for another full frame.
    harness
        .mailbox
        .try_enqueue(QueuedCommand::Poll(Tagged::new(old_key.clone(), ())))
        .map_err(|error| {
            test_error(&format!("failed to enqueue second blocked Poll: {error:?}"))
        })?;
    let ClientMessage::TerminalViewer(second_poll) = harness.carrier.take_obcs()? else {
        return Err(test_error("second ordinary Poll must reach the carrier"));
    };
    assert_eq!(
        second_poll.action(),
        &TerminalViewerAction::Poll {
            attachment_id: attached_viewer_id()?,
        }
    );
    assert_eq!(second_poll.request_id().as_str(), identity(0x109));
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                second_poll.request_id().clone(),
                TerminalViewerOutcome::Record(blocked_record(3)?),
            ),
        ))?;

    let create = harness.ui.admit_create()?;
    assert_eq!(create.reserved_generation(), 2);
    assert_eq!(create.request_id().as_str(), identity(0x10a));
    assert_eq!(harness.ui.state().phase_kind(), SessionPhaseKind::Creating);
    // NativeUi forwards Input only through `state.ready()`. The marker atomically moved the
    // canonical state to Creating, so the production admission guard now refuses later Input.
    assert!(harness.ui.state().ready().is_none());
    assert_eq!(harness.mailbox.is_closed(), None);

    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::NoFrameRedraw {
            ref key,
            feedback: None,
        } if key == &old_key
    ));
    assert_eq!(harness.ui.no_frame_redraws(), baseline_no_frame_redraws + 2);

    let WorkbenchRequest::Command(command) = harness.carrier.take_workbench()? else {
        return Err(test_error(
            "Create barrier must submit once after the blocked Poll's NoFeedback settlement",
        ));
    };
    assert_eq!(command.request_id, *create.request_id());
    assert!(matches!(command.action, WorkbenchAction::TabCreate { .. }));
    harness
        .carrier
        .respond_workbench(WorkbenchResponse::Command(WorkbenchCommandResult {
            protocol_version: ProtocolVersion,
            request_id: command.request_id,
            outcome: WorkbenchCommandOutcome::Rejected {
                revision: WorkbenchRevision::new(1),
                reason: WorkbenchRejectionReason::ResourceLimit,
            },
        }))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::CreatePrecommit { ref operation } if operation == &create
    ));
    assert_eq!(harness.ui.state().phase_kind(), SessionPhaseKind::Ready);
    assert_eq!(harness.ui.state().desired().unwrap(), &old_key);
    assert_eq!(harness.ui.state().projection_key(), Some(&old_key));
    assert!(!harness.ui.state().feedback_accepted());
    assert_eq!(harness.mailbox.is_closed(), None);

    // Script exhaustion proves that neither blocked Poll emitted feedback and only one Create was
    // submitted. The second Poll kept its own pre-marker deadline throughout the barrier drain.
    harness.finish()
}

#[test]
fn create_barrier_expiring_held_poll_retires_deadline_before_create_or_detach() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(
        vec![
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
        ],
        geometry,
    )?;
    let (active, _controller) = drive_controlling_ready(&mut harness, geometry)?;
    harness
        .mailbox
        .try_enqueue(QueuedCommand::Poll(Tagged::new(active.clone(), ())))
        .map_err(|error| test_error(&format!("failed to enqueue held Poll: {error:?}")))?;
    let ClientMessage::TerminalViewer(poll) = harness.carrier.take_obcs()? else {
        return Err(test_error("held Poll must reach the carrier"));
    };
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(poll.request_id().clone(), TerminalViewerOutcome::Empty {}),
        ))?;
    let _held_poll_event = harness.ui.take_next()?;
    let create = harness.ui.admit_create()?;
    assert_eq!(harness.ui.state().phase_kind(), SessionPhaseKind::Creating);

    // Create's 45-second forward barrier is earlier than the held Poll's 50-second deadline.
    // Letting that exact barrier expire must be a session Deadline, not a successful drain.
    harness.elapsed_millis.store(45_000, Ordering::SeqCst);
    harness.mailbox.wake_waiters();
    assert!(harness.ui.take_next().is_err());
    assert_eq!(harness.mailbox.is_closed(), Some(CarrierFailure::Deadline));
    assert_eq!(harness.ui.state().desired().unwrap(), &active);
    assert_eq!(harness.ui.state().projection_key(), Some(&active));
    assert_eq!(create.reserved_generation(), 2);

    // Exact scripted exhaustion proves that expiry did not advance to tab.create or Detach.
    harness.finish_after_worker_failure(CarrierFailure::Deadline)
}

#[test]
fn every_non_attachable_create_commit_waits_for_acks_and_never_attaches() -> TestResult {
    for case in [
        NonAttachableCommit::Pending,
        NonAttachableCommit::Unavailable,
        NonAttachableCommit::Failed,
        NonAttachableCommit::OutcomeUnknown,
    ] {
        run_non_attachable_commit(case)?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
enum NonAttachableCommit {
    Pending,
    Unavailable,
    Failed,
    OutcomeUnknown,
}

impl NonAttachableCommit {
    const fn provenance(self) -> CreateTerminalProvenance {
        match self {
            Self::Pending => CreateTerminalProvenance::Pending,
            Self::Unavailable => CreateTerminalProvenance::Unavailable(
                WorkbenchTerminalUnavailableReason::TerminalRuntimeUnavailable,
            ),
            Self::Failed => CreateTerminalProvenance::Failed(
                WorkbenchTerminalFailureReason::TerminalCreateFailed,
            ),
            Self::OutcomeUnknown => CreateTerminalProvenance::OutcomeUnknown,
        }
    }
}

fn run_non_attachable_commit(case: NonAttachableCommit) -> TestResult {
    let expected_provenance = case.provenance();
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(
        vec![
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Workbench,
            WireKind::Obcs,
        ],
        geometry,
    )?;
    let (old_key, _controller) = drive_controlling_ready(&mut harness, geometry)?;
    let operation = harness.ui.admit_create()?;

    let WorkbenchRequest::Command(command) = harness.carrier.take_workbench()? else {
        return Err(test_error("Create admission must submit tab.create"));
    };
    assert_eq!(command.request_id, *operation.request_id());
    let candidate = snapshot(2, 2);
    let created = &candidate.terminal_contexts[0].tabs[1];
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = &created.root
    else {
        return Err(test_error("created fixture must have one Panel root"));
    };
    let common = (
        WorkbenchRevision::new(2),
        candidate.terminal_contexts[0].terminal_context_id.clone(),
        created.tab_id.clone(),
        panel_id.clone(),
        primary_presentation.presentation_id.clone(),
    );
    let outcome = match case {
        NonAttachableCommit::Pending => WorkbenchCommandOutcome::TabCreatePending {
            revision: common.0,
            terminal_context_id: common.1,
            tab_id: common.2,
            panel_id: common.3,
            presentation_id: common.4,
        },
        NonAttachableCommit::Unavailable => WorkbenchCommandOutcome::TabCreated {
            revision: common.0,
            terminal_context_id: common.1,
            tab_id: common.2,
            panel_id: common.3,
            presentation_id: common.4,
            terminal_outcome: WorkbenchTerminalOutcome::Unavailable {
                reason: WorkbenchTerminalUnavailableReason::TerminalRuntimeUnavailable,
            },
        },
        NonAttachableCommit::Failed => WorkbenchCommandOutcome::TabCreated {
            revision: common.0,
            terminal_context_id: common.1,
            tab_id: common.2,
            panel_id: common.3,
            presentation_id: common.4,
            terminal_outcome: WorkbenchTerminalOutcome::Failed {
                reason: WorkbenchTerminalFailureReason::TerminalCreateFailed,
            },
        },
        NonAttachableCommit::OutcomeUnknown => WorkbenchCommandOutcome::TabCreated {
            revision: common.0,
            terminal_context_id: common.1,
            tab_id: common.2,
            panel_id: common.3,
            presentation_id: common.4,
            terminal_outcome: WorkbenchTerminalOutcome::OutcomeUnknown {},
        },
    };
    harness
        .carrier
        .respond_workbench(WorkbenchResponse::Command(WorkbenchCommandResult {
            protocol_version: ProtocolVersion,
            request_id: command.request_id,
            outcome,
        }))?;

    let retained = harness.ui.take_next()?;
    assert_eq!(harness.ui.state().phase_kind(), SessionPhaseKind::Creating);
    assert_eq!(harness.ui.state().desired().unwrap(), &old_key);
    assert_eq!(harness.ui.state().projection_key(), Some(&old_key));
    assert!(matches!(
        harness.ui.settle_taken(retained, FrameCut::None)?,
        UiEvent::CreateCommittedKnown { operation: ref actual } if actual == &operation
    ));
    assert_eq!(
        harness.ui.state().phase_kind(),
        SessionPhaseKind::CreateCommittedNeedDetach
    );
    assert_eq!(
        harness
            .ui
            .state()
            .committed_target()
            .map(|committed| committed.provenance()),
        Some(&expected_provenance)
    );

    let published = crate::terminal_session::state::AttachmentKey::new(
        operation.reserved_generation(),
        created.tab_id.clone(),
    );
    let read = harness.carrier.take_read()?;
    harness.carrier.respond_read(
        &read,
        WorkbenchSnapshotOutcome::Available {
            snapshot: candidate,
        },
    )?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::CreateProjection {
            operation: ref actual,
            decision: CreateProjectionDecision::Cleanup(_),
        } if actual == &operation
    ));
    assert_eq!(harness.ui.state().desired(), Some(&old_key));
    assert_eq!(harness.ui.state().projection_key(), Some(&old_key));
    assert!(harness.ui.state().ready().is_none());

    let ClientMessage::TerminalViewer(detach) = harness.carrier.take_obcs()? else {
        return Err(test_error(
            "validated candidate must license exact old Detach",
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
        UiEvent::CreateProjection {
            operation: ref actual,
            decision: CreateProjectionDecision::SettledWithoutAttachment,
        } if actual == &operation
    ));
    assert_eq!(
        harness.ui.state().phase_kind(),
        SessionPhaseKind::SelectedWithoutAttachment
    );
    assert_eq!(harness.ui.state().desired().unwrap(), &published);
    assert_eq!(harness.ui.state().provenance(1), Some(&expected_provenance));
    assert_eq!(harness.mailbox.is_closed(), None);

    // The scripted Server rejects any trailing request, so successful finish also proves that
    // non-attachable committed provenance did not issue Attach or replay tab.create.
    harness.finish()
}

#[test]
fn superseded_non_attachable_switch_skips_unattached_settlement_and_newest_gets_own_budget()
-> TestResult {
    let geometry = fixture_geometry()?;
    let mut kinds = vec![WireKind::Obcs, WireKind::Workbench];
    kinds.extend([WireKind::Obcs; 5]); // Initial Attach through accepted setup feedback.
    // Pending Create validates its Snapshot before cleaning up the old viewer.
    kinds.extend([WireKind::Workbench, WireKind::Workbench, WireKind::Obcs]);
    kinds.push(WireKind::Workbench); // Restored Switch Snapshot; no viewer remains to detach.
    kinds.extend([WireKind::Obcs; 5]);
    kinds.push(WireKind::Obcs); // Held Poll spans both Switch admissions.
    kinds.push(WireKind::Workbench); // Only the newest queued Switch reads a Snapshot.
    kinds.extend([WireKind::Obcs; 6]); // Exact old-viewer Detach, then successor setup.
    let mut harness = Harness::start(kinds, geometry)?;
    let (initial_key, _controller) = drive_controlling_ready(&mut harness, geometry)?;
    let (pending_key, _candidate) =
        drive_pending_create_to_selected_without_attachment(&mut harness, &initial_key)?;

    let restored = harness.ui.admit_switch(0)?;
    assert_eq!(restored.key.tab_id(), initial_key.tab_id());
    let restored_successor = match drive_switch_snapshot(&mut harness)? {
        UiEvent::SwitchSnapshot {
            work,
            decision: RefreshDecision::AttachRecovered(target),
        } if work == restored.key => {
            assert_eq!(
                target.selected().key().generation(),
                restored.key.generation() + 1
            );
            assert_eq!(target.provenance(), &CreateTerminalProvenance::BindingOnly);
            target.selected().key().clone()
        }
        _ => {
            return Err(test_error(
                "unattached Switch must recover through Snapshot",
            ));
        }
    };
    assert_eq!(drive_attach(&mut harness, 0)?, restored_successor);
    drive_controlling_after_attach(&mut harness, &restored_successor, geometry)?;

    harness
        .mailbox
        .try_enqueue(QueuedCommand::Poll(Tagged::new(
            restored_successor.clone(),
            (),
        )))
        .map_err(|error| test_error(&format!("failed to enqueue held Poll: {error:?}")))?;
    let ClientMessage::TerminalViewer(held_poll) = harness.carrier.take_obcs()? else {
        return Err(test_error("held Poll must reach the carrier"));
    };
    assert_eq!(
        held_poll.action(),
        &TerminalViewerAction::Poll {
            attachment_id: attached_viewer_id()?,
        }
    );

    let stale = harness.ui.admit_switch(1)?;
    assert_eq!(stale.key.tab_id(), pending_key.tab_id());
    harness.elapsed_millis.store(44_000, Ordering::SeqCst);
    harness.mailbox.wake_waiters();
    let newest = harness.ui.admit_switch(0)?;
    assert_eq!(newest.key.tab_id(), initial_key.tab_id());
    assert!(newest.deadlines.forward_cutoff() > stale.deadlines.forward_cutoff());

    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                held_poll.request_id().clone(),
                TerminalViewerOutcome::Empty {},
            ),
        ))?;

    assert!(matches!(
        drive_switch_snapshot(&mut harness)?,
        UiEvent::SwitchSnapshot {
            work,
            decision: RefreshDecision::Cleanup(_),
        } if work == newest.key
    ));

    let ClientMessage::TerminalViewer(detach) = harness.carrier.take_obcs()? else {
        return Err(test_error(
            "newest Switch must detach the installed old viewer",
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
    let successor = match harness.ui.settle_next(FrameCut::None)? {
        UiEvent::SwitchOldDetached {
            work,
            detached,
            continuation: SwitchDetachContinuation::Attach(target),
        } if work == newest.key && detached == restored_successor => {
            assert_eq!(
                target.selected().key().generation(),
                newest.key.generation() + 1
            );
            assert_eq!(target.deadlines(), newest.deadlines);
            assert_eq!(target.provenance(), &CreateTerminalProvenance::BindingOnly);
            target.selected().key().clone()
        }
        _ => return Err(test_error("newest Switch cleanup must publish a successor")),
    };
    assert_eq!(harness.ui.state().desired().unwrap(), &successor);
    assert_eq!(harness.ui.state().phase_kind(), SessionPhaseKind::Switching);

    // The stale marker's forward cutoff is 45s. Crossing it here proves that the queued newest
    // marker starts from its own sample at 44s. If the stale marker published
    // SetupWithoutAttachment, that event would occupy the rendezvous before this exact Attach.
    harness.elapsed_millis.store(46_000, Ordering::SeqCst);
    harness.mailbox.wake_waiters();
    assert_eq!(drive_attach(&mut harness, 0)?, successor);
    drive_controlling_after_attach(&mut harness, &successor, geometry)?;
    assert_eq!(harness.ui.state().phase_kind(), SessionPhaseKind::Ready);
    assert_eq!(
        harness.ui.state().active().map(|tab| tab.key()),
        Some(&successor)
    );
    assert_eq!(
        harness.ui.state().provenance(1),
        Some(&CreateTerminalProvenance::Pending)
    );

    harness.finish()
}

fn drive_pending_create_to_selected_without_attachment(
    harness: &mut Harness,
    old_key: &crate::terminal_session::state::AttachmentKey,
) -> TestResult<(
    crate::terminal_session::state::AttachmentKey,
    orchbee_workbench_protocol::Snapshot,
)> {
    let operation = harness.ui.admit_create()?;
    let candidate = snapshot(2, 2);
    let created = &candidate.terminal_contexts[0].tabs[1];
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = &created.root
    else {
        return Err(test_error("created fixture must have one Panel root"));
    };
    let committed_key = crate::terminal_session::state::AttachmentKey::new(
        operation.reserved_generation(),
        created.tab_id.clone(),
    );
    let WorkbenchRequest::Command(command) = harness.carrier.take_workbench()? else {
        return Err(test_error("Create admission must submit tab.create"));
    };
    harness
        .carrier
        .respond_workbench(WorkbenchResponse::Command(WorkbenchCommandResult {
            protocol_version: ProtocolVersion,
            request_id: command.request_id,
            outcome: WorkbenchCommandOutcome::TabCreatePending {
                revision: WorkbenchRevision::new(2),
                terminal_context_id: candidate.terminal_contexts[0].terminal_context_id.clone(),
                tab_id: created.tab_id.clone(),
                panel_id: panel_id.clone(),
                presentation_id: primary_presentation.presentation_id.clone(),
            },
        }))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::CreateCommittedKnown { operation: ref actual } if actual == &operation
    ));

    let read = harness.carrier.take_read()?;
    harness.carrier.respond_read(
        &read,
        WorkbenchSnapshotOutcome::Available {
            snapshot: candidate.clone(),
        },
    )?;
    assert!(
        matches!(harness.ui.settle_next(FrameCut::None)?, UiEvent::CreateProjection {
        operation: ref actual, decision: CreateProjectionDecision::Cleanup(_),
    } if actual == &operation)
    );
    let ClientMessage::TerminalViewer(detach) = harness.carrier.take_obcs().map_err(|error| {
        test_error(&format!(
            "committed Create cleanup after Snapshot must read Detach: {error}"
        ))
    })?
    else {
        return Err(test_error("committed Create must detach the old viewer"));
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
        UiEvent::CreateProjection {
            operation: ref actual,
            decision: CreateProjectionDecision::SettledWithoutAttachment,
        } if actual == &operation
    ));
    assert_eq!(
        harness.ui.state().phase_kind(),
        SessionPhaseKind::SelectedWithoutAttachment
    );
    assert!(!harness.ui.state().selected_has_viewer());
    assert_eq!(harness.ui.state().desired().unwrap(), &committed_key);
    assert_eq!(
        harness.ui.state().provenance(1),
        Some(&CreateTerminalProvenance::Pending)
    );
    assert_ne!(old_key, &committed_key);
    Ok((committed_key, candidate))
}

#[test]
fn started_create_accepts_a_later_renamed_snapshot_and_attaches_the_exact_binding() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(
        vec![
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
        ],
        geometry,
    )?;
    let (old_key, _old_controller) = drive_controlling_ready(&mut harness, geometry)?;
    let operation = harness.ui.admit_create()?;
    let candidate = snapshot(2, 2);

    let WorkbenchRequest::Command(command) = harness.carrier.take_workbench()? else {
        return Err(test_error("Create admission must submit tab.create"));
    };
    let created = &candidate.terminal_contexts[0].tabs[1];
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = &created.root
    else {
        return Err(test_error("created fixture must have one Panel root"));
    };
    let expected_new_tab = created.tab_id.clone();
    let WorkbenchAction::TabCreate {
        geometry: captured_geometry,
        label: requested_label,
        ..
    } = &command.action
    else {
        return Err(test_error("expected tab.create"));
    };
    assert_eq!(requested_label.as_str(), "2");
    harness
        .carrier
        .respond_workbench(WorkbenchResponse::Command(WorkbenchCommandResult {
            protocol_version: ProtocolVersion,
            request_id: command.request_id,
            outcome: WorkbenchCommandOutcome::TabCreated {
                revision: WorkbenchRevision::new(2),
                terminal_context_id: candidate.terminal_contexts[0].terminal_context_id.clone(),
                tab_id: expected_new_tab.clone(),
                panel_id: panel_id.clone(),
                presentation_id: primary_presentation.presentation_id.clone(),
                terminal_outcome: WorkbenchTerminalOutcome::Started {
                    geometry: *captured_geometry,
                },
            },
        }))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::CreateCommittedKnown { operation: ref actual } if actual == &operation
    ));
    assert_eq!(
        harness.ui.state().phase_kind(),
        SessionPhaseKind::CreateCommittedNeedDetach
    );
    assert_eq!(harness.ui.state().desired().unwrap(), &old_key);

    let new_key = crate::terminal_session::state::AttachmentKey::new(
        operation.reserved_generation(),
        expected_new_tab.clone(),
    );
    let read = harness.carrier.take_read()?;
    let mut renamed = candidate;
    renamed.revision = WorkbenchRevision::new(3);
    renamed.terminal_contexts[0].tabs[1].label = "renamed-after-create".to_owned();
    harness.carrier.respond_read(
        &read,
        WorkbenchSnapshotOutcome::Available { snapshot: renamed },
    )?;
    assert!(
        matches!(harness.ui.settle_next(FrameCut::None)?, UiEvent::CreateProjection {
        operation: ref actual, decision: CreateProjectionDecision::Cleanup(_),
    } if actual == &operation)
    );
    let ClientMessage::TerminalViewer(detach) = harness.carrier.take_obcs()? else {
        return Err(test_error(
            "committed Started Create must detach the old viewer",
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
        UiEvent::CreateProjection {
            operation: ref actual,
            decision: CreateProjectionDecision::Attach(ref target),
        } if actual == &operation
            && target.selected().key() == &new_key
            && target.provenance() == &CreateTerminalProvenance::Started
    ));
    assert_eq!(harness.ui.state().phase_kind(), SessionPhaseKind::Switching);
    assert_eq!(
        harness.ui.state().provenance(1),
        Some(&CreateTerminalProvenance::Started)
    );
    assert_eq!(
        harness.ui.state().cache().revision(),
        WorkbenchRevision::new(3)
    );
    let selected = harness
        .ui
        .state()
        .active()
        .ok_or_else(|| test_error("renamed created tab must be selected"))?;
    assert_eq!(selected.key(), &new_key);
    assert_eq!(selected.index(), 1);
    let renamed_tab = harness
        .ui
        .state()
        .tab(1)
        .ok_or_else(|| test_error("renamed created tab must be cached"))?;
    assert_eq!(renamed_tab.tab_label().as_str(), "renamed-after-create");
    assert_eq!(renamed_tab.presentation_label().as_str(), "2");
    let retained_binding = renamed_tab.binding().clone();

    assert_eq!(drive_attach(&mut harness, 1)?, new_key);
    let new_controller = drive_controlling_after_attach(&mut harness, &new_key, geometry)?;
    assert_eq!(harness.ui.state().phase_kind(), SessionPhaseKind::Ready);
    assert_eq!(harness.ui.state().desired().unwrap(), &new_key);
    assert_eq!(
        harness.ui.state().active().map(|selected| selected.index()),
        Some(1)
    );
    assert_eq!(
        harness.ui.state().controller_mode(),
        Some(&ControllerMode::Controlling(new_controller))
    );
    let renamed_tab = harness
        .ui
        .state()
        .tab(1)
        .ok_or_else(|| test_error("attached renamed tab must remain cached"))?;
    assert_eq!(renamed_tab.tab_label().as_str(), "renamed-after-create");
    assert_eq!(renamed_tab.presentation_label().as_str(), "2");
    assert_eq!(renamed_tab.binding(), &retained_binding);
    assert_eq!(harness.mailbox.is_closed(), None);

    harness.finish()
}

#[test]
fn late_known_create_without_candidate_retains_commit_but_never_detaches() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(
        vec![
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Workbench,
        ],
        geometry,
    )?;
    let (old_key, _controller) = drive_controlling_ready(&mut harness, geometry)?;
    let operation = harness.ui.admit_create()?;
    let candidate = snapshot(2, 2);

    let WorkbenchRequest::Command(command) = harness.carrier.take_workbench()? else {
        return Err(test_error("Create admission must submit tab.create"));
    };
    let created = &candidate.terminal_contexts[0].tabs[1];
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = &created.root
    else {
        return Err(test_error("created fixture must have one Panel root"));
    };
    harness
        .carrier
        .respond_workbench(WorkbenchResponse::Command(WorkbenchCommandResult {
            protocol_version: ProtocolVersion,
            request_id: command.request_id,
            outcome: WorkbenchCommandOutcome::TabCreatePending {
                revision: WorkbenchRevision::new(2),
                terminal_context_id: candidate.terminal_contexts[0].terminal_context_id.clone(),
                tab_id: created.tab_id.clone(),
                panel_id: panel_id.clone(),
                presentation_id: primary_presentation.presentation_id.clone(),
            },
        }))?;

    let retained = harness.ui.take_next()?;
    // A known commit remains recoverable through the total cleanup reserve. The UI may retain it
    // after the forward cutoff because this acknowledgement is the only source of the diagnostic
    // commit; it still cannot publish the new desired generation after that cutoff.
    harness.elapsed_millis.store(46_000, Ordering::SeqCst);
    assert!(matches!(
        harness.ui.settle_taken(retained, FrameCut::None)?,
        UiEvent::CreateCommittedKnown { operation: ref actual } if actual == &operation
    ));

    // No validated candidate exists before the original forward cutoff: no cleanup is licensed.
    assert!(harness.ui.take_next().is_err());
    assert_eq!(harness.mailbox.is_closed(), Some(CarrierFailure::Deadline));
    assert_eq!(
        harness.ui.state().phase_kind(),
        SessionPhaseKind::CreateCommittedNeedDetach
    );
    assert_eq!(harness.ui.state().desired().unwrap(), &old_key);
    assert_eq!(harness.ui.state().projection_key(), Some(&old_key));
    assert_eq!(
        harness
            .ui
            .state()
            .committed_target()
            .map(|committed| committed.provenance()),
        Some(&CreateTerminalProvenance::Pending)
    );

    harness.finish_after_worker_failure(CarrierFailure::Deadline)
}

#[test]
fn refresh_uses_one_pending_generation_and_ui_settles_after_transport_deadline() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(
        vec![
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Workbench,
        ],
        geometry,
    )?;
    let (active, _controller) = drive_controlling_ready(&mut harness, geometry)?;
    let generation = harness.ui.admit_refresh()?;
    assert_eq!(generation, 1);
    assert_eq!(
        harness.ui.state().pending_refresh_generation(),
        Some(generation)
    );
    assert!(harness.ui.admit_refresh().is_err());
    assert_eq!(
        harness.ui.state().pending_refresh_generation(),
        Some(generation)
    );

    let read = harness.carrier.take_read()?;
    assert_eq!(read.request_id().as_str(), identity(0x108));
    assert_eq!(
        harness.mailbox.pending_refresh_stage(),
        Some((generation, PendingRefreshStage::InFlight))
    );
    harness.carrier.respond_read(
        &read,
        WorkbenchSnapshotOutcome::Available {
            snapshot: snapshot(2, 2),
        },
    )?;

    let completion = harness.ui.take_next()?;
    assert_eq!(
        harness.mailbox.pending_refresh_stage(),
        Some((generation, PendingRefreshStage::AwaitingUi))
    );
    assert_eq!(
        harness.ui.state().refresh_status(),
        Some(NavigatorRefreshStatus::Refreshing)
    );
    harness.elapsed_millis.store(51_000, Ordering::SeqCst);
    assert!(matches!(
        harness.ui.settle_taken(completion, FrameCut::None)?,
        UiEvent::Refresh {
            generation: actual,
            decision: RefreshDecision::Continue,
        } if actual == generation
    ));
    assert_eq!(harness.mailbox.pending_refresh_stage(), None);
    assert_eq!(
        harness.ui.state().cache().revision(),
        WorkbenchRevision::new(2)
    );
    assert_eq!(harness.ui.state().desired().unwrap(), &active);
    assert_eq!(
        harness.ui.state().active().map(|tab| tab.key()),
        Some(&active)
    );
    assert_eq!(harness.ui.state().projection_key(), Some(&active));
    assert_eq!(
        harness.ui.state().refresh_status(),
        Some(NavigatorRefreshStatus::Success(WorkbenchRevision::new(2)))
    );
    assert_eq!(harness.mailbox.is_closed(), None);

    harness.finish()
}

#[test]
fn label_only_refresh_preserves_selection_viewer_frame_and_presentation_anchor() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut initial = snapshot(1, 1);
    set_tab_labels(&mut initial, 0, "old", "creation-anchor")?;
    let mut refreshed = initial.clone();
    refreshed.revision = WorkbenchRevision::new(2);
    refreshed.terminal_contexts[0].tabs[0].label = "renamed".to_owned();

    let mut harness = Harness::start(
        vec![
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Workbench,
        ],
        geometry,
    )?;
    let (active_key, controller) =
        drive_controlling_ready_from_snapshot(&mut harness, initial, geometry)?;
    let selected = harness
        .ui
        .state()
        .active()
        .cloned()
        .ok_or_else(|| test_error("Ready state must have an active tab"))?;
    let attached_key = harness
        .ui
        .state()
        .attached_key()
        .cloned()
        .ok_or_else(|| test_error("Ready state must retain its attached key"))?;
    let projection_key = harness
        .ui
        .state()
        .projection_key()
        .cloned()
        .ok_or_else(|| test_error("Ready state must retain its projection key"))?;
    let provenance = harness
        .ui
        .state()
        .provenance(selected.index())
        .cloned()
        .ok_or_else(|| test_error("Ready state must retain selected provenance"))?;
    let binding = harness
        .ui
        .state()
        .tab(selected.index())
        .ok_or_else(|| test_error("Ready cache must contain the selected tab"))?
        .binding()
        .clone();
    let projection_frame = harness
        .ui
        .projection_frame()
        .map(|frame| {
            (
                frame.route,
                frame.geometry,
                frame.cursor,
                frame.alternate_screen,
                frame.foreground,
                frame.background,
                frame.theme_revision,
                frame.rows.clone(),
            )
        })
        .ok_or_else(|| test_error("Ready projection must have a frame"))?;
    let drawn_frames = harness.ui.drawn_frames();
    let no_frame_redraws = harness.ui.no_frame_redraws();

    let generation = harness.ui.admit_refresh()?;
    let read = harness.carrier.take_read()?;
    harness.carrier.respond_read(
        &read,
        WorkbenchSnapshotOutcome::Available {
            snapshot: refreshed,
        },
    )?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Refresh {
            generation: actual,
            decision: RefreshDecision::Continue,
        } if actual == generation
    ));

    let renamed = harness
        .ui
        .state()
        .tab(0)
        .ok_or_else(|| test_error("renamed tab must remain cached"))?;
    assert_eq!(renamed.tab_label().as_str(), "renamed");
    assert_eq!(renamed.presentation_label().as_str(), "creation-anchor");
    assert_eq!(renamed.binding(), &binding);
    assert_eq!(harness.ui.state().active(), Some(&selected));
    assert_eq!(harness.ui.state().desired().unwrap(), &active_key);
    assert_eq!(harness.ui.state().attached_key(), Some(&attached_key));
    assert_eq!(harness.ui.state().projection_key(), Some(&projection_key));
    assert_eq!(
        harness.ui.state().controller_mode(),
        Some(&ControllerMode::Controlling(controller))
    );
    assert_eq!(
        harness.ui.state().provenance(selected.index()),
        Some(&provenance)
    );
    let retained_projection_frame = harness
        .ui
        .projection_frame()
        .map(|frame| {
            (
                frame.route,
                frame.geometry,
                frame.cursor,
                frame.alternate_screen,
                frame.foreground,
                frame.background,
                frame.theme_revision,
                frame.rows.clone(),
            )
        })
        .ok_or_else(|| test_error("label Refresh must retain the projection frame"))?;
    assert_eq!(retained_projection_frame, projection_frame);
    assert_eq!(harness.ui.drawn_frames(), drawn_frames);
    assert_eq!(harness.ui.no_frame_redraws(), no_frame_redraws);
    assert_eq!(
        harness.ui.state().refresh_status(),
        Some(NavigatorRefreshStatus::Success(WorkbenchRevision::new(2)))
    );
    assert_eq!(harness.mailbox.pending_refresh_stage(), None);
    assert_eq!(harness.mailbox.is_closed(), None);

    harness.finish()
}

#[test]
fn denied_refresh_preserves_cache_viewer_and_reports_baseline_failure() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(
        vec![
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Workbench,
        ],
        geometry,
    )?;
    let (active, _controller) = drive_controlling_ready(&mut harness, geometry)?;
    let generation = harness.ui.admit_refresh()?;
    let read = harness.carrier.take_read()?;
    harness
        .carrier
        .respond_read(&read, WorkbenchSnapshotOutcome::Denied {})?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Refresh {
            generation: actual,
            decision: RefreshDecision::Continue,
        } if actual == generation
    ));
    assert_eq!(harness.mailbox.pending_refresh_stage(), None);
    assert_eq!(
        harness.ui.state().refresh_status(),
        Some(NavigatorRefreshStatus::Failure(WorkbenchRevision::new(1)))
    );
    assert_eq!(
        harness.ui.state().cache().revision(),
        WorkbenchRevision::new(1)
    );
    assert_eq!(harness.ui.state().desired().unwrap(), &active);
    assert_eq!(
        harness.ui.state().active().map(|tab| tab.key()),
        Some(&active)
    );
    assert_eq!(harness.ui.state().projection_key(), Some(&active));
    assert!(harness.ui.projection_is_some());
    assert_eq!(harness.mailbox.is_closed(), None);

    harness.finish()
}

#[test]
fn settled_refresh_reply_can_be_superseded_before_worker_resume_and_then_runs_exact_switch()
-> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(
        vec![
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
        ],
        geometry,
    )?;
    let (old_key, _old_controller) =
        drive_controlling_ready_from_snapshot(&mut harness, snapshot(2, 1), geometry)?;

    let refresh_generation = harness.ui.admit_refresh()?;
    let read = harness.carrier.take_read()?;
    harness
        .carrier
        .respond_read(&read, WorkbenchSnapshotOutcome::Denied {})?;
    let refresh_result = harness.ui.take_next()?;
    assert_eq!(
        harness.mailbox.pending_refresh_stage(),
        Some((refresh_generation, PendingRefreshStage::AwaitingUi))
    );

    // The helper deposits the exact current Refresh reply while the worker is waiting, pauses the
    // worker after it observes that reply but before it reacquires the mailbox, and admits the
    // later Switch through the production mailbox transaction. This is the desired-fence cut: the
    // deposited reply names the old desired key while the resumed worker observes the new barrier.
    let (settled, superseding) = harness.ui.settle_refresh_and_supersede(refresh_result, 1)?;
    assert!(matches!(
        settled,
        UiEvent::Refresh {
            generation,
            decision: RefreshDecision::Continue,
        } if generation == refresh_generation
    ));
    assert_eq!(superseding.key.generation(), old_key.generation() + 1);
    assert_eq!(
        superseding.key.tab_id(),
        harness
            .ui
            .state()
            .tab(1)
            .ok_or_else(|| test_error("Switch target must remain in the settled cache"))?
            .tab_id()
    );
    assert_eq!(harness.mailbox.pending_refresh_stage(), None);
    assert_eq!(
        harness.ui.state().refresh_status(),
        Some(NavigatorRefreshStatus::Failure(WorkbenchRevision::new(1)))
    );
    assert_eq!(harness.ui.state().phase_kind(), SessionPhaseKind::Switching);
    assert_eq!(harness.ui.state().desired().unwrap(), &superseding.key);
    assert_eq!(harness.ui.state().attached_key(), Some(&old_key));

    assert!(matches!(
        drive_switch_snapshot(&mut harness)?,
        UiEvent::SwitchSnapshot {
            work,
            decision: RefreshDecision::Cleanup(_),
        } if work == superseding.key
    ));

    let ClientMessage::TerminalViewer(detach) = harness.carrier.take_obcs()? else {
        return Err(test_error(
            "resumed Refresh worker must advance to the exact queued Switch",
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
    let successor = match harness.ui.settle_next(FrameCut::None)? {
        UiEvent::SwitchOldDetached {
            work,
            detached,
            continuation: SwitchDetachContinuation::Attach(target),
        } if work == superseding.key && detached == old_key => {
            assert_eq!(
                target.selected().key().generation(),
                superseding.key.generation() + 1
            );
            assert_eq!(target.deadlines(), superseding.deadlines);
            assert_eq!(target.provenance(), &CreateTerminalProvenance::BindingOnly);
            target.selected().key().clone()
        }
        _ => return Err(test_error("Switch cleanup must publish a fresh successor")),
    };

    assert_eq!(drive_attach(&mut harness, 1)?, successor);
    let controller = drive_controlling_after_attach(&mut harness, &successor, geometry)?;
    assert_eq!(harness.ui.state().phase_kind(), SessionPhaseKind::Ready);
    assert_eq!(
        harness.ui.state().active().map(|selected| selected.key()),
        Some(&successor)
    );
    assert_eq!(
        harness.ui.state().controller_mode(),
        Some(&ControllerMode::Controlling(controller))
    );
    assert_eq!(
        harness.ui.state().refresh_status(),
        Some(NavigatorRefreshStatus::Failure(WorkbenchRevision::new(1)))
    );
    assert_eq!(harness.mailbox.is_closed(), None);

    harness.finish()
}

#[test]
fn refresh_deadline_overflow_changes_no_cache_viewer_or_refresh_facts() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start_with_ceiling_clock(
        vec![
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
        ],
        geometry,
    )?;
    let (active, controller) = drive_controlling_ready(&mut harness, geometry)?;
    let baseline_provenance = harness
        .ui
        .state()
        .provenance(0)
        .cloned()
        .ok_or_else(|| test_error("Ready tab must have provenance"))?;
    let baseline_drawn = harness.ui.drawn_frames();
    let baseline_no_frame = harness.ui.no_frame_redraws();
    assert_eq!(harness.ui.state().refresh_status(), None);
    assert_eq!(harness.ui.state().pending_refresh_generation(), None);
    assert_eq!(harness.mailbox.pending_refresh_stage(), None);

    harness.advance_clock_to_ceiling()?;
    assert!(harness.ui.admit_refresh().is_err());

    assert_eq!(harness.ui.state().phase_kind(), SessionPhaseKind::Ready);
    assert_eq!(
        harness.ui.state().cache().revision(),
        WorkbenchRevision::new(1)
    );
    assert_eq!(harness.ui.state().desired().unwrap(), &active);
    assert_eq!(
        harness.ui.state().active().map(|tab| tab.key()),
        Some(&active)
    );
    assert_eq!(harness.ui.state().projection_key(), Some(&active));
    assert_eq!(
        harness.ui.state().controller_mode(),
        Some(&ControllerMode::Controlling(controller))
    );
    assert_eq!(harness.ui.state().provenance(0), Some(&baseline_provenance));
    assert!(harness.ui.state().selected_has_viewer());
    assert!(harness.ui.state().feedback_accepted());
    assert!(harness.ui.projection_is_some());
    assert_eq!(harness.ui.drawn_frames(), baseline_drawn);
    assert_eq!(harness.ui.no_frame_redraws(), baseline_no_frame);
    assert_eq!(harness.ui.state().refresh_status(), None);
    assert_eq!(harness.ui.state().pending_refresh_generation(), None);
    assert_eq!(harness.mailbox.pending_refresh_stage(), None);
    assert_eq!(harness.mailbox.is_closed(), None);

    // The exact startup-only script proves overflow admitted no Refresh work.
    harness.finish()
}

#[test]
fn refresh_barrier_expiring_held_poll_retires_deadline_before_snapshot() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(
        vec![
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
        ],
        geometry,
    )?;
    let (active, _controller) = drive_controlling_ready(&mut harness, geometry)?;
    harness
        .mailbox
        .try_enqueue(QueuedCommand::Poll(Tagged::new(active.clone(), ())))
        .map_err(|error| test_error(&format!("failed to enqueue held Poll: {error:?}")))?;
    let ClientMessage::TerminalViewer(poll) = harness.carrier.take_obcs()? else {
        return Err(test_error("held Poll must reach the carrier"));
    };
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(poll.request_id().clone(), TerminalViewerOutcome::Empty {}),
        ))?;
    let _held_poll_event = harness.ui.take_next()?;
    let generation = harness.ui.admit_refresh()?;
    assert_eq!(
        harness.ui.state().pending_refresh_generation(),
        Some(generation)
    );

    // Both the already-issued Poll and later Refresh use the exact 50-second cutoff. The later
    // Refresh barrier still must not reclassify the expired ordinary event as a successful drain.
    harness.elapsed_millis.store(50_000, Ordering::SeqCst);
    harness.mailbox.wake_waiters();
    assert!(harness.ui.take_next().is_err());
    assert_eq!(harness.mailbox.is_closed(), Some(CarrierFailure::Deadline));
    assert_eq!(harness.mailbox.pending_refresh_stage(), None);
    assert_eq!(
        harness.ui.state().refresh_status(),
        Some(NavigatorRefreshStatus::Refreshing)
    );
    assert_eq!(
        harness.ui.state().cache().revision(),
        WorkbenchRevision::new(1)
    );
    assert_eq!(harness.ui.state().desired().unwrap(), &active);
    assert_eq!(harness.ui.state().projection_key(), Some(&active));

    // Exact scripted exhaustion proves that expiry did not advance to SnapshotRead or Detach.
    harness.finish_after_worker_failure(CarrierFailure::Deadline)
}

#[test]
fn divergent_refresh_retires_after_exact_typed_ui_settlement() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut initial = snapshot(1, 1);
    set_tab_labels(&mut initial, 0, "old", "creation-anchor")?;
    let mut divergent = initial.clone();
    divergent.terminal_contexts[0].tabs[0].label = "same-revision-rename".to_owned();
    let mut harness = Harness::start(
        vec![
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Workbench,
        ],
        geometry,
    )?;
    let (_active, _controller) =
        drive_controlling_ready_from_snapshot(&mut harness, initial, geometry)?;
    let _generation = harness.ui.admit_refresh()?;
    let read = harness.carrier.take_read()?;
    // The refresh read is a publication read once the baseline exists; a snapshot publication
    // that cannot advance the held revision is authority divergence and folds to a hard retire.
    let request_id = read.request_id().clone();
    harness
        .carrier
        .respond_workbench(WorkbenchResponse::Publication(
            orchbee_workbench_protocol::WorkbenchPublicationResult {
                protocol_version: ProtocolVersion,
                request_id,
                outcome: orchbee_workbench_protocol::WorkbenchPublicationOutcome::Published {
                    publications: vec![
                        orchbee_workbench_protocol::WorkbenchPublicationDocument::Snapshot(
                            divergent,
                        ),
                    ],
                },
            },
        ))?;
    // The fold failure retires the worker before any typed Refresh event reaches the UI; the
    // untouched cache still holds the baseline revision and labels.
    assert!(harness.ui.take_next().is_err());
    assert_eq!(
        harness.ui.state().cache().revision(),
        WorkbenchRevision::new(1)
    );
    let retained = harness
        .ui
        .state()
        .tab(0)
        .ok_or_else(|| test_error("divergent Refresh must retain the prior cached tab"))?;
    assert_eq!(retained.tab_label().as_str(), "old");
    assert_eq!(retained.presentation_label().as_str(), "creation-anchor");
    assert_eq!(
        harness.mailbox.is_closed(),
        Some(CarrierFailure::AuthorityDivergence)
    );

    harness.finish_after_worker_failure(CarrierFailure::AuthorityDivergence)
}

#[test]
fn refresh_folds_a_multi_revision_publication_batch_into_one_validated_snapshot() -> TestResult {
    let geometry = fixture_geometry()?;
    // Baseline revision 1: tab0 is the attached active leaf; tab1 starts as a single leaf.
    let initial = snapshot(2, 1);
    // Two committed splits on inactive tab1 since the baseline: the leaf split once, then its
    // fresh sibling split again. Each Delta upserts the whole terminal context.
    let mut mid = initial.clone();
    mid.revision = WorkbenchRevision::new(2);
    mid.terminal_contexts[0].tabs[1].root = split_node(
        panel_node(1011, 1012, 1013, 1014, "2"),
        1020,
        panel_node(1030, 1031, 1032, 1033, "2-b"),
        1021,
        PanelOrientation::Horizontal,
        1022,
    );
    let mut latest = mid.clone();
    latest.revision = WorkbenchRevision::new(3);
    latest.terminal_contexts[0].tabs[1].root = split_node(
        panel_node(1011, 1012, 1013, 1014, "2"),
        1020,
        split_node(
            panel_node(1030, 1031, 1032, 1033, "2-b"),
            1040,
            panel_node(1050, 1051, 1052, 1053, "2-c"),
            1041,
            PanelOrientation::Vertical,
            1042,
        ),
        1021,
        PanelOrientation::Horizontal,
        1022,
    );

    let mut harness = Harness::start(
        vec![
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Workbench,
        ],
        geometry,
    )?;
    let (active, _controller) =
        drive_controlling_ready_from_snapshot(&mut harness, initial.clone(), geometry)?;
    let generation = harness.ui.admit_refresh()?;
    let read = harness.carrier.take_read()?;
    let ObservedRead::Publication(intent) = &read else {
        return Err(test_error(
            "a retained baseline must issue publication.read",
        ));
    };
    assert_eq!(intent.known_revision, WorkbenchRevision::new(1));
    let delta = |base: u64, revision: u64, terminal_context: &TerminalContext| {
        WorkbenchPublicationDocument::Delta(Delta {
            base_revision: WorkbenchRevision::new(base),
            changes: vec![DeltaChange::TerminalContextUpserted {
                terminal_context: terminal_context.clone(),
            }],
            protocol_version: ProtocolVersion,
            revision: WorkbenchRevision::new(revision),
            site_id: initial.site_id.clone(),
        })
    };
    harness
        .carrier
        .respond_workbench(WorkbenchResponse::Publication(WorkbenchPublicationResult {
            protocol_version: ProtocolVersion,
            request_id: intent.request_id.clone(),
            outcome: WorkbenchPublicationOutcome::Published {
                publications: vec![
                    delta(1, 2, &mid.terminal_contexts[0]),
                    delta(2, 3, &latest.terminal_contexts[0]),
                ],
            },
        }))?;

    let completion = harness.ui.take_next()?;
    assert!(matches!(
        harness.ui.settle_taken(completion, FrameCut::None)?,
        UiEvent::Refresh {
            generation: actual,
            decision: RefreshDecision::Continue,
        } if actual == generation
    ));
    assert_eq!(
        harness.ui.state().cache().revision(),
        WorkbenchRevision::new(3)
    );
    let caught_up = harness
        .ui
        .state()
        .tab(1)
        .ok_or_else(|| test_error("folded refresh must retain the second tab"))?;
    assert_eq!(caught_up.panel_count(), 3);
    assert_eq!(
        harness.ui.state().active().map(|tab| tab.key()),
        Some(&active)
    );
    assert_eq!(harness.mailbox.is_closed(), None);

    harness.finish()
}

#[test]
fn refresh_validates_a_journal_gap_snapshot_fallback_against_the_retained_baseline() -> TestResult {
    let geometry = fixture_geometry()?;
    let initial = snapshot(2, 1);
    // The Server journal cannot connect the requested revision, so it answers publication.read
    // with one full Snapshot publication covering the same folded history.
    let mut fallback = initial.clone();
    fallback.revision = WorkbenchRevision::new(4);
    fallback.terminal_contexts[0].tabs[1].root = split_node(
        panel_node(1011, 1012, 1013, 1014, "2"),
        1020,
        split_node(
            panel_node(1030, 1031, 1032, 1033, "2-b"),
            1040,
            panel_node(1050, 1051, 1052, 1053, "2-c"),
            1041,
            PanelOrientation::Vertical,
            1042,
        ),
        1021,
        PanelOrientation::Horizontal,
        1022,
    );

    let mut harness = Harness::start(
        vec![
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Workbench,
        ],
        geometry,
    )?;
    let (active, _controller) =
        drive_controlling_ready_from_snapshot(&mut harness, initial, geometry)?;
    let generation = harness.ui.admit_refresh()?;
    let read = harness.carrier.take_read()?;
    harness.carrier.respond_read(
        &read,
        WorkbenchSnapshotOutcome::Available { snapshot: fallback },
    )?;

    let completion = harness.ui.take_next()?;
    assert!(matches!(
        harness.ui.settle_taken(completion, FrameCut::None)?,
        UiEvent::Refresh {
            generation: actual,
            decision: RefreshDecision::Continue,
        } if actual == generation
    ));
    assert_eq!(
        harness.ui.state().cache().revision(),
        WorkbenchRevision::new(4)
    );
    let caught_up = harness
        .ui
        .state()
        .tab(1)
        .ok_or_else(|| test_error("snapshot fallback must retain the second tab"))?;
    assert_eq!(caught_up.panel_count(), 3);
    assert_eq!(
        harness.ui.state().active().map(|tab| tab.key()),
        Some(&active)
    );
    assert_eq!(harness.mailbox.is_closed(), None);

    harness.finish()
}

#[test]
fn started_projection_recovery_uses_fresh_budget_same_generation_and_new_ids() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(
        vec![
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Workbench,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
        ],
        geometry,
    )?;
    let (operation, candidate, committed_key) =
        drive_started_create_to_projection_unavailable(&mut harness, geometry)?;

    // The original Create budget has expired before the human requests recovery.
    harness.elapsed_millis.store(60_000, Ordering::SeqCst);
    let refresh_generation = harness.ui.admit_refresh()?;
    assert_eq!(refresh_generation, 1);
    let refresh = harness.carrier.take_read()?;
    assert_eq!(refresh.request_id().as_str(), identity(0x10a));
    // The retained baseline revision is the cursor the Server journal fast-forwards from.
    assert_eq!(refresh.known_revision(), Some(WorkbenchRevision::new(1)));
    harness.carrier.respond_read(
        &refresh,
        WorkbenchSnapshotOutcome::Available {
            snapshot: candidate,
        },
    )?;

    let completion = harness.ui.take_next()?;
    // The transport result was placed before its 110-second deadline. Semantic UI settlement is
    // unbounded, and at 120 seconds it must sample a new setup budget instead of reusing either
    // the expired Create deadline or the completed Refresh deadline.
    harness.elapsed_millis.store(120_000, Ordering::SeqCst);
    assert!(
        matches!(harness.ui.settle_taken(completion, FrameCut::None)?, UiEvent::Refresh {
        generation, decision: RefreshDecision::Cleanup(_),
    } if generation == refresh_generation)
    );
    let ClientMessage::TerminalViewer(detach) = harness.carrier.take_obcs()? else {
        return Err(test_error("recovery candidate must clean old viewer"));
    };
    assert_eq!(
        detach.action(),
        &TerminalViewerAction::Detach {
            attachment_id: attached_viewer_id()?,
        }
    );
    assert_eq!(detach.request_id().as_str(), identity(0x10b));
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                detach.request_id().clone(),
                TerminalViewerOutcome::Detached {},
            ),
        ))?;
    let completion = harness.ui.take_next()?;
    assert!(matches!(
        harness.ui.settle_taken(completion, FrameCut::None)?,
        UiEvent::Refresh {
            generation,
            decision: RefreshDecision::AttachRecovered(ref target),
        } if generation == refresh_generation
            && target.selected().key() == &committed_key
            && target.selected().key().generation() == operation.reserved_generation()
            && target.provenance() == &CreateTerminalProvenance::Started
    ));
    assert_eq!(harness.ui.state().phase_kind(), SessionPhaseKind::Switching);
    assert_eq!(harness.ui.state().desired().unwrap(), &committed_key);

    let ClientMessage::TerminalViewer(attach) = harness.carrier.take_obcs()? else {
        return Err(test_error(
            "fresh recovery budget must license AttachBinding",
        ));
    };
    assert_eq!(attach.action(), &attach_action(1)?);
    assert_eq!(attach.request_id().as_str(), identity(0x10c));
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(attach.request_id().clone(), attached_outcome()?),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Attached { key: ref actual } if actual == &committed_key
    ));
    let controller = drive_controlling_after_attach(&mut harness, &committed_key, geometry)?;
    assert_eq!(harness.ui.state().phase_kind(), SessionPhaseKind::Ready);
    assert_eq!(harness.ui.state().desired().unwrap(), &committed_key);
    assert_eq!(
        harness.ui.state().controller_mode(),
        Some(&ControllerMode::Controlling(controller))
    );
    assert_eq!(harness.mailbox.is_closed(), None);

    harness.finish()
}

#[test]
fn action_incompatible_create_result_retires_unknown_without_detach_or_replay() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(
        vec![
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Workbench,
        ],
        geometry,
    )?;
    let (old_key, _controller) = drive_controlling_ready(&mut harness, geometry)?;
    let operation = harness.ui.admit_create()?;
    let WorkbenchRequest::Command(command) = harness.carrier.take_workbench()? else {
        return Err(test_error("Create admission must submit tab.create"));
    };
    assert_eq!(command.request_id, *operation.request_id());
    harness
        .carrier
        .respond_workbench(WorkbenchResponse::Command(WorkbenchCommandResult {
            protocol_version: ProtocolVersion,
            request_id: command.request_id,
            outcome: WorkbenchCommandOutcome::Unchanged {
                revision: WorkbenchRevision::new(1),
            },
        }))?;

    assert!(harness.ui.take_next().is_err());
    assert_eq!(
        harness.mailbox.is_closed(),
        Some(CarrierFailure::OutcomeUnknown)
    );
    assert_eq!(harness.ui.state().phase_kind(), SessionPhaseKind::Creating);
    assert_eq!(harness.ui.state().desired().unwrap(), &old_key);
    assert_eq!(harness.ui.state().projection_key(), Some(&old_key));

    // The scripted Server rejects trailing bytes, so finish establishes that the executor neither
    // detached the old viewer nor replayed the ambiguous Create.
    harness.finish_after_worker_failure(CarrierFailure::OutcomeUnknown)
}

#[test]
fn lost_create_response_retires_unknown_without_detach_or_replay() -> TestResult {
    run_create_wire_failure(CreateWireFailure::Lost)
}

#[test]
fn malformed_create_response_retires_unknown_without_detach_or_replay() -> TestResult {
    run_create_wire_failure(CreateWireFailure::Malformed)
}

#[derive(Clone, Copy)]
enum CreateWireFailure {
    Lost,
    Malformed,
}

fn run_create_wire_failure(failure: CreateWireFailure) -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(
        vec![
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Workbench,
        ],
        geometry,
    )?;
    let (old_key, _controller) = drive_controlling_ready(&mut harness, geometry)?;
    let operation = harness.ui.admit_create()?;
    let WorkbenchRequest::Command(command) = harness.carrier.take_workbench()? else {
        return Err(test_error("Create admission must submit tab.create"));
    };
    assert_eq!(command.request_id, *operation.request_id());
    match failure {
        CreateWireFailure::Lost => harness.carrier.disconnect()?,
        CreateWireFailure::Malformed => harness.carrier.respond_malformed_workbench()?,
    }

    assert!(harness.ui.take_next().is_err());
    assert_eq!(
        harness.mailbox.is_closed(),
        Some(CarrierFailure::OutcomeUnknown)
    );
    assert_eq!(harness.ui.state().phase_kind(), SessionPhaseKind::Creating);
    assert_eq!(harness.ui.state().desired().unwrap(), &old_key);
    assert_eq!(harness.ui.state().projection_key(), Some(&old_key));
    assert!(harness.ui.projection_is_some());

    // The one-step carrier either closes after the accepted request or rejects trailing bytes;
    // worker completion therefore proves there was no Detach and no automatic Create replay.
    harness.finish_after_worker_failure(CarrierFailure::OutcomeUnknown)
}

#[test]
fn create_projection_missing_committed_tab_retires_authority_divergence() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(
        vec![
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Workbench,
        ],
        geometry,
    )?;
    let (_old_key, _controller) = drive_controlling_ready(&mut harness, geometry)?;
    let operation = harness.ui.admit_create()?;
    let committed = snapshot(2, 2);
    let created = &committed.terminal_contexts[0].tabs[1];
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = &created.root
    else {
        return Err(test_error("created fixture must have one Panel root"));
    };
    let WorkbenchRequest::Command(command) = harness.carrier.take_workbench()? else {
        return Err(test_error("Create admission must submit tab.create"));
    };
    harness
        .carrier
        .respond_workbench(WorkbenchResponse::Command(WorkbenchCommandResult {
            protocol_version: ProtocolVersion,
            request_id: command.request_id,
            outcome: WorkbenchCommandOutcome::TabCreatePending {
                revision: WorkbenchRevision::new(2),
                terminal_context_id: committed.terminal_contexts[0].terminal_context_id.clone(),
                tab_id: created.tab_id.clone(),
                panel_id: panel_id.clone(),
                presentation_id: primary_presentation.presentation_id.clone(),
            },
        }))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::CreateCommittedKnown { operation: ref actual } if actual == &operation
    ));

    let read = harness.carrier.take_read()?;
    // Revision 2 is plausible, but the exact committed tuple is missing.
    harness.carrier.respond_read(
        &read,
        WorkbenchSnapshotOutcome::Available {
            snapshot: snapshot(1, 2),
        },
    )?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::CreateProjection {
            operation: ref actual,
            decision: CreateProjectionDecision::Retire,
        } if actual == &operation
    ));
    assert_eq!(harness.ui.state().phase_kind(), SessionPhaseKind::Retired);
    assert!(harness.ui.take_next().is_err());
    assert_eq!(
        harness.mailbox.is_closed(),
        Some(CarrierFailure::AuthorityDivergence)
    );

    // Exact scripted exhaustion proves that divergent validation cannot Attach or replay Create.
    harness.finish_after_worker_failure(CarrierFailure::AuthorityDivergence)
}

#[test]
fn ambiguous_input_retires_unknown_without_replay() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start(
        vec![
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
        ],
        geometry,
    )?;
    let (active, controller) = drive_controlling_ready(&mut harness, geometry)?;
    let input = ControlInput::new(vec![b'?'])?;
    harness.ui.admit_input(input.clone())?;
    let ClientMessage::TerminalControl(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("accepted Input must reach the carrier"));
    };
    assert_control_input(&request, &controller, 2, &input)?;
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalControlResult(
            TerminalControlResult::new(
                request.request_id().clone(),
                TerminalControlOutcome::OutcomeUnknown {},
            ),
        ))?;

    assert!(harness.ui.take_next().is_err());
    assert_eq!(
        harness.mailbox.is_closed(),
        Some(CarrierFailure::OutcomeUnknown)
    );
    assert_eq!(harness.ui.state().phase_kind(), SessionPhaseKind::Ready);
    assert_eq!(harness.ui.state().desired().unwrap(), &active);
    assert_eq!(harness.ui.state().projection_key(), Some(&active));

    // A second Input would be an unscripted trailing request and make finish fail.
    harness.finish_after_worker_failure(CarrierFailure::OutcomeUnknown)
}

#[test]
fn definite_precommit_keeps_viewer_and_runs_one_catch_up_resize_before_poll() -> TestResult {
    let geometry = fixture_geometry()?;
    let latest = Geometry::new(
        geometry.columns(),
        geometry.rows(),
        geometry
            .cell_pixel_width()
            .checked_add(1)
            .ok_or_else(|| test_error("cell-pixel width overflow"))?,
        geometry
            .cell_pixel_height()
            .checked_add(2)
            .ok_or_else(|| test_error("cell-pixel height overflow"))?,
    )?;
    let mut harness = Harness::start(
        vec![
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
        ],
        geometry,
    )?;
    let (active, controller) = drive_controlling_ready(&mut harness, geometry)?;
    let operation = harness.ui.admit_create()?;
    let WorkbenchRequest::Command(command) = harness.carrier.take_workbench()? else {
        return Err(test_error("Create admission must submit tab.create"));
    };
    let WorkbenchAction::TabCreate {
        geometry: captured, ..
    } = command.action
    else {
        return Err(test_error("expected tab.create"));
    };
    assert_eq!(captured.columns(), geometry.columns());
    assert_eq!(captured.rows(), geometry.rows());
    assert_eq!(captured.cell_pixel_width(), geometry.cell_pixel_width());
    assert_eq!(captured.cell_pixel_height(), geometry.cell_pixel_height());

    // Geometry observation remains live in Creating, but no effectful Resize is admitted until
    // the definite precommit result restores the old Ready identity.
    harness.ui.observe_geometry(latest)?;
    harness
        .carrier
        .respond_workbench(WorkbenchResponse::Command(WorkbenchCommandResult {
            protocol_version: ProtocolVersion,
            request_id: command.request_id,
            outcome: WorkbenchCommandOutcome::Denied {},
        }))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::CreatePrecommit { operation: ref actual } if actual == &operation
    ));
    assert_eq!(harness.ui.state().phase_kind(), SessionPhaseKind::Ready);
    assert_eq!(harness.ui.state().desired().unwrap(), &active);
    assert_eq!(harness.ui.state().projection_key(), Some(&active));

    let ClientMessage::TerminalControl(resize) = harness.carrier.take_obcs()? else {
        return Err(test_error(
            "precommit restore must enqueue one catch-up Resize",
        ));
    };
    let TerminalControlAction::Resize {
        attachment_id,
        controller_id,
        sequence,
        geometry: requested,
    } = resize.action()
    else {
        return Err(test_error("expected catch-up Resize"));
    };
    assert_eq!(attachment_id, &attached_viewer_id()?);
    assert_eq!(controller_id, &controller);
    assert_eq!(sequence.get(), 2);
    assert_eq!(*requested, latest);
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalControlResult(
            TerminalControlResult::new(
                resize.request_id().clone(),
                TerminalControlOutcome::Resized { geometry: latest },
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::SetupConfigured {
            key: ref actual,
            mode: ControllerMode::Controlling(ref actual_controller),
            applied_geometry: Some(actual_geometry),
        } if actual == &active && actual_controller == &controller && actual_geometry == latest
    ));

    harness
        .mailbox
        .try_enqueue(QueuedCommand::Poll(Tagged::new(active.clone(), ())))
        .map_err(|error| test_error(&format!("failed to enqueue resumed Poll: {error:?}")))?;
    let ClientMessage::TerminalViewer(poll) = harness.carrier.take_obcs()? else {
        return Err(test_error(
            "catch-up Resize must be followed by resumed Poll",
        ));
    };
    assert_eq!(
        poll.action(),
        &TerminalViewerAction::Poll {
            attachment_id: attached_viewer_id()?,
        }
    );
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                poll.request_id().clone(),
                TerminalViewerOutcome::Record(full_frame_record(2, 2, latest)?),
            ),
        ))?;
    let feedback = match harness.ui.settle_next(FrameCut::None)? {
        UiEvent::FrameDrawn { key, feedback } if key == active => feedback,
        _ => return Err(test_error("resumed Poll must draw the new geometry")),
    };
    let ClientMessage::TerminalViewer(feedback_request) = harness.carrier.take_obcs()? else {
        return Err(test_error("resumed draw must send exact feedback"));
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
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::FeedbackAccepted { key: ref actual } if actual == &active
    ));
    assert!(harness.ui.state().feedback_accepted());

    harness.finish()
}

fn drive_started_create_to_projection_unavailable(
    harness: &mut Harness,
    geometry: Geometry,
) -> TestResult<(
    crate::terminal_session::state::CreateOperationKey,
    orchbee_workbench_protocol::Snapshot,
    crate::terminal_session::state::AttachmentKey,
)> {
    let (old_key, _controller) = drive_controlling_ready(harness, geometry)?;
    let operation = harness.ui.admit_create()?;
    let candidate = snapshot(2, 2);
    let WorkbenchRequest::Command(command) = harness.carrier.take_workbench()? else {
        return Err(test_error("Create admission must submit tab.create"));
    };
    assert_eq!(command.request_id.as_str(), identity(0x108));
    let WorkbenchAction::TabCreate {
        geometry: captured_geometry,
        ..
    } = &command.action
    else {
        return Err(test_error("expected tab.create"));
    };
    let created = &candidate.terminal_contexts[0].tabs[1];
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = &created.root
    else {
        return Err(test_error("created fixture must have one Panel root"));
    };
    let committed_key = crate::terminal_session::state::AttachmentKey::new(
        operation.reserved_generation(),
        created.tab_id.clone(),
    );
    harness
        .carrier
        .respond_workbench(WorkbenchResponse::Command(WorkbenchCommandResult {
            protocol_version: ProtocolVersion,
            request_id: command.request_id,
            outcome: WorkbenchCommandOutcome::TabCreated {
                revision: WorkbenchRevision::new(2),
                terminal_context_id: candidate.terminal_contexts[0].terminal_context_id.clone(),
                tab_id: created.tab_id.clone(),
                panel_id: panel_id.clone(),
                presentation_id: primary_presentation.presentation_id.clone(),
                terminal_outcome: WorkbenchTerminalOutcome::Started {
                    geometry: *captured_geometry,
                },
            },
        }))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::CreateCommittedKnown { operation: ref actual } if actual == &operation
    ));

    let read = harness.carrier.take_read()?;
    assert_eq!(read.request_id().as_str(), identity(0x109));
    harness
        .carrier
        .respond_read(&read, WorkbenchSnapshotOutcome::Denied {})?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::CreateProjection {
            operation: ref actual,
            decision: CreateProjectionDecision::ProjectionUnavailable,
        } if actual == &operation
    ));
    assert_eq!(
        harness.ui.state().phase_kind(),
        SessionPhaseKind::ProjectionUnavailable
    );
    assert_eq!(
        harness.ui.state().cache().revision(),
        WorkbenchRevision::new(1)
    );
    assert_eq!(
        harness.ui.state().active().map(|selected| selected.key()),
        Some(&old_key)
    );
    assert_eq!(harness.ui.state().desired().unwrap(), &old_key);
    assert_eq!(
        harness
            .ui
            .state()
            .committed_target()
            .map(|committed| committed.provenance()),
        Some(&CreateTerminalProvenance::Started)
    );
    assert_eq!(harness.ui.state().projection_key(), Some(&old_key));
    assert!(harness.ui.projection_is_some());
    assert!(harness.ui.state().ready().is_none());
    Ok((operation, candidate, committed_key))
}

fn drive_controlling_ready(
    harness: &mut Harness,
    geometry: Geometry,
) -> TestResult<(
    crate::terminal_session::state::AttachmentKey,
    orchbee_client_server_protocol::ControllerId,
)> {
    let key = drive_open_snapshot_bootstrap(harness, 1)?;
    assert_eq!(drive_attach(harness, 0)?, key);

    let controller = drive_controlling_after_attach(harness, &key, geometry)?;
    Ok((key, controller))
}

fn set_tab_labels(
    snapshot: &mut orchbee_workbench_protocol::Snapshot,
    index: usize,
    tab_label: &str,
    presentation_label: &str,
) -> TestResult {
    let tab = snapshot
        .terminal_contexts
        .get_mut(0)
        .and_then(|context| context.tabs.get_mut(index))
        .ok_or_else(|| test_error("missing tab fixture"))?;
    tab.label = tab_label.to_owned();
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &mut tab.root
    else {
        return Err(test_error("tab fixture must have one Panel root"));
    };
    primary_presentation.label = presentation_label.to_owned();
    Ok(())
}

pub(super) fn drive_controlling_ready_from_snapshot(
    harness: &mut Harness,
    initial: orchbee_workbench_protocol::Snapshot,
    geometry: Geometry,
) -> TestResult<(
    crate::terminal_session::state::AttachmentKey,
    orchbee_client_server_protocol::ControllerId,
)> {
    let ClientMessage::TerminalOpen(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("startup must begin with terminal.open"));
    };
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalOpenResult(
            orchbee_client_server_protocol::TerminalOpenResult::new(
                request.request_id().clone(),
                orchbee_client_server_protocol::OpenOutcome::Opened {
                    terminal_create_request_id: obcs_request_id(900)?,
                },
            ),
        ))?;
    let request = harness.carrier.take_read()?;
    harness.carrier.respond_read(
        &request,
        WorkbenchSnapshotOutcome::Available { snapshot: initial },
    )?;
    let key = match harness.ui.settle_next(FrameCut::None)? {
        UiEvent::Bootstrap { key } => key,
        _ => return Err(test_error("custom snapshot must publish Bootstrap")),
    };
    assert_eq!(drive_attach(harness, 0)?, key);
    let controller = drive_controlling_after_attach(harness, &key, geometry)?;
    Ok((key, controller))
}

pub(super) fn drive_controlling_after_attach(
    harness: &mut Harness,
    key: &crate::terminal_session::state::AttachmentKey,
    geometry: Geometry,
) -> TestResult<orchbee_client_server_protocol::ControllerId> {
    let ClientMessage::TerminalControl(acquire) = harness.carrier.take_obcs()? else {
        return Err(test_error("Attach must be followed by Acquire"));
    };
    assert_eq!(
        acquire.action(),
        &TerminalControlAction::Acquire {
            attachment_id: attached_viewer_id()?,
        }
    );
    let controller = controller_id()?;
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalControlResult(
            TerminalControlResult::new(
                acquire.request_id().clone(),
                TerminalControlOutcome::Acquired {
                    controller_id: controller.clone(),
                },
            ),
        ))?;

    let ClientMessage::TerminalControl(resize) = harness.carrier.take_obcs()? else {
        return Err(test_error("controlling setup must Resize"));
    };
    let TerminalControlAction::Resize {
        attachment_id,
        controller_id,
        sequence,
        geometry: requested_geometry,
    } = resize.action()
    else {
        return Err(test_error("controlling setup must Resize"));
    };
    assert_eq!(attachment_id, &attached_viewer_id()?);
    assert_eq!(controller_id, &controller);
    assert_eq!(sequence.get(), 1);
    assert_eq!(*requested_geometry, geometry);
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
            key: ref actual_key,
            mode: ControllerMode::Controlling(ref actual),
            applied_geometry: Some(actual_geometry),
        } if actual_key == key && actual == &controller && actual_geometry == geometry
    ));

    let ClientMessage::TerminalViewer(poll) = harness.carrier.take_obcs()? else {
        return Err(test_error("setup must Poll"));
    };
    assert_eq!(
        poll.action(),
        &TerminalViewerAction::Poll {
            attachment_id: attached_viewer_id()?,
        }
    );
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                poll.request_id().clone(),
                TerminalViewerOutcome::Record(full_frame_record(1, 1, geometry)?),
            ),
        ))?;
    let feedback = match harness.ui.settle_next(FrameCut::None)? {
        UiEvent::FrameDrawn {
            key: actual,
            feedback,
        } if actual == *key => feedback,
        _ => return Err(test_error("setup full frame must be drawn")),
    };
    let ClientMessage::TerminalViewer(feedback_request) = harness.carrier.take_obcs()? else {
        return Err(test_error("setup draw must send feedback"));
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
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::FeedbackAccepted { key: ref actual } if actual == key
    ));
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Ready(crate::terminal_session::state::ReadyDecision::Ready)
    ));
    Ok(controller)
}

fn assert_control_input(
    request: &orchbee_client_server_protocol::TerminalControlRequest,
    controller: &orchbee_client_server_protocol::ControllerId,
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
    assert_eq!(attachment_id, &attached_viewer_id()?);
    assert_eq!(controller_id, controller);
    assert_eq!(sequence.get(), expected_sequence);
    assert_eq!(bytes, expected_input);
    Ok(())
}

fn identity(value: u128) -> String {
    format!("{value:032x}")
}
