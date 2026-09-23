//! Windows prompt preflight, delivery and activity/condition observation.
#![cfg(windows)]
#[path = "support/windows_agent_peer.rs"]
mod peer_support;
#[path = "cli_windows_agent_prompt/start.rs"]
mod start;
use orchbee_client_server_protocol::*;
use orchbee_local_ipc::{LocalListener, LocalStream};
use peer_support::*;
use serde_json::{Value, json};

fn idle() -> Value {
    let mut detail = session_detail("sess-alpha");
    detail["snapshot"]["turn"] = json!({"kind":"idle"});
    detail["snapshot"]["pendingRequests"] = json!([]);
    detail["status"] = json!({"kind":"idle"});
    detail
}
fn completed() -> Value {
    let mut detail = idle();
    detail["snapshot"]["observation"]["sequence"] = json!(12);
    detail["snapshot"]["lastOutcome"] =
        json!({"kind":"succeeded", "eventId":"finished-12", "turnId":"turn-new"});
    detail["status"] = json!({"kind":"succeeded"});
    detail
}
fn prompt_request(stream: &mut LocalStream, expected: &str, codex: bool) -> TerminalSendRequest {
    let ClientMessage::TerminalSend(request) = read_client_message(stream).unwrap() else {
        panic!("single send");
    };
    assert_eq!(
        request.terminal_id().as_str(),
        "33333333333333333333333333333333"
    );
    let records = request.records().as_slice();
    assert_eq!(records.len(), if codex { 3 } else { 2 });
    let SendRecord::Paste { content } = &records[0] else {
        panic!("literal paste");
    };
    assert_eq!(content.as_bytes(), expected.as_bytes());
    assert!(matches!(
        records.last().unwrap(),
        SendRecord::Key {
            key: SendKey::Enter
        }
    ));
    if codex {
        assert!(matches!(
            &records[1],
            SendRecord::Key {
                key: SendKey::Right
            }
        ));
    }
    request
}
fn accept_prompt(stream: &mut LocalStream, expected: &str) {
    let request = prompt_request(stream, expected, false);
    let target = SendTarget::new(
        std::num::NonZeroU64::MIN,
        HostEpoch::parse("66666666666666666666666666666666").unwrap(),
        HostTerminalId::parse("77777777777777777777777777777777").unwrap(),
    );
    write_server_message(
        stream,
        &ServerMessage::TerminalSendResult(
            TerminalSendResult::new(
                request.request_id().clone(),
                Some(target),
                TerminalSendOutcome::Accepted {
                    progress: SendProgress::complete(2, expected.len() as u64 + 1),
                    cleanup: SendCleanupStatus::Confirmed,
                },
            )
            .unwrap(),
        ),
    )
    .unwrap();
}

#[test]
fn prompt_refuses_attention_and_nonfresh_state_before_input() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    for blocked in [true, false] {
        let mut detail = if blocked {
            session_detail("sess-alpha")
        } else {
            idle()
        };
        if !blocked {
            detail["snapshot"]["observation"]["freshness"] = json!("stale");
        }
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let handler = peer(listener, move |mut stream| {
            bound_agent_reply(&mut stream, detail);
            assert_closed(&mut stream);
        });
        let output = fixture.run(&[
            "agent",
            "prompt",
            "--name",
            "alpha",
            "private prompt",
            "--format",
            "json",
        ]);
        handler.join().unwrap();
        assert_eq!(
            output.status.code(),
            Some(3),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &document,
            include_str!("../src/cli/schemas/orchbee.cli.failure.agent-prompt.json"),
        );
        assert_eq!(
            document["failure"]["details"]["reason"],
            if blocked {
                "agent-blocked"
            } else {
                "agent-not-ready"
            }
        );
        assert!(!String::from_utf8_lossy(&output.stdout).contains("private prompt"));
    }
}

