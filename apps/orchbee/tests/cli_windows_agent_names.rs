//! Windows Agent names and terminal controls over real CLI and named pipes.
#![cfg(windows)]

#[path = "cli_windows_agent_names/changes.rs"]
mod changes;
#[path = "support/windows_agent_peer/cohort_fixture.rs"]
mod cohort_fixture;
#[allow(dead_code)]
#[path = "support/terminal_metadata_peer.rs"]
mod metadata_peer;
#[path = "support/windows_agent_peer.rs"]
mod peer_support;
#[path = "cli_windows_agent_names/wait.rs"]
mod wait;
// Bound debug cohort copies and process startup beside the peer admission deadline.
use cohort_fixture::CohortFixture as Fixture;
use orchbee_agent_observation_protocol::{
    AgentRequest, AgentResult, read_agent_request, write_agent_result,
};
use orchbee_local_ipc::LocalListener;
use peer_support::*;
use serde_json::{Value, json};
use std::{
    io::Read,
    thread,
    time::{Duration, Instant},
};

fn join_peer(handler: PeerGuard<'_>, output: &std::process::Output) {
    assert!(
        handler.join().is_ok(),
        "peer failed; CLI status={} stdout={} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn names_commands_exchange_correlated_typed_results_on_windows() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    let cases: &[(&[&str], &str, Value, &str)] = &[
        (
            &["agent", "names"],
            "names.list",
            json!({"kind":"listed","pending":[],"bindings":[{"name":"alpha","sessionId":"s1","lifecycle":"live"}]}),
            include_str!("../src/cli/schemas/orchbee.agent.names.json"),
        ),
        (
            &["agent", "resolve", "--name", "alpha"],
            "resolve",
            json!({"kind":"resolved","sessionId":"s1","resolvedBy":"name"}),
            include_str!("../src/cli/schemas/orchbee.agent.resolve.json"),
        ),
        (
            &["agent", "rename", "--agent-session", "s1", "alpha"],
            "name.assign",
            json!({"kind":"assigned","sessionId":"s1","name":"alpha","previousName":null}),
            include_str!("../src/cli/schemas/orchbee.agent.rename.json"),
        ),
        (
            &["agent", "rename", "alpha", "--clear"],
            "name.release",
            json!({"kind":"released","sessionId":"s1","name":"alpha"}),
            include_str!("../src/cli/schemas/orchbee.agent.rename.json"),
        ),
    ];
    for (arguments, operation, outcome, schema) in cases {
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let operation = (*operation).to_owned();
        let outcome = outcome.clone();
        let expected = outcome.clone();
        let handler = peer(&fixture, listener, move |mut stream| {
            let AgentRequest::Names(request) = read_agent_request(&mut stream).unwrap() else {
                panic!("names request");
            };
            let request_document = json!(request);
            assert_eq!(
                request_document["kind"],
                format!("agent.session.{operation}.request")
            );
            validate(
                &request_document,
                include_str!("../../../schemas/agent-observation/v1/agent-observation.schema.json"),
            );
            let response = json!({"kind":format!("agent.session.{operation}.result"),"requestId":request.request_id(),"result":{"outcome":outcome}});
            validate(
                &response,
                include_str!("../../../schemas/agent-observation/v1/agent-observation.schema.json"),
            );
            let result = AgentResult::Names(serde_json::from_value(response).unwrap());
            write_agent_result(&mut stream, &result).unwrap();
            let mut byte = [0];
            assert_eq!(
                stream.read(&mut byte).unwrap(),
                0,
                "one request only, then close"
            );
        });
        let arguments = [*arguments, &["--format", "json"]].concat();
        let output = fixture.run(&arguments);
        join_peer(handler, &output);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(&document, schema);
        assert_eq!(document["result"], expected);
    }
}

#[test]
fn lost_and_mismatched_mutation_replies_are_unknown_without_replay() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    for wrong_reply in [false, true] {
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            let AgentRequest::Names(request) = read_agent_request(&mut stream).unwrap() else {
                panic!("names request");
            };
            if wrong_reply {
                let result = AgentResult::Names(serde_json::from_value(json!({"kind":"agent.session.names.list.result","requestId":request.request_id(),"result":{"outcome":{"kind":"listed","pending":[],"bindings":[]}}})).unwrap());
                write_agent_result(&mut stream, &result).unwrap();
                let mut byte = [0];
                assert_eq!(stream.read(&mut byte).unwrap(), 0);
            }
        });
        let output = fixture.run(&["agent", "rename", "s1", "alpha", "--format", "json"]);
        join_peer(handler, &output);
        assert_eq!(output.status.code(), Some(8));
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &document,
            include_str!("../src/cli/schemas/orchbee.cli.failure.agent-names.json"),
        );
        assert_eq!(document["failure"]["kind"], "outcome-unknown");
        assert!(document.get("source").is_none());
    }
}

