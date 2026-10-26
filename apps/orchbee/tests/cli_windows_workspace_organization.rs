//! Project organization changes submit one metadata action over globally stable Workspace scopes.
#![cfg(windows)]
#[allow(dead_code)]
#[path = "support/windows_agent_peer.rs"]
mod support;
use orchbee_local_ipc::{LocalListener, LocalStream};
use orchbee_workbench_protocol::*;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use support::{Fixture, assert_closed, peer, validate};

fn id(n: u8) -> String {
    format!("{n:032x}")
}
fn snapshot(stream: &mut LocalStream, grouped: bool) {
    let WorkbenchRequest::SnapshotRead(request) = read_workbench_request(stream).unwrap() else {
        panic!("one snapshot")
    };
    let snapshot = serde_json::from_value(json!({
        "protocolVersion":1,"sessionId":id(1),"revision":"7",
        "navigation":{"projects":[{"projectId":id(3),"label":"multi-repo"},{"projectId":id(9),"label":"empty"}],
            "directories":[{"directoryId":id(6),"workerId":id(7),"path":"C:/repo"}],
            "workspaces":[{"workspaceId":id(2),"terminalContextId":id(2),"projectId":null,"primaryDirectoryId":null,"label":"Workspace"},{"workspaceId":id(4),"projectId":if grouped {Some(id(3))} else {None},"label":"work","primaryDirectoryId":id(6),"terminalContextId":id(5)}],"workspaceOrder":[id(2),id(4)]},
        "terminalContexts":[{"terminalContextId":id(2),"groups":[],"tabs":[]},{"terminalContextId":id(5),"groups":[],"tabs":[]}]
    })).unwrap();
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
fn run(words: &[&str], script: impl FnOnce(LocalStream) + Send + 'static) -> (i32, Value) {
    let fixture = Fixture::new();
    let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
    let handler = peer(&fixture, listener, script);
    let output = fixture.run(words);
    assert!(
        handler.join().is_ok(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (
        output.status.code().unwrap(),
        serde_json::from_slice(&output.stdout).unwrap(),
    )
}
#[test]
fn organization_actions_preserve_exact_scope_and_never_submit_terminal_or_filesystem_work() {
    for (words, expected, outcome, unchanged) in [
        (
            vec![
                "workspace",
                "set-project",
                "@workspace:0x4",
                "--project=multi-repo",
                "--format=json",
            ],
            json!({"kind":"workspace.set-project","workspaceId":id(4),"projectId":id(3)}),
            json!({"kind":"workspace-project-changed","revision":"8","workspaceId":id(4),"projectId":id(3)}),
            false,
        ),
        (
            vec![
                "workspace",
                "set-project",
                "work",
                "--project=multi-repo",
                "--format=json",
            ],
            json!({"kind":"workspace.set-project","workspaceId":id(4),"projectId":id(3)}),
            json!({"kind":"unchanged","revision":"7"}),
            true,
        ),
        (
            vec!["workspace", "ungroup", "work", "--format=json"],
            json!({"kind":"workspace.set-project","workspaceId":id(4),"projectId":null}),
            json!({"kind":"workspace-project-changed","revision":"8","workspaceId":id(4),"projectId":null}),
            false,
        ),
        (
            vec![
                "workspace",
                "create-project",
                "work",
                "--label=two repos",
                "--format=json",
            ],
            json!({"kind":"workspace.create-project","workspaceId":id(4),"label":"two repos"}),
            json!({"kind":"workspace-project-changed","revision":"8","workspaceId":id(4),"projectId":id(8)}),
            false,
        ),
        (
            vec!["project", "dissolve", "multi-repo", "--format=json"],
            json!({"kind":"project.dissolve","projectId":id(3)}),
            json!({"kind":"project-dissolved","revision":"8","projectId":id(3)}),
            false,
        ),
    ] {
        let grouped = unchanged || words[1] == "ungroup" || words[0] == "project";
        let (status, doc) = run(&words, move |mut stream| {
            snapshot(&mut stream, grouped);
            let WorkbenchRequest::Command(command) = read_workbench_request(&mut stream).unwrap()
            else {
                panic!("one metadata command")
            };
            assert_eq!(command.session_id.as_str(), id(1));
            assert_eq!(command.expected_revision.value(), 7);
            assert_eq!(json!(command.action), expected);
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::Command(WorkbenchCommandResult {
                    protocol_version: ProtocolVersion,
                    request_id: command.request_id,
                    outcome: serde_json::from_value(outcome).unwrap(),
                }),
            )
            .unwrap();
            assert_closed(&mut stream);
        });
        assert_eq!(status, 0, "{words:?}: {doc}");
        validate(
            &doc,
            include_str!("../src/cli/schemas/orchbee.mutation.result.json"),
        );
        assert_eq!(doc["version"], 1);
        assert_eq!(
            doc["result"]["outcome"],
            if unchanged { "unchanged" } else { "committed" }
        );
        if words[0] == "workspace" {
            assert_eq!(doc["result"]["workspaceId"], id(4));
        }
    }
}
#[test]
fn unknown_organization_mutation_retains_the_original_request_and_is_not_replayed() {
    let captured = Arc::new(Mutex::new(None));
    let peer_request = Arc::clone(&captured);
    let (status, doc) = run(
        &["workspace", "ungroup", "work", "--format=json"],
        move |mut stream| {
            snapshot(&mut stream, true);
            let WorkbenchRequest::Command(command) = read_workbench_request(&mut stream).unwrap()
            else {
                panic!("metadata command")
            };
            assert!(matches!(
                command.action,
                WorkbenchAction::WorkspaceSetProject {
                    project_id: None,
                    ..
                }
            ));
            *peer_request.lock().unwrap() = Some(command.request_id.to_string());
        },
    );
    assert_eq!(status, 8, "{doc}");
    assert_eq!(doc["failure"]["kind"], "outcome-unknown");
    assert_eq!(
        doc["failure"]["details"]["requestId"],
        captured.lock().unwrap().as_ref().unwrap().as_str()
    );
}
#[test]
fn ordinary_close_submits_only_selected_workspace_without_project_scope() {
    let (status, doc) = run(
        &[
            "workspace",
            "close",
            "work",
            "--confirm",
            "--detach",
            "--format=json",
        ],
        |mut stream| {
            snapshot(&mut stream, true);
            let WorkbenchRequest::Command(command) = read_workbench_request(&mut stream).unwrap()
            else {
                panic!("ordinary close")
            };
            assert_eq!(
                json!(command.action),
                json!({"kind":"workspace.close","workspaceId":id(4)})
            );
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::Command(WorkbenchCommandResult {
                    protocol_version: ProtocolVersion,
                    request_id: command.request_id.clone(),
                    outcome: WorkbenchCommandOutcome::WorkspaceCloseAccepted {
                        close_request_id: command.request_id,
                        workspace_id: WorkbenchWorkspaceId::parse(&id(4)).unwrap(),
                        terminal_context_id: WorkbenchTerminalContextId::parse(&id(5)).unwrap(),
                        accepted_revision: WorkbenchRevision::new(7),
                    },
                }),
            )
            .unwrap();
            assert_closed(&mut stream);
        },
    );
    assert_eq!(status, 0, "{doc}");
    validate(
        &doc,
        include_str!("../src/cli/schemas/orchbee.mutation.result.json"),
    );
    assert!(doc["result"].get("projectId").is_none());
    assert_eq!(doc["result"]["workspaceId"], id(4));
}
#[test]
fn explicit_project_close_all_accepts_empty_membership_and_preserves_operation_identity() {
    let operation = id(10);
    for unknown in [false, true] {
        let (status, doc) = run(
            &[
                "project",
                "close-all",
                "empty",
                "--confirm",
                "--request-id",
                &operation,
                "--format=json",
            ],
            move |mut stream| {
                snapshot(&mut stream, true);
                let WorkbenchRequest::WorkspaceGroupClose(request) =
                    read_workbench_request(&mut stream).unwrap()
                else {
                    panic!("Project close-all")
                };
                assert_eq!(
                    json!(request.action),
                    json!({"kind":"start","operationId":id(10),"projectId":id(9)})
                );
                if unknown {
                    return;
                }
                write_workbench_response(
                    &mut stream,
                    &WorkbenchResponse::WorkspaceGroupClose(WorkbenchWorkspaceGroupCloseResult {
                        protocol_version: ProtocolVersion,
                        request_id: request.request_id,
                        operation_id: WorkbenchRequestId::parse(&id(10)).unwrap(),
                        outcome: WorkspaceGroupCloseOutcome::Available {
                            project_id: WorkbenchProjectId::parse(&id(9)).unwrap(),
                            attempt: 1,
                            status: WorkspaceGroupCloseStatus::Closed {},
                            members: vec![],
                        },
                    }),
                )
                .unwrap();
                assert_closed(&mut stream);
            },
        );
        assert_eq!(status, if unknown { 8 } else { 0 }, "{doc}");
        validate(
            &doc,
            include_str!("../src/cli/schemas/orchbee.project.close-all.json"),
        );
        assert_eq!(doc["result"]["operationId"], operation);
    }
}
