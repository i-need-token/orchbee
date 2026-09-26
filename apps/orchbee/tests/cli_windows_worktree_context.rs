//! Public Windows CLI witnesses for source precedence and separate Git/UI receipts.
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

fn id(n: u64) -> String {
    format!("{n:032x}")
}
fn context(context: u64, tab: u64, panel: u64) -> Value {
    json!({"terminalContextId":id(context),"groups":[],"tabs":[{"tabId":id(tab),"label":"shell","groupId":null,"pinned":false,"root":{"kind":"panel","panelId":id(panel),"primaryPresentation":{"presentationId":id(panel+1),"label":"shell","binding":{"terminalId":id(12),"workerId":id(9)}},"childPresentations":[]}}]})
}
fn snapshot(stream: &mut LocalStream, created: bool, primary: bool, standalone: bool) {
    let WorkbenchRequest::SnapshotRead(request) = read_workbench_request(stream).unwrap() else {
        panic!("snapshot")
    };
    let mut contexts = vec![if standalone {
        context(10, 6, 7)
    } else {
        json!({"terminalContextId":id(10),"groups":[],"tabs":[]})
    }];
    contexts.push(if standalone {
        json!({"terminalContextId":id(5),"groups":[],"tabs":[]})
    } else {
        context(5, 6, 7)
    });
    let mut directories = vec![json!({"directoryId":id(11),"workerId":id(9),"path":"C:/repo"})];
    let mut workspaces = vec![
        json!({"workspaceId":id(4),"label":"source","primaryDirectoryId":if primary {Some(id(11))} else {None},"terminalContextId":id(5)}),
    ];
    if created {
        directories.push(json!({"directoryId":id(25),"workerId":id(9),"path":"C:/trees/new"}));
        workspaces.push(json!({"workspaceId":id(20),"label":"new","primaryDirectoryId":id(25),"terminalContextId":id(21)}));
        // The newly opened context need not have a running terminal in a reused Workspace.
        contexts.push(json!({"terminalContextId":id(21),"groups":[],"tabs":[]}));
    }
    contexts.sort_by(|left, right| {
        left["terminalContextId"]
            .as_str()
            .cmp(&right["terminalContextId"].as_str())
    });
    let snapshot=serde_json::from_value(json!({"protocolVersion":1,"siteId":id(2),"revision":if created {"13"} else {"12"},"navigation":{"standaloneTerminalContextId":id(10),"projects":[{"projectId":id(3),"label":"project","directories":directories,"workspaces":workspaces}]},"terminalContexts":contexts})).unwrap();
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
fn record(fresh: bool, standalone: bool) -> Value {
    json!({"clientId":id(30),"heartbeat":"1","viewRevision":if fresh {"3"} else {"2"},"availability":"current","view":{"workbenchRevision":if fresh {"13"} else {"12"},"phase":"ready","selection":{"terminalContextId":id(if standalone {10} else {5}),"tabId":id(6),"panelId":id(7),"zoomed":false,"panelCount":1}}})
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
fn clients(stream: &mut LocalStream, rows: Vec<Value>) {
    let request = view(stream);
    assert!(matches!(request.action, ClientViewAction::List {}));
    reply_view(stream, request, json!({"kind":"listed","clients":rows}));
}
fn opened(stream: &mut LocalStream, expected_source: Value) {
    opened_after_wait(stream, expected_source, false);
}
fn opened_after_wait(stream: &mut LocalStream, expected_source: Value, pending: bool) {
    let WorkbenchRequest::WorktreeOperation(request) = read_workbench_request(stream).unwrap()
    else {
        panic!("single Worktree Start")
    };
    let WorkbenchWorktreeAction::Start {
        operation_id,
        project_id,
        intent,
    } = request.action
    else {
        panic!("Start")
    };
    assert_eq!(project_id.as_str(), id(3));
    assert_eq!(json!(intent)["source"], expected_source);
    let mut exchange_id = request.request_id;
    if pending {
        write_workbench_response(
            stream,
            &WorkbenchResponse::WorktreeOperation(WorkbenchWorktreeResult {
                protocol_version: ProtocolVersion,
                request_id: exchange_id,
                operation_id: operation_id.clone(),
                outcome: WorkbenchWorktreeOutcome::Available {
                    project_id: project_id.clone(),
                    attempt: 1,
                    status: Box::new(WorktreeOperationStatus::Pending {
                        phase: WorktreeOperationPhase::ObservingSource,
                        progress: WorktreeProgress {
                            path: None,
                            checkout: None,
                            git_effect: WorktreeGitEffect::None,
                            workspace: WorktreeWorkspaceDisposition::Unassociated {},
                        },
                    }),
                },
            }),
        )
        .unwrap();
        let WorkbenchRequest::WorktreeOperation(read) = read_workbench_request(stream).unwrap()
        else {
            panic!("retained read")
        };
        assert_eq!(
            read.action,
            WorkbenchWorktreeAction::Read {
                operation_id: operation_id.clone()
            }
        );
        exchange_id = read.request_id;
    }
    let outcome=serde_json::from_value(json!({"kind":"available","projectId":id(3),"attempt":1,"status":{"kind":"opened","checkout":{"repositoryRoot":"C:/repo","commonDirectory":"C:/repo/.git","path":"C:/trees/new","branch":null,"dirty":false},"workspaceId":id(20),"reused":true,"terminalOutcome":null}})).unwrap();
    write_workbench_response(
        stream,
        &WorkbenchResponse::WorktreeOperation(WorkbenchWorktreeResult {
            protocol_version: ProtocolVersion,
            request_id: exchange_id,
            operation_id,
            outcome,
        }),
    )
    .unwrap();
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
    let document = serde_json::from_slice(&output.stdout).unwrap();
    (output.status.code().unwrap(), document)
}

#[test]
fn implicit_source_uses_selected_workspace_or_only_its_root_cwd() {
    for (primary, standalone, caller) in [
        (true, false, false),
        (false, false, false),
        (false, true, false),
        (false, false, true),
        (false, true, true),
    ] {
        let terminal = id(12);
        let worker = id(9);
        let (status, document) = run(
            &["worktree", "create", "--format=json"],
            if caller {
                (Some(&terminal), Some(&worker))
            } else {
                (None, None)
            },
            move |mut stream| {
                snapshot(&mut stream, false, primary, standalone);
                if !caller {
                    clients(&mut stream, vec![record(false, standalone)]);
                }
                if !primary {
                    root_cwd(&mut stream, Some("C:/live"));
                }
                opened(
                    &mut stream,
                    if primary {
                        json!({"kind":"workspace","workspaceId":id(4)})
                    } else {
                        json!({"kind":"directory","path":"C:/live"})
                    },
                );
                assert_closed(&mut stream);
            },
        );
        assert_eq!(status, 0, "{document}");
        assert_eq!(document["version"], 1);
        validate(
            &document,
            include_str!("../src/cli/schemas/orchbee.worktree.operation.json"),
        );
    }
}

#[test]
fn source_refuses_ambiguous_stale_unready_and_missing_root_before_git() {
    for case in 0..5 {
        let (status, document) = run(
            &["worktree", "create", "--format=json"],
            (None, None),
            move |mut stream| {
                snapshot(&mut stream, false, false, false);
                let mut row = record(false, false);
                if case == 1 {
                    row["availability"] = json!("stale");
                }
                if case == 2 {
                    row["view"]["phase"] = json!("unavailable");
                }
                let mut rows = vec![row];
                if case == 0 {
                    let mut second = record(false, false);
                    second["clientId"] = json!(id(31));
                    rows.push(second);
                }
                clients(&mut stream, rows);
                if case >= 3 {
                    root_cwd(&mut stream, if case == 3 { None } else { Some("relative") });
                }
                assert_closed(&mut stream);
            },
        );
        assert_ne!(status, 0, "{document}");
        assert!(document.get("result").is_none(), "{document}");
    }
}

#[test]
fn explicit_sources_ignore_invalid_caller_and_explicit_client_overrides_it() {
    for case in 0..3 {
        let mut args = vec!["worktree", "create", "--format=json"];
        let client = id(30);
        args.extend(match case {
            0 => vec!["--cwd=C:/explicit"],
            1 => vec!["--workspace=source"],
            _ => vec!["--client", &client],
        });
        let (status, document) = run(&args, (Some("invalid"), None), move |mut stream| {
            snapshot(&mut stream, false, true, false);
            if case == 2 {
                clients(&mut stream, vec![record(false, false)]);
            }
            opened(
                &mut stream,
                if case == 0 {
                    json!({"kind":"directory","path":"C:/explicit"})
                } else {
                    json!({"kind":"workspace","workspaceId":id(4)})
                },
            );
            assert_closed(&mut stream);
        });
        assert_eq!(status, 0, "{document}");
    }
}

#[test]
fn explicit_workspace_can_use_its_proven_caller_but_invalid_implicit_caller_never_falls_back() {
    let terminal = id(12);
    let worker = id(9);
    let (status, document) = run(
        &["worktree", "create", "--workspace=source", "--format=json"],
        (Some(&terminal), Some(&worker)),
        |mut stream| {
            snapshot(&mut stream, false, false, false);
            root_cwd(&mut stream, Some("C:/live"));
            opened(&mut stream, json!({"kind":"directory","path":"C:/live"}));
            assert_closed(&mut stream);
        },
    );
    assert_eq!(status, 0, "{document}");
    for caller in [
        (Some("invalid"), None),
        (
            Some(terminal.as_str()),
            Some("00000000000000000000000000000099"),
        ),
    ] {
        let (status, document) = run(
            &["worktree", "create", "--format=json"],
            caller,
            |mut stream| {
                snapshot(&mut stream, false, true, false);
                assert_closed(&mut stream);
            },
        );
        assert_ne!(status, 0, "{document}");
        assert!(document.get("result").is_none());
    }
}

#[test]
fn directory_fallback_requires_no_clients_and_source_list_uses_the_same_selector() {
    for list in [false, true] {
        let client = id(30);
        let args = if list {
            vec!["worktree", "list", "--client", &client, "--format=json"]
        } else {
            vec!["worktree", "create", "--format=json"]
        };
        let (status, document) = run(&args, (None, None), move |mut stream| {
            snapshot(&mut stream, false, true, false);
            clients(
                &mut stream,
                if list {
                    vec![record(false, false)]
                } else {
                    vec![]
                },
            );
            if list {
                let WorkbenchRequest::WorktreeQuery(request) =
                    read_workbench_request(&mut stream).unwrap()
                else {
                    panic!("source query")
                };
                assert_eq!(
                    json!(request.source),
                    json!({"kind":"workspace","workspaceId":id(4)})
                );
                write_workbench_response(
                    &mut stream,
                    &WorkbenchResponse::WorktreeQuery(WorkbenchWorktreeQueryResult {
                        protocol_version: ProtocolVersion,
                        request_id: request.request_id,
                        outcome: WorkbenchWorktreeQueryOutcome::Failed {
                            reason: WorktreeQueryFailure::Unsupported {},
                        },
                    }),
                )
                .unwrap();
            } else {
                opened(
                    &mut stream,
                    json!({"kind":"directory","path":std::env::current_dir().unwrap().to_str().unwrap()}),
                );
            }
            assert_closed(&mut stream);
        });
        assert_eq!(status, if list { 4 } else { 0 }, "{document}");
        assert_eq!(
            document["schema"],
            if list {
                "orchbee.worktree.query"
            } else {
                "orchbee.worktree.operation"
            }
        );
    }
}

#[test]
fn focus_keeps_canonical_success_across_changed_view_and_uncertain_handoff() {
    for case in 0..7 {
        let (status, document) = run(
            &[
                "worktree",
                "create",
                "--focus",
                "--format=json",
                if case == 6 {
                    "--timeout=1500"
                } else {
                    "--timeout=30000"
                },
            ],
            (None, None),
            move |mut stream| {
                snapshot(&mut stream, false, true, false);
                clients(&mut stream, vec![record(false, false)]);
                opened_after_wait(
                    &mut stream,
                    json!({"kind":"workspace","workspaceId":id(4)}),
                    case == 5,
                );
                if case == 4 {
                    let _ = read_workbench_request(&mut stream).unwrap();
                    return;
                }
                snapshot(&mut stream, true, true, false);
                let request = view(&mut stream);
                assert!(matches!(request.action, ClientViewAction::Get { .. }));
                if case == 6 {
                    assert_closed(&mut stream);
                    return;
                }
                let mut current = record(true, false);
                if case == 1 {
                    current["view"]["selection"]["terminalContextId"] = json!(id(21));
                    current["view"]["selection"]["tabId"] = Value::Null;
                    current["view"]["selection"]["panelId"] = Value::Null;
                    current["view"]["selection"]["panelCount"] = json!(0);
                } else {
                    current["view"]["selection"]["panelCount"] = json!(2);
                }
                reply_view(
                    &mut stream,
                    request,
                    json!({"kind":"found","client":current}),
                );
                if case == 1 {
                    assert_closed(&mut stream);
                    return;
                }
                let request = view(&mut stream);
                let ClientViewAction::Submit { command } = &request.action else {
                    panic!("one focus submission")
                };
                assert_eq!(
                    json!(command.intent),
                    json!({"kind":"context","terminalContextId":id(21)})
                );
                assert_eq!(json!(command.expected_view_revision), "3");
                assert_eq!(
                    command.minimum_workbench_revision,
                    WorkbenchRevision::new(13)
                );
                if case == 2 {
                    return;
                }
                let outcome = json!({"kind":"operation","operation":{"command":command,"status":if case==3 {json!({"kind":"queued"})} else {json!({"kind":"applied","view":{"workbenchRevision":"13","phase":"ready","selection":{"terminalContextId":id(21),"tabId":null,"panelId":null,"zoomed":false,"panelCount":0}}})}}});
                reply_view(&mut stream, request, outcome);
                if case == 3 {
                    let request = view(&mut stream);
                    assert!(matches!(request.action, ClientViewAction::Read { .. }));
                    return;
                }
                assert_closed(&mut stream);
            },
        );
        assert_eq!(status == 0, matches!(case, 0 | 5), "{case}: {document}");
        validate(
            &document,
            include_str!("../src/cli/schemas/orchbee.worktree.operation.json"),
        );
        assert_eq!(document["version"], 2);
        assert_eq!(
            document["result"]["observation"]["reply"]["outcome"]["status"]["workspaceId"],
            id(20)
        );
        let post = &document["result"]["postOpen"];
        assert_eq!(post["clientId"], id(30));
        assert_eq!(
            post["outcome"],
            if matches!(case, 0 | 5) {
                "completed"
            } else {
                "incomplete"
            }
        );
        if case == 1 {
            assert_eq!(post["failure"], "denied");
            assert!(post["focus"].is_null());
        }
        if case == 2 {
            assert_eq!(status, 8);
            assert_eq!(post["focus"]["observation"]["kind"], "unknown");
        }
        if case == 6 {
            assert_eq!(status, 7);
            assert_eq!(post["failure"], "timeout");
            assert!(post["focus"].is_null());
        }
        if case == 0 {
            let schema: Value = serde_json::from_str(include_str!(
                "../src/cli/schemas/orchbee.worktree.operation.json"
            ))
            .unwrap();
            let validator = jsonschema::validator_for(&schema).unwrap();
            let mut wrong = document.clone();
            wrong["version"] = json!(1);
            assert!(!validator.is_valid(&wrong));
            wrong = document;
            wrong["result"].as_object_mut().unwrap().remove("postOpen");
            assert!(!validator.is_valid(&wrong));
        }
    }
}
