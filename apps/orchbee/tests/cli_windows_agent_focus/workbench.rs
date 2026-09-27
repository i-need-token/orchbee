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

fn selections(stream: &mut LocalStream) {
    let view = client_get::selected_reply(stream, false);
    let request = view_request(stream);
    let ClientViewAction::Submit { command } = &request.action else {
        panic!("read selections")
    };
    assert_eq!(
        json!(command.intent),
        json!({"kind":"selections","terminalContextIds":[id(4)]})
    );
    view_reply(
        stream,
        &request,
        json!({"kind":"operation","operation":{"command":command,"status":{"kind":"navigation-read","view":view,"navigation":{"workbenchRevision":"12","contexts":[{"terminalContextId":id(4),"activeTabId":id(5)}]}}}}),
    );
}

#[test]
fn workbench_queries_join_only_primary_agents_and_reject_changed_client_without_partial_output() {
    for case in ["headless", "seen", "unseen", "stale", "changed-view"] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let server = peer(&fixture, listener, move |mut stream| {
            snapshot_reply(&mut stream);
            if case != "headless" {
                selections(&mut stream);
            }
            page(&mut stream, 3);
            if case != "headless" {
                let view = client_get::selected_reply(&mut stream, false);
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
                client_get::selected_reply(&mut stream, false);
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
                "unseen" => json!("done"),
                _ => Value::Null,
            }
        );
        assert_eq!(doc["result"]["panels"][1]["agentStatus"], "unknown");
        assert_eq!(
            doc["result"]["agents"]["requestIds"]
                .as_array()
                .unwrap()
                .len(),
            if case == "headless" { 1 } else { 2 }
        );
    }
}