#[test]
fn invalid_names_and_conflicting_selectors_fail_before_runtime_creation() {
    let fixture = Fixture::new();
    for arguments in [
        vec!["agent", "rename", "s1", "Upper"],
        vec!["agent", "resolve", "s1", "--name", "alpha"],
        vec!["agent", "rename", "--clear"],
        vec!["agent", "names", "--clear"],
        vec!["agent", "prompt", "alpha", ""],
        vec!["agent", "prompt", "alpha", "text", "--timeout", "100"],
        vec!["agent", "send-keys", "alpha", "enter", "invalid-key"],
        vec!["agent", "read", "alpha", "--history"],
        vec!["agent", "get", "s1", "--name", "alpha"],
        vec!["agent", "wait", "--name", "Upper"],
        vec![
            "agent",
            "explain",
            "--name",
            "alpha",
            "--agent-session",
            "s1",
        ],
    ] {
        let output = fixture.run(&[arguments.as_slice(), &["--format", "json"]].concat());
        assert_eq!(output.status.code(), Some(2));
        assert!(!fixture.0.join("runtime").exists());
    }
}

fn explanation() -> serde_json::Value {
    serde_json::json!({
        "actionPath": "navigate-only",
        "capabilityCeiling": ["questions", "activity"],
        "contentAvailable": false,
        "fidelity": "rich",
        "freshness": "fresh",
        "freshnessMeaning": "current-observation",
        "generation": 3,
        "sequence": 11,
        "source": "native",
        "status": {"kind": "needs-attention", "count": 1},
        "statusBasis": "attention-request"
    })
}

fn begin_get_projection(
    stream: &mut orchbee_local_ipc::LocalStream,
    revision: u64,
    details: &[Value],
) -> AgentProjectionFixture {
    bound_snapshot_reply(stream);
    let projection = AgentProjectionFixture::canonical(bound_snapshot(9), revision, details);
    projection.read(stream, true);
    projection
}

#[test]
fn named_get_and_explain_share_one_admission_and_publish_only_the_read_source() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    for verb in ["get", "explain"] {
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            let projection = (verb == "get")
                .then(|| begin_get_projection(&mut stream, 7, &[session_detail("sess-alpha")]));
            let resolution = resolve_reply(
                &mut stream,
                json!({"kind":"resolved","sessionId":"sess-alpha","resolvedBy":"name"}),
            );
            let mut outcome =
                json!({"kind":"found","revision":7,"session":session_detail("sess-alpha")});
            if verb == "explain" {
                outcome["explanation"] = explanation();
            }
            let read = read_reply(&mut stream, verb, outcome);
            assert_ne!(resolution, read);
            if verb == "get" {
                metadata_peer::sample(&mut stream, "observed", "claude-code.exe");
                read_reply(
                    &mut stream,
                    "get",
                    json!({"kind":"found","revision":7,"session":session_detail("sess-alpha")}),
                );
                bound_snapshot_reply(&mut stream);
                management_reply(&mut stream, 7, json!({"phase":"unmanaged"}));
                projection.unwrap().confirm(&mut stream, true);
            }
            assert_closed(&mut stream);
        });
        let output = fixture.run(&["agent", verb, "alpha", "--format", "json"]);
        join_peer(handler, &output);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            document["source"]["kind"],
            if verb == "get" {
                "agent-projection"
            } else {
                "agent-read"
            }
        );
        let result = if verb == "get" {
            &document["result"]["inspection"]
        } else {
            &document["result"]
        };
        assert_eq!(result["session"]["snapshot"]["sessionId"], "sess-alpha");
        validate(
            &document,
            if verb == "get" {
                include_str!("../src/cli/schemas/orchbee.agent.get.json")
            } else {
                include_str!("../src/cli/schemas/orchbee.agent.explain.json")
            },
        );
    }
}

#[test]
fn named_wait_resolves_once_and_keeps_polling_the_original_session() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let handler = peer(&fixture, listener, move |mut stream| {
        resolve_reply(
            &mut stream,
            json!({"kind":"resolved","sessionId":"sess-alpha","resolvedBy":"name"}),
        );
        // A quiet first observation keeps the wait alive. After this response
        // a reused alias could represent another session. The pinned-ID contract
        // forbids a second Resolve; read_reply rejects it before answering.
        let mut first = session_detail("sess-alpha");
        first["snapshot"]["pendingRequests"] = json!([]);
        first["status"] = json!({"kind":"working","activity":"tool","phase":"working"});
        wait::checkpoint(&mut stream, 7);
        read_reply(
            &mut stream,
            "get",
            json!({"kind":"found","revision":7,"session":first}),
        );
        wait::checkpoint(&mut stream, 7);
        read_reply(
            &mut stream,
            "get",
            json!({"kind":"found","revision":8,"session":session_detail("sess-alpha")}),
        );
        assert_closed(&mut stream);
    });
    let output = fixture.run(&[
        "agent",
        "wait",
        "--name",
        "alpha",
        "--timeout",
        "2000",
        "--format",
        "json",
    ]);
    join_peer(handler, &output);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    validate(
        &document,
        include_str!("../src/cli/schemas/orchbee.agent.wait.json"),
    );
    assert_eq!(
        document["result"]["session"]["snapshot"]["sessionId"],
        "sess-alpha"
    );
    assert_eq!(document["result"]["wait"]["observations"], 2);
}

