use orchbee_agent_integration::{
    IntegrationId, IntegrationOperation, IntegrationReport, IntegrationRequest,
};
use orchbee_workbench_protocol::*;

fn hex(value: u128) -> String {
    format!("{value:032x}")
}

fn job(id: u128) -> AgentIntegrationJob {
    AgentIntegrationJob {
        worker_id: WorkerId::parse(&hex(4)).unwrap(),
        generation: WorkbenchRevision::new(1),
        job_id: hex(id),
    }
}

fn request(action: AgentIntegrationAction) -> AgentIntegrationRequest {
    AgentIntegrationRequest {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&hex(1)).unwrap(),
        session_id: SessionId::parse(&hex(2)).unwrap(),
        action,
    }
}

fn start(operation: IntegrationOperation) -> AgentIntegrationRequest {
    request(AgentIntegrationAction::Start {
        request: IntegrationRequest {
            operation,
            adapter_source: None,
            budget_milliseconds: None,
        },
    })
}

fn result(
    request: &AgentIntegrationRequest,
    outcome: AgentIntegrationOutcome,
) -> AgentIntegrationResult {
    AgentIntegrationResult {
        protocol_version: ProtocolVersion,
        request_id: request.request_id.clone(),
        session_id: request.session_id.clone(),
        outcome,
    }
}

#[test]
fn agent_integration_shared_corpus() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/workbench-agent-integration-conformance-v1.json"
    ))
    .unwrap();
    for (key, expected) in [("valid", true), ("invalid", false)] {
        for case in corpus[key].as_array().unwrap() {
            let bytes = serde_json::to_vec(&case["document"]).unwrap();
            let actual = if case["direction"] == "request" {
                parse_workbench_request(&bytes).is_ok()
            } else {
                parse_workbench_response(&bytes).is_ok()
            };
            assert_eq!(actual, expected, "{}", case["name"]);
        }
    }
}

#[test]
fn agent_integration_uses_distinct_carriers_and_answers_only_its_job() {
    let list = start(IntegrationOperation::List { provider: None });
    assert!(!list.action.mutating());
    assert!(
        start(IntegrationOperation::Disable {
            provider: IntegrationId::ALL[0]
        })
        .action
        .mutating()
    );
    let mut bytes = Vec::new();
    write_workbench_request(
        &mut bytes,
        &WorkbenchRequest::AgentIntegration(list.clone()),
    )
    .unwrap();
    assert_eq!(&bytes[6..8], &63_u16.to_be_bytes());
    assert_eq!(
        read_workbench_request(&mut bytes.as_slice()).unwrap(),
        WorkbenchRequest::AgentIntegration(list.clone())
    );
    let accepted = result(&list, AgentIntegrationOutcome::Accepted { job: job(3) });
    assert!(accepted.answers(&list));
    bytes.clear();
    write_workbench_response(
        &mut bytes,
        &WorkbenchResponse::AgentIntegration(accepted.clone()),
    )
    .unwrap();
    assert_eq!(&bytes[6..8], &64_u16.to_be_bytes());
    assert_eq!(
        read_workbench_response(&mut bytes.as_slice()).unwrap(),
        WorkbenchResponse::AgentIntegration(accepted)
    );
    assert!(!result(&list, AgentIntegrationOutcome::Pending { job: job(3) }).answers(&list));

    let poll = request(AgentIntegrationAction::Poll { job: job(3) });
    let completed = AgentIntegrationOutcome::Completed {
        job: job(3),
        report: Box::new(IntegrationReport::HomeUnavailable {}),
    };
    assert!(result(&poll, completed).answers(&poll));
    assert!(!result(&poll, AgentIntegrationOutcome::Pending { job: job(5) }).answers(&poll));
    let cancel = request(AgentIntegrationAction::Cancel { job: job(3) });
    let failed = AgentIntegrationOutcome::Failed {
        reason: AgentIntegrationFailure::WorkerChanged,
    };
    assert!(result(&cancel, failed).answers(&cancel));
    assert!(!result(&cancel, AgentIntegrationOutcome::Pending { job: job(3) }).answers(&cancel));
}

#[test]
fn agent_integration_rejects_unfenced_jobs_and_invalid_budgets() {
    let mut unfenced = job(3);
    unfenced.generation = WorkbenchRevision::new(0);
    let mut uppercase = job(0xA);
    uppercase.job_id.make_ascii_uppercase();
    for job in [unfenced, uppercase] {
        assert!(
            request(AgentIntegrationAction::Poll { job })
                .validate()
                .is_err()
        );
    }
    let mut budget = start(IntegrationOperation::List { provider: None });
    if let AgentIntegrationAction::Start { request } = &mut budget.action {
        request.budget_milliseconds = Some(0);
    }
    assert!(budget.validate().is_err());
}
