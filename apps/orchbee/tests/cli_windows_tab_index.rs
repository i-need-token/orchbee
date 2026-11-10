//! Real named-pipe receipts and loss cuts for one indexed Tab command.
#![cfg(windows)]
use crate::peer_support;
use orchbee_local_ipc::{LocalListener, LocalStream};
use orchbee_workbench_protocol::*;
use peer_support::{Fixture, assert_closed, peer, validate};
use serde_json::{Value, json};
const TAB: &str = "00000000000000000000000000000002";
const CONTEXT: &str = "00000000000000000000000000000001";
const RESULT: &str = include_str!("../src/cli/schemas/orchbee.tab.move.json");
const FAILURE: &str = include_str!("../src/cli/schemas/orchbee.cli.failure.tab-move.json");

fn snapshot_reply(stream: &mut LocalStream) {
    snapshot_reply_at(stream, 9, 0);
}

/// A snapshot at `revision` whose labels start `shift` Tabs later, so `tab 0` can name another Tab.
fn snapshot_reply_at(stream: &mut LocalStream, revision: u64, shift: usize) {
    let WorkbenchRequest::SnapshotRead(intent) = read_workbench_request(stream).unwrap() else {
        panic!("snapshot")
    };
    let tabs: Vec<_> = (0..3).map(|i| (i, i + shift)).map(|(i, n)| json!({
        "tabId":format!("{:032x}",2 + n * 3),"label":format!("tab {i}"),"pinned":false,"groupId":null,
        "root":{"kind":"panel","panelId":format!("{:032x}",3 + n * 3),
            "primaryPresentation":{"customLabel":null,"presentationId":format!("{:032x}",4 + n * 3),"label":"shell",
                "binding":{"terminalId":format!("{:032x}",20 + n),"workerId":"6f7263686265652d6c6f63616c2d7631"}},
            "childPresentations":[]}
    })).collect();
    let document = json!({"kind":"workbench.snapshot","protocolVersion":1,"sessionId":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","revision":revision.to_string(),
        "navigation":{ "projects":[], "directories":[], "workspaces":[{"workspaceId":CONTEXT,"terminalContextId":CONTEXT,"projectId":null,"primaryDirectoryId":null,"label":"Workspace"}]},
        "terminalContexts":[{"terminalContextId":CONTEXT,"groups":[],"tabs":tabs}]});
    let snapshot = parse_snapshot(&serde_json::to_vec(&document).unwrap()).unwrap();
    write_workbench_response(
        stream,
        &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
            protocol_version: ProtocolVersion,
            request_id: intent.request_id,
            outcome: WorkbenchSnapshotOutcome::Available { snapshot },
        }),
    )
    .unwrap();
}

