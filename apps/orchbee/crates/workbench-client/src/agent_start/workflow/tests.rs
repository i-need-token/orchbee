use super::*;
use orchbee_local_ipc::{AsLocalIo, LocalStream};

/// Only the Host's launch record admits a shell; its command line is never read. Any other live
/// program, an incomplete sample or a missing root refuses, and a sample without a launch record
/// comes from a Host too old to confirm a prompt.
#[test]
fn idle_shell_admits_only_a_shell_the_host_launched_with_prompt_marks() {
    let process = |pid, image: &str, command: &str| TerminalProcess {
        pid,
        parent_pid: None,
        creation_time: 1,
        console: orchbee_terminal_protocol::TerminalProcessConsole::Attached,
        image: Some(image.into()),
        command_line: Some(command.into()),
        current_directory: None,
    };
    let mut sample = TerminalProcessInfo {
        attachment_id: orchbee_terminal_protocol::AttachmentId::from_bytes([1; 16]).unwrap(),
        terminal_id: orchbee_terminal_protocol::TerminalId::from_bytes([2; 16]).unwrap(),
        root_pid: 1,
        processes: vec![process(1, "C:\\Windows\\cmd.exe", "cmd.exe /c task")],
        launch_kind: Some(TerminalLaunchKind::Cmd),
    };
    assert!(idle_shell(&sample).is_ok());
    sample.launch_kind = Some(TerminalLaunchKind::ManagedPowershell);
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
    sample.launch_kind = Some(TerminalLaunchKind::Other);
    assert!(matches!(
        idle_shell(&sample),
        Err(Failure::Start(StartFailure::TerminalNotIdle))
    ));
    sample.launch_kind = None;
    assert!(matches!(
        idle_shell(&sample),
        Err(Failure::Start(StartFailure::TerminalTooOld))
    ));
    sample.launch_kind = Some(TerminalLaunchKind::Cmd);
    sample.processes.clear();
    assert!(idle_shell(&sample).is_err());
}

#[test]
fn explicit_wake_reports_cancelled_without_overwriting_mutation_uncertainty() {
    let (reader, wake) = LocalStream::pair().unwrap();
    wake.shutdown(std::net::Shutdown::Both).unwrap();
    let cancelled = settle_cancellation::<()>(
        Err(Failure::Start(StartFailure::Exchange(
            ExchangeFailure::Transport,
        ))),
        Some(reader.as_local_io()),
    )
    .unwrap_err();
    assert!(matches!(cancelled, Failure::Start(StartFailure::Cancelled)));
    let unknown = settle_cancellation::<()>(
        Err(Failure::Start(StartFailure::ReplyNotConfirmed)),
        Some(reader.as_local_io()),
    )
    .unwrap_err();
    assert!(matches!(
        unknown,
        Failure::Start(StartFailure::ReplyNotConfirmed)
    ));
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
