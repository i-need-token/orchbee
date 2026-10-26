//! Batch joins preserve independent seen states across canonical continuity changes.
use super::*;
use orchbee_agent_observation_protocol::{
    AgentRequest, AgentResult, read_agent_request, write_agent_result,
};

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
fn change_summary(id: &str) -> Value {
    let mut changed = summary(id);
    let fields = changed.as_object_mut().unwrap();
    fields.remove("executionLocation");
    fields.remove("presentationAttachments");
    changed
}
fn batch(stream: &mut LocalStream, changed: Option<&str>, allowed: bool) {
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
    if !allowed {
        return;
    }
    if let Some(changed) = changed {
        let AgentRequest::Changes(request) = read_agent_request(stream).unwrap() else {
            panic!("one canonical continuity read")
        };
        assert_eq!(json!(request)["cursor"], attention::token());
        assert_eq!(json!(request)["limit"], 100);
        let outcome = json!({"kind":"changes","cursor":attention::token(),"observedThroughRevision":4,
        "runtimeRevision":4,"hasMore":false,"changes":[{
            "revision":4,"transition":"presentation-visibility-changed",
            "session":change_summary(changed)
        }]});
        write_agent_result(stream,&AgentResult::Changes(serde_json::from_value(json!({"kind":"agent.session.changes.result","requestId":request.request_id(),"result":{"outcome":outcome}})).unwrap())).unwrap();
    }
}
#[test]
fn selected_client_lists_batch_once_and_preserves_truth_across_canonical_changes() {
    for case in ["stable", "changed", "other-change", "empty", "foreign"] {
        let fixture = FocusFixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let server = peer(&fixture, listener, move |mut stream| {
            snapshot_reply(&mut stream);
            let revision = if matches!(case, "changed" | "other-change" | "foreign") {
                4
            } else {
                3
            };
            let summaries = if case == "empty" {
                Vec::new()
            } else {
                vec![summary("sess-alpha"), summary("sess-beta")]
            };
            let projection = AgentProjectionFixture::summaries(snapshot(12), revision, summaries);
            projection.read(&mut stream, false);
            client_get::selected_reply(&mut stream, false);
            if case == "empty" {
                client_get::selected_reply(&mut stream, false);
            } else {
                let changed = match case {
                    "changed" => Some("sess-alpha"),
                    "other-change" | "foreign" => Some("sess-new"),
                    _ => None,
                };
                batch(&mut stream, changed, case != "foreign");
                if case != "foreign" {
                    client_get::selected_reply(&mut stream, false);
                }
            }
            if case != "foreign" {
                projection.confirm(&mut stream, false);
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
        if case == "foreign" {
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
        assert_eq!(doc["version"], 1);
        assert_eq!(doc["source"]["kind"], "agent-projection");
        assert_eq!(doc["result"]["clientId"], id(7));
        let items = doc["result"]["items"].as_array().unwrap();
        if case == "empty" {
            assert!(items.is_empty());
            continue;
        }
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["ordinal"], 1);
        assert_eq!(items[0]["status"], "done");
        assert_eq!(items[0]["source"], "canonical");
        assert!(items[0]["client"]["terminal"].is_null());
        assert_eq!(
            items[0]["client"]["canonical"]["seen"],
            if case == "changed" {
                Value::Null
            } else {
                json!(true)
            }
        );
        assert_eq!(
            items[0]["client"]["canonical"]["state"],
            if case == "changed" {
                "stale"
            } else {
                "current"
            }
        );
        assert_eq!(items[1]["client"]["canonical"]["seen"], json!(false));
        assert_eq!(items[1]["client"]["canonical"]["state"], "current");
    }
}

#[test]
fn unified_list_keeps_matched_aliases_and_physical_only_agents_as_distinct_rows() {
    let fixture = FocusFixture::new();
    let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
    let server = peer(&fixture, listener, move |mut stream| {
        snapshot_reply(&mut stream);
        let mut canonical = client_get::completed();
        canonical["snapshot"]["providerId"] = json!("grok");
        canonical["snapshot"]["presentationAttachments"] = json!([{
            "generation":id(1),"navigable":true,"presentationId":id(106),"visibility":"visible"
        }]);
        let projection = AgentProjectionFixture::canonical(snapshot(12), 5, &[canonical])
            .with_state(
                &id(106),
                Some(peer_support::terminal_agent_state(
                    &id(106),
                    &id(1),
                    &id(2),
                    &id(206),
                    Some("grok-build"),
                    "idle",
                )),
            )
            .with_state(
                &id(108),
                Some(peer_support::terminal_agent_state(
                    &id(108),
                    &id(1),
                    &id(2),
                    &id(208),
                    Some("pi"),
                    "working",
                )),
            );
        projection.read(&mut stream, false);
        super::metadata_peer::unavailable(&mut stream);
        super::metadata_peer::unavailable(&mut stream);
        projection.confirm(&mut stream, false);
        assert_closed(&mut stream);
    });
    let output = fixture.run(&["agent", "list", "--format=json"]);
    server.join().unwrap();
    assert!(output.status.success(), "{output:?}");
    let doc = document(&output);
    validate(
        &doc,
        include_str!("../../src/cli/schemas/orchbee.agent.list.json"),
    );
    assert_eq!(doc["version"], 1);
    assert_eq!(doc["result"]["items"][0]["ordinal"], 1);
    assert_eq!(doc["result"]["items"][0]["provider"], "grok");
    assert_eq!(doc["result"]["items"][0]["source"], "both");
    assert_eq!(doc["result"]["items"][0]["status"], "done");
    assert!(doc["result"]["items"][0]["cwd"].is_null());
    assert_eq!(doc["result"]["items"][1]["ordinal"], 2);
    assert_eq!(doc["result"]["items"][1]["provider"], "pi");
    assert_eq!(doc["result"]["items"][1]["source"], "terminal");
    assert!(doc["result"]["items"][1]["canonical"].is_null());
}
