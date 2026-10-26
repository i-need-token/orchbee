use super::*;
use crate::local_client::tests::server::FakeServer;
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
use orchbee_workbench_protocol::*;
use std::{
    io::{self, Read, Write},
    num::NonZeroU64,
    time::Duration,
};

fn command() -> WorkbenchCommand {
    serde_json::from_value(serde_json::json!({"protocolVersion":1,
        "requestId":format!("{:032x}",1),"sessionId":format!("{:032x}",2),"expectedRevision":"3",
        "action":{"kind":"layout.set-split-ratio","terminalContextId":format!("{:032x}",4),
            "tabId":format!("{:032x}",5),"split":{"splitId":format!("{:032x}",6),
                "firstSlotId":format!("{:032x}",7),"secondSlotId":format!("{:032x}",8)},
            "previousRatio":0.5,"ratio":0.7}}))
    .unwrap()
}
#[test]
fn canonical_reply_is_correlated_and_unknown_reply_drops_connection_without_replay() {
    for case in [
        "commit",
        "reject",
        "wrong-id",
        "wrong-revision",
        "wrong-outcome",
        "malformed",
        "lost",
    ] {
        let (closed, client_closed) = std::sync::mpsc::sync_channel(0);
        let server = FakeServer::start_connections(1, move |_, mut stream| {
            assert!(matches!(
                read_message(&mut stream).map_err(io::Error::other)?,
                LocalClientMessage::ClientHello(_)
            ));
            write_message(
                &mut stream,
                &LocalClientMessage::ClientAccepted {
                    generation: NonZeroU64::new(1).unwrap(),
                },
            )
            .map_err(io::Error::other)?;
            let WorkbenchRequest::Command(request) =
                read_workbench_request(&mut stream).map_err(io::Error::other)?
            else {
                panic!("one command")
            };
            assert_eq!(request, command());
            if case == "lost" {
                return Ok(());
            }
            if case == "malformed" {
                stream.write_all(&[1, 0, 0, 0, 0])?;
            } else {
                let outcome = match case {
                    "reject" => WorkbenchCommandOutcome::Rejected {
                        reason: WorkbenchRejectionReason::RevisionMismatch,
                        revision: WorkbenchRevision::new(5),
                    },
                    "wrong-outcome" => WorkbenchCommandOutcome::Unchanged {
                        revision: WorkbenchRevision::new(4),
                    },
                    _ => WorkbenchCommandOutcome::SplitRatioConfigured {
                        revision: WorkbenchRevision::new(if case == "wrong-revision" {
                            5
                        } else {
                            4
                        }),
                    },
                };
                write_workbench_response(
                    &mut stream,
                    &WorkbenchResponse::Command(WorkbenchCommandResult {
                        protocol_version: ProtocolVersion,
                        request_id: if case == "wrong-id" {
                            WorkbenchRequestId::parse(&format!("{:032x}", 9)).unwrap()
                        } else {
                            request.request_id
                        },
                        outcome,
                    }),
                )
                .map_err(io::Error::other)?;
            }
            // Probe after the owning Client has dropped: buffered retry bytes would still fail
            // the zero-byte assertion, while Client scheduling cannot consume the EOF guard.
            client_closed
                .recv_timeout(Duration::from_secs(3))
                .map_err(io::Error::other)?;
            assert_eq!(
                stream.read(&mut [0])?,
                0,
                "no retry on the mutation connection"
            );
            Ok(())
        })
        .unwrap();
        let mut client = LocalClient::connect(server.endpoint(), "test-cohort").unwrap();
        let result =
            client.set_split_ratio(command(), Instant::now() + Duration::from_secs(1), None);
        if case == "commit" {
            assert_eq!(result, Ok(WorkbenchRevision::new(4)));
        } else {
            assert_eq!(
                result,
                Err(SplitRatioFailure {
                    may_have_committed: case != "reject",
                    reason: if case == "reject" {
                        Reason::StaleView
                    } else {
                        Reason::ProjectionUnavailable
                    }
                })
            );
        }
        if !matches!(case, "commit" | "reject") {
            assert!(client.connection.is_none());
            assert_eq!(
                client.set_split_ratio(command(), Instant::now() + Duration::from_secs(1), None),
                Err(SplitRatioFailure {
                    may_have_committed: false,
                    reason: Reason::ClientDisconnected
                })
            );
        }
        drop(client);
        if case != "lost" {
            closed.send(()).unwrap();
        }
        server
            .finish()
            .unwrap_or_else(|error| panic!("{case}: {error}"));
    }
}
