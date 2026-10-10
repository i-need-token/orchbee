//! Real CLI processes with controlled Windows IPC; no actual Worker/provider acceptance claim.
#![cfg(windows)]
#[allow(dead_code)]
#[path = "support/windows_agent_peer.rs"]
mod peer_support;
use orchbee_local_ipc::{LocalListener, LocalStream};
use orchbee_workbench_protocol::*;
use peer_support::{Fixture, assert_closed, peer, validate};
use serde_json::{Value, json};
fn id(n: u64) -> String {
    format!("{n:032x}")
}
fn snapshot(children: usize, bound: bool) -> Snapshot {
    serde_json::from_value(json!({"protocolVersion":1,"siteId":id(2),"revision":"12",
    "navigation":{"projects":[], "standaloneTerminalContextId":id(4), "directories":[], "workspaces":[]},"terminalContexts":[{"terminalContextId":id(4),"groups":[],
    "tabs":[{"tabId":id(5),"label":"Existing","pinned":false,"groupId":null,"root":{"kind":"panel","panelId":id(6),"rightClick":"panel",
      "primaryPresentation":{"presentationId":id(8),"label":"Empty","binding":if bound { json!({"workerId":id(80),"terminalId":id(81)}) } else { Value::Null }},
      "childPresentations":(0..children).map(|i|json!({"presentationId":id(9+i as u64),"label":"Child","binding":null})).collect::<Vec<_>>()}}]}]})).unwrap()
}
fn initial(stream: &mut LocalStream, snapshot: Snapshot) {
    let WorkbenchRequest::SnapshotRead(request) = read_workbench_request(stream).unwrap() else {
        panic!("snapshot first")
    };
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
fn command(stream: &mut LocalStream) -> WorkbenchCommand {
    let WorkbenchRequest::Command(command) = read_workbench_request(stream).unwrap() else {
        panic!("one member creation; no runtime read or focus")
    };
    command
}
fn reply(stream: &mut LocalStream, command: WorkbenchCommand, outcome: WorkbenchCommandOutcome) {
    write_workbench_response(
        stream,
        &WorkbenchResponse::Command(WorkbenchCommandResult {
            protocol_version: ProtocolVersion,
            request_id: command.request_id,
            outcome,
        }),
    )
    .unwrap();
    assert_closed(stream);
}
fn outcome(member: u64) -> WorkbenchCommandOutcome {
    WorkbenchCommandOutcome::PanelTerminalPending {
        revision: WorkbenchRevision::new(13),
        terminal_context_id: WorkbenchTerminalContextId::parse(&id(4)).unwrap(),
        tab_id: WorkbenchTabId::parse(&id(5)).unwrap(),
        panel_id: WorkbenchPanelId::parse(&id(6)).unwrap(),
        presentation_id: TerminalPresentationId::parse(&id(member)).unwrap(),
    }
}
fn document(output: std::process::Output, status: i32) -> Value {
    assert_eq!(
        output.status.code(),
        Some(status),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let doc = serde_json::from_slice(&output.stdout).unwrap();
    validate(
        &doc,
        include_str!("../src/cli/schemas/orchbee.panel.terminal-create.json"),
    );
    doc
}
#[test]
fn creates_primary_exact_child_and_new_child_without_focus_or_secret_echo() {
    for mode in 0..3 {
        let fixture = Fixture::new();
        let rendezvous = fixture.rendezvous();
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let expected_cwd = fixture.0.to_str().unwrap().to_owned();
        let cwd = expected_cwd.clone();
        let executable = fixture.executable().to_str().unwrap().to_owned();
        let expected_executable = executable.clone();
        let handler = peer(&fixture, listener, move |mut stream| {
            initial(&mut stream, snapshot(1, false));
            let request = command(&mut stream);
            assert_eq!(request.expected_revision.value(), 12);
            let WorkbenchAction::PanelCreateTerminal {
                target,
                launch,
                profile,
                options,
                ..
            } = &request.action
            else {
                panic!()
            };
            let member = match mode {
                0 => {
                    assert!(
                        matches!(target,WorkbenchPanelTerminalTarget::Unbound{presentation_id} if presentation_id.as_str()==id(8))
                    );
                    8
                }
                1 => {
                    assert!(
                        matches!(target,WorkbenchPanelTerminalTarget::Unbound{presentation_id} if presentation_id.as_str()==id(9))
                    );
                    9
                }
                _ => {
                    assert!(
                        matches!(target,WorkbenchPanelTerminalTarget::Child{label} if label.as_str()=="Helper")
                    );
                    30
                }
            };
            assert!(profile.is_none());
            let value = serde_json::to_value(launch).unwrap();
            assert_eq!(value["exec"], expected_executable);
            assert_eq!(value["args"], json!(["a & b", ""]));
            assert_eq!(options.cwd.as_ref().unwrap().as_str(), expected_cwd);
            assert_eq!(
                options.environment.entries()[0],
                ("MEMBER_SECRET".to_owned(), "do-not-print-this".to_owned())
            );
            reply(&mut stream, request, outcome(member));
        });
        let panel = id(6);
        let child = id(9);
        let mut args = vec![
            "panel",
            "create-terminal",
            &panel,
            "--cwd",
            &cwd,
            "--exec",
            &executable,
            "--arg",
            "a & b",
            "--arg",
            "",
            "--env",
            "MEMBER_SECRET=do-not-print-this",
            "--format=json",
        ];
        if mode == 1 {
            args.extend(["--presentation", &child]);
        } else if mode == 2 {
            args.extend(["--child", "--label", "Helper"]);
        }
        let output = fixture.run(&args);
        handler.join().unwrap();
        let doc = document(output, 0);
        assert_eq!(doc["result"]["kind"], "panel-terminal-pending");
        assert_eq!(doc["failure"], Value::Null);
        assert!(!doc.to_string().contains("do-not-print-this"));
        assert!(!doc.to_string().contains("a & b"));
    }
}
#[test]
fn unknown_and_physical_failure_preserve_request_and_committed_ids_without_retry() {
    for mode in 0..3 {
        let fixture = Fixture::new();
        let rendezvous = fixture.rendezvous();
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            initial(&mut stream, snapshot(1, false));
            let request = command(&mut stream);
            if mode == 0 {
                return;
            }
            let result = if mode == 2 {
                outcome(9)
            } else {
                WorkbenchCommandOutcome::PanelTerminalCreated {
                    revision: WorkbenchRevision::new(13),
                    terminal_context_id: WorkbenchTerminalContextId::parse(&id(4)).unwrap(),
                    tab_id: WorkbenchTabId::parse(&id(5)).unwrap(),
                    panel_id: WorkbenchPanelId::parse(&id(6)).unwrap(),
                    presentation_id: TerminalPresentationId::parse(&id(30)).unwrap(),
                    terminal_outcome: WorkbenchTerminalOutcome::Failed {
                        reason: WorkbenchTerminalFailureReason::TerminalCreateFailed,
                    },
                }
            };
            reply(&mut stream, request, result);
        });
        let output = fixture.run(&[
            "panel",
            "create-terminal",
            &id(6),
            "--child",
            "--cwd",
            "workspace",
            "--format=json",
        ]);
        handler.join().unwrap();
        let doc = document(output, if mode == 1 { 3 } else { 8 });
        assert!(
            doc["source"]["requestId"]
                .as_str()
                .is_some_and(|s| s.len() == 32)
        );
        if mode == 1 {
            assert_eq!(doc["result"]["presentationId"], id(30));
        } else {
            assert_eq!(doc["result"]["kind"], "outcome-unknown");
        }
    }
}
#[test]
fn rejects_bound_foreign_and_full_targets_before_any_launch() {
    for mode in 0..3 {
        let fixture = Fixture::new();
        let rendezvous = fixture.rendezvous();
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            initial(
                &mut stream,
                snapshot(if mode == 2 { 16 } else { 1 }, mode == 0),
            );
            assert_closed(&mut stream);
        });
        let panel = id(6);
        let foreign = id(88);
        let mut args = vec!["panel", "create-terminal", &panel, "--format=json"];
        if mode == 1 {
            args.extend(["--presentation", &foreign]);
        } else if mode == 2 {
            args.push("--child");
        }
        let output = fixture.run(&args);
        handler.join().unwrap();
        assert!(!output.status.success());
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &doc,
            include_str!("../src/cli/schemas/orchbee.cli.failure.json"),
        );
    }
}

