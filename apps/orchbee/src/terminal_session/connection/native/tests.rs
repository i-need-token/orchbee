#[cfg(windows)]
mod agent_startup;
mod commands;
mod context_selection;
mod location_labels;
mod panel;
mod presentations;
mod setup;
#[cfg(windows)]
mod startup_directory;
#[cfg(windows)]
mod startup_preference;
mod switch_membership;
mod tab_move;
mod workspace_setup;

use self::setup::*;
use super::*;
use crate::terminal_session::{
    mailbox::{MutationResult, QueuedCommand, Tagged, TakenEvent},
    state::{CarrierFailure, CleanupContinuation, ControllerMode, ReadyDecision, SessionPhaseKind},
};
use orchbee_client_server_protocol::{
    ClientMessage, ControlRejectedReason, OpenOutcome, ServerMessage, TerminalControlAction,
    TerminalControlOutcome, TerminalControlResult, TerminalOpenResult, TerminalViewerAction,
    TerminalViewerOutcome, TerminalViewerResult, ViewerRejectedReason, ViewerSnapshotReason,
    ViewerUnavailableReason,
};

use std::{
    sync::atomic::Ordering,
    thread,
    time::{Duration, Instant},
};

#[test]
fn startup_controlling_draws_before_exact_feedback_and_clears_its_ceiling() -> TestResult {
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
    let mut request_ids = Vec::new();

    let ClientMessage::TerminalOpen(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("startup must begin with terminal.open"));
    };
    assert_eq!(request.geometry(), geometry);
    request_ids.push(request.request_id().as_str().to_owned());
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalOpenResult(TerminalOpenResult::new(
            request.request_id().clone(),
            OpenOutcome::Opened {
                terminal_create_request_id: obcs_request_id(900)?,
            },
        )))?;

    let request = harness.carrier.take_read()?;
    request_ids.push(request.request_id().as_str().to_owned());
    harness.carrier.respond_read(
        &request,
        WorkbenchSnapshotOutcome::Available {
            snapshot: setup::snapshot(2, 1),
        },
    )?;

    let UiEvent::Bootstrap { key } = harness.ui.settle_next(FrameCut::None)? else {
        return Err(test_error("snapshot must publish Bootstrap"));
    };
    assert_eq!(key.generation(), 1);
    assert_eq!(key.tab_id(), &workbench_tab_id(1_000)?);

    let ClientMessage::TerminalViewer(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("Bootstrap reply must license AttachBinding"));
    };
    assert_eq!(request.action(), &attach_action(0)?);
    request_ids.push(request.request_id().as_str().to_owned());
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(request.request_id().clone(), attached_outcome()?),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Attached { key: ref attached } if attached == &key
    ));

    let ClientMessage::TerminalControl(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("Attach must be followed by Acquire"));
    };
    assert_eq!(
        request.action(),
        &TerminalControlAction::Acquire {
            attachment_id: attached_viewer_id()?,
        }
    );
    request_ids.push(request.request_id().as_str().to_owned());
    let controller = controller_id()?;
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalControlResult(
            TerminalControlResult::new(
                request.request_id().clone(),
                TerminalControlOutcome::Acquired {
                    controller_id: controller.clone(),
                },
            ),
        ))?;

    let ClientMessage::TerminalControl(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("controlling setup must Resize"));
    };
    let TerminalControlAction::Resize {
        attachment_id,
        controller_id,
        sequence,
        geometry: requested_geometry,
    } = request.action()
    else {
        return Err(test_error("controlling setup must send Resize"));
    };
    assert_eq!(attachment_id, &attached_viewer_id()?);
    assert_eq!(controller_id, &controller);
    assert_eq!(sequence.get(), 1);
    assert_eq!(requested_geometry.columns(), geometry.columns());
    assert_eq!(requested_geometry.rows(), geometry.rows());
    assert_eq!(
        requested_geometry.cell_pixel_width(),
        geometry.cell_pixel_width()
    );
    assert_eq!(
        requested_geometry.cell_pixel_height(),
        geometry.cell_pixel_height()
    );
    request_ids.push(request.request_id().as_str().to_owned());
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalControlResult(
            TerminalControlResult::new(
                request.request_id().clone(),
                TerminalControlOutcome::Resized { geometry },
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::SetupConfigured {
            mode: ControllerMode::Controlling(ref actual),
            applied_geometry: Some(actual_geometry),
            ..
        } if actual == &controller && actual_geometry == geometry
    ));

    let ClientMessage::TerminalViewer(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("setup must Poll for a full frame"));
    };
    assert_eq!(
        request.action(),
        &TerminalViewerAction::Poll {
            attachment_id: attached_viewer_id()?,
        }
    );
    request_ids.push(request.request_id().as_str().to_owned());
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                request.request_id().clone(),
                TerminalViewerOutcome::Record(full_frame_record(1, 1, geometry)?),
            ),
        ))?;
    let feedback = match harness.ui.settle_next(FrameCut::None)? {
        UiEvent::FrameDrawn {
            key: actual,
            feedback,
        } if actual == key => feedback,
        _ => {
            return Err(test_error(
                "full frame must cross the actual draw checkpoint",
            ));
        }
    };
    assert!(matches!(
        feedback,
        TerminalViewerAction::FrameApplied { .. }
    ));
    assert_eq!(harness.ui.drawn_frames(), 1);

    let ClientMessage::TerminalViewer(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("drawn frame must send feedback"));
    };
    assert_eq!(request.action(), &feedback);
    request_ids.push(request.request_id().as_str().to_owned());
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                request.request_id().clone(),
                TerminalViewerOutcome::FeedbackAccepted {},
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::FeedbackAccepted { key: ref actual } if actual == &key
    ));
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Ready(ReadyDecision::Ready)
    ));
    assert_eq!(harness.ui.state().phase_kind(), SessionPhaseKind::Ready);
    assert!(harness.ui.state().feedback_accepted());

    assert_strictly_sequential_ids(&request_ids, IDENTITY_SEED)?;

    // If the startup ceiling were still installed, this post-Ready Poll would expire immediately.
    harness.elapsed_millis.store(51_000, Ordering::SeqCst);
    harness
        .mailbox
        .try_enqueue(QueuedCommand::Poll(Tagged::new(
            harness.ui.pane_ref(&key)?,
            (),
        )))
        .map_err(|error| test_error(&format!("failed to enqueue post-Ready Poll: {error:?}")))?;
    let ClientMessage::TerminalViewer(request) = harness.carrier.take_obcs()? else {
        return Err(test_error(
            "Ready traffic must outlive the cleared startup ceiling",
        ));
    };
    assert_eq!(
        request.action(),
        &TerminalViewerAction::Poll {
            attachment_id: attached_viewer_id()?,
        }
    );
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                request.request_id().clone(),
                TerminalViewerOutcome::Empty {},
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::EmptyPoll { key: ref actual } if actual == &key
    ));
    assert_eq!(harness.ui.drawn_frames(), 1);
    assert_eq!(harness.ui.no_frame_redraws(), 0);

    harness.finish()
}

#[test]
fn read_only_setup_sends_no_resize_and_empty_poll_requires_no_draw() -> TestResult {
    let geometry = fixture_geometry()?;
    let changed = Geometry::new(
        geometry.columns(),
        geometry.rows(),
        geometry
            .cell_pixel_width()
            .checked_add(1)
            .ok_or("pixel width")?,
        geometry
            .cell_pixel_height()
            .checked_add(2)
            .ok_or("pixel height")?,
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
        ],
        geometry,
    )?;
    drive_open_snapshot_bootstrap(&mut harness, 1)?;
    drive_attach(&mut harness, 0)?;

    let ClientMessage::TerminalControl(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("Attach must be followed by Acquire"));
    };
    assert_eq!(
        request.action(),
        &TerminalControlAction::Acquire {
            attachment_id: attached_viewer_id()?,
        }
    );
    harness.ui.observe_geometry(changed)?;
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalControlResult(
            TerminalControlResult::new(
                request.request_id().clone(),
                TerminalControlOutcome::Rejected {
                    reason: ControlRejectedReason::ControllerBusy,
                },
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::SetupConfigured {
            mode: ControllerMode::ReadOnly,
            applied_geometry: None,
            ..
        }
    ));

    // The next scripted OBCS step is Poll. Any Resize would fail this exact action assertion.
    let ClientMessage::TerminalViewer(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("ReadOnly setup must not send Resize"));
    };
    assert_eq!(
        request.action(),
        &TerminalViewerAction::Poll {
            attachment_id: attached_viewer_id()?,
        }
    );
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                request.request_id().clone(),
                TerminalViewerOutcome::Empty {},
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::EmptyPoll { .. }
    ));
    assert_eq!(harness.ui.drawn_frames(), 0);
    assert_eq!(harness.ui.no_frame_redraws(), 0);

    let ClientMessage::TerminalViewer(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("Empty setup Poll must repeat"));
    };
    assert_eq!(
        request.action(),
        &TerminalViewerAction::Poll {
            attachment_id: attached_viewer_id()?,
        }
    );
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                request.request_id().clone(),
                TerminalViewerOutcome::Record(full_frame_record(1, 1, geometry)?),
            ),
        ))?;
    let feedback = match harness.ui.settle_next(FrameCut::None)? {
        UiEvent::FrameDrawn { feedback, .. } => feedback,
        _ => return Err(test_error("ReadOnly full frame must be drawn")),
    };
    let ClientMessage::TerminalViewer(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("drawn ReadOnly frame must send feedback"));
    };
    assert_eq!(request.action(), &feedback);
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                request.request_id().clone(),
                TerminalViewerOutcome::FeedbackAccepted {},
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::FeedbackAccepted { .. }
    ));
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Ready(ReadyDecision::Ready)
    ));
    assert_eq!(harness.ui.state().latest_geometry(), changed);
    assert!(matches!(
        harness.ui.state().controller_mode(),
        Some(ControllerMode::ReadOnly)
    ));

    harness.finish()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GeometryCatchUpCut {
    AcquireHandoff,
    ResizeHandoff,
    PollHandoff,
    BeforeDraw,
    AfterDraw,
    FeedbackHandoff,
}

#[test]
fn controlling_switch_catches_up_geometry_at_all_six_blocking_cuts() -> TestResult {
    for cut in [
        GeometryCatchUpCut::AcquireHandoff,
        GeometryCatchUpCut::ResizeHandoff,
        GeometryCatchUpCut::PollHandoff,
        GeometryCatchUpCut::BeforeDraw,
        GeometryCatchUpCut::AfterDraw,
        GeometryCatchUpCut::FeedbackHandoff,
    ] {
        run_geometry_catch_up_cut(cut)?;
    }
    Ok(())
}

