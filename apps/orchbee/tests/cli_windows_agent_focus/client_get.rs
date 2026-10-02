//! Canonical details and selected-Client status stay separate across races.
use super::*;
use crate::metadata_peer;
use orchbee_agent_observation_protocol::{AgentResult, write_agent_result};

pub(super) fn selected_reply(stream: &mut LocalStream, busy: bool) -> Value {
    let request = view_request(stream);
    assert!(
        matches!(&request.action, ClientViewAction::Get { client_id } if client_id.as_str()==id(7))
    );
    let mut client = record(7, 6);
    if busy {
        client["view"]["phase"] = json!("busy");
    }
    view_reply(stream, &request, json!({"kind":"found","client":client}));
    client["view"].clone()
}

fn attention_reply(
    stream: &mut LocalStream,
    view: Value,
    state: Value,
    outer: Value,
    allowed: bool,
) {
    let request = view_request(stream);
    let ClientViewAction::Submit { command } = &request.action else {
        panic!("attention read")
    };
    assert_eq!(
        json!(command.intent),
        json!({"kind":"agent-attention","sessionIds":["sess-alpha"]})
    );
    let mut observed = attention::observation(state);
    observed["outerFocused"] = outer;
    view_reply(
        stream,
        &request,
        json!({"kind":"operation","operation":{"command":command,"status":{"kind":"attention-read","attention":observed,"view":view}}}),
    );
    attention::authenticate(stream, allowed);
}

pub(super) fn completed() -> Value {
    let mut agent = detail(&[106]);
    agent["snapshot"]["observation"]["generation"] = json!(3);
    agent["snapshot"]["observation"]["sequence"] = json!(11);
    agent["snapshot"]["pendingRequests"] = json!([]);
    agent["snapshot"]["turn"] = json!({"kind":"idle"});
    agent["snapshot"]["lastOutcome"] =
        json!({"kind":"succeeded","eventId":"finished","turnId":"turn-9"});
    agent["status"] = json!({"kind":"succeeded"});
    agent
}

#[test]
fn selected_client_get_joins_only_current_versions_and_keeps_canonical_outcome() {
    for (state, outer, busy, expected_state, seen, status, focused) in [
        (
            Value::Null,
            json!(true),
            false,
            "missing",
            Value::Null,
            Value::Null,
            true,
        ),
        (
            json!({"generation":3,"sequence":11,"stateChangeRevision":3,"seen":true}),
            json!(true),
            false,
            "current",
            json!(true),
            json!("idle"),
            true,
        ),
        (
            json!({"generation":3,"sequence":11,"stateChangeRevision":3,"seen":false}),
            json!(false),
            false,
            "current",
            json!(false),
            json!("done"),
            false,
        ),
        (
            json!({"generation":2,"sequence":11,"stateChangeRevision":3,"seen":true}),
            Value::Null,
            false,
            "stale",
            Value::Null,
            Value::Null,
            true,
        ),
        (
            json!({"generation":3,"sequence":10,"stateChangeRevision":3,"seen":true}),
            json!(true),
            false,
            "stale",
            Value::Null,
            Value::Null,
            true,
        ),
        (
            json!({"generation":3,"sequence":11,"stateChangeRevision":3,"seen":true}),
            json!(true),
            true,
            "current",
            json!(true),
            json!("idle"),
            false,
        ),
    ] {
        let fixture = Fixture::new_limited();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let server = peer(&fixture, listener, move |mut stream| {
            snapshot_reply(&mut stream);
            let view = selected_reply(&mut stream, busy);
            attention_reply(&mut stream, view, state, outer, true);
            read_reply(
                &mut stream,
                "get",
                json!({"kind":"found","revision":3,"session":completed()}),
            );
            metadata_peer::sample(&mut stream, "observed", "codex.exe");
            read_reply(
                &mut stream,
                "get",
                json!({"kind":"found","revision":3,"session":completed()}),
            );
            snapshot_reply(&mut stream);
            peer_support::management_reply(&mut stream, 3, json!({"phase":"unmanaged"}));
            assert_closed(&mut stream);
        });
        let output = fixture.run(&[
            "agent",
            "get",
            "--session",
            "sess-alpha",
            "--client",
            &id(7),
            "--format=json",
        ]);
        server.join().unwrap();
        assert!(output.status.success(), "{output:?}");
        let doc = document(&output);
        validate(
            &doc,
            include_str!("../../src/cli/schemas/orchbee.agent.get.json"),
        );
        assert_eq!(doc["version"], 12);
        assert_eq!(doc["result"]["session"]["status"]["kind"], "succeeded");
        let client = &doc["result"]["client"];
        assert_eq!(client["state"], expected_state);
        assert_eq!(client["seen"], seen);
        assert_eq!(client["status"], status);
        assert_eq!(client["focused"], focused);
    }
}

