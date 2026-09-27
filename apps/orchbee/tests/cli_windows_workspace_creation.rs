//! Public Windows process witnesses for optional Project creation, source defaults and guarded focus.
#![cfg(windows)]
#[allow(dead_code)]
#[path = "support/windows_agent_peer.rs"]
mod support;
use orchbee_client_server_protocol as c;
use orchbee_local_ipc::{LocalListener, LocalStream};
use orchbee_terminal_protocol as t;
use orchbee_workbench_protocol::*;
use serde_json::{Value, json};
use support::{Fixture, assert_closed, peer, validate};
const SCHEMA: &str = include_str!("../src/cli/schemas/orchbee.workspace.creation.json");
fn id(n: u64) -> String {
    format!("{n:032x}")
}
fn context(context: u64, tab: u64, panel: u64, bound: bool) -> Value {
    json!({"terminalContextId":id(context),"groups":[],"tabs":[{"tabId":id(tab),"label":"shell","pinned":false,"groupId":null,"root":{"kind":"panel","panelId":id(panel),"primaryPresentation":{"presentationId":id(panel+1),"label":"shell","binding":if bound {json!({"terminalId":id(12),"workerId":id(9)})} else {Value::Null}},"childPresentations":[]}}]})
}
fn state(count: u64, source: bool, created: bool, revision: u64) -> Snapshot {
    let mut contexts = vec![json!({"terminalContextId":id(10),"groups":[],"tabs":[]})];
    let mut projects = vec![];
    for n in 0..count {
        let mut directories = vec![];
        let mut workspaces = vec![];
        if source && n == 0 {
            directories.push(json!({"directoryId":id(11),"workerId":id(9),"path":"C:/source"}));
            workspaces.push(json!({"workspaceId":id(4),"label":"source","primaryDirectoryId":id(11),"terminalContextId":id(5)}));
            contexts.push(context(5, 6, 7, true));
        }
        if created && n == 0 {
            directories.push(json!({"directoryId":id(25),"workerId":id(9),"path":"C:/repo"}));
            workspaces.push(json!({"workspaceId":id(20),"label":"repo","primaryDirectoryId":id(25),"terminalContextId":id(21)}));
            contexts.push(context(21, 22, 23, false));
        }
        projects.push(json!({"projectId":id(3+n*100),"label":if n==0 {"Default"} else {"other"},"directories":directories,"workspaces":workspaces}));
    }
    contexts.sort_by(|a, b| {
        a["terminalContextId"]
            .as_str()
            .cmp(&b["terminalContextId"].as_str())
    });
    serde_json::from_value(json!({"protocolVersion":1,"siteId":id(2),"revision":revision.to_string(),"navigation":{"standaloneTerminalContextId":id(10),"projects":projects},"terminalContexts":contexts})).unwrap()
}
fn snapshot(stream: &mut LocalStream, state: Snapshot) {
    let WorkbenchRequest::SnapshotRead(request) = read_workbench_request(stream).unwrap() else {
        panic!("snapshot")
    };
    write_workbench_response(
        stream,
        &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            outcome: WorkbenchSnapshotOutcome::Available { snapshot: state },
        }),
    )
    .unwrap();
}
fn command(stream: &mut LocalStream) -> WorkbenchCommand {
    let WorkbenchRequest::Command(command) = read_workbench_request(stream).unwrap() else {
        panic!("canonical command")
    };
    command
}
fn reply_command(stream: &mut LocalStream, command: WorkbenchCommand, outcome: Value) {
    write_workbench_response(
        stream,
        &WorkbenchResponse::Command(WorkbenchCommandResult {
            protocol_version: ProtocolVersion,
            request_id: command.request_id,
            outcome: serde_json::from_value(outcome).unwrap(),
        }),
    )
    .unwrap();
}
fn project(stream: &mut LocalStream) {
    let request = command(stream);
    assert_eq!(
        json!(request.action),
        json!({"kind":"project.create","label":"Default"})
    );
    assert_eq!(request.expected_revision, WorkbenchRevision::new(12));
    reply_command(
        stream,
        request,
        json!({"kind":"project-created","revision":"13","projectId":id(3)}),
    );
}
fn launch(stream: &mut LocalStream, path: &str, outcome: &str) -> bool {
    let label = std::path::Path::new(path)
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();
    launch_named(stream, path, outcome, label)
}
fn launch_named(stream: &mut LocalStream, path: &str, outcome: &str, label: &str) -> bool {
    let request = command(stream);
    let action = json!(request.action);
    assert_eq!(action["kind"], "workspace.launch");
    assert_eq!(action["projectId"], id(3));
    assert_eq!(action["options"]["cwd"], path);
    assert_eq!(action["label"], label);
    if outcome == "unknown" {
        return false;
    }
    if outcome == "refused" {
        reply_command(
            stream,
            request,
            json!({"kind":"rejected","revision":"14","reason":"revision-mismatch"}),
        );
        return true;
    }
    let mut reply = json!({"kind":"workspace-launch-pending","revision":"14","projectId":id(3),"workspaceId":id(20),"directoryId":id(25),"terminalContextId":id(21),"tabId":id(22),"panelId":id(23),"presentationId":id(24)});
    reply["revision"] = json!((request.expected_revision.value() + 1).to_string());
    if outcome != "pending" {
        reply["kind"] = json!("workspace-launched");
        reply["terminalOutcome"] = if outcome == "failed" {
            json!({"kind":"failed","reason":"terminal-create-failed"})
        } else if outcome == "unavailable" {
            json!({"kind":"unavailable","reason":"terminal-runtime-unavailable"})
        } else {
            json!({"kind":"outcome-unknown"})
        };
    }
    reply_command(stream, request, reply);
    true
}
fn view(stream: &mut LocalStream) -> WorkbenchClientViewRequest {
    let WorkbenchRequest::ClientView(request) = read_workbench_request(stream).unwrap() else {
        panic!("Client view")
    };
    request
}
fn reply_view(stream: &mut LocalStream, request: WorkbenchClientViewRequest, outcome: Value) {
    write_workbench_response(
        stream,
        &WorkbenchResponse::ClientView(WorkbenchClientViewResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            site_id: request.site_id,
            outcome: serde_json::from_value(outcome).unwrap(),
        }),
    )
    .unwrap();
}
fn record(source: bool, created: bool) -> Value {
    json!({"clientId":id(30),"heartbeat":"1","viewRevision":if created {"3"} else {"2"},"availability":"current","view":{"workbenchRevision":if created {"14"} else {"12"},"phase":"ready","selection":{"terminalContextId":id(if created {21} else if source {5} else {10}),"tabId":if created {Some(id(22))} else if source {Some(id(6))} else {None},"panelId":if created {Some(id(23))} else if source {Some(id(7))} else {None},"zoomed":false,"panelCount":if created || source {1} else {0}}}})
}
fn clients(stream: &mut LocalStream, records: Vec<Value>) {
    let request = view(stream);
    assert!(matches!(request.action, ClientViewAction::List {}));
    reply_view(stream, request, json!({"kind":"listed","clients":records}));
}
fn run(
    words: &[&str],
    caller: (Option<&str>, Option<&str>),
    script: impl FnOnce(LocalStream) + Send + 'static,
) -> (i32, Value) {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    let handler = peer(
        &fixture,
        LocalListener::bind(rendezvous.client_socket_path()).unwrap(),
        script,
    );
    let output = fixture.run_with_caller(words, caller.0, caller.1);
    assert!(
        handler.join().is_ok(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    if document["schema"] == "orchbee.workspace.creation" {
        validate(&document, SCHEMA);
        let schema: Value = serde_json::from_str(SCHEMA).unwrap();
        let validator = jsonschema::validator_for(&schema).unwrap();
        if !document["failure"].is_null() {
            let mut invalid = document.clone();
            invalid["failure"]["details"] = json!({"unexpected":true});
            assert!(
                !validator.is_valid(&invalid),
                "failure details must remain closed"
            );
        }
        if document["outcome"] == "incomplete" {
            let mut invalid = document.clone();
            invalid["outcome"] = json!("completed");
            invalid["stage"] = json!("complete");
            assert!(
                !validator.is_valid(&invalid),
                "partial effects cannot claim completion"
            );
        }
    } else if document["schema"] == "orchbee.cli.failure" {
        validate(
            &document,
            include_str!("../src/cli/schemas/orchbee.cli.failure.json"),
        );
    }
    (output.status.code().unwrap(), document)
}

#[test]
fn headless_defaults_use_home_and_explicit_destination_can_differ_from_source() {
    let home = std::env::var("USERPROFILE").unwrap();
    let (status, doc) = run(
        &["workspace", "create", "--format=json"],
        (None, None),
        move |mut stream| {
            snapshot(&mut stream, state(0, false, false, 12));
            clients(&mut stream, vec![]);
            project(&mut stream);
            snapshot(&mut stream, state(1, false, false, 13));
            launch(&mut stream, &home, "pending");
            assert_closed(&mut stream);
        },
    );
    assert_eq!(status, 0, "{doc}");
    assert_eq!(doc["directory"]["source"], "home");
    let (status, doc) = run(
        &[
            "workspace",
            "create",
            "--workspace=source",
            "--cwd=workspace",
            "--project=other",
            "--format=json",
        ],
        (None, None),
        |mut stream| {
            snapshot(&mut stream, state(2, true, false, 12));
            let request = command(&mut stream);
            let action = json!(request.action);
            assert_eq!(action["projectId"], id(103));
            assert_eq!(action["options"]["cwd"], "C:/source");
            reply_command(
                &mut stream,
                request,
                json!({"kind":"workspace-launch-pending","revision":"13","projectId":id(103),"workspaceId":id(20),"directoryId":id(25),"terminalContextId":id(21),"tabId":id(22),"panelId":id(23),"presentationId":id(24)}),
            );
            assert_closed(&mut stream);
        },
    );
    assert_eq!(status, 0, "{doc}");
    assert_eq!(doc["workspace"]["result"]["projectId"], id(103));
    assert!(doc["project"].is_null());
}

#[test]
fn first_project_is_created_once_and_existing_project_is_reused() {
    for count in [0, 1] {
        let (status, doc) = run(
            &["workspace", "create", "--cwd=C:/repo", "--format=json"],
            (Some("invalid-unrelated-caller"), None),
            move |mut stream| {
                snapshot(&mut stream, state(count, false, false, 12));
                if count == 0 {
                    project(&mut stream);
                    snapshot(&mut stream, state(1, false, false, 13));
                }
                launch(&mut stream, "C:/repo", "pending");
                assert_closed(&mut stream);
            },
        );
        assert_eq!(status, 0, "{doc}");
        assert_eq!(doc["outcome"], "completed");
        assert_eq!(doc["workspace"]["result"]["workspaceId"], id(20));
        assert_eq!(doc["project"].is_null(), count == 1);
        assert!(doc["focus"].is_null());
    }
}
#[test]
fn project_race_unknown_and_later_workspace_failures_keep_separate_receipts() {
    for cut in [
        "project-unknown",
        "project-race",
        "project-snapshot-lost",
        "workspace-unknown",
        "workspace-refused",
        "failed",
        "unavailable",
        "terminal-unknown",
    ] {
        let (status, doc) = run(
            &["workspace", "create", "--cwd=C:/repo", "--format=json"],
            (None, None),
            move |mut stream| {
                snapshot(&mut stream, state(0, false, false, 12));
                if cut == "project-unknown" {
                    let _ = command(&mut stream);
                    return;
                }
                if cut == "project-race" {
                    let request = command(&mut stream);
                    reply_command(
                        &mut stream,
                        request,
                        json!({"kind":"rejected","revision":"13","reason":"revision-mismatch"}),
                    );
                    assert_closed(&mut stream);
                    return;
                }
                project(&mut stream);
                if cut == "project-snapshot-lost" {
                    let _ = read_workbench_request(&mut stream).unwrap();
                    return;
                }
                snapshot(&mut stream, state(1, false, false, 13));
                if launch(
                    &mut stream,
                    "C:/repo",
                    match cut {
                        "workspace-unknown" => "unknown",
                        "workspace-refused" => "refused",
                        other => other,
                    },
                ) {
                    assert_closed(&mut stream);
                }
            },
        );
        assert_ne!(status, 0, "{cut}: {doc}");
        assert_eq!(doc["outcome"], "incomplete");
        assert_eq!(
            doc["project"].is_null(),
            matches!(cut, "project-unknown" | "project-race")
        );
        if matches!(cut, "project-unknown" | "workspace-unknown") {
            assert_eq!(doc["failure"]["kind"], "outcome-unknown");
            assert!(doc["failure"]["details"]["requestId"].is_string());
        }
        if matches!(cut, "failed" | "unavailable" | "terminal-unknown") {
            assert_eq!(doc["workspace"]["result"]["workspaceId"], id(20));
        }
    }
}
#[test]
fn focus_accepts_exact_new_workspace_reconciliation_and_refuses_other_navigation() {
    for cut in [
        "original", "created", "changed", "zoomed", "unknown", "timeout",
    ] {
        let (status, doc) = run(
            &[
                "workspace",
                "create",
                "--cwd=C:/repo",
                "--focus",
                "--timeout=1600",
                "--format=json",
            ],
            (None, None),
            move |mut stream| {
                snapshot(&mut stream, state(1, false, false, 12));
                clients(&mut stream, vec![record(false, false)]);
                launch(&mut stream, "C:/repo", "pending");
                snapshot(&mut stream, state(1, false, true, 14));
                let request = view(&mut stream);
                assert!(matches!(request.action, ClientViewAction::Get { .. }));
                let mut current = record(false, cut != "original");
                current["view"]["workbenchRevision"] = json!("14");
                current["viewRevision"] = json!("3");
                if cut == "changed" {
                    current["view"]["selection"]["panelId"] = json!(id(99));
                }
                if cut == "zoomed" {
                    current["view"]["selection"]["zoomed"] = json!(true);
                }
                reply_view(
                    &mut stream,
                    request,
                    json!({"kind":"found","client":current}),
                );
                if matches!(cut, "changed" | "zoomed") {
                    assert_closed(&mut stream);
                    return;
                }
                let request = view(&mut stream);
                let ClientViewAction::Submit { command } = &request.action else {
                    panic!("one focus")
                };
                assert_eq!(
                    command.minimum_workbench_revision,
                    WorkbenchRevision::new(14)
                );
                assert_eq!(
                    json!(command.intent),
                    json!({"kind":"context","terminalContextId":id(21)})
                );
                if cut == "unknown" {
                    return;
                }
                let status = if cut == "timeout" {
                    json!({"kind":"queued"})
                } else {
                    json!({"kind":"applied","view":record(false,true)["view"]})
                };
                let outcome =
                    json!({"kind":"operation","operation":{"command":command,"status":status}});
                reply_view(&mut stream, request, outcome);
                if cut == "timeout" {
                    let _ = view(&mut stream);
                    std::thread::sleep(std::time::Duration::from_millis(1800));
                }
                assert_closed(&mut stream);
            },
        );
        assert_eq!(
            status == 0,
            matches!(cut, "original" | "created"),
            "{cut}: {doc}"
        );
        assert_eq!(doc["workspace"]["result"]["workspaceId"], id(20));
    }
}
#[test]
fn source_directory_policy_does_not_confuse_destination_project_or_other_panels() {
    for mode in [
        "workspace",
        "live",
        "absent",
        "relative",
        "explicit-ignore-source",
        "ambiguous",
        "invalid-caller",
        "stale-client",
    ] {
        let mut args = vec!["workspace", "create", "--format=json"];
        match mode {
            "workspace" => args.extend(["--workspace=source", "--cwd=workspace"]),
            "explicit-ignore-source" => args.extend(["--workspace=missing", "--cwd=C:/repo"]),
            "ambiguous" => args.push("--cwd=C:/repo"),
            _ => {}
        }
        let (status, doc) = run(
            &args,
            if mode == "invalid-caller" {
                (Some("broken"), None)
            } else {
                (None, None)
            },
            move |mut stream| {
                snapshot(
                    &mut stream,
                    state(if mode == "ambiguous" { 2 } else { 1 }, true, false, 12),
                );
                if matches!(mode, "ambiguous" | "invalid-caller") {
                    assert_closed(&mut stream);
                    return;
                }
                if !matches!(mode, "workspace" | "explicit-ignore-source") {
                    let mut selected = record(true, false);
                    if mode == "stale-client" {
                        selected["availability"] = json!("stale");
                    }
                    clients(&mut stream, vec![selected]);
                    if mode == "stale-client" {
                        assert_closed(&mut stream);
                        return;
                    }
                    root_cwd(
                        &mut stream,
                        match mode {
                            "live" => Some("C:/live"),
                            "relative" => Some("relative"),
                            _ => None,
                        },
                    );
                    if mode == "relative" {
                        assert_closed(&mut stream);
                        return;
                    }
                }
                launch(
                    &mut stream,
                    match mode {
                        "live" => "C:/live",
                        "explicit-ignore-source" => "C:/repo",
                        _ => "C:/source",
                    },
                    "pending",
                );
                assert_closed(&mut stream);
            },
        );
        assert_eq!(
            status == 0,
            matches!(
                mode,
                "workspace" | "live" | "absent" | "explicit-ignore-source"
            ),
            "{mode}: {doc}"
        );
        if mode == "live" {
            assert_eq!(doc["directory"]["source"], "terminal-process");
        }
        if mode == "absent" {
            assert_eq!(doc["directory"]["source"], "workspace");
        }
    }
}

#[test]
fn worktree_first_project_setup_has_v3_receipt_even_before_git_submission() {
    for cut in ["complete", "unknown", "snapshot-lost"] {
        let (status, doc) = run(
            &["worktree", "create", "--cwd=C:/repo", "--format=json"],
            (None, None),
            move |mut stream| {
                snapshot(&mut stream, state(0, false, false, 12));
                project(&mut stream);
                if cut == "snapshot-lost" {
                    let _ = read_workbench_request(&mut stream).unwrap();
                    return;
                }
                snapshot(&mut stream, state(1, false, false, 13));
                let WorkbenchRequest::WorktreeOperation(request) =
                    read_workbench_request(&mut stream).unwrap()
                else {
                    panic!("Git start")
                };
                let WorkbenchWorktreeAction::Start {
                    operation_id,
                    project_id,
                    ..
                } = request.action
                else {
                    panic!("start")
                };
                assert_eq!(project_id.as_str(), id(3));
                if cut == "unknown" {
                    return;
                }
                let outcome=serde_json::from_value(json!({"kind":"available","projectId":id(3),"attempt":1,"status":{"kind":"opened","checkout":{"repositoryRoot":"C:/repo","commonDirectory":"C:/repo/.git","path":"C:/trees/new","branch":null,"dirty":false},"workspaceId":id(20),"reused":true,"terminalOutcome":null}})).unwrap();
                write_workbench_response(
                    &mut stream,
                    &WorkbenchResponse::WorktreeOperation(WorkbenchWorktreeResult {
                        protocol_version: ProtocolVersion,
                        request_id: request.request_id,
                        operation_id,
                        outcome,
                    }),
                )
                .unwrap();
                assert_closed(&mut stream);
            },
        );
        validate(
            &doc,
            include_str!("../src/cli/schemas/orchbee.worktree.operation.json"),
        );
        assert_eq!(doc["version"], 3);
        assert_eq!(
            doc["result"]["setup"]["project"]["result"]["projectId"],
            id(3)
        );
        assert_eq!(status == 0, cut == "complete", "{cut}: {doc}");
    }
}

fn viewer(stream: &mut LocalStream) -> c::TerminalViewerRequest {
    let c::ClientMessage::TerminalViewer(request) = c::read_client_message(stream).unwrap() else {
        panic!("viewer")
    };
    request
}
fn viewer_reply(
    stream: &mut LocalStream,
    request: c::TerminalViewerRequest,
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
fn root_cwd(stream: &mut LocalStream, cwd: Option<&str>) {
    let request = viewer(stream);
    assert_eq!(
        json!(request.action()),
        json!({"kind":"attach-binding","siteId":id(2),"workerId":id(9),"terminalId":id(12)})
    );
    viewer_reply(
        stream,
        request,
        c::TerminalViewerOutcome::Attached {
            attachment_id: c::ViewerId::parse(&id(50)).unwrap(),
            host_epoch: c::HostEpoch::parse(&id(51)).unwrap(),
        },
    );
    let request = viewer(stream);
    assert_eq!(json!(request.action())["kind"], "read-process-info");
    let row = |pid, cwd| t::TerminalProcess {
        pid,
        parent_pid: None,
        creation_time: 10,
        image: None,
        command_line: None,
        current_directory: cwd,
    };
    let payload = t::encode_process_info_control(&t::ProcessInfoControl::ProcessInfo(
        t::TerminalProcessInfo {
            attachment_id: t::AttachmentId::parse(&id(50)).unwrap(),
            terminal_id: t::TerminalId::parse(&id(52)).unwrap(),
            root_pid: 40,
            processes: vec![
                row(39, Some("C:/wrong-child".to_owned())),
                row(40, cwd.map(str::to_owned)),
            ],
        },
    ))
    .unwrap();
    let record = t::encode_transport_envelope(&t::Envelope {
        protocol: t::CURRENT_PROTOCOL_VERSION,
        role: t::MessageRole::Response,
        kind: payload.kind,
        optional_message: false,
        payload_encoding: t::PayloadEncoding::ControlJson,
        correlation_id: Some(t::CorrelationId::parse(&id(53)).unwrap()),
        host_epoch: Some(t::HostEpoch::parse(&id(51)).unwrap()),
        payload: &payload.payload,
    })
    .unwrap();
    viewer_reply(stream, request, c::TerminalViewerOutcome::Record(record));
    let request = viewer(stream);
    assert_eq!(
        json!(request.action()),
        json!({"kind":"detach","attachmentId":id(50)})
    );
    viewer_reply(stream, request, c::TerminalViewerOutcome::Detached {});
}

#[path = "cli_windows_workspace_creation/unified.rs"]
mod unified;
