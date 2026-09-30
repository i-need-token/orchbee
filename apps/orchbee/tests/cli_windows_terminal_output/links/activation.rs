//! Actual CLI pipes prove link preflight, exact context, one Job invocation and uncertainty.
use super::*;
use orchbee_agent_observation_protocol::{
    AgentRequest, AgentResult, read_agent_request, write_agent_result,
};
use orchbee_workbench_protocol::*;
use serde_json::json;
const SCHEMA: &str = include_str!("../../../src/cli/schemas/orchbee.terminal.link-activate.json");
const URL: &str = "custom://example";
fn plugin(fixture: &Fixture) -> std::path::PathBuf {
    let source = fixture.0.join("orchbee-plugin.toml");
    std::fs::write(&source,"id='tools'\nname='tools'\nversion='1'\n[[actions]]\nid='open'\ntitle='Open'\ncommand=['cmd.exe','/c','exit 0']\n[[link_handlers]]\nid='handler'\ntitle='Handler'\npattern='^(custom|https?)://'\naction='open'\n").unwrap();
    let out = fixture.run(&["plugin", "link", source.to_str().unwrap()]);
    assert!(out.status.success(), "{out:?}");
    source
}
fn render(stream: &mut LocalStream, sequence: u64, uri: &str) {
    let req = row_request(stream, 10, 1, sequence, obcs::ViewerRowDetail::Render);
    reply(
        stream,
        &req,
        Outcome::Record(history_record(&render_window(
            10,
            sequence,
            Some(uri),
            false,
            "x",
        ))),
    );
}
fn context(stream: &mut LocalStream, terminal: &str) {
    let AgentRequest::Read(req) = read_agent_request(stream).unwrap() else {
        panic!("agent read")
    };
    assert_eq!(json!(req)["kind"], "agent.session.list.request");
    write_agent_result(stream,&AgentResult::Read(serde_json::from_value(json!({"kind":"agent.session.list.result","requestId":req.request_id(),"result":{"outcome":{"kind":"listed","revision":3,"items":[],"nextCursor":null}}})).unwrap())).unwrap();
    let req = request(stream);
    assert!(
        matches!(req.action(),Action::AttachBinding{terminal_id,..} if terminal_id.as_str()==terminal)
    );
    reply(
        stream,
        &req,
        Outcome::Unavailable {
            reason: obcs::ViewerUnavailableReason::TerminalRuntimeUnavailable,
        },
    );
    // Existing fixture answers typed metadata reads before the final snapshot.
    bound_snapshot_reply(stream);
}
fn output(fixture: &Fixture, terminal: &str) -> std::process::Output {
    fixture.run_with_caller(
        &[
            "terminal",
            "link-activate",
            terminal,
            "--cursor",
            "0:0",
            "--format=json",
        ],
        Some("invalid-inherited"),
        Some("invalid-inherited"),
    )
}
fn document(out: &std::process::Output) -> Value {
    let doc = serde_json::from_slice(&out.stdout).unwrap();
    validate(&doc, SCHEMA);
    doc
}