#[test]
fn prompt_rechecks_freshness_and_blockers_after_process_preflight() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    for blocked in [false, true] {
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let handler = peer(listener, move |mut stream| {
            bound_agent_reply(&mut stream, idle());
            let mut next = if blocked {
                session_detail("sess-alpha")
            } else {
                idle()
            };
            if !blocked {
                next["snapshot"]["observation"]["freshness"] = json!("stale");
            }
            runtime_preflight(&mut stream, next);
            assert_closed(&mut stream);
        });
        let output = fixture.run(&[
            "agent",
            "prompt",
            "alpha",
            "private text",
            "--format",
            "json",
        ]);
        handler.join().unwrap();
        assert_eq!(output.status.code(), Some(3));
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &doc,
            include_str!("../src/cli/schemas/orchbee.cli.failure.agent-prompt.json"),
        );
        assert_eq!(
            doc["failure"]["details"]["reason"],
            if blocked {
                "agent-blocked"
            } else {
                "agent-not-ready"
            }
        );
    }
}

#[test]
fn prompt_pastes_literal_text_once_and_keeps_the_terminal_receipt() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let text = "private line one\n中文 'quotes' $(literal)";
    let handler = peer(listener, move |mut stream| {
        bound_agent_reply(&mut stream, idle());
        runtime_preflight(&mut stream, idle());
        accept_prompt(&mut stream, text);
        assert_closed(&mut stream);
    });
    let output = fixture.run(&["agent", "prompt", "alpha", text, "--format", "json"]);
    handler.join().unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    validate(
        &document,
        include_str!("../src/cli/schemas/orchbee.agent.prompt.json"),
    );
    assert_eq!(
        document["result"]["submission"]["result"]["inputStatus"],
        "accepted"
    );
    assert!(document["result"]["wait"].is_null());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("private line"));
}

#[test]
fn prompt_wait_rejects_old_idle_but_accepts_a_new_completed_turn_when_working_was_missed() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let handler = peer(listener, move |mut stream| {
        bound_agent_reply(&mut stream, idle());
        runtime_preflight(&mut stream, idle());
        accept_prompt(&mut stream, "task");
        read_reply(
            &mut stream,
            "get",
            json!({"kind":"found", "revision":1, "session":idle()}),
        );
        read_reply(
            &mut stream,
            "get",
            json!({"kind":"found", "revision":2, "session":completed()}),
        );
        assert_closed(&mut stream);
    });
    let output = fixture.run(&[
        "agent", "prompt", "--name", "alpha", "task", "--wait", "--format", "json",
    ]);
    handler.join().unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    validate(
        &document,
        include_str!("../src/cli/schemas/orchbee.agent.prompt.json"),
    );
    assert_eq!(document["result"]["wait"]["observations"], 2);
    assert_eq!(
        document["result"]["wait"]["matchedConditions"],
        json!(["succeeded"])
    );
}

#[test]
fn prompt_lost_send_reply_is_unknown_and_a_later_read_loss_keeps_accepted_input() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    for after_send in [false, true] {
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let handler = peer(listener, move |mut stream| {
            bound_agent_reply(&mut stream, idle());
            runtime_preflight(&mut stream, idle());
            if after_send {
                accept_prompt(&mut stream, "task");
                let request =
                    orchbee_agent_observation_protocol::read_agent_request(&mut stream).unwrap();
                assert!(matches!(
                    request,
                    orchbee_agent_observation_protocol::AgentRequest::Read(_)
                ));
            } else {
                assert!(matches!(
                    read_client_message(&mut stream).unwrap(),
                    ClientMessage::TerminalSend(_)
                ));
            }
        });
        let output = fixture.run(&[
            "agent", "prompt", "alpha", "task", "--wait", "--format", "json",
        ]);
        handler.join().unwrap();
        assert_eq!(output.status.code(), Some(if after_send { 5 } else { 8 }));
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &document,
            if after_send {
                include_str!("../src/cli/schemas/orchbee.cli.failure.agent-prompt.json")
            } else {
                include_str!("../src/cli/schemas/orchbee.cli.failure.json")
            },
        );
        if after_send {
            assert_eq!(
                document["failure"]["details"]["submission"]["result"]["inputStatus"],
                "accepted"
            );
        }
    }
}

