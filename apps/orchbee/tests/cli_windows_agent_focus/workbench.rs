//! Workbench query composition uses the same admitted pipe without state-changing controls.
use super::*;
use orchbee_agent_observation_protocol::{AgentResult, write_agent_result};
#[path = "workbench/runtime.rs"]
mod runtime;

fn page(stream: &mut LocalStream, revision: u64) {
    let AgentRequest::Read(request) = read_agent_request(stream).unwrap() else {
        panic!("Agent list")
    };
    assert_eq!(json!(request)["kind"], "agent.session.list.request");
    assert_eq!(json!(request)["limit"], 100);
    assert_eq!(json!(request)["includeEnded"], false);
    assert!(json!(request)["cursor"].is_null());
    let items=[("primary",106), ("child",306)].map(|(name,presentation)| {
        let detail=client_get::completed();
        let mut item=json!({"sessionId":name,"pendingRequestCount":0,"status":detail["status"]});
        for key in ["executionLocation","lifecycle","observation","parentSessionId","providerId"] {item[key]=detail["snapshot"][key].clone();}
        item["presentationAttachments"]=json!([{"generation":"gen-2","navigable":true,"presentationId":id(presentation),"visibility":"visible"}]);
        item
    });
    write_agent_result(stream,&AgentResult::Read(serde_json::from_value(json!({"kind":"agent.session.list.result","requestId":request.request_id(),"result":{"outcome":{"kind":"listed","revision":revision,"items":items,"nextCursor":null}}})).unwrap())).unwrap();
}

fn viewport(stream: &mut LocalStream, case: &str) {
    let request = view_request(stream);
    let ClientViewAction::Submit { command } = &request.action else {
        panic!("viewport read")
    };
    assert_eq!(
        json!(command.intent),
        json!({"kind":"viewports","panelIds":[id(6),id(8)]})
    );
    if case == "viewport-lost" {
        return;
    }
    if case == "viewport-timeout" {
        let command = command.clone();
        view_reply(
            stream,
            &request,
            json!({"kind":"operation","operation":{"command":command,"status":{"kind":"queued"}}}),
        );
        while let Ok(WorkbenchRequest::ClientView(next)) = read_workbench_request(stream) {
            assert!(
                matches!(&next.action, ClientViewAction::Read { operation_id } if *operation_id == command.operation_id)
            );
            view_reply(
                stream,
                &next,
                json!({"kind":"operation","operation":{"command":command,"status":{"kind":"queued"}}}),
            );
        }
        return;
    }
    let mut view = record(7, 6)["view"].clone();
    if case == "busy" {
        view["phase"] = json!("busy");
    }
    let primary = if case == "wrong-binding" { 207 } else { 206 };
    let panels = json!([
        {"panelId":id(6),"primaryTerminalId":id(primary),"observation":{"state":"observed","mode":"history","source":{"hostEpoch":id(50),"attachmentId":id(51),"generation":"1","revision":"5","columns":80,"alternateScreen":false,"totalRows":"400","startRow":"100"},"scroll":{"offsetFromBottom":"276","maxOffsetFromBottom":"376","viewportRows":24}}},
        {"panelId":id(8),"primaryTerminalId":id(208),"observation":{"state":"unavailable","reason":"not-attached"}}
    ]);
    view_reply(
        stream,
        &request,
        json!({"kind":"operation","operation":{"command":command,"status":{"kind":"viewport-read","view":view,"viewport":{"workbenchRevision":"12","panels":panels}}}}),
    );
}

