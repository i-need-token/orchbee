use super::*;
use orchbee_local_ipc::{AsLocalIo, LocalStream};

#[test]
fn idle_shell_refuses_commands_unknown_processes_and_incomplete_samples() {
    let process = |pid, image: &str, command: &str| TerminalProcess {
        pid,
        parent_pid: None,
        creation_time: 1,
        image: Some(image.into()),
        command_line: Some(command.into()),
        current_directory: None,
    };
    let mut sample = TerminalProcessInfo {
        attachment_id: orchbee_terminal_protocol::AttachmentId::from_bytes([1; 16]).unwrap(),
        terminal_id: orchbee_terminal_protocol::TerminalId::from_bytes([2; 16]).unwrap(),
        root_pid: 1,
        processes: vec![process(1, "C:\\Windows\\cmd.exe", "cmd.exe /d")],
    };
    assert!(idle_shell(&sample).is_ok());
    sample
        .processes
        .push(process(2, "conhost.exe", "conhost.exe"));
    assert!(idle_shell(&sample).is_ok());
    sample
        .processes
        .push(process(3, "node.exe", "node.exe server.js"));
    assert!(idle_shell(&sample).is_err());
    sample.processes.truncate(1);
    sample.processes[0].command_line = Some("cmd.exe /c task".into());
    assert!(idle_shell(&sample).is_err());
    sample.processes[0] = process(1, "powershell.exe", "powershell.exe -NoLogo -NoProfile");
    assert!(idle_shell(&sample).is_ok());
    sample.processes[0].command_line = Some("cmd.exe".into());
    assert!(idle_shell(&sample).is_err());
    sample.processes[0].command_line = None;
    assert!(idle_shell(&sample).is_err());
    sample.processes.clear();
    assert!(idle_shell(&sample).is_err());
}

#[test]
fn idle_shell_accepts_only_the_complete_runtime_powershell_initialization() {
    for shell in ["powershell.exe", "pwsh.exe"] {
        let prompt = orchbee_local_runtime::windows_interactive_powershell_initialization();
        let mut sample = TerminalProcessInfo {
            attachment_id: orchbee_terminal_protocol::AttachmentId::from_bytes([1; 16]).unwrap(),
            terminal_id: orchbee_terminal_protocol::TerminalId::from_bytes([2; 16]).unwrap(),
            root_pid: 1,
            processes: vec![TerminalProcess {
                pid: 1,
                parent_pid: None,
                creation_time: 1,
                image: Some(shell.into()),
                command_line: Some(format!(
                    "{shell} -NoLogo -NoProfile -NoExit -Command {}",
                    quoted_windows_argument(&prompt)
                )),
                current_directory: None,
            }],
        };
        assert!(
            sample.processes[0]
                .command_line
                .as_ref()
                .unwrap()
                .encode_utf16()
                .count()
                <= orchbee_terminal_protocol::MAXIMUM_PROCESS_TEXT_UTF16_UNITS
        );
        assert!(
            idle_shell(&sample).is_ok(),
            "runtime-created {shell} must admit startup"
        );
        for command in [
            format!("{shell} -NoExit -Command task"),
            format!("{shell} -Command {}", quoted_windows_argument(&prompt)),
            format!(
                "{shell} -NoExit -Command {}",
                quoted_windows_argument(&format!("{prompt}; Start-Sleep 10"))
            ),
            format!(
                "{shell} -NoExit -Command {} extra",
                quoted_windows_argument(&prompt)
            ),
            format!(
                "{shell} -ExecutionPolicy Bypass -NoExit -Command {}",
                quoted_windows_argument(&prompt)
            ),
        ] {
            sample.processes[0].command_line = Some(command.clone());
            assert!(idle_shell(&sample).is_err(), "unsafe shell: {command}");
        }
    }
}

#[test]
fn explicit_wake_reports_cancelled_without_overwriting_mutation_uncertainty() {
    let (reader, wake) = LocalStream::pair().unwrap();
    wake.shutdown(std::net::Shutdown::Both).unwrap();
    let cancelled = settle_cancellation::<()>(
        Err(error(FailureKind::TransportLoss, "launch-exchange-failed")),
        Some(reader.as_local_io()),
    )
    .unwrap_err();
    assert_eq!(cancelled.kind, FailureKind::Cancelled);
    let unknown = settle_cancellation::<()>(
        Err(error(FailureKind::OutcomeUnknown, "reply-not-confirmed")),
        Some(reader.as_local_io()),
    )
    .unwrap_err();
    assert_eq!(unknown.kind, FailureKind::OutcomeUnknown);
}

#[test]
fn typed_send_verdict_not_process_status_decides_delivery() {
    use crate::agent_runtime::SendVerdict;

    for (verdict, expected) in [
        (SendVerdict::Quiet, DeliveryDisposition::Applied),
        (
            SendVerdict::AcceptedCleanupUnconfirmed,
            DeliveryDisposition::Applied,
        ),
        (SendVerdict::Rejected, DeliveryDisposition::NotApplied),
        (
            SendVerdict::PartialAccepted,
            DeliveryDisposition::PartiallyApplied,
        ),
        (SendVerdict::Unavailable, DeliveryDisposition::NotApplied),
        (SendVerdict::OutcomeUnknown, DeliveryDisposition::Unknown),
    ] {
        assert_eq!(delivery_disposition(verdict), expected);
    }
}

// Model the real CreateProcess argv boundary, including quotes in the reader asset.
fn quoted_windows_argument(value: &str) -> String {
    let mut quoted = String::from("\"");
    let mut slashes = 0;
    for character in value.chars() {
        if character == '\\' {
            slashes += 1;
        } else {
            quoted.extend(std::iter::repeat_n(
                '\\',
                if character == '"' {
                    slashes * 2 + 1
                } else {
                    slashes
                },
            ));
            quoted.push(character);
            slashes = 0;
        }
    }
    quoted.extend(std::iter::repeat_n('\\', slashes * 2));
    quoted.push('"');
    quoted
}