fn run_geometry_catch_up_cut(cut: GeometryCatchUpCut) -> TestResult {
    let initial = fixture_geometry()?;
    let changed = Geometry::new(
        initial.columns(),
        initial.rows(),
        initial
            .cell_pixel_width()
            .checked_add(1)
            .ok_or("pixel width")?,
        initial
            .cell_pixel_height()
            .checked_add(2)
            .ok_or("pixel height")?,
    )?;
    let needs_catch_up = cut != GeometryCatchUpCut::AcquireHandoff;
    let mut steps = vec![
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
        WireKind::Obcs,
    ];
    if needs_catch_up {
        steps.extend([WireKind::Obcs, WireKind::Obcs, WireKind::Obcs]);
    }
    steps.insert(7, WireKind::Workbench);
    let mut harness = Harness::start(steps, initial)?;
    let old_key = drive_controlling_startup(&mut harness, initial, 2)?;
    let switch = harness.ui.admit_switch(1)?;
    assert_eq!(switch.key.generation(), old_key.generation() + 1);
    assert_eq!(harness.ui.no_frame_redraws(), 1);
    assert!(matches!(
        drive_switch_snapshot(&mut harness)?,
        UiEvent::SwitchSnapshot {
            decision: RefreshDecision::Cleanup(_),
            ..
        }
    ));

    let ClientMessage::TerminalViewer(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("staged Switch must Detach the old viewer"));
    };
    assert_eq!(
        request.action(),
        &TerminalViewerAction::Detach {
            attachment_id: attached_viewer_id()?,
        }
    );
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                request.request_id().clone(),
                TerminalViewerOutcome::Detached {},
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::SwitchOldDetached {
            work,
            detached,
            continuation: SwitchDetachContinuation::Attach(_),
        } if work == switch.key && detached == old_key
    ));

    let installed = harness.ui.state().desired().unwrap().clone();
    assert_eq!(installed.generation(), switch.key.generation() + 1);
    let ClientMessage::TerminalViewer(request) = harness.carrier.take_obcs()? else {
        return Err(test_error(
            "old Detach must license the copied target AttachBinding",
        ));
    };
    assert_eq!(request.action(), &attach_action(1)?);
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(request.request_id().clone(), attached_outcome()?),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Attached { key } if key == installed
    ));

    let ClientMessage::TerminalControl(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("fresh Switch attachment must Acquire"));
    };
    assert_eq!(
        request.action(),
        &TerminalControlAction::Acquire {
            attachment_id: attached_viewer_id()?,
        }
    );
    if cut == GeometryCatchUpCut::AcquireHandoff {
        harness.ui.observe_geometry(changed)?;
    }
    let controller = controller_id()?;
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalControlResult(
            TerminalControlResult::new(
                request.request_id().clone(),
                TerminalControlOutcome::Acquired {
                    controller_id: controller.clone(),
                },
            ),
        ))?;

    let ClientMessage::TerminalControl(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("controlling Switch setup must Resize"));
    };
    let TerminalControlAction::Resize {
        attachment_id,
        controller_id,
        sequence,
        geometry: first_geometry,
    } = request.action()
    else {
        return Err(test_error(
            "controlling Switch setup sent a non-Resize action",
        ));
    };
    assert_eq!(attachment_id, &attached_viewer_id()?);
    assert_eq!(controller_id, &controller);
    assert_eq!(sequence.get(), 1);
    assert_eq!(
        *first_geometry,
        if cut == GeometryCatchUpCut::AcquireHandoff {
            changed
        } else {
            initial
        },
        "geometry cut {cut:?}"
    );
    if cut == GeometryCatchUpCut::ResizeHandoff {
        harness.ui.observe_geometry(changed)?;
    }
    let first_geometry = *first_geometry;
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalControlResult(
            TerminalControlResult::new(
                request.request_id().clone(),
                TerminalControlOutcome::Resized {
                    geometry: first_geometry,
                },
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::SetupConfigured {
            key,
            mode: ControllerMode::Controlling(actual),
            applied_geometry: Some(geometry),
        } if key == installed && actual == controller && geometry == first_geometry
    ));

    let ClientMessage::TerminalViewer(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("configured Switch setup must Poll"));
    };
    assert_eq!(
        request.action(),
        &TerminalViewerAction::Poll {
            attachment_id: attached_viewer_id()?,
        }
    );
    if cut == GeometryCatchUpCut::PollHandoff {
        harness.ui.observe_geometry(changed)?;
    }
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                request.request_id().clone(),
                TerminalViewerOutcome::Record(full_frame_record(1, 1, first_geometry)?),
            ),
        ))?;
    let frame_cut = match cut {
        GeometryCatchUpCut::BeforeDraw => FrameCut::BeforeDraw(changed),
        GeometryCatchUpCut::AfterDraw => FrameCut::AfterDraw(changed),
        GeometryCatchUpCut::AcquireHandoff
        | GeometryCatchUpCut::ResizeHandoff
        | GeometryCatchUpCut::PollHandoff
        | GeometryCatchUpCut::FeedbackHandoff => FrameCut::None,
    };
    let first_feedback = match harness.ui.settle_next(frame_cut)? {
        UiEvent::FrameDrawn { key, feedback } if key == installed => feedback,
        _ => {
            return Err(test_error(
                "Switch full frame must cross the draw checkpoint",
            ));
        }
    };

    let ClientMessage::TerminalViewer(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("drawn Switch frame must send exact feedback"));
    };
    assert_eq!(request.action(), &first_feedback);
    if cut == GeometryCatchUpCut::FeedbackHandoff {
        harness.ui.observe_geometry(changed)?;
    }
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                request.request_id().clone(),
                TerminalViewerOutcome::FeedbackAccepted {},
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::FeedbackAccepted { key } if key == installed
    ));

    if !needs_catch_up {
        assert!(matches!(
            harness.ui.settle_next(FrameCut::None)?,
            UiEvent::Ready(ReadyDecision::Ready)
        ));
        assert_eq!(harness.ui.state().phase_kind(), SessionPhaseKind::Ready);
        assert_eq!(harness.ui.state().latest_geometry(), changed);
        return harness.finish();
    }

    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Ready(ReadyDecision::CatchUpResize(geometry)) if geometry == changed
    ));
    assert_ne!(harness.ui.state().phase_kind(), SessionPhaseKind::Ready);

    let ClientMessage::TerminalControl(request) = harness.carrier.take_obcs()? else {
        return Err(test_error(
            "geometry drift must send one setup catch-up Resize",
        ));
    };
    let TerminalControlAction::Resize {
        attachment_id,
        controller_id,
        sequence,
        geometry,
    } = request.action()
    else {
        return Err(test_error("catch-up action must be Resize"));
    };
    assert_eq!(attachment_id, &attached_viewer_id()?);
    assert_eq!(controller_id, &controller);
    assert_eq!(sequence.get(), 2);
    assert_eq!(*geometry, changed);
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalControlResult(
            TerminalControlResult::new(
                request.request_id().clone(),
                TerminalControlOutcome::Resized { geometry: changed },
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::SetupConfigured {
            key,
            mode: ControllerMode::Controlling(actual),
            applied_geometry: Some(geometry),
        } if key == installed && actual == controller && geometry == changed
    ));
    assert!(!harness.ui.state().feedback_accepted());
    assert_eq!(harness.ui.no_frame_redraws(), 2);

    let ClientMessage::TerminalViewer(request) = harness.carrier.take_obcs()? else {
        return Err(test_error(
            "catch-up setup must restart the full-frame gate",
        ));
    };
    assert_eq!(
        request.action(),
        &TerminalViewerAction::Poll {
            attachment_id: attached_viewer_id()?,
        }
    );
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                request.request_id().clone(),
                TerminalViewerOutcome::Record(full_frame_record(2, 2, changed)?),
            ),
        ))?;
    let second_feedback = match harness.ui.settle_next(FrameCut::None)? {
        UiEvent::FrameDrawn { key, feedback } if key == installed => feedback,
        _ => return Err(test_error("catch-up full frame must be drawn")),
    };
    let ClientMessage::TerminalViewer(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("catch-up draw must send exact FrameApplied"));
    };
    assert_eq!(request.action(), &second_feedback);
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                request.request_id().clone(),
                TerminalViewerOutcome::FeedbackAccepted {},
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::FeedbackAccepted { key } if key == installed
    ));
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Ready(ReadyDecision::Ready)
    ));
    assert_eq!(harness.ui.state().phase_kind(), SessionPhaseKind::Ready);
    assert_eq!(harness.ui.drawn_frames(), 3);
    harness.finish()
}

fn drive_controlling_startup(
    harness: &mut Harness,
    geometry: Geometry,
    tab_count: usize,
) -> TestResult<crate::terminal_session::state::AttachmentKey> {
    let key = drive_open_snapshot_bootstrap(harness, tab_count)?;
    assert_eq!(drive_attach(harness, 0)?, key);

    let ClientMessage::TerminalControl(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("startup attachment must Acquire"));
    };
    assert_eq!(
        request.action(),
        &TerminalControlAction::Acquire {
            attachment_id: attached_viewer_id()?,
        }
    );
    let controller = controller_id()?;
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalControlResult(
            TerminalControlResult::new(
                request.request_id().clone(),
                TerminalControlOutcome::Acquired {
                    controller_id: controller.clone(),
                },
            ),
        ))?;

    let ClientMessage::TerminalControl(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("controlling startup must Resize"));
    };
    assert!(matches!(
        request.action(),
        TerminalControlAction::Resize {
            attachment_id,
            controller_id,
            sequence,
            geometry: actual,
        } if attachment_id == &attached_viewer_id()?
            && controller_id == &controller
            && sequence.get() == 1
            && actual == &geometry
    ));
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalControlResult(
            TerminalControlResult::new(
                request.request_id().clone(),
                TerminalControlOutcome::Resized { geometry },
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::SetupConfigured { key: actual, .. } if actual == key
    ));

    let ClientMessage::TerminalViewer(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("startup setup must Poll"));
    };
    assert_eq!(
        request.action(),
        &TerminalViewerAction::Poll {
            attachment_id: attached_viewer_id()?,
        }
    );
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                request.request_id().clone(),
                TerminalViewerOutcome::Record(full_frame_record(1, 1, geometry)?),
            ),
        ))?;
    let feedback = match harness.ui.settle_next(FrameCut::None)? {
        UiEvent::FrameDrawn {
            key: actual,
            feedback,
        } if actual == key => feedback,
        _ => return Err(test_error("startup frame must be drawn")),
    };
    let ClientMessage::TerminalViewer(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("startup draw must send feedback"));
    };
    assert_eq!(request.action(), &feedback);
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                request.request_id().clone(),
                TerminalViewerOutcome::FeedbackAccepted {},
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::FeedbackAccepted { key: actual } if actual == key
    ));
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Ready(ReadyDecision::Ready)
    ));
    Ok(key)
}

