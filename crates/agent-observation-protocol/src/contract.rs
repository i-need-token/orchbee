use std::collections::HashSet;
use std::fmt;

use crate::model::{
    AgentCapability, AgentLifecycle, AgentObservationSource, AgentSessionDetail,
    AgentSessionExplanation, AgentSessionSnapshot, AgentSessionSummary, AgentTurnState,
};
use crate::names::{
    AgentNameAssignOutcome, AgentNameAssignRejectionReason, AgentNamesRequest, AgentNamesResult,
    AgentNamesListOutcome, MAXIMUM_NAME_BINDINGS,
};
use crate::read::{
    AgentExplainOutcome, AgentGetOutcome, AgentListOutcome, AgentReadRequest, AgentReadResult,
    AgentRequest, AgentResult, MAXIMUM_CAPABILITIES, MAXIMUM_LIST_ITEMS, MAXIMUM_LIST_LIMIT,
    MAXIMUM_PENDING_REQUESTS, MAXIMUM_PRESENTATION_ATTACHMENTS,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentContractError {
    EncodedValueTooLarge { actual: usize, maximum: usize },
    InvalidDocument,
    WrongDocumentKind,
}

impl fmt::Display for AgentContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EncodedValueTooLarge { actual, maximum } => {
                write!(
                    formatter,
                    "encoded value uses {actual} bytes; maximum is {maximum}"
                )
            }
            Self::InvalidDocument => {
                formatter.write_str("invalid Agent observation contract document")
            }
            Self::WrongDocumentKind => {
                formatter.write_str("unexpected Agent observation document kind")
            }
        }
    }
}

impl std::error::Error for AgentContractError {}

/// Parses one request payload into the closed union. The read and names
/// families carry disjoint `kind` values, so the family is decided by which
/// typed decode succeeds.
pub fn parse_agent_request(encoded: &[u8]) -> Result<AgentRequest, AgentContractError> {
    let request = if let Ok(request) = serde_json::from_slice::<AgentReadRequest>(encoded) {
        AgentRequest::Read(request)
    } else {
        AgentRequest::Names(
            serde_json::from_slice::<AgentNamesRequest>(encoded)
                .map_err(|_error| AgentContractError::InvalidDocument)?,
        )
    };
    validate_request(&request)?;
    Ok(request)
}

pub fn parse_agent_result(encoded: &[u8]) -> Result<AgentResult, AgentContractError> {
    let result = if let Ok(result) = serde_json::from_slice::<AgentReadResult>(encoded) {
        AgentResult::Read(result)
    } else {
        AgentResult::Names(
            serde_json::from_slice::<AgentNamesResult>(encoded)
                .map_err(|_error| AgentContractError::InvalidDocument)?,
        )
    };
    validate_result(&result)?;
    Ok(result)
}

pub fn serialize_agent_request(request: &AgentRequest) -> Result<Vec<u8>, AgentContractError> {
    validate_request(request)?;
    let encoded = match request {
        AgentRequest::Read(request) => serde_json::to_vec(request),
        AgentRequest::Names(request) => serde_json::to_vec(request),
    };
    encoded.map_err(|_error| AgentContractError::InvalidDocument)
}

pub fn serialize_agent_result(result: &AgentResult) -> Result<Vec<u8>, AgentContractError> {
    validate_result(result)?;
    let encoded = match result {
        AgentResult::Read(result) => serde_json::to_vec(result),
        AgentResult::Names(result) => serde_json::to_vec(result),
    };
    encoded.map_err(|_error| AgentContractError::InvalidDocument)
}

fn validate_request(request: &AgentRequest) -> Result<(), AgentContractError> {
    match request {
        AgentRequest::Read(AgentReadRequest::List { limit, .. }) => {
            if !(1..=MAXIMUM_LIST_LIMIT).contains(&limit.value()) {
                return Err(AgentContractError::InvalidDocument);
            }
        }
        // Selectors and names are already typed at construction; no further
        // request invariant exists beyond the schema.
        AgentRequest::Read(_) | AgentRequest::Names(_) => {}
    }
    Ok(())
}

fn validate_result(result: &AgentResult) -> Result<(), AgentContractError> {
    match result {
        AgentResult::Read(result) => validate_read_result(result),
        AgentResult::Names(result) => validate_names_result(result),
    }
}

fn validate_read_result(result: &AgentReadResult) -> Result<(), AgentContractError> {
    match result {
        AgentReadResult::List { result, .. } => match &result.outcome {
            AgentListOutcome::Listed { items, .. } => {
                if items.len() > MAXIMUM_LIST_ITEMS {
                    return Err(AgentContractError::InvalidDocument);
                }
                for summary in items {
                    validate_summary(summary)?;
                }
            }
            AgentListOutcome::Rejected { .. } | AgentListOutcome::Unavailable { .. } => {}
        },
        AgentReadResult::Get { result, .. } => match &result.outcome {
            AgentGetOutcome::Found { session, .. } => validate_detail(session)?,
            AgentGetOutcome::Rejected { .. } | AgentGetOutcome::Unavailable { .. } => {}
        },
        AgentReadResult::Explain { result, .. } => match &result.outcome {
            AgentExplainOutcome::Found {
                session,
                explanation,
                ..
            } => {
                validate_detail(session)?;
                validate_explanation(explanation)?;
            }
            AgentExplainOutcome::Rejected { .. } | AgentExplainOutcome::Unavailable { .. } => {}
        },
    }
    Ok(())
}

