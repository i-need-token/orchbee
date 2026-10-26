//! Resolve a live Agent before one Client navigation; never replay uncertain focus.
#![cfg(windows)]
#[path = "support/windows_agent_peer/cohort_fixture.rs"]
mod cohort_fixture;
#[allow(dead_code)]
#[path = "support/terminal_metadata_peer.rs"]
mod metadata_peer;
#[allow(dead_code)]
#[path = "support/windows_agent_peer.rs"]
mod peer_support;
use cohort_fixture::CohortFixture as FocusFixture;
use orchbee_agent_observation_protocol::{AgentRequest, read_agent_request, write_agent_result};
use orchbee_local_ipc::{LocalListener, LocalStream};
use orchbee_workbench_protocol::*;
use peer_support::{
    AgentProjectionFixture, assert_closed, peer, read_reply, resolve_reply, session_detail,
    validate,
};
use serde_json::{Value, json};
use std::{
    io::Write,
    time::{Duration, Instant},
};

#[path = "cli_windows_agent_focus/client_get.rs"]
mod client_get;
#[path = "cli_windows_agent_focus/client_list.rs"]
mod client_list;
#[path = "cli_windows_agent_focus/client_wait.rs"]
mod client_wait;
#[path = "cli_windows_agent_focus/workbench.rs"]
mod workbench;

fn id(n: u64) -> String {
    format!("{n:032x}")
}
fn panel(n: u64) -> Value {
    json!({"kind":"panel","panelId":id(n),"primaryPresentation":{"customLabel":null,"presentationId":id(n+100),"label":"Agent","binding":{"terminalId":id(n+200),"workerId":"6f7263686265652d6c6f63616c2d7631"}},"childPresentations":[{"customLabel":null,"presentationId":id(n+300),"label":"child","binding":{"terminalId":id(n+400),"workerId":"6f7263686265652d6c6f63616c2d7631"}}]})
}
fn snapshot_reply(stream: &mut LocalStream) {
    snapshot_at(stream, 12);
}
fn snapshot(revision: u64) -> Snapshot {
    serde_json::from_value(json!({"protocolVersion":1,"siteId":id(2),"revision":revision.to_string(),"navigation":{"projects":[], "directories":[], "workspaces":[{"workspaceId":id(4),"terminalContextId":id(4),"projectId":null,"primaryDirectoryId":null,"label":"Workspace"}]},"terminalContexts":[{"terminalContextId":id(4),"groups":[],"tabs":[{"tabId":id(5),"label":"two","pinned":false,"groupId":null,"root":{"kind":"split","splitId":id(50),"orientation":"horizontal","children":[{"slotId":id(51),"node":panel(6)},{"slotId":id(52),"node":panel(8)}]}}]}]})).unwrap()
}
fn snapshot_at(stream: &mut LocalStream, revision: u64) {
    let admitted = snapshot(12);
    let request = peer_support::snapshot_after_metadata(stream, &admitted);
    write_workbench_response(
        stream,
        &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            outcome: WorkbenchSnapshotOutcome::Available {
                snapshot: Snapshot {
                    revision: WorkbenchRevision::new(revision),
                    ..admitted
                },
            },
        }),
    )
    .unwrap();
}
fn record(client: u64, panel: u64) -> Value {
    json!({"clientId":id(client),"heartbeat":"1","viewRevision":"2","availability":"current","view":{"workbenchRevision":"12","phase":"ready","selection":{"terminalContextId":id(4),"tabId":id(5),"panelId":id(panel),"zoomed":false,"panelCount":2}}})
}
fn view_request(stream: &mut LocalStream) -> WorkbenchClientViewRequest {
    let WorkbenchRequest::ClientView(request) = read_workbench_request(stream).unwrap() else {
        panic!("Client view only");
    };
    request
}
fn view_reply(stream: &mut LocalStream, request: &WorkbenchClientViewRequest, outcome: Value) {
    write_workbench_response(
        stream,
        &WorkbenchResponse::ClientView(WorkbenchClientViewResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id.clone(),
            site_id: request.site_id.clone(),
            outcome: serde_json::from_value(outcome).unwrap(),
        }),
    )
    .unwrap();
}
fn list_reply(stream: &mut LocalStream, clients: Vec<Value>) {
    let request = view_request(stream);
    assert!(matches!(request.action, ClientViewAction::List {}));
    view_reply(stream, &request, json!({"kind":"listed","clients":clients}));
}
fn detail(presentations: &[u64]) -> Value {
    let mut detail = session_detail("sess-alpha");
    detail["snapshot"]["presentationAttachments"]=json!(presentations.iter().map(|n|json!({"generation":"gen-2","navigable":true,"presentationId":id(*n),"visibility":"visible"})).collect::<Vec<_>>());
    detail
}
fn resolved(stream: &mut LocalStream) {
    resolve_reply(
        stream,
        json!({"kind":"resolved","sessionId":"sess-alpha","resolvedBy":"name"}),
    );
}
fn get_reply(stream: &mut LocalStream, detail: Value) {
    read_reply(
        stream,
        "get",
        json!({"kind":"found","revision":1,"session":detail}),
    );
}
fn submitted(stream: &mut LocalStream) -> (WorkbenchClientViewRequest, Box<ClientViewControl>) {
    let request = view_request(stream);
    let ClientViewAction::Submit { command } = &request.action else {
        panic!("one Submit");
    };
    assert_eq!(command.client_id.as_str(), id(7));
    assert_eq!(json!(command.expected_view_revision), "2");
    assert_eq!(
        command.minimum_workbench_revision,
        WorkbenchRevision::new(12)
    );
    assert_eq!(
        json!(command.intent),
        json!({"kind":"agent-focus","terminalContextId":id(4),"tabId":id(5),"panelId":id(8),
            "presentationId":id(108),"target":{"kind":"session","sessionId":"sess-alpha"}})
    );
    let command = command.clone();
    (request, command)
}
fn settled(
    stream: &mut LocalStream,
    request: &WorkbenchClientViewRequest,
    command: &ClientViewControl,
) {
    view_reply(
        stream,
        request,
        json!({"kind":"operation","operation":{"command":command,"status":{"kind":"applied","view":record(7,8)["view"]}}}),
    );
}
fn document(output: &std::process::Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|err| panic!("{err}: {output:?}"))
}
fn receipt(document: &Value) {
    validate(
        document,
        include_str!("../src/cli/schemas/orchbee.client.control.json"),
    );
}

