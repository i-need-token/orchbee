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
        json!({"kind":"reserved","reservation":reservation}),
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
fn shell(stream: &mut LocalStream) {
    process_reply(stream, vec![process(10, None, "cmd.exe")]);
}
fn input(stream: &mut LocalStream, outcome: TerminalSendOutcome) {
    let ClientMessage::TerminalSend(request) = read_client_message(stream).unwrap() else {
        panic!("one terminal send");
    };
    let records = request.records().as_slice();
    assert_eq!(records.len(), 2);
    let SendRecord::Paste { content } = &records[0] else {
        panic!("literal command");
    };
    assert!(
        content
            .as_bytes()
            .starts_with(b"powershell.exe -NoLogo -NoProfile -EncodedCommand ")
    );
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
fn run(script: impl FnOnce(LocalStream) + Send + 'static) -> (std::process::Output, Value) {
    run_with_timeout("30000", script)
}

fn run_with_timeout(
    timeout: &str,
    script: impl FnOnce(LocalStream) + Send + 'static,
) -> (std::process::Output, Value) {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    let peer = peer(
        &fixture,
        LocalListener::bind(rendezvous.client_socket_path()).unwrap(),
        script,
    );
    let output = fixture.run(&[
        "agent",
        "start",
        "alpha",
        "--provider=amp",
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
            json!({"kind":"pending","reservation":reservation,"candidateSessionId":null}),
        );
        process_reply(
            &mut stream,
            vec![
                process(10, None, "cmd.exe"),
                process(11, Some(10), "amp.exe"),
            ],
        );
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
                process_reply(&mut stream, vec![process(10, None, "amp.exe")]);
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

#[test]
fn startup_samples_again_after_process_churn_but_preserves_permanent_host_failure() {
    let (output, document) = run(|mut stream| {
        bound_snapshot_reply(&mut stream);
        process_sample_reply(&mut stream, Err("process-sample-changed"));
        shell(&mut stream);
        let reservation = reserve(&mut stream);
        shell(&mut stream);
        input(&mut stream, accepted());
        let inspect = request(&mut stream);
        respond(
            &mut stream,
            &inspect,
            "inspect",
            json!({"kind":"pending","reservation":reservation,"candidateSessionId":null}),
        );
        process_sample_reply(&mut stream, Err("process-sample-unavailable"));
        process_reply(
            &mut stream,
            vec![
                process(10, None, "cmd.exe"),
                process(11, Some(10), "claude.exe"),
            ],
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
                json!({"kind":"pending","reservation":reservation,"candidateSessionId":"sess-alpha"}),
            );
            process_reply(
                &mut stream,
                vec![
                    process(10, None, "cmd.exe"),
                    process(11, Some(10), "amp.exe"),
                ],
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

/// A managed provider whose executable is absent, or whose integration is not
/// enabled, refuses locally before any Session contact: nothing is reserved or
/// typed, and the diagnostic names the one command that resolves it.
#[test]
fn startup_preflight_refuses_unready_integration_before_any_session_contact() {
    for (installed, reason, hint) in [
        (false, "provider-not-found", "was not found on PATH"),
        (
            true,
            "integration-not-enabled",
            "orchbee agent integration enable --provider codex",
        ),
    ] {
        let fixture = Fixture::new();
        let bin = fixture.0.join("bin");
        let home = fixture.0.join("home");
        let codex_home = home.join(".codex");
        for directory in [&bin, &codex_home] {
            std::fs::create_dir_all(directory).unwrap();
        }
        if installed {
            std::fs::write(bin.join("codex.cmd"), b"@exit /b 1\r\n").unwrap();
        }
        let output = fixture.run_with_env(
            &[
                "agent",
                "start",
                "alpha",
                "--provider=codex",
                "--panel",
                PANEL,
                "--format=json",
            ],
            None,
            None,
            &[
                ("PATH", bin.as_os_str()),
                ("USERPROFILE", home.as_os_str()),
                ("CODEX_HOME", codex_home.as_os_str()),
            ],
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{reason}: {stderr}");
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &document,
            include_str!("../../src/cli/schemas/orchbee.cli.failure.agent-start.json"),
        );
        let details = &document["failure"]["details"];
        assert_eq!(document["failure"]["kind"], "check-failed");
        assert_eq!(details["reason"], reason);
        assert_eq!(details["reservationState"], "none");
        assert!(details["submission"].is_null());
        assert!(stderr.contains(hint), "{reason}: {stderr}");
    }
}
