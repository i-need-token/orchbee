use super::*;
use crate::local_client::{LocalClient, tests::server::FakeServer};
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
use orchbee_workbench_protocol::{
    WorkbenchRequest, parse_workbench_request, read_workbench_request,
};
use std::{
    io::Read,
    num::NonZeroU64,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
};

mod split_ratios;

#[test]
fn graphics_draw_publication_uses_the_registered_connection_and_shutdown_interrupts_it() {
    use orchbee_workbench_protocol::*;
    for stalled in [false, true] {
        let (reached_tx, reached_rx) = mpsc::sync_channel(1);
        let server = FakeServer::start_connections(2, move |index, mut stream| {
            assert!(matches!(
                read_message(&mut stream).map_err(io::Error::other)?,
                LocalClientMessage::ClientHello(_)
            ));
            write_message(
                &mut stream,
                &LocalClientMessage::ClientAccepted {
                    generation: NonZeroU64::new(index as u64 + 1).unwrap(),
                },
            )
            .map_err(io::Error::other)?;
            if index == 0 {
                return Ok(());
            }
            let WorkbenchRequest::ClientView(request) =
                read_workbench_request(&mut stream).map_err(io::Error::other)?
            else {
                panic!("view must precede draw publication");
            };
            let ClientViewAction::Synchronize {
                heartbeat,
                view_revision,
                view,
                ..
            } = request.action
            else {
                panic!("synchronize");
            };
            let client_id = WorkbenchClientId::parse(&format!("{:032x}", 2)).unwrap();
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::ClientView(WorkbenchClientViewResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id.clone(),
                    site_id: request.site_id.clone(),
                    outcome: ClientViewOutcome::Synchronized {
                        client: Box::new(ClientViewRecord {
                            client_id: client_id.clone(),
                            heartbeat,
                            view_revision,
                            view: (*view).clone(),
                            availability: ClientViewAvailability::Current,
                        }),
                        control: None,
                    },
                }),
            )
            .map_err(io::Error::other)?;
            let WorkbenchRequest::PaneGraphics(graphics) =
                read_workbench_request(&mut stream).map_err(io::Error::other)?
            else {
                panic!("graphics publication");
            };
            assert_ne!(graphics.request_id, request.request_id);
            assert_eq!(graphics.site_id, request.site_id);
            let PaneGraphicsAction::Observe {
                client_id: owner,
                heartbeat: seen,
                observation,
            } = graphics.action
            else {
                panic!("observe only");
            };
            assert_eq!(owner, client_id);
            assert_eq!(seen, heartbeat);
            assert_eq!(observation.workbench_revision, view.workbench_revision);
            assert_eq!(
                observation.capabilities.transport,
                PaneGraphicsTransport::CellRaster
            );
            assert!(observation.capabilities.cell_pixels.is_none());
            assert!(!observation.capabilities.pixel_mouse);
            if !stalled {
                write_workbench_response(
                    &mut stream,
                    &WorkbenchResponse::PaneGraphics(PaneGraphicsResult {
                        protocol_version: ProtocolVersion,
                        request_id: graphics.request_id,
                        site_id: graphics.site_id,
                        outcome: PaneGraphicsOutcome::Observed {
                            client_id,
                            heartbeat,
                        },
                    }),
                )
                .map_err(io::Error::other)?;
            }
            reached_tx.send(()).unwrap();
            assert_eq!(stream.read(&mut [0])?, 0);
            Ok(())
        })
        .unwrap();
        let client = LocalClient::connect(server.endpoint(), "test-cohort").unwrap();
        let origin = client.origin();
        let clock = client.operation_clock();
        drop(client);
        let corpus: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../../schemas/workbench/v1/workbench-client-view-conformance-v1.json"
        ))
        .unwrap();
        let WorkbenchRequest::ClientView(request) =
            parse_workbench_request(&serde_json::to_vec(&corpus["valid"][0]["document"]).unwrap())
                .unwrap()
        else {
            panic!("Client sample");
        };
        let ClientViewAction::Synchronize {
            heartbeat,
            view_revision,
            view,
            ..
        } = request.action
        else {
            panic!("sample");
        };
        let graphics = PaneGraphicsClientObservation {
            workbench_revision: view.workbench_revision,
            capabilities: PaneGraphicsCapabilities {
                transport: PaneGraphicsTransport::CellRaster,
                cell_pixels: None,
                pixel_mouse: false,
            },
            complete: true,
            panes: vec![],
        };
        let mut lane = ClientControlLane::spawn(origin, clock, request.site_id).unwrap();
        assert!(lane.publish(Sample {
            heartbeat,
            revision: view_revision,
            view: *view,
            graphics: Some(graphics)
        }));
        reached_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        lane.close();
        assert!(lane.task.is_none());
        server.finish().unwrap();
    }
}