#[test]
fn prompt_deadlines_distinguish_stalled_activity_from_an_earlier_caller_timeout() {
    use orchbee_agent_observation_protocol::{
        AgentRequest, AgentResult, read_agent_request, write_agent_result,
    };
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    for (timeout, reason) in [
        ("15000", "prompt-stalled"),
        ("3000", "wait-deadline"),
        ("forever", "prompt-stalled"),
    ] {
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let handler = peer(listener, move |mut stream| {
            bound_agent_reply(&mut stream, idle());
            runtime_preflight(&mut stream, idle());
            accept_prompt(&mut stream, "task");
            while let Ok(request) = read_agent_request(&mut stream) {
                let AgentRequest::Read(request) = request else {
                    panic!("no alias retarget or send replay");
                };
                assert_eq!(json!(request)["sessionId"], "sess-alpha");
                let reply = AgentResult::Read(serde_json::from_value(json!({"kind":"agent.session.get.result", "requestId":request.request_id(), "result":{"outcome":{"kind":"found","revision":1,"session":idle()}}})).unwrap());
                if write_agent_result(&mut stream, &reply).is_err() {
                    break;
                }
            }
        });
        let mut args = vec![
            "agent", "prompt", "alpha", "task", "--wait", "--format", "json",
        ];
        if timeout == "forever" {
            args.push("--forever");
        } else {
            args.extend(["--timeout", timeout]);
        }
        let output = fixture.run(&args);
        handler.join().unwrap();
        assert_eq!(
            output.status.code(),
            Some(7),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &document,
            include_str!("../src/cli/schemas/orchbee.cli.failure.agent-prompt.json"),
        );
        assert_eq!(document["failure"]["details"]["reason"], reason);
        assert_eq!(
            document["failure"]["details"]["submission"]["result"]["inputStatus"],
            "accepted"
        );
    }
}

#[test]
fn prompt_partial_delivery_never_starts_waiting_or_resubmits_the_prompt() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let handler = peer(listener, move |mut stream| {
        bound_agent_reply(&mut stream, idle());
        runtime_preflight(&mut stream, idle());
        let ClientMessage::TerminalSend(request) = read_client_message(&mut stream).unwrap() else {
            panic!("send");
        };
        let target = SendTarget::new(
            std::num::NonZeroU64::MIN,
            HostEpoch::parse("66666666666666666666666666666666").unwrap(),
            HostTerminalId::parse("77777777777777777777777777777777").unwrap(),
        );
        write_server_message(
            &mut stream,
            &ServerMessage::TerminalSendResult(
                TerminalSendResult::new(
                    request.request_id().clone(),
                    Some(target),
                    TerminalSendOutcome::Unavailable {
                        reason: SendUnavailableReason::InputBudgetExhausted,
                        progress: SendProgress::stopped(1, 4),
                        cleanup: SendCleanupStatus::Confirmed,
                    },
                )
                .unwrap(),
            ),
        )
        .unwrap();
        assert_closed(&mut stream);
    });
    let output = fixture.run(&[
        "agent", "prompt", "alpha", "task", "--wait", "--format", "json",
    ]);
    handler.join().unwrap();
    assert_eq!(output.status.code(), Some(9));
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    validate(
        &document,
        include_str!("../src/cli/schemas/orchbee.terminal.send.json"),
    );
    assert_eq!(document["result"]["recordsAccepted"], 1);
}

