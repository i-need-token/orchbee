//! Notification submission receipts through an actual Windows named-pipe peer.
#![cfg(windows)]
#[allow(dead_code)]
#[path = "support/windows_agent_peer.rs"]
mod support;
use orchbee_local_ipc::LocalListener;
use orchbee_workbench_protocol::*;
use serde_json::Value;
use support::{Fixture, assert_closed, bound_snapshot_reply, peer, validate};

#[test]
fn notification_pipe_receipts_are_correlated_and_never_retried() {
    for case in [
        "shown",
        "busy",
        "disabled",
        "rate",
        "absent",
        "denied",
        "lost",
        "wrong-action",
        "en",
        "zh",
    ] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_snapshot_reply(&mut stream);
            let WorkbenchRequest::Notification(request) =
                read_workbench_request(&mut stream).unwrap()
            else {
                panic!("notification");
            };
            assert_eq!(
                request.action,
                NotificationAction::Show {
                    notification: Notification {
                        title: "build ready".into(),
                        body: Some("all tests passed".into()),
                        position: Some(NotificationPosition::BottomLeft),
                        sound: NotificationSound::Done,
                    }
                }
            );
            if case == "lost" {
                return;
            }
            let reason = match case {
                "busy" => NotificationReason::Busy,
                "disabled" => NotificationReason::Disabled,
                "rate" => NotificationReason::RateLimited,
                "absent" => NotificationReason::NoForegroundClient,
                _ => NotificationReason::Shown,
            };
            let outcome = match case {
                "denied" => NotificationOutcome::Failed {
                    reason: NotificationFailure::Denied,
                },
                "wrong-action" => NotificationOutcome::Listening { notification: None },
                _ => NotificationOutcome::Show {
                    shown: reason == NotificationReason::Shown,
                    reason,
                },
            };
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::Notification(NotificationResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    session_id: request.session_id,
                    outcome,
                }),
            )
            .unwrap();
            assert_closed(&mut stream);
        });
        let text = matches!(case, "en" | "zh");
        let output = fixture.run_with_env(
            &[
                "notification",
                "show",
                " build\nready\u{7} ",
                "--body",
                "all\ttests  passed",
                "--position",
                "bottom-left",
                "--sound",
                "done",
                "--format",
                if text { "text" } else { "json" },
            ],
            None,
            None,
            &[(
                "LC_ALL",
                std::ffi::OsStr::new(if case == "zh" {
                    "zh_CN.UTF-8"
                } else {
                    "en_US.UTF-8"
                }),
            )],
        );
        handler.join().unwrap();
        assert_eq!(
            output.status.code(),
            Some(match case {
                "lost" | "wrong-action" => 8,
                "denied" => 3,
                _ => 0,
            }),
            "{case}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        if text {
            assert!(
                String::from_utf8(output.stdout)
                    .unwrap()
                    .contains(if case == "zh" {
                        "通知已排入"
                    } else {
                        "Notification queued"
                    })
            );
        } else {
            let document: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(document["version"], 1);
            validate(
                &document,
                include_str!("../src/cli/schemas/orchbee.notification.show.json"),
            );
            if matches!(case, "lost" | "wrong-action") {
                assert!(document["result"]["reply"].is_null());
            }
        }
    }
}

#[test]
fn notification_rejects_bad_arguments_before_admission_and_never_starts_session() {
    let fixture = Fixture::new();
    let _rendezvous = fixture.rendezvous();
    assert_eq!(
        fixture
            .run(&["notification", "show", "hello", "--format", "json"])
            .status
            .code(),
        Some(4)
    );
    for arguments in [
        vec!["notification", "show", " \t "],
        vec!["notification", "show", "hello", "--position", "center"],
        vec!["notification", "show", "hello", "--sound", "beep"],
        vec!["notification", "show", "hello", "--workspace", "x"],
        vec![
            "notification",
            "show",
            "hello",
            "--body",
            "a",
            "--body",
            "b",
        ],
        vec!["notification", "show"],
        vec!["notification", "show", "hello", "extra"],
        vec!["server", "config", "--body", "a"],
    ] {
        assert_eq!(
            fixture.run(&arguments).status.code(),
            Some(2),
            "{arguments:?}"
        );
    }
}