#[test]
fn unresolved_name_publishes_its_own_failure_without_get_or_wait_poll() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    for verb in ["get", "explain", "wait"] {
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            if verb == "get" {
                begin_get_projection(&mut stream, 7, &[]);
            }
            resolve_reply(&mut stream, json!({"kind":"rejected","reason":"not-found"}));
            assert_closed(&mut stream);
        });
        let output = fixture.run(&["agent", verb, "--name", "missing", "--format", "json"]);
        join_peer(handler, &output);
        assert_eq!(output.status.code(), Some(3));
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &document,
            include_str!("../src/cli/schemas/orchbee.cli.failure.agent-names.json"),
        );
        assert_eq!(document["source"]["kind"], "agent-names");
        assert_eq!(document["failure"]["kind"], "not-found");
    }
}

#[test]
fn explicit_session_bypasses_name_resolution_even_for_a_valid_alias_spelling() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let handler = peer(&fixture, listener, move |mut stream| {
        let projection = begin_get_projection(&mut stream, 7, &[session_detail("sess-alpha")]);
        read_reply(
            &mut stream,
            "get",
            json!({"kind":"found","revision":7,"session":session_detail("sess-alpha")}),
        );
        metadata_peer::sample(&mut stream, "observed", "claude-code.exe");
        read_reply(
            &mut stream,
            "get",
            json!({"kind":"found","revision":7,"session":session_detail("sess-alpha")}),
        );
        bound_snapshot_reply(&mut stream);
        management_reply(&mut stream, 7, json!({"phase":"unmanaged"}));
        projection.confirm(&mut stream, true);
        assert_closed(&mut stream);
    });
    let output = fixture.run(&[
        "agent",
        "get",
        "--agent-session",
        "sess-alpha",
        "--format",
        "json",
    ]);
    join_peer(handler, &output);
    assert!(output.status.success());
}

#[test]
fn named_get_resolution_and_read_share_the_original_five_second_deadline() {
    let fixture = Fixture::deadline_sensitive();
    let rendezvous = fixture.rendezvous();
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let handler = peer(&fixture, listener, move |mut stream| {
        begin_get_projection(&mut stream, 7, &[session_detail("sess-alpha")]);
        // Spending most of the command budget on resolution must leave only
        // the remainder for Get; renewing another five seconds would fail.
        let AgentRequest::Names(request) = read_agent_request(&mut stream).unwrap() else {
            panic!("resolve");
        };
        let exchange_started = Instant::now();
        thread::sleep(Duration::from_millis(3200));
        let reply = AgentResult::Names(serde_json::from_value(json!({"kind":"agent.session.resolve.result","requestId":request.request_id(),"result":{"outcome":{"kind":"resolved","sessionId":"sess-alpha","resolvedBy":"name"}}})).unwrap());
        write_agent_result(&mut stream, &reply).unwrap();
        assert!(matches!(
            read_agent_request(&mut stream).unwrap(),
            AgentRequest::Read(_)
        ));
        assert_closed(&mut stream);
        let elapsed = exchange_started.elapsed();
        // Measure from the peer-observed Resolve, excluding process creation,
        // Defender scanning and output finalization outside this wire budget.
        assert!(
            elapsed < Duration::from_millis(7250),
            "exchange budget renewed: {elapsed:?}"
        );
    });
    let output = fixture.run(&["agent", "get", "--name", "alpha", "--format", "json"]);
    join_peer(handler, &output);
    assert_eq!(output.status.code(), Some(7));
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    validate(
        &document,
        include_str!("../src/cli/schemas/orchbee.cli.failure.agent.json"),
    );
    assert_eq!(document["failure"]["kind"], "timeout");
    assert!(document.get("source").is_none());
}

#[test]
fn agent_terminal_refuses_inactive_missing_and_ambiguous_bindings_before_terminal_effects() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    for verb in ["read", "send-keys"] {
        for case in [
            "empty",
            "panel-is-not-presentation",
            "not-navigable",
            "ended",
            "ambiguous",
        ] {
            let mut detail = session_detail("sess-alpha");
            match case {
                "empty" => detail["snapshot"]["presentationAttachments"] = json!([]),
                "panel-is-not-presentation" => {
                    detail["snapshot"]["presentationAttachments"][0]["presentationId"] =
                        json!("22222222222222222222222222222222")
                }
                "not-navigable" => {
                    detail["snapshot"]["presentationAttachments"][0]["navigable"] = json!(false)
                }
                "ended" => {
                    detail["snapshot"]["lifecycle"] = json!("ended");
                    detail["snapshot"]["turn"] = json!({"kind":"idle"});
                    detail["snapshot"]["pendingRequests"] = json!([]);
                    detail["status"] = json!({"kind":"ended"});
                }
                "ambiguous" => {
                    let mut second = detail["snapshot"]["presentationAttachments"][0].clone();
                    second["presentationId"] = json!("44444444444444444444444444444444");
                    detail["snapshot"]["presentationAttachments"]
                        .as_array_mut()
                        .unwrap()
                        .push(second);
                }
                _ => unreachable!(),
            }
            let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
            let handler = peer(&fixture, listener, move |mut stream| {
                bound_agent_reply(&mut stream, detail);
                assert_closed(&mut stream);
            });
            let mut arguments = vec!["agent", verb, "--name", "alpha", "--format", "json"];
            if verb == "send-keys" {
                arguments.push("enter");
            }
            let output = fixture.run(&arguments);
            join_peer(handler, &output);
            assert_eq!(
                output.status.code(),
                Some(3),
                "{case}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let document: Value = serde_json::from_slice(&output.stdout).unwrap();
            validate(
                &document,
                include_str!("../src/cli/schemas/orchbee.cli.failure.json"),
            );
            assert_eq!(
                document["failure"]["kind"],
                if case == "ambiguous" {
                    "ambiguous"
                } else {
                    "not-found"
                }
            );
            if case == "ambiguous" {
                assert_eq!(document["failure"]["details"]["totalCount"], 2);
                assert_eq!(
                    document["failure"]["details"]["candidates"],
                    json!([
                        "33333333333333333333333333333333",
                        "55555555555555555555555555555555"
                    ])
                );
            }
        }
    }
}

