//! Correlated OBAP observation on the same admitted Client used by startup.
use crate::agent_runtime::AgentObservationRead;
use crate::local_client::{LocalClient, agent::AgentExchangeError};
use orchbee_agent_observation_protocol::{
    AgentGetOutcome, AgentReadRequest, AgentReadResult, AgentSessionId,
    AgentSessionRejectionReason, ObapRequestId,
};
use orchbee_local_ipc::LocalIo;
use std::time::Instant;

/// Preserve exchange evidence, response correlation failures and explicit Server read refusals.
#[derive(Debug)]
pub enum Error {
    Identity,
    Exchange(AgentExchangeError),
    Timeout,
    Protocol,
    Rejected {
        request: ObapRequestId,
        session: AgentSessionId,
        reason: AgentSessionRejectionReason,
    },
    Unavailable {
        request: ObapRequestId,
    },
}

/// Read one Agent session using the admitted Client and the caller's absolute budget.
/// A successful observation matches the requested session and includes its request and revision.
///
/// # Errors
/// Distinguishes identity or exchange failure, an expired post-read deadline, malformed
/// response correlation, and the Server's rejected or unavailable outcome. It never retries.
pub fn read(
    client: &mut LocalClient,
    session: &AgentSessionId,
    deadline: Instant,
    wake: Option<LocalIo<'_>>,
) -> Result<AgentObservationRead, Error> {
    let request_id = super::obap_identity().map_err(|_| Error::Identity)?;
    let result = client
        .submit_agent_read_before_cancellable(
            AgentReadRequest::Get {
                request_id: request_id.clone(),
                session_id: session.clone(),
            },
            deadline,
            wake,
        )
        .map_err(Error::Exchange)?;
    if Instant::now() >= deadline {
        return Err(Error::Timeout);
    }
    let AgentReadResult::Get { result, .. } = result else {
        return Err(Error::Protocol);
    };
    match result.outcome {
        AgentGetOutcome::Found {
            session: observed,
            revision,
        } => {
            if observed.snapshot.session_id != *session {
                return Err(Error::Protocol);
            }
            Ok(AgentObservationRead {
                request_id,
                revision,
                session: observed,
            })
        }
        AgentGetOutcome::Rejected { reason } => Err(Error::Rejected {
            request: request_id,
            session: session.clone(),
            reason,
        }),
        AgentGetOutcome::Unavailable { .. } => Err(Error::Unavailable {
            request: request_id,
        }),
    }
}
