//! Process-level startup effects, wire identity and recovery on Windows pipes.
use super::peer_support;
use orchbee_agent_observation_protocol::{
    AgentLaunchRequest, AgentRequest, AgentResult, read_agent_request, write_agent_result,
};
use orchbee_client_server_protocol::*;
use orchbee_local_ipc::{LocalListener, LocalStream};
use peer_support::*;
use serde_json::{Value, json};

const PANEL: &str = "22222222222222222222222222222222";
const GENERATION: &str = "55555555555555555555555555555555";
// `amp` is a shared alias, so only an install root proves the Agent.
const AMP: &str = r"C:\Users\orchbee\.amp\bin\amp.exe";

const RUN: &str = "99999999999999999999999999999999";
const SETTLING: &str = r#"{"result":"unknown","reason":"settling"}"#;

fn request(stream: &mut LocalStream) -> AgentLaunchRequest {
    let AgentRequest::Launch(request) = read_agent_request(stream).unwrap() else {
        panic!("launch request");
    };
    request
}
fn respond(
    stream: &mut LocalStream,
    request: &AgentLaunchRequest,
    operation: &str,
    outcome: Value,
) {
    let result: AgentResult = AgentResult::Launch(serde_json::from_value(json!({"kind":format!("agent.launch.{operation}.result"),"requestId":request.request_id(),"result":{"outcome":outcome}})).unwrap());
    write_agent_result(stream, &result).unwrap();
}
fn reserve(stream: &mut LocalStream) -> Value {
    let request = request(stream);
    let AgentLaunchRequest::Reserve {
        name,
        provider_id,
        presentation_id,
        host_epoch,
        host_terminal_id,
        ttl_ms,
        ..
    } = &request
    else {
        panic!("reserve");
    };
    assert_eq!(name.as_str(), "alpha");
    assert_eq!(provider_id.as_str(), "amp");
    assert!((3000..=30000).contains(&ttl_ms.value()));
    let reservation = json!({"reservationId":request.request_id(),"name":name,"providerId":provider_id,"binding":{"presentationId":presentation_id,"generation":GENERATION,"hostEpoch":host_epoch,"hostTerminalId":host_terminal_id},"remainingMs":ttl_ms,"settleRemainingMs":3000});
    respond(
        stream,
        &request,
        "reserve",
        json!({"kind":"reserved","reservation":reservation,"runToken":RUN}),
    );
    reservation
}
fn release(stream: &mut LocalStream, reservation: &Value) {
    let request = request(stream);
    assert!(
        matches!(&request, AgentLaunchRequest::Cancel { reservation_id,.. } if json!(reservation_id)==reservation["reservationId"])
    );
    respond(
        stream,
        &request,
        "cancel",
        json!({"kind":"released","reservationId":reservation["reservationId"]}),
    );
}
/// The Server's identity result for the reservation's Terminal, read after each process sample.
fn identify(stream: &mut LocalStream, reservation: &Value, identity: &str) {
    use orchbee_workbench_protocol::{
        PanelAgentAction, PanelAgentOutcome, PanelAgentResult, ProtocolVersion, WorkbenchRequest,
        WorkbenchResponse, WorkbenchRevision, read_workbench_request, write_workbench_response,
    };
    let WorkbenchRequest::PanelAgent(request) = read_workbench_request(stream).unwrap() else {
        panic!("one Identify after the process sample")
    };
    assert!(matches!(&request.action, PanelAgentAction::Identify { .. }));
    let identity: Value = serde_json::from_str(identity).unwrap();
    // An `agent` result always names the published run; the sample's Agent row is PID 11.
    let anchor = (identity["result"] == "agent")
        .then(|| json!({"pid": 11, "providerId": identity["providerId"]}));
    write_workbench_response(
        stream,
        &WorkbenchResponse::PanelAgent(PanelAgentResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            session_id: request.session_id,
            outcome: PanelAgentOutcome::Identified {
                workbench_revision: WorkbenchRevision::new(1),
                binding: serde_json::from_value(reservation["binding"].clone()).unwrap(),
                identity: serde_json::from_value(identity).unwrap(),
                anchor: anchor.map(|anchor| serde_json::from_value(anchor).unwrap()),
            },
        }),
    )
    .unwrap();
}
fn pending(reservation: &Value, candidate: Value, receipt: Value) -> Value {
    json!({"kind":"pending","reservation":reservation,"candidateSessionId":candidate,"receipt":receipt})
}
fn shell(stream: &mut LocalStream) {
    process_reply(stream, vec![process(10, None, "cmd.exe")]);
}
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
fn run(script: impl FnOnce(LocalStream) + Send + 'static) -> (std::process::Output, Value) {
    run_with_timeout("30000", script)
}

