use super::*;
use crate::agent_start::ReservationState;
use orchbee_agent_observation_protocol::{
    AgentName, AgentProviderId, AgentSessionId, ObapRequestId,
};
use orchbee_client_server_protocol::{
    SendCleanupStatus, SendProgress, SendRejectedReason, SendUnavailableReason, TerminalSendOutcome,
};
use orchbee_workbench_client::test_support::agent_runtime::startup::send::ReceiptTestAccess;
use orchbee_workbench_client::test_support::agent_start::DeliveryDisposition;
use orchbee_workbench_client::test_support::agent_start::ReportFixture;
use serde_json::Value;

fn report(receipt: &Value, state: ReservationState, verdict: Verdict) -> Report {
    Report {
        name: AgentName::parse("alpha").unwrap(),
        provider: AgentProviderId::parse("codex").unwrap(),
        reservation: Some(ObapRequestId::parse(&"1".repeat(32)).unwrap()),
        last_observed: None,
        reservation_state: state,
        submission: Some(receipt.clone()),
        submission_status: Some(0),
        elapsed_ms: 1,
        verdict,
    }
}

#[test]
fn every_non_success_send_receipt_keeps_its_cli_status_and_exact_envelope() {
    use super::super::send::SendVerdict;
    use crate::agent_runtime::startup::{binding::Binding, send::Receipt as DomainReceipt};
    use crate::agent_start::{
        Failure as DomainFailure, Report as DomainReport, StartFailure, Submission,
        Verdict as DomainVerdict,
    };
    use orchbee_client_server_protocol::{
        HostEpoch, HostTerminalId, RequestId, SendTarget, TerminalSendResult,
    };
    use orchbee_workbench_client::test_support::agent_runtime::startup::send::Context;

    let cases = [
        (
            TerminalSendOutcome::Rejected {
                reason: SendRejectedReason::HostInputBackpressure,
                progress: SendProgress::none(),
                cleanup: SendCleanupStatus::Confirmed,
            },
            3,
            SendVerdict::Rejected,
            DeliveryDisposition::NotApplied,
            FailureKind::Rejected,
        ),
        (
            TerminalSendOutcome::Unavailable {
                reason: SendUnavailableReason::WorkerUnavailable,
                progress: SendProgress::none(),
                cleanup: SendCleanupStatus::NotNeeded,
            },
            4,
            SendVerdict::Unavailable,
            DeliveryDisposition::NotApplied,
            FailureKind::ServiceUnavailable,
        ),
        (
            TerminalSendOutcome::Unavailable {
                reason: SendUnavailableReason::HostProtocolFault,
                progress: SendProgress::none(),
                cleanup: SendCleanupStatus::NotNeeded,
            },
            5,
            SendVerdict::Unavailable,
            DeliveryDisposition::NotApplied,
            FailureKind::TransportLoss,
        ),
        (
            TerminalSendOutcome::Accepted {
                progress: SendProgress::complete(2, 10),
                cleanup: SendCleanupStatus::Unconfirmed,
            },
            5,
            SendVerdict::AcceptedCleanupUnconfirmed,
            DeliveryDisposition::Applied,
            FailureKind::TransportLoss,
        ),
        (
            TerminalSendOutcome::OutcomeUnknown {
                progress: SendProgress::none(),
                cleanup: SendCleanupStatus::Unconfirmed,
            },
            8,
            SendVerdict::OutcomeUnknown,
            DeliveryDisposition::Unknown,
            FailureKind::OutcomeUnknown,
        ),
        (
            TerminalSendOutcome::Rejected {
                reason: SendRejectedReason::HostInputBackpressure,
                progress: SendProgress::stopped(1, 4),
                cleanup: SendCleanupStatus::Confirmed,
            },
            9,
            SendVerdict::PartialAccepted,
            DeliveryDisposition::PartiallyApplied,
            FailureKind::Rejected,
        ),
        (
            TerminalSendOutcome::Unavailable {
                reason: SendUnavailableReason::InputBudgetExhausted,
                progress: SendProgress::stopped(1, 4),
                cleanup: SendCleanupStatus::Confirmed,
            },
            9,
            SendVerdict::PartialAccepted,
            DeliveryDisposition::PartiallyApplied,
            FailureKind::Rejected,
        ),
    ];
    let schema = jsonschema::validator_for(
        &serde_json::from_str(include_str!(
            "../schemas/orchbee.cli.failure.agent-start.json"
        ))
        .unwrap(),
    )
    .unwrap();
    for (outcome, status, expected_verdict, delivery, failure_kind) in cases {
        let receipt = super::super::send::test_receipt(outcome.clone());
        assert_eq!(receipt.exit_status(), status);
        assert_eq!(receipt.verdict(), expected_verdict);
        let document = receipt.document();
        let request = RequestId::parse(&"1".repeat(32)).unwrap();
        let target = SendTarget::new(
            std::num::NonZeroU64::MIN,
            HostEpoch::parse(&"2".repeat(32)).unwrap(),
            HostTerminalId::parse(&"3".repeat(32)).unwrap(),
        );
        let receipt = DomainReceipt {
            context: Context {
                binding: Binding::parse(&"4".repeat(32), &"5".repeat(32), &"6".repeat(32)).unwrap(),
                revision: orchbee_workbench_protocol::WorkbenchRevision::new(1),
                request: request.clone(),
            },
            result: TerminalSendResult::new(request, Some(target), outcome).unwrap(),
            records: 2,
            bytes: 10,
        };
        assert_eq!(receipt.verdict(), expected_verdict);
        let failure =
            DomainFailure::Start(StartFailure::InputNotAccepted(receipt.refusal().unwrap()));
        let report = Report::from(DomainReport::from(ReportFixture {
            name: AgentName::parse("alpha").unwrap(),
            provider: AgentProviderId::parse("codex").unwrap(),
            reservation: Some(ObapRequestId::parse(&"1".repeat(32)).unwrap()),
            last_observed: None,
            reservation_state: ReservationState::Released,
            submission: Some(Submission::Sent(Box::new(receipt))),
            delivery,
            elapsed_ms: 1,
            verdict: DomainVerdict::Failed {
                failure,
                read_cause: None,
            },
        }));
        assert_eq!(report.failure_kind(), Some(failure_kind));
        let answer = project(report, OutputFormat::Json, Locale::EnUs, None);
        assert_eq!(answer.status, status);
        let envelope: Value = serde_json::from_slice(&answer.stdout).unwrap();
        assert!(schema.is_valid(&envelope), "{envelope}");
        assert_eq!(envelope["failure"]["details"]["submission"], document);
        assert!(
            envelope["failure"]["details"].get("delivery").is_none(),
            "delivery is private state, not a new CLI field"
        );
    }
}

