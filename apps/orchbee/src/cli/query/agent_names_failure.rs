//! Classifies Agent-name rejections and transport failures without owning signal capture or
//! connection admission.
use super::{
    AgentExchangeError, AgentExchangeFailure, Failure, FailureKind, is_deadline_elapsed,
    wake_marker_failure,
};
use crate::invocation::grammar::schemas::AGENT_NAMES_FAILURE_ID as FAILURE_SCHEMA;
use serde_json::{Value, json};

/// An Agent-name rejection token classified into the shared failure kinds, with the conflicting
/// session when the Server reported one.
pub(in crate::cli) fn rejection(
    reason: Value,
    conflict: Option<&orchbee_agent_observation_protocol::AgentSessionId>,
) -> Failure {
    let kind = match reason.as_str() {
        Some("ambiguous-target") => FailureKind::Ambiguous,
        Some("read-denied" | "manage-denied") => FailureKind::Denied,
        Some("not-found" | "name-not-found" | "session-not-found") => FailureKind::NotFound,
        _ => FailureKind::Rejected,
    };
    let mut details = serde_json::Map::from_iter([
        ("stage".to_owned(), json!("agent-names")),
        ("reason".to_owned(), reason),
    ]);
    if let Some(id) = conflict {
        details.insert(
            "conflictTarget".to_owned(),
            json!({"kind":"session","sessionId":id}),
        );
    }
    Failure::new(kind).details(details)
}

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
