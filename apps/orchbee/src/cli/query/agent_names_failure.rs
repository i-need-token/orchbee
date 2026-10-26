//! Classifies Agent-name transport failures without owning signal capture or connection admission.
use super::{
    AgentExchangeError, AgentExchangeFailure, Failure, FailureKind, is_deadline_elapsed,
    wake_marker_failure,
};
use crate::cli::agent_names::FAILURE_SCHEMA;
pub(super) fn exchange_failure(error: AgentExchangeError) -> Failure {
    let kind = match error {
        AgentExchangeError::ConnectionUnavailable | AgentExchangeError::RequestEncoding(_) => {
            FailureKind::InternalFailure
        }
        AgentExchangeError::RequestNotSubmitted(error) => io_failure_kind(&error),
        AgentExchangeError::Failed { reason, .. } => match reason {
            AgentExchangeFailure::Write(error)
            | AgentExchangeFailure::ResponseDeadline(error)
            | AgentExchangeFailure::Response(
                orchbee_agent_observation_protocol::AgentWireError::Io(error),
            ) => io_failure_kind(&error),
            // Only the explicitly recognized protocol failures have a more specific transport classification here.
            _ => FailureKind::ProtocolInvalid,
        },
    };
    Failure::new(kind).failure_schema(FAILURE_SCHEMA)
}

fn io_failure_kind(error: &std::io::Error) -> FailureKind {
    if wake_marker_failure(error).is_some() {
        FailureKind::InternalFailure
    } else if is_deadline_elapsed(error) {
        FailureKind::Timeout
    } else {
        FailureKind::TransportLoss
    }
}
