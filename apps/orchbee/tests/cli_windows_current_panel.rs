//! Windows CLI current-Panel reads: target precedence, stale views and no control effects.
#![cfg(windows)]
#[allow(dead_code)]
#[path = "support/windows_agent_peer.rs"]
mod support;
use orchbee_client_server_protocol as c;
use orchbee_local_ipc::{LocalListener, LocalStream};
use orchbee_terminal_protocol as t;
use orchbee_workbench_protocol::*;
use serde_json::{Value, json};
use std::{
    io::Write,
    time::{Duration, Instant},
};
use support::{Fixture, assert_closed, peer, validate};
fn id(n: u64) -> String {
    format!("{n:032x}")
}
fn panel(n: u64) -> Value {
    json!({"kind":"panel","panelId":id(n),"primaryPresentation":{"customLabel":null,"presentationId":id(n+100),"label":"Agent","binding":{"terminalId":id(n+200),"workerId":"6f7263686265652d6c6f63616c2d7631"}},"childPresentations":[{"customLabel":null,"presentationId":id(n+300),"label":"child","binding":{"terminalId":id(n+400),"workerId":"6f7263686265652d6c6f63616c2d7631"}}]})
}
fn snapshot_reply(stream: &mut LocalStream) {
    snapshot_reply_at(stream, 12);
}
fn snapshot_reply_at(stream: &mut LocalStream, revision: u64) {
    send_snapshot(stream, revision, None);
}
fn send_snapshot(
    stream: &mut LocalStream,
    revision: u64,
    delayed: Option<std::sync::mpsc::Sender<Instant>>,
) -> bool {
    let mut snapshot:Snapshot=serde_json::from_value(json!({"protocolVersion":1,"sessionId":id(2),"revision":"12","navigation":{"projects":[], "directories":[], "workspaces":[{"workspaceId":id(4),"terminalContextId":id(4),"projectId":null,"primaryDirectoryId":null,"label":"Workspace"}]},"terminalContexts":[{"terminalContextId":id(4),"groups":[],"tabs":[{"tabId":id(5),"label":"two","pinned":false,"groupId":null,"root":{"kind":"split","splitId":id(50),"orientation":"horizontal","children":[{"slotId":id(51),"node":panel(6)},{"slotId":id(52),"node":panel(8)}]}}]}]})).unwrap();
    snapshot.revision = WorkbenchRevision::new(revision);
    let request = support::snapshot_after_metadata(stream, &snapshot);
    if let Some(started) = &delayed {
        // Measure the request budget after cohort startup and metadata preflight.
        started.send(Instant::now()).unwrap();
        std::thread::sleep(Duration::from_secs(4));
    }
    let result = write_workbench_response(
        stream,
        &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            outcome: WorkbenchSnapshotOutcome::Available { snapshot },
        }),
    );
    if delayed.is_some() && result.is_err() {
        // The shared deadline may expire during the delayed snapshot itself.
        // The caller still requires a timeout receipt within the original budget.
        return false;
    }
    result.unwrap();
    true
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
            session_id: request.session_id.clone(),
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
fn viewer(stream: &mut LocalStream) -> c::TerminalViewerRequest {
    let c::ClientMessage::TerminalViewer(request) = c::read_client_message(stream).unwrap() else {
        panic!("read-only viewer");
    };
    request
}
fn answer(
    stream: &mut LocalStream,
    request: &c::TerminalViewerRequest,
    outcome: c::TerminalViewerOutcome,
) {
    c::write_server_message(
        stream,
        &c::ServerMessage::TerminalViewerResult(c::TerminalViewerResult::new(
            request.request_id().clone(),
            outcome,
        )),
    )
    .unwrap();
}

