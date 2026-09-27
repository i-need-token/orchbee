//! Agent Get joins a real bounded process sample only across a stable binding.
#![cfg(windows)]
#[allow(dead_code)]
#[path = "support/terminal_metadata_peer.rs"]
mod metadata_peer;
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
                metadata_peer::sample(
                    &mut stream,
                    case,
                    match case {
                        "mismatch" => "codex.exe",
                        "shell" => "pwsh.exe",
                        _ => "claude-code.exe",
                    },
                );
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
            if case != "unbound" {
                bound_snapshot_reply(&mut stream);
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
        assert_eq!(doc["version"], 9);
        assert!(doc["result"].get("client").is_none());
        if case == "ready" {
            let schema: Value =
                serde_json::from_str(include_str!("../src/cli/schemas/orchbee.agent.get.json"))
                    .unwrap();
            let validator = jsonschema::validator_for(&schema).unwrap();
            for field in [
                "terminal",
                "terminalTitle",
                "cwd",
                "cwdSource",
                "foregroundCwd",
                "terminalObservation",
                "source",
                "terminalId",
                "metadata",
            ] {
                let mut bad = doc.clone();
                if field == "terminal" {
                    bad["result"].as_object_mut().unwrap().remove(field);
                } else {
                    bad["result"]["terminal"]
                        .as_object_mut()
                        .unwrap()
                        .remove(field);
                }
                assert!(!validator.is_valid(&bad), "missing {field}");
            }
            let mut bad = doc.clone();
            bad["result"]["terminal"] = Value::Null;
            assert!(!validator.is_valid(&bad));
            let mut bad = doc.clone();
            bad["result"]["terminal"]["invented"] = json!(true);
            assert!(!validator.is_valid(&bad));
            for version in [1, 3, 5] {
                let mut legacy = doc.clone();
                legacy["version"] = json!(version);
                legacy["result"].as_object_mut().unwrap().remove("terminal");
                if version < 5 {
                    legacy["result"]
                        .as_object_mut()
                        .unwrap()
                        .remove("management");
                }
                if version < 3 {
                    legacy["result"].as_object_mut().unwrap().remove("runtime");
                }
                assert!(validator.is_valid(&legacy), "legacy {version}");
                legacy["result"]["terminal"] = doc["result"]["terminal"].clone();
                assert!(!validator.is_valid(&legacy));
            }
        }

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
            assert!(doc["result"]["terminal"].is_null());
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
            let terminal = &doc["result"]["terminal"];
            assert_eq!(terminal["terminalTitle"], "Orchbee 🐝");
            assert_eq!(terminal["terminalTitleStripped"], "Orchbee 🐝");
            assert_eq!(terminal["cwd"], r"C:\frame 项目");
            assert_eq!(terminal["cwdSource"], "terminal-frame");
            assert_eq!(terminal["foregroundCwd"], r"C:\root");
            assert_eq!(terminal["source"]["revision"], "9");
            assert_eq!(terminal["terminalId"], "3".repeat(32));
            assert_eq!(runtime["kind"], "observed");
            assert_eq!(runtime["promptReady"], matches!(case, "ready" | "active"));
            assert_eq!(runtime["source"]["kind"], "terminal-process-query");
            assert_eq!(
                runtime["processInfo"]["processes"][0]["currentDirectory"],
                r"C:\root"
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

#[test]
fn agent_terminal_observation_preserves_partial_availability_and_rejects_stale_or_invalid_reads() {
    for mode in [
        "terminal-unavailable",
        "process-unavailable",
        "changed-workbench",
        "wrong-epoch",
        "deadline",
    ] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_agent_reply(&mut stream, ready());
            if mode == "terminal-unavailable" {
                metadata_peer::unavailable(&mut stream);
            } else {
                metadata_peer::sample(&mut stream, mode, "claude-code.exe");
            }
            if !matches!(mode, "wrong-epoch" | "deadline") {
                read_reply(
                    &mut stream,
                    "get",
                    json!({"kind":"found","revision":2,"session":ready()}),
                );
                bound_snapshot_at(
                    &mut stream,
                    if mode == "changed-workbench" { 10 } else { 9 },
                );
                management_reply(&mut stream, 2, json!({"phase":"unmanaged"}));
            }
            assert_closed(&mut stream);
        });
        let started = std::time::Instant::now();
        let output = fixture.run(&["agent", "get", "alpha", "--format=json"]);
        handler.join().unwrap();
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        if matches!(mode, "wrong-epoch" | "deadline") {
            assert_eq!(
                output.status.code(),
                Some(if mode == "deadline" { 7 } else { 5 }),
                "{doc}"
            );
            assert!(doc.get("result").is_none());
            if mode == "deadline" {
                assert!(started.elapsed() < std::time::Duration::from_secs(8));
            }
            continue;
        }
        assert!(output.status.success(), "{mode}: {doc}");
        validate(
            &doc,
            include_str!("../src/cli/schemas/orchbee.agent.get.json"),
        );
        let result = &doc["result"];
        assert_eq!(result["runtime"]["kind"], "unavailable");
        assert!(result["runtime"].get("promptReady").is_none());
        assert_eq!(
            result["runtime"]["reason"],
            if mode == "changed-workbench" {
                "binding-changed"
            } else {
                mode
            }
        );
        let terminal = &result["terminal"];
        if mode == "changed-workbench" {
            assert!(terminal.is_null());
        } else if mode == "terminal-unavailable" {
            assert!(terminal["terminalTitle"].is_null());
            assert_eq!(terminal["terminalObservation"]["state"], "unavailable");
        } else {
            assert_eq!(terminal["terminalTitle"], "Orchbee 🐝");
            assert_eq!(terminal["cwdSource"], "terminal-frame");
            assert!(terminal["foregroundCwd"].is_null());
            assert_eq!(
                terminal["terminalObservation"]["processes"]["state"],
                "unavailable"
            );
        }
        let schema: Value =
            serde_json::from_str(include_str!("../src/cli/schemas/orchbee.agent.get.json"))
                .unwrap();
        let validator = jsonschema::validator_for(&schema).unwrap();
        let mut invalid = doc.clone();
        invalid["result"]["runtime"]["promptReady"] = json!(true);
        assert!(!validator.is_valid(&invalid));
        let mut invalid = doc.clone();
        invalid["version"] = json!(5);
        invalid["result"]
            .as_object_mut()
            .unwrap()
            .remove("terminal");
        if mode != "changed-workbench" {
            assert!(!validator.is_valid(&invalid));
        }
    }
}
