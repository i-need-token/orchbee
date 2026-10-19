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
fn projected_agent_reply(
    stream: &mut orchbee_local_ipc::LocalStream,
    detail: Value,
    revision: u64,
) -> AgentProjectionFixture {
    bound_snapshot_reply(stream);
    let projection = AgentProjectionFixture::canonical(
        bound_snapshot(9),
        revision,
        std::slice::from_ref(&detail),
    );
    projection.read(stream, true);
    resolve_reply(
        stream,
        json!({"kind":"resolved", "sessionId":"sess-alpha", "resolvedBy":"name"}),
    );
    read_reply(
        stream,
        "get",
        json!({"kind":"found", "revision":revision, "session":detail}),
    );
    projection
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
            if case == "stale" {
                initial["snapshot"]["observation"]["freshness"] = json!("stale");
            }
            if case == "blocked" {
                initial = session_detail("sess-alpha");
            }
            let projection = projected_agent_reply(
                &mut stream,
                initial.clone(),
                if case == "binding-change" { 1 } else { 2 },
            );
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
                let mut current = initial;
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
            if case == "binding-change" {
                let mut changed = ready();
                changed["snapshot"]["presentationAttachments"][0]["generation"] = json!("gen-new");
                projection.canonical_drift(&mut stream, true, 2, &[changed]);
            } else {
                projection.confirm(&mut stream, true);
            }
            assert_closed(&mut stream);
        });
        let output = fixture.run(&["agent", "get", "alpha", "--format=json"]);
        handler.join().unwrap();
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        if case == "binding-change" {
            assert_eq!(output.status.code(), Some(3), "{doc}");
            validate(
                &doc,
                include_str!("../src/cli/schemas/orchbee.cli.failure.agent-projection.json"),
            );
            assert_eq!(doc["schema"], "orchbee.cli.failure.agent-projection");
            assert_eq!(doc["failure"]["details"]["reason"], "changed");
            continue;
        }
        assert!(output.status.success(), "{case}: {output:?}");
        validate(
            &doc,
            include_str!("../src/cli/schemas/orchbee.agent.get.json"),
        );
        assert_eq!(doc["version"], 19);
        assert_eq!(doc["source"]["kind"], "agent-projection");
        assert_eq!(doc["result"]["kind"], "projected");
        assert_eq!(doc["result"]["agent"]["source"], "canonical");
        assert!(doc["result"]["inspection"].get("client").is_none());
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
                "reportedAgent",
            ] {
                let mut bad = doc.clone();
                if field == "terminal" {
                    bad["result"]["inspection"]
                        .as_object_mut()
                        .unwrap()
                        .remove(field);
                } else {
                    bad["result"]["inspection"]["terminal"]
                        .as_object_mut()
                        .unwrap()
                        .remove(field);
                }
                assert!(!validator.is_valid(&bad), "missing {field}");
            }
            let mut bad = doc.clone();
            bad["result"]["inspection"]["terminal"] = Value::Null;
            assert!(!validator.is_valid(&bad));
            let mut bad = doc.clone();
            bad["result"]["agent"]["invented"] = json!(true);
            assert!(!validator.is_valid(&bad));
            let mut obsolete = doc.clone();
            obsolete["version"] = json!(18);
            assert!(!validator.is_valid(&obsolete));
        }

        assert_eq!(
            doc["result"]["inspection"]["management"]["state"]["phase"],
            match case {
                "pending" => "pending",
                "active" => "active",
                _ => "unmanaged",
            }
        );
        let runtime = &doc["result"]["inspection"]["runtime"];
        if case == "unbound" {
            assert!(doc["result"]["inspection"]["terminal"].is_null());
            assert_eq!(runtime["kind"], "unavailable");
            assert_eq!(runtime["reason"], "unbound");
        } else {
            let terminal = &doc["result"]["inspection"]["terminal"];
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
            let projection = projected_agent_reply(&mut stream, ready(), 2);
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
                if mode == "changed-workbench" {
                    projection.workbench_drift(&mut stream, true, 10);
                } else {
                    projection.confirm(&mut stream, true);
                }
            }
            assert_closed(&mut stream);
        });
        let started = std::time::Instant::now();
        let output = fixture.run(&["agent", "get", "alpha", "--format=json"]);
        handler.join().unwrap();
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        if matches!(mode, "wrong-epoch" | "deadline" | "changed-workbench") {
            assert_eq!(
                output.status.code(),
                Some(match mode {
                    "deadline" => 7,
                    "changed-workbench" => 3,
                    _ => 5,
                }),
                "{doc}"
            );
            assert!(doc.get("result").is_none());
            if mode == "deadline" {
                assert!(started.elapsed() < std::time::Duration::from_secs(8));
            }
            if mode == "changed-workbench" {
                validate(
                    &doc,
                    include_str!("../src/cli/schemas/orchbee.cli.failure.agent-projection.json"),
                );
                assert_eq!(doc["schema"], "orchbee.cli.failure.agent-projection");
                assert_eq!(doc["failure"]["details"]["reason"], "changed");
            }
            continue;
        }
        assert!(output.status.success(), "{mode}: {doc}");
        validate(
            &doc,
            include_str!("../src/cli/schemas/orchbee.agent.get.json"),
        );
        let result = &doc["result"]["inspection"];
        assert_eq!(result["runtime"]["kind"], "unavailable");
        assert!(result["runtime"].get("promptReady").is_none());
        assert_eq!(result["runtime"]["reason"], mode);
        let terminal = &result["terminal"];
        if mode == "terminal-unavailable" {
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
        invalid["result"]["inspection"]["runtime"]["promptReady"] = json!(true);
        assert!(!validator.is_valid(&invalid));
        let mut invalid = doc.clone();
        invalid["version"] = json!(18);
        assert!(!validator.is_valid(&invalid));
    }
}