#[test]
fn selected_client_routes_read_only_and_retains_profile_refusal() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let handler = peer(&fixture, listener, move |mut stream| {
        initial(&mut stream, snapshot(1, false));
        let WorkbenchRequest::ClientView(request) = read_workbench_request(&mut stream).unwrap()
        else {
            panic!("Client list only")
        };
        assert!(matches!(request.action, ClientViewAction::List {}));
        let client = json!({"clientId":id(7),"heartbeat":"1","viewRevision":"2","availability":"current",
            "view":{"workbenchRevision":"12","phase":"ready","selection":{"terminalContextId":id(4),"tabId":id(5),"panelId":id(6),"zoomed":false,"panelCount":1}}});
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::ClientView(WorkbenchClientViewResult {
                protocol_version: ProtocolVersion,
                request_id: request.request_id,
                site_id: request.site_id,
                outcome: serde_json::from_value(json!({"kind":"listed","clients":[client]}))
                    .unwrap(),
            }),
        )
        .unwrap();
        let request = command(&mut stream);
        assert!(
            matches!(&request.action,WorkbenchAction::PanelCreateTerminal {profile:Some(profile),target:WorkbenchPanelTerminalTarget::Unbound {presentation_id},..} if profile.as_str()=="shell" && presentation_id.as_str()==id(8))
        );
        reply(
            &mut stream,
            request,
            WorkbenchCommandOutcome::LaunchProfileAmbiguous {
                candidate_ids: vec![
                    LaunchProfileId::parse(&id(30)).unwrap(),
                    LaunchProfileId::parse(&id(31)).unwrap(),
                ],
            },
        );
    });
    let output = fixture.run(&[
        "panel",
        "create-terminal",
        "--client",
        &id(7),
        "--profile",
        "shell",
        "--format=json",
    ]);
    handler.join().unwrap();
    let doc = document(output, 3);
    assert_eq!(doc["result"]["kind"], "launch-profile-ambiguous");
    assert_eq!(doc["result"]["candidateIds"].as_array().unwrap().len(), 2);
}