#[test]
fn no_frame_results_and_old_feedback_cannot_license_ready_after_catch_up() -> TestResult {
    let initial = fixture_geometry()?;
    let changed = Geometry::new(
        initial.columns(),
        initial.rows(),
        initial
            .cell_pixel_width()
            .checked_add(1)
            .ok_or("pixel width")?,
        initial
            .cell_pixel_height()
            .checked_add(1)
            .ok_or("pixel height")?,
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
            WireKind::Obcs,
            WireKind::Obcs,
        ],
        initial,
    )?;
    let key = drive_open_snapshot_bootstrap(&mut harness, 1)?;
    assert_eq!(drive_attach(&mut harness, 0)?, key);

    let ClientMessage::TerminalControl(acquire) = harness.carrier.take_obcs()? else {
        return Err(test_error("startup attachment must Acquire"));
    };
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
        return Err(test_error("controlling startup must Resize"));
    };
    assert!(matches!(
        resize.action(),
        TerminalControlAction::Resize {
            attachment_id,
            geometry,
            ..
        } if attachment_id == &attached_viewer_id()? && geometry == &initial
    ));
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalControlResult(
            TerminalControlResult::new(
                resize.request_id().clone(),
                TerminalControlOutcome::Resized { geometry: initial },
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::SetupConfigured { applied_geometry: Some(geometry), .. } if geometry == initial
    ));

    let ClientMessage::TerminalViewer(poll) = harness.carrier.take_obcs()? else {
        return Err(test_error("configured startup must Poll"));
    };
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                poll.request_id().clone(),
                TerminalViewerOutcome::Record(full_frame_record(1, 1, initial)?),
            ),
        ))?;
    let old_feedback = match harness.ui.settle_next(FrameCut::None)? {
        UiEvent::FrameDrawn {
            key: actual,
            feedback,
        } if actual == key => feedback,
        _ => return Err(test_error("first full frame must be drawn")),
    };
    let ClientMessage::TerminalViewer(feedback) = harness.carrier.take_obcs()? else {
        return Err(test_error("first draw must send FrameApplied"));
    };
    assert_eq!(feedback.action(), &old_feedback);
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                feedback.request_id().clone(),
                TerminalViewerOutcome::FeedbackAccepted {},
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::FeedbackAccepted { key: actual } if actual == key
    ));

    // Geometry changes after accepted old feedback but before the Ready settlement.
    harness.ui.observe_geometry(changed)?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Ready(ReadyDecision::CatchUpResize(geometry)) if geometry == changed
    ));
    assert_ne!(harness.ui.state().phase_kind(), SessionPhaseKind::Ready);

    let ClientMessage::TerminalControl(resize) = harness.carrier.take_obcs()? else {
        return Err(test_error(
            "old feedback must be followed by catch-up Resize",
        ));
    };
    assert!(matches!(
        resize.action(),
        TerminalControlAction::Resize {
            attachment_id,
            controller_id,
            sequence,
            geometry,
        } if attachment_id == &attached_viewer_id()?
            && controller_id == &controller
            && sequence.get() == 2
            && geometry == &changed
    ));
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalControlResult(
            TerminalControlResult::new(
                resize.request_id().clone(),
                TerminalControlOutcome::Resized { geometry: changed },
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::SetupConfigured { applied_geometry: Some(geometry), .. } if geometry == changed
    ));
    assert!(!harness.ui.state().feedback_accepted());
    assert_eq!(harness.ui.drawn_frames(), 1);
    assert_eq!(harness.ui.no_frame_redraws(), 1);

    // A well-formed full frame for the old dimensions produces NeedSnapshot only after
    // the no-frame redraw and cannot restore Ready evidence.
    let ClientMessage::TerminalViewer(poll) = harness.carrier.take_obcs()? else {
        return Err(test_error("catch-up setup must Poll"));
    };
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                poll.request_id().clone(),
                TerminalViewerOutcome::Record(full_frame_record(2, 2, initial)?),
            ),
        ))?;
    let need_snapshot = match harness.ui.settle_next(FrameCut::None)? {
        UiEvent::NoFrameRedraw {
            key: actual,
            feedback: Some(feedback),
        } if actual == key => feedback,
        _ => return Err(test_error("dimension mismatch must redraw without a frame")),
    };
    assert!(matches!(
        need_snapshot,
        TerminalViewerAction::NeedSnapshot {
            ref attachment_id,
            sequence,
            reason: ViewerSnapshotReason::DimensionsMismatch,
        } if attachment_id == &attached_viewer_id()? && sequence.get() == 2
    ));
    assert_eq!(harness.ui.drawn_frames(), 1);
    assert_eq!(harness.ui.no_frame_redraws(), 2);
    let ClientMessage::TerminalViewer(feedback) = harness.carrier.take_obcs()? else {
        return Err(test_error("NeedSnapshot must be sent after the redraw"));
    };
    assert_eq!(feedback.action(), &need_snapshot);
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                feedback.request_id().clone(),
                TerminalViewerOutcome::FeedbackAccepted {},
            ),
        ))?;

    // ProjectionBlocked carries no feedback and therefore the very next request is Poll.
    let ClientMessage::TerminalViewer(poll) = harness.carrier.take_obcs()? else {
        return Err(test_error("accepted NeedSnapshot must resume Poll"));
    };
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                poll.request_id().clone(),
                TerminalViewerOutcome::Record(blocked_record(3)?),
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::NoFrameRedraw {
            key: actual,
            feedback: None,
        } if actual == key
    ));
    assert_eq!(harness.ui.drawn_frames(), 1);
    assert_eq!(harness.ui.no_frame_redraws(), 3);

    let ClientMessage::TerminalViewer(poll) = harness.carrier.take_obcs()? else {
        return Err(test_error(
            "ProjectionBlocked must resume Poll without feedback",
        ));
    };
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(poll.request_id().clone(), TerminalViewerOutcome::Empty {}),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::EmptyPoll { key: actual } if actual == key
    ));
    assert_eq!(harness.ui.drawn_frames(), 1);
    assert_eq!(harness.ui.no_frame_redraws(), 3);

    let ClientMessage::TerminalViewer(poll) = harness.carrier.take_obcs()? else {
        return Err(test_error("Empty must resume Poll"));
    };
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                poll.request_id().clone(),
                TerminalViewerOutcome::Record(full_frame_record(3, 4, changed)?),
            ),
        ))?;
    let fresh_feedback = match harness.ui.settle_next(FrameCut::None)? {
        UiEvent::FrameDrawn {
            key: actual,
            feedback,
        } if actual == key => feedback,
        _ => return Err(test_error("later full frame must be drawn")),
    };
    let ClientMessage::TerminalViewer(feedback) = harness.carrier.take_obcs()? else {
        return Err(test_error("later draw must send FrameApplied"));
    };
    assert_eq!(feedback.action(), &fresh_feedback);
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                feedback.request_id().clone(),
                TerminalViewerOutcome::FeedbackAccepted {},
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::FeedbackAccepted { key: actual } if actual == key
    ));
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Ready(ReadyDecision::Ready)
    ));
    assert_eq!(harness.ui.drawn_frames(), 2);
    assert_eq!(harness.ui.state().phase_kind(), SessionPhaseKind::Ready);
    harness.finish()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SwitchSupersessionCut {
    BeforeHandoff,
    AfterPollHandoff,
    AfterFrameDelivery,
    AfterDraw,
    AfterFeedbackHandoff,
}

#[test]
fn rapid_switch_supersession_detaches_exact_viewer_at_every_blocking_cut() -> TestResult {
    for cut in [
        SwitchSupersessionCut::BeforeHandoff,
        SwitchSupersessionCut::AfterPollHandoff,
        SwitchSupersessionCut::AfterFrameDelivery,
        SwitchSupersessionCut::AfterDraw,
        SwitchSupersessionCut::AfterFeedbackHandoff,
    ] {
        run_switch_supersession_cut(cut)?;
    }
    Ok(())
}