fn process_read(stream: &mut LocalStream, terminal: u64, unavailable: bool) {
    let request = viewer(stream);
    assert_eq!(
        json!(request.action()),
        json!({"kind":"attach-binding","sessionId":id(2),"workerId":"6f7263686265652d6c6f63616c2d7631","terminalId":id(terminal)})
    );
    answer(
        stream,
        &request,
        c::TerminalViewerOutcome::Attached {
            attachment_id: c::ViewerId::parse(&id(20)).unwrap(),
            host_epoch: c::HostEpoch::parse(&id(21)).unwrap(),
            host_terminal_id: orchbee_client_server_protocol::HostTerminalId::parse(&id(21))
                .unwrap(),
        },
    );
    let request = viewer(stream);
    assert_eq!(
        json!(request.action()),
        json!({"kind":"read-process-info","attachmentId":id(20)})
    );
    if unavailable {
        answer(
            stream,
            &request,
            c::TerminalViewerOutcome::Unavailable {
                reason: c::ViewerUnavailableReason::TerminalRuntimeUnavailable,
            },
        );
    } else {
        let payload = t::encode_process_info_control(&t::ProcessInfoControl::ProcessInfo(
            t::TerminalProcessInfo {
                attachment_id: t::AttachmentId::parse(&id(20)).unwrap(),
                terminal_id: t::TerminalId::parse(&id(30)).unwrap(),
                root_pid: 40,
                processes: vec![t::TerminalProcess {
                    pid: 40,
                    parent_pid: None,
                    creation_time: 10,
                    image: Some("C:\\Windows\\System32\\cmd.exe".into()),
                    command_line: Some("cmd.exe".into()),
                    current_directory: Some("C:\\项目".into()),
                }],
            },
        ))
        .unwrap();
        let record = t::encode_transport_envelope(&t::Envelope {
            protocol: t::CURRENT_PROTOCOL_VERSION,
            role: t::MessageRole::Response,
            kind: payload.kind,
            optional_message: false,
            payload_encoding: t::PayloadEncoding::ControlJson,
            correlation_id: Some(t::CorrelationId::parse(&id(31)).unwrap()),
            host_epoch: Some(t::HostEpoch::parse(&id(21)).unwrap()),
            payload: &payload.payload,
        })
        .unwrap();
        answer(stream, &request, c::TerminalViewerOutcome::Record(record));
    }
    let detach = viewer(stream);
    assert_eq!(
        json!(detach.action()),
        json!({"kind":"detach","attachmentId":id(20)})
    );
    answer(stream, &detach, c::TerminalViewerOutcome::Detached {});
}
fn document(output: &std::process::Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| panic!("{error}: {output:?}"))
}