#[test]
fn closing_the_lane_wakes_and_joins_blocked_admission_and_exchange() {
    for during_admission in [false, true] {
        let (reached_tx, reached_rx) = mpsc::sync_channel(1);
        let server = FakeServer::start_connections(2, move |index, mut stream| {
            assert!(matches!(
                read_message(&mut stream).map_err(io::Error::other)?,
                LocalClientMessage::ClientHello(_)
            ));
            if index == 0 || !during_admission {
                write_message(
                    &mut stream,
                    &LocalClientMessage::ClientAccepted {
                        generation: NonZeroU64::new(u64::try_from(index + 1).unwrap()).unwrap(),
                    },
                )
                .map_err(io::Error::other)?;
            }
            if index == 1 {
                if !during_admission {
                    assert!(matches!(
                        read_workbench_request(&mut stream).map_err(io::Error::other)?,
                        WorkbenchRequest::ClientView(_)
                    ));
                }
                reached_tx.send(()).unwrap();
                let mut byte = [0];
                assert_eq!(
                    stream.read(&mut byte)?,
                    0,
                    "lane shutdown closes its connection and wakes blocked IO"
                );
            }
            Ok(())
        })
        .unwrap();
        let client = LocalClient::connect(server.endpoint(), "test-cohort").unwrap();
        let origin = client.origin();
        let clock = client.operation_clock();
        drop(client);
        let corpus: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../../schemas/workbench/v1/workbench-client-view-conformance-v1.json"
        ))
        .unwrap();
        let WorkbenchRequest::ClientView(request) =
            parse_workbench_request(&serde_json::to_vec(&corpus["valid"][0]["document"]).unwrap())
                .unwrap()
        else {
            panic!("view request");
        };
        let ClientViewAction::Synchronize {
            heartbeat,
            view_revision,
            view,
            ..
        } = request.action
        else {
            panic!("sample");
        };
        let mut lane = ClientControlLane::spawn(origin, clock, request.site_id).unwrap();
        assert!(lane.publish(Sample {
            graphics: None,
            heartbeat,
            revision: view_revision,
            view: *view
        }));
        reached_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        lane.close();
        assert!(lane.task.is_none());
        lane.close();
        server.finish().unwrap();
    }
}

#[test]
fn transient_startup_admission_failure_retries_before_first_synchronize() {
    use orchbee_workbench_protocol::*;

    let (synchronized_tx, synchronized_rx) = mpsc::sync_channel(1);
    let server = FakeServer::start_optional_connections(3, move |index, mut stream| {
        assert!(matches!(
            read_message(&mut stream).map_err(io::Error::other)?,
            LocalClientMessage::ClientHello(_)
        ));
        if index == 1 {
            // Close during admission. No Client identity or Workbench request was admitted.
            return Ok(());
        }
        write_message(
            &mut stream,
            &LocalClientMessage::ClientAccepted {
                generation: NonZeroU64::new(u64::try_from(index + 1).unwrap()).unwrap(),
            },
        )
        .map_err(io::Error::other)?;
        if index == 2 {
            let WorkbenchRequest::ClientView(request) =
                read_workbench_request(&mut stream).map_err(io::Error::other)?
            else {
                panic!("the retry must publish exactly one first Synchronize request");
            };
            let ClientViewAction::Synchronize {
                heartbeat,
                view_revision,
                view,
                ..
            } = request.action
            else {
                panic!("the first Workbench request must be Synchronize");
            };
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::ClientView(WorkbenchClientViewResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    site_id: request.site_id,
                    outcome: ClientViewOutcome::Synchronized {
                        client: Box::new(ClientViewRecord {
                            client_id: WorkbenchClientId::parse(&format!("{:032x}", 3)).unwrap(),
                            heartbeat,
                            view_revision,
                            view: *view,
                            availability: ClientViewAvailability::Current,
                        }),
                        control: None,
                    },
                }),
            )
            .map_err(io::Error::other)?;
            synchronized_tx.send(()).unwrap();
            let mut discarded = [0; 512];
            loop {
                match stream.read(&mut discarded) {
                    Ok(0) => break,
                    Ok(_) => {}
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                        ) => {}
                    Err(error) => return Err(error),
                }
            }
        }
        Ok(())
    })
    .unwrap();
    let client = LocalClient::connect(server.endpoint(), "test-cohort").unwrap();
    let origin = client.origin();
    let clock = client.operation_clock();
    drop(client);
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../../schemas/workbench/v1/workbench-client-view-conformance-v1.json"
    ))
    .unwrap();
    let WorkbenchRequest::ClientView(request) =
        parse_workbench_request(&serde_json::to_vec(&corpus["valid"][0]["document"]).unwrap())
            .unwrap()
    else {
        panic!("view request");
    };
    let ClientViewAction::Synchronize {
        heartbeat,
        view_revision,
        view,
        ..
    } = request.action
    else {
        panic!("sample");
    };
    let mut lane = ClientControlLane::spawn(origin, clock, request.site_id).unwrap();
    assert!(lane.publish(Sample {
        graphics: None,
        heartbeat,
        revision: view_revision,
        view: *view,
    }));

    let synchronized = synchronized_rx.recv_timeout(Duration::from_millis(2_500));
    let mut registered = false;
    if synchronized.is_ok() {
        let deadline = Instant::now() + Duration::from_secs(1);
        while Instant::now() < deadline {
            if lane.registered_for_test() {
                registered = true;
                break;
            }
            thread::yield_now();
        }
    }
    lane.close();
    server.finish().unwrap();
    synchronized.expect("a transient pre-Synchronize admission failure must retry once");
    assert!(
        registered,
        "the retry response must register the Client view"
    );
}

