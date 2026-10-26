use super::*;
use crate::cli::agent_prompt::workflow::settle;
use orchbee_client_server_protocol::{SendCleanupStatus, SendProgress, TerminalSendOutcome};

fn binding() -> AgentLaunchBinding {
    let id = "1".repeat(32);
    serde_json::from_value(
        json!({"presentationId":id,"generation":id,"hostEpoch":id,"hostTerminalId":id}),
    )
    .unwrap()
}
fn receipt() -> send::Receipt {
    send::test_receipt(TerminalSendOutcome::Accepted {
        progress: SendProgress::complete(2, 5),
        cleanup: SendCleanupStatus::Confirmed,
    })
}
fn validate(output: &Output, schema: &str) -> Value {
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    let schema: Value = serde_json::from_str(schema).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let errors: Vec<_> = validator
        .iter_errors(&document)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{errors:?}: {document}");
    document
}

#[test]
fn physical_prompt_success_and_post_send_cancellation_preserve_receipts() {
    for waiting in [false, true] {
        let sent = Sent {
            receipt: receipt(),
            completion: Completion::Terminal {
                binding: binding(),
                waited: waiting.then(|| Err(error(FailureKind::Timeout, "effect-deadline"))),
            },
        };
        let output = settle(Ok(sent), true, false, OutputFormat::Json, Locale::EnUs);
        let doc = validate(
            &output,
            if waiting {
                include_str!("../../../schemas/orchbee.cli.failure.agent-prompt.json")
            } else {
                include_str!("../../../schemas/orchbee.agent.prompt.json")
            },
        );
        assert_eq!(output.status, if waiting { 130 } else { 0 });
        let result = if waiting {
            &doc["failure"]["details"]
        } else {
            &doc["result"]
        };
        assert_eq!(result["submission"]["result"]["inputStatus"], "accepted");
        assert_eq!(result["target"]["kind"], "terminal");
        assert!(result.get("sessionId").is_none());
    }
}

#[test]
fn physical_prompt_failure_and_legacy_variants_are_closed() {
    let output = failed_after_send(
        error(FailureKind::Timeout, "effect-deadline"),
        binding(),
        receipt().document(),
        OutputFormat::Json,
        Locale::EnUs,
    );
    let schema: Value = serde_json::from_str(include_str!(
        "../../../schemas/orchbee.cli.failure.agent-prompt.json"
    ))
    .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let mut doc = validate(
        &output,
        include_str!("../../../schemas/orchbee.cli.failure.agent-prompt.json"),
    );
    doc["failure"]["details"]["sessionId"] = json!("fabricated");
    assert!(!validator.is_valid(&doc));
    let output = success(
        binding(),
        receipt().document(),
        None,
        OutputFormat::Json,
        Locale::EnUs,
    );
    let mut doc = validate(
        &output,
        include_str!("../../../schemas/orchbee.agent.prompt.json"),
    );
    let schema: Value =
        serde_json::from_str(include_str!("../../../schemas/orchbee.agent.prompt.json")).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    doc["version"] = json!(4);
    assert!(!validator.is_valid(&doc));
}

#[test]
fn physical_prompt_unknown_state_requires_known_provider_and_blocked_refuses() {
    let mut read = TerminalAgentRead {
        source: json!({}),
        report_revision: orchbee_workbench_protocol::WorkbenchRevision::new(1),
        observation: Box::new(orchbee_workbench_protocol::TerminalAgentObservation {
            binding: binding(),
            sequence: orchbee_workbench_protocol::WorkbenchRevision::new(1),
            changes: vec![],
            view: orchbee_workbench_protocol::PanelAgentView {
                agent: Some("pi".into()),
                ..Default::default()
            },
        }),
    };
    assert_eq!(provider(&read).unwrap(), "pi");
    read.observation.view.state = PanelAgentState::Blocked;
    assert!(provider(&read).is_err());
    read.observation.view.state = PanelAgentState::Idle;
    read.observation.view.agent = Some("custom".into());
    assert!(provider(&read).is_err());
}
