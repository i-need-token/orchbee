//! Title receipts over a real Windows named pipe, with no live console mutation.
#![cfg(windows)]
#[allow(dead_code)]
#[path = "support/windows_agent_peer.rs"]
mod support;
use orchbee_local_ipc::LocalListener;
use orchbee_workbench_protocol::*;
use serde_json::Value;
use support::{Fixture, assert_closed, bound_snapshot_reply, peer, validate};
#[test]
fn window_title_pipe_set_clear_absent_failure_and_lost_reply() {
    for case in [
        "set",
        "clear",
        "absent",
        "denied",
        "busy",
        "lost",
        "wrong-action",
        "wrong-title",
        "zh",
    ] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_snapshot_reply(&mut stream);
            let WorkbenchRequest::WindowTitle(request) =
                read_workbench_request(&mut stream).unwrap()
            else {
                panic!()
            };
            let title = (case != "clear").then(|| WindowTitle::normalize("ab  中").unwrap());
            assert_eq!(
                request.action,
                match &title {
                    Some(title) => WindowTitleAction::Set {
                        title: title.clone()
                    },
                    None => WindowTitleAction::Clear {},
                }
            );
            if case == "lost" {
                return;
            }
            let outcome = match case {
                "denied" => WindowTitleOutcome::Failed {
                    reason: WindowTitleFailure::Denied,
                },
                "busy" => WindowTitleOutcome::Failed {
                    reason: WindowTitleFailure::Busy,
                },
                "wrong-action" => WindowTitleOutcome::Listening { snapshot: None },
                _ => WindowTitleOutcome::Published {
                    snapshot: WindowTitleSnapshot {
                        revision: WorkbenchRevision::new(1),
                        title: if case == "wrong-title" {
                            Some(WindowTitle::normalize("other").unwrap())
                        } else {
                            title
                        },
                    },
                    changed: case != "absent",
                    reason: if case == "absent" {
                        WindowTitleReason::NoForegroundClient
                    } else if case == "clear" {
                        WindowTitleReason::Cleared
                    } else {
                        WindowTitleReason::Set
                    },
                },
            };
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::WindowTitle(WindowTitleResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    site_id: request.site_id,
                    outcome,
                }),
            )
            .unwrap();
            assert_closed(&mut stream);
        });
        let mut args = vec![
            "terminal",
            "title",
            if case == "clear" { "clear" } else { "set" },
        ];
        if case != "clear" {
            args.push(" a\tb  中\x1b ");
        }
        args.extend(["--format", if case == "zh" { "text" } else { "json" }]);
        let output = fixture.run_with_env(
            &args,
            None,
            None,
            &[("LC_ALL", std::ffi::OsStr::new("zh_CN.UTF-8"))],
        );
        handler.join().unwrap();
        assert_eq!(
            output.status.code(),
            Some(match case {
                "lost" | "wrong-action" | "wrong-title" => 8,
                "denied" | "busy" => 3,
                _ => 0,
            }),
            "{case}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        if case == "zh" {
            assert!(
                String::from_utf8(output.stdout)
                    .unwrap()
                    .contains("标题覆盖已发布")
            );
        } else {
            let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
            validate(
                &doc,
                include_str!("../src/cli/schemas/orchbee.terminal.title.json"),
            );
            if matches!(case, "lost" | "wrong-action" | "wrong-title") {
                assert!(doc["result"]["reply"].is_null());
            }
        }
    }
}
#[test]
fn window_title_bad_arguments_never_admit_or_start_a_site() {
    let fixture = Fixture::new();
    let _rendezvous = fixture.rendezvous();
    assert_eq!(
        fixture
            .run(&["terminal", "title", "clear", "--format", "json"])
            .status
            .code(),
        Some(4)
    );
    for args in [
        vec!["terminal", "title", "set"],
        vec!["terminal", "title", "set", " \t "],
        vec!["terminal", "title", "set", "x", "extra"],
        vec!["terminal", "title", "clear", "extra"],
        vec!["terminal", "title", "clear", "--workspace", "x"],
        vec!["terminal", "title", "set", "x", "--body", "x"],
    ] {
        assert_eq!(fixture.run(&args).status.code(), Some(2), "{args:?}");
    }
}
