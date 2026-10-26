//! Real Windows one-effect Panel policy receipts, caller fences and lost replies.
#![cfg(windows)]
use crate::peer_support;
use orchbee_local_ipc::LocalListener;
use orchbee_workbench_protocol::*;
use peer_support::{Fixture, assert_closed, bound_snapshot_reply, peer, validate};
use serde_json::{Value, json};
const PANEL: &str = "22222222222222222222222222222222";
const TERMINAL: &str = "33333333333333333333333333333333";
const WORKER: &str = "6f7263686265652d6c6f63616c2d7631";
const INPUT: &str = include_str!("../src/cli/schemas/orchbee.panel.input.json");
const FAILURE: &str = include_str!("../src/cli/schemas/orchbee.cli.failure.json");

#[test]
fn exact_policy_mutation_has_one_exchange_and_truthful_known_or_unknown_receipts() {
    for (case, outcome, failure) in [
        (
            "configured",
            json!({"kind":"panel-input-configured","revision":"10"}),
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
            json!({"kind":"panel-input-configured","revision":"10"}),
            Some("outcome-unknown"),
        ),
        (
            "revision",
            json!({"kind":"panel-input-configured","revision":"11"}),
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
        ("denied", json!({"kind":"denied"}), Some("denied")),
        (
            "stale",
            json!({"kind":"rejected","revision":"10","reason":"revision-mismatch"}),
            Some("rejected"),
        ),
        (
            "missing",
            json!({"kind":"rejected","revision":"9","reason":"panel-unavailable"}),
            Some("rejected"),
        ),
        (
            "capacity",
            json!({"kind":"unavailable","reason":"request-ledger-full"}),
            Some("service-unavailable"),
        ),
    ] {
        let fixture = Fixture::new();
        let rendezvous = fixture.rendezvous();
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let worker = peer(&fixture, listener, move |mut stream| {
            bound_snapshot_reply(&mut stream);
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
                json!({"kind":"panel.input.set","terminalContextId":"00000000000000000000000000000001","tabId":"00000000000000000000000000000002","panelId":PANEL,"rightClick":"panel"})
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
                "panel",
                "input",
                PANEL,
                "--right-click=panel",
                "--format=json",
            ],
            Some("invalid-metadata"),
            None,
        );
        worker.join().unwrap();
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        if let Some(kind) = failure {
            assert!(!output.status.success(), "{case}");
            assert_eq!(doc["failure"]["kind"], kind, "{case}: {doc}");
            validate(&doc, FAILURE);
        } else {
            assert!(output.status.success(), "{case}: {doc}");
            assert!(output.stderr.is_empty());
            validate(&doc, INPUT);
            assert_eq!(doc["result"]["outcome"], case);
            assert_eq!(doc["result"]["panelId"], PANEL);
            assert_eq!(doc["result"]["rightClick"], "panel");
            assert_eq!(doc["source"]["expectedRevision"], "9");
        }
    }
}

#[test]
fn current_policy_target_uses_exact_binding_and_stale_context_never_mutates() {
    for valid in [true, false] {
        let fixture = Fixture::new();
        let rendezvous = fixture.rendezvous();
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let worker = peer(&fixture, listener, move |mut stream| {
            bound_snapshot_reply(&mut stream);
            if valid {
                let WorkbenchRequest::Command(command) =
                    read_workbench_request(&mut stream).unwrap()
                else {
                    panic!("mutation")
                };
                assert!(
                    matches!(&command.action, WorkbenchAction::PanelInputSet { panel_id, right_click: PanelRightClickTarget::Orchbee, .. } if panel_id.as_str() == PANEL)
                );
                write_workbench_response(
                    &mut stream,
                    &WorkbenchResponse::Command(WorkbenchCommandResult {
                        protocol_version: ProtocolVersion,
                        request_id: command.request_id,
                        outcome: WorkbenchCommandOutcome::Unchanged {
                            revision: WorkbenchRevision::new(9),
                        },
                    }),
                )
                .unwrap();
            }
            assert_closed(&mut stream);
        });
        let output = fixture.run_with_caller(
            &[
                "panel",
                "input",
                "--current",
                "--right-click=orchbee",
                "--format=json",
            ],
            Some(if valid {
                TERMINAL
            } else {
                "99999999999999999999999999999999"
            }),
            Some(WORKER),
        );
        worker.join().unwrap();
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(output.status.success(), valid, "{doc}");
        if valid {
            validate(&doc, INPUT);
        } else {
            assert_eq!(doc["failure"]["kind"], "not-found");
            validate(&doc, FAILURE);
        }
    }
}
