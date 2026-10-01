//! Executable Windows event CLI witnesses reuse the existing admitted local-peer owner.
use super::*;
fn reply(
    stream: &mut orchbee_local_ipc::LocalStream,
    request: SemanticEventsRequest,
    outcome: SemanticEventsOutcome,
) {
    write_workbench_response(
        stream,
        &WorkbenchResponse::Events(SemanticEventsResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            site_id: request.site_id,
            outcome,
        }),
    )
    .unwrap();
}
#[test]
fn semantic_events_executable_retains_one_subscription_and_distinguishes_match_timeout_loss_and_refusal()
 {
    for mode in [
        "matched",
        "slice-retained",
        "baseline-pending",
        "baseline-unavailable",
        "timeout",
        "gap",
        "refused",
        "lost",
        "wrong-owner",
        "wrong-filter",
    ] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_snapshot_reply(&mut stream);
            let WorkbenchRequest::Events(open) = read_workbench_request(&mut stream).unwrap()
            else {
                panic!("open")
            };
            let SemanticEventsAction::Subscribe {
                subscription_id,
                filters,
                after,
            } = &open.action
            else {
                panic!()
            };
            assert_eq!(filters.len(), 1);
            assert!(after.is_none());
            let id = subscription_id.clone();
            let mut cursor = SemanticEventCursor {
                epoch: WorkbenchRequestId::parse(&"e".repeat(32)).unwrap(),
                sequence: WorkbenchRevision::new(0),
            };
            if mode == "refused" {
                reply(
                    &mut stream,
                    open,
                    SemanticEventsOutcome::Failed {
                        reason: SemanticEventsFailure::SourceUnavailable,
                    },
                );
                assert_closed(&mut stream);
                return;
            }
            reply(
                &mut stream,
                open,
                SemanticEventsOutcome::Opened {
                    subscription_id: id.clone(),
                    cursor: cursor.clone(),
                    existing: false,
                },
            );
            let WorkbenchRequest::Events(mut wait) = read_workbench_request(&mut stream).unwrap()
            else {
                panic!("wait")
            };
            assert!(
                matches!(&wait.action,SemanticEventsAction::Wait {subscription_id,..} if *subscription_id==id)
            );
            if mode == "lost" {
                return;
            }
            if matches!(mode, "slice-retained" | "baseline-pending") {
                reply(
                    &mut stream,
                    wait.clone(),
                    if mode == "baseline-pending" {
                        SemanticEventsOutcome::Failed {
                            reason: SemanticEventsFailure::Busy,
                        }
                    } else {
                        SemanticEventsOutcome::TimedOut {
                            subscription_id: id.clone(),
                            cursor: cursor.clone(),
                        }
                    },
                );
                let WorkbenchRequest::Events(next) = read_workbench_request(&mut stream).unwrap()
                else {
                    panic!("retained slice")
                };
                assert_ne!(wait.request_id, next.request_id);
                assert!(
                    matches!(&next.action,SemanticEventsAction::Wait {subscription_id,..} if *subscription_id==id)
                );
                wait = next;
            }
            let outcome = if mode == "baseline-unavailable" {
                SemanticEventsOutcome::Failed {
                    reason: SemanticEventsFailure::Busy,
                }
            } else if mode == "timeout" {
                SemanticEventsOutcome::TimedOut {
                    subscription_id: id.clone(),
                    cursor: cursor.clone(),
                }
            } else if mode == "gap" {
                cursor.sequence = WorkbenchRevision::new(1);
                SemanticEventsOutcome::Gap {
                    subscription_id: id.clone(),
                    cursor: cursor.clone(),
                    gap: SemanticEventGap {
                        after: WorkbenchRevision::new(0),
                        through: WorkbenchRevision::new(1),
                    },
                }
            } else {
                cursor.sequence = WorkbenchRevision::new(1);
                SemanticEventsOutcome::Matched {
                    subscription_id: if mode == "wrong-owner" {
                        WorkbenchRequestId::parse(&"f".repeat(32)).unwrap()
                    } else {
                        id.clone()
                    },
                    cursor: cursor.clone(),
                    event: Box::new(SemanticEvent {
                        workspace_id: None,
                        position: SemanticEventPosition::Journal {
                            cursor: cursor.clone(),
                        },
                        workbench_revision: WorkbenchRevision::new(2),
                        event: if mode == "wrong-filter" {
                            SemanticEventKind::TabClosed
                        } else {
                            SemanticEventKind::TabRenamed
                        },
                        data: SemanticEventData::Tab {
                            terminal_context_id: WorkbenchTerminalContextId::parse(&"5".repeat(32))
                                .unwrap(),
                            tab_id: WorkbenchTabId::parse(&"6".repeat(32)).unwrap(),
                            label: "renamed".into(),
                            index: 0,
                        },
                    }),
                }
            };
            reply(&mut stream, wait, outcome);
            if mode == "wrong-owner" {
                assert_closed(&mut stream);
                return;
            }
            let WorkbenchRequest::Events(close) = read_workbench_request(&mut stream).unwrap()
            else {
                panic!("close")
            };
            assert!(
                matches!(&close.action,SemanticEventsAction::Close {subscription_id} if *subscription_id==id)
            );
            reply(
                &mut stream,
                close,
                SemanticEventsOutcome::Closed {
                    subscription_id: id,
                },
            );
            assert_closed(&mut stream);
        });
        let output = fixture.run(&[
            "events",
            "wait",
            "--event",
            "tab.renamed",
            "--timeout",
            if matches!(mode, "timeout" | "baseline-unavailable") {
                "0"
            } else {
                "2000"
            },
            "--format=json",
        ]);
        assert_eq!(
            output.status.code(),
            Some(match mode {
                "timeout" => 7,
                "baseline-unavailable" => 3,
                "gap" => 8,
                "refused" => 4,
                "lost" | "wrong-owner" | "wrong-filter" => 5,
                _ => 0,
            }),
            "{mode}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let doc: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &doc,
            if matches!(mode, "lost" | "wrong-owner" | "wrong-filter") {
                include_str!("../../src/cli/schemas/orchbee.cli.failure.events.json")
            } else {
                include_str!("../../src/cli/schemas/orchbee.events.wait.json")
            },
        );
        handler.join().unwrap();
    }
}
#[test]
fn semantic_events_executable_stream_emits_open_and_closed_records_with_owned_cleanup() {
    let fixture = Fixture::new();
    let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
    let handler = peer(&fixture, listener, move |mut stream| {
        bound_snapshot_reply(&mut stream);
        let WorkbenchRequest::Events(open) = read_workbench_request(&mut stream).unwrap() else {
            panic!()
        };
        let id = open.action.subscription_id().clone();
        let cursor = SemanticEventCursor {
            epoch: WorkbenchRequestId::parse(&"e".repeat(32)).unwrap(),
            sequence: WorkbenchRevision::new(0),
        };
        reply(
            &mut stream,
            open,
            SemanticEventsOutcome::Opened {
                subscription_id: id.clone(),
                cursor: cursor.clone(),
                existing: false,
            },
        );
        let WorkbenchRequest::Events(wait) = read_workbench_request(&mut stream).unwrap() else {
            panic!()
        };
        assert!(matches!(
            wait.action,
            SemanticEventsAction::Wait {
                timeout_milliseconds: 0,
                ..
            }
        ));
        reply(
            &mut stream,
            wait,
            SemanticEventsOutcome::TimedOut {
                subscription_id: id.clone(),
                cursor,
            },
        );
        let WorkbenchRequest::Events(close) = read_workbench_request(&mut stream).unwrap() else {
            panic!()
        };
        reply(
            &mut stream,
            close,
            SemanticEventsOutcome::Closed {
                subscription_id: id,
            },
        );
        assert_closed(&mut stream);
    });
    let output = fixture.run(&[
        "events",
        "subscribe",
        "--event",
        "tab.renamed",
        "--timeout",
        "0",
        "--format=json",
    ]);
    assert_eq!(
        output.status.code(),
        Some(7),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let lines: Vec<_> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    for line in &lines {
        validate(
            line,
            include_str!("../../src/cli/schemas/orchbee.events.stream.json"),
        );
    }
    assert_eq!(lines[0]["kind"], "opened");
    assert_eq!(lines[1]["kind"], "closed");
    assert_eq!(lines[1]["cleanup"], "closed");
    handler.join().unwrap();
}
