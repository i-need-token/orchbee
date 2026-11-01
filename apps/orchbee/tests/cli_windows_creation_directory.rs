//! Real Windows CLI: directory observation precedes the one creation effect.
#![cfg(windows)]
use crate::peer_support as support;
use orchbee_client_server_protocol as c;
use orchbee_local_ipc::{LocalListener, LocalStream};
use orchbee_terminal_protocol as t;
use orchbee_workbench_protocol::*;
use serde_json::{Value, json};
use support::{Fixture, assert_closed, configuration_reply, peer, validate};

fn id(n: u64) -> String {
    format!("{n:032x}")
}
fn snapshot(stream: &mut LocalStream) {
    let WorkbenchRequest::SnapshotRead(request) = read_workbench_request(stream).unwrap() else {
        panic!("snapshot");
    };
    let snapshot = serde_json::from_value(json!({"protocolVersion":1,"sessionId":id(2),"revision":"12","navigation":{"projects":[], "directories":[], "workspaces":[{"workspaceId":id(4),"terminalContextId":id(4),"projectId":null,"primaryDirectoryId":null,"label":"Workspace"}]},"terminalContexts":[{"terminalContextId":id(4),"groups":[],"tabs":[{"tabId":id(5),"label":"one","groupId":null,"pinned":false,"root":{"kind":"panel","panelId":id(6),"primaryPresentation":{"customLabel":null,"presentationId":id(7),"label":"shell","binding":{"terminalId":id(3),"workerId":id(9)}},"childPresentations":[]}}]}]})).unwrap();
    write_workbench_response(
        stream,
        &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            outcome: WorkbenchSnapshotOutcome::Available { snapshot },
        }),
    )
    .unwrap();
}
fn selected_client(stream: &mut LocalStream, same_context: bool) {
    let WorkbenchRequest::ClientView(request) = read_workbench_request(stream).unwrap() else {
        panic!("client list");
    };
    assert!(matches!(request.action, ClientViewAction::List {}));
    let outcome = serde_json::from_value(json!({"kind":"listed","clients":[{"clientId":id(10),"heartbeat":"1","viewRevision":"2","availability":"current","view":{"workbenchRevision":"12","phase":"ready","selection":{"terminalContextId":id(if same_context {4} else {14}),"tabId":id(if same_context {5} else {15}),"panelId":id(if same_context {6} else {16}),"zoomed":false,"panelCount":1}}}]})).unwrap();
    write_workbench_response(
        stream,
        &WorkbenchResponse::ClientView(WorkbenchClientViewResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            session_id: request.session_id,
            outcome,
        }),
    )
    .unwrap();
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
#[derive(Clone, Copy, Debug, PartialEq)]
enum Cut {
    Good,
    Absent,
    Relative,
    AttachUnavailable,
    ReadUnavailable,
    Denied,
    Lost,
    WrongAttachment,
    Timeout,
    CreateLost,
}
fn observe(stream: &mut LocalStream, cut: Cut) -> bool {
    let request = viewer(stream);
    assert_eq!(
        json!(request.action()),
        json!({"kind":"attach-binding","sessionId":id(2),"workerId":id(9),"terminalId":id(3)})
    );
    if cut == Cut::AttachUnavailable {
        answer(
            stream,
            &request,
            c::TerminalViewerOutcome::Unavailable {
                reason: c::ViewerUnavailableReason::TerminalRuntimeUnavailable,
            },
        );
        return true;
    }
    if cut == Cut::Denied {
        answer(
            stream,
            &request,
            c::TerminalViewerOutcome::Rejected {
                reason: c::ViewerRejectedReason::HostRejected,
            },
        );
        assert_closed(stream);
        return false;
    }
    answer(
        stream,
        &request,
        c::TerminalViewerOutcome::Attached {
            creation_id: orchbee_client_server_protocol::HostCreationId::parse(&"c".repeat(32))
                .unwrap(),
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
    if cut == Cut::Lost {
        return false;
    }
    if cut == Cut::Timeout {
        assert_closed(stream);
        return false;
    }
    let mut sampled = None;
    if cut == Cut::ReadUnavailable {
        answer(
            stream,
            &request,
            c::TerminalViewerOutcome::Unavailable {
                reason: c::ViewerUnavailableReason::TerminalRuntimeUnavailable,
            },
        );
    } else {
        let row = |pid, cwd| t::TerminalProcess {
            pid,
            parent_pid: None,
            creation_time: 10,
            console: orchbee_terminal_protocol::TerminalProcessConsole::Attached,
            image: None,
            command_line: None,
            current_directory: cwd,
        };
        let process = t::TerminalProcessInfo {
            attachment_id: t::AttachmentId::parse(&id(if cut == Cut::WrongAttachment {
                99
            } else {
                20
            }))
            .unwrap(),
            terminal_id: t::TerminalId::parse(&id(30)).unwrap(),
            root_pid: 40,
            processes: vec![
                row(39, Some(r"C:\child-agent".to_owned())),
                row(
                    40,
                    match cut {
                        Cut::Absent => None,
                        Cut::Relative => Some("relative".to_owned()),
                        _ => Some(r"C:\项目 source".to_owned()),
                    },
                ),
            ],
        };
        sampled = Some(process.processes.clone());
        let payload =
            t::encode_process_info_control(&t::ProcessInfoControl::ProcessInfo(process)).unwrap();
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
    if cut == Cut::WrongAttachment {
        assert_closed(stream);
        return false;
    }
    // Every shared CLI process read is followed by the Server identity read of that Terminal.
    if let Some(processes) = sampled {
        support::identify_reply(stream, &processes, &id(30));
    }
    true
}
fn create(stream: &mut LocalStream, split: bool, cwd: Option<&str>, lost: bool) {
    let WorkbenchRequest::Command(command) = read_workbench_request(stream).unwrap() else {
        panic!("one create");
    };
    assert_eq!(command.expected_revision, WorkbenchRevision::new(12));
    let action = json!(command.action);
    assert_eq!(
        action["kind"],
        if split { "panel.split" } else { "tab.create" }
    );
    assert_eq!(action["options"]["cwd"].as_str(), cwd);
    if lost {
        return;
    }
    let outcome = if split {
        json!({"kind":"panel-split-pending","revision":"13","terminalContextId":id(4),"tabId":id(5),"panelId":id(6),"newPanelId":id(40),"splitId":id(41),"firstSlotId":id(42),"secondSlotId":id(43),"presentationId":id(44)})
    } else {
        json!({"kind":"tab-create-pending","revision":"13","terminalContextId":id(4),"tabId":id(39),"panelId":id(40),"presentationId":id(44)})
    };
    write_workbench_response(
        stream,
        &WorkbenchResponse::Command(WorkbenchCommandResult {
            protocol_version: ProtocolVersion,
            request_id: command.request_id,
            outcome: serde_json::from_value(outcome).unwrap(),
        }),
    )
    .unwrap();
    assert_closed(stream);
}
fn check(output: std::process::Output, success: bool) -> Value {
    let document: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|_| panic!("{}", String::from_utf8_lossy(&output.stderr)));
    assert_eq!(output.status.success(), success, "{document}");
    if success {
        validate(
            &document,
            include_str!("../src/cli/schemas/orchbee.creation.result.json"),
        );
        assert_eq!(document["version"], 1);
    }
    document
}
#[test]
fn source_observation_is_exact_bounded_read_only_and_never_replays_creation() {
    for cut in [
        Cut::Good,
        Cut::Absent,
        Cut::Relative,
        Cut::AttachUnavailable,
        Cut::ReadUnavailable,
        Cut::Denied,
        Cut::Lost,
        Cut::WrongAttachment,
        Cut::Timeout,
        Cut::CreateLost,
    ] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            snapshot(&mut stream);
            configuration_reply(&mut stream, "follow", None);
            if observe(&mut stream, cut) {
                create(
                    &mut stream,
                    true,
                    if matches!(cut, Cut::Good | Cut::CreateLost) {
                        Some(r"C:\项目 source")
                    } else {
                        None
                    },
                    cut == Cut::CreateLost,
                );
            }
        });
        let output = fixture.run(&[
            "panel",
            "split",
            "@panel:0x6",
            "--direction=right",
            if cut == Cut::Timeout {
                "--timeout=1500"
            } else {
                "--timeout=30000"
            },
            "--format=json",
        ]);
        handler.join().unwrap();
        let success = matches!(
            cut,
            Cut::Good | Cut::Absent | Cut::Relative | Cut::AttachUnavailable | Cut::ReadUnavailable
        );
        let doc = check(output, success);
        if success {
            assert_eq!(
                doc["directory"]["source"],
                if cut == Cut::Good {
                    "terminal-process"
                } else {
                    "launch-default"
                }
            );
            assert_eq!(doc["directory"]["sourcePanelId"], id(6));
        } else {
            assert_eq!(
                doc["failure"]["kind"],
                match cut {
                    Cut::Denied => "rejected",
                    Cut::WrongAttachment => "protocol-invalid",
                    Cut::Timeout => "timeout",
                    Cut::CreateLost => "outcome-unknown",
                    _ => "transport-loss",
                }
            );
        }
    }
}
#[test]
fn explicit_directory_modes_skip_viewer_and_tab_source_respects_the_destination_context() {
    for mode in ["workspace", "home", "current", r"C:\explicit directory"] {
        let expected = match mode {
            "workspace" => None,
            "home" => Some(std::env::var("USERPROFILE").unwrap()),
            "current" => Some(
                std::env::current_dir()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_owned(),
            ),
            _ => Some(mode.to_owned()),
        };
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let expected_peer = expected.clone();
        let handler = peer(&fixture, listener, move |mut stream| {
            snapshot(&mut stream);
            create(&mut stream, true, expected_peer.as_deref(), false);
        });
        let output = fixture.run(&[
            "panel",
            "split",
            "@panel:0x6",
            "--direction=right",
            "--cwd",
            mode,
            "--format=json",
        ]);
        handler.join().unwrap();
        let doc = check(output, true);
        assert_eq!(doc["directory"]["path"].as_str(), expected.as_deref());
        assert!(doc["directory"]["sourcePanelId"].is_null());
    }
    // Headless explicit context, validated caller, matching Client, different Client context.
    for (explicit, client_context) in [
        (true, None),
        (false, None),
        (true, Some(true)),
        (true, Some(false)),
    ] {
        let follow = !explicit || client_context == Some(true);
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            snapshot(&mut stream);
            configuration_reply(&mut stream, "follow", None);
            if let Some(same) = client_context {
                selected_client(&mut stream, same);
            }
            if follow {
                assert!(observe(&mut stream, Cut::Good));
            }
            create(
                &mut stream,
                false,
                follow.then_some(r"C:\项目 source"),
                false,
            );
        });
        let mut args = vec!["tab", "create", "--format=json"];
        if explicit {
            args.extend(["--workspace", "Workspace"]);
        }
        let client = id(10);
        if client_context.is_some() {
            args.extend(["--client", &client]);
        }
        let output = fixture.run_with_caller(
            &args,
            (!explicit).then(|| id(3)).as_deref(),
            (!explicit).then(|| id(9)).as_deref(),
        );
        handler.join().unwrap();
        let doc = check(output, true);
        assert_eq!(
            doc["directory"]["source"],
            if follow {
                "terminal-process"
            } else {
                "launch-default"
            }
        );
    }
}

#[test]
fn directory_receipt_schema_rejects_invented_and_inconsistent_facts() {
    let doc: Value = serde_json::from_str(include_str!(
        "../src/cli/schemas/orchbee.creation.result.json"
    ))
    .unwrap();
    // The single version-one document owns the receipt.
    let directory = &doc["properties"]["directory"];
    assert!(directory.is_object());
    let validator = jsonschema::validator_for(directory).unwrap();
    let valid = json!({"policy":"follow","source":"terminal-process","path":r"C:\source","sourcePanelId":id(6)});
    assert!(validator.is_valid(&valid));
    for (key, value) in [
        ("policy", json!("unknown")),
        ("source", json!("child-agent")),
        ("sourcePanelId", Value::Null),
        ("path", Value::Null),
    ] {
        let mut wrong = valid.clone();
        wrong[key] = value;
        assert!(!validator.is_valid(&wrong), "{wrong}");
    }
    let mut default =
        json!({"policy":"workspace","source":"launch-default","path":null,"sourcePanelId":null});
    assert!(validator.is_valid(&default));
    default["path"] = json!(r"C:\invented");
    assert!(!validator.is_valid(&default));
}
