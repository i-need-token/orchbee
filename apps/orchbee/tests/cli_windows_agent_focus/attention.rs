//! Process-level selected Client observations, including boot authentication.
use super::*;
use orchbee_agent_observation_protocol::{AgentResult, write_agent_result};

pub(super) fn token() -> String {
    format!("v1.{}.{}", "A".repeat(59), "A".repeat(43))
}

pub(super) fn observation(state: Value) -> Value {
    json!({"cursor":token(),"revision":3,"outerFocused":false,"entries":[{"sessionId":"sess-alpha","state":state}]})
}

pub(super) fn authenticate(stream: &mut LocalStream, allowed: bool) {
    let AgentRequest::Changes(request) = read_agent_request(stream).unwrap() else {
        panic!("cursor authentication before exposure")
    };
    assert_eq!(json!(request)["cursor"], token());
    assert_eq!(json!(request)["limit"], 1);
    let outcome = if allowed {
        json!({"kind":"current","cursor":token(),"revision":3})
    } else {
        json!({"kind":"rejected","reason":"cursor-invalid"})
    };
    write_agent_result(stream, &AgentResult::Changes(serde_json::from_value(json!({"kind":"agent.session.changes.result","requestId":request.request_id(),"result":{"outcome":outcome}})).unwrap())).unwrap();
}

#[test]
fn attention_process_observes_a_busy_client_without_navigation_or_acknowledgement() {
    for state in [
        Value::Null,
        json!({"generation":1,"sequence":2,"stateChangeRevision":3,"seen":false}),
        json!({"generation":1,"sequence":2,"stateChangeRevision":3,"seen":true}),
    ] {
        let fixture = FocusFixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let expected = state.clone();
        let server = peer(&fixture, listener, move |mut stream| {
            snapshot_reply(&mut stream);
            let mut busy = record(7, 6);
            busy["view"]["phase"] = json!("busy");
            list_reply(&mut stream, vec![busy.clone(), record(9, 8)]);
            resolved(&mut stream);
            let request = view_request(&mut stream);
            let ClientViewAction::Submit { command } = &request.action else {
                panic!("read submission")
            };
            assert_eq!(command.client_id.as_str(), id(7));
            assert_eq!(
                json!(command.intent),
                json!({"kind":"agent-attention","sessionIds":["sess-alpha"]})
            );
            view_reply(
                &mut stream,
                &request,
                json!({"kind":"operation","operation":{"command":command,"status":{"kind":"attention-read","attention":observation(state),"view":busy["view"]}}}),
            );
            authenticate(&mut stream, true);
            assert_closed(&mut stream);
        });
        let output = fixture.run(&[
            "agent",
            "attention",
            "alpha",
            "--client",
            &id(7),
            "--format=json",
        ]);
        server.join().unwrap();
        assert!(output.status.success(), "{output:?}");
        let doc = document(&output);
        receipt(&doc);
        let result = &doc["observation"]["reply"]["outcome"]["operation"]["status"];
        assert_eq!(result["attention"]["entries"][0]["state"], expected);
        assert_eq!(result["view"]["selection"]["panelId"], id(6));
        assert_eq!(result["attention"]["outerFocused"], false);
    }
}

#[test]
fn attention_process_foreign_boot_never_exposes_an_old_seen_flag() {
    let fixture = FocusFixture::new();
    let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
    let server = peer(&fixture, listener, move |mut stream| {
        snapshot_reply(&mut stream);
        list_reply(&mut stream, vec![record(7, 6)]);
        let request = view_request(&mut stream);
        let ClientViewAction::Submit { command } = &request.action else {
            panic!("read submission")
        };
        view_reply(
            &mut stream,
            &request,
            json!({"kind":"operation","operation":{"command":command,"status":{"kind":"attention-read","attention":observation(json!({"generation":1,"sequence":2,"stateChangeRevision":3,"seen":true})),"view":record(7,6)["view"]}}}),
        );
        authenticate(&mut stream, false);
        assert_closed(&mut stream);
    });
    let output = fixture.run(&[
        "agent",
        "attention",
        "--session",
        "sess-alpha",
        "--format=json",
    ]);
    server.join().unwrap();
    assert!(!output.status.success(), "{output:?}");
    let doc = document(&output);
    assert_eq!(doc["schema"], "orchbee.cli.failure.agent");
    validate(
        &doc,
        include_str!("../../src/cli/schemas/orchbee.cli.failure.agent.json"),
    );
    assert!(
        !String::from_utf8(output.stdout)
            .unwrap()
            .contains("\"seen\"")
    );
}