fn run_with_timeout(
    timeout: &str,
    script: impl FnOnce(LocalStream) + Send + 'static,
) -> (std::process::Output, Value) {
    run_answering(
        timeout,
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
        "--name",
        "alpha",
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
fn startup_observation_deadline_keeps_accepted_input_and_pending_reservation() {
    let (output, document) = run_with_timeout("6000", |mut stream| {
        bound_snapshot_reply(&mut stream);
        shell(&mut stream);
        let reservation = reserve(&mut stream);
        shell(&mut stream);
        input(&mut stream, accepted());
        let inspect = request(&mut stream);
        assert!(matches!(inspect, AgentLaunchRequest::Inspect { .. }));
        std::thread::sleep(std::time::Duration::from_millis(1100));
        respond(
            &mut stream,
            &inspect,
            "inspect",
            pending(&reservation, Value::Null, json!({"phase":"awaiting"})),
        );
        process_reply(
            &mut stream,
            vec![process(10, None, "cmd.exe"), process(11, Some(10), AMP)],
        );
        identify(&mut stream, &reservation, SETTLING);
        assert!(matches!(
            request(&mut stream),
            AgentLaunchRequest::Inspect { .. }
        ));
        std::thread::sleep(std::time::Duration::from_millis(5100));
        assert_closed(&mut stream);
    });
    assert_eq!(output.status.code(), Some(7));
    let details = &document["failure"]["details"];
    assert_eq!(details["reason"], "start-deadline");
    assert_eq!(details["reservationState"], "pending");
    assert_eq!(details["submission"]["result"]["inputStatus"], "accepted");
    assert!(details["cause"].is_object());
}

#[test]
fn startup_busy_terminal_and_name_conflict_never_type_input() {
    for busy in [true, false] {
        let (output, document) = run(move |mut stream| {
            bound_snapshot_reply(&mut stream);
            if busy {
                process_reply(&mut stream, vec![process(10, None, AMP)]);
            } else {
                shell(&mut stream);
                let request = request(&mut stream);
                respond(
                    &mut stream,
                    &request,
                    "reserve",
                    json!({"kind":"rejected","reason":"name-in-use"}),
                );
            }
            assert_closed(&mut stream);
        });
        assert_eq!(output.status.code(), Some(3));
        assert!(document["failure"]["details"]["submission"].is_null());
        assert_eq!(document["failure"]["details"]["reservationState"], "none");
    }
}

fn refused(reason: SendRejectedReason) -> TerminalSendOutcome {
    TerminalSendOutcome::Rejected {
        progress: SendProgress::none(),
        cleanup: SendCleanupStatus::Confirmed,
        reason,
    }
}

/// Every prompt refusal types nothing and releases the name, so retrying is safe. A `settling`
/// prompt is asked again, more than once, and the first other refusal is final; a peer without
/// the gate is incompatible, never sent ungated.
#[test]
fn startup_types_nothing_where_the_prompt_or_a_peer_cannot_confirm_it() {
    for (answers, code, reason, state) in [
        (
            vec![
                SendPromptState::Settling,
                SendPromptState::Settling,
                SendPromptState::Unmarked,
            ],
            3,
            "prompt-unconfirmed",
            Some("unmarked"),
        ),
        (
            vec![SendPromptState::Unmarked],
            3,
            "prompt-unconfirmed",
            Some("unmarked"),
        ),
        (vec![SendPromptState::Busy], 3, "terminal-not-idle", None),
    ] {
        let (output, document) = run(move |mut stream| {
            bound_snapshot_reply(&mut stream);
            shell(&mut stream);
            let reservation = reserve(&mut stream);
            shell(&mut stream);
            for answer in answers {
                input(
                    &mut stream,
                    refused(SendRejectedReason::PromptUnconfirmed(answer)),
                );
            }
            release(&mut stream, &reservation);
            assert_closed(&mut stream);
        });
        assert_eq!(output.status.code(), Some(code));
        let details = &document["failure"]["details"];
        assert_eq!(details["reason"], reason);
        assert_eq!(details.get("promptState").and_then(Value::as_str), state);
        assert_eq!(details["reservationState"], "released");
        assert_eq!(details["submission"]["result"]["inputStatus"], "rejected");
    }

    let (output, document) = run(|mut stream| {
        bound_snapshot_reply(&mut stream);
        shell(&mut stream);
        let reservation = reserve(&mut stream);
        shell(&mut stream);
        input(
            &mut stream,
            refused(SendRejectedReason::HostPromptGateUnsupported),
        );
        release(&mut stream, &reservation);
        assert_closed(&mut stream);
    });
    assert_eq!(output.status.code(), Some(6));
    let details = &document["failure"]["details"];
    assert_eq!(document["failure"]["kind"], "incompatible");
    assert_eq!(details["cause"]["stage"], "capability");
    assert_eq!(details["cause"]["reason"], "terminal-too-old");
    assert_eq!(details["reservationState"], "released");

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
    assert_eq!(details["reservationState"], "none");
    assert!(details["submission"].is_null());
}

#[test]
fn startup_samples_again_after_unavailable_sample_but_preserves_permanent_host_failure() {
    let (output, document) = run(|mut stream| {
        bound_snapshot_reply(&mut stream);
        process_sample_reply(&mut stream, Err("process-sample-unavailable"));
        shell(&mut stream);
        let reservation = reserve(&mut stream);
        shell(&mut stream);
        input(&mut stream, accepted());
        let inspect = request(&mut stream);
        respond(
            &mut stream,
            &inspect,
            "inspect",
            pending(&reservation, Value::Null, json!({"phase":"awaiting"})),
        );
        process_sample_reply(&mut stream, Err("process-sample-unavailable"));
        process_reply(
            &mut stream,
            vec![
                process(10, None, "cmd.exe"),
                process(11, Some(10), "claude.exe"),
            ],
        );
        // The Server's run decides the provider; a settling Server would select nothing.
        identify(
            &mut stream,
            &reservation,
            r#"{"result":"agent","providerId":"claude-code","grade":"recognized"}"#,
        );
        release(&mut stream, &reservation);
        assert_closed(&mut stream);
    });
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(
        document["failure"]["details"]["reason"],
        "agent-provider-mismatch"
    );
    assert_eq!(
        document["failure"]["details"]["submission"]["result"]["inputStatus"],
        "accepted"
    );
    let (output, document) = run(|mut stream| {
        bound_snapshot_reply(&mut stream);
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
fn startup_rechecks_shell_incarnation_before_input_and_releases_name() {
    let (output, document) = run(|mut stream| {
        bound_snapshot_reply(&mut stream);
        shell(&mut stream);
        let reservation = reserve(&mut stream);
        let mut changed = process(10, None, "cmd.exe");
        changed.creation_time += 1;
        process_reply(&mut stream, vec![changed]);
        release(&mut stream, &reservation);
        assert_closed(&mut stream);
    });
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(document["failure"]["details"]["reason"], "binding-changed");
    assert_eq!(
        document["failure"]["details"]["reservationState"],
        "released"
    );
    assert!(document["failure"]["details"]["submission"].is_null());
}

#[test]
fn startup_unknown_and_partial_input_keep_receipts_and_release_only_their_name() {
    for unknown in [true, false] {
        let (output, document) = run(move |mut stream| {
            bound_snapshot_reply(&mut stream);
            shell(&mut stream);
            let reservation = reserve(&mut stream);
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
            release(&mut stream, &reservation);
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
        assert_eq!(
            document["failure"]["details"]["reservationState"],
            "released"
        );
    }
}

#[test]
fn startup_lost_reserve_reply_is_unknown_and_never_replayed() {
    let (output, document) = run(|mut stream| {
        bound_snapshot_reply(&mut stream);
        shell(&mut stream);
        assert!(matches!(
            request(&mut stream),
            AgentLaunchRequest::Reserve { .. }
        ));
    });
    assert_eq!(output.status.code(), Some(8));
    assert_eq!(
        document["failure"]["details"]["reservationState"],
        "unknown"
    );
    assert!(document["failure"]["details"]["reservationId"].is_string());
    assert!(document["failure"]["details"]["submission"].is_null());
}

#[test]
fn startup_binds_only_ready_matching_agent_and_keeps_unknown_commit_distinct() {
    for lose_reply in [false, true] {
        let (output, document) = run(move |mut stream| {
            bound_snapshot_reply(&mut stream);
            shell(&mut stream);
            let mut reservation = reserve(&mut stream);
            shell(&mut stream);
            input(&mut stream, accepted());
            let inspect = request(&mut stream);
            assert!(matches!(inspect, AgentLaunchRequest::Inspect { .. }));
            std::thread::sleep(std::time::Duration::from_millis(3050));
            reservation["settleRemainingMs"] = json!(0);
            respond(
                &mut stream,
                &inspect,
                "inspect",
                pending(
                    &reservation,
                    json!("sess-alpha"),
                    json!({"phase":"awaiting"}),
                ),
            );
            process_reply(
                &mut stream,
                vec![process(10, None, "cmd.exe"), process(11, Some(10), AMP)],
            );
            // Readiness binds only the Server's published run of the requested provider.
            identify(
                &mut stream,
                &reservation,
                r#"{"result":"agent","providerId":"amp","grade":"recognized"}"#,
            );
            let mut detail = session_detail("sess-alpha");
            detail["snapshot"]["providerId"] = json!("amp");
            detail["snapshot"]["turn"] = json!({"kind":"idle"});
            detail["snapshot"]["pendingRequests"] = json!([]);
            detail["status"] = json!({"kind":"idle"});
            detail["snapshot"]["presentationAttachments"][0]["generation"] = json!(GENERATION);
            read_reply(
                &mut stream,
                "get",
                json!({"kind":"found","revision":2,"session":detail}),
            );
            let complete = request(&mut stream);
            assert!(
                matches!(&complete,AgentLaunchRequest::Complete { session_id,.. } if session_id.as_str()=="sess-alpha")
            );
            if !lose_reply {
                respond(
                    &mut stream,
                    &complete,
                    "complete",
                    json!({"kind":"completed","reservationId":reservation["reservationId"],"name":"alpha","sessionId":"sess-alpha"}),
                );
                assert_closed(&mut stream);
            }
        });
        assert_eq!(
            output.status.code(),
            Some(if lose_reply { 8 } else { 0 }),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        if lose_reply {
            assert_eq!(
                document["failure"]["details"]["reservationState"],
                "unknown"
            );
            assert_eq!(
                document["failure"]["details"]["submission"]["result"]["inputStatus"],
                "accepted"
            );
        } else {
            assert_eq!(document["result"]["sessionId"], "sess-alpha");
        }
    }
}

/// A settled Server run naming another provider fails startup; the sample's own chain is never
/// consulted. A finished launch receipt reports the shell status at once.
#[test]
fn startup_consumes_server_identity_and_finished_launch_receipts() {
    let cases = [
        (
            json!({"phase":"started"}),
            Some(r#"{"result":"agent","providerId":"grok","grade":"recognized"}"#),
            "agent-provider-mismatch",
        ),
        (
            json!({"phase":"finished","succeeded":false}),
            None,
            "launch-command-failed",
        ),
        (
            json!({"phase":"finished","succeeded":true}),
            None,
            "agent-exited",
        ),
    ];
    for (receipt, identity, reason) in cases {
        let (output, document) = run(move |mut stream| {
            bound_snapshot_reply(&mut stream);
            shell(&mut stream);
            let reservation = reserve(&mut stream);
            shell(&mut stream);
            input(&mut stream, accepted());
            let inspect = request(&mut stream);
            respond(
                &mut stream,
                &inspect,
                "inspect",
                pending(&reservation, Value::Null, receipt),
            );
            if let Some(identity) = identity {
                process_reply(
                    &mut stream,
                    vec![process(10, None, "cmd.exe"), process(11, Some(10), AMP)],
                );
                self::identify(&mut stream, &reservation, identity);
            }
            release(&mut stream, &reservation);
            assert_closed(&mut stream);
        });
        assert_eq!(output.status.code(), Some(3), "{reason}");
        assert_eq!(document["failure"]["details"]["reason"], reason);
        assert_eq!(
            document["failure"]["details"]["submission"]["result"]["inputStatus"],
            "accepted"
        );
    }
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
