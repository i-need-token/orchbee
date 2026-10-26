use super::*;
use orchbee_workbench_protocol::*;
use std::sync::atomic::{AtomicBool, Ordering};

#[test]
fn divider_submission_atomically_distinguishes_a_dead_lane_from_transient_backlog() {
    let command: WorkbenchCommand = serde_json::from_value(serde_json::json!({
        "protocolVersion":1, "requestId":format!("{:032x}",1), "sessionId":format!("{:032x}",2),
        "expectedRevision":"0", "action":{"kind":"layout.set-split-ratio", "terminalContextId":format!("{:032x}",3),
        "tabId":format!("{:032x}",4), "split":{"splitId":format!("{:032x}",5),
        "firstSlotId":format!("{:032x}",6), "secondSlotId":format!("{:032x}",7)}, "previousRatio":0.5, "ratio":0.7}
    })).unwrap();
    for state in 0..3 {
        let lane = ClientControlLane::for_test();
        assert_eq!(lane.layout_admission(), Ok(()));
        {
            let mut handoff = lane.shared.0.lock().unwrap();
            match state {
                0 => handoff.stopped = true,
                1 => handoff.failed = true,
                _ => handoff.layout_pending = true,
            }
        }
        let expected = Err(if state == 2 {
            ClientViewControlReason::Busy
        } else {
            ClientViewControlReason::ProjectionUnavailable
        });
        assert_eq!(lane.submit_layout(command.clone()), expected);
        assert!(lane.take_layout_for_test().is_none());
    }
}