#[test]
fn prompt_to_an_already_working_agent_does_not_promise_a_new_turn() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let handler = peer(listener, move |mut stream| {
        let mut detail = session_detail("sess-alpha");
        detail["snapshot"]["pendingRequests"] = json!([]);
        detail["status"] = json!({"kind":"working", "phase":"working", "activity":"tool"});
        bound_agent_reply(&mut stream, detail.clone());
        runtime_preflight(&mut stream, detail.clone());
        accept_prompt(&mut stream, "task");
        read_reply(
            &mut stream,
            "get",
            json!({"kind":"found", "revision":1, "session":detail}),
        );
        assert_closed(&mut stream);
    });
    let output = fixture.run(&[
        "agent",
        "prompt",
        "alpha",
        "task",
        "--wait",
        "--until",
        "working",
        "--forever",
        "--format",
        "json",
    ]);
    handler.join().unwrap();
    assert_eq!(output.status.code(), Some(0));
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    validate(
        &document,
        include_str!("../src/cli/schemas/orchbee.agent.prompt.json"),
    );
    assert_eq!(document["result"]["wait"]["observations"], 1);
}

#[test]
fn windows_codex_boundary_precedes_enter_and_partial_delivery_is_never_completed_by_retry() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    for partial in [false, true] {
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let handler = peer(listener, move |mut stream| {
            let mut detail = idle();
            detail["snapshot"]["providerId"] = json!("codex");
            bound_agent_reply(&mut stream, detail.clone());
            runtime_preflight(&mut stream, detail);
            let request = prompt_request(&mut stream, "literal\n中文", true);
            let target = SendTarget::new(
                std::num::NonZeroU64::MIN,
                HostEpoch::parse("66666666666666666666666666666666").unwrap(),
                HostTerminalId::parse("77777777777777777777777777777777").unwrap(),
            );
            let outcome = if partial {
                TerminalSendOutcome::Unavailable {
                    reason: SendUnavailableReason::InputBudgetExhausted,
                    progress: SendProgress::stopped(2, 16),
                    cleanup: SendCleanupStatus::Confirmed,
                }
            } else {
                TerminalSendOutcome::Accepted {
                    progress: SendProgress::complete(3, 17),
                    cleanup: SendCleanupStatus::Confirmed,
                }
            };
            write_server_message(
                &mut stream,
                &ServerMessage::TerminalSendResult(
                    TerminalSendResult::new(request.request_id().clone(), Some(target), outcome)
                        .unwrap(),
                ),
            )
            .unwrap();
            assert_closed(&mut stream);
        });
        let mut args = vec![
            "agent",
            "prompt",
            "alpha",
            "literal\n中文",
            "--format",
            "json",
        ];
        if partial {
            args.push("--wait");
        }
        let output = fixture.run(&args);
        handler.join().unwrap();
        assert_eq!(
            output.status.code(),
            Some(if partial { 9 } else { 0 }),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        let receipt = if partial {
            validate(
                &document,
                include_str!("../src/cli/schemas/orchbee.terminal.send.json"),
            );
            &document
        } else {
            validate(
                &document,
                include_str!("../src/cli/schemas/orchbee.agent.prompt.json"),
            );
            &document["result"]["submission"]
        };
        assert_eq!(receipt["result"]["records"], 3);
        assert_eq!(
            receipt["result"]["recordsAccepted"],
            if partial { 2 } else { 3 }
        );
        if partial {
            assert_eq!(receipt["result"]["firstStoppedRecord"], 2);
        }
    }
}

#[test]
fn panel_prompt_submits_to_the_primary_binding() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let handler = peer(listener, move |mut stream| {
        bound_snapshot_reply(&mut stream);
        resolve_reply(
            &mut stream,
            json!({"kind":"resolved","sessionId":"sess-alpha","resolvedBy":"presentation"}),
        );
        read_reply(
            &mut stream,
            "get",
            json!({"kind":"found","revision":1,"session":idle()}),
        );
        runtime_preflight(&mut stream, idle());
        accept_prompt(&mut stream, "panel prompt");
        assert_closed(&mut stream);
    });
    let output = fixture.run(&[
        "agent",
        "prompt",
        "--panel",
        "@panel:222",
        "panel prompt",
        "--format",
        "json",
    ]);
    handler.join().unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
    validate(
        &doc,
        include_str!("../src/cli/schemas/orchbee.agent.prompt.json"),
    );
    assert_eq!(
        doc["result"]["submission"]["result"]["terminalId"],
        "33333333333333333333333333333333"
    );
}