#[test]
fn caller_explicit_and_client_targets_preserve_precedence_without_mutating_controls() {
    for process in [false, true] {
        for mode in 0..5 {
            let fixture = Fixture::new();
            let rendezvous = fixture.rendezvous();
            let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
            let selected = if mode <= 1 { 6 } else { 8 };
            let server = peer(&fixture, listener, move |mut stream| {
                snapshot_reply(&mut stream);
                if mode == 3 {
                    list_reply(&mut stream, vec![record(10, 8)]);
                }
                if mode == 4 {
                    list_reply(&mut stream, vec![record(10, 6), record(12, 8)]);
                }
                if process {
                    let client = match mode {
                        3 => Some(10),
                        4 => Some(12),
                        _ => None,
                    };
                    if let Some(client) = client {
                        presentations::read(&mut stream, client, selected, true, 7, "stable");
                    }
                    process_read(
                        &mut stream,
                        selected + if mode == 1 || mode >= 3 { 400 } else { 200 },
                        false,
                    );
                    snapshot_reply(&mut stream);
                    if let Some(client) = client {
                        presentations::read(&mut stream, client, selected, true, 7, "stable");
                    }
                } else {
                    enrichment::read(
                        &mut stream,
                        selected,
                        match mode {
                            3 => Some(10),
                            4 => Some(12),
                            _ => None,
                        },
                        if mode == 1 { "caller-child" } else { "stable" },
                    );
                }
                assert_closed(&mut stream);
            });
            let client = id(12);
            let caller = id(if mode == 0 { 206 } else { 406 });
            let mut args = vec![
                "panel",
                if process { "process-info" } else { "current" },
                "--format=json",
            ];
            if mode == 2 {
                args.push("@panel:0x8");
            }
            if mode == 4 {
                args.extend(["--client", &client, "--current"]);
            }
            let output = fixture.run_with_caller(
                &args,
                match mode {
                    0 | 1 => Some(&caller),
                    2 | 4 => Some("invalid-secret"),
                    _ => None,
                },
                match mode {
                    0 | 1 => Some("6f7263686265652d6c6f63616c2d7631"),
                    _ => None,
                },
            );
            server.join().unwrap();
            assert!(output.status.success(), "{mode}: {output:?}");
            let doc = document(&output);
            validate(
                &doc,
                if process {
                    include_str!("../src/cli/schemas/orchbee.panel.process-info.json")
                } else {
                    include_str!("../src/cli/schemas/orchbee.panel.current.json")
                },
            );
            if process {
                assert_eq!(
                    doc["result"]["terminal"]["terminalId"],
                    id(selected + if mode == 1 || mode >= 3 { 400 } else { 200 })
                );
            } else {
                assert_eq!(doc["version"], 1);
                assert_eq!(doc["result"]["panel"]["panelId"], id(selected));
                assert_eq!(
                    doc["result"]["panel"]["currentPresentation"]["presentationId"],
                    id(selected + if mode == 1 || mode >= 3 { 300 } else { 100 })
                );
                assert_eq!(
                    doc["result"]["panel"]["primaryPresentation"]["presentationId"],
                    id(selected + 100)
                );
                assert_eq!(doc["result"]["panel"]["agentSessionIds"], json!([]));
                assert_eq!(
                    doc["result"]["panel"]["terminalObservation"]["state"],
                    "unavailable"
                );
                if mode < 3 {
                    assert!(doc["result"]["client"].is_null());
                    assert_eq!(doc["result"]["panel"].get("focused"), Some(&Value::Null));
                    assert!(doc["result"]["panel"]["scroll"].is_null());
                    assert_eq!(
                        doc["result"]["panel"]["scrollObservation"]["state"],
                        "no-client"
                    );
                } else {
                    assert_eq!(doc["result"]["panel"]["focused"], true);
                    assert_eq!(doc["result"]["panel"]["scroll"]["offsetFromBottom"], "276");
                    let operations = doc["result"]["client"]["operationIds"].as_array().unwrap();
                    assert_eq!(operations.len(), 2);
                    assert_ne!(operations[0], operations[1]);
                }
            }
        }
    }
}

#[test]
fn absent_ambiguous_unready_stale_or_moved_client_panels_fail_without_viewer_or_control() {
    for case in 0..8 {
        let fixture = Fixture::new();
        let rendezvous = fixture.rendezvous();
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let server = peer(&fixture, listener, move |mut stream| {
            snapshot_reply(&mut stream);
            let mut client = record(10, 6);
            match case {
                2 => {
                    client["view"]["phase"] = json!("starting");
                    client["view"]["selection"] = Value::Null;
                }
                3 => client["availability"] = json!("stale"),
                4 => client["view"]["selection"]["terminalContextId"] = json!(id(55)),
                5 => client["view"]["selection"]["tabId"] = json!(id(55)),
                6 => client["view"]["selection"]["panelId"] = json!(id(55)),
                _ => {}
            }
            let clients = match case {
                0 => vec![],
                1 => vec![client, record(12, 8)],
                _ => vec![client],
            };
            list_reply(&mut stream, clients);
            assert_closed(&mut stream);
        });
        let client = id(if case == 7 { 99 } else { 10 });
        let mut args = vec!["panel", "process-info", "--format=json"];
        if case >= 3 {
            args.extend(["--client", &client]);
        }
        let output = fixture.run(&args);
        server.join().unwrap();
        assert!(!output.status.success());
        let doc = document(&output);
        validate(
            &doc,
            include_str!("../src/cli/schemas/orchbee.cli.failure.client-control.json"),
        );
        assert_eq!(
            doc["failure"]["details"]["reason"],
            match case {
                0 | 7 => "client-not-found",
                1 => "client-ambiguous",
                2 => "client-not-ready",
                3 => "client-stale",
                _ => "client-panel-unavailable",
            }
        );
    }
}

