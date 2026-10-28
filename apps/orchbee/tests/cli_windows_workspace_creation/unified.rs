//! Ordinary explicit operands must never select an empty-Workspace mutation.
use super::*;

#[test]
fn ordinary_and_no_focus_forms_submit_one_first_terminal_launch() {
    for explicit_cwd in [false, true] {
        for no_focus in [false, true] {
            let path = if explicit_cwd {
                "C:/repo".into()
            } else {
                std::env::var("USERPROFILE").unwrap()
            };
            let expected = path.clone();
            let mut args = vec![
                "workspace",
                "create",
                "--project=Default",
                "--label=manual",
                "--format=json",
            ];
            if explicit_cwd {
                args.push("--cwd=C:/repo");
            }
            if no_focus {
                args.push("--no-focus");
            }
            let (status, doc) = run(&args, (None, None), move |mut stream| {
                snapshot(&mut stream, state(1, false, false, 12));
                if !explicit_cwd {
                    configuration_reply(&mut stream, "follow", None);
                    clients(&mut stream, vec![]);
                }
                assert!(launch_named(&mut stream, &expected, "pending", "manual"));
                assert_closed(&mut stream);
            });
            assert_eq!(status, 0, "{args:?}: {doc}");
            assert_eq!(doc["schema"], "orchbee.workspace.creation");
            assert_eq!(doc["outcome"], "completed");
            assert!(doc["project"].is_null());
            assert_eq!(doc["directory"]["path"], path);
            assert_eq!(
                doc["directory"]["source"],
                if explicit_cwd { "argument" } else { "home" }
            );
            assert_eq!(doc["workspace"]["result"]["panelId"], id(23));
            assert_eq!(doc["workspace"]["result"]["workspaceId"], id(20));
            assert_eq!(doc["requestedFocus"], false);
            assert!(doc["clientId"].is_null() && doc["focus"].is_null());
        }
    }
}

#[test]
fn explicit_creation_retains_failed_terminal_and_unknown_submission_receipts() {
    for outcome in [
        "failed",
        "unavailable",
        "terminal-unknown",
        "unknown",
        "refused",
    ] {
        let (status, doc) = run(
            &[
                "workspace",
                "create",
                "--project=Default",
                "--label=manual",
                "--cwd=C:/repo",
                "--format=json",
            ],
            (Some("ignored-invalid-caller"), None),
            move |mut stream| {
                snapshot(&mut stream, state(1, false, false, 12));
                if launch_named(&mut stream, "C:/repo", outcome, "manual") {
                    assert_closed(&mut stream);
                }
            },
        );
        assert_ne!(status, 0, "{outcome}: {doc}");
        assert_eq!(doc["schema"], "orchbee.workspace.creation");
        assert_eq!(doc["outcome"], "incomplete");
        assert!(doc["project"].is_null() && doc["focus"].is_null());
        if matches!(outcome, "failed" | "unavailable" | "terminal-unknown") {
            assert_eq!(doc["workspace"]["result"]["workspaceId"], id(20));
            assert_eq!(doc["workspace"]["result"]["panelId"], id(23));
        }
        if outcome == "unknown" {
            assert_eq!(doc["failure"]["kind"], "outcome-unknown");
            assert!(doc["failure"]["details"]["requestId"].is_string());
        }
    }
}

#[test]
fn ordinary_follow_checks_caller_before_any_creation() {
    let (status, doc) = run(
        &[
            "workspace",
            "create",
            "--project=Default",
            "--label=manual",
            "--format=json",
        ],
        (Some("invalid-secret"), None),
        |mut stream| {
            snapshot(&mut stream, state(1, false, false, 12));
            configuration_reply(&mut stream, "follow", None);
            assert_closed(&mut stream);
        },
    );
    assert_eq!(status, 2);
    assert_eq!(doc["failure"]["kind"], "invalid-usage");
    assert!(!doc.to_string().contains("invalid-secret"));
}
