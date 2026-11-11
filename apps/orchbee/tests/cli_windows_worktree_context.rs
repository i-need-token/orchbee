//! Public Windows CLI witnesses for source precedence and separate Git/UI receipts.
#![cfg(windows)]
use crate::peer_support as support;
use orchbee_local_ipc::{LocalListener, LocalStream};
use orchbee_workbench_protocol::*;
use serde_json::{Value, json};
use support::{Fixture, assert_closed, configuration_reply, peer, validate};

fn id(n: u64) -> String {
    format!("{n:032x}")
}
fn context(context: u64, tab: u64, panel: u64) -> Value {
    json!({"terminalContextId":id(context),"groups":[],"tabs":[{"tabId":id(tab),"label":"shell","groupId":null,"pinned":false,"root":{"kind":"panel","panelId":id(panel),"primaryPresentation":{"customLabel":null,"presentationId":id(panel+1),"label":"shell","binding":{"terminalId":id(12),"workerId":id(9)}},"childPresentations":[]}}]})
}
fn snapshot(stream: &mut LocalStream, created: bool, primary: bool, initial_workspace: bool) {
    let WorkbenchRequest::SnapshotRead(request) = read_workbench_request(stream).unwrap() else {
        panic!("snapshot")
    };
    let mut contexts = vec![if initial_workspace {
        context(10, 6, 7)
    } else {
        json!({"terminalContextId":id(10),"groups":[],"tabs":[]})
    }];
    contexts.push(if initial_workspace {
        json!({"terminalContextId":id(5),"groups":[],"tabs":[]})
    } else {
        context(5, 6, 7)
    });
    let mut directories = vec![json!({"directoryId":id(11),"workerId":id(9),"path":"C:/repo"})];
    let mut workspaces = vec![
        json!({"workspaceId":id(4),"projectId":if created {Some(id(3))} else {None},"label":"source","primaryDirectoryId":if primary {Some(id(11))} else {None},"terminalContextId":id(5)}),
    ];
    if created {
        directories.push(json!({"directoryId":id(25),"workerId":id(9),"path":"C:/trees/new"}));
        workspaces.push(json!({"workspaceId":id(20),"projectId":id(3),"label":"new","primaryDirectoryId":id(25),"terminalContextId":id(21)}));
        // The newly opened context need not have a running terminal in a reused Workspace.
        contexts.push(json!({"terminalContextId":id(21),"groups":[],"tabs":[]}));
    }
    workspaces.push(json!({"workspaceId":id(10),"terminalContextId":id(10),"projectId":null,"primaryDirectoryId":null,"label":"Workspace"}));
    workspaces.sort_by(|left, right| {
        left["workspaceId"]
            .as_str()
            .cmp(&right["workspaceId"].as_str())
    });
    contexts.sort_by(|left, right| {
        left["terminalContextId"]
            .as_str()
            .cmp(&right["terminalContextId"].as_str())
    });
    let snapshot=serde_json::from_value(json!({"protocolVersion":1,"sessionId":id(2),"revision":if created {"13"} else {"12"},"navigation":{"projects":if created {vec![json!({"projectId":id(3),"label":"project"})]} else {vec![]},"directories":directories,"workspaces":workspaces},"terminalContexts":contexts})).unwrap();
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
fn record(fresh: bool, initial_workspace: bool) -> Value {
    json!({"clientId":id(30),"heartbeat":"1","viewRevision":if fresh {"3"} else {"2"},"availability":"current","view":{"workbenchRevision":if fresh {"13"} else {"12"},"phase":"ready","selection":{"terminalContextId":id(if initial_workspace {10} else {5}),"tabId":id(6),"panelId":id(7),"zoomed":false,"panelCount":1}}})
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
            session_id: request.session_id,
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
    assert_eq!(project_id, None);
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
                            restoration: None,
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
    let outcome=serde_json::from_value(json!({"kind":"available","projectId":null,"attempt":1,"status":{"kind":"opened","checkout":{"repositoryRoot":"C:/repo","commonDirectory":"C:/repo/.git","path":"C:/trees/new","branch":null,"dirty":false},"workspaceId":id(20),"reused":true,"terminalOutcome":null}})).unwrap();
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
fn explicit_paths_use_the_final_cli_context_in_complete_typed_intents() {
    for verb in ["list", "create", "open"] {
        let fixture = Fixture::new();
        let executor = fixture.0.join("execution-context");
        let home = fixture.0.join("home-context");
        std::fs::create_dir(&executor).unwrap();
        std::fs::create_dir(&home).unwrap();
        let source = if verb == "create" { &home } else { &executor }.join("source");
        let target = if verb == "create" { &executor } else { &home }.join("target");
        let source_path = WorkbenchDirectoryPath::parse(source.to_str().unwrap()).unwrap();
        let target_path = WorkbenchDirectoryPath::parse(target.to_str().unwrap()).unwrap();
        let rendezvous = fixture.rendezvous();
        let handler = peer(
            &fixture,
            LocalListener::bind(rendezvous.client_socket_path()).unwrap(),
            move |mut stream| {
                snapshot(&mut stream, false, false, false);
                if verb == "list" {
                    let WorkbenchRequest::WorktreeQuery(request) =
                        read_workbench_request(&mut stream).unwrap()
                    else {
                        panic!("query")
                    };
                    assert_eq!(
                        request.source,
                        WorktreeSource::Directory { path: source_path }
                    );
                    assert!(request.trust_repository);
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
                    let WorkbenchRequest::WorktreeOperation(request) =
                        read_workbench_request(&mut stream).unwrap()
                    else {
                        panic!("operation")
                    };
                    let WorkbenchWorktreeAction::Start {
                        operation_id,
                        project_id,
                        intent,
                    } = request.action
                    else {
                        panic!("start")
                    };
                    assert_eq!(project_id, None);
                    let source = WorktreeSource::Directory { path: source_path };
                    let expected = if verb == "create" {
                        WorktreeIntent::Create {
                            source,
                            branch: Some(WorktreeReference::parse("feature").unwrap()),
                            base: Some(WorktreeReference::parse("HEAD~2").unwrap()),
                            path: Some(target_path),
                            label: None,
                            geometry: WorkbenchTerminalGeometry::new(80, 24, 0, 0).unwrap(),
                            trust_repository: true,
                            agent_start: None,
                        }
                    } else {
                        WorktreeIntent::Open {
                            source,
                            selector: WorktreeSelector::Path { path: target_path },
                            label: None,
                            geometry: WorkbenchTerminalGeometry::new(80, 24, 0, 0).unwrap(),
                            trust_repository: true,
                        }
                    };
                    assert_eq!(intent, expected);
                    write_workbench_response(
                        &mut stream,
                        &WorkbenchResponse::WorktreeOperation(WorkbenchWorktreeResult {
                            protocol_version: ProtocolVersion,
                            request_id: request.request_id,
                            operation_id,
                            outcome: WorkbenchWorktreeOutcome::Denied {},
                        }),
                    )
                    .unwrap();
                }
                assert_closed(&mut stream);
            },
        );
        let mut words = vec![
            "worktree",
            verb,
            "--cwd",
            if verb == "create" {
                r"~\source"
            } else {
                "source"
            },
            "--trust-repository",
            "--format=json",
        ];
        if verb != "list" {
            words.extend([
                "--path",
                if verb == "create" {
                    "target"
                } else {
                    r"~\target"
                },
            ]);
        }
        if verb == "create" {
            words.extend(["--branch=feature", "--base=HEAD~2"]);
        }
        let mut command = fixture.command(&words, None, None, &[("USERPROFILE", home.as_os_str())]);
        command.current_dir(&executor);
        let output = fixture.run_command(
            command,
            None,
            std::time::Duration::from_secs(45),
            8 * 1024 * 1024,
        );
        assert!(
            handler.join().is_ok(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.status.code(),
            Some(if verb == "list" { 4 } else { 3 }),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &document,
            if verb == "list" {
                include_str!("../src/cli/schemas/orchbee.worktree.query.json")
            } else {
                include_str!("../src/cli/schemas/orchbee.worktree.operation.json")
            },
        );
        assert!(!source.exists());
        assert!(!target.exists());
    }
}

#[test]
fn path_context_failure_precedes_configuration_and_runtime_in_both_formats() {
    let fixture = Fixture::new();
    let config = fixture.0.join("invalid-config.toml");
    std::fs::write(&config, "[").unwrap();
    for (verb, option) in [
        ("list", "--cwd"),
        ("create", "--cwd"),
        ("create", "--path"),
        ("open", "--path"),
        ("open", "--cwd"),
    ] {
        for format in ["json", "text"] {
            let mut words = vec!["worktree", verb, option, "~", "--format", format];
            if verb == "open" && option == "--cwd" {
                words.push("--branch=main");
            }
            let output = fixture.run_with_env(
                &words,
                None,
                None,
                &[
                    ("USERPROFILE", std::ffi::OsStr::new("private-invalid-home")),
                    ("ORCHBEE_CONFIG_FILE", config.as_os_str()),
                ],
            );
            assert_eq!(output.status.code(), Some(2));
            if format == "json" {
                let document: Value = serde_json::from_slice(&output.stdout).unwrap();
                validate(
                    &document,
                    include_str!("../src/cli/schemas/orchbee.cli.failure.json"),
                );
                assert_eq!(document["failure"]["kind"], "invalid-usage");
            } else {
                assert!(output.stdout.is_empty());
            }
            let diagnostic = String::from_utf8(output.stderr).unwrap();
            assert!(diagnostic.contains("USERPROFILE"), "{diagnostic}");
            assert!(!diagnostic.contains("private-invalid-home"));
            assert!(!diagnostic.contains("invalid-config.toml"));
            assert!(!fixture.0.join("appdata").exists());
            assert!(!fixture.0.join("runtime").exists());
        }
    }
    assert_eq!(std::fs::read(&config).unwrap(), b"[");
}

#[test]
fn implicit_source_keeps_the_selected_workspace_identity_with_or_without_a_directory() {
    for (primary, initial_workspace, caller) in [
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
                snapshot(&mut stream, false, primary, initial_workspace);
                configuration_reply(&mut stream, "follow", None);
                if !caller {
                    clients(&mut stream, vec![record(false, initial_workspace)]);
                }
                opened(
                    &mut stream,
                    if !initial_workspace {
                        json!({"kind":"workspace","workspaceId":id(4)})
                    } else {
                        json!({"kind":"workspace","workspaceId":id(10)})
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
fn source_refuses_ambiguous_stale_unready_and_missing_membership_before_git() {
    for case in 0..5 {
        let (status, document) = run(
            &["worktree", "create", "--format=json"],
            (None, None),
            move |mut stream| {
                snapshot(&mut stream, false, false, true);
                configuration_reply(&mut stream, "follow", None);
                let mut row = record(false, true);
                if case == 1 {
                    row["availability"] = json!("stale");
                }
                if case == 2 {
                    row["view"]["phase"] = json!("unavailable");
                }
                if case == 3 {
                    row["view"]["selection"]["terminalContextId"] = json!(id(99));
                }
                if case == 4 {
                    row["view"]["selection"]["panelId"] = json!(id(99));
                }
                let mut rows = vec![row];
                if case == 0 {
                    let mut second = record(false, false);
                    second["clientId"] = json!(id(31));
                    rows.push(second);
                }
                clients(&mut stream, rows);
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
            configuration_reply(&mut stream, "follow", None);
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
            configuration_reply(&mut stream, "follow", None);
            opened(&mut stream, json!({"kind":"workspace","workspaceId":id(4)}));
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
                configuration_reply(&mut stream, "follow", None);
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
            if !list {
                configuration_reply(&mut stream, "follow", None);
            }
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
                configuration_reply(&mut stream, "follow", None);
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
        assert_eq!(document["version"], 1);
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
            wrong["result"].as_object_mut().unwrap().remove("postOpen");
            assert!(!validator.is_valid(&wrong));
            wrong["result"]["postOpen"] = Value::Null;
            assert!(validator.is_valid(&wrong));
        }
    }
}
