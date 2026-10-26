//! Bounded reads of Server-owned startup facts, shared by Get and prompt.
use super::{
    AgentReadOperation, AgentSafeInteger, AgentSessionId, BorrowedFd, Failure, FailureKind,
    Instant, LocalClient, agent_protocol_invalid, agent_rejection_details, agent_session_details,
    agent_timeout_details, agent_unavailable_details, classify_agent_read, obap_request_id,
};

#[cfg(windows)]
pub(in crate::cli) fn read_terminal_management(
    client: &mut LocalClient,
    binding: &orchbee_agent_observation_protocol::AgentLaunchBinding,
    deadline: Instant,
    wake: Option<BorrowedFd<'_>>,
) -> Result<crate::cli::results::TerminalManagementDoc, Failure> {
    use orchbee_agent_observation_protocol::{
        AgentLaunchOutcome, AgentLaunchRejectionReason, AgentLaunchRequest,
    };
    let operation = AgentReadOperation::Management;
    let request_id = obap_request_id()?;
    let result = client
        .submit_agent_launch_before_cancellable(
            AgentLaunchRequest::TerminalStatus {
                request_id: request_id.clone(),
                binding: binding.clone(),
            },
            deadline,
            wake,
        )
        .map_err(|error| classify_agent_read(error, operation))?;
    if Instant::now() >= deadline {
        return Err(Failure::new(FailureKind::Timeout));
    }
    match result.outcome() {
        AgentLaunchOutcome::TerminalStatus {
            binding: confirmed,
            management,
        } if confirmed == binding => Ok(crate::cli::results::TerminalManagementDoc {
            source: crate::cli::results::agent_source(&request_id),
            state: management.clone(),
        }),
        AgentLaunchOutcome::Rejected {
            reason: AgentLaunchRejectionReason::ReadDenied,
        } => Err(Failure::new(FailureKind::Denied)),
        AgentLaunchOutcome::Rejected {
            reason: AgentLaunchRejectionReason::BindingChanged,
        }
        | AgentLaunchOutcome::Unavailable { .. } => {
            Err(Failure::new(FailureKind::ServiceUnavailable))
        }
        _ => Err(Failure::new(FailureKind::ProtocolInvalid)),
    }
}

/// Startup ownership is a separate read, with its own sampled revision and source.
#[cfg(windows)]
pub(in crate::cli) fn read_agent_management(
    client: &mut LocalClient,
    session_id: &AgentSessionId,
    minimum_revision: AgentSafeInteger,
    deadline: Instant,
    wake: Option<BorrowedFd<'_>>,
) -> Result<crate::cli::results::AgentManagementDoc, Failure> {
    use orchbee_agent_observation_protocol::{
        AgentLaunchOutcome, AgentLaunchRejectionReason, AgentLaunchRequest,
    };
    let mut read = || {
        let operation = AgentReadOperation::Management;
        let request_id = obap_request_id()?;
        let result = client
            .submit_agent_launch_before_cancellable(
                AgentLaunchRequest::Status {
                    request_id: request_id.clone(),
                    session_id: session_id.clone(),
                },
                deadline,
                wake,
            )
            .map_err(|error| classify_agent_read(error, operation))?;
        if Instant::now() >= deadline {
            return Err(
                Failure::new(FailureKind::Timeout).details(agent_timeout_details(operation))
            );
        }
        let source = crate::cli::results::agent_source(&request_id);
        match result.outcome() {
            AgentLaunchOutcome::Status {
                session_id: confirmed,
                revision,
                management,
            } if confirmed == session_id && revision.value() >= minimum_revision.value() => {
                Ok(crate::cli::results::AgentManagementDoc {
                    source,
                    revision: *revision,
                    management: management.clone(),
                })
            }
            AgentLaunchOutcome::Rejected {
                reason: AgentLaunchRejectionReason::SessionNotFound,
            } => Err(Failure::new(FailureKind::NotFound)
                .details(agent_session_details(operation, session_id.as_str()))
                .source(source)),
            AgentLaunchOutcome::Rejected {
                reason: AgentLaunchRejectionReason::ReadDenied,
            } => Err(Failure::new(FailureKind::Denied)
                .details(agent_rejection_details(operation, "read-denied"))
                .source(source)),
            AgentLaunchOutcome::Unavailable { .. } => {
                Err(Failure::new(FailureKind::ServiceUnavailable)
                    .details(agent_unavailable_details(operation))
                    .source(source))
            }
            _ => Err(agent_protocol_invalid(operation)),
        }
    };
    read().map_err(|failure| failure.failure_schema(crate::cli::schema::AGENT_FAILURE_ID))
}