#[test]
fn closing_the_lane_wakes_and_joins_a_blocked_retry_admission() {
    let (retry_started_tx, retry_started_rx) = mpsc::sync_channel(1);
    let server = FakeServer::start_optional_connections(3, move |index, mut stream| {
        assert!(matches!(
            read_message(&mut stream).map_err(io::Error::other)?,
            LocalClientMessage::ClientHello(_)
        ));
        if index == 0 {
            write_message(
                &mut stream,
                &LocalClientMessage::ClientAccepted {
                    generation: NonZeroU64::new(1).unwrap(),
                },
            )
            .map_err(io::Error::other)?;
        } else if index == 1 {
            // The first lane admission ends before it receives a generation.
            return Ok(());
        } else {
            retry_started_tx.send(()).unwrap();
            assert_eq!(
                stream.read(&mut [0])?,
                0,
                "lane shutdown must interrupt the blocked retry admission"
            );
        }
        Ok(())
    })
    .unwrap();
    let client = LocalClient::connect(server.endpoint(), "test-cohort").unwrap();
    let admission_started = Instant::now();
    let mut lane = ClientControlLane::spawn(
        client.origin(),
        client.operation_clock(),
        SiteId::parse("00000000000000000000000000000001").unwrap(),
    )
    .unwrap();
    drop(client);

    let retry_started = retry_started_rx.recv_timeout(Duration::from_millis(2_500));
    let retry_start_elapsed = admission_started.elapsed();
    let close_started = Instant::now();
    lane.close();
    let close_elapsed = close_started.elapsed();
    assert!(lane.task.is_none());
    server.finish().unwrap();
    retry_started.expect("the first transient failure must reach the retry admission");
    assert!(
        retry_start_elapsed < Duration::from_secs(1),
        "the retry barrier must leave more than one second of the startup budget"
    );
    assert!(
        close_elapsed < Duration::from_secs(1),
        "shutdown must wake the retry admission instead of waiting for its deadline"
    );
}

#[test]
fn startup_admission_retry_is_single_and_shares_the_original_deadline() {
    use crate::local_client::tests::server::adjustable_operation_clock;

    let clock = adjustable_operation_clock(Arc::new(AtomicU64::new(0)));
    let mut deadlines = Vec::new();
    let admitted = admit_startup_with(&clock, |deadline| {
        deadlines.push(deadline);
        if deadlines.len() == 1 {
            Err(LocalClientConnectError::Admission(ConnectClientError::Io(
                io::ErrorKind::UnexpectedEof.into(),
            )))
        } else {
            Ok(7_u8)
        }
    })
    .unwrap();
    assert_eq!(admitted, 7);
    assert_eq!(deadlines.len(), 2);
    assert_eq!(deadlines[0], deadlines[1]);

    let mut attempts = 0;
    let error = admit_startup_with::<()>(&clock, |_| {
        attempts += 1;
        Err(LocalClientConnectError::Admission(
            ConnectClientError::ConnectFailed(io::ErrorKind::ConnectionRefused.into()),
        ))
    })
    .unwrap_err();
    assert!(matches!(
        error,
        LocalClientConnectError::Admission(ConnectClientError::ConnectFailed(_))
    ));
    assert_eq!(attempts, 2, "startup admission has at most one retry");
}

