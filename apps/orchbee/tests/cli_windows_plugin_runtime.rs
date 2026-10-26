//! Real native CLI processes against admitted, correlated plugin peers.
#![cfg(windows)]
use crate::peer_support as support;
use orchbee_local_ipc::LocalListener;
use orchbee_workbench_protocol::*;
use serde_json::Value;
use support::{Fixture, assert_closed, bound_snapshot_reply, peer, validate};
fn worker() -> PluginWorker {
    PluginWorker {
        worker_id: WorkerId::parse(&"3".repeat(32)).unwrap(),
        generation: WorkbenchRevision::new(1),
    }
}
fn log(job_id: String, sequence: u64) -> PluginCommandLog {
    PluginCommandLog {
        job_id,
        sequence,
        plugin_id: "tools".into(),
        command_id: "hello".into(),
        command: vec!["cmd.exe".into()],
        started_ms: 1,
        finished_ms: Some(2),
        exit_code: Some(0),
        failure: None,
        stdout: "hello\u{1b}[31m".into(),
        stderr: String::new(),
        stdout_truncated: false,
        stderr_truncated: false,
    }
}
const SCHEMA: &str = include_str!("../src/cli/schemas/orchbee.plugin.runtime.json");
#[test]
fn plugin_invoke_real_cli_reports_admission_failure_or_unknown_without_resubmission() {
    for mode in [
        "accepted",
        "disabled",
        "lost",
        "wrong-action",
        "text",
        "zh",
        "lost-text",
    ] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_snapshot_reply(&mut stream);
            let WorkbenchRequest::ClientView(list) = read_workbench_request(&mut stream).unwrap()
            else {
                panic!("context client discovery")
            };
            assert!(matches!(list.action, ClientViewAction::List {}));
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::ClientView(WorkbenchClientViewResult {
                    protocol_version: ProtocolVersion,
                    request_id: list.request_id,
                    session_id: list.session_id,
                    outcome: ClientViewOutcome::Listed { clients: vec![] },
                }),
            )
            .unwrap();
            bound_snapshot_reply(&mut stream);
            let WorkbenchRequest::Plugin(request) = read_workbench_request(&mut stream).unwrap()
            else {
                panic!()
            };
            let PluginAction::Invoke {
                operation_id,
                plugin_id,
                action_id,
                context,
                ..
            } = &request.action
            else {
                panic!()
            };
            assert_eq!(plugin_id.as_deref(), Some("tools"));
            assert_eq!(action_id, "hello");
            assert_eq!(context.selected_text.as_deref(), Some("你好"));
            assert_eq!(context.invocation_source.as_deref(), Some("cli"));
            assert_eq!(
                context.correlation_id.as_deref(),
                Some(operation_id.as_str())
            );
            if matches!(mode, "lost" | "lost-text") {
                return;
            }
            let outcome = match mode {
                "disabled" => PluginOutcome::Failed {
                    reason: PluginFailure::Disabled,
                },
                "wrong-action" => PluginOutcome::Logs {
                    worker: worker(),
                    logs: vec![],
                    next: None,
                },
                _ => PluginOutcome::Started {
                    worker: worker(),
                    log: Box::new(log(operation_id.clone(), 1)),
                    existing: false,
                },
            };
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::Plugin(PluginResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    session_id: request.session_id,
                    outcome,
                }),
            )
            .unwrap();
            assert_closed(&mut stream);
        });
        let text = matches!(mode, "text" | "zh" | "lost-text");
        let output = fixture.run_with_env(
            &[
                "plugin",
                "action",
                "invoke",
                "hello",
                "--plugin",
                "tools",
                "--context",
                r#"{"selectedText":"你好"}"#,
                "--format",
                if text { "text" } else { "json" },
            ],
            None,
            None,
            &[(
                "LC_ALL",
                std::ffi::OsStr::new(if mode == "zh" {
                    "zh_CN.UTF-8"
                } else {
                    "en_US.UTF-8"
                }),
            )],
        );
        assert_eq!(
            output.status.code(),
            Some(match mode {
                "disabled" => 3,
                "lost" | "wrong-action" | "lost-text" => 8,
                _ => 0,
            }),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        if text {
            let out = String::from_utf8(output.stdout).unwrap();
            assert!(!out.contains('\u{1b}'));
            assert!(out.contains(if mode == "zh" {
                "执行成功"
            } else if mode == "lost-text" {
                "outcome unknown"
            } else {
                "succeeded"
            }));
            assert!(out.contains(if mode == "zh" {
                "操作 ID:"
            } else {
                "Operation ID:"
            }));
        } else {
            let document: Value = serde_json::from_slice(&output.stdout).unwrap();
            validate(&document, SCHEMA);
            assert!(document["result"]["operationId"].as_str().is_some());
            assert_eq!(
                document["result"]["complete"],
                !matches!(mode, "lost" | "wrong-action")
            );
        }
        handler.join().unwrap();
    }
}
#[test]
fn plugin_log_cli_follows_exact_worker_cursor_and_requested_limit() {
    let fixture = Fixture::new();
    let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
    let handler = peer(&fixture, listener, move |mut stream| {
        bound_snapshot_reply(&mut stream);
        for page in 0..2 {
            let WorkbenchRequest::Plugin(request) = read_workbench_request(&mut stream).unwrap()
            else {
                panic!()
            };
            let PluginAction::Logs {
                plugin_id,
                cursor,
                limit,
            } = &request.action
            else {
                panic!()
            };
            assert_eq!(plugin_id.as_deref(), Some("tools"));
            assert_eq!(*limit, if page == 0 { 4 } else { 1 });
            assert_eq!(
                *cursor,
                if page == 0 {
                    None
                } else {
                    Some(PluginLogCursor {
                        worker: worker(),
                        before: 7,
                    })
                }
            );
            let sequences = if page == 0 {
                vec![10, 9, 8, 7]
            } else {
                vec![6]
            };
            let logs = sequences
                .into_iter()
                .map(|n| log(format!("{n:032x}"), n))
                .collect();
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::Plugin(PluginResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    session_id: request.session_id,
                    outcome: PluginOutcome::Logs {
                        worker: worker(),
                        logs,
                        next: Some(PluginLogCursor {
                            worker: worker(),
                            before: if page == 0 { 7 } else { 6 },
                        }),
                    },
                }),
            )
            .unwrap();
        }
        assert_closed(&mut stream);
    });
    let output = fixture.run(&[
        "plugin",
        "log",
        "list",
        "--plugin",
        "tools",
        "--limit",
        "5",
        "--format=json",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    validate(
        &document,
        include_str!("../src/cli/schemas/orchbee.plugin.logs.json"),
    );
    let logs = document["result"]["logs"].as_array().unwrap();
    assert_eq!(logs.len(), 5);
    assert_eq!(logs[0]["sequence"], 6);
    assert_eq!(logs[4]["sequence"], 10);
    handler.join().unwrap();
}

#[test]
fn plugin_history_defaults_to_latest_fifty_in_chronological_json_and_text_order() {
    for format in ["json", "text"] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_snapshot_reply(&mut stream);
            let mut remaining = 50;
            let mut sequence = 100;
            while remaining > 0 {
                let WorkbenchRequest::Plugin(request) =
                    read_workbench_request(&mut stream).unwrap()
                else {
                    panic!()
                };
                let PluginAction::Logs { limit, cursor, .. } = request.action else {
                    panic!()
                };
                assert_eq!(usize::from(limit), remaining.min(4));
                assert_eq!(
                    cursor.map(|c| c.before),
                    (sequence < 100).then_some(sequence + 1)
                );
                let logs = (0..limit)
                    .map(|_| {
                        let value = log(format!("{sequence:032x}"), sequence);
                        sequence -= 1;
                        value
                    })
                    .collect();
                remaining -= usize::from(limit);
                write_workbench_response(
                    &mut stream,
                    &WorkbenchResponse::Plugin(PluginResult {
                        protocol_version: ProtocolVersion,
                        request_id: request.request_id,
                        session_id: request.session_id,
                        outcome: PluginOutcome::Logs {
                            worker: worker(),
                            logs,
                            next: Some(PluginLogCursor {
                                worker: worker(),
                                before: sequence + 1,
                            }),
                        },
                    }),
                )
                .unwrap();
            }
            assert_closed(&mut stream);
        });
        let output = fixture.run(&["plugin", "log", "list", "--format", format]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        if format == "json" {
            let document: Value = serde_json::from_slice(&output.stdout).unwrap();
            validate(
                &document,
                include_str!("../src/cli/schemas/orchbee.plugin.logs.json"),
            );
            let logs = document["result"]["logs"].as_array().unwrap();
            assert_eq!(logs.len(), 50);
            for (index, row) in logs.iter().enumerate() {
                assert_eq!(row["sequence"], 51 + index);
            }
        } else {
            let text = String::from_utf8(output.stdout).unwrap();
            let positions: Vec<_> = (51..=100)
                .map(|sequence| text.find(&format!("{sequence:032x}")).unwrap())
                .collect();
            assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
        }
        handler.join().unwrap();
    }
}
