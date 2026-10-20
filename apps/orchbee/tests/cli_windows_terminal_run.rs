//! Windows terminal-run submission and uncertain-delivery evidence.
#![cfg(windows)]
#[path = "support/windows_agent_peer.rs"]
// The shared fixture also serves Agent-specific process suites.
#[allow(dead_code)]
mod peer_support;
use orchbee_client_server_protocol::*;
use orchbee_local_ipc::LocalListener;
use peer_support::*;
use serde_json::Value;

#[path = "cli_windows_terminal_run/wait.rs"]
mod wait;

#[test]
fn windows_run_sends_one_opaque_paste_then_enter_and_preserves_partial_receipts() {
    for (partial, format) in [false, true]
        .into_iter()
        .flat_map(|partial| ["json", "text"].map(|format| (partial, format)))
    {
        let fixture = Fixture::new();
        let rendezvous = fixture.rendezvous();
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let command = "  Write-Output '中文';\nWrite-Output \"quoted\"  ";
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_snapshot_reply(&mut stream);
            let ClientMessage::TerminalSend(request) = read_client_message(&mut stream).unwrap()
            else {
                panic!("single send, no readiness/viewer/controller operation");
            };
            assert_eq!(
                request.terminal_id().as_str(),
                "33333333333333333333333333333333"
            );
            let records = request.records().as_slice();
            assert_eq!(records.len(), 2);
            let SendRecord::Paste { content } = &records[0] else {
                panic!("paste first");
            };
            assert_eq!(content.as_bytes(), command.as_bytes());
            assert!(matches!(
                records[1],
                SendRecord::Key {
                    key: SendKey::Enter
                }
            ));
            let target = SendTarget::new(
                std::num::NonZeroU64::MIN,
                HostEpoch::parse("66666666666666666666666666666666").unwrap(),
                HostTerminalId::parse("77777777777777777777777777777777").unwrap(),
            );
            let outcome = if partial {
                TerminalSendOutcome::Unavailable {
                    reason: SendUnavailableReason::InputBudgetExhausted,
                    progress: SendProgress::stopped(1, command.len() as u64),
                    cleanup: SendCleanupStatus::Confirmed,
                }
            } else {
                TerminalSendOutcome::Accepted {
                    progress: SendProgress::complete(2, (command.len() + 1) as u64),
                    cleanup: SendCleanupStatus::Confirmed,
                }
            };
            write_server_message(
                &mut stream,
                &ServerMessage::TerminalSendResult(
                    TerminalSendResult::new(request.request_id().clone(), Some(target), outcome)
                        .unwrap(),
                ),
            )
            .unwrap();
            assert_closed(&mut stream);
        });
        let output = fixture.run(&[
            "terminal",
            "run",
            "33333333333333333333333333333333",
            command,
            &format!("--format={format}"),
        ]);
        handler.join().unwrap();
        assert_eq!(
            output.status.code(),
            Some(if partial { 9 } else { 0 }),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        if format == "text" {
            if partial {
                assert!(output.stdout.is_empty());
                assert!(!output.stderr.is_empty());
            } else {
                assert_eq!(
                    String::from_utf8(output.stdout).unwrap(),
                    "Sent to terminal 33333333333333333333333333333333.\n"
                );
                assert!(output.stderr.is_empty());
            }
            continue;
        }
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &doc,
            include_str!("../src/cli/schemas/orchbee.terminal.send.json"),
        );
        assert_eq!(doc["result"]["records"], 2);
        assert_eq!(
            doc["result"]["recordsAccepted"],
            if partial { 1 } else { 2 }
        );
    }
}

#[test]
fn windows_run_lost_reply_keeps_unknown_instead_of_resending_command() {
    for format in ["json", "text"] {
        let fixture = Fixture::new();
        let rendezvous = fixture.rendezvous();
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_snapshot_reply(&mut stream);
            assert!(matches!(
                read_client_message(&mut stream).unwrap(),
                ClientMessage::TerminalSend(_)
            ));
        });
        let output = fixture.run(&[
            "terminal",
            "run",
            "33333333333333333333333333333333",
            "echo one",
            &format!("--format={format}"),
        ]);
        handler.join().unwrap();
        assert_eq!(output.status.code(), Some(8));
        if format == "text" {
            assert!(output.stdout.is_empty());
            assert!(!output.stderr.is_empty());
            continue;
        }
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &doc,
            include_str!("../src/cli/schemas/orchbee.cli.failure.json"),
        );
        assert_eq!(doc["failure"]["kind"], "outcome-unknown");
        assert_eq!(doc["failure"]["details"]["stage"], "send");
    }
}
