use std::collections::HashSet;
use std::fmt;

use crate::model::{
    AgentCapability, AgentObservationSource, AgentSessionDetail, AgentSessionExplanation,
    AgentSessionSnapshot, AgentSessionSummary, AgentTurnState,
};
use crate::read::{
    AgentExplainOutcome, AgentGetOutcome, AgentListOutcome, AgentReadRequest, AgentReadResult,
    MAXIMUM_CAPABILITIES, MAXIMUM_LIST_ITEMS, MAXIMUM_LIST_LIMIT, MAXIMUM_PENDING_REQUESTS,
    MAXIMUM_PRESENTATION_ATTACHMENTS,
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

pub fn parse_agent_request(encoded: &[u8]) -> Result<AgentReadRequest, AgentContractError> {
    let request = serde_json::from_slice::<AgentReadRequest>(encoded)
        .map_err(|_error| AgentContractError::InvalidDocument)?;
    validate_request(&request)?;
    Ok(request)
}

pub fn parse_agent_result(encoded: &[u8]) -> Result<AgentReadResult, AgentContractError> {
    let result = serde_json::from_slice::<AgentReadResult>(encoded)
        .map_err(|_error| AgentContractError::InvalidDocument)?;
    validate_result(&result)?;
    Ok(result)
}

pub fn serialize_agent_request(request: &AgentReadRequest) -> Result<Vec<u8>, AgentContractError> {
    validate_request(request)?;
    serde_json::to_vec(request).map_err(|_error| AgentContractError::InvalidDocument)
}

pub fn serialize_agent_result(result: &AgentReadResult) -> Result<Vec<u8>, AgentContractError> {
    validate_result(result)?;
    serde_json::to_vec(result).map_err(|_error| AgentContractError::InvalidDocument)
}

fn validate_request(request: &AgentReadRequest) -> Result<(), AgentContractError> {
    if let AgentReadRequest::List { limit, .. } = request
        && !(1..=MAXIMUM_LIST_LIMIT).contains(&limit.value())
    {
        return Err(AgentContractError::InvalidDocument);
    }
    Ok(())
}

fn validate_result(result: &AgentReadResult) -> Result<(), AgentContractError> {
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
