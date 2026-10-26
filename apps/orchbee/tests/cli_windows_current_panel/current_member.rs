//! Current member joins use exact identities through actual CLI processes and named pipes.
use super::*;
use orchbee_agent_observation_protocol::{
    AgentRequest, AgentResult, read_agent_request, write_agent_result,
};

fn snapshot(stream: &mut LocalStream, unbound: bool, revision: u64) {
    if !unbound {
        snapshot_reply_at(stream, revision);
        return;
    }
    let WorkbenchRequest::SnapshotRead(request) = read_workbench_request(stream).unwrap() else {
        panic!("snapshot")
    };
    let mut leaf = panel(6);
    leaf["primaryPresentation"]["binding"] = Value::Null;
    let source: Snapshot = serde_json::from_value(json!({"protocolVersion":1,"siteId":id(2),"revision":revision.to_string(),
        "navigation":{"projects":[], "directories":[], "workspaces":[{"workspaceId":id(4),"terminalContextId":id(4),"projectId":null,"primaryDirectoryId":null,"label":"Workspace"}]},
        "terminalContexts":[{"terminalContextId":id(4),"groups":[],"tabs":[{"tabId":id(5),"label":"tab","pinned":false,"groupId":null,"root":leaf}]}]})).unwrap();
    write_workbench_response(
        stream,
        &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            outcome: WorkbenchSnapshotOutcome::Available { snapshot: source },
        }),
    )
    .unwrap();
}
fn agents(stream: &mut LocalStream) {
    let AgentRequest::Read(request) = read_agent_request(stream).unwrap() else {
        panic!("Agent list")
    };
    assert_eq!(json!(request)["kind"], "agent.session.list.request");
    let summary = |n, presentation| {
        json!({"executionLocation":{"executionLocationId":"local"},"lifecycle":"live",
        "observation":{"fidelity":"rich","freshness":"fresh","generation":1,"sequence":2,"source":"native"},
        "parentSessionId":null,"pendingRequestCount":0,"presentationAttachments":[{"generation":"generation-1","navigable":true,"presentationId":id(presentation),"visibility":"visible"}],
        "providerId":"codex","sessionId":format!("session-{n}"),"status":{"kind":"succeeded"}})
    };
    write_agent_result(
        stream,
        &AgentResult::Read(
            serde_json::from_value(json!({"kind":"agent.session.list.result",
        "requestId":request.request_id(),"result":{"outcome":{"kind":"listed","revision":3,
            "items":[summary(1,106),summary(2,306)],"nextCursor":null}}}))
            .unwrap(),
        ),
    )
    .unwrap();
}
fn metadata(stream: &mut LocalStream, case: &str, primary: bool, unbound: bool, fence: bool) {
    let WorkbenchRequest::PanelMetadata(request) = read_workbench_request(stream).unwrap() else {
        panic!("metadata")
    };
    let pid = id(if primary { 106 } else { 306 });
    assert_eq!(
        json!(request.action),
        if fence {
            json!({"kind":"read","panelIds":[]})
        } else {
            json!({"kind":"read-presentations","presentationIds":[pid]})
        }
    );
    let binding = if unbound {
        Value::Null
    } else {
        json!({"terminalId":id(if primary || case=="wrong-metadata-binding" {206} else {406}),"workerId":"6f7263686265652d6c6f63616c2d7631"})
    };
    let rows = if fence {
        vec![]
    } else {
        vec![
            json!({"presentationId":pid,"panel":{"panelId":id(6),"binding":binding,
        "tokens":if unbound {json!({})} else {json!({"member":"selected"})},
        "presentations":if unbound {json!([])} else {json!([{"source":"child-display","agent":"codex","appliesToSource":"selected-hook",
            "title":{"value":"selected title","order":"1"},"displayAgent":null,"stateLabels":{}}])}}}),
        ]
    };
    let outcome = if fence {
        json!({"kind":"read","workbenchRevision":"12","metadataRevision":if case=="metadata-changed" {"6"} else {"5"},"panels":[]})
    } else {
        json!({"kind":"presentations-read","workbenchRevision":"12","metadataRevision":"5","presentations":rows})
    };
    write_workbench_response(
        stream,
        &WorkbenchResponse::PanelMetadata(PanelMetadataResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            site_id: request.site_id,
            outcome: serde_json::from_value(outcome).unwrap(),
        }),
    )
    .unwrap();
}
fn report(stream: &mut LocalStream, case: &str, primary: bool, unbound: bool, fence: bool) {
    let WorkbenchRequest::PanelAgent(request) = read_workbench_request(stream).unwrap() else {
        panic!("report")
    };
    let pid = id(if primary { 106 } else { 306 });
    assert_eq!(
        json!(request.action),
        if fence {
            json!({"kind":"read","panelIds":[]})
        } else {
            json!({"kind":"read-presentations","presentationIds":[pid]})
        }
    );
    let binding = if unbound {
        Value::Null
    } else {
        json!({"terminalId":id(if primary || case=="wrong-report-binding" {206} else {406}),"workerId":"6f7263686265652d6c6f63616c2d7631"})
    };
    let view = if unbound {
        Value::Null
    } else {
        json!({"agent":"codex","state":"blocked","authoritySource":"selected-hook","message":"selected message"})
    };
    let rows = if fence {
        vec![]
    } else {
        vec![
            json!({"presentationId":pid,"panel":{"panelId":id(6),"binding":binding,"report":view}}),
        ]
    };
    let outcome = if fence {
        json!({"kind":"read","workbenchRevision":"12","reportRevision":if case=="report-changed" {"6"} else {"5"},"panels":[]})
    } else {
        json!({"kind":"presentations-read","workbenchRevision":"12","reportRevision":"5","presentations":rows})
    };
    write_workbench_response(
        stream,
        &WorkbenchResponse::PanelAgent(PanelAgentResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            site_id: request.site_id,
            outcome: serde_json::from_value(outcome).unwrap(),
        }),
    )
    .unwrap();
}