#[test]
fn agent_keys_use_the_exact_bound_terminal_once_and_preserve_unknown_outcomes() {
    use orchbee_client_server_protocol::*;
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    for mode in ["accepted", "partial", "rejected", "unknown", "lost"] {
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_agent_reply(&mut stream, session_detail("sess-alpha"));
            runtime_preflight(&mut stream, session_detail("sess-alpha"));
            let ClientMessage::TerminalSend(request) = read_client_message(&mut stream).unwrap()
            else {
                panic!("one send");
            };
            assert_eq!(
                request.terminal_id().as_str(),
                "33333333333333333333333333333333"
            );
            assert_eq!(request.records().len(), 2);
            assert!(matches!(
                request.records().as_slice()[0],
                SendRecord::Key {
                    key: SendKey::Enter
                }
            ));
            if mode != "lost" {
                let target = SendTarget::new(
                    std::num::NonZeroU64::new(1).unwrap(),
                    HostEpoch::parse("66666666666666666666666666666666").unwrap(),
                    HostTerminalId::parse("77777777777777777777777777777777").unwrap(),
                );
                let outcome = match mode {
                    "accepted" => TerminalSendOutcome::Accepted {
                        progress: SendProgress::complete(2, 4),
                        cleanup: SendCleanupStatus::Confirmed,
                    },
                    "partial" => TerminalSendOutcome::Unavailable {
                        reason: SendUnavailableReason::InputBudgetExhausted,
                        progress: SendProgress::stopped(1, 1),
                        cleanup: SendCleanupStatus::Confirmed,
                    },
                    "rejected" => TerminalSendOutcome::Rejected {
                        reason: SendRejectedReason::HostInputBackpressure,
                        progress: SendProgress::stopped(0, 0),
                        cleanup: SendCleanupStatus::Confirmed,
                    },
                    "unknown" => TerminalSendOutcome::OutcomeUnknown {
                        progress: SendProgress::stopped(1, 1),
                        cleanup: SendCleanupStatus::Unconfirmed,
                    },
                    _ => unreachable!(),
                };
                let result =
                    TerminalSendResult::new(request.request_id().clone(), Some(target), outcome)
                        .unwrap();
                write_server_message(&mut stream, &ServerMessage::TerminalSendResult(result))
                    .unwrap();
                assert_closed(&mut stream);
            }
        });
        let output = fixture.run(&[
            "agent",
            "send-keys",
            "--name",
            "alpha",
            "enter",
            "ctrl+shift+a",
            "--format",
            "json",
        ]);
        join_peer(handler, &output);
        assert_eq!(
            output.status.code(),
            Some(match mode {
                "accepted" => 0,
                "partial" => 9,
                "rejected" => 3,
                _ => 8,
            }),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &document,
            if mode == "lost" {
                include_str!("../src/cli/schemas/orchbee.cli.failure.json")
            } else {
                include_str!("../src/cli/schemas/orchbee.terminal.send.json")
            },
        );
    }
}

#[test]
fn agent_read_attaches_only_to_the_exact_presentation_join() {
    use orchbee_client_server_protocol::*;
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let handler = peer(&fixture, listener, move |mut stream| {
        bound_agent_reply(&mut stream, session_detail("sess-alpha"));
        let ClientMessage::TerminalViewer(request) = read_client_message(&mut stream).unwrap()
        else {
            panic!("viewer");
        };
        match request.action() {
            TerminalViewerAction::AttachBinding { terminal_id, .. } => {
                assert_eq!(terminal_id.as_str(), "33333333333333333333333333333333")
            }
            _ => panic!("attach only"),
        }
        write_server_message(
            &mut stream,
            &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                request.request_id().clone(),
                TerminalViewerOutcome::Rejected {
                    reason: ViewerRejectedReason::TerminalUnavailable,
                },
            )),
        )
        .unwrap();
        assert_closed(&mut stream);
    });
    let output = fixture.run(&["agent", "read", "alpha", "--format", "json"]);
    join_peer(handler, &output);
    assert_eq!(output.status.code(), Some(4));
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    validate(
        &document,
        include_str!("../src/cli/schemas/orchbee.cli.failure.json"),
    );
}