#[test]
fn stalled_mutation_preserves_heartbeats_and_lane_shutdown_joins_its_connection() {
    for during_admission in [false, true] {
        let started = Instant::now();
        let stalled = Arc::new(AtomicBool::new(false));
        let seen = Arc::clone(&stalled);
        let (heartbeat_tx, heartbeat_rx) = mpsc::sync_channel(1);
        let server = FakeServer::start_optional_connections(3, move |index, mut stream| {
            eprintln!(
                "split-ratio during_admission={during_admission} lane={index} stage=hello-read elapsed={:?}",
                started.elapsed()
            );
            let hello_started = Instant::now();
            assert!(matches!(
                read_message(&mut stream)
                    .inspect_err(|error| {
                        eprintln!(
                            "split-ratio during_admission={during_admission} lane={index} stage=hello-read elapsed={:?} stage_elapsed={:?} error={error:?}",
                            started.elapsed(), hello_started.elapsed()
                        );
                    })
                    .map_err(io::Error::other)?,
                LocalClientMessage::ClientHello(_)
            ));
            if index != 2 || !during_admission {
                let accepted_started = Instant::now();
                write_message(
                    &mut stream,
                    &LocalClientMessage::ClientAccepted {
                        generation: NonZeroU64::new(index as u64 + 1).unwrap(),
                    },
                )
                .inspect_err(|error| {
                    eprintln!(
                        "split-ratio during_admission={during_admission} lane={index} stage=accepted-write elapsed={:?} stage_elapsed={:?} error={error:?}",
                        started.elapsed(), accepted_started.elapsed()
                    );
                })
                .map_err(io::Error::other)?;
            }
            if index == 0 {
                return Ok(());
            }
            if index == 2 {
                if !during_admission {
                    let command_started = Instant::now();
                    assert!(matches!(
                        read_workbench_request(&mut stream)
                            .inspect_err(|error| {
                                eprintln!(
                                    "split-ratio during_admission={during_admission} lane={index} stage=command-read elapsed={:?} stage_elapsed={:?} error={error:?}",
                                    started.elapsed(), command_started.elapsed()
                                );
                            })
                            .map_err(io::Error::other)?,
                        WorkbenchRequest::Command(WorkbenchCommand {
                            action: WorkbenchAction::SplitRatioSet { .. },
                            ..
                        })
                    ));
                }
                seen.store(true, Ordering::SeqCst);
                eprintln!(
                    "split-ratio during_admission={during_admission} lane={index} stage=mutation-stalled elapsed={:?}",
                    started.elapsed()
                );
                let shutdown_started = Instant::now();
                assert_eq!(
                    stream
                        .read(&mut [0])
                        .inspect(|bytes| {
                            eprintln!(
                                "split-ratio during_admission={during_admission} lane={index} stage=shutdown-read elapsed={:?} stage_elapsed={:?} bytes={bytes}",
                                started.elapsed(), shutdown_started.elapsed()
                            );
                        })
                        .inspect_err(|error| {
                            eprintln!(
                                "split-ratio during_admission={during_admission} lane={index} stage=shutdown-read elapsed={:?} stage_elapsed={:?} error={error:?}",
                                started.elapsed(), shutdown_started.elapsed()
                            );
                        })?,
                    0,
                    "owned shutdown wakes the stalled mutation"
                );
                return Ok(());
            }
            let mut witnessed = false;
            let mut request_ordinal = 0_usize;
            loop {
                request_ordinal += 1;
                let read_started = Instant::now();
                let request = match read_workbench_request(&mut stream) {
                    Ok(WorkbenchRequest::ClientView(request)) => request,
                    Err(error) => {
                        eprintln!(
                            "split-ratio during_admission={during_admission} lane={index} stage=heartbeat-read ordinal={request_ordinal} elapsed={:?} stage_elapsed={:?} witnessed={witnessed} error={error:?}",
                            started.elapsed(), read_started.elapsed()
                        );
                        return Ok(());
                    }
                    Ok(_) => panic!("heartbeat lane is independent"),
                };
                let read_elapsed = read_started.elapsed();
                let decoded_elapsed = started.elapsed();
                let ClientViewAction::Synchronize {
                    heartbeat,
                    view_revision,
                    view,
                    ..
                } = request.action
                else {
                    panic!()
                };
                let write_started = Instant::now();
                write_workbench_response(
                    &mut stream,
                    &WorkbenchResponse::ClientView(WorkbenchClientViewResult {
                        protocol_version: ProtocolVersion,
                        request_id: request.request_id,
                        session_id: request.session_id,
                        outcome: ClientViewOutcome::Synchronized {
                            control: None,
                            client: Box::new(ClientViewRecord {
                                client_id: WorkbenchClientId::parse(&format!("{:032x}", 2))
                                    .unwrap(),
                                heartbeat,
                                view_revision,
                                view: *view,
                                availability: ClientViewAvailability::Current,
                            }),
                        },
                    }),
                )
                .inspect_err(|error| {
                    eprintln!(
                        "split-ratio during_admission={during_admission} lane={index} stage=heartbeat-write ordinal={request_ordinal} elapsed={:?} stage_elapsed={:?} read_elapsed={read_elapsed:?} error={error:?}",
                        started.elapsed(), write_started.elapsed()
                    );
                })
                .map_err(io::Error::other)?;
                let write_elapsed = write_started.elapsed();
                if seen.load(Ordering::SeqCst) && !witnessed {
                    witnessed = true;
                    heartbeat_tx.send(()).unwrap();
                    eprintln!(
                        "split-ratio during_admission={during_admission} lane={index} stage=heartbeat-witness ordinal={request_ordinal} elapsed={:?}",
                        started.elapsed()
                    );
                }
                if request_ordinal == 1 {
                    eprintln!(
                        "split-ratio during_admission={during_admission} lane={index} stage=first-heartbeat-replied ordinal={request_ordinal} decoded_elapsed={decoded_elapsed:?} read_elapsed={read_elapsed:?} elapsed={:?} write_elapsed={write_elapsed:?}",
                        started.elapsed()
                    );
                }
            }
        })
        .unwrap();
        let client = LocalClient::connect(server.endpoint(), "test-cohort").unwrap();
        let mut lane = ClientControlLane::spawn(
            client.origin(),
            client.operation_clock(),
            SessionId::parse(&format!("{:032x}", 2)).unwrap(),
        )
        .unwrap();
        drop(client);
        eprintln!(
            "split-ratio during_admission={during_admission} stage=lane-spawned elapsed={:?}",
            started.elapsed()
        );
        let command: WorkbenchCommand =
            serde_json::from_value(serde_json::json!({"protocolVersion":1,
            "requestId":format!("{:032x}",1),"sessionId":format!("{:032x}",2),"expectedRevision":"0",
            "action":{"kind":"layout.set-split-ratio","terminalContextId":format!("{:032x}",3),
                "tabId":format!("{:032x}",4),"split":{"splitId":format!("{:032x}",5),
                "firstSlotId":format!("{:032x}",6),"secondSlotId":format!("{:032x}",7)},
                "previousRatio":0.5,"ratio":0.7}}))
            .unwrap();
        assert_eq!(lane.submit_layout(command.clone()), Ok(()));
        assert_eq!(
            lane.submit_layout(command),
            Err(ClientViewControlReason::Busy),
            "only one unresolved mutation"
        );
        let corpus: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../../../schemas/workbench/v1/workbench-client-view-conformance-v1.json"
        ))
        .unwrap();
        let WorkbenchRequest::ClientView(request) =
            parse_workbench_request(&serde_json::to_vec(&corpus["valid"][0]["document"]).unwrap())
                .unwrap()
        else {
            panic!()
        };
        let ClientViewAction::Synchronize {
            heartbeat,
            view_revision,
            view,
            ..
        } = request.action
        else {
            panic!()
        };
        assert!(lane.publish(Sample {
            heartbeat,
            revision: view_revision,
            view: *view,
            graphics: None
        }));
        eprintln!(
            "split-ratio during_admission={during_admission} stage=sample-published elapsed={:?}",
            started.elapsed()
        );
        let receipt_started = Instant::now();
        let receipt = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            heartbeat_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        }));
        if let Err(panic) = receipt {
            eprintln!(
                "split-ratio during_admission={during_admission} stage=heartbeat-receipt-panicked elapsed={:?} stage_elapsed={:?} stalled={} lane_failed={}",
                started.elapsed(),
                receipt_started.elapsed(),
                stalled.load(Ordering::SeqCst),
                lane.failed()
            );
            lane.close();
            let server_result = server.finish();
            eprintln!(
                "split-ratio during_admission={during_admission} stage=failure-cleanup elapsed={:?} server={server_result:?}",
                started.elapsed()
            );
            std::panic::resume_unwind(panic);
        }
        assert!(stalled.load(Ordering::SeqCst));
        assert!(lane.take_layout_result().is_none());
        lane.close();
        assert!(lane.task.is_none());
        let result = lane.take_layout_result().unwrap().reply.unwrap_err();
        assert_eq!(result.may_have_committed, !during_admission);
        lane.close();
        server.finish().unwrap();
        eprintln!(
            "split-ratio during_admission={during_admission} stage=complete elapsed={:?}",
            started.elapsed()
        );
    }
}
