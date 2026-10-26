//! Actual Windows command dispatch, source ownership, and uncertain mutation receipts.
#![cfg(windows)]
#[allow(dead_code)]
#[path = "support/windows_agent_peer.rs"]
mod support;
use orchbee_local_ipc::LocalListener;
use orchbee_workbench_protocol::*;
use serde_json::Value;
use support::{Fixture, assert_closed, peer, validate};

const SPEC: &str = r#"{"source":" tools.views ","label":" Views ","filter":{"op":"eq","field":"status","value":"blocked"},"sort":[{"field":{"token":"phase"},"order":"desc"}]}"#;

#[test]
fn source_view_cli_returns_correlated_effects_and_never_claims_lost_mutations_succeeded() {
    for (verb, case) in [
        ("get", "active"),
        ("get", "lost"),
        ("set", "active"),
        ("set", "disabled"),
        ("set", "lost"),
        ("set", "wrong-effect"),
        ("clear", "cleared"),
        ("clear", "other-owner"),
        ("clear", "wrong-effect"),
        ("clear", "unqualified"),
    ] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let server = peer(&fixture, listener, move |mut stream| {
            support::bound_snapshot_reply(&mut stream);
            let WorkbenchRequest::AgentView(request) = read_workbench_request(&mut stream).unwrap()
            else {
                panic!("one source-view exchange after snapshot")
            };
            assert_eq!(
                request.session_id.as_str(),
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            );
            match &request.action {
                AgentViewAction::Read {} => assert_eq!(verb, "get"),
                AgentViewAction::Set { spec } => {
                    assert_eq!(verb, "set");
                    assert_eq!(spec.source, "tools.views");
                    assert_eq!(spec.label.as_deref(), Some("Views"));
                }
                AgentViewAction::Clear { source } => {
                    assert_eq!(verb, "clear");
                    assert_eq!(
                        source.as_deref(),
                        (case != "unqualified").then_some("tools.views")
                    );
                }
            }
            if case == "lost" {
                return;
            }
            let outcome = if case == "disabled" {
                AgentViewOutcome::Failed {
                    reason: AgentViewFailure::PluginDisabled,
                }
            } else {
                let mut spec = parse_agent_view_spec(SPEC.as_bytes()).unwrap();
                if case == "other-owner" || (verb == "set" && case == "wrong-effect") {
                    spec.source = "other.views".into();
                }
                AgentViewOutcome::Available {
                    snapshot: AgentViewSnapshot {
                        revision: WorkbenchRevision::new(4),
                        spec: (!matches!(case, "cleared" | "unqualified")).then_some(spec),
                    },
                }
            };
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::AgentView(AgentViewResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    session_id: request.session_id,
                    outcome,
                }),
            )
            .unwrap();
            assert_closed(&mut stream);
        });
        let mut args = vec!["agent", "view", verb, "--format=json"];
        match verb {
            "set" => args.extend(["--spec", SPEC]),
            "clear" if case != "unqualified" => args.extend(["--source", " tools.views "]),
            _ => {}
        }
        let output = fixture.run(&args);
        server.join().unwrap();
        let doc: Value = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|e| panic!("{verb}/{case}: {e}: {output:?}"));
        if verb == "get" && case == "lost" {
            assert_eq!(output.status.code(), Some(5));
            validate(
                &doc,
                include_str!("../src/cli/schemas/orchbee.agent.view.json"),
            );
            assert_eq!(doc["result"]["kind"], "exchange-failed");
            assert_eq!(doc["result"]["reason"], "response-read");
            continue;
        }
        validate(
            &doc,
            include_str!("../src/cli/schemas/orchbee.agent.view.json"),
        );
        assert_eq!(doc["operation"], verb);
        assert_eq!(doc["sessionId"], "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        if matches!(case, "lost" | "wrong-effect") {
            assert_eq!(output.status.code(), Some(8), "{verb}/{case}: {output:?}");
            assert_eq!(doc["result"]["kind"], "unknown");
        } else if case == "disabled" {
            assert_eq!(output.status.code(), Some(3));
            assert_eq!(doc["result"]["reason"], "pluginDisabled");
        } else {
            assert!(output.status.success(), "{verb}/{case}: {output:?}");
            assert_eq!(doc["result"]["kind"], "available");
            let spec = &doc["result"]["snapshot"]["spec"];
            match case {
                "cleared" | "unqualified" => assert!(spec.is_null()),
                "other-owner" => assert_eq!(spec["source"], "other.views"),
                _ => assert_eq!(spec["source"], "tools.views"),
            }
        }
    }
}

#[test]
fn malformed_view_commands_refuse_before_runtime_and_schema_and_help_are_discoverable() {
    let fixture = Fixture::new();
    for args in [
        vec!["agent", "view", "set", "--format=json"],
        vec!["agent", "view", "set", "--spec", "{bad}", "--format=json"],
        vec![
            "agent",
            "view",
            "set",
            "--spec",
            "{\"source\":\"x\",\"extra\":true}",
            "--format=json",
        ],
        vec!["agent", "view", "get", "--spec", SPEC, "--format=json"],
        vec![
            "agent",
            "view",
            "clear",
            "--source=bad/source",
            "--format=json",
        ],
        vec!["agent", "list", "--spec", SPEC, "--format=json"],
    ] {
        let output = fixture.run(&args);
        assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &doc,
            include_str!("../src/cli/schemas/orchbee.cli.failure.json"),
        );
        assert!(!fixture.0.join("runtime").exists());
    }
    let schema = fixture.run(&["api", "schema", "orchbee.agent.view"]);
    assert!(schema.status.success(), "{schema:?}");
    let document: Value = serde_json::from_slice(&schema.stdout).unwrap();
    assert!(document["result"]["schema"]["document"]["$defs"]["spec"].is_object());
    let help = fixture.run(&["help", "agent", "view", "set"]);
    assert!(help.status.success(), "{help:?}");
    assert!(String::from_utf8(help.stdout).unwrap().contains("--spec"));
    assert!(!fixture.0.join("runtime").exists());
}