#[test]
fn physical_agent_focus_pins_the_full_binding_and_never_substitutes_a_childs_parent() {
    for case in ["primary", "child", "no-agent"] {
        let fixture = FocusFixture::new();
        let rendezvous = fixture.rendezvous();
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let server = peer(&fixture, listener, move |mut stream| {
            snapshot_reply(&mut stream);
            list_reply(&mut stream, vec![record(7, 6)]);
            let binding = json!({"presentationId":id(if case == "child" {308} else {108}),
                "generation":id(800),"hostEpoch":id(801),"hostTerminalId":id(802)});
            resolve_reply(
                &mut stream,
                json!({"kind":"resolved-target","resolvedBy":"name",
                "target":{"kind":"terminal","binding":binding}}),
            );
            let WorkbenchRequest::PanelAgent(request) =
                read_workbench_request(&mut stream).unwrap()
            else {
                panic!("physical Observe")
            };
            assert_eq!(
                json!(request.action),
                json!({"kind":"observe","binding":binding,"afterSequence":null})
            );
            write_workbench_response(&mut stream,&WorkbenchResponse::PanelAgent(PanelAgentResult {
                protocol_version:ProtocolVersion, request_id:request.request_id, site_id:request.site_id,
                outcome:PanelAgentOutcome::Observed {
                    workbench_revision:WorkbenchRevision::new(12), report_revision:WorkbenchRevision::new(3),
                    observation:Box::new(serde_json::from_value(json!({"binding":binding,"sequence":"0","changes":[],
                        "view":{"agent":if case == "no-agent" {Value::Null} else {json!("pi")},"state":"unknown","authoritySource":null,"message":null}})).unwrap()),
                },
            })).unwrap();
            if case == "primary" {
                let request = view_request(&mut stream);
                let ClientViewAction::Submit { command } = &request.action else {
                    panic!("single focus Submit")
                };
                assert_eq!(
                    json!(command.intent),
                    json!({"kind":"agent-focus","terminalContextId":id(4),"tabId":id(5),"panelId":id(8),"presentationId":id(108),"target":{"kind":"terminal","binding":binding}})
                );
                settled(&mut stream, &request, command);
            }
            assert_closed(&mut stream);
        });
        let output = fixture.run(&["agent", "focus", "--name", "physical", "--format=json"]);
        server.join().unwrap();
        assert_eq!(output.status.success(), case == "primary", "{output:?}");
        if case == "primary" {
            let document = document(&output);
            assert_eq!(document["version"], 1);
            receipt(&document);
        }
    }
}

