use super::*;
use crate::local_client::tests::server::FakeServer;
use orchbee_client_server_protocol::{
    ClientMessage, ControlInput, ControlRejectedReason, ControllerId, Geometry, HostEpoch,
    RequestId, ServerMessage, SiteId, TerminalControlResult, TerminalId, TerminalViewerResult,
    ViewerId, ViewerRejectedReason, ViewerSnapshotReason, WorkerId, read_client_message,
    write_server_message,
};
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
use orchbee_local_ipc::LocalStream as UnixStream;
use std::io::{self, Read};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;
const ID: &str = "11111111111111111111111111111111";
fn admit(stream: &mut UnixStream) -> io::Result<()> {
    assert!(matches!(
        read_message(stream).map_err(io::Error::other)?,
        LocalClientMessage::ClientHello(_)
    ));
    write_message(
        stream,
        &LocalClientMessage::ClientAccepted {
            generation: NonZeroU64::MIN,
        },
    )
    .map_err(io::Error::other)
}
fn no_more_requests(stream: &mut UnixStream) -> io::Result<()> {
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes)?;
    assert!(
        bytes.is_empty(),
        "completed or ambiguous requests must not replay"
    );
    Ok(())
}

#[test]
fn viewer_busy_reissues_only_identity_and_preserves_exact_feedback() -> TestResult {
    for action in [
        Viewer::Attach {
            terminal_create_request_id: RequestId::parse(ID)?,
        },
        Viewer::AttachBinding {
            site_id: SiteId::parse(ID)?,
            worker_id: WorkerId::parse(ID)?,
            terminal_id: TerminalId::parse(ID)?,
        },
        Viewer::Poll {
            attachment_id: ViewerId::parse(ID)?,
        },
        Viewer::FrameApplied {
            attachment_id: ViewerId::parse(ID)?,
            sequence: NonZeroU64::new(7).unwrap(),
            generation: NonZeroU64::MIN,
            revision: 11,
        },
        Viewer::NeedSnapshot {
            attachment_id: ViewerId::parse(ID)?,
            sequence: NonZeroU64::new(7).unwrap(),
            reason: ViewerSnapshotReason::BaseMismatch,
        },
    ] {
        let expected = action.clone();
        let server = FakeServer::start(move |mut stream| {
            admit(&mut stream)?;
            let mut previous = None;
            for busy in [true, false] {
                let ClientMessage::TerminalViewer(request) =
                    read_client_message(&mut stream).map_err(io::Error::other)?
                else {
                    panic!("viewer request")
                };
                assert_eq!(request.action(), &expected);
                assert_ne!(previous.as_ref(), Some(request.request_id()));
                previous = Some(request.request_id().clone());
                let outcome = if busy {
                    ViewerOutcome::Unavailable {
                        reason: ViewerUnavailableReason::ServerBusy,
                    }
                } else {
                    match &expected {
                        Viewer::Attach { .. } | Viewer::AttachBinding { .. } => {
                            ViewerOutcome::Attached {
                                attachment_id: ViewerId::parse(ID).unwrap(),
                                host_epoch: HostEpoch::parse(ID).unwrap(),
                                host_terminal_id:
                                    orchbee_client_server_protocol::HostTerminalId::parse(ID)
                                        .unwrap(),
                            }
                        }
                        Viewer::Poll { .. } => ViewerOutcome::Empty {},
                        _ => ViewerOutcome::FeedbackAccepted {},
                    }
                };
                write_server_message(
                    &mut stream,
                    &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                        request.request_id().clone(),
                        outcome,
                    )),
                )
                .map_err(io::Error::other)?;
            }
            no_more_requests(&mut stream)
        })?;
        let mut client = LocalClient::connect(server.endpoint(), "test-cohort")?;
        let original = Instant::now() + Duration::from_secs(1);
        client.set_submission_deadline(Some(original));
        let result = viewer(&mut client, &mut Identities(1), action);
        assert!(result.is_ok());
        assert_eq!(client.submission_deadline(), Some(original));
        drop(client);
        server.finish()?;
    }
    Ok(())
}