#[test]
fn indexed_move_known_refused_and_unknown_outcomes_never_replay_or_focus() {
    for (case, outcome, failure) in [
        (
            "committed",
            json!({"kind":"tab-moved","revision":"10","terminalContextId":CONTEXT,"tabId":TAB}),
            None,
        ),
        (
            "unchanged",
            json!({"kind":"unchanged","revision":"9"}),
            None,
        ),
        ("lost", Value::Null, Some("outcome-unknown")),
        (
            "correlation",
            json!({"kind":"unchanged","revision":"9"}),
            Some("outcome-unknown"),
        ),
        (
            "revision",
            json!({"kind":"tab-moved","revision":"11","terminalContextId":CONTEXT,"tabId":TAB}),
            Some("outcome-unknown"),
        ),
        (
            "context",
            json!({"kind":"tab-moved","revision":"10","terminalContextId":"00000000000000000000000000000999","tabId":TAB}),
            Some("outcome-unknown"),
        ),
        (
            "tab",
            json!({"kind":"tab-moved","revision":"10","terminalContextId":CONTEXT,"tabId":"00000000000000000000000000000999"}),
            Some("outcome-unknown"),
        ),
        (
            "unchanged-revision",
            json!({"kind":"unchanged","revision":"10"}),
            Some("outcome-unknown"),
        ),
        (
            "wrong-action",
            json!({"kind":"renamed","revision":"10"}),
            Some("outcome-unknown"),
        ),
        (
            "wrong-rejection",
            json!({"kind":"rejected","revision":"9","reason":"identity-exhausted"}),
            Some("outcome-unknown"),
        ),
        ("denied", json!({"kind":"denied"}), Some("denied")),
        (
            "bounds",
            json!({"kind":"rejected","revision":"9","reason":"tab-move-index-out-of-bounds"}),
            Some("rejected"),
        ),
        (
            "order-conflict",
            json!({"kind":"rejected","revision":"9","reason":"tab-move-order-conflict"}),
            Some("rejected"),
        ),
        (
            "capacity",
            json!({"kind":"unavailable","reason":"request-ledger-full"}),
            Some("service-unavailable"),
        ),
        (
            "close-fence",
            json!({"kind":"unavailable","reason":"terminal-busy"}),
            Some("service-unavailable"),
        ),
    ] {
        let boundary = if case == "unchanged" {
            "1"
        } else if case == "bounds" {
            "4"
        } else {
            "2"
        };
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let worker = peer(&fixture, listener, move |mut stream| {
            snapshot_reply(&mut stream);
            let WorkbenchRequest::Command(command) = read_workbench_request(&mut stream).unwrap()
            else {
                panic!("one mutation")
            };
            assert_eq!(command.expected_revision, WorkbenchRevision::new(9));
            assert_eq!(
                command.session_id.as_str(),
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            );
            assert_eq!(
                json!(command.action),
                json!({"kind":"tab.move-index","terminalContextId":CONTEXT,"tabId":TAB,"insertIndex":boundary.parse::<usize>().unwrap()})
            );
            if case == "lost" {
                return;
            }
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::Command(WorkbenchCommandResult {
                    protocol_version: ProtocolVersion,
                    request_id: if case == "correlation" {
                        WorkbenchRequestId::parse("00000000000000000000000000000999").unwrap()
                    } else {
                        command.request_id
                    },
                    outcome: serde_json::from_value(outcome).unwrap(),
                }),
            )
            .unwrap();
            assert_closed(&mut stream);
        });
        let output = fixture.run_with_caller(
            &[
                "tab",
                "move",
                "tab 0",
                "--insert-index",
                boundary,
                "--format=json",
            ],
            Some("invalid-caller"),
            None,
        );
        worker.join().unwrap();
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        if let Some(kind) = failure {
            assert!(!output.status.success(), "{case}");
            assert_eq!(doc["failure"]["kind"], kind, "{case}: {doc}");
            assert_eq!(doc["version"], 1);
            validate(&doc, FAILURE);
            if case == "order-conflict" {
                assert_eq!(
                    doc["failure"]["details"]["reason"],
                    "tab-move-order-conflict"
                );
            }
            if case == "bounds" {
                assert_eq!(
                    doc["failure"]["details"]["reason"],
                    "tab-move-index-out-of-bounds"
                );
            }
        } else {
            assert!(output.status.success(), "{case}: {doc}");
            assert!(output.stderr.is_empty());
            validate(&doc, RESULT);
            assert_eq!(doc["result"]["outcome"], case);
            assert_eq!(
                doc["result"]["insertIndex"],
                boundary.parse::<usize>().unwrap()
            );
            assert_eq!(doc["result"]["tabId"], TAB);
            assert_eq!(doc["source"]["expectedRevision"], "9");
        }
    }
}