#[test]
fn selected_client_get_checks_intervening_changes_and_refuses_foreign_boot() {
    for case in ["same-session", "other-session", "foreign-boot"] {
        let fixture = Fixture::new_limited();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let server = peer(&fixture, listener, move |mut stream| {
            snapshot_reply(&mut stream);
            let view = selected_reply(&mut stream, false);
            attention_reply(
                &mut stream,
                view,
                json!({"generation":3,"sequence":11,"stateChangeRevision":3,"seen":true}),
                json!(true),
                case != "foreign-boot",
            );
            if case != "foreign-boot" {
                let mut current = completed();
                if case == "same-session" {
                    current["snapshot"]["observation"]["freshness"] = json!("stale");
                }
                read_reply(
                    &mut stream,
                    "get",
                    json!({"kind":"found","revision":4,"session":current}),
                );
                metadata_peer::sample(&mut stream, "observed", "codex.exe");
                read_reply(
                    &mut stream,
                    "get",
                    json!({"kind":"found","revision":4,"session":current}),
                );
                snapshot_reply(&mut stream);
                peer_support::management_reply(&mut stream, 4, json!({"phase":"unmanaged"}));
                let AgentRequest::Changes(request) = read_agent_request(&mut stream).unwrap()
                else {
                    panic!("join continuity proof")
                };
                assert_eq!(json!(request)["cursor"], attention::token());
                let session = if case == "same-session" {
                    "sess-alpha"
                } else {
                    "other"
                };
                write_agent_result(&mut stream,&AgentResult::Changes(serde_json::from_value(json!({"kind":"agent.session.changes.result","requestId":request.request_id(),"result":{"outcome":{
                    "kind":"changes","cursor":attention::token(),"observedThroughRevision":4,"runtimeRevision":4,"hasMore":false,
                    "changes":[{"revision":4,"transition":"observation-became-stale","session":{
                        "sessionId":session,"providerId":"acme.agent","parentSessionId":null,"pendingRequestCount":0,"lifecycle":"live",
                        "observation":{"generation":3,"sequence":11,"freshness":"stale","fidelity":"rich","source":"native"},"status":{"kind":"succeeded"}
                    }}]
                }}})).unwrap())).unwrap();
            }
            assert_closed(&mut stream);
        });
        let output = fixture.run(&[
            "agent",
            "get",
            "--session",
            "sess-alpha",
            "--client",
            &id(7),
            "--format=json",
        ]);
        server.join().unwrap();
        let doc = document(&output);
        if case == "foreign-boot" {
            assert!(!output.status.success());
            assert!(doc.get("result").is_none());
            validate(
                &doc,
                include_str!("../../src/cli/schemas/orchbee.cli.failure.agent.json"),
            );
        } else {
            assert!(output.status.success(), "{output:?}");
            validate(
                &doc,
                include_str!("../../src/cli/schemas/orchbee.agent.get.json"),
            );
            assert_eq!(
                doc["result"]["client"]["state"],
                if case == "same-session" {
                    "stale"
                } else {
                    "current"
                }
            );
            assert_eq!(
                doc["result"]["client"]["seen"],
                if case == "same-session" {
                    Value::Null
                } else {
                    json!(true)
                }
            );
        }
    }
}

#[test]
fn selected_client_get_failures_keep_their_declared_contract() {
    for case in ["missing", "stale", "denied", "wrong-client", "deadline"] {
        let fixture = Fixture::new_limited();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let server = peer(&fixture, listener, move |mut stream| {
            snapshot_reply(&mut stream);
            let request = view_request(&mut stream);
            assert!(matches!(request.action, ClientViewAction::Get { .. }));
            let outcome = match case {
                "missing" => json!({"kind":"not-found"}),
                "denied" => json!({"kind":"denied"}),
                "stale" => {
                    let mut client = record(7, 6);
                    client["availability"] = json!("stale");
                    json!({"kind":"found","client":client})
                }
                _ => json!({"kind":"found","client":record(8,6)}),
            };
            if case != "deadline" {
                view_reply(&mut stream, &request, outcome);
            }
            assert_closed(&mut stream);
        });
        let output = fixture.run(&[
            "agent",
            "get",
            "--session",
            "sess-alpha",
            "--client",
            &id(7),
            "--format=json",
        ]);
        server.join().unwrap();
        let doc = document(&output);
        validate(
            &doc,
            include_str!("../../src/cli/schemas/orchbee.cli.failure.client-control.json"),
        );
        let (kind, version) = match case {
            "missing" => ("not-found", 1),
            "stale" => ("service-unavailable", 1),
            "denied" => ("denied", 2),
            "deadline" => ("timeout", 2),
            _ => ("protocol-invalid", 2),
        };
        assert!(!output.status.success());
        assert_eq!(doc["failure"]["kind"], kind);
        assert_eq!(doc["version"], version);
    }
}
