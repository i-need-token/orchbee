//! Windows prompt preflight, delivery and activity/condition observation.
#![cfg(windows)]
#[path = "cli_windows_agent_prompt/client_wait.rs"]
mod client_wait;
#[path = "support/windows_agent_peer.rs"]
mod peer_support;
#[path = "cli_windows_agent_prompt/start.rs"]
mod start;
use orchbee_client_server_protocol::*;
use orchbee_local_ipc::{LocalListener, LocalStream};
use peer_support::*;
use serde_json::{Value, json};

fn prompt_preflight(stream: &mut LocalStream, detail: Value) {
    let accepted = detail["snapshot"]["pendingRequests"]
        .as_array()
        .unwrap()
        .is_empty()
        && detail["snapshot"]["observation"]["freshness"] == "fresh"
        && !matches!(
            detail["status"]["kind"].as_str(),
            Some("unknown" | "needs-attention")
        );
    runtime_preflight(stream, detail);
    if accepted {
        management_reply(stream, 2, json!({"phase":"unmanaged"}));
    }
}

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
        let handler = peer(&fixture, listener, move |mut stream| {
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
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_agent_reply(&mut stream, idle());
            let mut next = if blocked {
                session_detail("sess-alpha")
            } else {
                idle()
            };
            if !blocked {
                next["snapshot"]["observation"]["freshness"] = json!("stale");
            }
            prompt_preflight(&mut stream, next);
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
    let handler = peer(&fixture, listener, move |mut stream| {
        bound_agent_reply(&mut stream, idle());
        prompt_preflight(&mut stream, idle());
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
    let handler = peer(&fixture, listener, move |mut stream| {
        bound_agent_reply(&mut stream, idle());
        prompt_preflight(&mut stream, idle());
        checkpoint(&mut stream);
        accept_prompt(&mut stream, "task");
        checkpoint(&mut stream);
        read_reply(
            &mut stream,
            "get",
            json!({"kind":"found", "revision":2, "session":idle()}),
        );
        checkpoint(&mut stream);
        read_reply(
            &mut stream,
            "get",
            json!({"kind":"found", "revision":3, "session":completed()}),
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
        json!(["done"])
    );
}

#[test]
fn prompt_lost_send_reply_is_unknown_and_a_later_read_loss_keeps_accepted_input() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    for after_send in [false, true] {
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_agent_reply(&mut stream, idle());
            prompt_preflight(&mut stream, idle());
            checkpoint(&mut stream);
            if after_send {
                accept_prompt(&mut stream, "task");
                let request =
                    orchbee_agent_observation_protocol::read_agent_request(&mut stream).unwrap();
                assert!(matches!(
                    request,
                    orchbee_agent_observation_protocol::AgentRequest::Changes(_)
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
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_agent_reply(&mut stream, idle());
            prompt_preflight(&mut stream, idle());
            checkpoint(&mut stream);
            accept_prompt(&mut stream, "task");
            while let Ok(request) = read_agent_request(&mut stream) {
                if let AgentRequest::Changes(request) = &request {
                    let reply = AgentResult::Changes(serde_json::from_value(json!({"kind":"agent.session.changes.result","requestId":request.request_id(),"result":{"outcome":current_changes()}})).unwrap());
                    if write_agent_result(&mut stream, &reply).is_err() {
                        break;
                    }
                    continue;
                }
                let AgentRequest::Read(request) = request else {
                    panic!("no alias retarget or send replay");
                };
                assert_eq!(json!(request)["sessionId"], "sess-alpha");
                let reply = AgentResult::Read(serde_json::from_value(json!({"kind":"agent.session.get.result", "requestId":request.request_id(), "result":{"outcome":{"kind":"found","revision":2,"session":idle()}}})).unwrap());
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
    let handler = peer(&fixture, listener, move |mut stream| {
        bound_agent_reply(&mut stream, idle());
        prompt_preflight(&mut stream, idle());
        checkpoint(&mut stream);
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
    let handler = peer(&fixture, listener, move |mut stream| {
        let mut detail = session_detail("sess-alpha");
        detail["snapshot"]["pendingRequests"] = json!([]);
        detail["status"] = json!({"kind":"working", "phase":"working", "activity":"tool"});
        bound_agent_reply(&mut stream, detail.clone());
        prompt_preflight(&mut stream, detail.clone());
        checkpoint(&mut stream);
        accept_prompt(&mut stream, "task");
        checkpoint(&mut stream);
        read_reply(
            &mut stream,
            "get",
            json!({"kind":"found", "revision":2, "session":detail}),
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
        let handler = peer(&fixture, listener, move |mut stream| {
            let mut detail = idle();
            detail["snapshot"]["providerId"] = json!("codex");
            bound_agent_reply(&mut stream, detail.clone());
            prompt_preflight(&mut stream, detail);
            if partial {
                checkpoint(&mut stream);
            }
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
    let handler = peer(&fixture, listener, move |mut stream| {
        bound_snapshot_reply(&mut stream);
        AgentProjectionFixture::canonical_with_current_incarnation(
            bound_snapshot(9),
            1,
            &[idle()],
            "11111111111111111111111111111111",
            "33333333333333333333333333333333",
            "66666666666666666666666666666666",
            "77777777777777777777777777777777",
        )
        .read(&mut stream, false);
        resolve_reply(
            &mut stream,
            json!({"kind":"resolved","sessionId":"sess-alpha","resolvedBy":"session-id"}),
        );
        read_reply(
            &mut stream,
            "get",
            json!({"kind":"found","revision":1,"session":idle()}),
        );
        prompt_preflight(&mut stream, idle());
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

fn current_changes() -> Value {
    json!({"kind":"current","cursor":format!("v1.{}.{}","A".repeat(59),"A".repeat(43)),"revision":2})
}
fn changes_reply(stream: &mut LocalStream, outcome: Value) {
    use orchbee_agent_observation_protocol::{
        AgentRequest, AgentResult, read_agent_request, write_agent_result,
    };
    let AgentRequest::Changes(request) = read_agent_request(stream).unwrap() else {
        panic!("changes")
    };
    write_agent_result(stream,&AgentResult::Changes(serde_json::from_value(json!({"kind":"agent.session.changes.result","requestId":request.request_id(),"result":{"outcome":outcome}})).unwrap())).unwrap();
}
fn checkpoint(stream: &mut LocalStream) {
    changes_reply(stream, current_changes());
}

fn history_event(revision: u64, sequence: u64, kind: &str, transition: &str) -> Value {
    let status = match kind {
        "working" => json!({"kind":"working","activity":null,"phase":"working"}),
        "needs-attention" => session_detail("sess-alpha")["status"].clone(),
        _ => json!({"kind":kind}),
    };
    let detail = idle();
    json!({"revision":revision,"transition":transition,"session":{
        "sessionId":"sess-alpha","providerId":detail["snapshot"]["providerId"],"parentSessionId":null,
        "pendingRequestCount":if kind=="needs-attention" {1} else {0},"lifecycle":"live",
        "observation":{"generation":detail["snapshot"]["observation"]["generation"],"sequence":sequence,"freshness":"fresh","fidelity":"rich","source":"native"},"status":status
    }})
}
fn history_page(stream: &mut LocalStream, changes: Value, head: u64) {
    changes_reply(
        stream,
        json!({"kind":"changes","cursor":current_changes()["cursor"],"observedThroughRevision":head,"runtimeRevision":head,"hasMore":false,"changes":changes}),
    );
}
#[test]
fn prompt_history_opens_the_activity_gate_but_completion_must_remain_current() {
    for case in [
        "working-idle",
        "completed-working",
        "blocked-working",
        "metadata-idle",
    ] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_agent_reply(&mut stream, idle());
            prompt_preflight(&mut stream, idle());
            checkpoint(&mut stream);
            accept_prompt(&mut stream, "task");
            let (changes, head) = match case {
                "completed-working" => (
                    json!([
                        history_event(3, 12, "working", "turn-started"),
                        history_event(4, 13, "succeeded", "turn-finished"),
                        history_event(5, 14, "working", "turn-started")
                    ]),
                    5,
                ),
                "blocked-working" => (
                    json!([
                        history_event(3, 12, "needs-attention", "attention-requested"),
                        history_event(4, 13, "working", "turn-started")
                    ]),
                    4,
                ),
                "metadata-idle" => (
                    json!([history_event(
                        3,
                        12,
                        "idle",
                        "presentation-visibility-changed"
                    )]),
                    3,
                ),
                _ => (
                    json!([
                        history_event(3, 12, "working", "turn-started"),
                        history_event(4, 13, "idle", "session-idle-observed")
                    ]),
                    4,
                ),
            };
            history_page(&mut stream, changes, head);
            let mut current = idle();
            current["snapshot"]["observation"]["sequence"] = json!(head + 9);
            if matches!(case, "completed-working" | "blocked-working") {
                current["status"] = json!({"kind":"working","phase":"working","activity":null});
                current["snapshot"]["turn"] =
                    session_detail("sess-alpha")["snapshot"]["turn"].clone();
            }
            read_reply(
                &mut stream,
                "get",
                json!({"kind":"found","revision":head,"session":current}),
            );
            if case == "completed-working" {
                history_page(
                    &mut stream,
                    json!([history_event(6, 15, "succeeded", "turn-finished")]),
                    6,
                );
                let mut current = completed();
                current["snapshot"]["observation"]["sequence"] = json!(15);
                read_reply(
                    &mut stream,
                    "get",
                    json!({"kind":"found","revision":6,"session":current}),
                );
            } else if case == "metadata-idle" {
                history_page(
                    &mut stream,
                    json!([
                        history_event(4, 13, "working", "turn-started"),
                        history_event(5, 14, "idle", "session-idle-observed")
                    ]),
                    5,
                );
                let mut current = idle();
                current["snapshot"]["observation"]["sequence"] = json!(14);
                read_reply(
                    &mut stream,
                    "get",
                    json!({"kind":"found","revision":5,"session":current}),
                );
            }
            assert_closed(&mut stream);
        });
        let condition = match case {
            "completed-working" => "done",
            "blocked-working" => "blocked",
            _ => "idle",
        };
        let output = fixture.run(&[
            "agent",
            "prompt",
            "alpha",
            "task",
            "--wait",
            "--until",
            condition,
            "--timeout",
            "6000",
            "--format=json",
        ]);
        handler.join().unwrap();
        assert!(output.status.success(), "{output:?}");
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &doc,
            include_str!("../src/cli/schemas/orchbee.agent.prompt.json"),
        );
        assert_eq!(doc["version"], 7);
        assert_eq!(
            doc["result"]["wait"]["observations"],
            if matches!(case, "completed-working" | "metadata-idle") {
                2
            } else {
                1
            }
        );
        if case == "blocked-working" {
            assert_eq!(
                doc["result"]["matchedChange"]["change"]["session"]["status"]["kind"],
                "needs-attention"
            );
            assert_eq!(
                doc["result"]["observation"]["session"]["status"]["kind"],
                "working"
            );
        } else {
            assert!(doc["result"]["matchedChange"].is_null());
        }
    }
}
#[test]
fn prompt_checkpoint_failure_prevents_input_and_lost_history_preserves_submission() {
    for case in ["checkpoint-denied", "reset", "foreign"] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_agent_reply(&mut stream, idle());
            prompt_preflight(&mut stream, idle());
            if case == "checkpoint-denied" {
                changes_reply(
                    &mut stream,
                    json!({"kind":"rejected","reason":"read-denied"}),
                );
            } else {
                checkpoint(&mut stream);
                accept_prompt(&mut stream, "task");
                changes_reply(
                    &mut stream,
                    if case == "reset" {
                        json!({"kind":"reset-required","cursor":current_changes()["cursor"],"revision":900})
                    } else {
                        json!({"kind":"rejected","reason":"cursor-invalid"})
                    },
                );
            }
            assert_closed(&mut stream);
        });
        let output = fixture.run(&[
            "agent",
            "prompt",
            "alpha",
            "task",
            "--wait",
            "--timeout",
            "6000",
            "--format=json",
        ]);
        handler.join().unwrap();
        assert!(!output.status.success());
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &doc,
            include_str!("../src/cli/schemas/orchbee.cli.failure.agent-prompt.json"),
        );
        if case == "checkpoint-denied" {
            assert_eq!(doc["failure"]["kind"], "denied");
            assert!(doc["failure"]["details"].get("submission").is_none());
        } else {
            assert_eq!(
                doc["failure"]["details"]["reason"],
                if case == "reset" {
                    "history-lost"
                } else {
                    "cursor-invalid"
                }
            );
            assert_eq!(
                doc["failure"]["details"]["submission"]["result"]["inputStatus"],
                "accepted"
            );
        }
    }
}

#[test]
fn prompt_refuses_pending_startup_and_failed_management_reads_before_any_input() {
    use orchbee_agent_observation_protocol::{
        AgentRequest, AgentResult, read_agent_request, write_agent_result,
    };
    for case in ["pending", "denied", "wrong-session", "behind"] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_agent_reply(&mut stream, idle());
            runtime_preflight(&mut stream, idle());
            let AgentRequest::Launch(request) = read_agent_request(&mut stream).unwrap() else {
                panic!("management before input")
            };
            assert_eq!(json!(request)["kind"], "agent.launch.status.request");
            let outcome = if case == "denied" {
                json!({"kind":"rejected","reason":"read-denied"})
            } else {
                let management = if case == "pending" {
                    json!({"phase":"pending","blocked":false,"reservation":{
                        "reservationId":"11111111111111111111111111111111","name":"alpha","providerId":"claude-code","remainingMs":9000,"settleRemainingMs":0,
                        "binding":{"presentationId":"22222222222222222222222222222222","generation":"33333333333333333333333333333333","hostEpoch":"66666666666666666666666666666666","hostTerminalId":"77777777777777777777777777777777"}
                    }})
                } else {
                    json!({"phase":"unmanaged"})
                };
                json!({"kind":"status","sessionId":if case=="wrong-session" {"other"} else {"sess-alpha"},"revision":if case=="behind" {1} else {2},"management":management})
            };
            write_agent_result(&mut stream,&AgentResult::Launch(serde_json::from_value(json!({"kind":"agent.launch.status.result","requestId":request.request_id(),"result":{"outcome":outcome}})).unwrap())).unwrap();
            assert_closed(&mut stream);
        });
        let output = fixture.run(&["agent", "prompt", "alpha", "task", "--format=json"]);
        handler.join().unwrap();
        assert!(!output.status.success(), "{output:?}");
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        if case == "pending" {
            validate(
                &doc,
                include_str!("../src/cli/schemas/orchbee.cli.failure.agent-prompt.json"),
            );
            assert_eq!(doc["failure"]["details"]["reason"], "agent-launch-pending");
        } else {
            validate(
                &doc,
                include_str!("../src/cli/schemas/orchbee.cli.failure.agent.json"),
            );
            assert_eq!(doc["version"], 3);
            assert_eq!(doc["failure"]["details"]["operation"], "management");
        }
        assert!(doc["failure"]["details"].get("submission").is_none());
    }
}
