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
        assert_eq!(doc["version"], 3);
        assert!(doc["result"].get("client").is_none());
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
            assert_eq!(runtime["promptReady"], case == "ready");
            assert_eq!(runtime["source"]["kind"], "terminal-process-query");
            assert_eq!(
                runtime["processInfo"]["processes"][0]["currentDirectory"],
                r"C:\Users\fixture\project"
            );
            assert_eq!(runtime["processInfo"]["processes"][0]["pid"], 10);
        }
    }
}
