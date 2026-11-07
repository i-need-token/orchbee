//! A change whose effect may have begun must read as unknown, never as a definite refusal, and is
//! never resubmitted; only an unanswered start proves that nothing changed.
use super::*;

const AGENT: IntegrationId = IntegrationId::ClaudeCode;

fn item(id: IntegrationId, state: State) -> SnapshotItem {
    SnapshotItem {
        availability: Availability::Available,
        integration_id: id,
        issue: None,
        management_mode: ManagementMode::ManagedLocal,
        state,
        timed_out: false,
    }
}

fn capsule() -> CapsuleStatus {
    CapsuleStatus {
        ready: true,
        root: std::path::PathBuf::new(),
        source_kind: "cohort-sidecar",
        binding_failure: false,
        payload_sha256: String::new(),
    }
}

/// Every Agent reported available and not installed, then one enable admitted and submitted.
fn enabling() -> Integrations {
    let mut integrations = Integrations::default();
    assert!(integrations.begin(IntegrationJob::Refresh));
    assert_eq!(
        integrations.take_submission(),
        Some(IntegrationJob::Refresh)
    );
    integrations.settle(JobEnd::Completed(Box::new(IntegrationReport::List {
        items: IntegrationId::ALL
            .map(|id| item(id, State::NotInstalled))
            .to_vec(),
        capsule: capsule(),
    })));
    let job = integrations
        .change(AGENT)
        .expect("an available Agent can be enabled");
    assert_eq!(
        job,
        IntegrationJob::Change {
            id: AGENT,
            direction: Direction::Enable
        }
    );
    assert!(integrations.begin(job));
    assert_eq!(integrations.take_submission(), Some(job));
    integrations
}

#[test]
fn a_change_that_may_have_begun_settles_unknown_without_resubmission() {
    for end in [
        JobEnd::Deadline,
        JobEnd::Failed {
            reason: AgentIntegrationFailure::WorkerChanged,
            started: true,
        },
        JobEnd::Failed {
            reason: AgentIntegrationFailure::Abandoned,
            started: false,
        },
        JobEnd::Exchange {
            started: false,
            submitted: true,
        },
        JobEnd::Completed(Box::new(IntegrationReport::Mutation {
            report: MutationReport {
                integration_id: AGENT,
                direction: Direction::Enable,
                attempted: Vec::new(),
                residuals: Vec::new(),
                observed: None,
                provenance: orchbee_agent_integration::Provenance::Unavailable,
                effect_begun: true,
                verdict: Verdict::Unknown,
            },
        })),
    ] {
        let mut integrations = enabling();
        assert!(integrations.settle(end));
        assert_eq!(integrations.status(AGENT), IntegrationStatus::Unknown);
        assert_eq!(
            integrations.failure(),
            Some(IntegrationFailure::VerificationFailed)
        );
        assert_eq!(integrations.block(AGENT), Some(IntegrationBlock::Unknown));
        assert!(!integrations.pending());
        assert_eq!(integrations.take_submission(), None);
        assert_eq!(
            integrations.status(IntegrationId::Codex),
            IntegrationStatus::NotInstalled
        );
    }
}

#[test]
fn an_unanswered_start_is_definite_and_keeps_the_reported_row() {
    for (end, failure) in [
        (
            JobEnd::Failed {
                reason: AgentIntegrationFailure::Busy,
                started: false,
            },
            IntegrationFailure::Busy,
        ),
        (
            JobEnd::Exchange {
                started: false,
                submitted: false,
            },
            IntegrationFailure::ControlUnavailable,
        ),
    ] {
        let mut integrations = enabling();
        integrations.settle(end);
        assert_eq!(integrations.failure(), Some(failure));
        assert_eq!(integrations.status(AGENT), IntegrationStatus::NotInstalled);
        assert_eq!(integrations.block(AGENT), None);
    }
}

#[test]
fn a_refresh_reports_an_unlisted_agent_as_unavailable_not_missing() {
    let mut integrations = Integrations::default();
    assert!(
        integrations.block(AGENT).is_some(),
        "rows start uninspected"
    );
    integrations.begin(IntegrationJob::Refresh);
    integrations.settle(JobEnd::Completed(Box::new(IntegrationReport::List {
        items: vec![item(IntegrationId::Codex, State::Enabled)],
        capsule: capsule(),
    })));
    assert!(integrations.reported());
    assert_eq!(
        integrations.status(IntegrationId::Codex),
        IntegrationStatus::Enabled
    );
    assert_eq!(
        integrations.status(AGENT),
        IntegrationStatus::InspectionFailed
    );
    assert_eq!(integrations.block(AGENT), Some(IntegrationBlock::Unknown));
}

#[test]
fn a_diagnosis_separates_definite_findings_from_unknown_results() {
    let explain = |item: SnapshotItem| {
        let mut integrations = Integrations::default();
        integrations.begin(IntegrationJob::Explain(AGENT));
        assert_eq!(integrations.diagnosis(AGENT), Some(&Diagnosis::Pending));
        integrations.settle(JobEnd::Completed(Box::new(IntegrationReport::Explain {
            item,
            detail: InspectionDetail::Plain {},
            capsule: capsule(),
            plan: Vec::new(),
        })));
        integrations
    };
    let finding = |item: SnapshotItem| match explain(item).diagnosis(AGENT) {
        Some(Diagnosis::Explained(explanation)) => explanation.finding(),
        other => panic!("explained diagnosis expected, got {other:?}"),
    };
    assert_eq!(
        finding(SnapshotItem {
            availability: Availability::NotFound,
            ..item(AGENT, State::NotInstalled)
        }),
        IntegrationFinding::AgentMissing
    );
    assert_eq!(
        finding(item(AGENT, State::NotInstalled)),
        IntegrationFinding::NotInstalled
    );
    assert_eq!(
        finding(SnapshotItem {
            timed_out: true,
            ..item(AGENT, State::Unknown)
        }),
        IntegrationFinding::Unknown(IntegrationUnknown::TimedOut)
    );
    assert_eq!(
        finding(SnapshotItem {
            issue: Some(Issue::InspectionFailed),
            ..item(AGENT, State::NotInstalled)
        }),
        IntegrationFinding::Unknown(IntegrationUnknown::InspectionFailed)
    );
    let other = explain(item(IntegrationId::Codex, State::Enabled));
    assert!(matches!(other.diagnosis(AGENT), Some(Diagnosis::Failed(_))));
    assert_eq!(
        other.status(IntegrationId::Codex),
        IntegrationStatus::Unknown
    );
}
