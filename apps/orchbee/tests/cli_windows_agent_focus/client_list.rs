//! Batch joins preserve independent seen states across canonical page races.
use super::*;
use orchbee_agent_observation_protocol::{AgentResult, write_agent_result};

fn summary(id: &str) -> Value {
    let detail = client_get::completed();
    let mut summary = json!({});
    for key in [
        "executionLocation",
        "lifecycle",
        "observation",
        "parentSessionId",
        "presentationAttachments",
        "providerId",
    ] {
        summary[key] = detail["snapshot"][key].clone();
    }
    summary["sessionId"] = json!(id);
    summary["pendingRequestCount"] = json!(0);
    summary["status"] = detail["status"].clone();
    summary
}
fn listed_reply(stream: &mut LocalStream, outcome: Value) {
    let AgentRequest::Read(request) = read_agent_request(stream).unwrap() else {
        panic!("list read")
    };
    assert_eq!(json!(request)["kind"], "agent.session.list.request");
    assert_eq!(json!(request)["limit"], 2);
    assert_eq!(json!(request)["includeEnded"], false);
    assert!(json!(request)["cursor"].is_null());
    write_agent_result(stream,&AgentResult::Read(serde_json::from_value(json!({"kind":"agent.session.list.result","requestId":request.request_id(),"result":{"outcome":outcome}})).unwrap())).unwrap();
}
fn page(stream: &mut LocalStream, revision: u64, names: &[&str]) {
    listed_reply(
        stream,
        json!({"kind":"listed","revision":revision,"nextCursor":null,
        "items":names.iter().map(|id|summary(id)).collect::<Vec<_>>()}),
    );
}
fn batch(stream: &mut LocalStream, allowed: bool) {
    let view = client_get::selected_reply(stream, false);
    let request = view_request(stream);
    let ClientViewAction::Submit { command } = &request.action else {
        panic!("one attention batch")
    };
    assert_eq!(
        json!(command.intent),
        json!({"kind":"agent-attention","sessionIds":["sess-alpha","sess-beta"]})
    );
    let entries = ["sess-alpha", "sess-beta"]
        .iter()
        .enumerate()
        .map(|(i, id)| {
            json!({"sessionId":id,
        "state":{"generation":3,"sequence":11,"stateChangeRevision":3,"seen":i==0}})
        })
        .collect::<Vec<_>>();
    view_reply(
        stream,
        &request,
        json!({"kind":"operation","operation":{"command":command,"status":{
            "kind":"attention-read","view":view,"attention":{"revision":3,"cursor":attention::token(),"outerFocused":true,"entries":entries}
        }}}),
    );
    attention::authenticate(stream, allowed);
}
#[test]
fn selected_client_lists_batch_once_and_preserves_truth_across_page_changes() {
    for case in [
        "stable",
        "changed",
        "membership",
        "emptied",
        "empty",
        "foreign",
        "stale-cursor",
    ] {
        let fixture = FocusFixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let server = peer(&fixture, listener, move |mut stream| {
            snapshot_reply(&mut stream);
            page(
                &mut stream,
                3,
                if case == "empty" {
                    &[]
                } else {
                    &["sess-alpha", "sess-beta"]
                },
            );
            if case == "empty" {
                client_get::selected_reply(&mut stream, false);
            } else {
                batch(&mut stream, case != "foreign");
                if case == "stale-cursor" {
                    listed_reply(
                        &mut stream,
                        json!({"kind":"rejected","reason":"cursor-stale"}),
                    );
                } else if case != "foreign" {
                    let newer = matches!(case, "changed" | "membership");
                    let names: &[&str] = match case {
                        "membership" => &["sess-alpha", "sess-new"],
                        "emptied" => &[],
                        _ => &["sess-alpha", "sess-beta"],
                    };
                    page(&mut stream, if newer { 4 } else { 3 }, names);
                    if newer {
                        let AgentRequest::Changes(request) =
                            read_agent_request(&mut stream).unwrap()
                        else {
                            panic!("one shared continuity read")
                        };
                        let target = if case == "changed" {
                            "sess-alpha"
                        } else {
                            "sess-new"
                        };
                        let change = json!({"revision":4,"transition":"presentation-visibility-changed","session":{
                            "sessionId":target,"providerId":"claude-code","parentSessionId":null,"pendingRequestCount":0,"lifecycle":"live",
                            "observation":{"generation":3,"sequence":11,"freshness":"fresh","fidelity":"rich","source":"native"},"status":{"kind":"succeeded"}}});
                        write_agent_result(&mut stream,&AgentResult::Changes(serde_json::from_value(json!({"kind":"agent.session.changes.result","requestId":request.request_id(),"result":{"outcome":{
                            "kind":"changes","cursor":attention::token(),"observedThroughRevision":4,"runtimeRevision":4,"hasMore":false,"changes":[change]
                        }}})).unwrap())).unwrap();
                    }
                }
            }
            assert_closed(&mut stream);
        });
        let output = fixture.run(&[
            "agent",
            "list",
            "--client",
            &id(7),
            "--limit",
            "2",
            "--format=json",
        ]);
        server.join().unwrap();
        let doc = document(&output);
        if matches!(case, "foreign" | "stale-cursor") {
            assert!(!output.status.success(), "{output:?}");
            validate(
                &doc,
                include_str!("../../src/cli/schemas/orchbee.cli.failure.agent.json"),
            );
            assert!(doc.get("result").is_none());
            continue;
        }
        assert!(output.status.success(), "{output:?}");
        validate(
            &doc,
            include_str!("../../src/cli/schemas/orchbee.agent.list.json"),
        );
        assert_eq!(doc["version"], 2);
        assert_eq!(doc["result"]["clientId"], id(7));
        let items = doc["result"]["items"].as_array().unwrap();
        if matches!(case, "empty" | "emptied") {
            assert!(items.is_empty());
            continue;
        }
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["status"]["kind"], "succeeded");
        assert_eq!(
            items[0]["client"]["seen"],
            if case == "changed" {
                Value::Null
            } else {
                json!(true)
            }
        );
        assert_eq!(
            items[0]["client"]["state"],
            if case == "changed" {
                "stale"
            } else {
                "current"
            }
        );
        assert_eq!(
            items[1]["client"]["seen"],
            if case == "membership" {
                Value::Null
            } else {
                json!(false)
            }
        );
        assert_eq!(
            items[1]["client"]["state"],
            if case == "membership" {
                "missing"
            } else {
                "current"
            }
        );
    }
}