fn run_switch_supersession_cut(cut: SwitchSupersessionCut) -> TestResult {
    let geometry = fixture_geometry()?;
    let mut steps = vec![
        WireKind::Obcs,
        WireKind::Workbench,
        WireKind::Obcs,
        WireKind::Obcs,
        WireKind::Obcs,
        WireKind::Obcs,
        WireKind::Obcs,
        WireKind::Obcs,
    ];
    if cut != SwitchSupersessionCut::BeforeHandoff {
        steps.extend([
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
        ]);
        if cut == SwitchSupersessionCut::AfterFeedbackHandoff {
            steps.push(WireKind::Obcs);
        }
    }
    steps.push(WireKind::Workbench);
    steps.push(WireKind::Obcs);
    steps.insert(7, WireKind::Workbench);
    let mut harness = Harness::start(steps, geometry)?;
    let old_key = drive_controlling_startup(&mut harness, geometry, 3)?;
    let first = harness.ui.admit_switch(1)?;
    assert!(matches!(
        drive_switch_snapshot(&mut harness)?,
        UiEvent::SwitchSnapshot {
            decision: RefreshDecision::Cleanup(_),
            ..
        }
    ));

    let ClientMessage::TerminalViewer(detach) = harness.carrier.take_obcs()? else {
        return Err(test_error("first Switch must Detach the old Ready viewer"));
    };
    assert_eq!(
        detach.action(),
        &TerminalViewerAction::Detach {
            attachment_id: attached_viewer_id()?,
        }
    );

    if cut == SwitchSupersessionCut::BeforeHandoff {
        let newest = harness.ui.admit_switch(2)?;
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
            UiEvent::SwitchOldDetached {
                work,
                detached,
                continuation: SwitchDetachContinuation::Superseded { desired },
            } if work == first.key && detached == old_key && desired == newest.key
        ));
        settle_newest_as_attach_unavailable(&mut harness, &newest.key, cut)?;
        return harness.finish();
    }

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
        UiEvent::SwitchOldDetached {
            work,
            detached,
            continuation: SwitchDetachContinuation::Attach(_),
        } if work == first.key && detached == old_key
    ));

    let first_installed = harness.ui.state().desired().unwrap().clone();
    assert_eq!(first_installed.generation(), first.key.generation() + 1);
    let ClientMessage::TerminalViewer(attach) = harness.carrier.take_obcs()? else {
        return Err(test_error("first Switch target must AttachBinding"));
    };
    assert_eq!(attach.action(), &attach_action(1)?);
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(attach.request_id().clone(), attached_outcome()?),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Attached { key } if key == first_installed
    ));

    let ClientMessage::TerminalControl(acquire) = harness.carrier.take_obcs()? else {
        return Err(test_error("first Switch target must Acquire"));
    };
    assert_eq!(
        acquire.action(),
        &TerminalControlAction::Acquire {
            attachment_id: attached_viewer_id()?,
        }
    );
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalControlResult(
            TerminalControlResult::new(
                acquire.request_id().clone(),
                TerminalControlOutcome::Rejected {
                    reason: ControlRejectedReason::ControllerBusy,
                },
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::SetupConfigured {
            key,
            mode: ControllerMode::ReadOnly,
            applied_geometry: None,
        } if key == first_installed
    ));

    let ClientMessage::TerminalViewer(poll) = harness.carrier.take_obcs()? else {
        return Err(test_error("first Switch setup must Poll"));
    };
    assert_eq!(
        poll.action(),
        &TerminalViewerAction::Poll {
            attachment_id: attached_viewer_id()?,
        }
    );
    let newest = match cut {
        SwitchSupersessionCut::AfterPollHandoff => {
            let newest = harness.ui.admit_switch(2)?;
            harness
                .carrier
                .respond_obcs(ServerMessage::TerminalViewerResult(
                    TerminalViewerResult::new(
                        poll.request_id().clone(),
                        TerminalViewerOutcome::Record(full_frame_record(1, 1, geometry)?),
                    ),
                ))?;
            newest
        }
        SwitchSupersessionCut::AfterFrameDelivery => {
            harness
                .carrier
                .respond_obcs(ServerMessage::TerminalViewerResult(
                    TerminalViewerResult::new(
                        poll.request_id().clone(),
                        TerminalViewerOutcome::Record(full_frame_record(1, 1, geometry)?),
                    ),
                ))?;
            let delivered = harness.ui.take_next()?;
            let newest = harness.ui.admit_switch(2)?;
            assert!(matches!(
                harness.ui.settle_taken(delivered, FrameCut::None)?,
                UiEvent::EmptyPoll { key } if key == first_installed
            ));
            assert_eq!(harness.ui.drawn_frames(), 1);
            newest
        }
        SwitchSupersessionCut::AfterDraw => {
            harness
                .carrier
                .respond_obcs(ServerMessage::TerminalViewerResult(
                    TerminalViewerResult::new(
                        poll.request_id().clone(),
                        TerminalViewerOutcome::Record(full_frame_record(1, 1, geometry)?),
                    ),
                ))?;
            assert!(matches!(
                harness.ui.settle_next(FrameCut::AfterDrawSwitch(2))?,
                UiEvent::FrameDrawn { key, .. } if key == first_installed
            ));
            assert_eq!(harness.ui.drawn_frames(), 2);
            AdmittedSwitch {
                key: harness.ui.state().desired().unwrap().anchor().clone(),
                deadlines: first.deadlines,
            }
        }
        SwitchSupersessionCut::AfterFeedbackHandoff => {
            harness
                .carrier
                .respond_obcs(ServerMessage::TerminalViewerResult(
                    TerminalViewerResult::new(
                        poll.request_id().clone(),
                        TerminalViewerOutcome::Record(full_frame_record(1, 1, geometry)?),
                    ),
                ))?;
            let feedback = match harness.ui.settle_next(FrameCut::None)? {
                UiEvent::FrameDrawn { key, feedback } if key == first_installed => feedback,
                _ => return Err(test_error("first target frame must be drawn")),
            };
            let ClientMessage::TerminalViewer(feedback_request) = harness.carrier.take_obcs()?
            else {
                return Err(test_error("drawn first-target frame must send feedback"));
            };
            assert_eq!(feedback_request.action(), &feedback);
            let newest = harness.ui.admit_switch(2)?;
            harness
                .carrier
                .respond_obcs(ServerMessage::TerminalViewerResult(
                    TerminalViewerResult::new(
                        feedback_request.request_id().clone(),
                        TerminalViewerOutcome::FeedbackAccepted {},
                    ),
                ))?;
            newest
        }
        SwitchSupersessionCut::BeforeHandoff => unreachable!("handled before first Attach"),
    };

    let ClientMessage::TerminalViewer(cleanup) = harness.carrier.take_obcs()? else {
        return Err(test_error(
            "superseded attached target must be exactly Detached",
        ));
    };
    assert_eq!(
        cleanup.action(),
        &TerminalViewerAction::Detach {
            attachment_id: attached_viewer_id()?,
        }
    );
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                cleanup.request_id().clone(),
                TerminalViewerOutcome::Detached {},
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::CleanupSettled {
            key,
            continuation: crate::terminal_session::state::CleanupContinuation::Superseded {
                desired,
            },
        } if key == first_installed && desired == newest.key
    ));
    settle_newest_as_attach_unavailable(&mut harness, &newest.key, cut)?;
    harness.finish()
}

fn settle_newest_as_attach_unavailable(
    harness: &mut Harness,
    newest: &crate::terminal_session::state::AttachmentKey,
    cut: SwitchSupersessionCut,
) -> TestResult {
    assert!(matches!(
        drive_switch_snapshot(harness)?,
        UiEvent::SwitchSnapshot {
            decision: RefreshDecision::AttachRecovered(_),
            ..
        }
    ));
    let installed = harness.ui.state().desired().unwrap().clone();
    assert_eq!(installed.generation(), newest.generation() + 1);
    let ClientMessage::TerminalViewer(attach) = harness.carrier.take_obcs()? else {
        return Err(test_error("newest Switch must continue with AttachBinding"));
    };
    assert_eq!(attach.action(), &attach_action(2)?, "cut {cut:?}");
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                attach.request_id().clone(),
                TerminalViewerOutcome::Rejected {
                    reason: ViewerRejectedReason::TerminalUnavailable,
                },
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::SetupWithoutAttachment {
            key,
            continuation: crate::terminal_session::state::CleanupContinuation::Stable,
        } if key == installed
    ));
    assert_eq!(
        harness.ui.state().phase_kind(),
        SessionPhaseKind::RetryableAttachUnavailable
    );
    assert!(!harness.ui.projection_is_some());
    Ok(())
}

#[test]
fn switch_superseding_a_taken_attached_event_detaches_that_exact_viewer() -> TestResult {
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
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Obcs,
        ],
        geometry,
    )?;
    drive_controlling_startup(&mut harness, geometry, 3)?;
    let (first, attached) = drive_switch_to_pending_attached(&mut harness, 1)?;
    let newest = harness.ui.admit_switch(2)?;

    assert!(matches!(
        harness.ui.settle_taken(attached, FrameCut::None)?,
        UiEvent::AttachedSuperseded { key, desired }
            if key == first.key && desired == newest.key
    ));
    assert!(!harness.ui.projection_is_some());

    let ClientMessage::TerminalViewer(detach) = harness.carrier.take_obcs()? else {
        return Err(test_error(
            "superseded Attached result must be followed by exact Detach",
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
        UiEvent::CleanupSettled {
            key,
            continuation: CleanupContinuation::Superseded { desired },
        } if key == first.key && desired == newest.key
    ));

    assert!(matches!(
        drive_switch_snapshot(&mut harness)?,
        UiEvent::SwitchSnapshot {
            decision: RefreshDecision::AttachRecovered(_),
            ..
        }
    ));
    let newest_installed = harness.ui.state().desired().unwrap().clone();
    assert_eq!(newest_installed.generation(), newest.key.generation() + 1);
    let ClientMessage::TerminalViewer(attach) = harness.carrier.take_obcs()? else {
        return Err(test_error("cleanup must continue to the newest Switch"));
    };
    assert_eq!(attach.action(), &attach_action(2)?);
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                attach.request_id().clone(),
                TerminalViewerOutcome::Rejected {
                    reason: ViewerRejectedReason::TerminalUnavailable,
                },
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::SetupWithoutAttachment {
            key,
            continuation: CleanupContinuation::Stable,
        } if key == newest_installed
    ));
    harness.finish()
}

#[derive(Clone, Copy, Debug)]
enum TakenSetupSupersessionCut {
    Configuration,
    Feedback,
}

#[test]
fn switch_superseding_taken_setup_and_feedback_asserts_exact_generations() -> TestResult {
    for cut in [
        TakenSetupSupersessionCut::Configuration,
        TakenSetupSupersessionCut::Feedback,
    ] {
        run_taken_setup_supersession_cut(cut)?;
    }
    Ok(())
}

fn run_taken_setup_supersession_cut(cut: TakenSetupSupersessionCut) -> TestResult {
    let geometry = fixture_geometry()?;
    let mut steps = vec![
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
    ];
    if matches!(cut, TakenSetupSupersessionCut::Feedback) {
        steps.extend([WireKind::Obcs, WireKind::Obcs]);
    }
    steps.insert(7, WireKind::Workbench);
    steps.extend([WireKind::Obcs, WireKind::Workbench, WireKind::Obcs]);
    let mut harness = Harness::start(steps, geometry)?;
    drive_controlling_startup(&mut harness, geometry, 3)?;
    let (first, attached) = drive_switch_to_pending_attached(&mut harness, 1)?;
    assert!(matches!(
        harness.ui.settle_taken(attached, FrameCut::None)?,
        UiEvent::Attached { key } if key == first.key
    ));

    let ClientMessage::TerminalControl(acquire) = harness.carrier.take_obcs()? else {
        return Err(test_error("first Switch target must Acquire"));
    };
    assert_eq!(
        acquire.action(),
        &TerminalControlAction::Acquire {
            attachment_id: attached_viewer_id()?,
        }
    );
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalControlResult(
            TerminalControlResult::new(
                acquire.request_id().clone(),
                TerminalControlOutcome::Rejected {
                    reason: ControlRejectedReason::ControllerBusy,
                },
            ),
        ))?;

    let pending = match cut {
        TakenSetupSupersessionCut::Configuration => harness.ui.take_next()?,
        TakenSetupSupersessionCut::Feedback => {
            assert!(matches!(
                harness.ui.settle_next(FrameCut::None)?,
                UiEvent::SetupConfigured {
                    key,
                    mode: ControllerMode::ReadOnly,
                    applied_geometry: None,
                } if key == first.key
            ));
            let ClientMessage::TerminalViewer(poll) = harness.carrier.take_obcs()? else {
                return Err(test_error("configured first target must Poll"));
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
                UiEvent::FrameDrawn { key, feedback } if key == first.key => feedback,
                _ => return Err(test_error("first target frame must be drawn")),
            };
            let ClientMessage::TerminalViewer(request) = harness.carrier.take_obcs()? else {
                return Err(test_error("drawn first-target frame must send feedback"));
            };
            assert_eq!(request.action(), &feedback);
            harness
                .carrier
                .respond_obcs(ServerMessage::TerminalViewerResult(
                    TerminalViewerResult::new(
                        request.request_id().clone(),
                        TerminalViewerOutcome::FeedbackAccepted {},
                    ),
                ))?;
            harness.ui.take_next()?
        }
    };

    let newest = harness.ui.admit_switch(2)?;
    let settled = harness.ui.settle_taken(pending, FrameCut::None)?;
    match cut {
        TakenSetupSupersessionCut::Configuration => assert!(matches!(
            settled,
            UiEvent::SetupConfigurationSuperseded { key, desired }
                if key == first.key && desired == newest.key
        )),
        TakenSetupSupersessionCut::Feedback => assert!(matches!(
            settled,
            UiEvent::FeedbackSuperseded { key, desired }
                if key == first.key && desired == newest.key
        )),
    }

    let ClientMessage::TerminalViewer(detach) = harness.carrier.take_obcs()? else {
        return Err(test_error("superseded setup must Detach its exact viewer"));
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
        UiEvent::CleanupSettled {
            key,
            continuation: CleanupContinuation::Superseded { desired },
        } if key == first.key && desired == newest.key
    ));

    assert!(matches!(
        drive_switch_snapshot(&mut harness)?,
        UiEvent::SwitchSnapshot {
            decision: RefreshDecision::AttachRecovered(_),
            ..
        }
    ));
    let newest_installed = harness.ui.state().desired().unwrap().clone();
    assert_eq!(newest_installed.generation(), newest.key.generation() + 1);
    let ClientMessage::TerminalViewer(attach) = harness.carrier.take_obcs()? else {
        return Err(test_error("cleanup must continue to the newest Switch"));
    };
    assert_eq!(attach.action(), &attach_action(2)?);
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                attach.request_id().clone(),
                TerminalViewerOutcome::Rejected {
                    reason: ViewerRejectedReason::TerminalUnavailable,
                },
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::SetupWithoutAttachment {
            key,
            continuation: CleanupContinuation::Stable,
        } if key == newest_installed
    ));
    harness.finish()
}

