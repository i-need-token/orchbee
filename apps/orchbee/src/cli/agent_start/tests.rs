use super::*;
use crate::agent_start::{ReservationState, Verdict};
use orchbee_agent_observation_protocol::{
    AgentName, AgentProviderId, AgentSessionId, ObapRequestId,
};
use orchbee_client_server_protocol::{SendCleanupStatus, SendProgress, TerminalSendOutcome};
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
                        failure: crate::agent_start::failure(kind, reason),
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
            failure: crate::agent_start::failure(FailureKind::Timeout, "start-deadline"),
            reason: "start-deadline".into(),
            cause: Some(read_cause.clone()),
        },
    );
    retained.replace_failure(crate::agent_start::failure(
        FailureKind::Cancelled,
        "cancelled",
    ));
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
    upstream.replace_failure(crate::agent_start::failure(
        FailureKind::Cancelled,
        "cancelled",
    ));
    let Verdict::Failed { cause, .. } = upstream.verdict else {
        panic!("replacement stays failed")
    };
    assert_eq!(cause, None);
}
