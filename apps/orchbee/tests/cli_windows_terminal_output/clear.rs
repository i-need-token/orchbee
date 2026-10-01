use super::*;
use std::num::NonZeroU64;
const SCHEMA: &str = include_str!("../../src/cli/schemas/orchbee.terminal.clear.json");

#[test]
fn clear_cli_submits_once_for_primary_child_or_suffix_and_retains_known_or_unknown_effect() {
    for (mode, target) in [
        ("cleared", ID),
        ("cleared", "55555555555555555555555555555555"),
        ("cleared", "@terminal:5555"),
        ("cleanup-lost", ID),
        ("alternate", ID),
        ("unavailable", ID),
        ("unknown", ID),
        ("lost", ID),
        ("misrouted", ID),
    ] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let expected = if target == ID {
            ID
        } else {
            "55555555555555555555555555555555"
        };
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_snapshot_reply(&mut stream);
            let viewer = ViewerId::parse("88888888888888888888888888888888").unwrap();
            let req = request(&mut stream);
            assert!(
                matches!(req.action(),Action::AttachBinding {terminal_id,..} if terminal_id.as_str()==expected)
            );
            reply(
                &mut stream,
                &req,
                Outcome::Attached {
                    attachment_id: viewer.clone(),
                    host_epoch: obcs::HostEpoch::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap(),
                    host_terminal_id: obcs::HostTerminalId::parse(
                        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                    )
                    .unwrap(),
                },
            );
            let ClientMessage::TerminalControl(req) = read_client_message(&mut stream).unwrap()
            else {
                panic!("one clear, no poll or acquire")
            };
            assert_eq!(
                req.action(),
                &obcs::TerminalControlAction::Clear {
                    attachment_id: viewer.clone()
                }
            );
            if mode == "lost" {
                return;
            }
            let result = match mode {
                "cleared" | "cleanup-lost" => obcs::TerminalControlOutcome::Cleared {
                    generation: NonZeroU64::new(2).unwrap(),
                    revision: NonZeroU64::new(7).unwrap(),
                    cleanup_confirmed: mode == "cleared",
                },
                "alternate" => obcs::TerminalControlOutcome::Rejected {
                    reason: obcs::ControlRejectedReason::HostClearAlternateScreen,
                },
                "unavailable" => obcs::TerminalControlOutcome::Unavailable {
                    reason: obcs::ControlUnavailableReason::TerminalRuntimeUnavailable,
                },
                "misrouted" => obcs::TerminalControlOutcome::InputAccepted {},
                _ => obcs::TerminalControlOutcome::OutcomeUnknown {},
            };
            write_server_message(
                &mut stream,
                &ServerMessage::TerminalControlResult(obcs::TerminalControlResult::new(
                    req.request_id().clone(),
                    result,
                )),
            )
            .unwrap();
            if mode == "misrouted" {
                assert_closed(&mut stream);
            } else {
                detached(&mut stream, &viewer);
            }
        });
        let output = fixture.run(&["terminal", "clear", target, "--format", "json"]);
        handler.join().unwrap();
        let expected_status = match mode {
            "cleared" => 0,
            "alternate" => 3,
            "unavailable" => 4,
            _ => 8,
        };
        assert_eq!(
            output.status.code(),
            Some(expected_status),
            "{mode}: {output:?}"
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(&document, SCHEMA);
        assert_eq!(document["result"]["terminal"]["terminalId"], expected);
        assert_eq!(
            document["result"]["terminal"]["boundTo"]["presentationId"],
            if expected == ID {
                "11111111111111111111111111111111"
            } else {
                "44444444444444444444444444444444"
            }
        );
        assert_eq!(
            document["result"]["operationId"].as_str().unwrap().len(),
            32
        );
        assert_eq!(
            document["result"]["outcome"]["kind"],
            match mode {
                "cleared" | "cleanup-lost" => "cleared",
                "alternate" => "rejected",
                "unavailable" => "unavailable",
                _ => "outcome-unknown",
            }
        );
    }
}

#[test]
fn clear_cli_help_localization_and_invalid_options_are_offline() {
    let fixture = Fixture::new();
    let help = fixture.run(&["help", "terminal", "clear"]);
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("preserving the complete prompt"));
    let help = fixture.run_with_env(
        &["help", "terminal", "clear"],
        None,
        None,
        &[("LC_ALL", std::ffi::OsStr::new("zh_CN.UTF-8"))],
    );
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("保留完整提示行"));
    for args in [
        vec!["terminal", "clear", ID, "--takeover", "--format", "json"],
        vec![
            "terminal",
            "clear",
            ID,
            "--generation",
            "1",
            "--format",
            "json",
        ],
        vec!["terminal", "clear", "--format", "json"],
    ] {
        let output = fixture.run(&args);
        assert_eq!(output.status.code(), Some(2));
    }
}