#[test]
fn unified_agent_selectors_complete_before_resolving_or_submitting_focus() {
    for case in [
        "physical-only",
        "matched-alias",
        "matched-ordinal",
        "ambiguous",
        "not-found",
    ] {
        let fixture = FocusFixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let server = peer(&fixture, listener, move |mut stream| {
            snapshot_reply(&mut stream);
            list_reply(&mut stream, vec![record(7, 6)]);
            let mut canonical = client_get::completed();
            canonical["snapshot"]["providerId"] = json!("grok");
            canonical["snapshot"]["presentationAttachments"] = json!([{
                "generation":id(1),"navigable":true,"presentationId":id(106),"visibility":"visible"
            }]);
            let details = if matches!(case, "matched-alias" | "matched-ordinal") {
                vec![canonical]
            } else {
                Vec::new()
            };
            let mut projection = AgentProjectionFixture::canonical(snapshot(12), 5, &details);
            if matches!(case, "physical-only" | "matched-alias" | "matched-ordinal") {
                projection = projection.with_state(
                    &id(106),
                    Some(peer_support::terminal_agent_state(
                        &id(106),
                        &id(1),
                        &id(2),
                        &id(206),
                        Some(if case == "physical-only" {
                            "pi"
                        } else {
                            "grok-build"
                        }),
                        "idle",
                    )),
                );
            } else if case == "ambiguous" {
                for presentation in [106, 108] {
                    projection = projection.with_state(
                        &id(presentation),
                        Some(peer_support::terminal_agent_state(
                            &id(presentation),
                            &id(1),
                            &id(2),
                            &id(presentation + 100),
                            Some("codex"),
                            "idle",
                        )),
                    );
                }
            }
            projection.read(&mut stream, false);
            if matches!(case, "ambiguous" | "not-found") {
                assert_closed(&mut stream);
                return;
            }
            let binding = json!({
                "presentationId":id(106),"generation":id(1),
                "hostEpoch":id(2),"hostTerminalId":id(206)
            });
            let AgentRequest::Names(request) = read_agent_request(&mut stream).unwrap() else {
                panic!("selector resolves its exact projected target")
            };
            assert_eq!(
                json!(request)["target"],
                json!({"kind":"terminal","binding":binding})
            );
            write_agent_result(&mut stream,&orchbee_agent_observation_protocol::AgentResult::Names(serde_json::from_value(json!({
                "kind":"agent.session.resolve.result","requestId":request.request_id(),"result":{"outcome":{
                    "kind":"resolved-target","resolvedBy":"presentation","target":{"kind":"terminal","binding":binding}
                }}
            })).unwrap())).unwrap();
            let WorkbenchRequest::PanelAgent(request) =
                read_workbench_request(&mut stream).unwrap()
            else {
                panic!("physical selector observation")
            };
            assert_eq!(
                json!(request.action),
                json!({"kind":"observe","binding":binding,"afterSequence":null})
            );
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::PanelAgent(PanelAgentResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    site_id: request.site_id,
                    outcome: PanelAgentOutcome::Observed {
                        workbench_revision: WorkbenchRevision::new(12),
                        report_revision: WorkbenchRevision::new(7),
                        observation: Box::new(
                            serde_json::from_value(
                                json!({"binding":binding,"sequence":"4","changes":[],
                        "view":{"agent":if case == "physical-only" { "pi" } else { "grok-build" },
                            "state":"idle","authoritySource":null,"message":null}}),
                            )
                            .unwrap(),
                        ),
                    },
                }),
            )
            .unwrap();
            let request = view_request(&mut stream);
            let ClientViewAction::Submit { command } = &request.action else {
                panic!("one focus submission after complete selection")
            };
            assert_eq!(
                json!(command.intent),
                json!({"kind":"agent-focus","terminalContextId":id(4),"tabId":id(5),
                    "panelId":id(6),"presentationId":id(106),
                    "target":{"kind":"terminal","binding":binding}})
            );
            view_reply(
                &mut stream,
                &request,
                json!({"kind":"operation","operation":{"command":command,"status":{
                    "kind":"applied","view":record(7,6)["view"]
                }}}),
            );
            assert_closed(&mut stream);
        });
        let client = id(7);
        let selector = match case {
            "physical-only" => "@agent:pi",
            "matched-alias" => "@agent:grok",
            "matched-ordinal" => "@agent:1",
            "ambiguous" => "@agent:codex",
            _ => "@agent:qwen",
        };
        let output = fixture.run(&[
            "agent",
            "focus",
            selector,
            "--client",
            &client,
            "--format=json",
        ]);
        server.join().unwrap();
        let document = document(&output);
        if matches!(case, "ambiguous" | "not-found") {
            assert_eq!(output.status.code(), Some(3), "{document}");
            validate(
                &document,
                include_str!("../src/cli/schemas/orchbee.cli.failure.agent-projection.json"),
            );
            assert_eq!(
                document["failure"]["kind"],
                if case == "ambiguous" {
                    "ambiguous"
                } else {
                    "not-found"
                }
            );
            continue;
        }
        assert!(output.status.success(), "{document}");
        receipt(&document);
    }
}

