//! Process-level startup effects, prompt delivery and sample recovery on Windows pipes.
use super::peer_support;
use orchbee_client_server_protocol::*;
use orchbee_local_ipc::{LocalListener, LocalStream};
use peer_support::*;
use serde_json::{Value, json};

const PANEL: &str = "22222222222222222222222222222222";
// `amp` is a shared alias, so only an install root proves the Agent.
const AMP: &str = r"C:\Users\orchbee\.amp\bin\amp.exe";
/// The terminal the peer answers process samples for.
const TERMINAL: &str = "33333333333333333333333333333333";

fn input(stream: &mut LocalStream, outcome: TerminalSendOutcome) {
    let ClientMessage::TerminalSend(request) = read_client_message(stream).unwrap() else {
        panic!("one terminal send");
    };
    // The start line is typed only at a prompt the Host confirms; it is never sent ungated.
    assert_eq!(request.precondition(), SendPrecondition::IdlePrompt);
    let records = request.records().as_slice();
    assert_eq!(records.len(), 2);
    let SendRecord::Paste { content } = &records[0] else {
        panic!("literal command");
    };
    // An argument-free start types only the fixed provider command into cmd.
    assert_eq!(content.as_bytes(), b"amp");
    assert!(matches!(
        records[1],
        SendRecord::Stroke(stroke) if stroke.key() == orchbee_client_server_protocol::SendStrokeKey::Named(SendKey::Enter) && stroke.modifiers() == 0
    ));
    write_server_message(
        stream,
        &ServerMessage::TerminalSendResult(
            TerminalSendResult::new(
                request.request_id().clone(),
                Some(SendTarget::new(
                    std::num::NonZeroU64::MIN,
                    HostEpoch::parse(&"06".repeat(16)).unwrap(),
                    HostTerminalId::parse(&"07".repeat(16)).unwrap(),
                )),
                outcome,
            )
            .unwrap(),
        ),
    )
    .unwrap();
}
fn accepted() -> TerminalSendOutcome {
    TerminalSendOutcome::Accepted {
        progress: SendProgress::complete(2, 100),
        cleanup: SendCleanupStatus::Confirmed,
    }
}
fn refused(reason: SendRejectedReason) -> TerminalSendOutcome {
    TerminalSendOutcome::Rejected {
        progress: SendProgress::none(),
        cleanup: SendCleanupStatus::Confirmed,
        reason,
    }
}

/// The longest the peer waits for the CLI's next connection; a missing reconnect fails the test
/// with a message instead of blocking it.
const PEER_ACCEPT_BUDGET: std::time::Duration = std::time::Duration::from_secs(30);

