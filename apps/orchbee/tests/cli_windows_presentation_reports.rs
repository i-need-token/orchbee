//! Actual Windows CLI report writes retain exact member identity and never replay.
#![cfg(windows)]
#[allow(dead_code)]
#[path = "support/windows_agent_peer.rs"]
mod support;
use orchbee_local_ipc::LocalListener;
use orchbee_workbench_protocol::*;
use serde_json::{Value, json};
use support::{Fixture, assert_closed, peer, validate};

fn id(n: u64) -> String {
    format!("{n:032x}")
}
fn snapshot(unbound: bool) -> Snapshot {
    serde_json::from_value(json!({"protocolVersion":1,"sessionId":id(2),"revision":"12",
        "navigation":{"projects":[], "directories":[], "workspaces":[{"workspaceId":id(4),"terminalContextId":id(4),"projectId":null,"primaryDirectoryId":null,"label":"Workspace"}]},
        "terminalContexts":[{"terminalContextId":id(4),"groups":[],"tabs":[{"tabId":id(5),"label":"tab","pinned":false,"groupId":null,
        "root":{"kind":"panel","panelId":id(6),"primaryPresentation":{"customLabel":null,"presentationId":id(106),"label":"primary","binding":{"terminalId":id(206),"workerId":id(99)}},
        "childPresentations":[{"customLabel":null,"presentationId":id(306),"label":"child","binding":if unbound {Value::Null} else {json!({"terminalId":id(406),"workerId":id(99)})}}]}}]}]})).unwrap()
}

#[test]
fn exact_report_cli_preserves_primary_defaults_and_member_identity_through_unknown_outcomes() {
    let verbs = [
        "report-agent",
        "clear-agent-authority",
        "release-agent",
        "report-metadata",
    ];
    for (verb, case) in verbs
        .into_iter()
        .flat_map(|verb| [(verb, "primary"), (verb, "child")])
        .chain([
            ("report-agent", "wrong-reply"),
            ("report-metadata", "wrong-reply"),
            ("report-agent", "lost"),
            ("report-metadata", "lost"),
            ("report-agent", "missing"),
            ("report-metadata", "unbound"),
        ])
    {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let server = peer(&fixture, listener, move |mut stream| {
            let snapshot = snapshot(case == "unbound");
            let request = support::snapshot_after_metadata(&mut stream, &snapshot);
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    outcome: WorkbenchSnapshotOutcome::Available { snapshot },
                }),
            )
            .unwrap();
            if matches!(case, "missing" | "unbound") {
                assert_closed(&mut stream);
                return;
            }
            let request = read_workbench_request(&mut stream).unwrap();
            let encoded = serde_json::to_value(&request).unwrap();
            let action = &encoded["action"];
            let primary = case == "primary";
            assert_eq!(
                action["kind"],
                if primary {
                    "report"
                } else {
                    "report-presentation"
                }
            );
            assert_eq!(action["panelId"], id(6));
            assert_eq!(
                action["expectedBinding"]["terminalId"],
                id(if primary { 206 } else { 406 })
            );
            if !primary {
                assert_eq!(action["presentationId"], id(306));
            }
            if case == "lost" {
                return;
            }
            let mut outcome = json!({"kind":if primary {"reported"} else {"presentation-reported"},
                "workbenchRevision":"12","panelId":id(6),"binding":action["expectedBinding"],"disposition":"applied"});
            if !primary {
                outcome["presentationId"] =
                    json!(id(if case == "wrong-reply" { 999 } else { 306 }));
            }
            let response = match request {
                WorkbenchRequest::PanelAgent(request) => {
                    outcome["reportRevision"] = json!("2");
                    outcome["changed"] = json!(true);
                    outcome["view"] = json!(PanelAgentView::default());
                    WorkbenchResponse::PanelAgent(PanelAgentResult {
                        protocol_version: ProtocolVersion,
                        request_id: request.request_id,
                        site_id: request.site_id,
                        outcome: serde_json::from_value(outcome).unwrap(),
                    })
                }
                WorkbenchRequest::PanelMetadata(request) => {
                    outcome["metadataRevision"] = json!("2");
                    WorkbenchResponse::PanelMetadata(PanelMetadataResult {
                        protocol_version: ProtocolVersion,
                        request_id: request.request_id,
                        site_id: request.site_id,
                        outcome: serde_json::from_value(outcome).unwrap(),
                    })
                }
                _ => panic!("one report submission only"),
            };
            write_workbench_response(&mut stream, &response).unwrap();
            assert_closed(&mut stream);
        });
        let member = id(if case == "missing" { 999 } else { 306 });
        let mut args = vec!["panel", verb, "@panel:0x6", "--format=json"];
        if verb == "report-metadata" {
            args.extend(["--source=custom", "--title=child title"]);
        } else if verb != "clear-agent-authority" {
            args.extend(["--source=custom", "--agent=codex"]);
        }
        if verb == "report-agent" {
            args.push("--state=blocked");
        }
        if case != "primary" {
            args.extend(["--presentation", &member]);
        }
        let output = fixture.run_with_caller(&args, Some("ignored-invalid-caller"), None);
        server.join().unwrap();
        let doc: Value = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|error| panic!("{verb}/{case}: {error}: {output:?}"));
        if matches!(case, "missing" | "unbound") {
            assert!(!output.status.success());
            validate(
                &doc,
                include_str!("../src/cli/schemas/orchbee.cli.failure.json"),
            );
            continue;
        }
        let schema = if verb == "report-metadata" {
            include_str!("../src/cli/schemas/orchbee.panel.metadata.json")
        } else {
            include_str!("../src/cli/schemas/orchbee.panel.agent-report.json")
        };
        validate(&doc, schema);
        assert_eq!(doc["version"], 1);
        if case != "primary" {
            assert_eq!(doc["presentationId"], id(306));
            let validator =
                jsonschema::validator_for(&serde_json::from_str::<Value>(schema).unwrap()).unwrap();
            let mut invalid = doc.clone();
            invalid.as_object_mut().unwrap().remove("presentationId");
            assert!(!validator.is_valid(&invalid));
        }
        if matches!(case, "wrong-reply" | "lost") {
            assert_eq!(output.status.code(), Some(8));
            assert_eq!(doc["observation"]["kind"], "unknown");
        } else {
            assert!(output.status.success(), "{verb}/{case}: {output:?}");
            assert_eq!(
                doc["observation"]["reply"]["outcome"]["binding"]["terminalId"],
                id(if case == "primary" { 206 } else { 406 })
            );
        }
    }
}