#[test]
fn agent_visible_read_returns_a_schema_valid_frame_and_detaches_before_output() {
    use orchbee_client_server_protocol::*;
    use orchbee_terminal_protocol::{
        CURRENT_PROTOCOL_VERSION, Envelope, MessageRole, PayloadEncoding,
        encode_transport_envelope, inspect_terminal_frame_route,
    };
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    for projected in [false, true] {
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_agent_reply(&mut stream, session_detail("sess-alpha"));
            let bytes = include_bytes!(
                "../../../packages/terminal-protocol/fixtures/ghostty-surface-v0.0.3.bin"
            );
            let route = inspect_terminal_frame_route(bytes).unwrap();
            let attachment_id = ViewerId::parse(&route.attachment_id.to_text()).unwrap();
            let mut exchange = |expected: &str, outcome| {
                let ClientMessage::TerminalViewer(request) =
                    read_client_message(&mut stream).unwrap()
                else {
                    panic!("viewer");
                };
                match (expected, request.action()) {
                    ("attach", TerminalViewerAction::AttachBinding { terminal_id, .. }) => {
                        assert_eq!(terminal_id.as_str(), "33333333333333333333333333333333")
                    }
                    (
                        "poll",
                        TerminalViewerAction::Poll {
                            attachment_id: actual,
                        },
                    )
                    | (
                        "detach",
                        TerminalViewerAction::Detach {
                            attachment_id: actual,
                        },
                    ) => assert_eq!(actual, &attachment_id),
                    (
                        "applied",
                        TerminalViewerAction::FrameApplied {
                            attachment_id: actual,
                            sequence,
                            generation,
                            revision,
                        },
                    ) => {
                        assert_eq!(actual, &attachment_id);
                        assert_eq!(*sequence, route.sequence);
                        assert_eq!(*generation, route.generation);
                        assert_eq!(*revision, route.revision);
                    }
                    _ => panic!("wrong viewer action"),
                }
                write_server_message(
                    &mut stream,
                    &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                        request.request_id().clone(),
                        outcome,
                    )),
                )
                .unwrap();
            };
            exchange(
                "attach",
                TerminalViewerOutcome::Attached {
                    attachment_id: attachment_id.clone(),
                    host_epoch: HostEpoch::parse(&route.host_epoch.to_text()).unwrap(),
                    host_terminal_id: orchbee_client_server_protocol::HostTerminalId::parse(
                        &route.host_epoch.to_text(),
                    )
                    .unwrap(),
                },
            );
            let record = encode_transport_envelope(&Envelope {
                protocol: CURRENT_PROTOCOL_VERSION,
                role: MessageRole::Event,
                kind: 41,
                payload_encoding: PayloadEncoding::TerminalFrame,
                host_epoch: Some(route.host_epoch),
                correlation_id: None,
                optional_message: false,
                payload: bytes,
            })
            .unwrap();
            exchange("poll", TerminalViewerOutcome::Record(record));
            exchange("applied", TerminalViewerOutcome::FeedbackAccepted {});
            exchange("detach", TerminalViewerOutcome::Detached {});
            assert_closed(&mut stream);
        });
        let mut arguments = vec![
            "agent",
            "read",
            "--name",
            "alpha",
            "--source=visible",
            "--format",
            "json",
        ];
        if projected {
            arguments.extend(["--lines", "1"]);
        }
        let output = fixture.run(&arguments);
        join_peer(handler, &output);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &document,
            include_str!("../src/cli/schemas/orchbee.terminal.output.json"),
        );
        assert_eq!(document["version"], 1);
        assert_eq!(
            document["result"]["terminal"]["terminalId"],
            "33333333333333333333333333333333"
        );
    }
}

#[test]
fn agent_terminal_never_acts_on_a_different_session_in_the_get_reply() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let handler = peer(&fixture, listener, move |mut stream| {
        bound_agent_reply(&mut stream, session_detail("other-session"));
        assert_closed(&mut stream);
    });
    let output = fixture.run(&[
        "agent",
        "send-keys",
        "--name",
        "alpha",
        "enter",
        "--format",
        "json",
    ]);
    join_peer(handler, &output);
    assert_eq!(output.status.code(), Some(5));
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    validate(
        &document,
        include_str!("../src/cli/schemas/orchbee.cli.failure.agent.json"),
    );
    assert_eq!(document["failure"]["kind"], "protocol-invalid");
}

#[test]
fn infinite_wait_preserves_condition_names_and_rejects_a_different_session() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    for (condition, wrong_session) in [("blocked", false), ("done", false), ("done", true)] {
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            let mut detail = session_detail(if wrong_session {
                "sess-other"
            } else {
                "sess-alpha"
            });
            if condition == "done" {
                detail["snapshot"]["pendingRequests"] = json!([]);
                detail["snapshot"]["turn"] = json!({"kind":"idle"});
                detail["snapshot"]["lastOutcome"] =
                    json!({"kind":"succeeded", "eventId":"finished", "turnId":"turn-9"});
                detail["status"] = json!({"kind":"succeeded"});
            }
            wait::checkpoint(&mut stream, 7);
            read_reply(
                &mut stream,
                "get",
                json!({"kind":"found", "revision":7, "session":detail}),
            );
            assert_closed(&mut stream);
        });
        let output = fixture.run(&[
            "agent",
            "wait",
            "--agent-session",
            "sess-alpha",
            "--forever",
            "--until",
            condition,
            "--format",
            "json",
        ]);
        join_peer(handler, &output);
        assert_eq!(
            output.status.code(),
            Some(if wrong_session { 5 } else { 0 })
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        if wrong_session {
            validate(
                &document,
                include_str!("../src/cli/schemas/orchbee.cli.failure.agent.json"),
            );
            assert_eq!(document["failure"]["kind"], "protocol-invalid");
        } else {
            validate(
                &document,
                include_str!("../src/cli/schemas/orchbee.agent.wait.json"),
            );
            assert_eq!(document["version"], 7);
            assert_eq!(document["result"]["matchedConditions"], json!([condition]));
            assert!(document["result"]["wait"]["timeoutMs"].is_null());
        }
    }
}