#[test]
fn workbench_queries_join_only_primary_agents_and_reject_changed_client_without_partial_output() {
    for case in [
        "headless",
        "seen",
        "unseen",
        "stale",
        "changed-view",
        "busy",
        "wrong-binding",
        "viewport-timeout",
        "viewport-lost",
    ] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let server = peer(&fixture, listener, move |mut stream| {
            snapshot_reply(&mut stream);
            if case != "headless" {
                client_get::selected_reply(&mut stream, case == "busy");
            }
            page(&mut stream, 3);
            if case != "headless" {
                let view = client_get::selected_reply(&mut stream, case == "busy");
                let request = view_request(&mut stream);
                let ClientViewAction::Submit { command } = &request.action else {
                    panic!("read attention")
                };
                assert_eq!(
                    json!(command.intent),
                    json!({"kind":"agent-attention","sessionIds":["primary"]})
                );
                let generation = if case == "stale" { 2 } else { 3 };
                view_reply(
                    &mut stream,
                    &request,
                    json!({"kind":"operation","operation":{"command":command,"status":{"kind":"attention-read","view":view,"attention":{"revision":3,"cursor":attention::token(),"outerFocused":false,"entries":[{"sessionId":"primary","state":{"generation":generation,"sequence":11,"stateChangeRevision":3,"seen":case=="seen"}}]}}}}),
                );
                attention::authenticate(&mut stream, true);
                page(&mut stream, 3);
            }
            for _ in 0..2 {
                runtime::unavailable(&mut stream);
            }
            if case != "headless" {
                viewport(&mut stream, case);
            }
            if matches!(case, "wrong-binding" | "viewport-timeout" | "viewport-lost") {
                assert_closed(&mut stream);
                return;
            }
            snapshot_reply(&mut stream);
            if case == "changed-view" {
                let request = view_request(&mut stream);
                let mut client = record(7, 8);
                client["viewRevision"] = json!("3");
                view_reply(
                    &mut stream,
                    &request,
                    json!({"kind":"found","client":client}),
                );
            } else if case != "headless" {
                client_get::selected_reply(&mut stream, case == "busy");
            }
            assert_closed(&mut stream);
        });
        let client = id(7);
        let args = if case == "headless" {
            vec!["panel", "list", "--format=json"]
        } else {
            vec!["panel", "list", "--client", &client, "--format=json"]
        };
        let output = fixture.run(&args);
        server.join().unwrap();
        let doc = document(&output);
        if matches!(case, "wrong-binding" | "viewport-timeout" | "viewport-lost") {
            assert_eq!(
                output.status.code(),
                Some(if case == "viewport-timeout" { 7 } else { 5 })
            );
            assert!(doc.get("result").is_none());
            continue;
        }
        if case == "changed-view" {
            assert_eq!(output.status.code(), Some(3));
            validate(
                &doc,
                include_str!("../../src/cli/schemas/orchbee.cli.failure.workbench-query.json"),
            );
            assert_eq!(doc["failure"]["details"]["reason"], "client-changed");
            assert!(doc.get("result").is_none());
            continue;
        }
        assert!(output.status.success(), "{output:?}");
        validate(
            &doc,
            include_str!("../../src/cli/schemas/orchbee.panel.list.json"),
        );
        assert_eq!(
            doc["result"]["agents"]["sessions"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            doc["result"]["panels"][0]["agentSessionIds"],
            json!(["primary"])
        );
        assert_eq!(
            doc["result"]["panels"][0]["agentStatus"],
            match case {
                "seen" => json!("idle"),
                "unseen" | "busy" => json!("done"),
                _ => Value::Null,
            }
        );
        assert_eq!(doc["result"]["panels"][1]["agentStatus"], "unknown");
        let row = &doc["result"]["panels"][0];
        if case == "headless" {
            assert!(row["scroll"].is_null());
            assert_eq!(row["scrollObservation"]["state"], "no-client");
        } else {
            assert_eq!(row["scroll"]["offsetFromBottom"], "276");
            assert_eq!(
                doc["result"]["client"]["operationIds"]
                    .as_array()
                    .unwrap()
                    .len(),
                1
            );
        }
        assert_eq!(
            doc["result"]["agents"]["requestIds"]
                .as_array()
                .unwrap()
                .len(),
            if case == "headless" { 1 } else { 2 }
        );
    }
}
