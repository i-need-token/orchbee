//! Bounded canonical list without signal registration; admission selects the existing host route.
use std::time::Instant;

use orchbee_agent_observation_protocol::{
    AgentListCursor, AgentReadRequest, AgentReadResult, AgentSafeInteger,
};
use orchbee_local_runtime::Cohort;

use crate::{
    cli::{Failure, FailureKind, query::AgentQueryError, results::Projection},
    local_client::AgentReadOperation,
    local_client::LocalClient,
};

pub(super) fn run(
    include_ended: bool,
    limit: u64,
    cursor: Option<AgentListCursor>,
) -> Result<(serde_json::Value, Projection), AgentQueryError> {
    let total = Instant::now()
        .checked_add(super::AGENT_TOTAL_BUDGET)
        .ok_or_else(|| AgentQueryError::Admission(Failure::new(FailureKind::InternalFailure)))?;
    let request_id = super::obap_request_id().map_err(AgentQueryError::Admission)?;

    let cohort_deadline = Instant::now()
        .checked_add(super::COHORT_BUDGET)
        .ok_or_else(|| AgentQueryError::Admission(Failure::new(FailureKind::InternalFailure)))?
        .min(total);
    let cohort = Cohort::current(cohort_deadline).map_err(|_| {
        AgentQueryError::Admission(
            Failure::new(FailureKind::CheckFailed).details(super::stage_details("cohort")),
        )
    })?;

    let session = super::rendezvous().map_err(AgentQueryError::Admission)?;
    match session.client_socket_exists() {
        Ok(true) => {}
        Ok(false) => {
            return Err(AgentQueryError::Admission(
                Failure::new(FailureKind::ServiceUnavailable)
                    .details(super::stage_details("discovery")),
            ));
        }
        Err(_) => {
            return Err(AgentQueryError::Admission(
                Failure::new(FailureKind::CheckFailed).details(super::stage_details("socket")),
            ));
        }
    }

    let mut client =
        LocalClient::connect_before(session.client_socket_path(), cohort.identity(), total)
            .map_err(|error| AgentQueryError::Admission(super::classify_connect(error)))?;
    if Instant::now() >= total {
        drop(client);
        return Err(AgentQueryError::Read(timeout_failure()));
    }

    let request = AgentReadRequest::List {
        request_id: request_id.clone(),
        include_ended,
        limit: AgentSafeInteger::new(limit),
        cursor,
    };
    let exchange = client.submit_agent_read_before_cancellable(request, total, None);
    drop(client);
    if Instant::now() >= total {
        return Err(AgentQueryError::Read(timeout_failure()));
    }

    let AgentReadResult::List { result, .. } = exchange.map_err(|error| {
        AgentQueryError::Read(super::classify_agent_read(error, AgentReadOperation::List))
    })?
    else {
        return Err(AgentQueryError::Read(super::agent_protocol_invalid(
            AgentReadOperation::List,
        )));
    };
    let outcome =
        super::listed_outcome(result.outcome, &request_id).map_err(AgentQueryError::Read)?;
    Ok((
        crate::cli::results::agent_source(&request_id),
        Projection::AgentList(outcome),
    ))
}

fn timeout_failure() -> Failure {
    Failure::new(FailureKind::Timeout)
        .details(super::agent_timeout_details(AgentReadOperation::List))
}
