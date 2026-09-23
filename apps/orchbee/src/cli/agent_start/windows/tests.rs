use super::*;

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
fn startup_outputs_preserve_delivery_and_reservation_facts_under_both_locales() {
    use orchbee_client_server_protocol::{SendCleanupStatus, SendProgress, TerminalSendOutcome};
    let options = AgentStartCommand {
        name: orchbee_agent_observation_protocol::AgentName::parse("alpha").unwrap(),
        provider: "codex".into(),
        panel: None,
        args: vec![],
        timeout_ms: 30000,
    };
    let now = Instant::now();
    let receipt = send::test_receipt(TerminalSendOutcome::Accepted {
        progress: SendProgress::complete(2, 10),
        cleanup: SendCleanupStatus::Confirmed,
    })
    .document();
    let mut workflow = Workflow {
        command: &options,
        started: now,
        deadline: now + Duration::from_secs(30),
        reservation: Some(ObapRequestId::parse(&"1".repeat(32)).unwrap()),
        last_observed: None,
        state: "pending",
        submission: Some(receipt.clone()),
        submission_status: Some(0),
        last_read_failure: None,
    };
    let success_schema = jsonschema::validator_for(
        &serde_json::from_str(include_str!("../../schemas/orchbee.agent.start.json")).unwrap(),
    )
    .unwrap();
    let failure_schema = jsonschema::validator_for(
        &serde_json::from_str(include_str!(
            "../../schemas/orchbee.cli.failure.agent-start.json"
        ))
        .unwrap(),
    )
    .unwrap();
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for (kind, reason, state) in [
            (FailureKind::Timeout, "start-deadline", "pending"),
            (FailureKind::Cancelled, "cancelled", "released"),
            (
                FailureKind::OutcomeUnknown,
                "reply-not-confirmed",
                "unknown",
            ),
        ] {
            workflow.state = state;
            let answer = workflow.failure(error(kind, reason), OutputFormat::Json, locale);
            let document: Value = serde_json::from_slice(&answer.stdout).unwrap();
            assert!(failure_schema.is_valid(&document), "{document}");
            assert_eq!(document["failure"]["details"]["submission"], receipt);
            assert_eq!(document["failure"]["details"]["reservationState"], state);
            assert_eq!(answer.status, kind.exit_code());
            assert!(answer.stderr.contains(if locale == Locale::EnUs {
                "No process was terminated"
            } else {
                "没有终止进程"
            }));
        }
        workflow.state = "completed";
        let answer = workflow.success(
            &AgentSessionId::parse("s1").unwrap(),
            OutputFormat::Json,
            locale,
        );
        let document: Value = serde_json::from_slice(&answer.stdout).unwrap();
        assert!(success_schema.is_valid(&document), "{document}");
        assert_eq!(document["result"]["submission"], receipt);
    }
}
