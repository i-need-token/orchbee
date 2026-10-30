//! Relays an authorized Agent integration job action to the selected Worker. Session and Worker
//! generation checks precede the relay; only the Worker inspects or changes provider configuration.
use super::LocalTerminalService;
use crate::worker_obcn_carrier::{
    WorkerOperationRequest, WorkerOperationResult, WorkerRequestError,
};
use orchbee_capability_node_protocol as node;
use orchbee_workbench_protocol::{
    AgentIntegrationAction as Action, AgentIntegrationFailure as Failure, AgentIntegrationJob,
    AgentIntegrationOutcome as Outcome, AgentIntegrationRequest, AgentIntegrationResult,
    ProtocolVersion, WorkbenchRevision, WorkerId,
};

impl LocalTerminalService {
    pub(crate) fn execute_agent_integration(
        &self,
        request: AgentIntegrationRequest,
    ) -> AgentIntegrationResult {
        let outcome = self
            .agent_integration(&request)
            .unwrap_or_else(|reason| Outcome::Failed { reason });
        AgentIntegrationResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            session_id: request.session_id,
            outcome,
        }
    }

    fn agent_integration(&self, request: &AgentIntegrationRequest) -> Result<Outcome, Failure> {
        request.validate().map_err(|_| Failure::InvalidRequest)?;
        if self.workbench_session_matches(&request.session_id) != Some(true) {
            return Err(Failure::SessionMismatch);
        }
        let handle = self
            .workers
            .lock()
            .map_err(|_| Failure::WorkerUnavailable)?
            .request_handle()
            .ok_or(Failure::WorkerUnavailable)?;
        let worker_id = WorkerId::parse(&hex(handle.worker_id().as_bytes()))
            .map_err(|_| Failure::WorkerUnavailable)?;
        let generation = WorkbenchRevision::new(handle.generation().get());
        let action = match &request.action {
            Action::Start { request } => {
                let document = orchbee_agent_integration::encode_request(request)
                    .map_err(|_| Failure::InvalidRequest)?;
                node::AgentIntegrationJobAction::start(document)
                    .map_err(|_| Failure::InvalidRequest)?
            }
            // A job is fenced to the Worker generation that admitted it; a replacement Worker
            // never answers for it.
            Action::Poll { job } | Action::Cancel { job }
                if job.worker_id != worker_id || job.generation != generation =>
            {
                return Err(Failure::WorkerChanged);
            }
            Action::Poll { job } => node::AgentIntegrationJobAction::Poll {
                job_id: identity(&job.job_id)?,
            },
            Action::Cancel { job } => node::AgentIntegrationJobAction::Cancel {
                job_id: identity(&job.job_id)?,
            },
        };
        let value = self
            .coordinator
            .lock()
            .map_err(|_| Failure::WorkerUnavailable)?
            .allocate_identity_block(1)
            .ok_or(Failure::WorkerUnavailable)?;
        let dispatch = node::AgentIntegrationJobRequest {
            operation_id: node::OperationId::from_bytes(value.to_be_bytes())
                .map_err(|_| Failure::WorkerUnavailable)?,
            worker_id: node::WorkerId::from_bytes(handle.worker_id().as_bytes())
                .map_err(|_| Failure::WorkerUnavailable)?,
            worker_generation: node::WorkerGeneration::new(handle.generation()),
            action,
        };
        let result = match handle.request(WorkerOperationRequest::AgentIntegrationJob(dispatch)) {
            Ok(WorkerOperationResult::AgentIntegrationJob(result)) => result,
            Err(WorkerRequestError::Backpressure) => return Err(Failure::Busy),
            // A delivered enable or disable may have started: its effects are unknown.
            Err(WorkerRequestError::PostDeliveryOutcomeUnknown(_)) | Ok(_)
                if request.action.mutating() =>
            {
                return Err(Failure::Abandoned);
            }
            Err(_) | Ok(_) => return Err(Failure::WorkerUnavailable),
        };
        let job = AgentIntegrationJob {
            worker_id,
            generation,
            job_id: hex(result.job_id.as_bytes()),
        };
        use node::{
            AgentIntegrationJobCancelOutcome as Cancel, AgentIntegrationJobOutcome as Job,
            AgentIntegrationJobPollOutcome as Poll, AgentIntegrationJobStartOutcome as Start,
        };
        Ok(match result.outcome {
            Job::Start(Start::Accepted) => Outcome::Accepted { job },
            Job::Start(Start::Busy) => return Err(Failure::Busy),
            Job::Start(Start::InvalidRequest) => return Err(Failure::InvalidRequest),
            Job::Start(Start::SpawnThreadFailed | Start::Dead) => {
                return Err(Failure::WorkerUnavailable);
            }
            Job::Poll(Poll::Pending) => Outcome::Pending { job },
            Job::Poll(Poll::Completed { report }) => Outcome::Completed {
                job,
                report: Box::new(
                    orchbee_agent_integration::decode_report(&report)
                        .map_err(|_| Failure::Malformed)?,
                ),
            },
            Job::Poll(Poll::Abandoned) => return Err(Failure::Abandoned),
            Job::Cancel(Cancel::Registered) => Outcome::CancelRegistered { job },
            Job::Cancel(Cancel::AlreadyTerminal) => Outcome::AlreadyTerminal { job },
            Job::Poll(Poll::Unknown) | Job::Cancel(Cancel::Unknown) => {
                return Err(Failure::JobUnknown);
            }
            Job::Start(Start::RejectedFencing)
            | Job::Poll(Poll::RejectedFencing)
            | Job::Cancel(Cancel::RejectedFencing) => return Err(Failure::WorkerChanged),
        })
    }
}

fn identity(value: &str) -> Result<node::OperationId, Failure> {
    u128::from_str_radix(value, 16)
        .ok()
        .and_then(|n| node::OperationId::from_bytes(n.to_be_bytes()).ok())
        .ok_or(Failure::InvalidRequest)
}

fn hex(value: [u8; 16]) -> String {
    format!("{:032x}", u128::from_be_bytes(value))
}