#[test]
fn stale_caller_binding_never_falls_back_and_process_failures_detach() {
    for unavailable in [false, true] {
        let fixture = Fixture::new();
        let rendezvous = fixture.rendezvous();
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let server = peer(&fixture, listener, move |mut stream| {
            snapshot_reply(&mut stream);
            if unavailable {
                list_reply(&mut stream, vec![record(10, 8)]);
                presentations::read(&mut stream, 10, 8, false, 7, "stable");
                process_read(&mut stream, 208, true);
            }
            assert_closed(&mut stream);
        });
        let terminal = id(999);
        let output = fixture.run_with_caller(
            &["panel", "process-info", "--format=json"],
            if unavailable { None } else { Some(&terminal) },
            if unavailable {
                None
            } else {
                Some("6f7263686265652d6c6f63616c2d7631")
            },
        );
        server.join().unwrap();
        assert!(!output.status.success());
        let doc = document(&output);
        validate(
            &doc,
            include_str!("../src/cli/schemas/orchbee.cli.failure.json"),
        );
        assert_eq!(
            doc["failure"]["kind"],
            if unavailable {
                "service-unavailable"
            } else {
                "not-found"
            }
        );
    }
}

#[test]
fn client_list_faults_and_partial_reply_have_closed_failures_and_one_total_budget() {
    for case in 0..5 {
        let fixture = Fixture::new();
        let rendezvous = fixture.rendezvous();
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let (started, request_started) = std::sync::mpsc::channel();
        let server = peer(&fixture, listener, move |mut stream| {
            if !send_snapshot(&mut stream, 12, (case == 4).then_some(started)) {
                return;
            }
            let request = match read_workbench_request(&mut stream) {
                Ok(WorkbenchRequest::ClientView(request)) => request,
                Err(_) if case == 4 => return,
                other => panic!("Client list after snapshot: {other:?}"),
            };
            assert!(matches!(request.action, ClientViewAction::List {}));
            match case {
                0 => return,
                1 => view_reply(&mut stream, &request, json!({"kind":"denied"})),
                2 => view_reply(
                    &mut stream,
                    &request,
                    json!({"kind":"unavailable","reason":"server-busy"}),
                ),
                _ => stream.write_all(b"OBWP").unwrap(),
            }
            assert_closed(&mut stream);
        });
        let output = fixture.run(&["panel", "current", "--format=json"]);
        let finished = Instant::now();
        server.join().unwrap();
        assert!(!output.status.success());
        let doc = document(&output);
        validate(
            &doc,
            include_str!("../src/cli/schemas/orchbee.cli.failure.json"),
        );
        assert_eq!(
            doc["failure"]["kind"],
            match case {
                0 => "transport-loss",
                1 => "denied",
                2 => "service-unavailable",
                _ => "timeout",
            }
        );
        if case == 4 {
            let elapsed = finished.duration_since(request_started.recv().unwrap());
            assert!(elapsed < Duration::from_millis(5900), "{elapsed:?}");
        }
    }
}

#[path = "cli_windows_current_panel/enrichment.rs"]
mod enrichment;

#[path = "cli_windows_current_panel/presentations.rs"]
mod presentations;

use support as peer_support;
#[path = "cli_windows_current_panel/current_member.rs"]
mod current_member;
#[allow(dead_code)]
#[path = "support/terminal_metadata_peer.rs"]
mod terminal_metadata_peer;

#[path = "cli_windows_current_panel/plugin_context.rs"]
mod plugin_context;