#[test]
fn switch_entry_expiry_distinguishes_unattempted_attach_from_busy_attempt() -> TestResult {
    run_unattempted_switch_entry_expiry()?;
    run_attempted_busy_switch_expiry()
}

fn run_unattempted_switch_entry_expiry() -> TestResult {
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
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Workbench,
        ],
        geometry,
    )?;
    drive_controlling_startup(&mut harness, geometry, 3)?;
    let (first, pending) = drive_switch_to_pending_unattached(&mut harness, 1)?;
    let newest = harness.ui.admit_switch(2)?;

    harness.elapsed_millis.store(44_998, Ordering::SeqCst);
    assert!(matches!(
        harness.ui.settle_taken(pending, FrameCut::None)?,
        UiEvent::SetupWithoutAttachment {
            key,
            continuation: CleanupContinuation::Superseded { desired },
        } if key == first.key && desired == newest.key
    ));
    let mailbox = Arc::clone(&harness.mailbox);
    let request = harness.carrier.take_read()?;
    let checkpoint = mailbox.pause_next_ui_reply_resume_for_test();
    harness.carrier.respond_read(
        &request,
        WorkbenchSnapshotOutcome::Available {
            snapshot: setup::snapshot(3, 1),
        },
    )?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::SwitchSnapshot {
            decision: RefreshDecision::AttachRecovered(_),
            ..
        }
    ));
    let installed = harness.ui.state().desired().unwrap().clone();
    assert_eq!(installed.generation(), newest.key.generation() + 1);
    assert!(checkpoint.await_reached_before(std::time::Instant::now() + Duration::from_secs(2)));
    harness.elapsed_millis.store(45_000, Ordering::SeqCst);
    drop(checkpoint);
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::SwitchEntryExpired {
            key,
            continuation: CleanupContinuation::Stable,
        } if key == installed
    ));
    assert_eq!(
        harness.ui.state().phase_kind(),
        SessionPhaseKind::RetryableAttachUnavailable
    );
    harness.finish()
}

fn run_attempted_busy_switch_expiry() -> TestResult {
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
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Obcs,
        ],
        geometry,
    )?;
    drive_controlling_startup(&mut harness, geometry, 3)?;
    let (first, pending) = drive_switch_to_pending_unattached(&mut harness, 1)?;
    let newest = harness.ui.admit_switch(2)?;
    assert!(matches!(
        harness.ui.settle_taken(pending, FrameCut::None)?,
        UiEvent::SetupWithoutAttachment {
            key,
            continuation: CleanupContinuation::Superseded { desired },
        } if key == first.key && desired == newest.key
    ));

    assert!(matches!(
        drive_switch_snapshot(&mut harness)?,
        UiEvent::SwitchSnapshot {
            decision: RefreshDecision::AttachRecovered(_),
            ..
        }
    ));
    let newest_installed = harness.ui.state().desired().unwrap().clone();
    assert_eq!(newest_installed.generation(), newest.key.generation() + 1);
    let ClientMessage::TerminalViewer(attach) = harness.carrier.take_obcs()? else {
        return Err(test_error("newest Switch must attempt AttachBinding"));
    };
    assert_eq!(attach.action(), &attach_action(2)?);
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                attach.request_id().clone(),
                TerminalViewerOutcome::Unavailable {
                    reason: ViewerUnavailableReason::ServerBusy,
                },
            ),
        ))?;
    if !harness
        .mailbox
        .await_condition_waiter_before(std::time::Instant::now() + Duration::from_secs(2))
    {
        return Err(test_error(
            "attempted Busy Attach did not enter its bounded retry wait",
        ));
    }
    harness.elapsed_millis.store(45_000, Ordering::SeqCst);
    harness.mailbox.wake_waiters();
    harness.finish_after_worker_failure(CarrierFailure::Deadline)
}

fn drive_switch_to_pending_unattached(
    harness: &mut Harness,
    target_index: usize,
) -> TestResult<(InstalledSwitch, TakenEvent)> {
    let old = harness.ui.state().desired().unwrap().clone();
    let switch = harness.ui.admit_switch(target_index)?;
    assert!(matches!(
        drive_switch_snapshot(harness)?,
        UiEvent::SwitchSnapshot {
            decision: RefreshDecision::Cleanup(_),
            ..
        }
    ));
    let ClientMessage::TerminalViewer(detach) = harness.carrier.take_obcs()? else {
        return Err(test_error("Switch must Detach the old Ready viewer"));
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
        UiEvent::SwitchOldDetached {
            work,
            detached,
            continuation: SwitchDetachContinuation::Attach(_),
        } if work == switch.key && detached == old
    ));

    let installed = InstalledSwitch {
        key: harness.ui.state().desired().unwrap().anchor().clone(),
        deadlines: switch.deadlines,
    };
    assert_eq!(installed.key.generation(), switch.key.generation() + 1);
    let ClientMessage::TerminalViewer(attach) = harness.carrier.take_obcs()? else {
        return Err(test_error("Switch target must AttachBinding"));
    };
    assert_eq!(attach.action(), &attach_action(target_index)?);
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                attach.request_id().clone(),
                TerminalViewerOutcome::Rejected {
                    reason: ViewerRejectedReason::TerminalUnavailable,
                },
            ),
        ))?;
    Ok((installed, harness.ui.take_next()?))
}

#[test]
fn superseded_first_attach_prewrite_timeout_skips_the_stale_entry() -> TestResult {
    let geometry = fixture_geometry()?;
    let mut harness = Harness::start_with_prewrite_gate(
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
            WireKind::Workbench,
            WireKind::Workbench,
            WireKind::Obcs,
        ],
        geometry,
    )?;
    drive_controlling_startup(&mut harness, geometry, 3)?;
    let (settled, pending) = drive_switch_to_pending_unattached(&mut harness, 1)?;
    assert!(matches!(
        harness.ui.settle_taken(pending, FrameCut::None)?,
        UiEvent::SetupWithoutAttachment {
            key,
            continuation: CleanupContinuation::Stable,
        } if key == settled.key
    ));
    if !harness
        .mailbox
        .await_condition_waiter_before(std::time::Instant::now() + Duration::from_secs(2))
    {
        return Err(test_error("worker did not finish the settled Switch lease"));
    }

    let stale = harness.ui.admit_switch(1)?;
    let read = harness.carrier.take_read()?;
    harness.carrier.respond_read(
        &read,
        WorkbenchSnapshotOutcome::Available {
            snapshot: setup::snapshot(3, 1),
        },
    )?;
    harness.arm_prewrite_gate()?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::SwitchSnapshot {
            decision: RefreshDecision::AttachRecovered(_),
            ..
        }
    ));
    let stale_installed = harness.ui.state().desired().unwrap().clone();
    assert_eq!(stale_installed.generation(), stale.key.generation() + 1);
    harness.await_prewrite_blocked()?;
    harness.elapsed_millis.store(10_000, Ordering::SeqCst);
    let newest = harness.ui.admit_switch(2)?;
    assert!(stale.key.generation() < newest.key.generation());
    harness.elapsed_millis.store(45_000, Ordering::SeqCst);
    harness.release_prewrite_gate()?;

    assert!(matches!(
        drive_switch_snapshot(&mut harness)?,
        UiEvent::SwitchSnapshot {
            decision: RefreshDecision::AttachRecovered(_),
            ..
        }
    ));
    let newest_installed = harness.ui.state().desired().unwrap().clone();
    assert_eq!(newest_installed.generation(), newest.key.generation() + 1);
    let ClientMessage::TerminalViewer(attach) = harness.carrier.take_obcs()? else {
        return Err(test_error(
            "newest Switch must retain its own forward budget",
        ));
    };
    assert_eq!(attach.action(), &attach_action(2)?);
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                attach.request_id().clone(),
                TerminalViewerOutcome::Rejected {
                    reason: ViewerRejectedReason::TerminalUnavailable,
                },
            ),
        ))?;
    assert!(
        matches!(harness.ui.settle_next(FrameCut::None)?, UiEvent::SetupWithoutAttachment { key, continuation: CleanupContinuation::Stable } if key == newest_installed)
    );
    assert_eq!(
        harness.ui.state().phase_kind(),
        SessionPhaseKind::RetryableAttachUnavailable
    );
    harness.finish()
}

#[derive(Clone, Copy, Debug)]
enum ReadyZeroWriteCut {
    Poll,
    Feedback,
}

#[test]
fn superseded_ready_poll_and_feedback_zero_write_at_old_deadline_then_switch() -> TestResult {
    for cut in [ReadyZeroWriteCut::Poll, ReadyZeroWriteCut::Feedback] {
        run_ready_zero_write_cut(cut)
            .map_err(|error| test_error(&format!("ready zero-write failed at {cut:?}: {error}")))?;
    }
    Ok(())
}