#[test]
fn infinite_wait_still_times_out_a_stalled_read_without_reconnecting() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let handler = peer(&fixture, listener, move |mut stream| {
        let AgentRequest::Changes(_) = read_agent_request(&mut stream).unwrap() else {
            panic!("initial checkpoint");
        };
        let started = Instant::now();
        assert_closed(&mut stream);
        assert!(started.elapsed() < Duration::from_millis(6500));
    });
    let output = fixture.run(&[
        "agent",
        "wait",
        "--agent-session",
        "sess-alpha",
        "--forever",
        "--format",
        "json",
    ]);
    join_peer(handler, &output);
    assert_eq!(output.status.code(), Some(7));
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    validate(
        &document,
        include_str!("../src/cli/schemas/orchbee.cli.failure.agent.json"),
    );
    assert_eq!(document["version"], 2);
    assert_eq!(document["failure"]["details"]["reason"], "read-deadline");
    assert!(document["failure"]["details"]["timeoutMs"].is_null());
}

fn presentation_reply(
    stream: &mut orchbee_local_ipc::LocalStream,
    operation: &str,
    outcome: Value,
) -> String {
    let AgentRequest::Names(request) = read_agent_request(stream).unwrap() else {
        panic!("names request")
    };
    assert_eq!(
        json!(request)["kind"],
        format!("agent.session.{operation}.request")
    );
    assert_eq!(
        json!(request)["target"],
        json!({"kind":"presentation","presentationId":"11111111111111111111111111111111"})
    );
    let request_id = request.request_id().as_str().to_owned();
    write_agent_result(stream, &AgentResult::Names(serde_json::from_value(json!({
        "kind":format!("agent.session.{operation}.result"), "requestId":request_id, "result":{"outcome":outcome}
    })).unwrap())).unwrap();
    request_id
}

fn session_target_reply(
    stream: &mut orchbee_local_ipc::LocalStream,
    operation: &str,
    outcome: Value,
) -> String {
    let AgentRequest::Names(request) = read_agent_request(stream).unwrap() else {
        panic!("names request")
    };
    assert_eq!(
        json!(request)["kind"],
        format!("agent.session.{operation}.request")
    );
    assert_eq!(
        json!(request)["target"],
        json!({"kind":"session","sessionId":"sess-alpha"})
    );
    let request_id = request.request_id().as_str().to_owned();
    write_agent_result(
        stream,
        &AgentResult::Names(
            serde_json::from_value(json!({
                "kind":format!("agent.session.{operation}.result"), "requestId":request_id,
                "result":{"outcome":outcome}
            }))
            .unwrap(),
        ),
    )
    .unwrap();
    request_id
}

fn panel_projection(revision: u64, details: &[Value]) -> AgentProjectionFixture {
    AgentProjectionFixture::canonical_with_current_incarnation(
        bound_snapshot(9),
        revision,
        details,
        "11111111111111111111111111111111",
        "33333333333333333333333333333333",
        "66666666666666666666666666666666",
        "77777777777777777777777777777777",
    )
}

#[test]
fn panel_get_and_wait_share_admission_and_pin_the_primary_presentations_session() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    for wait in [false, true] {
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            let detail = session_detail("sess-alpha");
            let outer = (!wait).then(|| begin_get_projection(&mut stream, 2, &[detail.clone()]));
            if wait {
                bound_snapshot_reply(&mut stream);
            }
            let selected = panel_projection(if wait { 1 } else { 2 }, &[detail.clone()]);
            selected.read(&mut stream, false);
            session_target_reply(
                &mut stream,
                "resolve",
                json!({"kind":"resolved","sessionId":"sess-alpha","resolvedBy":"session-id"}),
            );
            let mut detail = detail;
            if wait {
                wait::checkpoint(&mut stream, 1);
            }
            read_reply(
                &mut stream,
                "get",
                json!({"kind":"found","revision":if wait { 1 } else { 2 },"session":detail}),
            );
            if wait {
                detail["snapshot"]["turn"] = json!({"kind":"idle"});
                detail["snapshot"]["pendingRequests"] = json!([]);
                detail["status"] = json!({"kind":"idle"});
                detail["snapshot"]["observation"]["sequence"] = json!(12);
                if wait {
                    wait::checkpoint(&mut stream, 1);
                }
                read_reply(
                    &mut stream,
                    "get",
                    json!({"kind":"found","revision":2,"session":detail}),
                );
            }
            if !wait {
                metadata_peer::sample(&mut stream, "observed", "claude-code.exe");
                read_reply(
                    &mut stream,
                    "get",
                    json!({"kind":"found","revision":2,"session":detail}),
                );
                bound_snapshot_reply(&mut stream);
                management_reply(&mut stream, 2, json!({"phase":"unmanaged"}));
                outer.unwrap().confirm(&mut stream, true);
            }
            assert_closed(&mut stream);
        });
        let mut args = vec![
            "agent",
            if wait { "wait" } else { "get" },
            "--panel",
            "@panel:222",
            "--format",
            "json",
        ];
        if wait {
            args.extend(["--until", "idle", "--timeout", "5000"]);
        }
        let output = fixture.run(&args);
        join_peer(handler, &output);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &doc,
            if wait {
                include_str!("../src/cli/schemas/orchbee.agent.wait.json")
            } else {
                include_str!("../src/cli/schemas/orchbee.agent.get.json")
            },
        );
    }
}