/// Accepts the CLI's next connection by polling, so the peer never blocks past
/// [`PEER_ACCEPT_BUDGET`] or past `done`. Returns `None` once either has passed.
fn accept_before(
    listener: &LocalListener,
    done: &std::sync::atomic::AtomicBool,
) -> Option<LocalStream> {
    listener.set_nonblocking(true).unwrap();
    let until = std::time::Instant::now() + PEER_ACCEPT_BUDGET;
    loop {
        match listener.accept() {
            Ok((stream, _)) => return Some(stream),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("peer accept failed: {error}"),
        }
        if done.load(std::sync::atomic::Ordering::Acquire) || std::time::Instant::now() >= until {
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
fn run(script: impl FnOnce(LocalStream) + Send + 'static) -> (std::process::Output, Value) {
    run_answering(
        "30000",
        orchbee_workbench_client::test_support::local_client::handshake::answer_offer,
        script,
    )
}

fn run_answering(
    timeout: &str,
    answer: fn(&mut LocalStream) -> std::io::Result<()>,
    script: impl FnOnce(LocalStream) + Send + 'static,
) -> (std::process::Output, Value) {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    let peer = peer_answering(
        &fixture,
        LocalListener::bind(rendezvous.client_socket_path()).unwrap(),
        std::time::Duration::from_secs(15),
        answer,
        script,
    );
    let output = fixture.run(&[
        "agent",
        "start",
        "amp",
        "--here",
        "--panel",
        PANEL,
        "--format=json",
        "--timeout",
        timeout,
    ]);
    assert!(
        peer.join().is_ok(),
        "Agent startup peer failed: status={:?}\nstdout={}\nstderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    let schema = if output.status.success() {
        include_str!("../../src/cli/schemas/orchbee.agent.start.json")
    } else {
        include_str!("../../src/cli/schemas/orchbee.cli.failure.agent-start.json")
    };
    validate(&document, schema);
    (output, document)
}

#[test]
fn startup_busy_terminal_never_types_input() {
    let (output, document) = run(|mut stream| {
        bound_snapshot_reply(&mut stream);
        process_reply(&mut stream, vec![process(10, None, AMP)]);
        assert_closed(&mut stream);
    });
    assert_eq!(output.status.code(), Some(3));
    assert!(document["failure"]["details"]["submission"].is_null());
}

#[test]
fn startup_types_nothing_where_the_prompt_or_a_peer_cannot_confirm_it() {
    // `Busy` refuses immediately without a retry loop.
    let (output, document) = run(|mut stream| {
        bound_snapshot_reply(&mut stream);
        shell(&mut stream);
        input(
            &mut stream,
            refused(SendRejectedReason::PromptUnconfirmed(SendPromptState::Busy)),
        );
        assert_closed(&mut stream);
    });
    assert_eq!(output.status.code(), Some(3));
    let details = &document["failure"]["details"];
    assert_eq!(details["reason"], "terminal-not-idle");
    assert!(details.get("promptState").is_none());
    assert_eq!(details["submission"]["result"]["inputStatus"], "rejected");

    // `Settling` is asked again on the same connection, and `Busy` after it is final.
    let (output, document) = run(|mut stream| {
        bound_snapshot_reply(&mut stream);
        shell(&mut stream);
        for answer in [
            SendPromptState::Settling,
            SendPromptState::Settling,
            SendPromptState::Busy,
        ] {
            input(
                &mut stream,
                refused(SendRejectedReason::PromptUnconfirmed(answer)),
            );
        }
        assert_closed(&mut stream);
    });
    assert_eq!(output.status.code(), Some(3));
    let details = &document["failure"]["details"];
    assert_eq!(details["reason"], "terminal-not-idle");
    assert!(details.get("promptState").is_none());
    assert_eq!(details["submission"]["result"]["inputStatus"], "rejected");

    // Transient `Unmarked` on `--here` retries and succeeds once initial prompt marks appear.
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let transient = std::thread::spawn(move || {
        let never = std::sync::atomic::AtomicBool::new(false);
        for attempt in 0..2 {
            let Some(mut stream) = accept_before(&listener, &never) else {
                panic!("attempt {attempt}: the CLI did not reconnect");
            };
            accept_client(&mut stream);
            bound_snapshot_reply(&mut stream);
            shell(&mut stream);
            if attempt == 0 {
                input(
                    &mut stream,
                    refused(SendRejectedReason::PromptUnconfirmed(
                        SendPromptState::Unmarked,
                    )),
                );
                assert_closed(&mut stream);
            } else {
                input(&mut stream, accepted());
                // The early check samples the Agent's shell until its settle window ends.
                while try_process_sample_for(
                    &mut stream,
                    Ok(vec![
                        process(10, None, "cmd.exe"),
                        process(11, Some(10), AMP),
                    ]),
                    TERMINAL,
                ) {}
            }
        }
    });
    let output = fixture.run(&[
        "agent",
        "start",
        "amp",
        "--here",
        "--panel",
        PANEL,
        "--format=json",
        "--timeout",
        "10000",
    ]);
    transient.join().unwrap();
    let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(output.status.code(), Some(0), "{doc}");

    // Persistent `Unmarked` on `--here` retries, then stops with its details preserved.
    let persistent_fixture = Fixture::new();
    let persistent_rendezvous = persistent_fixture.rendezvous();
    let endpoint = persistent_rendezvous.client_socket_path().to_path_buf();
    let listener = LocalListener::bind(&endpoint).unwrap();
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let done_peer = done.clone();
    let persistent = std::thread::spawn(move || -> usize {
        let mut attempts = 0;
        loop {
            let Some(mut stream) = accept_before(&listener, &done_peer) else {
                break;
            };
            if done_peer.load(std::sync::atomic::Ordering::Acquire) {
                break;
            }
            attempts += 1;
            accept_client(&mut stream);
            bound_snapshot_reply(&mut stream);
            shell(&mut stream);
            input(
                &mut stream,
                refused(SendRejectedReason::PromptUnconfirmed(
                    SendPromptState::Unmarked,
                )),
            );
            assert_closed(&mut stream);
        }
        attempts
    });
    let output = persistent_fixture.run(&[
        "agent",
        "start",
        "amp",
        "--here",
        "--panel",
        PANEL,
        "--format=json",
        "--timeout",
        "6000",
    ]);
    done.store(true, std::sync::atomic::Ordering::Release);
    let attempts = persistent.join().unwrap();
    assert!(
        attempts >= 2,
        "expected bounded retry attempts, got {attempts}"
    );
    assert_eq!(output.status.code(), Some(3));
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    let details = &document["failure"]["details"];
    assert_eq!(details["reason"], "prompt-unconfirmed");
    assert_eq!(
        details.get("promptState").and_then(Value::as_str),
        Some("unmarked")
    );
    assert_eq!(details["submission"]["result"]["inputStatus"], "rejected");

    let (output, document) = run(|mut stream| {
        bound_snapshot_reply(&mut stream);
        shell(&mut stream);
        input(
            &mut stream,
            refused(SendRejectedReason::HostPromptGateUnsupported),
        );
        assert_closed(&mut stream);
    });
    assert_eq!(output.status.code(), Some(6));
    let details = &document["failure"]["details"];
    assert_eq!(document["failure"]["kind"], "incompatible");
    assert_eq!(details["cause"]["stage"], "capability");
    assert_eq!(details["cause"]["reason"], "terminal-too-old");

    let (output, document) = run_answering(
        "30000",
        orchbee_workbench_client::test_support::local_client::handshake::answer_offer_without_capabilities,
        |mut stream| {
            bound_snapshot_reply(&mut stream);
            assert_closed(&mut stream);
        },
    );
    assert_eq!(output.status.code(), Some(6));
    let details = &document["failure"]["details"];
    assert_eq!(document["failure"]["kind"], "incompatible");
    assert_eq!(details["cause"]["stage"], "capability");
    assert_eq!(details["cause"]["reason"], "session-too-old");
    assert!(details["submission"].is_null());
}

#[test]
fn startup_samples_again_after_unavailable_sample_but_preserves_permanent_host_failure() {
    let (output, document) = run(|mut stream| {
        bound_snapshot_reply(&mut stream);
        process_sample_reply(&mut stream, Err("process-sample-unavailable"));
        process_sample_reply(&mut stream, Err("process-sample-unsupported"));
        assert_closed(&mut stream);
    });
    assert_eq!(output.status.code(), Some(4));
    assert_eq!(
        document["failure"]["details"]["cause"]["reason"],
        "process-sample-unsupported"
    );
    assert!(document["failure"]["details"]["submission"].is_null());
}

#[test]
fn startup_unknown_and_partial_input_keep_their_receipts() {
    for unknown in [true, false] {
        let (output, document) = run(move |mut stream| {
            bound_snapshot_reply(&mut stream);
            shell(&mut stream);
            let outcome = if unknown {
                TerminalSendOutcome::OutcomeUnknown {
                    progress: SendProgress::stopped(1, 10),
                    cleanup: SendCleanupStatus::Unconfirmed,
                }
            } else {
                TerminalSendOutcome::Rejected {
                    progress: SendProgress::stopped(1, 10),
                    cleanup: SendCleanupStatus::Confirmed,
                    reason: SendRejectedReason::HostInputBackpressure,
                }
            };
            input(&mut stream, outcome);
            assert_closed(&mut stream);
        });
        assert_eq!(output.status.code(), Some(if unknown { 8 } else { 9 }));
        assert_eq!(
            document["failure"]["details"]["submission"]["result"]["inputStatus"],
            if unknown {
                "unknown"
            } else {
                "partial-accepted"
            }
        );
    }
}

fn shell(stream: &mut LocalStream) {
    process_reply(stream, vec![process(10, None, "cmd.exe")]);
}
fn accept_client(stream: &mut LocalStream) {
    use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .unwrap();
    stream
        .set_write_timeout(Some(std::time::Duration::from_secs(10)))
        .unwrap();
    orchbee_workbench_client::test_support::local_client::handshake::answer_offer(stream).unwrap();
    assert!(matches!(
        read_message(stream).unwrap(),
        LocalClientMessage::ClientHello
    ));
    write_message(
        stream,
        &LocalClientMessage::ClientAccepted {
            generation: std::num::NonZeroU64::new(1).unwrap(),
        },
    )
    .unwrap();
}
#[test]
fn new_tab_and_split_start_forward_cwd_and_guard_focus_before_and_after_creation() {
    use orchbee_workbench_protocol::*;

    let ctx_id = "00000000000000000000000000000001";
    let base_tab_id = "00000000000000000000000000000002";
    let new_tab_id = "00000000000000000000000000000019";
    let client_id = "77777777777777777777777777777777";
    let ready_record = move |rev: u64, view_rev: u64, panel_count: u64| {
        json!({
            "clientId": client_id,
            "heartbeat": "1",
            "viewRevision": view_rev.to_string(),
            "availability": "current",
            "view": {
                "workbenchRevision": rev.to_string(),
                "phase": "ready",
                "selection": {
                    "terminalContextId": ctx_id,
                    "tabId": base_tab_id,
                    "panelId": PANEL,
                    "zoomed": false,
                    "panelCount": panel_count,
                },
            },
        })
    };
    let reply_view =
        move |stream: &mut LocalStream, req: &WorkbenchClientViewRequest, outcome: Value| {
            write_workbench_response(
                stream,
                &WorkbenchResponse::ClientView(WorkbenchClientViewResult {
                    protocol_version: ProtocolVersion,
                    request_id: req.request_id.clone(),
                    session_id: req.session_id.clone(),
                    outcome: serde_json::from_value(outcome).unwrap(),
                }),
            )
            .unwrap();
        };

    // 1. `--focus` with no Clients refuses before creation so no Panel is created.
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let no_client = peer(&fixture, listener, move |mut stream| {
        bound_snapshot_reply(&mut stream);
        let WorkbenchRequest::ClientView(req) = read_workbench_request(&mut stream).unwrap() else {
            panic!("client list before creation")
        };
        assert!(matches!(req.action, ClientViewAction::List {}));
        reply_view(&mut stream, &req, json!({"kind": "listed", "clients": []}));
        assert_closed(&mut stream);
    });
    let output = fixture.run(&[
        "agent",
        "start",
        "amp",
        "--panel",
        PANEL,
        "--cwd",
        r"C:\proj",
        "--focus",
        "--format=json",
    ]);
    no_client.join().unwrap();
    assert_eq!(output.status.code(), Some(3));
    let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        doc["failure"]["details"]["createdPanelId"].is_null(),
        "{doc}"
    );

    // 2. `--cwd <abs>` + `--focus` on `--new-tab` succeeds and submits `ClientViewIntent::Panel`.
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let happy = std::thread::spawn(move || {
        // Connection 1: client discovery, tab creation, client observation + focus.
        let (mut stream, _) = listener.accept().unwrap();
        accept_client(&mut stream);
        bound_snapshot_reply(&mut stream);
        let WorkbenchRequest::ClientView(req) = read_workbench_request(&mut stream).unwrap() else {
            panic!("client list")
        };
        reply_view(
            &mut stream,
            &req,
            json!({"kind": "listed", "clients": [ready_record(9, 2, 1)]}),
        );
        let WorkbenchRequest::Command(cmd) = read_workbench_request(&mut stream).unwrap() else {
            panic!("tab create")
        };
        let action = json!(cmd.action);
        assert_eq!(action["kind"], "tab.create");
        assert_eq!(action["options"]["cwd"], r"C:\proj");
        assert_eq!(
            action["options"]["agentStart"],
            json!(AgentStartSelector::parse("amp").unwrap())
        );
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Command(WorkbenchCommandResult {
                protocol_version: ProtocolVersion,
                request_id: cmd.request_id,
                outcome: serde_json::from_value(json!({
                    "kind": "tab-create-pending",
                    "revision": "10",
                    "terminalContextId": ctx_id,
                    "tabId": new_tab_id,
                    "panelId": PANEL,
                    "presentationId": "11111111111111111111111111111111",
                }))
                .unwrap(),
            }),
        )
        .unwrap();
        let WorkbenchRequest::ClientView(get_req) = read_workbench_request(&mut stream).unwrap()
        else {
            panic!("client observe")
        };
        reply_view(
            &mut stream,
            &get_req,
            json!({"kind": "found", "client": ready_record(10, 3, 1)}),
        );
        let WorkbenchRequest::ClientView(sub_req) = read_workbench_request(&mut stream).unwrap()
        else {
            panic!("client focus submit")
        };
        let ClientViewAction::Submit { command } = &sub_req.action else {
            panic!("client submit")
        };
        assert_eq!(
            json!(command.intent),
            json!({"kind": "panel", "terminalContextId": ctx_id, "tabId": new_tab_id, "panelId": PANEL})
        );
        let mut applied_view = ready_record(10, 4, 1)["view"].clone();
        applied_view["selection"]["tabId"] = json!(new_tab_id);
        reply_view(
            &mut stream,
            &sub_req,
            json!({
                "kind": "operation",
                "operation": {
                    "command": command,
                    "status": {"kind": "applied", "view": applied_view},
                },
            }),
        );
        assert_closed(&mut stream);

        // Connection 2: launched-as-first-command verification returns immediately on interactive shell.
        let (mut stream, _) = listener.accept().unwrap();
        accept_client(&mut stream);
        bound_snapshot_reply(&mut stream);
        shell(&mut stream);
        assert_closed(&mut stream);
    });
    let output = fixture.run(&[
        "agent",
        "start",
        "amp",
        "--panel",
        PANEL,
        "--cwd",
        r"C:\proj",
        "--focus",
        "--format=json",
    ]);
    happy.join().unwrap();
    let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(output.status.code(), Some(0), "{doc}");
    assert_eq!(doc["result"]["confirmation"], "launched");

    // 3. `--cwd workspace --no-focus` leaves `options.cwd` unset and skips Client discovery.
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let no_focus = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        accept_client(&mut stream);
        bound_snapshot_reply(&mut stream);
        let WorkbenchRequest::Command(cmd) = read_workbench_request(&mut stream).unwrap() else {
            panic!("tab create without client list")
        };
        let action = json!(cmd.action);
        assert_eq!(action["kind"], "tab.create");
        assert!(action["options"]["cwd"].is_null());
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Command(WorkbenchCommandResult {
                protocol_version: ProtocolVersion,
                request_id: cmd.request_id,
                outcome: serde_json::from_value(json!({
                    "kind": "tab-create-pending",
                    "revision": "10",
                    "terminalContextId": ctx_id,
                    "tabId": new_tab_id,
                    "panelId": PANEL,
                    "presentationId": "11111111111111111111111111111111",
                }))
                .unwrap(),
            }),
        )
        .unwrap();
        assert_closed(&mut stream);

        let (mut stream, _) = listener.accept().unwrap();
        accept_client(&mut stream);
        bound_snapshot_reply(&mut stream);
        shell(&mut stream);
        assert_closed(&mut stream);
    });
    let output = fixture.run(&[
        "agent",
        "start",
        "amp",
        "--panel",
        PANEL,
        "--cwd",
        "workspace",
        "--no-focus",
        "--format=json",
    ]);
    no_focus.join().unwrap();
    let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(output.status.code(), Some(0), "{doc}");

    // 4. A split with `--focus` that the Client rejects still starts the Agent in the new Panel:
    //    exit 0, `result.focus` names the miss, and `panelId` is the new Panel, not the reference.
    let new_panel_id = "00000000000000000000000000000029";
    let new_terminal = "66666666666666666666666666666666";
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let rejected = std::thread::spawn(move || {
        // Connection 1: client discovery, split creation, client observation + rejected focus.
        let (mut stream, _) = listener.accept().unwrap();
        accept_client(&mut stream);
        bound_snapshot_reply(&mut stream);
        let WorkbenchRequest::ClientView(req) = read_workbench_request(&mut stream).unwrap() else {
            panic!("client list")
        };
        reply_view(
            &mut stream,
            &req,
            json!({"kind": "listed", "clients": [ready_record(9, 2, 1)]}),
        );
        let WorkbenchRequest::Command(cmd) = read_workbench_request(&mut stream).unwrap() else {
            panic!("panel split")
        };
        let action = json!(cmd.action);
        assert_eq!(action["kind"], "panel.split");
        assert_eq!(action["options"]["cwd"], r"C:\proj");
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Command(WorkbenchCommandResult {
                protocol_version: ProtocolVersion,
                request_id: cmd.request_id,
                outcome: serde_json::from_value(json!({
                    "kind": "panel-split-pending",
                    "revision": "10",
                    "terminalContextId": ctx_id,
                    "tabId": base_tab_id,
                    "panelId": PANEL,
                    "newPanelId": new_panel_id,
                    "splitId": "00000000000000000000000000000030",
                    "firstSlotId": "00000000000000000000000000000031",
                    "secondSlotId": "00000000000000000000000000000032",
                    "presentationId": "11111111111111111111111111111111",
                }))
                .unwrap(),
            }),
        )
        .unwrap();
        let WorkbenchRequest::ClientView(get_req) = read_workbench_request(&mut stream).unwrap()
        else {
            panic!("client observe")
        };
        reply_view(
            &mut stream,
            &get_req,
            json!({"kind": "found", "client": ready_record(10, 3, 2)}),
        );
        let WorkbenchRequest::ClientView(sub_req) = read_workbench_request(&mut stream).unwrap()
        else {
            panic!("client focus submit")
        };
        let ClientViewAction::Submit { command } = &sub_req.action else {
            panic!("client submit")
        };
        assert_eq!(
            json!(command.intent),
            json!({"kind": "panel", "terminalContextId": ctx_id, "tabId": base_tab_id, "panelId": new_panel_id})
        );
        reply_view(
            &mut stream,
            &sub_req,
            json!({
                "kind": "operation",
                "operation": {
                    "command": command,
                    "status": {"kind": "rejected", "reason": "stale-view"},
                },
            }),
        );
        assert_closed(&mut stream);

        // Connection 2: the Agent launches in the new Panel as its first command. The confirmation
        // reads the Workbench after the split, so the snapshot holds the new Panel.
        let (mut stream, _) = listener.accept().unwrap();
        accept_client(&mut stream);
        bound_split_snapshot_reply(&mut stream, 10, new_panel_id, new_terminal);
        // The Agent's shell runs in the new Panel's own terminal, bound in the split snapshot.
        process_sample_for(
            &mut stream,
            Ok(vec![process(10, None, "cmd.exe")]),
            new_terminal,
        );
        assert_closed(&mut stream);
    });
    let output = fixture.run(&[
        "agent",
        "start",
        "amp",
        "--split=right",
        "--panel",
        PANEL,
        "--cwd",
        r"C:\proj",
        "--focus",
        "--format=json",
    ]);
    rejected.join().unwrap();
    let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(output.status.code(), Some(0), "{doc}");
    assert_eq!(doc["result"]["confirmation"], "launched");
    assert_eq!(doc["result"]["panelId"], new_panel_id);
    assert_eq!(doc["result"]["focus"]["stage"], "focus");
    assert_eq!(doc["result"]["focus"]["reason"], "rejected");
}