#[test]
fn link_activation_no_link_or_handler_is_an_effect_free_success_and_releases_viewer() {
    for mode in ["no-link", "no-handler"] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_snapshot_reply(&mut stream);
            let viewer = observed_frame(&mut stream);
            if mode == "no-handler" {
                render(&mut stream, 1, URL);
            } else {
                let req = row_request(&mut stream, 10, 1, 1, obcs::ViewerRowDetail::Render);
                reply(
                    &mut stream,
                    &req,
                    Outcome::Record(history_record(&render_window(10, 1, None, false, ""))),
                );
                let frame = decode_terminal_text_frame(OBTF).unwrap();
                let req = row_request(&mut stream, 10, 2, 2, obcs::ViewerRowDetail::Text);
                reply(
                    &mut stream,
                    &req,
                    Outcome::Record(history_record(&obrw_window(
                        10,
                        10 + u64::from(frame.geometry.rows),
                        frame.geometry.columns,
                        2,
                        frame.route.revision,
                        frame.route.generation.get(),
                        &[(0, ""), (0, "")],
                    ))),
                );
            }
            detached(&mut stream, &viewer);
        });
        let out = output(&fixture, ID);
        handler.join().unwrap();
        assert!(out.status.success(), "{out:?}");
        let doc = document(&out);
        assert_eq!(doc["result"]["handled"], false);
        assert_eq!(doc["result"]["activation"]["state"], mode);
        assert!(!fixture.0.join("appdata").exists());
    }
}
#[test]
fn link_activation_invokes_once_with_exact_clicked_primary_or_child_and_preserves_unknown_identity()
{
    for mode in [
        "primary",
        "child",
        "plain",
        "refused",
        "unknown",
        "lost",
        "wrong-job",
    ] {
        let fixture = Fixture::new();
        plugin(&fixture);
        let terminal = if mode == "child" {
            "55555555555555555555555555555555"
        } else {
            ID
        };
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_snapshot_reply(&mut stream);
            let viewer = observed_frame_for_terminal(&mut stream, terminal);
            let clicked = if mode == "plain" { "http://a" } else { URL };
            if mode == "plain" {
                let req = row_request(&mut stream, 10, 1, 1, obcs::ViewerRowDetail::Render);
                reply(
                    &mut stream,
                    &req,
                    Outcome::Record(history_record(&render_window(10, 1, None, false, clicked))),
                );
                let frame = decode_terminal_text_frame(OBTF).unwrap();
                let req = row_request(&mut stream, 10, 2, 2, obcs::ViewerRowDetail::Text);
                reply(
                    &mut stream,
                    &req,
                    Outcome::Record(history_record(&obrw_window(
                        10,
                        10 + u64::from(frame.geometry.rows),
                        frame.geometry.columns,
                        2,
                        frame.route.revision,
                        frame.route.generation.get(),
                        &[(0, clicked), (0, "")],
                    ))),
                );
            } else {
                render(&mut stream, 1, clicked);
            }
            context(&mut stream, terminal);
            if mode == "plain" {
                let req = row_request(&mut stream, 10, 1, 3, obcs::ViewerRowDetail::Render);
                reply(
                    &mut stream,
                    &req,
                    Outcome::Record(history_record(&render_window(10, 3, None, false, clicked))),
                );
            } else {
                render(&mut stream, 2, clicked);
            }

            let WorkbenchRequest::Plugin(req) = read_workbench_request(&mut stream).unwrap() else {
                panic!("one invocation")
            };
            let PluginAction::Invoke {
                operation_id,
                plugin_id,
                action_id,
                context,
            } = &req.action
            else {
                panic!()
            };
            assert_eq!(plugin_id.as_deref(), Some("tools"));
            assert_eq!(action_id, "open");
            assert_eq!(
                context.focused_pane_id.as_deref(),
                Some("22222222222222222222222222222222")
            );
            assert_eq!(context.tab_label.as_deref(), Some("tab"));
            assert!(context.workspace_id.is_none());
            assert_eq!(context.invocation_source.as_deref(), Some("link_click"));
            assert_eq!(context.clicked_url.as_deref(), Some(clicked));
            assert_eq!(context.link_handler_id.as_deref(), Some("handler"));
            assert_eq!(
                context.correlation_id.as_deref(),
                Some(operation_id.as_str())
            );
            assert!(context.focused_pane_cwd.is_none() && context.selected_text.is_none());
            if mode == "lost" {
                return;
            }
            let worker = PluginWorker {
                worker_id: WorkerId::parse("6f7263686265652d6c6f63616c2d7631").unwrap(),
                generation: WorkbenchRevision::new(1),
            };
            let outcome = match mode {
                "refused" => PluginOutcome::Failed {
                    reason: PluginFailure::Disabled,
                },
                "unknown" => PluginOutcome::Unknown { worker },
                _ => PluginOutcome::Started {
                    worker,
                    existing: false,
                    log: Box::new(PluginCommandLog {
                        job_id: if mode == "wrong-job" {
                            "77777777777777777777777777777777".into()
                        } else {
                            operation_id.clone()
                        },
                        sequence: 1,
                        plugin_id: "tools".into(),
                        command_id: "open".into(),
                        command: vec!["cmd.exe".into()],
                        started_ms: 1,
                        finished_ms: None,
                        exit_code: None,
                        failure: None,
                        stdout: String::new(),
                        stderr: String::new(),
                        stdout_truncated: false,
                        stderr_truncated: false,
                    }),
                },
            };
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::Plugin(PluginResult {
                    protocol_version: ProtocolVersion,
                    request_id: req.request_id,
                    site_id: req.site_id,
                    outcome,
                }),
            )
            .unwrap();
            if mode == "wrong-job" {
                assert_closed(&mut stream);
            } else {
                detached(&mut stream, &viewer);
            }
        });
        let out = output(&fixture, terminal);
        handler.join().unwrap();
        let doc = document(&out);
        let (status, state) = match mode {
            "refused" => (3, "failed"),
            "unknown" | "lost" | "wrong-job" => (8, "unknown"),
            _ => (0, "started"),
        };
        assert_eq!(out.status.code(), Some(status), "{mode}: {out:?}");
        assert_eq!(doc["result"]["activation"]["state"], state);
        assert_eq!(doc["result"]["handled"], state == "started");
        assert_eq!(doc["result"]["terminal"]["terminalId"], terminal);
        assert_eq!(
            doc["result"]["activation"]["operationId"]
                .as_str()
                .unwrap()
                .len(),
            32
        );
    }
}
#[test]
fn link_activation_refuses_stale_content_changed_catalog_and_oversized_context_without_invoking() {
    for mode in ["stale", "catalog", "large-url"] {
        let fixture = Fixture::new();
        let source = plugin(&fixture);
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_snapshot_reply(&mut stream);
            let viewer = observed_frame(&mut stream);
            let url = if mode == "large-url" {
                format!("custom://{}", "x".repeat(4096))
            } else {
                URL.into()
            };
            render(&mut stream, 1, &url);
            if mode != "large-url" {
                context(&mut stream, ID);
                if mode == "stale" {
                    let req = row_request(&mut stream, 10, 1, 2, obcs::ViewerRowDetail::Render);
                    reply(
                        &mut stream,
                        &req,
                        Outcome::RowsRejected {
                            reason: obcs::ViewerRowsRejectedReason::StaleRevision,
                        },
                    );
                } else {
                    let text = std::fs::read_to_string(&source)
                        .unwrap()
                        .replace("pattern='^(custom|https?)://'", "pattern='^other://'");
                    std::fs::write(&source, text).unwrap();
                    render(&mut stream, 2, URL);
                }
            }
            detached(&mut stream, &viewer);
        });
        let out = output(&fixture, ID);
        handler.join().unwrap();
        assert!(!out.status.success(), "{mode}: {out:?}");
        let doc: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert!(doc["failure"].is_object());
        assert!(doc.get("result").is_none());
    }
}
