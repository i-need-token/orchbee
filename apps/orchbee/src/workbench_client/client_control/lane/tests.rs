use super::*;
use crate::local_client::{LocalClient, tests::server::FakeServer};
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
use orchbee_workbench_protocol::{
    WorkbenchRequest, parse_workbench_request, read_workbench_request,
};
use std::{
    io::Read,
    num::NonZeroU64,
    sync::{Arc, mpsc},
    time::Instant,
};

mod configured_invocation;
mod split_ratios;

#[test]
fn graphics_draw_publication_uses_the_registered_connection_and_shutdown_interrupts_it() {
    use orchbee_workbench_protocol::*;
    for stalled in [false, true] {
        let (reached_tx, reached_rx) = mpsc::sync_channel(1);
        let server = FakeServer::start_connections(1, move |index, stream| {
            let mut stream = crate::local_client::tests::multiplex_peer::Peer::new(stream);
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
                    session_id: request.session_id.clone(),
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
            assert_eq!(graphics.session_id, request.session_id);
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
                        session_id: graphics.session_id,
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
        let client = LocalClient::connect(server.endpoint(), "test-cohort")
            .unwrap()
            .into_multiplexed()
            .unwrap();
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
        let mut lane = ClientControlLane::spawn(origin, clock, request.session_id).unwrap();
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

// Admission retries are owned by the shared carrier; cancellation and reconnect evidence
// lives with that transport, rather than retaining a second lane-specific retry algorithm.

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
        let server = FakeServer::start_connections(1, move |index, stream| {
            let mut stream = crate::local_client::tests::multiplex_peer::Peer::new(stream);
            assert!(matches!(
                read_message(&mut stream).map_err(io::Error::other)?,
                LocalClientMessage::ClientHello(_)
            ));
            write_message(
                &mut stream,
                &LocalClientMessage::ClientAccepted {
                    generation: NonZeroU64::new(7).unwrap(),
                },
            )
            .map_err(io::Error::other)?;
            {
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
        let client = LocalClient::connect(server.endpoint(), "test-cohort")
            .unwrap()
            .into_multiplexed()
            .unwrap();
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
        let mut lane = ClientControlLane::spawn(origin, clock, request.session_id).unwrap();
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
    for during_admission in [false] {
        let (reached_tx, reached_rx) = mpsc::sync_channel(1);
        let server = FakeServer::start_connections(1, move |index, stream| {
            let mut stream = crate::local_client::tests::multiplex_peer::Peer::new(stream);
            assert!(matches!(
                read_message(&mut stream).map_err(io::Error::other)?,
                LocalClientMessage::ClientHello(_)
            ));
            if !during_admission {
                write_message(
                    &mut stream,
                    &LocalClientMessage::ClientAccepted {
                        generation: NonZeroU64::new(1).unwrap(),
                    },
                )
                .map_err(io::Error::other)?;
            }
            {
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
        let client = LocalClient::connect(server.endpoint(), "test-cohort")
            .unwrap()
            .into_multiplexed()
            .unwrap();
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
            reply.session_id.clone(),
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
            Err(if during_admission {
                CommandFailure::NotSubmitted
            } else {
                CommandFailure::OutcomeUnknown
            })
        );
        drop(handoff);
        server.finish().unwrap();
    }
}
