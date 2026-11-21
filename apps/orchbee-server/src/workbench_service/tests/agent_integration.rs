use super::*;
use orchbee_agent_integration::{IntegrationId, IntegrationOperation, IntegrationRequest};
use orchbee_workbench_protocol::{
    AgentIntegrationAction, AgentIntegrationFailure, AgentIntegrationJob, AgentIntegrationOutcome,
    AgentIntegrationRequest, ProtocolVersion, SessionId, WorkbenchRequestId, WorkbenchRevision,
    WorkerId,
};

fn outcome(
    service: &WorkbenchService,
    action: AgentIntegrationAction,
    session_id: SessionId,
) -> AgentIntegrationOutcome {
    let request = AgentIntegrationRequest {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&"1".repeat(32)).unwrap(),
        session_id,
        action,
    };
    let WorkbenchResponse::AgentIntegration(result) =
        response(service.execute(WorkbenchRequest::AgentIntegration(request)))
    else {
        panic!()
    };
    result.outcome
}

#[test]
fn enable_is_withheld_before_any_job_while_disable_poll_and_cancel_stay_served() {
    let terminals = Arc::new(LocalTerminalService::new(Arc::new(Mutex::new(
        WorkerRegistry::new(),
    ))));
    let service = WorkbenchService::new(
        WorkbenchPolicy {
            read: true,
            manage_navigation: true,
            associate_directory: true,
            create_terminal: true,
            manage_terminal_lifecycle: true,
        },
        terminals,
    );
    // No Session of this Server matches this identity. A request that passes the withheld check
    // answers `session-mismatch` before any Worker lookup, so an enable answer other than
    // `worker-unavailable` would show that the refusal did not run first.
    let stranger = SessionId::parse(&"f".repeat(32)).unwrap();
    let start = |operation| AgentIntegrationAction::Start {
        request: IntegrationRequest {
            operation,
            adapter_source: None,
            budget_milliseconds: None,
        },
    };
    let job = AgentIntegrationJob {
        worker_id: WorkerId::parse(&"4".repeat(32)).unwrap(),
        generation: WorkbenchRevision::new(1),
        job_id: "9".repeat(32),
    };
    let provider = IntegrationId::ALL[0];
    assert_eq!(
        outcome(
            &service,
            start(IntegrationOperation::Enable { provider }),
            stranger.clone()
        ),
        AgentIntegrationOutcome::Failed {
            reason: AgentIntegrationFailure::WorkerUnavailable
        }
    );
    assert_eq!(
        outcome(
            &service,
            start(IntegrationOperation::Disable { provider }),
            stranger.clone()
        ),
        AgentIntegrationOutcome::Failed {
            reason: AgentIntegrationFailure::SessionMismatch
        }
    );
    assert_eq!(
        outcome(
            &service,
            AgentIntegrationAction::Poll { job: job.clone() },
            stranger.clone()
        ),
        AgentIntegrationOutcome::Failed {
            reason: AgentIntegrationFailure::SessionMismatch
        }
    );
    assert_eq!(
        outcome(&service, AgentIntegrationAction::Cancel { job }, stranger),
        AgentIntegrationOutcome::Failed {
            reason: AgentIntegrationFailure::SessionMismatch
        }
    );
}
