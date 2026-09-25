//! The same admitted Windows CLI peer exercises the bounded changes family.
use super::*;

fn cursor() -> String {
    format!("v1.{}.{}", "A".repeat(59), "A".repeat(43))
}

fn changes() -> Value {
    json!({"kind":"changes","changes":[{"revision":1,"transition":"session-observed","session":{
        "lifecycle":"live","observation":{"fidelity":"rich","freshness":"fresh","generation":1,"sequence":1,"source":"native"},
        "parentSessionId":null,"pendingRequestCount":0,"providerId":"test","sessionId":"s1","status":{"kind":"idle"}
    }}],"cursor":cursor(),"observedThroughRevision":1,"runtimeRevision":1,"hasMore":false})
}

#[test]
fn changes_windows_process_validates_schema_and_closes_after_one_page() {
    let fixture = Fixture::new();
    for (outcome, checkpoint) in [
        (
            json!({"kind":"current","cursor":cursor(),"revision":0}),
            true,
        ),
        (changes(), false),
        (
            json!({"kind":"reset-required","cursor":cursor(),"revision":999}),
            false,
        ),
    ] {
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let expected = outcome.clone();
        let handler = peer(&fixture, listener, move |mut stream| {
            let AgentRequest::Changes(request) = read_agent_request(&mut stream).unwrap() else {
                panic!("changes request")
            };
            let doc = json!(request);
            assert_eq!(doc["limit"], 1);
            assert_eq!(doc.get("cursor").is_none(), checkpoint);
            validate(
                &doc,
                include_str!(
                    "../../../../schemas/agent-observation/v1/agent-observation.schema.json"
                ),
            );
            let response = json!({"kind":"agent.session.changes.result","requestId":request.request_id(),"result":{"outcome":outcome}});
            validate(
                &response,
                include_str!(
                    "../../../../schemas/agent-observation/v1/agent-observation.schema.json"
                ),
            );
            write_agent_result(
                &mut stream,
                &AgentResult::Changes(serde_json::from_value(response).unwrap()),
            )
            .unwrap();
            assert_closed(&mut stream);
        });
        let token = cursor();
        let mut args = vec!["agent", "changes", "--limit", "1", "--format", "json"];
        if !checkpoint {
            args.extend(["--cursor", token.as_str()]);
        }
        let output = fixture.run(&args);
        join_peer(handler, &output);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &doc,
            include_str!("../../src/cli/schemas/orchbee.agent.changes.json"),
        );
        assert_eq!(doc["result"], expected);
    }
}

#[test]
fn changes_windows_refusals_keep_machine_failure_and_reject_wrong_correlation() {
    let fixture = Fixture::new();
    for (outcome, failure, wrong_id) in [
        (
            json!({"kind":"rejected","reason":"cursor-invalid"}),
            "rejected",
            false,
        ),
        (
            json!({"kind":"rejected","reason":"read-denied"}),
            "denied",
            false,
        ),
        (
            json!({"kind":"unavailable","reason":"runtime-unavailable"}),
            "service-unavailable",
            false,
        ),
        (changes(), "protocol-invalid", true),
    ] {
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            let AgentRequest::Changes(request) = read_agent_request(&mut stream).unwrap() else {
                panic!("changes request")
            };
            let response = json!({"kind":"agent.session.changes.result","requestId": if wrong_id { "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee" } else { request.request_id().as_str() },"result":{"outcome":outcome}});
            write_agent_result(
                &mut stream,
                &AgentResult::Changes(serde_json::from_value(response).unwrap()),
            )
            .unwrap();
            assert_closed(&mut stream);
        });
        let output = fixture.run(&[
            "agent",
            "changes",
            "--cursor",
            &cursor(),
            "--format",
            "json",
        ]);
        join_peer(handler, &output);
        assert!(!output.status.success());
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &doc,
            include_str!("../../src/cli/schemas/orchbee.cli.failure.agent.json"),
        );
        assert_eq!(doc["failure"]["kind"], failure);
        assert_eq!(doc["failure"]["details"]["operation"], "changes");
    }
}

#[test]
fn changes_windows_refuses_page_larger_than_requested_without_retry() {
    let fixture = Fixture::new();
    let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
    let handler = peer(&fixture, listener, move |mut stream| {
        let AgentRequest::Changes(request) = read_agent_request(&mut stream).unwrap() else {
            panic!("changes request")
        };
        let mut outcome = changes();
        let mut second = outcome["changes"][0].clone();
        second["revision"] = json!(2);
        outcome["changes"].as_array_mut().unwrap().push(second);
        outcome["observedThroughRevision"] = json!(2);
        outcome["runtimeRevision"] = json!(2);
        let response = json!({"kind":"agent.session.changes.result","requestId":request.request_id(),"result":{"outcome":outcome}});
        write_agent_result(
            &mut stream,
            &AgentResult::Changes(serde_json::from_value(response).unwrap()),
        )
        .unwrap();
        assert_closed(&mut stream);
    });
    let output = fixture.run(&[
        "agent",
        "changes",
        "--cursor",
        &cursor(),
        "--limit",
        "1",
        "--format",
        "json",
    ]);
    join_peer(handler, &output);
    let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(doc["failure"]["kind"], "protocol-invalid");
}