fn run_ready_zero_write_cut(cut: ReadyZeroWriteCut) -> TestResult {
    let geometry = fixture_geometry()?;
    let mut steps = vec![
        WireKind::Obcs,
        WireKind::Workbench,
        WireKind::Obcs,
        WireKind::Obcs,
        WireKind::Obcs,
        WireKind::Obcs,
        WireKind::Obcs,
    ];
    if matches!(cut, ReadyZeroWriteCut::Feedback) {
        steps.push(WireKind::Obcs);
    }
    steps.extend([WireKind::Workbench, WireKind::Obcs, WireKind::Obcs]);
    let mut harness = Harness::start_with_prewrite_gate(steps, geometry)?;
    let old = drive_controlling_startup(&mut harness, geometry, 2)?;

    if matches!(cut, ReadyZeroWriteCut::Poll) {
        harness.arm_prewrite_gate()?;
    }
    harness
        .mailbox
        .try_enqueue(QueuedCommand::Poll(Tagged::new(
            harness.ui.pane_ref(&old)?,
            (),
        )))
        .map_err(|error| test_error(&format!("failed to enqueue ordinary Poll: {error:?}")))?;
    match cut {
        ReadyZeroWriteCut::Poll => {}
        ReadyZeroWriteCut::Feedback => {
            let ClientMessage::TerminalViewer(poll) = harness.carrier.take_obcs()? else {
                return Err(test_error("ordinary Poll must reach the carrier"));
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
                        TerminalViewerOutcome::Record(full_frame_record(2, 2, geometry)?),
                    ),
                ))?;
            harness.arm_prewrite_gate()?;
            assert!(matches!(
                harness.ui.settle_next(FrameCut::None)?,
                UiEvent::FrameDrawn { key, .. } if key == old
            ));
        }
    }
    harness.await_prewrite_blocked()?;

    harness.elapsed_millis.store(10_000, Ordering::SeqCst);
    let switch = harness.ui.admit_switch(1)?;
    assert_eq!(
        switch
            .deadlines
            .forward_cutoff()
            .checked_duration_since(harness.ui.clock_now()),
        Some(Duration::from_secs(45))
    );
    harness.elapsed_millis.store(50_000, Ordering::SeqCst);
    harness.release_prewrite_gate()?;

    assert!(matches!(
        drive_switch_snapshot(&mut harness)?,
        UiEvent::SwitchSnapshot {
            decision: RefreshDecision::Cleanup(_),
            ..
        }
    ));
    let ClientMessage::TerminalViewer(detach) = harness.carrier.take_obcs()? else {
        return Err(test_error(
            "zero-write old request must drain before Switch Detach",
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
        UiEvent::SwitchOldDetached {
            work,
            detached,
            continuation: SwitchDetachContinuation::Attach(_),
        } if work == switch.key && detached == old
    ));

    let installed = harness.ui.state().desired().unwrap().clone();
    assert_eq!(installed.generation(), switch.key.generation() + 1);
    let ClientMessage::TerminalViewer(attach) = harness.carrier.take_obcs()? else {
        return Err(test_error(
            "Switch must retain its independent forward budget",
        ));
    };
    assert_eq!(attach.action(), &attach_action(1)?);
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                attach.request_id().clone(),
                TerminalViewerOutcome::Rejected {
                    reason: ViewerRejectedReason::TerminalUnavailable,
                },
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::SetupWithoutAttachment {
            key,
            continuation: CleanupContinuation::Stable,
        } if key == installed
    ));
    harness.finish()
}

/// Both legal same-tick orders at the 45-second forward boundary, forced by the
/// `CommandMailbox` test gates (`pause_next_ui_reply_await_sample_for_test` parks the
/// worker at its reply sample; `pause_next_ui_reply_deposit_for_test` parks the UI at its
/// deposit sample):
///   * UI-first — the worker is parked in its reply await while the exact-cut deposit
///     marks the cell `ForwardExpired`; releasing it lets the worker consume the mark and
///     emit the cleanup `Detach`.
///   * worker-first — the deposit parks at its clock sample until the worker's expiry has
///     emitted the cleanup `Detach`; the resumed same-tick deposit must observe
///     `MutationResult::Stale`, not `ForwardExpired`.
///
/// An unsynchronized `ForwardExpired`-only deposit oracle is not a correctness boundary:
/// a stale deposit is the legal outcome once the worker's expiry retires the taken event.
#[test]
fn switch_forward_and_cleanup_cuts_accept_before_and_expire_at_boundary() -> TestResult {
    run_switch_forward_boundary_ui_first()?;
    run_switch_forward_boundary_worker_first()
}

struct SwitchForwardBoundary {
    harness: Harness,
    configured: TakenEvent,
}

/// Drives a switch to a pending `SetupConfigured` handoff whose forward cutoff is exactly
/// 45 seconds from the harness clock origin — the point where the two legal same-tick
/// expiry orders diverge.
fn switch_forward_boundary() -> TestResult<SwitchForwardBoundary> {
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
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
        ],
        geometry,
    )?;
    drive_controlling_startup(&mut harness, geometry, 2)?;
    let sampled_at = harness.ui.clock_now();
    let (switch, attached) = drive_switch_to_pending_attached(&mut harness, 1)?;
    assert_eq!(
        switch
            .deadlines
            .forward_cutoff()
            .checked_duration_since(sampled_at),
        Some(Duration::from_secs(45))
    );
    assert_eq!(
        switch.deadlines.total().checked_duration_since(sampled_at),
        Some(Duration::from_secs(50))
    );
    assert_eq!(
        switch
            .deadlines
            .total()
            .checked_duration_since(switch.deadlines.forward_cutoff()),
        Some(Duration::from_secs(5))
    );

    harness.elapsed_millis.store(44_999, Ordering::SeqCst);
    assert!(matches!(
        harness.ui.settle_taken(attached, FrameCut::None)?,
        UiEvent::Attached { key } if key == switch.key
    ));
    let ClientMessage::TerminalControl(acquire) = harness.carrier.take_obcs()? else {
        return Err(test_error("accepted attachment must proceed to Acquire"));
    };
    assert_eq!(
        acquire.action(),
        &TerminalControlAction::Acquire {
            attachment_id: attached_viewer_id()?,
        }
    );
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalControlResult(
            TerminalControlResult::new(
                acquire.request_id().clone(),
                TerminalControlOutcome::Rejected {
                    reason: ControlRejectedReason::ControllerBusy,
                },
            ),
        ))?;
    let configured = harness
        .ui
        .take_next()
        .map_err(|error| test_error(&format!("missing boundary SetupConfigured: {error}")))?;
    Ok(SwitchForwardBoundary {
        harness,
        configured,
    })
}

/// From the observed boundary cleanup `Detach`: respond `Detached`, take the published
/// `CleanupSettled`, and settle it at exactly the 50-second total cut — the same
/// worker-ordered tail for both boundary orders.
fn settle_switch_forward_cleanup_at_total_cut(
    mut harness: Harness,
    detach: ClientMessage,
) -> TestResult {
    let ClientMessage::TerminalViewer(cleanup) = detach else {
        return Err(test_error("forward expiry must Detach the attached target"));
    };
    assert_eq!(
        cleanup.action(),
        &TerminalViewerAction::Detach {
            attachment_id: attached_viewer_id()?,
        }
    );
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                cleanup.request_id().clone(),
                TerminalViewerOutcome::Detached {},
            ),
        ))?;
    let cleanup = harness
        .ui
        .take_next()
        .map_err(|error| test_error(&format!("missing boundary CleanupSettled: {error}")))?;

    harness.elapsed_millis.store(50_000, Ordering::SeqCst);
    if harness.ui.settle_taken(cleanup, FrameCut::None).is_ok() {
        return Err(test_error(
            "cleanup handoff at exactly 50 seconds must retire the session",
        ));
    }
    assert_eq!(harness.mailbox.is_closed(), Some(CarrierFailure::Deadline));
    harness.finish_with_failure(CarrierFailure::Deadline)
}

fn run_switch_forward_boundary_ui_first() -> TestResult {
    let SwitchForwardBoundary {
        mut harness,
        configured,
    } = switch_forward_boundary()?;

    let worker_pause = harness.mailbox.pause_next_ui_reply_await_sample_for_test();
    if !worker_pause.await_reached_before(Instant::now() + WALL_GUARD) {
        return Err(test_error(
            "worker did not reach the armed reply-sample pause",
        ));
    }

    harness.elapsed_millis.store(45_000, Ordering::SeqCst);
    let error = match harness.ui.settle_taken(configured, FrameCut::None) {
        Err(error) => error,
        Ok(_) => {
            return Err(test_error(
                "a forward handoff at exactly 45 seconds must expire",
            ));
        }
    };
    assert_eq!(error.to_string(), "mailbox event failed: ForwardExpired");
    assert_eq!(harness.mailbox.is_closed(), None);
    drop(worker_pause);

    let detach = harness.carrier.take_obcs().map_err(|error| {
        test_error(&format!(
            "missing boundary cleanup Detach: {error}; mailbox={:?}",
            harness.mailbox.is_closed()
        ))
    })?;
    settle_switch_forward_cleanup_at_total_cut(harness, detach)
}

fn run_switch_forward_boundary_worker_first() -> TestResult {
    let SwitchForwardBoundary {
        mut harness,
        configured,
    } = switch_forward_boundary()?;

    let deposit_gate = harness.mailbox.pause_next_ui_reply_deposit_for_test();
    harness.elapsed_millis.store(45_000, Ordering::SeqCst);
    let detach = {
        let ui = &mut harness.ui;
        let carrier = &harness.carrier;
        thread::scope(|scope| -> TestResult<ClientMessage> {
            let parked_deposit = scope.spawn(move || ui.deposit_setup_configuration(configured));
            if !deposit_gate.await_reached_before(Instant::now() + WALL_GUARD) {
                drop(deposit_gate);
                return Err(test_error("UI deposit did not reach the armed sample gate"));
            }
            let detach = match carrier.take_obcs() {
                Ok(detach @ ClientMessage::TerminalViewer(_)) => detach,
                Ok(_) | Err(_) => {
                    drop(deposit_gate);
                    return Err(test_error(
                        "worker expiry did not emit the boundary cleanup Detach \
                         while the deposit was parked",
                    ));
                }
            };
            drop(deposit_gate);
            let deposited = parked_deposit
                .join()
                .map_err(|_panic| test_error("parked SetupConfigured deposit panicked"))??;
            if !matches!(deposited, Ok(MutationResult::Stale)) {
                return Err(test_error(
                    "a worker-first boundary deposit must observe Stale",
                ));
            }
            Ok(detach)
        })?
    };
    assert_eq!(harness.mailbox.is_closed(), None);
    settle_switch_forward_cleanup_at_total_cut(harness, detach)
}

