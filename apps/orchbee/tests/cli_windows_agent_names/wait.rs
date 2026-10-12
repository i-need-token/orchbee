//! Windows event-aware wait over actual named pipes and CLI processes.
use super::*;

fn cursor() -> String {
    format!("v1.{}.{}", "A".repeat(59), "A".repeat(43))
}

fn changes_reply(stream: &mut orchbee_local_ipc::LocalStream, outcome: Value) -> String {
    let AgentRequest::Changes(request) = read_agent_request(stream).unwrap() else {
        panic!("changes request")
    };
    assert_eq!(json!(request)["limit"], 100);
    let id = request.request_id().as_str().to_owned();
    write_agent_result(
        stream,
        &AgentResult::Changes(
            serde_json::from_value(json!({
                "kind":"agent.session.changes.result", "requestId":id, "result":{"outcome":outcome}
            }))
            .unwrap(),
        ),
    )
    .unwrap();
    id
}

pub(super) fn checkpoint(stream: &mut orchbee_local_ipc::LocalStream, revision: u64) {
    changes_reply(
        stream,
        json!({"kind":"current","cursor":cursor(),"revision":revision}),
    );
}

fn working() -> Value {
    let mut detail = session_detail("sess-alpha");
    detail["snapshot"]["pendingRequests"] = json!([]);
    detail["status"] = json!({"kind":"working","activity":null,"phase":"working"});
    detail
}

#[test]
fn wait_keeps_a_transient_completion_separate_from_the_current_working_sample() {
    let fixture = Fixture::new();
    let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
    let handler = peer(&fixture, listener, move |mut stream| {
        checkpoint(&mut stream, 7);
        read_reply(
            &mut stream,
            "get",
            json!({"kind":"found","revision":7,"session":working()}),
        );
        let session = |sequence, status| {
            json!({
                "lifecycle":"live","observation":{"fidelity":"rich","freshness":"fresh","generation":3,"sequence":sequence,"source":"native"},
                "parentSessionId":null,"pendingRequestCount":0,"providerId":"acme.agent","sessionId":"sess-alpha","status":status
            })
        };
        changes_reply(
            &mut stream,
            json!({"kind":"changes","cursor":cursor(),"observedThroughRevision":9,"runtimeRevision":9,"hasMore":false,"changes":[
                {"revision":8,"transition":"turn-finished","session":session(12,json!({"kind":"succeeded"}))},
                {"revision":9,"transition":"turn-started","session":session(13,json!({"kind":"working","activity":null,"phase":"working"}))}
            ]}),
        );
        read_reply(
            &mut stream,
            "get",
            json!({"kind":"found","revision":9,"session":working()}),
        );
        assert_closed(&mut stream);
    });
    let output = fixture.run(&[
        "agent",
        "wait",
        "--session",
        "sess-alpha",
        "--until",
        "done",
        "--timeout",
        "2000",
        "--format",
        "json",
    ]);
    join_peer(handler, &output);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    validate(
        &document,
        include_str!("../../src/cli/schemas/orchbee.agent.wait.json"),
    );
    assert_eq!(document["version"], 7);
    assert_eq!(document["result"]["revision"], 9);
    assert_eq!(document["result"]["session"]["status"]["kind"], "working");
    assert_eq!(document["result"]["matchedChange"]["revision"], 8);
    assert_eq!(
        document["result"]["matchedChange"]["session"]["status"]["kind"],
        "succeeded"
    );
    assert_eq!(document["result"]["matchedConditions"], json!(["done"]));
}

#[test]
fn wait_refuses_lost_history_without_restarting_or_publishing_a_match() {
    let fixture = Fixture::new();
    for (kind, reason) in [
        ("reset-required", "history-lost"),
        ("rejected", "cursor-invalid"),
    ] {
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            checkpoint(&mut stream, 7);
            read_reply(
                &mut stream,
                "get",
                json!({"kind":"found","revision":7,"session":working()}),
            );
            changes_reply(
                &mut stream,
                if kind == "reset-required" {
                    json!({"kind":kind,"cursor":cursor(),"revision":999})
                } else {
                    json!({"kind":kind,"reason":"cursor-invalid"})
                },
            );
            assert_closed(&mut stream);
        });
        let output = fixture.run(&[
            "agent",
            "wait",
            "--session",
            "sess-alpha",
            "--until",
            "done",
            "--timeout",
            "2000",
            "--format",
            "json",
        ]);
        join_peer(handler, &output);
        assert!(!output.status.success());
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &document,
            include_str!("../../src/cli/schemas/orchbee.cli.failure.agent.json"),
        );
        assert_eq!(document["failure"]["details"]["reason"], reason);
        assert!(document.get("result").is_none());
    }
}

#[test]
fn wait_total_deadline_covers_the_initial_changes_checkpoint() {
    let fixture = Fixture::deadline_sensitive();
    let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
    let handler = peer(&fixture, listener, move |mut stream| {
        assert!(matches!(
            read_agent_request(&mut stream).unwrap(),
            AgentRequest::Changes(_)
        ));
        assert_closed(&mut stream);
    });
    let output = fixture.run(&[
        "agent",
        "wait",
        "--session",
        "sess-alpha",
        "--timeout",
        "3000",
        "--format",
        "json",
    ]);
    join_peer(handler, &output);
    assert_eq!(output.status.code(), Some(7));
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    validate(
        &document,
        include_str!("../../src/cli/schemas/orchbee.cli.failure.agent.json"),
    );
    assert_eq!(document["failure"]["details"]["reason"], "wait-deadline");
    assert_eq!(document["failure"]["details"]["completedObservations"], 0);
}