#[test]
fn startup_admission_does_not_renew_an_exhausted_deadline() {
    use crate::local_client::tests::server::adjustable_operation_clock;

    let elapsed = Arc::new(AtomicU64::new(0));
    let clock = adjustable_operation_clock(Arc::clone(&elapsed));
    let mut attempts = 0;
    let error = admit_startup_with::<()>(&clock, |_| {
        attempts += 1;
        elapsed.store(2_000, Ordering::Relaxed);
        Err(LocalClientConnectError::Admission(ConnectClientError::Io(
            io::ErrorKind::TimedOut.into(),
        )))
    })
    .unwrap_err();
    assert!(matches!(
        error,
        LocalClientConnectError::Admission(ConnectClientError::Io(_))
    ));
    assert_eq!(attempts, 1);
}

#[test]
fn startup_admission_hard_failures_are_not_retried() {
    use crate::local_client::tests::server::adjustable_operation_clock;
    use orchbee_local_client_protocol::{ProtocolError, RejectionReason};

    let failures = [
        LocalClientConnectError::Admission(ConnectClientError::Io(
            io::ErrorKind::PermissionDenied.into(),
        )),
        LocalClientConnectError::Admission(ConnectClientError::Io(
            io::ErrorKind::InvalidInput.into(),
        )),
        LocalClientConnectError::Admission(ConnectClientError::ConnectFailed(
            io::ErrorKind::PermissionDenied.into(),
        )),
        LocalClientConnectError::Admission(ConnectClientError::ConnectFailed(
            io::ErrorKind::InvalidInput.into(),
        )),
        LocalClientConnectError::Admission(ConnectClientError::Protocol(
            ProtocolError::InvalidMagic,
        )),
        LocalClientConnectError::Admission(ConnectClientError::Rejected(
            RejectionReason::CapacityExceeded,
        )),
        LocalClientConnectError::Admission(ConnectClientError::UnexpectedResponse),
        LocalClientConnectError::Admission(ConnectClientError::CancellationReady),
        LocalClientConnectError::Admission(ConnectClientError::CancellationSourceFault(
            io::ErrorKind::BrokenPipe.into(),
        )),
        LocalClientConnectError::RequestIoConfiguration(io::ErrorKind::InvalidInput.into()),
    ];
    for failure in failures {
        let clock = adjustable_operation_clock(Arc::new(AtomicU64::new(0)));
        let mut failure = Some(failure);
        let mut attempts = 0;
        let outcome = admit_startup_with::<()>(&clock, |_| {
            attempts += 1;
            Err(failure.take().unwrap())
        });
        assert!(outcome.is_err());
        assert_eq!(attempts, 1);
    }
}