#[test]
fn control_busy_uses_fresh_ids_and_consumes_each_effect_sequence_once() -> TestResult {
    let geometry = Geometry::new(80, 24, 8, 16)?;
    for action in [
        Control::Acquire {
            attachment_id: ViewerId::parse(ID)?,
        },
        Control::Input {
            attachment_id: ViewerId::parse(ID)?,
            controller_id: ControllerId::parse(ID)?,
            sequence: NonZeroU64::new(41).unwrap(),
            bytes: ControlInput::new(b"one input".to_vec())?,
        },
        Control::Resize {
            attachment_id: ViewerId::parse(ID)?,
            controller_id: ControllerId::parse(ID)?,
            sequence: NonZeroU64::new(41).unwrap(),
            geometry,
        },
    ] {
        let expected = action.clone();
        let has_sequence = !matches!(action, Control::Acquire { .. });
        let server = FakeServer::start(move |mut stream| {
            admit(&mut stream)?;
            let mut previous = None;
            for attempt in 0..2 {
                let ClientMessage::TerminalControl(request) =
                    read_client_message(&mut stream).map_err(io::Error::other)?
                else {
                    panic!("control request")
                };
                assert_ne!(previous.as_ref(), Some(request.request_id()));
                previous = Some(request.request_id().clone());
                let mut expected = expected.clone();
                if let Control::Input { sequence, .. } | Control::Resize { sequence, .. } =
                    &mut expected
                {
                    *sequence = NonZeroU64::new(41 + attempt).unwrap();
                }
                assert_eq!(request.action(), &expected);
                let outcome = if attempt == 0 {
                    ControlOutcome::Unavailable {
                        reason: ControlUnavailableReason::ServerBusy,
                    }
                } else {
                    match expected {
                        Control::Clear { .. } => panic!("unexpected clear in controller fixture"),
                        Control::Takeover { .. } => {
                            panic!("unexpected takeover in backpressure fixture")
                        }
                        Control::Acquire { .. } => ControlOutcome::Acquired {
                            controller_id: ControllerId::parse(ID).unwrap(),
                        },
                        Control::Input { .. } => ControlOutcome::InputAccepted {},
                        Control::HostColors { .. } => ControlOutcome::HostColorsAccepted {},
                        Control::Resize { geometry, .. } => ControlOutcome::Resized { geometry },
                        Control::Paste { .. } => ControlOutcome::Pasted {
                            accepted_bytes:
                                orchbee_client_server_protocol::PasteAcceptedBytes::new(1).unwrap(),
                        },
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
            }
            no_more_requests(&mut stream)
        })?;
        let mut client = LocalClient::connect(server.endpoint(), "test-cohort")?;
        let mut sequence = 40;
        assert!(control(&mut client, &mut Identities(1), action, &mut sequence).is_ok());
        assert_eq!(sequence, if has_sequence { 42 } else { 40 });
        assert_eq!(client.submission_deadline(), None);
        drop(client);
        server.finish()?;
    }
    Ok(())
}

#[test]
fn a_busy_result_cannot_renew_the_deadline_for_a_silent_second_attempt() -> TestResult {
    let server = FakeServer::start(|mut stream| {
        admit(&mut stream)?;
        let ClientMessage::TerminalViewer(request) =
            read_client_message(&mut stream).map_err(io::Error::other)?
        else {
            panic!("viewer")
        };
        write_server_message(
            &mut stream,
            &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                request.request_id().clone(),
                ViewerOutcome::Unavailable {
                    reason: ViewerUnavailableReason::ServerBusy,
                },
            )),
        )
        .map_err(io::Error::other)?;
        let ClientMessage::TerminalViewer(second) =
            read_client_message(&mut stream).map_err(io::Error::other)?
        else {
            panic!("fresh viewer attempt")
        };
        assert_ne!(request.request_id(), second.request_id());
        no_more_requests(&mut stream)
    })?;
    let mut client = LocalClient::connect(server.endpoint(), "test-cohort")?;
    let started = Instant::now();
    client.set_submission_deadline(Some(started + Duration::from_millis(100)));
    assert_eq!(
        viewer(
            &mut client,
            &mut Identities(1),
            Viewer::Poll {
                attachment_id: ViewerId::parse(ID)?,
            },
        ),
        Err(TerminalStatus::OutcomeUnknown)
    );
    assert!(started.elapsed() < Duration::from_millis(400));
    drop(client);
    server.finish()?;
    Ok(())
}

#[test]
fn non_busy_results_and_unknown_effects_are_never_resubmitted() -> TestResult {
    for outcome in [
        ControlOutcome::OutcomeUnknown {},
        ControlOutcome::Rejected {
            reason: ControlRejectedReason::ControllerBusy,
        },
        ControlOutcome::Unavailable {
            reason: ControlUnavailableReason::TerminalRuntimeUnavailable,
        },
    ] {
        let expected = outcome.clone();
        let server = FakeServer::start(move |mut stream| {
            admit(&mut stream)?;
            let ClientMessage::TerminalControl(request) =
                read_client_message(&mut stream).map_err(io::Error::other)?
            else {
                panic!("control")
            };
            write_server_message(
                &mut stream,
                &ServerMessage::TerminalControlResult(TerminalControlResult::new(
                    request.request_id().clone(),
                    outcome,
                )),
            )
            .map_err(io::Error::other)?;
            no_more_requests(&mut stream)
        })?;
        let mut client = LocalClient::connect(server.endpoint(), "test-cohort")?;
        let result = control(
            &mut client,
            &mut Identities(1),
            Control::Acquire {
                attachment_id: ViewerId::parse(ID)?,
            },
            &mut 0,
        );
        if matches!(expected, ControlOutcome::OutcomeUnknown {}) {
            assert_eq!(result, Err(TerminalStatus::OutcomeUnknown));
        } else {
            assert_eq!(result, Ok(expected));
        }
        drop(client);
        server.finish()?;
    }
    for outcome in [
        ViewerOutcome::Rejected {
            reason: ViewerRejectedReason::FeedbackMismatch,
        },
        ViewerOutcome::Unavailable {
            reason: ViewerUnavailableReason::TerminalRuntimeUnavailable,
        },
    ] {
        let expected = outcome.clone();
        let server = FakeServer::start(move |mut stream| {
            admit(&mut stream)?;
            let ClientMessage::TerminalViewer(request) =
                read_client_message(&mut stream).map_err(io::Error::other)?
            else {
                panic!("viewer")
            };
            write_server_message(
                &mut stream,
                &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                    request.request_id().clone(),
                    outcome,
                )),
            )
            .map_err(io::Error::other)?;
            no_more_requests(&mut stream)
        })?;
        let mut client = LocalClient::connect(server.endpoint(), "test-cohort")?;
        assert_eq!(
            viewer(
                &mut client,
                &mut Identities(1),
                Viewer::Poll {
                    attachment_id: ViewerId::parse(ID)?,
                },
            ),
            Ok(expected)
        );
        drop(client);
        server.finish()?;
    }
    Ok(())
}