#[test]
fn cleanup_reply_deposited_before_total_cut_wins_after_clock_reaches_cut() -> TestResult {
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
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Obcs,
            WireKind::Workbench,
            WireKind::Obcs,
        ],
        geometry,
    )?;
    drive_controlling_startup(&mut harness, geometry, 2)?;
    let (switch, attached) = drive_switch_to_pending_attached(&mut harness, 1)?;

    harness.elapsed_millis.store(45_000, Ordering::SeqCst);
    if harness.ui.settle_taken(attached, FrameCut::None).is_ok() {
        return Err(test_error(
            "the target projection handoff must expire at the forward cut",
        ));
    }
    assert_eq!(harness.mailbox.is_closed(), None);

    let ClientMessage::TerminalViewer(cleanup) = harness
        .carrier
        .take_obcs()
        .map_err(|error| test_error(&format!("missing deposited-reply cleanup Detach: {error}")))?
    else {
        return Err(test_error("forward expiry must enter the cleanup reserve"));
    };
    assert_eq!(
        cleanup.action(),
        &TerminalViewerAction::Detach {
            attachment_id: attached_viewer_id()?,
        }
    );
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                cleanup.request_id().clone(),
                TerminalViewerOutcome::Detached {},
            ),
        ))?;
    let cleanup = harness
        .ui
        .take_next()
        .map_err(|error| test_error(&format!("missing deposited-reply CleanupSettled: {error}")))?;

    harness.elapsed_millis.store(49_999, Ordering::SeqCst);
    assert!(matches!(
        harness.ui.settle_cleanup_before_and_advance_to(
            cleanup,
            harness.elapsed_millis.as_ref(),
            50_000,
        )?,
        UiEvent::CleanupSettled {
            key,
            continuation: CleanupContinuation::Stable,
        } if key == switch.key
    ));
    assert_eq!(harness.elapsed_millis.load(Ordering::SeqCst), 50_000);
    assert_eq!(harness.mailbox.is_closed(), None);
    assert_eq!(
        harness.ui.state().phase_kind(),
        SessionPhaseKind::RetryableAttachUnavailable
    );

    let retry = harness.ui.admit_switch(1)?;
    assert_eq!(retry.key.tab_id(), switch.key.tab_id());
    assert!(matches!(
        drive_switch_snapshot(&mut harness)?,
        UiEvent::SwitchSnapshot {
            decision: RefreshDecision::AttachRecovered(_),
            ..
        }
    ));
    let installed = harness.ui.state().desired().unwrap().clone();
    assert_eq!(installed.generation(), retry.key.generation() + 1);
    let ClientMessage::TerminalViewer(attach) = harness
        .carrier
        .take_obcs()
        .map_err(|error| test_error(&format!("missing post-cut retry Attach: {error}")))?
    else {
        return Err(test_error(
            "a deposited cleanup reply must license later traffic",
        ));
    };
    assert_eq!(attach.action(), &attach_action(1)?);
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                attach.request_id().clone(),
                TerminalViewerOutcome::Rejected {
                    reason: ViewerRejectedReason::TerminalUnavailable,
                },
            ),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::SetupWithoutAttachment {
            key,
            continuation: CleanupContinuation::Stable,
        } if key == installed
    ));
    harness.finish()
}

#[derive(Clone, Copy, Debug)]
enum ReadyDrainCut {
    ResizeSettlement,
    FeedbackSettlement,
}

#[test]
fn later_switch_drains_ready_resize_and_feedback_into_exact_detach_reserve() -> TestResult {
    for cut in [
        ReadyDrainCut::ResizeSettlement,
        ReadyDrainCut::FeedbackSettlement,
    ] {
        run_ready_drain_cut(cut)
            .map_err(|error| test_error(&format!("ready drain failed at {cut:?}: {error}")))?;
    }
    Ok(())
}

fn run_ready_drain_cut(cut: ReadyDrainCut) -> TestResult {
    let geometry = fixture_geometry()?;
    let changed = Geometry::new(
        geometry.columns(),
        geometry.rows(),
        geometry
            .cell_pixel_width()
            .checked_add(1)
            .ok_or("pixel width")?,
        geometry
            .cell_pixel_height()
            .checked_add(1)
            .ok_or("pixel height")?,
    )?;
    let mut steps = vec![
        WireKind::Obcs,
        WireKind::Workbench,
        WireKind::Obcs,
        WireKind::Obcs,
        WireKind::Obcs,
        WireKind::Obcs,
        WireKind::Obcs,
        WireKind::Obcs,
    ];
    if matches!(cut, ReadyDrainCut::FeedbackSettlement) {
        steps.push(WireKind::Obcs);
    }
    steps.push(WireKind::Obcs);
    let mut harness = Harness::start(steps, geometry)?;
    let old = drive_controlling_startup(&mut harness, geometry, 2)?;

    let pending = match cut {
        ReadyDrainCut::ResizeSettlement => {
            harness.ui.admit_resize(changed)?;
            let ClientMessage::TerminalControl(resize) = harness.carrier.take_obcs()? else {
                return Err(test_error("ordinary Resize must reach the carrier"));
            };
            assert!(matches!(
                resize.action(),
                TerminalControlAction::Resize {
                    attachment_id,
                    sequence,
                    geometry: actual,
                    ..
                } if attachment_id == &attached_viewer_id()?
                    && sequence.get() == 2
                    && actual == &changed
            ));
            harness
                .carrier
                .respond_obcs(ServerMessage::TerminalControlResult(
                    TerminalControlResult::new(
                        resize.request_id().clone(),
                        TerminalControlOutcome::Resized { geometry: changed },
                    ),
                ))?;
            harness.ui.take_next()?
        }
        ReadyDrainCut::FeedbackSettlement => {
            harness
                .mailbox
                .try_enqueue(QueuedCommand::Poll(Tagged::new(
                    harness.ui.pane_ref(&old)?,
                    (),
                )))
                .map_err(|error| {
                    test_error(&format!("failed to enqueue ordinary Poll: {error:?}"))
                })?;
            let ClientMessage::TerminalViewer(poll) = harness.carrier.take_obcs()? else {
                return Err(test_error("ordinary Poll must reach the carrier"));
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
                        TerminalViewerOutcome::Record(full_frame_record(2, 2, geometry)?),
                    ),
                ))?;
            let feedback = match harness.ui.settle_next(FrameCut::None)? {
                UiEvent::FrameDrawn { key, feedback } if key == old => feedback,
                _ => return Err(test_error("ordinary full frame must be drawn")),
            };
            let ClientMessage::TerminalViewer(request) = harness.carrier.take_obcs()? else {
                return Err(test_error("drawn ordinary frame must send feedback"));
            };
            assert_eq!(request.action(), &feedback);
            harness
                .carrier
                .respond_obcs(ServerMessage::TerminalViewerResult(
                    TerminalViewerResult::new(
                        request.request_id().clone(),
                        TerminalViewerOutcome::FeedbackAccepted {},
                    ),
                ))?;
            harness.ui.take_next()?
        }
    };

    let switch = harness.ui.admit_switch(1)?;
    harness.elapsed_millis.store(45_000, Ordering::SeqCst);
    let error = match harness.ui.settle_taken(pending, FrameCut::None) {
        Err(error) => error,
        Ok(_) => return Err(test_error("later Switch must expire the ready event drain")),
    };
    // At the exact cut, either the UI observes ForwardExpired directly or the worker
    // persists that receipt first and makes this now-late UI deposit stale.
    assert!(matches!(
        error.to_string().as_str(),
        "mailbox event failed: ForwardExpired" | "current UI reply was stale"
    ));
    assert_eq!(harness.mailbox.is_closed(), None);

    // No candidate was installed before the forward cut. The typed expiry event licenses
    // only exact old-viewer cleanup; its settlement retires this preinstall operation.
    assert!(
        matches!(harness.ui.settle_next(FrameCut::None)?, UiEvent::SwitchSnapshot { work, decision: RefreshDecision::Cleanup(_) } if work == switch.key)
    );
    let ClientMessage::TerminalViewer(detach) = harness.carrier.take_obcs().map_err(|error| {
        test_error(&format!(
            "ready drain must enter exact Detach reserve: {error}; mailbox={:?}",
            harness.mailbox.is_closed()
        ))
    })?
    else {
        return Err(test_error("ready drain must send terminal.viewer Detach"));
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
        UiEvent::SwitchOldDetached {
            work,
            detached,
            continuation: SwitchDetachContinuation::SettledWithoutAttachment,
        } if work == switch.key && detached == old
    ));
    assert_eq!(harness.ui.state().phase_kind(), SessionPhaseKind::Retired);
    harness.finish_after_worker_failure(CarrierFailure::Deadline)
}

/// Both legal same-tick orders of an ordinary `SetupConfigured` reply expiring past its
/// forward cutoff while a later Switch is still inside its own deadline — forced by the
/// `CommandMailbox` test gates (`pause_next_ui_reply_await_sample_for_test` parks the worker
/// at its reply-await sample; `pause_next_ui_reply_deposit_for_test` parks the UI at its
/// deposit sample):
///   * UI-first — the worker is parked at its await sample while the past-cut deposit
///     evaluates `ForwardExpired`; releasing it lets the worker consume the expiry and
///     retire the mailbox as `Deadline` without Detach. The harness tolerates idle
///     publication probes, so this does not assert a completely silent connection.
///   * worker-first — the deposit parks at its clock sample until the worker's expired
///     await closes the mailbox; a worker-originated close resumes the same deposit so it
///     must observe `Closed`, never `ForwardExpired` or `Stale`.
///
/// An unsynchronized `ForwardExpired`-or-`Stale` oracle is not a correctness boundary: a
/// `Closed` deposit is the legal outcome once the worker's expiry closes the mailbox first.
#[test]
fn ordinary_own_deadline_before_later_switch_retires_without_detach() -> TestResult {
    ordinary_own_deadline_ui_first()?;
    ordinary_own_deadline_worker_first()
}

struct OrdinaryDeadlineBoundary {
    harness: Harness,
    pending: TakenEvent,
}