#[test]
fn positional_panel_fallback_preserves_session_or_alias_precedence_and_resolve_provenance() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    for resolved_by in ["session-id", "name", "presentation"] {
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let (source_tx, source_rx) = std::sync::mpsc::channel();
        let handler = peer(&fixture, listener, move |mut stream| {
            let source = if resolved_by == "presentation" {
                resolve_reply(&mut stream, json!({"kind":"rejected","reason":"not-found"}));
                bound_snapshot_reply(&mut stream);
                presentation_reply(
                    &mut stream,
                    "resolve",
                    json!({"kind":"resolved","sessionId":"sess-alpha","resolvedBy":"presentation"}),
                )
            } else {
                resolve_reply(
                    &mut stream,
                    json!({"kind":"resolved","sessionId":"sess-alpha","resolvedBy":resolved_by}),
                )
            };
            assert_closed(&mut stream);
            source_tx.send(source).unwrap();
        });
        let output = fixture.run(&[
            "agent",
            "resolve",
            "22222222222222222222222222222222",
            "--format",
            "json",
        ]);
        join_peer(handler, &output);
        let source = source_rx.recv().unwrap();
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &doc,
            include_str!("../src/cli/schemas/orchbee.agent.resolve.json"),
        );
        assert_eq!(doc["source"]["requestId"], source);
        assert_eq!(doc["result"]["resolvedBy"], resolved_by);
        assert_eq!(
            doc["version"],
            if resolved_by == "presentation" { 2 } else { 1 }
        );
    }
}

#[test]
fn panel_rename_distinguishes_ambiguous_refusal_from_a_pre_mutation_read_loss() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    for read_loss in [false, true] {
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            if read_loss {
                assert!(matches!(
                    orchbee_workbench_protocol::read_workbench_request(&mut stream).unwrap(),
                    orchbee_workbench_protocol::WorkbenchRequest::SnapshotRead(_)
                ));
                return;
            }
            bound_snapshot_reply(&mut stream);
            let selected = panel_projection(1, &[session_detail("sess-alpha")]);
            selected.read(&mut stream, false);
            session_target_reply(
                &mut stream,
                "name.assign",
                json!({"kind":"rejected","reason":"ambiguous-target","conflictSessionId":null}),
            );
            assert_closed(&mut stream);
        });
        let output = fixture.run(&[
            "agent",
            "rename",
            "--panel",
            "@panel:222",
            "new-name",
            "--format",
            "json",
        ]);
        join_peer(handler, &output);
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_ne!(doc["failure"]["kind"], "outcome-unknown");
        if read_loss {
            validate(
                &doc,
                include_str!("../src/cli/schemas/orchbee.cli.failure.json"),
            );
            assert_ne!(output.status.code(), Some(0));
        } else {
            validate(
                &doc,
                include_str!("../src/cli/schemas/orchbee.cli.failure.agent-names.json"),
            );
            assert_eq!(doc["failure"]["kind"], "ambiguous");
            assert_eq!(doc["version"], 2);
        }
    }
}

#[test]
fn panel_send_keeps_primary_presentation_when_the_agent_also_owns_a_child() {
    use orchbee_client_server_protocol::{ClientMessage, read_client_message};
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let handler = peer(&fixture, listener, move |mut stream| {
        bound_snapshot_reply(&mut stream);
        let selected = panel_projection(1, &[session_detail("sess-alpha")]);
        selected.read(&mut stream, false);
        session_target_reply(
            &mut stream,
            "resolve",
            json!({"kind":"resolved","sessionId":"sess-alpha","resolvedBy":"session-id"}),
        );
        let mut detail = session_detail("sess-alpha");
        detail["snapshot"]["presentationAttachments"].as_array_mut().unwrap().push(json!({"generation":"gen-2","navigable":true,"presentationId":"44444444444444444444444444444444","visibility":"hidden"}));
        read_reply(
            &mut stream,
            "get",
            json!({"kind":"found","revision":1,"session":detail}),
        );
        runtime_preflight(&mut stream, detail.clone());
        let ClientMessage::TerminalSend(request) = read_client_message(&mut stream).unwrap() else {
            panic!("terminal send")
        };
        assert_eq!(
            request.terminal_id().as_str(),
            "33333333333333333333333333333333"
        );
        // A lost delivery reply must neither switch to the child nor replay input.
    });
    let output = fixture.run(&[
        "agent",
        "send-keys",
        "--panel",
        "@panel:222",
        "enter",
        "--format",
        "json",
    ]);
    join_peer(handler, &output);
    let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(doc["failure"]["kind"], "outcome-unknown", "{doc}");
    assert_eq!(output.status.code(), Some(8));
    validate(
        &doc,
        include_str!("../src/cli/schemas/orchbee.cli.failure.json"),
    );
}