#[test]
fn names_sessions_and_explicit_panels_select_one_primary_without_touching_other_clients() {
    for selector in [
        vec!["alpha"],
        vec!["--name", "alpha"],
        vec!["--agent-session", "sess-alpha"],
        vec!["--panel", "@panel:0x8"],
    ] {
        let fixture = FocusFixture::new();
        let rendezvous = fixture.rendezvous();
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let direct = selector[0] == "--agent-session";
        let pinned = selector[0] == "--panel";
        let busy = selector[0] == "--name";
        let server = peer(&fixture, listener, move |mut stream| {
            snapshot_reply(&mut stream);
            let mut selected = record(7, 6);
            if busy {
                selected["view"]["phase"] = json!("busy");
            }
            list_reply(&mut stream, vec![selected, record(9, 6)]);
            let session = if pinned {
                peer_support::with_attachment_generation(detail(&[106, 108]), &id(108), &id(1))
            } else {
                detail(&[108])
            };
            if pinned {
                AgentProjectionFixture::canonical_with_current_incarnation(
                    snapshot(12),
                    1,
                    std::slice::from_ref(&session),
                    &id(108),
                    &id(1),
                    &id(2),
                    &id(208),
                )
                .read(&mut stream, false);
            }
            if !direct {
                resolved(&mut stream);
            }
            get_reply(&mut stream, session);
            let (request, command) = submitted(&mut stream);
            settled(&mut stream, &request, &command);
            assert_closed(&mut stream);
        });
        let client = id(7);
        let mut args = vec!["agent", "focus"];
        args.extend(selector);
        args.extend(["--client", &client, "--format=json"]);
        // Explicit Client suppresses invalid caller metadata, as other navigation does.
        let output = fixture.run_with_caller(&args, Some("invalid"), Some("invalid"));
        server.join().unwrap();
        assert!(output.status.success(), "{output:?}");
        receipt(&document(&output));
    }
}

#[test]
fn absent_ended_child_only_stale_and_ambiguous_bindings_never_submit() {
    for case in [
        "absent",
        "ended",
        "child-only",
        "stale",
        "non-navigable",
        "ambiguous",
    ] {
        let fixture = FocusFixture::new();
        let rendezvous = fixture.rendezvous();
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let server = peer(&fixture, listener, move |mut stream| {
            snapshot_reply(&mut stream);
            list_reply(&mut stream, vec![record(7, 6)]);
            if case == "absent" {
                resolve_reply(&mut stream, json!({"kind":"rejected","reason":"not-found"}));
                AgentProjectionFixture::summaries(snapshot(12), 1, Vec::new())
                    .read(&mut stream, false);
            } else {
                resolved(&mut stream);
                let mut session = detail(match case {
                    "child-only" => &[308],
                    "stale" => &[999],
                    "ambiguous" => &[106, 108],
                    _ => &[108],
                });
                if case == "ended" {
                    session["snapshot"]["lifecycle"] = json!("ended");
                    session["snapshot"]["pendingRequests"] = json!([]);
                    session["snapshot"]["turn"] = json!({"kind":"idle"});
                }
                if case == "non-navigable" {
                    session["snapshot"]["presentationAttachments"][0]["navigable"] = json!(false);
                }
                get_reply(&mut stream, session);
            }
            assert_closed(&mut stream);
        });
        let output = fixture.run(&["agent", "focus", "alpha", "--format=json"]);
        server.join().unwrap();
        assert!(!output.status.success(), "{case}");
        let document = document(&output);
        validate(
            &document,
            if case == "absent" {
                include_str!("../src/cli/schemas/orchbee.cli.failure.agent-projection.json")
            } else {
                include_str!("../src/cli/schemas/orchbee.cli.failure.json")
            },
        );
        assert_eq!(
            document["failure"]["kind"],
            if case == "ambiguous" {
                "ambiguous"
            } else {
                "not-found"
            },
            "{case}: {document}"
        );
    }
}