#[test]
fn startup_outputs_preserve_delivery_and_reservation_facts_under_both_locales() {
    let receipt = super::super::send::test_receipt(TerminalSendOutcome::Accepted {
        progress: SendProgress::complete(2, 10),
        cleanup: SendCleanupStatus::Confirmed,
    })
    .document();
    let success_schema = jsonschema::validator_for(
        &serde_json::from_str(include_str!("../schemas/orchbee.agent.start.json")).unwrap(),
    )
    .unwrap();
    let failure_schema = jsonschema::validator_for(
        &serde_json::from_str(include_str!(
            "../schemas/orchbee.cli.failure.agent-start.json"
        ))
        .unwrap(),
    )
    .unwrap();
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for (kind, reason, state) in [
            (
                FailureKind::Timeout,
                "start-deadline",
                ReservationState::Pending,
            ),
            (
                FailureKind::Cancelled,
                "cancelled",
                ReservationState::Released,
            ),
            (
                FailureKind::OutcomeUnknown,
                "reply-not-confirmed",
                ReservationState::Unknown,
            ),
        ] {
            let answer = project(
                report(
                    &receipt,
                    state,
                    Verdict::Failed {
                        failure: super::failure(kind, reason),
                        reason: reason.to_owned(),
                        cause: None,
                    },
                ),
                OutputFormat::Json,
                locale,
                None,
            );
            let document: Value = serde_json::from_slice(&answer.stdout).unwrap();
            assert!(failure_schema.is_valid(&document), "{document}");
            assert_eq!(document["failure"]["details"]["submission"], receipt);
            assert_eq!(
                document["failure"]["details"]["reservationState"],
                state.as_str()
            );
            assert_eq!(answer.status, kind.exit_code());
            assert!(answer.stderr.contains(if locale == Locale::EnUs {
                "No process was terminated"
            } else {
                "没有终止进程"
            }));
        }
        let answer = project(
            report(
                &receipt,
                ReservationState::Completed,
                Verdict::Started {
                    session: AgentSessionId::parse("s1").unwrap(),
                },
            ),
            OutputFormat::Json,
            locale,
            None,
        );
        let document: Value = serde_json::from_slice(&answer.stdout).unwrap();
        assert!(success_schema.is_valid(&document), "{document}");
        assert_eq!(document["result"]["submission"], receipt);
    }
}

#[test]
fn finish_boundary_replacement_retains_only_the_workflow_read_cause() {
    let receipt = serde_json::json!({"kind":"fixture"});
    let read_cause = serde_json::json!({"kind":"transport-loss", "stage":"agent-read"});
    let mut retained = report(
        &receipt,
        ReservationState::Pending,
        Verdict::Failed {
            failure: super::failure(FailureKind::Timeout, "start-deadline"),
            reason: "start-deadline".into(),
            cause: Some(read_cause.clone()),
        },
    );
    retained.replace_failure(super::failure(FailureKind::Cancelled, "cancelled"));
    let Verdict::Failed { cause, .. } = retained.verdict else {
        panic!("replacement stays failed")
    };
    assert_eq!(cause, Some(read_cause));

    let mut upstream = report(
        &receipt,
        ReservationState::None,
        Verdict::Failed {
            failure: Failure::new(FailureKind::TransportLoss)
                .details(super::super::stage_details("admission")),
            reason: "startup-failed".into(),
            cause: Some(serde_json::json!({"kind":"transport-loss"})),
        },
    );
    upstream.replace_failure(super::failure(FailureKind::Cancelled, "cancelled"));
    let Verdict::Failed { cause, .. } = upstream.verdict else {
        panic!("replacement stays failed")
    };
    assert_eq!(cause, None);
}
