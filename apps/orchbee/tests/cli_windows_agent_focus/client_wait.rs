//! Client-relative idle/done do not reinterpret canonical success or acknowledge it.
use super::*;
use orchbee_agent_observation_protocol::{AgentResult, write_agent_result};

fn changes_reply(stream: &mut LocalStream, outcome: Value, limit: u64) {
    let AgentRequest::Changes(request) = read_agent_request(stream).unwrap() else {
        panic!("changes read")
    };
    assert_eq!(json!(request)["limit"], limit);
    write_agent_result(stream,&AgentResult::Changes(serde_json::from_value(json!({
        "kind":"agent.session.changes.result","requestId":request.request_id(),"result":{"outcome":outcome}
    })).unwrap())).unwrap();
}

fn checkpoint(stream: &mut LocalStream, revision: u64, limit: u64) {
    changes_reply(
        stream,
        json!({"kind":"current","revision":revision,"cursor":attention::token()}),
        limit,
    );
}

fn sample(stream: &mut LocalStream, revision: u64, sequence: u64, seen: Option<bool>) {
    let view = client_get::selected_reply(stream, false);
    let request = view_request(stream);
    let ClientViewAction::Submit { command } = &request.action else {
        panic!("read-only attention")
    };
    assert_eq!(
        json!(command.intent),
        json!({"kind":"agent-attention","sessionIds":["sess-alpha"]})
    );
    let state = seen.map(|seen| json!({"generation":3,"sequence":sequence,"stateChangeRevision":revision,"seen":seen}));
    view_reply(
        stream,
        &request,
        json!({"kind":"operation","operation":{"command":command,"status":{
            "kind":"attention-read","view":view,"attention":{"cursor":attention::token(),"revision":revision,"outerFocused":false,"entries":[{"sessionId":"sess-alpha","state":state}]}
        }}}),
    );
    checkpoint(stream, revision, 1);
    let mut canonical = client_get::completed();
    canonical["snapshot"]["observation"]["sequence"] = json!(sequence);
    read_reply(
        stream,
        "get",
        json!({"kind":"found","revision":revision,"session":canonical}),
    );
}

#[test]
fn selected_wait_matches_seen_idle_unseen_done_and_waits_for_missing_observation() {
    for mode in ["idle", "done", "missing"] {
        let fixture = FocusFixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let server = peer(&fixture, listener, move |mut stream| {
            snapshot_reply(&mut stream);
            checkpoint(&mut stream, 3, 100);
            sample(
                &mut stream,
                3,
                11,
                if mode == "missing" {
                    None
                } else {
                    Some(mode == "done")
                },
            );
            if mode == "done" {
                let event = |revision, sequence, status| {
                    json!({"revision":revision,"transition":"turn-activity-changed","session":{
                        "sessionId":"sess-alpha","providerId":"acme.agent","parentSessionId":null,"pendingRequestCount":0,"lifecycle":"live",
                        "observation":{"generation":3,"sequence":sequence,"freshness":"fresh","fidelity":"rich","source":"native"},"status":status
                    }})
                };
                changes_reply(
                    &mut stream,
                    json!({"kind":"changes","cursor":attention::token(),"observedThroughRevision":5,"runtimeRevision":5,"hasMore":false,"changes":[
                        event(4,12,json!({"kind":"working","activity":null,"phase":"working"})),event(5,13,json!({"kind":"succeeded"}))
                    ]}),
                    100,
                );
                sample(&mut stream, 5, 13, Some(false));
            } else {
                checkpoint(&mut stream, 3, 100);
                sample(&mut stream, 3, 11, Some(mode == "idle"));
            }
            assert_closed(&mut stream);
        });
        let condition = if mode == "idle" { "idle" } else { "done" };
        let output = fixture.run(&[
            "agent",
            "wait",
            "--session",
            "sess-alpha",
            "--client",
            &id(7),
            "--until",
            condition,
            "--timeout",
            "6000",
            "--format=json",
        ]);
        server.join().unwrap();
        assert!(output.status.success(), "{output:?}");
        let doc = document(&output);
        validate(
            &doc,
            include_str!("../../src/cli/schemas/orchbee.agent.wait.json"),
        );
        assert_eq!(doc["version"], 4);
        assert_eq!(doc["result"]["wait"]["observations"], 2);
        assert_eq!(doc["result"]["client"]["status"], condition);
        assert_eq!(doc["result"]["client"]["clientId"], id(7));
        assert_eq!(doc["result"]["session"]["status"]["kind"], "succeeded");
        assert!(doc["result"]["matchedChange"].is_null());
    }
}

#[test]
fn selected_wait_client_read_shares_the_total_deadline_and_missing_client_does_not_fallback() {
    for missing in [false, true] {
        let fixture = FocusFixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let server = peer(&fixture, listener, move |mut stream| {
            snapshot_reply(&mut stream);
            checkpoint(&mut stream, 3, 100);
            let request = view_request(&mut stream);
            assert!(matches!(request.action, ClientViewAction::Get { .. }));
            if missing {
                view_reply(&mut stream, &request, json!({"kind":"not-found"}));
            }
            assert_closed(&mut stream);
        });
        let output = fixture.run(&[
            "agent",
            "wait",
            "--session",
            "sess-alpha",
            "--client",
            &id(7),
            "--until",
            "done",
            "--timeout",
            "3000",
            "--format=json",
        ]);
        server.join().unwrap();
        assert!(!output.status.success());
        let doc = document(&output);
        validate(
            &doc,
            if missing {
                include_str!("../../src/cli/schemas/orchbee.cli.failure.client-control.json")
            } else {
                include_str!("../../src/cli/schemas/orchbee.cli.failure.agent.json")
            },
        );
        if missing {
            assert_eq!(doc["failure"]["kind"], "not-found");
        } else {
            assert_eq!(output.status.code(), Some(7));
            let reason = doc["failure"]["details"]["reason"].as_str().unwrap();
            assert!(matches!(reason, "wait-deadline" | "read-deadline"));
            if reason == "read-deadline" {
                assert!(doc["failure"]["details"]["readBudgetMs"].as_u64().unwrap() <= 2000);
            }
        }
    }
}