#[test]
fn missing_panel_is_a_snapshot_refusal_without_an_agent_operation() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let handler = peer(&fixture, listener, move |mut stream| {
        bound_snapshot_reply(&mut stream);
        assert_closed(&mut stream);
    });
    let output = fixture.run(&[
        "agent",
        "rename",
        "--panel",
        "@panel:999",
        "alpha",
        "--format",
        "json",
    ]);
    join_peer(handler, &output);
    let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(doc["failure"]["kind"], "not-found");
    assert_eq!(doc["failure"]["details"]["resource"], "panel");
    validate(
        &doc,
        include_str!("../src/cli/schemas/orchbee.cli.failure.json"),
    );
}

#[test]
fn agent_input_refuses_runtime_absence_decoys_mismatch_and_ambiguity_without_a_send() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    for (case, reason) in [
        ("shell", "agent-not-running"),
        ("eval", "agent-not-running"),
        ("mismatch", "agent-provider-mismatch"),
        ("ambiguous", "agent-runtime-ambiguous"),
        ("unknown", "agent-provider-unsupported"),
        ("alias", "agent-provider-unsupported"),
        ("empty", "agent-not-running"),
    ] {
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            let mut detail = session_detail("sess-alpha");
            if case == "unknown" {
                detail["snapshot"]["providerId"] = json!("acme.agent");
            }
            if case == "alias" {
                detail["snapshot"]["providerId"] = json!("codex.exe");
            }
            bound_agent_reply(&mut stream, detail);
            let mut rows = vec![process(10, None, "cmd.exe")];
            match case {
                "shell" => {}
                "empty" => rows.clear(),
                "eval" => {
                    let mut node = process(20, Some(10), "node.exe");
                    node.command_line = Some("node.exe -e claude claude.js".into());
                    rows.push(node);
                }
                "mismatch" => rows.push(process(20, Some(10), "codex.exe")),
                "ambiguous" => {
                    rows.push(process(20, Some(10), "claude.exe"));
                    rows.push(process(30, Some(10), "codex.exe"));
                }
                "unknown" => rows.push(process(20, Some(10), "claude.exe")),
                "alias" => rows.push(process(20, Some(10), "codex.exe")),
                _ => unreachable!(),
            }
            process_reply(&mut stream, rows);
            assert_closed(&mut stream);
        });
        let output = fixture.run(&["agent", "send-keys", "alpha", "enter", "--format", "json"]);
        join_peer(handler, &output);
        assert_eq!(
            output.status.code(),
            Some(3),
            "{case}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &document,
            include_str!("../src/cli/schemas/orchbee.cli.failure.agent-runtime.json"),
        );
        assert_eq!(document["failure"]["details"]["reason"], reason);
        assert!(String::from_utf8_lossy(&output.stderr).contains("panel process-info"));
    }
}

#[test]
fn runtime_preflight_rechecks_canonical_binding_before_input() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    for change in ["ended", "provider", "presentation", "generation"] {
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_agent_reply(&mut stream, session_detail("sess-alpha"));
            process_reply(&mut stream, vec![process(10, None, "claude.exe")]);
            let mut next = session_detail("sess-alpha");
            match change {
                "ended" => {
                    next["snapshot"]["lifecycle"] = json!("ended");
                    next["snapshot"]["turn"] = json!({"kind":"idle"});
                    next["snapshot"]["pendingRequests"] = json!([]);
                    next["status"] = json!({"kind":"ended"});
                }
                "provider" => next["snapshot"]["providerId"] = json!("codex"),
                "generation" => {
                    next["snapshot"]["presentationAttachments"][0]["generation"] = json!("gen-new")
                }
                "presentation" => {
                    next["snapshot"]["presentationAttachments"][0]["navigable"] = json!(false)
                }
                _ => unreachable!(),
            }
            read_reply(
                &mut stream,
                "get",
                json!({"kind":"found","revision":2,"session":next}),
            );
            assert_closed(&mut stream);
        });
        let output = fixture.run(&["agent", "send-keys", "alpha", "enter", "--format", "json"]);
        join_peer(handler, &output);
        assert_eq!(
            output.status.code(),
            Some(3),
            "{change}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &doc,
            include_str!("../src/cli/schemas/orchbee.cli.failure.agent-runtime.json"),
        );
        assert_eq!(doc["failure"]["details"]["reason"], "agent-binding-changed");
    }
}

#[test]
fn a_panel_get_refuses_a_reply_for_another_session() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let handler = peer(&fixture, listener, move |mut stream| {
        let detail = session_detail("sess-alpha");
        begin_get_projection(&mut stream, 1, &[detail.clone()]);
        let selected = panel_projection(1, &[detail]);
        selected.read(&mut stream, false);
        session_target_reply(
            &mut stream,
            "resolve",
            json!({"kind":"resolved","sessionId":"sess-alpha","resolvedBy":"session-id"}),
        );
        read_reply(
            &mut stream,
            "get",
            json!({"kind":"found","revision":1,"session":session_detail("other-session")}),
        );
        assert_closed(&mut stream);
    });
    let output = fixture.run(&["agent", "get", "--panel", "@panel:222", "--format", "json"]);
    join_peer(handler, &output);
    let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(doc["failure"]["kind"], "protocol-invalid");
    assert!(doc.get("source").is_none());
    validate(
        &doc,
        include_str!("../src/cli/schemas/orchbee.cli.failure.agent.json"),
    );
}
