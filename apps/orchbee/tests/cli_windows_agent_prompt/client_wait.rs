//! Selected Client observation never changes delivery or acknowledgement ownership.
use super::*;
use orchbee_workbench_protocol::*;
const CLIENT: &str = "11111111111111111111111111111117";
fn request(stream: &mut LocalStream) -> WorkbenchClientViewRequest {
    let WorkbenchRequest::ClientView(request) = read_workbench_request(stream).unwrap() else {
        panic!("Client read");
    };
    request
}
fn reply(stream: &mut LocalStream, request: &WorkbenchClientViewRequest, outcome: Value) {
    write_workbench_response(
        stream,
        &WorkbenchResponse::ClientView(WorkbenchClientViewResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id.clone(),
            session_id: request.session_id.clone(),
            outcome: serde_json::from_value(outcome).unwrap(),
        }),
    )
    .unwrap();
}
fn attention(stream: &mut LocalStream, revision: u64, sequence: u64, seen: Option<bool>) {
    let get = request(stream);
    assert!(
        matches!(&get.action,ClientViewAction::Get { client_id } if client_id.as_str()==CLIENT)
    );
    let view = json!({"workbenchRevision":"9","phase":"ready","selection":{"terminalContextId":"00000000000000000000000000000001","tabId":"00000000000000000000000000000002","panelId":"22222222222222222222222222222222","zoomed":false,"panelCount":1}});
    reply(
        stream,
        &get,
        json!({"kind":"found","client":{"clientId":CLIENT,"heartbeat":"1","viewRevision":"1","availability":"current","view":view}}),
    );
    let submit = request(stream);
    let ClientViewAction::Submit { command } = &submit.action else {
        panic!("one retained read")
    };
    assert_eq!(
        json!(command.intent),
        json!({"kind":"agent-attention","sessionIds":["sess-alpha"]})
    );
    assert_eq!(command.client_id.as_str(), CLIENT);
    let state=seen.map(|seen| json!({"generation":idle()["snapshot"]["observation"]["generation"],"sequence":sequence,"stateChangeRevision":revision,"seen":seen}));
    reply(
        stream,
        &submit,
        json!({"kind":"operation","operation":{"command":command,"status":{"kind":"attention-read","view":view,"attention":{"cursor":current_changes()["cursor"],"revision":revision,"outerFocused":false,"entries":[{"sessionId":"sess-alpha","state":state}]}}}}),
    );
    current(stream, revision);
}
fn current(stream: &mut LocalStream, revision: u64) {
    let mut outcome = current_changes();
    outcome["revision"] = json!(revision);
    changes_reply(stream, outcome);
}
#[test]
fn prompt_wait_uses_actual_state_and_outcomes_independently_of_client_seen() {
    for (condition, outcome, seen) in [
        ("idle", "idle", Some(true)),
        ("idle", "idle", Some(false)),
        ("idle", "idle", None),
        ("done", "succeeded", Some(true)),
        ("done", "succeeded", Some(false)),
        ("done", "succeeded", None),
        ("done", "failed", Some(true)),
        ("done", "failed", None),
    ] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_agent_reply(&mut stream, idle());
            prompt_preflight(&mut stream, idle());
            checkpoint(&mut stream);
            attention(&mut stream, 2, 11, Some(true));
            accept_prompt(&mut stream, "task");
            history_page(
                &mut stream,
                json!([history_event(3, 12, "working", "turn-started")]),
                3,
            );
            attention(&mut stream, 4, 13, seen);
            let mut settled = completed();
            settled["snapshot"]["observation"]["sequence"] = json!(13);
            settled["status"] = json!({"kind":outcome});
            if outcome == "idle" {
                settled["snapshot"]["lastOutcome"] = Value::Null;
            } else {
                settled["snapshot"]["lastOutcome"]["kind"] = json!(outcome);
            }
            read_reply(
                &mut stream,
                "get",
                json!({"kind":"found","revision":4,"session":settled}),
            );
            assert_closed(&mut stream);
        });
        let output = fixture.run(&[
            "agent",
            "prompt",
            "alpha",
            "task",
            "--wait",
            "--client",
            CLIENT,
            "--until",
            condition,
            "--timeout",
            "6000",
            "--format=json",
        ]);
        assert!(
            handler.join().is_ok(),
            "Client observation peer failed for condition={condition}, outcome={outcome}, seen={seen:?}: status={:?}\nstdout={}\nstderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.status.success(), "{output:?}");
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &doc,
            include_str!("../../src/cli/schemas/orchbee.agent.prompt.json"),
        );
        assert_eq!(doc["version"], 1);
        assert_eq!(
            doc["result"]["client"]["status"],
            if outcome == "failed" {
                "error"
            } else {
                condition
            }
        );
        assert_eq!(doc["result"]["client"]["seen"], json!(seen));
        assert_eq!(doc["result"]["client"]["clientId"], CLIENT);
        assert_eq!(doc["result"]["wait"]["observations"], 1);
        assert_eq!(
            doc["result"]["observation"]["session"]["status"]["kind"],
            outcome
        );
        assert!(doc["result"]["matchedChange"].is_null());
    }
}
#[test]
fn prompt_client_loss_before_input_or_after_acceptance_never_replays() {
    for case in ["before", "after", "deadline"] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_agent_reply(&mut stream, idle());
            prompt_preflight(&mut stream, idle());
            checkpoint(&mut stream);
            if case != "before" {
                attention(&mut stream, 2, 11, Some(true));
                accept_prompt(&mut stream, "task");
                current(&mut stream, 2);
            }
            let get = request(&mut stream);
            assert!(matches!(get.action, ClientViewAction::Get { .. }));
            if case != "deadline" {
                reply(&mut stream, &get, json!({"kind":"not-found"}));
            }
            assert_closed(&mut stream);
        });
        let output = fixture.run(&[
            "agent",
            "prompt",
            "alpha",
            "task",
            "--wait",
            "--client",
            CLIENT,
            "--timeout",
            "6000",
            "--format=json",
        ]);
        assert!(
            handler.join().is_ok(),
            "Client loss peer failed for {case}: status={:?}\nstdout={}\nstderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!output.status.success());
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &doc,
            include_str!("../../src/cli/schemas/orchbee.cli.failure.agent-prompt.json"),
        );
        assert_eq!(doc["failure"]["details"]["clientId"], CLIENT);
        assert_eq!(
            doc["failure"]["kind"],
            if case == "deadline" {
                "timeout"
            } else {
                "not-found"
            }
        );
        if case == "before" {
            assert!(doc["failure"]["details"].get("submission").is_none());
        } else {
            assert_eq!(
                doc["failure"]["details"]["submission"]["result"]["inputStatus"],
                "accepted"
            );
        }
    }
}