#[test]
fn order_classifies_order_conflict_but_not_index_only_refusal() {
    for (reason, kind, schema) in [
        (
            WorkbenchRejectionReason::TabMoveIndexOutOfBounds,
            "outcome-unknown",
            "orchbee.cli.failure",
        ),
        (
            WorkbenchRejectionReason::TabMoveOrderConflict,
            "rejected",
            "orchbee.cli.failure.tab-move",
        ),
    ] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let worker = peer(&fixture, listener, move |mut stream| {
            snapshot_reply(&mut stream);
            let WorkbenchRequest::Command(command) = read_workbench_request(&mut stream).unwrap()
            else {
                panic!("command")
            };
            assert!(matches!(command.action, WorkbenchAction::TabMove { .. }));
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::Command(WorkbenchCommandResult {
                    protocol_version: ProtocolVersion,
                    request_id: command.request_id,
                    outcome: WorkbenchCommandOutcome::Rejected {
                        revision: WorkbenchRevision::new(9),
                        reason,
                    },
                }),
            )
            .unwrap();
            assert_closed(&mut stream);
        });
        let output = fixture.run(&["tab", "move", TAB, "--order=last", "--format=json"]);
        worker.join().unwrap();
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(doc["schema"], schema);
        assert_eq!(doc["failure"]["kind"], kind);
        if reason == WorkbenchRejectionReason::TabMoveOrderConflict {
            assert_eq!(doc["version"], 1);
            validate(&doc, FAILURE);
        } else {
            assert_eq!(doc["version"], 1);
        }
    }
}

/// A `revision-mismatch` applied nothing, so the CLI reads a fresh snapshot and submits the same
/// action again under a fresh request identity at the fresh revision, at most three times, and
/// never once the target names another Tab (ORC-413).
#[test]
fn a_revision_mismatch_is_resubmitted_only_to_the_same_tab_and_at_most_three_times() {
    for case in ["same", "moved", "persistent"] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let (sender, receiver) = std::sync::mpsc::channel();
        let worker = peer(&fixture, listener, move |mut stream| {
            snapshot_reply(&mut stream);
            let mut requests = Vec::new();
            for attempt in 0..4_u64 {
                let WorkbenchRequest::Command(command) =
                    read_workbench_request(&mut stream).unwrap()
                else {
                    panic!("one mutation per attempt")
                };
                assert_eq!(
                    command.expected_revision,
                    WorkbenchRevision::new(9 + attempt)
                );
                assert_eq!(
                    json!(command.action),
                    json!({"kind":"tab.move-index","terminalContextId":CONTEXT,"tabId":TAB,"insertIndex":2})
                );
                assert!(
                    !requests.contains(&command.request_id),
                    "{case}: every attempt has a fresh request identity"
                );
                requests.push(command.request_id.clone());
                let committed = case == "same" && attempt == 1;
                let outcome = if committed {
                    json!({"kind":"tab-moved","revision":"11","terminalContextId":CONTEXT,"tabId":TAB})
                } else {
                    json!({"kind":"rejected","revision":(10 + attempt).to_string(),"reason":"revision-mismatch"})
                };
                write_workbench_response(
                    &mut stream,
                    &WorkbenchResponse::Command(WorkbenchCommandResult {
                        protocol_version: ProtocolVersion,
                        request_id: command.request_id,
                        outcome: serde_json::from_value(outcome).unwrap(),
                    }),
                )
                .unwrap();
                if committed || attempt == 3 {
                    break;
                }
                snapshot_reply_at(&mut stream, 10 + attempt, usize::from(case == "moved"));
                if case == "moved" {
                    break;
                }
            }
            assert_closed(&mut stream);
            sender.send(requests).unwrap();
        });
        let output = fixture.run(&[
            "tab",
            "move",
            "tab 0",
            "--insert-index",
            "2",
            "--format=json",
        ]);
        worker.join().unwrap();
        let requests = receiver.recv().unwrap();
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        let last = requests.last().unwrap();
        assert_eq!(doc["source"]["requestId"], last.as_str(), "{case}: {doc}");
        if case == "same" {
            assert!(output.status.success(), "{case}: {doc}");
            assert_eq!(requests.len(), 2);
            assert_eq!(doc["source"]["expectedRevision"], "10");
            validate(&doc, RESULT);
        } else {
            assert_eq!(output.status.code(), Some(3), "{case}: {doc}");
            assert_eq!(doc["failure"]["details"]["reason"], "revision-mismatch");
            assert_eq!(requests.len(), if case == "moved" { 1 } else { 4 });
        }
    }
}
