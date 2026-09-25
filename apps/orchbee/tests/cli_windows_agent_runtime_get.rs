//! Agent Get joins a real bounded process sample only across a stable binding.
#![cfg(windows)]
#[allow(dead_code)]
#[path = "support/windows_agent_peer.rs"]
mod peer_support;
use orchbee_local_ipc::LocalListener;
use peer_support::*;
use serde_json::{Value, json};
fn ready() -> Value {
    let mut detail = session_detail("sess-alpha");
    detail["snapshot"]["pendingRequests"] = json!([]);
    detail["snapshot"]["turn"] = json!({"kind":"idle"});
    detail["status"] = json!({"kind":"idle"});
    detail
}
#[test]
fn agent_get_returns_process_facts_and_bounded_prompt_readiness() {
    for case in [
        "ready",
        "mismatch",
        "shell",
        "stale",
        "blocked",
        "binding-change",
        "unbound",
        "pending",
        "active",
    ] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            let mut initial = ready();
            if case == "unbound" {
                initial["snapshot"]["presentationAttachments"] = json!([]);
            }
            bound_agent_reply(&mut stream, initial);
            if case != "unbound" {
                let mut sample = process(
                    10,
                    None,
                    match case {
                        "mismatch" => "codex.exe",
                        "shell" => "pwsh.exe",
                        _ => "claude-code.exe",
                    },
                );
                sample.current_directory = Some(r"C:\Users\fixture\project".into());
                process_reply(&mut stream, vec![sample]);
                let mut current = ready();
                if case == "stale" {
                    current["snapshot"]["observation"]["freshness"] = json!("stale");
                }
                if case == "blocked" {
                    current = session_detail("sess-alpha");
                }
                if case == "binding-change" {
                    current["snapshot"]["presentationAttachments"][0]["generation"] =
                        json!("gen-new");
                }
                read_reply(
                    &mut stream,
                    "get",
                    json!({"kind":"found","revision":2,"session":current}),
                );
            }
            let binding = json!({"presentationId":"22222222222222222222222222222222","generation":"33333333333333333333333333333333","hostEpoch":"66666666666666666666666666666666","hostTerminalId":"77777777777777777777777777777777"});
            let management = match case {
                "pending" => {
                    json!({"phase":"pending","blocked":false,"reservation":{"reservationId":"11111111111111111111111111111111","providerId":"claude-code","name":"alpha","binding":binding,"remainingMs":10000,"settleRemainingMs":0}})
                }
                "active" => {
                    json!({"phase":"active","reservationId":"11111111111111111111111111111111","providerId":"claude-code","binding":binding})
                }
                _ => json!({"phase":"unmanaged"}),
            };
            management_reply(&mut stream, 2, management);
            assert_closed(&mut stream);
        });
        let output = fixture.run(&["agent", "get", "alpha", "--format=json"]);
        handler.join().unwrap();
        assert!(output.status.success(), "{case}: {output:?}");
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &doc,
            include_str!("../src/cli/schemas/orchbee.agent.get.json"),
        );
        assert_eq!(doc["version"], 5);
        assert!(doc["result"].get("client").is_none());
        assert_eq!(
            doc["result"]["management"]["state"]["phase"],
            match case {
                "pending" => "pending",
                "active" => "active",
                _ => "unmanaged",
            }
        );
        let runtime = &doc["result"]["runtime"];
        if matches!(case, "unbound" | "binding-change") {
            assert_eq!(runtime["kind"], "unavailable");
            assert_eq!(
                runtime["reason"],
                if case == "unbound" {
                    "unbound"
                } else {
                    "binding-changed"
                }
            );
        } else {
            assert_eq!(runtime["kind"], "observed");
            assert_eq!(runtime["promptReady"], matches!(case, "ready" | "active"));
            assert_eq!(runtime["source"]["kind"], "terminal-process-query");
            assert_eq!(
                runtime["processInfo"]["processes"][0]["currentDirectory"],
                r"C:\Users\fixture\project"
            );
            assert_eq!(runtime["processInfo"]["processes"][0]["pid"], 10);
        }
    }
}

#[test]
fn management_wire_schema_accepts_each_phase_and_rejects_unowned_readiness() {
    use orchbee_agent_observation_protocol::{parse_agent_request, parse_agent_result};
    let schema =
        include_str!("../../../schemas/agent-observation/v1/agent-observation.schema.json");
    let request = json!({"kind":"agent.launch.status.request","requestId":"11111111111111111111111111111111","sessionId":"sess-alpha"});
    validate(&request, schema);
    assert!(parse_agent_request(&serde_json::to_vec(&request).unwrap()).is_ok());
    let binding = json!({"presentationId":"22222222222222222222222222222222","generation":"33333333333333333333333333333333","hostEpoch":"66666666666666666666666666666666","hostTerminalId":"77777777777777777777777777777777"});
    for management in [
        json!({"phase":"unmanaged"}),
        json!({"phase":"active","reservationId":request["requestId"],"providerId":"codex","binding":binding}),
        json!({"phase":"pending","blocked":true,"reservation":{"reservationId":request["requestId"],"name":"alpha","providerId":"codex","binding":binding,"remainingMs":5000,"settleRemainingMs":0}}),
    ] {
        let doc = json!({"kind":"agent.launch.status.result","requestId":request["requestId"],"result":{"outcome":{"kind":"status","sessionId":"sess-alpha","revision":2,"management":management}}});
        validate(&doc, schema);
        assert!(parse_agent_result(&serde_json::to_vec(&doc).unwrap()).is_ok());
        let mut invalid = doc;
        invalid["result"]["outcome"]["management"]["interactiveReady"] = json!(true);
        let schema: Value = serde_json::from_str(schema).unwrap();
        assert!(
            !jsonschema::validator_for(&schema)
                .unwrap()
                .is_valid(&invalid)
        );
        assert!(parse_agent_result(&serde_json::to_vec(&invalid).unwrap()).is_err());
    }
}