/// Drives an ordinary Resize to a pending `SetupConfigured` handoff whose reply cutoff is
/// already behind the clock once it reaches 50 seconds, then admits a later Switch whose
/// forward cutoff stays ahead of that point — the race the expiry order must be forced for.
fn ordinary_deadline_before_switch() -> TestResult<OrdinaryDeadlineBoundary> {
    let geometry = fixture_geometry()?;
    let changed = Geometry::new(
        geometry.columns(),
        geometry.rows(),
        geometry
            .cell_pixel_width()
            .checked_add(1)
            .ok_or("pixel width")?,
        geometry
            .cell_pixel_height()
            .checked_add(1)
            .ok_or("pixel height")?,
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
        ],
        geometry,
    )?;
    drive_controlling_startup(&mut harness, geometry, 2)?;
    harness.ui.admit_resize(changed)?;
    let ClientMessage::TerminalControl(resize) = harness.carrier.take_obcs()? else {
        return Err(test_error("ordinary Resize must reach the carrier"));
    };
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalControlResult(
            TerminalControlResult::new(
                resize.request_id().clone(),
                TerminalControlOutcome::Resized { geometry: changed },
            ),
        ))?;
    let pending = harness.ui.take_next()?;

    harness.elapsed_millis.store(10_000, Ordering::SeqCst);
    let later = harness.ui.admit_switch(1)?;
    assert_eq!(
        later
            .deadlines
            .forward_cutoff()
            .checked_duration_since(harness.ui.clock_now()),
        Some(Duration::from_secs(45))
    );
    Ok(OrdinaryDeadlineBoundary { harness, pending })
}

fn ordinary_own_deadline_ui_first() -> TestResult {
    let OrdinaryDeadlineBoundary {
        mut harness,
        pending,
    } = ordinary_deadline_before_switch()?;
    let worker_pause = harness.mailbox.pause_next_ui_reply_await_sample_for_test();
    if !worker_pause.await_reached_before(Instant::now() + WALL_GUARD) {
        return Err(test_error(
            "worker did not reach the armed reply-await sample",
        ));
    }

    harness.elapsed_millis.store(50_000, Ordering::SeqCst);
    let error = match harness.ui.settle_taken(pending, FrameCut::None) {
        Err(error) => error,
        Ok(_) => return Err(test_error("ordinary own deadline must expire its UI event")),
    };
    assert_eq!(error.to_string(), "mailbox event failed: ForwardExpired");
    assert_eq!(harness.mailbox.is_closed(), None);
    drop(worker_pause);

    harness.finish_after_worker_failure(CarrierFailure::Deadline)
}

fn ordinary_own_deadline_worker_first() -> TestResult {
    let OrdinaryDeadlineBoundary {
        mut harness,
        pending,
    } = ordinary_deadline_before_switch()?;
    let deposit_gate = harness.mailbox.pause_next_ui_reply_deposit_for_test();
    harness.elapsed_millis.store(50_000, Ordering::SeqCst);
    let error = {
        let ui = &mut harness.ui;
        thread::scope(
            |scope| -> TestResult<Box<dyn std::error::Error + Send + Sync>> {
                let parked_deposit = scope.spawn(move || ui.settle_taken(pending, FrameCut::None));
                if !deposit_gate.await_reached_before(Instant::now() + WALL_GUARD) {
                    drop(deposit_gate);
                    return Err(test_error("UI deposit did not reach the armed sample gate"));
                }
                match parked_deposit
                    .join()
                    .map_err(|_panic| test_error("parked SetupConfigured deposit panicked"))?
                {
                    Err(error) => Ok(error),
                    Ok(_) => {
                        drop(deposit_gate);
                        Err(test_error("ordinary own deadline must expire its UI event"))
                    }
                }
            },
        )?
    };
    assert_eq!(
        error.to_string(),
        "mailbox event failed: Closed(MailboxClosed { failure: Deadline })"
    );
    assert_eq!(harness.mailbox.is_closed(), Some(CarrierFailure::Deadline));
    harness.finish_after_worker_failure(CarrierFailure::Deadline)
}

fn drive_switch_to_pending_attached(
    harness: &mut Harness,
    target_index: usize,
) -> TestResult<(InstalledSwitch, TakenEvent)> {
    let old = harness.ui.state().desired().unwrap().clone();
    let switch = harness.ui.admit_switch(target_index)?;
    assert!(matches!(
        drive_switch_snapshot(harness)?,
        UiEvent::SwitchSnapshot {
            decision: RefreshDecision::Cleanup(_),
            ..
        }
    ));
    let ClientMessage::TerminalViewer(detach) = harness
        .carrier
        .take_obcs()
        .map_err(|error| test_error(&format!("missing old-viewer Detach: {error}")))?
    else {
        return Err(test_error("Switch must first Detach its old viewer"));
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
        UiEvent::SwitchOldDetached {
            work,
            detached,
            continuation: SwitchDetachContinuation::Attach(_),
        } if work == switch.key && detached == old
    ));

    let installed = InstalledSwitch {
        key: harness.ui.state().desired().unwrap().anchor().clone(),
        deadlines: switch.deadlines,
    };
    assert_eq!(installed.key.generation(), switch.key.generation() + 1);
    let ClientMessage::TerminalViewer(attach) = harness
        .carrier
        .take_obcs()
        .map_err(|error| test_error(&format!("missing target Attach: {error}")))?
    else {
        return Err(test_error("Switch must AttachBinding its target"));
    };
    assert_eq!(attach.action(), &attach_action(target_index)?);
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(attach.request_id().clone(), attached_outcome()?),
        ))?;
    let attached = harness.ui.take_next()?;
    Ok((installed, attached))
}

#[derive(Clone, Copy, Debug)]
enum BlockedWaitCut {
    WireResponse,
    EventReply,
    CausalSettlement,
    ConditionVariable,
}

#[test]
fn close_preserves_first_cause_and_joins_every_native_blocking_wait() -> TestResult {
    for cut in [
        BlockedWaitCut::WireResponse,
        BlockedWaitCut::EventReply,
        BlockedWaitCut::CausalSettlement,
        BlockedWaitCut::ConditionVariable,
    ] {
        run_close_wait_cut(cut)
            .map_err(|error| test_error(&format!("close failed at {cut:?}: {error}")))?;
    }
    Ok(())
}

fn run_close_wait_cut(cut: BlockedWaitCut) -> TestResult {
    let geometry = fixture_geometry()?;
    match cut {
        BlockedWaitCut::WireResponse => {
            let harness = Harness::start(vec![WireKind::Obcs], geometry)?;
            let ClientMessage::TerminalOpen(open) = harness.carrier.take_obcs()? else {
                return Err(test_error("wire cut must stop on terminal.open"));
            };
            assert_eq!(open.geometry(), geometry);
            harness.finish()
        }
        BlockedWaitCut::EventReply => {
            let harness = Harness::start(vec![WireKind::Obcs, WireKind::Workbench], geometry)?;
            let ClientMessage::TerminalOpen(open) = harness.carrier.take_obcs()? else {
                return Err(test_error("event cut must open the terminal"));
            };
            harness
                .carrier
                .respond_obcs(ServerMessage::TerminalOpenResult(TerminalOpenResult::new(
                    open.request_id().clone(),
                    OpenOutcome::Opened {
                        terminal_create_request_id: obcs_request_id(900)?,
                    },
                )))?;
            let request = harness.carrier.take_read()?;
            harness.carrier.respond_read(
                &request,
                WorkbenchSnapshotOutcome::Available {
                    snapshot: setup::snapshot(1, 1),
                },
            )?;
            let _bootstrap_waiting_for_reply = harness.ui.take_next()?;
            harness.finish()
        }
        BlockedWaitCut::CausalSettlement => {
            let mut harness = Harness::start(
                vec![
                    WireKind::Obcs,
                    WireKind::Workbench,
                    WireKind::Obcs,
                    WireKind::Obcs,
                    WireKind::Obcs,
                ],
                geometry,
            )?;
            drive_open_snapshot_bootstrap(&mut harness, 1)?;
            drive_attach(&mut harness, 0)?;
            let ClientMessage::TerminalControl(acquire) = harness.carrier.take_obcs()? else {
                return Err(test_error("causal cut must Acquire"));
            };
            harness
                .carrier
                .respond_obcs(ServerMessage::TerminalControlResult(
                    TerminalControlResult::new(
                        acquire.request_id().clone(),
                        TerminalControlOutcome::Rejected {
                            reason: ControlRejectedReason::ControllerBusy,
                        },
                    ),
                ))?;
            assert!(matches!(
                harness.ui.settle_next(FrameCut::None)?,
                UiEvent::SetupConfigured {
                    mode: ControllerMode::ReadOnly,
                    ..
                }
            ));
            let ClientMessage::TerminalViewer(poll) = harness.carrier.take_obcs()? else {
                return Err(test_error("causal cut must Poll"));
            };
            harness
                .carrier
                .respond_obcs(ServerMessage::TerminalViewerResult(
                    TerminalViewerResult::new(
                        poll.request_id().clone(),
                        TerminalViewerOutcome::Empty {},
                    ),
                ))?;
            let _poll_waiting_for_causal_settlement = harness.ui.take_next()?;
            harness.finish()
        }
        BlockedWaitCut::ConditionVariable => {
            let mut harness = Harness::start(
                vec![
                    WireKind::Obcs,
                    WireKind::Workbench,
                    WireKind::Obcs,
                    WireKind::Obcs,
                ],
                geometry,
            )?;
            drive_open_snapshot_bootstrap(&mut harness, 1)?;
            let ClientMessage::TerminalViewer(first_attach) = harness.carrier.take_obcs()? else {
                return Err(test_error("condition-variable cut must AttachBinding"));
            };
            assert_eq!(first_attach.action(), &attach_action(0)?);
            harness
                .carrier
                .respond_obcs(ServerMessage::TerminalViewerResult(
                    TerminalViewerResult::new(
                        first_attach.request_id().clone(),
                        TerminalViewerOutcome::Unavailable {
                            reason: ViewerUnavailableReason::ServerBusy,
                        },
                    ),
                ))?;

            let ClientMessage::TerminalViewer(retry_attach) =
                harness.take_obcs_with_advancing_clock()?
            else {
                return Err(test_error(
                    "clock advance must release the condition-variable backoff",
                ));
            };
            assert_eq!(retry_attach.action(), &attach_action(0)?);
            harness
                .carrier
                .respond_obcs(ServerMessage::TerminalViewerResult(
                    TerminalViewerResult::new(
                        retry_attach.request_id().clone(),
                        TerminalViewerOutcome::Unavailable {
                            reason: ViewerUnavailableReason::ServerBusy,
                        },
                    ),
                ))?;
            if !harness
                .mailbox
                .await_condition_waiter_before(std::time::Instant::now() + Duration::from_secs(2))
            {
                return Err(test_error(
                    "worker did not re-enter the condition-variable wait after the second Busy",
                ));
            }
            harness.finish()
        }
    }
}