#[test]
fn closing_during_the_one_workbench_snapshot_read_wakes_and_joins_without_delivery() {
    use orchbee_workbench_protocol::{parse_workbench_response, write_workbench_response};
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../../schemas/workbench/v1/workbench-client-view-conformance-v1.json"
    ))
    .unwrap();
    let document = |name: &str| {
        corpus["valid"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["name"] == name)
            .unwrap()["document"]
            .clone()
    };
    for intent in ["layout", "panel"] {
        let mut reply = document("synchronized");
        if intent == "layout" {
            reply["outcome"]["control"]["intent"] =
                document("submit-layout")["action"]["command"]["intent"].clone();
        } else {
            assert_eq!(reply["outcome"]["control"]["intent"]["kind"], "panel");
        }
        let reply = parse_workbench_response(&serde_json::to_vec(&reply).unwrap()).unwrap();
        let (reached_tx, reached_rx) = mpsc::sync_channel(1);
        let server = FakeServer::start_connections(2, move |index, mut stream| {
            assert!(matches!(
                read_message(&mut stream).map_err(io::Error::other)?,
                LocalClientMessage::ClientHello(_)
            ));
            write_message(
                &mut stream,
                &LocalClientMessage::ClientAccepted {
                    generation: NonZeroU64::new(if index == 1 { 7 } else { 1 }).unwrap(),
                },
            )
            .map_err(io::Error::other)?;
            if index == 1 {
                assert!(matches!(
                    read_workbench_request(&mut stream).map_err(io::Error::other)?,
                    WorkbenchRequest::ClientView(_)
                ));
                write_workbench_response(&mut stream, &reply).map_err(io::Error::other)?;
                let WorkbenchRequest::SnapshotRead(request) =
                    read_workbench_request(&mut stream).map_err(io::Error::other)?
                else {
                    panic!("exactly one canonical read");
                };
                assert_eq!(
                    request.request_id.as_str(),
                    "00000000000000000000000000000002"
                );
                reached_tx.send(()).unwrap();
                let mut byte = [0];
                assert_eq!(
                    stream.read(&mut byte)?,
                    0,
                    "shutdown interrupts the snapshot read without another request"
                );
            }
            Ok(())
        })
        .unwrap();
        let client = LocalClient::connect(server.endpoint(), "test-cohort").unwrap();
        let origin = client.origin();
        let clock = client.operation_clock();
        drop(client);
        let WorkbenchRequest::ClientView(request) =
            parse_workbench_request(&serde_json::to_vec(&document("synchronize")).unwrap())
                .unwrap()
        else {
            panic!("view");
        };
        let ClientViewAction::Synchronize {
            heartbeat,
            view_revision,
            view,
            ..
        } = request.action
        else {
            panic!("sample");
        };
        let mut lane = ClientControlLane::spawn(origin, clock, request.site_id).unwrap();
        assert!(lane.publish(Sample {
            graphics: None,
            heartbeat,
            revision: view_revision,
            view: *view
        }));
        reached_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(lane.take_control().is_none());
        lane.close();
        assert!(lane.task.is_none());
        assert!(lane.take_control().is_none());
        server.finish().unwrap();
    }
}

#[test]
fn configured_command_actor_cancels_and_joins_with_truthful_delivery_receipt() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../../schemas/workbench/v1/workbench-client-view-conformance-v1.json"
    ))
    .unwrap();
    let found = corpus["valid"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == "found")
        .unwrap()["document"]
        .clone();
    let orchbee_workbench_protocol::WorkbenchResponse::ClientView(reply) =
        serde_json::from_value(found).unwrap()
    else {
        panic!()
    };
    let ClientViewOutcome::Found { client: registered } = reply.outcome else {
        panic!()
    };
    for during_admission in [false, true] {
        let (reached_tx, reached_rx) = mpsc::sync_channel(1);
        let server = FakeServer::start_connections(2, move |index, mut stream| {
            assert!(matches!(
                read_message(&mut stream).map_err(io::Error::other)?,
                LocalClientMessage::ClientHello(_)
            ));
            if index == 0 || !during_admission {
                write_message(
                    &mut stream,
                    &LocalClientMessage::ClientAccepted {
                        generation: NonZeroU64::new(1).unwrap(),
                    },
                )
                .map_err(io::Error::other)?;
            }
            if index == 1 {
                if !during_admission {
                    assert!(matches!(
                        read_workbench_request(&mut stream).map_err(io::Error::other)?,
                        WorkbenchRequest::ConfiguredCommand(_)
                    ));
                }
                reached_tx.send(()).unwrap();
                assert_eq!(stream.read(&mut [0])?, 0);
            }
            Ok(())
        })
        .unwrap();
        let client = LocalClient::connect(server.endpoint(), "test-cohort").unwrap();
        let shared = Arc::new((
            Mutex::new(Handoff {
                invocation_pending: true,
                ..Default::default()
            }),
            Condvar::new(),
        ));
        let actor = configured_command::Invocation::start(
            Arc::clone(&shared),
            client.origin(),
            client.operation_clock(),
            reply.site_id.clone(),
            (*registered).clone(),
            orchbee_workbench_protocol::ConfiguredCommandInvocation {
                command_id: "cmd_tools".into(),
                source: None,
                selection: None,
                expected_revision: registered.view.workbench_revision,
                context: Box::default(),
            },
        )
        .unwrap();
        drop(client);
        reached_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        drop(actor);
        let mut handoff = shared.0.lock().unwrap();
        assert!(!handoff.invocation_pending);
        assert_eq!(
            handoff.invocation_result.take().unwrap().reply,
            Err(!during_admission)
        );
        drop(handoff);
        server.finish().unwrap();
    }
}