#[test]
fn client_empty_current_is_observable_and_never_queries_another_terminal() {
    for case in [
        "stable",
        "away-and-back",
        "wrong-member",
        "bound-as-empty",
        "process-info",
    ] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let server = peer(&fixture, listener, move |mut stream| {
            snapshot(&mut stream, case != "bound-as-empty", 12);
            let mut selected = record(10, 6);
            if case != "bound-as-empty" {
                selected["view"]["selection"]["panelCount"] = json!(1);
            }
            list_reply(&mut stream, vec![selected]);
            presentations::read(&mut stream, 10, 6, case == "wrong-member", 7, "unbound");
            if matches!(case, "stable" | "away-and-back") {
                agents(&mut stream);
                metadata(&mut stream, "stable", true, true, false);
                report(&mut stream, "stable", true, true, false);
                metadata(&mut stream, "stable", true, true, true);
                report(&mut stream, "stable", true, true, true);
                snapshot(&mut stream, true, 12);
                presentations::read(
                    &mut stream,
                    10,
                    6,
                    false,
                    if case == "away-and-back" { 8 } else { 7 },
                    if case == "bound-as-empty" {
                        "unbound-two-panels"
                    } else {
                        "unbound"
                    },
                );
            }
            // Any Host attach, process read, focus or fallback fails this exact script.
            assert_closed(&mut stream);
        });
        let output = fixture.run(&[
            "panel",
            if case == "process-info" {
                "process-info"
            } else {
                "current"
            },
            "--format=json",
        ]);
        server.join().unwrap();
        let doc = document(&output);
        if case == "stable" {
            assert!(output.status.success(), "{output:?}");
            assert_eq!(doc["version"], 1);
            let row = &doc["result"]["panel"];
            assert_eq!(row["currentPresentation"]["presentationId"], id(106));
            assert_eq!(row["currentPresentation"]["binding"], Value::Null);
            assert_eq!(
                row["currentPresentation"]["source"],
                json!({"kind":"client-unbound","selectionGeneration":"7"})
            );
            assert_eq!(
                row["scrollObservation"],
                json!({"state":"unavailable","reason":"not-attached"})
            );
            assert_eq!(
                doc["result"]["client"]["operationIds"]
                    .as_array()
                    .unwrap()
                    .len(),
                2
            );
            let schema: Value = serde_json::from_str(include_str!(
                "../../src/cli/schemas/orchbee.panel.current.json"
            ))
            .unwrap();
            let validator = jsonschema::validator_for(&schema).unwrap();
            validator.validate(&doc).unwrap();
        } else {
            assert!(!output.status.success(), "{case}: {output:?}");
            assert!(doc.get("result").is_none());
            if case == "away-and-back" {
                assert_eq!(doc["failure"]["details"]["reason"], "client-changed");
            }
        }
    }
}
#[test]
fn current_joins_selected_terminal_agent_report_and_metadata_without_rewriting_primary() {
    for case in [
        "child",
        "primary",
        "unbound",
        "wrong-metadata-binding",
        "wrong-report-binding",
        "metadata-changed",
        "report-changed",
        "workbench-changed",
    ] {
        let primary = matches!(case, "primary" | "unbound");
        let unbound = case == "unbound";
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let server = peer(&fixture, listener, move |mut stream| {
            snapshot(&mut stream, unbound, 12);
            agents(&mut stream);
            if !unbound {
                terminal_metadata_peer::sample_for(
                    &mut stream,
                    "stable",
                    "codex.exe",
                    Some(&id(if primary { 206 } else { 406 })),
                );
            }
            metadata(&mut stream, case, primary, unbound, false);
            if case == "wrong-metadata-binding" {
                assert_closed(&mut stream);
                return;
            }
            report(&mut stream, case, primary, unbound, false);
            if case == "wrong-report-binding" {
                assert_closed(&mut stream);
                return;
            }
            metadata(&mut stream, case, primary, unbound, true);
            if case == "metadata-changed" {
                assert_closed(&mut stream);
                return;
            }
            report(&mut stream, case, primary, unbound, true);
            if case == "report-changed" {
                assert_closed(&mut stream);
                return;
            }
            snapshot(
                &mut stream,
                unbound,
                if case == "workbench-changed" { 13 } else { 12 },
            );
            assert_closed(&mut stream);
        });
        let panel = id(6);
        let terminal = id(406);
        let mut args = vec!["panel", "current", "--format=json"];
        if primary {
            args.push(&panel);
        }
        let output = fixture.run_with_caller(
            &args,
            Some(&terminal),
            Some("6f7263686265652d6c6f63616c2d7631"),
        );
        server.join().unwrap();
        let doc = document(&output);
        if matches!(case, "child" | "primary" | "unbound") {
            assert!(output.status.success(), "{case}: {output:?}");
            validate(
                &doc,
                include_str!("../../src/cli/schemas/orchbee.panel.current.json"),
            );
            assert_eq!(doc["version"], 1);
            let row = &doc["result"]["panel"];
            assert_eq!(row["primaryPresentation"]["presentationId"], id(106));
            assert_eq!(row["childPresentations"][0]["presentationId"], id(306));
            assert_eq!(
                row["currentPresentation"]["presentationId"],
                id(if primary { 106 } else { 306 })
            );
            assert_eq!(
                row["currentPresentation"]["source"]["kind"],
                if primary { "primary" } else { "caller" }
            );
            assert_eq!(row["scrollObservation"]["state"], "no-client");
            assert!(doc["result"]["client"].is_null());
            if unbound {
                assert_eq!(row["terminalObservation"]["state"], "unbound");
                assert!(row["metadata"]["binding"].is_null());
                assert!(row["reportedAgent"]["view"].is_null());
                assert_eq!(row["agentSessionIds"], json!([]));
            } else {
                assert_eq!(row["terminalTitle"], "Orchbee 🐝");
                assert_eq!(row["cwd"], r"C:\frame 项目");
                assert_eq!(
                    row["agentSessionIds"],
                    json!([if primary { "session-1" } else { "session-2" }])
                );
                assert_eq!(
                    doc["result"]["agents"]["sessions"]
                        .as_array()
                        .unwrap()
                        .len(),
                    1
                );
                assert_eq!(
                    row["metadata"]["binding"]["terminalId"],
                    id(if primary { 206 } else { 406 })
                );
                assert_eq!(row["metadata"]["presentation"]["title"], "selected title");
                assert_eq!(row["reportedAgent"]["view"]["message"], "selected message");
            }
            let schema: Value = serde_json::from_str(include_str!(
                "../../src/cli/schemas/orchbee.panel.current.json"
            ))
            .unwrap();
            let validator = jsonschema::draft202012::new(&schema).unwrap();
            let mut missing = doc.clone();
            missing["result"]["panel"]
                .as_object_mut()
                .unwrap()
                .remove("currentPresentation");
            assert!(!validator.is_valid(&missing));
        } else {
            assert!(!output.status.success(), "{case}: {output:?}");
            assert!(doc.get("result").is_none());
            if case.starts_with("wrong-") {
                validate(
                    &doc,
                    include_str!("../../src/cli/schemas/orchbee.cli.failure.json"),
                );
            } else {
                validate(
                    &doc,
                    include_str!("../../src/cli/schemas/orchbee.cli.failure.workbench-query.json"),
                );
            }
        }
    }
}