#[test]
fn ambiguous_or_unready_clients_fail_before_agent_resolution() {
    for ambiguous in [true, false] {
        let fixture = FocusFixture::new();
        let rendezvous = fixture.rendezvous();
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let server = peer(&fixture, listener, move |mut stream| {
            snapshot_reply(&mut stream);
            let mut first = record(7, 6);
            if !ambiguous {
                first["view"]["phase"] = json!("starting");
                first["view"]["selection"] = Value::Null;
            }
            list_reply(
                &mut stream,
                if ambiguous {
                    vec![first, record(9, 6)]
                } else {
                    vec![first]
                },
            );
            assert_closed(&mut stream);
        });
        let output = fixture.run(&["agent", "focus", "alpha", "--format=json"]);
        server.join().unwrap();
        assert!(!output.status.success());
        if !ambiguous {
            assert!(String::from_utf8_lossy(&output.stderr).contains("not ready"));
        }
        let document = document(&output);
        validate(
            &document,
            include_str!("../src/cli/schemas/orchbee.cli.failure.client-control.json"),
        );
    }
}

#[test]
fn lost_submit_preserves_identity_for_explicit_status_without_resolving_again() {
    let fixture = FocusFixture::new();
    let operation = id(90);
    let rendezvous = fixture.rendezvous();
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let (sent, received) = std::sync::mpsc::channel();
    let server = peer(&fixture, listener, move |mut stream| {
        snapshot_reply(&mut stream);
        list_reply(&mut stream, vec![record(7, 6)]);
        resolved(&mut stream);
        get_reply(&mut stream, detail(&[108]));
        sent.send(submitted(&mut stream).1).unwrap();
    });
    let output = fixture.run(&[
        "agent",
        "focus",
        "alpha",
        "--request-id",
        &operation,
        "--format=json",
    ]);
    server.join().unwrap();
    let command = received.recv().unwrap();
    let document = document(&output);
    receipt(&document);
    assert!(!output.status.success());
    let rendezvous = fixture.rendezvous();
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let server = peer(&fixture, listener, move |mut stream| {
        snapshot_reply(&mut stream);
        let request = view_request(&mut stream);
        assert!(
            matches!(&request.action,ClientViewAction::Read {operation_id} if operation_id==&command.operation_id)
        );
        settled(&mut stream, &request, &command);
        assert_closed(&mut stream);
    });
    let output = fixture.run(&["client", "control-status", &operation, "--format=json"]);
    server.join().unwrap();
    assert!(output.status.success());
    receipt(&serde_json::from_slice(&output.stdout).unwrap());
}

#[test]
fn partial_agent_reply_expires_without_submission_and_uses_total_navigation_deadline() {
    let fixture = FocusFixture::deadline_sensitive();
    let rendezvous = fixture.rendezvous();
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let server = peer(&fixture, listener, |mut stream| {
        snapshot_reply(&mut stream);
        list_reply(&mut stream, vec![record(7, 6)]);
        resolved(&mut stream);
        assert!(matches!(
            read_agent_request(&mut stream).unwrap(),
            AgentRequest::Read(_)
        ));
        stream.write_all(b"OBAP").unwrap();
        assert_closed(&mut stream);
    });
    let started = Instant::now();
    let output = fixture.run(&["agent", "focus", "alpha", "--timeout=1000", "--format=json"]);
    let exited_after = started.elapsed();
    assert!(
        server.join().is_ok(),
        "peer failed; CLI exited after {exited_after:?}; output={output:?}"
    );
    assert_eq!(output.status.code(), Some(7));
    assert!(
        started.elapsed() < Duration::from_secs(8),
        "elapsed={:?}; output={output:?}",
        started.elapsed()
    );
    receipt(&document(&output));
}

#[path = "cli_windows_agent_focus/attention.rs"]
mod attention;