fn validate_names_result(result: &AgentNamesResult) -> Result<(), AgentContractError> {
    match result {
        AgentNamesResult::NamesList { result, .. } => {
            if let AgentNamesListOutcome::Listed { bindings } = &result.outcome {
                if bindings.len() > MAXIMUM_NAME_BINDINGS {
                    return Err(AgentContractError::InvalidDocument);
                }
                // Only live sessions hold bindings: an emitted ended row is
                // outside the fold contract, never silently tolerated.
                if bindings
                    .iter()
                    .any(|binding| binding.lifecycle != AgentLifecycle::Live)
                {
                    return Err(AgentContractError::InvalidDocument);
                }
            }
        }
        AgentNamesResult::NameAssign { result, .. } => {
            if let AgentNameAssignOutcome::Rejected {
                reason,
                conflict_session_id,
            } = &result.outcome
            {
                let conflicted = *reason == AgentNameAssignRejectionReason::NameInUse;
                if conflict_session_id.is_some() != conflicted {
                    return Err(AgentContractError::InvalidDocument);
                }
            }
        }
        AgentNamesResult::Resolve { .. } | AgentNamesResult::NameRelease { .. } => {}
    }
    Ok(())
}

/// The canonical semantic invariants the schema alone cannot express —
/// enforced identically by every binding, never claimed as a canonical
/// superRefinement proof.
fn validate_summary(summary: &AgentSessionSummary) -> Result<(), AgentContractError> {
    if summary.presentation_attachments.len() > MAXIMUM_PRESENTATION_ATTACHMENTS
        || summary.pending_request_count.value() > MAXIMUM_PENDING_REQUESTS as u64
    {
        return Err(AgentContractError::InvalidDocument);
    }
    validate_attachments(&summary.presentation_attachments)
}

fn validate_detail(detail: &AgentSessionDetail) -> Result<(), AgentContractError> {
    validate_snapshot(&detail.snapshot)
}

fn validate_explanation(explanation: &AgentSessionExplanation) -> Result<(), AgentContractError> {
    validate_capabilities(&explanation.capability_ceiling)
}

fn validate_snapshot(snapshot: &AgentSessionSnapshot) -> Result<(), AgentContractError> {
    if snapshot.pending_requests.len() > MAXIMUM_PENDING_REQUESTS
        || snapshot.presentation_attachments.len() > MAXIMUM_PRESENTATION_ATTACHMENTS
    {
        return Err(AgentContractError::InvalidDocument);
    }
    validate_capabilities(&snapshot.capabilities)?;
    validate_attachments(&snapshot.presentation_attachments)?;
    if snapshot.activity_hint.is_some()
        && (!snapshot.capabilities.contains(&AgentCapability::Activity)
            || snapshot.observation.source != AgentObservationSource::Terminal)
    {
        return Err(AgentContractError::InvalidDocument);
    }
    let mut request_ids = HashSet::with_capacity(snapshot.pending_requests.len());
    for request in &snapshot.pending_requests {
        if !request_ids.insert(request.request_id.as_str()) {
            return Err(AgentContractError::InvalidDocument);
        }
        if let Some(turn_id) = &request.turn_id {
            match &snapshot.turn {
                AgentTurnState::Active {
                    turn_id: active, ..
                } if active == turn_id => {}
                _ => return Err(AgentContractError::InvalidDocument),
            }
        }
    }
    if snapshot.lifecycle == crate::model::AgentLifecycle::Ended
        && (snapshot.activity_hint.is_some()
            || matches!(snapshot.turn, AgentTurnState::Active { .. })
            || !snapshot.pending_requests.is_empty())
    {
        return Err(AgentContractError::InvalidDocument);
    }
    Ok(())
}

fn validate_capabilities(capabilities: &[AgentCapability]) -> Result<(), AgentContractError> {
    if capabilities.len() > MAXIMUM_CAPABILITIES {
        return Err(AgentContractError::InvalidDocument);
    }
    let mut seen = HashSet::with_capacity(capabilities.len());
    for capability in capabilities {
        if !seen.insert(*capability) {
            return Err(AgentContractError::InvalidDocument);
        }
    }
    Ok(())
}

fn validate_attachments(
    attachments: &[crate::model::AgentPresentationAttachment],
) -> Result<(), AgentContractError> {
    let mut keys = HashSet::with_capacity(attachments.len());
    let mut navigable = HashSet::new();
    for attachment in attachments {
        if !keys.insert((
            attachment.presentation_id.as_str(),
            attachment.generation.as_str(),
        )) {
            return Err(AgentContractError::InvalidDocument);
        }
        if attachment.navigable && !navigable.insert(attachment.presentation_id.as_str()) {
            return Err(AgentContractError::InvalidDocument);
        }
    }
    Ok(())
}
