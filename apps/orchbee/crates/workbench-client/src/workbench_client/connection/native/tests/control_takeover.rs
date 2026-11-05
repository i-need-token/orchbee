//! Actual framed control and native UI; the Server stub never supplies a UI verdict.
use super::super::{AttachedTerminal, ClientExecutor};
use super::setup::{TestResult, admit};
use crate::client_view::ClientAction;
use crate::local_client::LocalClient;
use crate::local_client::test_support::server::FakeServer;
use crate::workbench_client::mailbox::QueuedCommand;
use crate::workbench_client::ui::tests::move_membership::ready_ui;
use crate::workbench_state::control_takeover::TakeoverResult;
use crate::workbench_state::{CarrierFailure, ControllerMode};
use orchbee_client_server_protocol::*;
use std::{
    io::{self, Read},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Cut {
    Refused,
    Unknown,
    ResizeLost,
    PauseLost,
    ResumeLost,
    UiLost,
    LostAgain,
    Complete,
}
fn exercise(cut: Cut) -> TestResult {
    let (mut ui, mailbox, _, _) = ready_ui();
    if cut == Cut::Complete {
        mailbox
            .observe_host_colors(Some(crate::client_view::TerminalQueryColors {
                foreground: 0xcccccc,
                background: 0x101010,
                cursor: None,
                palette: [None; 16],
            }))
            .unwrap();
    }
    ui.apply_client_action(ClientAction::RequestControlTakeover);
    assert!(!ui.forward(b"blocked"));
    let lease = mailbox.try_recv().unwrap().expect("explicit operation");
    let QueuedCommand::Refresh(marker) = lease.record() else {
        panic!("one refresh barrier");
    };
    let marker = marker.clone();
    let operation = marker.takeover().unwrap().clone();
    let expected = operation.clone();
    let viewer = ViewerId::parse(&"22".repeat(16))?;
    let controller = ControllerId::parse(&"55".repeat(16))?;
    let new_controller = controller.clone();
    let expected_viewer = viewer.clone();
    let server = FakeServer::start(move |mut stream| {
        admit(&mut stream)?;
        let ClientMessage::TerminalControl(request) =
            read_client_message(&mut stream).map_err(io::Error::other)?
        else {
            panic!("control required");
        };
        assert_eq!(request.request_id(), &expected.request_id);
        assert_eq!(
            request.action(),
            &TerminalControlAction::Takeover {
                attachment_id: expected_viewer.clone()
            }
        );
        if cut == Cut::Unknown {
            return Ok(());
        }
        let outcome = if cut == Cut::Refused {
            TerminalControlOutcome::Rejected {
                reason: ControlRejectedReason::ControllerBusy,
            }
        } else {
            TerminalControlOutcome::Acquired {
                controller_id: new_controller.clone(),
            }
        };
        write_server_message(
            &mut stream,
            &ServerMessage::TerminalControlResult(TerminalControlResult::new(
                request.request_id().clone(),
                outcome,
            )),
        )
        .map_err(io::Error::other)?;
        if cut != Cut::Refused {
            if cut == Cut::Complete {
                let ClientMessage::TerminalControl(colors) =
                    read_client_message(&mut stream).map_err(io::Error::other)?
                else {
                    panic!("query colors must precede input after a new grant")
                };
                let TerminalControlAction::HostColors {
                    foreground,
                    background,
                    sequence,
                    ..
                } = colors.action()
                else {
                    panic!("new controller must report even colors it reported under an old grant")
                };
                assert_eq!((foreground.get(), background.get()), (0xcccccc, 0x101010));
                assert_eq!(sequence.get(), 48);
                write_server_message(
                    &mut stream,
                    &ServerMessage::TerminalControlResult(TerminalControlResult::new(
                        colors.request_id().clone(),
                        TerminalControlOutcome::HostColorsAccepted {},
                    )),
                )
                .map_err(io::Error::other)?;
            }
            let ClientMessage::TerminalControl(resize) =
                read_client_message(&mut stream).map_err(io::Error::other)?
            else {
                panic!("resize required");
            };
            assert_eq!(
                resize.action(),
                &TerminalControlAction::Resize {
                    attachment_id: expected_viewer.clone(),
                    controller_id: new_controller,
                    sequence: std::num::NonZeroU64::new(if cut == Cut::Complete { 49 } else { 48 })
                        .unwrap(),
                    geometry: expected.geometry
                }
            );
            if cut == Cut::ResizeLost {
                return Ok(());
            }
            let outcome = if cut == Cut::LostAgain {
                TerminalControlOutcome::Rejected {
                    reason: ControlRejectedReason::NotController,
                }
            } else {
                TerminalControlOutcome::Resized {
                    geometry: expected.geometry,
                }
            };
            write_server_message(
                &mut stream,
                &ServerMessage::TerminalControlResult(TerminalControlResult::new(
                    resize.request_id().clone(),
                    outcome,
                )),
            )
            .map_err(io::Error::other)?;
            if cut != Cut::LostAgain {
                for demand in [ProjectionDemand::Paused, ProjectionDemand::Active] {
                    let ClientMessage::TerminalViewer(refresh) =
                        read_client_message(&mut stream).map_err(io::Error::other)?
                    else {
                        panic!("projection re-arm required even without new terminal output");
                    };
                    assert_eq!(
                        refresh.action(),
                        &TerminalViewerAction::SetProjectionDemand {
                            attachment_id: expected_viewer.clone(),
                            demand,
                        }
                    );
                    if matches!(
                        (cut, demand),
                        (Cut::PauseLost, ProjectionDemand::Paused)
                            | (Cut::ResumeLost, ProjectionDemand::Active)
                    ) {
                        return Ok(());
                    }
                    write_server_message(
                        &mut stream,
                        &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                            refresh.request_id().clone(),
                            TerminalViewerOutcome::FeedbackAccepted {},
                        )),
                    )
                    .map_err(io::Error::other)?;
                }
            }
        }
        let mut trailing = [0];
        assert_eq!(
            stream.read(&mut trailing)?,
            0,
            "no takeover replay, input replay or Viewer detach"
        );
        Ok(())
    })?;
    let client = LocalClient::connect(server.endpoint(), "native-control-test")?;
    let worker_mailbox = Arc::clone(&mailbox);
    let pane = operation.key.clone();
    let worker = thread::spawn(move || {
        let mut executor = ClientExecutor::new(client, Arc::clone(&worker_mailbox));
        executor.attachments.push(AttachedTerminal {
            control_observation_due: None,
            control_frame: None,
            key: pane,
            attachment_id: viewer,
            setup: None,
            announced: true,
            mode: Some(ControllerMode::ReadOnly),
            next_sequence: 47,
            accepted_geometry: None,
            reported_host_colors: if cut == Cut::Complete {
                worker_mailbox.host_colors().unwrap()
            } else {
                None
            },
        });
        let result = executor.run_refresh(&lease, marker);
        let evidence = executor.pending_takeover.take();
        match result {
            Ok(()) => worker_mailbox.finish_lease(lease).unwrap(),
            Err(failure) => {
                worker_mailbox.close(failure);
            }
        }
        (result, evidence)
    });
    let deadline = Instant::now() + Duration::from_secs(3);
    while !worker.is_finished() && Instant::now() < deadline {
        if cut == Cut::UiLost {
            if mailbox.cells().0 == "event" {
                mailbox.close(CarrierFailure::Transport);
            }
        } else {
            ui.tick();
        }
        thread::sleep(Duration::from_millis(1));
    }
    if !worker.is_finished() {
        mailbox.close(CarrierFailure::Deadline);
    }
    let (result, evidence) = worker.join().unwrap();
    server.finish()?;
    if cut == Cut::Unknown {
        assert!(ui.view().takeover_status().is_some());
        assert!(!ui.forward(b"uncertain authority"));
    } else if matches!(cut, Cut::Refused | Cut::LostAgain | Cut::Complete) {
        assert_eq!(result, Ok(()), "{cut:?}");
        assert!(evidence.is_none());
        assert!(mailbox.takeover_worker_loss().is_none());
        assert_eq!(
            ui.view().controller(),
            Some(if cut == Cut::Complete {
                crate::localization::DisplayController::Controlling
            } else {
                crate::localization::DisplayController::ReadOnly
            })
        );
        assert!(
            !ui.forward(b"old frame"),
            "new mode alone cannot authorize input"
        );
    } else {
        assert!(result.is_err());
        let evidence = evidence.expect("retain before every fallible boundary");
        assert_eq!(mailbox.takeover_worker_loss(), Some(evidence.clone()));
        assert_eq!(
            evidence.result,
            match cut {
                Cut::Unknown => TakeoverResult::Unknown,
                Cut::ResizeLost | Cut::PauseLost | Cut::ResumeLost => {
                    TakeoverResult::Granted(controller.clone())
                }
                Cut::UiLost => TakeoverResult::Ready(controller),
                _ => unreachable!(),
            }
        );
        ui.tick();
        assert!(ui.finalize_takeover_worker(Some(evidence)));
    }
    Ok(())
}
#[test]
fn control_takeover_carrier_success_and_native_ui_commit() -> TestResult {
    exercise(Cut::Complete)
}
#[test]
fn control_takeover_carrier_definite_refusal_keeps_viewer() -> TestResult {
    exercise(Cut::Refused)
}
#[test]
fn control_takeover_carrier_unknown_does_not_resubmit() -> TestResult {
    exercise(Cut::Unknown)
}
#[test]
fn control_takeover_carrier_grant_survives_resize_transport_loss() -> TestResult {
    exercise(Cut::ResizeLost)
}
#[test]
fn control_takeover_carrier_grant_survives_projection_pause_loss() -> TestResult {
    exercise(Cut::PauseLost)
}
#[test]
fn control_takeover_carrier_grant_survives_projection_resume_loss() -> TestResult {
    exercise(Cut::ResumeLost)
}
#[test]
fn control_takeover_carrier_ready_survives_ui_delivery_loss() -> TestResult {
    exercise(Cut::UiLost)
}
#[test]
fn control_takeover_carrier_peer_transfer_during_resize_is_read_only() -> TestResult {
    exercise(Cut::LostAgain)
}
