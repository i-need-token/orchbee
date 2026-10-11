use super::*;
use orchbee_client_server_protocol::{
    SendCleanupStatus, SendProgress, SendUnavailableReason, TerminalSendOutcome,
};

fn sent(outcome: TerminalSendOutcome, waiting: bool) -> Result<Sent, Failure> {
    Ok(Sent {
        receipt: send::test_receipt(outcome),
        completion: Completion::Session {
            session_id: "sess-alpha".to_owned(),
            waited: waiting
                .then(|| Err(prompt_error(FailureKind::TransportLoss, "wait-read-failed"))),
        },
    })
}
fn accepted() -> TerminalSendOutcome {
    TerminalSendOutcome::Accepted {
        progress: SendProgress::complete(2, 5),
        cleanup: SendCleanupStatus::Confirmed,
    }
}
fn document(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap()
}
fn validate(output: &Output, schema: &str) {
    let schema = serde_json::from_str::<Value>(schema).unwrap();
    assert!(
        jsonschema::validator_for(&schema)
            .unwrap()
            .is_valid(&document(output))
    );
}

#[test]
fn cancellation_preserves_delivery_and_only_cancels_the_wait() {
    let submitted = settle(
        sent(accepted(), false),
        true,
        false,
        OutputFormat::Json,
        Locale::EnUs,
    );
    assert_eq!(submitted.status, 0);
    validate(
        &submitted,
        include_str!("../../schemas/orchbee.agent.prompt.json"),
    );
    let waiting = settle(
        sent(accepted(), true),
        true,
        false,
        OutputFormat::Json,
        Locale::EnUs,
    );
    assert_eq!(waiting.status, 130);
    assert_eq!(
        document(&waiting)["failure"]["details"]["submission"]["result"]["inputStatus"],
        "accepted"
    );
    validate(
        &waiting,
        include_str!("../../schemas/orchbee.cli.failure.agent-prompt.json"),
    );
    let guard_fault = settle(
        sent(accepted(), false),
        false,
        true,
        OutputFormat::Json,
        Locale::EnUs,
    );
    assert_eq!(guard_fault.status, 1);
    validate(
        &guard_fault,
        include_str!("../../schemas/orchbee.cli.failure.agent-prompt.json"),
    );
    assert_eq!(
        document(&guard_fault)["failure"]["details"]["submission"]["result"]["inputStatus"],
        "accepted"
    );
}

#[test]
fn cancellation_never_erases_unknown_or_partial_delivery() {
    let lost = settle(
        Err(Failure::new(FailureKind::OutcomeUnknown)),
        true,
        false,
        OutputFormat::Json,
        Locale::EnUs,
    );
    assert_eq!(lost.status, 8);
    let unknown = TerminalSendOutcome::OutcomeUnknown {
        progress: SendProgress::stopped(1, 4),
        cleanup: SendCleanupStatus::Unconfirmed,
    };
    let output = settle(
        sent(unknown, false),
        true,
        false,
        OutputFormat::Json,
        Locale::EnUs,
    );
    assert_eq!(output.status, 8);
    validate(
        &output,
        include_str!("../../schemas/orchbee.terminal.send.json"),
    );
    let partial = TerminalSendOutcome::Unavailable {
        reason: SendUnavailableReason::InputBudgetExhausted,
        progress: SendProgress::stopped(1, 4),
        cleanup: SendCleanupStatus::Confirmed,
    };
    let output = settle(
        sent(partial, false),
        true,
        false,
        OutputFormat::Json,
        Locale::EnUs,
    );
    assert_eq!(output.status, 9);
    validate(
        &output,
        include_str!("../../schemas/orchbee.terminal.send.json"),
    );
    let before = settle(
        Err(Failure::new(FailureKind::TransportLoss)),
        true,
        false,
        OutputFormat::Json,
        Locale::EnUs,
    );
    assert_eq!(before.status, 130);
    validate(
        &before,
        include_str!("../../schemas/orchbee.cli.failure.agent-prompt.json"),
    );
    assert!(
        document(&before)["failure"]["details"]
            .get("submission")
            .is_none()
    );
}
