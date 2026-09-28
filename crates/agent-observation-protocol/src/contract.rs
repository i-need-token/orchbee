use crate::changes::{
    AgentChangesOutcome, AgentChangesRequest, AgentChangesResult, MAXIMUM_CHANGE_ITEMS,
};
use std::collections::HashSet;
use std::fmt;

use crate::identity::AgentHostEpoch;
use crate::launch::{
    AgentLaunchOutcome, AgentLaunchRequest, AgentLaunchReservation, AgentLaunchResult,
    LAUNCH_SETTLE_MS, MAXIMUM_LAUNCH_TTL_MS,
};

use crate::model::{
    AgentCapability, AgentLifecycle, AgentObservationSource, AgentSessionDetail,
    AgentSessionExplanation, AgentSessionSnapshot, AgentSessionSummary, AgentTurnState,
};
use crate::names::{
    AgentNameAssignOutcome, AgentNameAssignRejectionReason, AgentNameTarget, AgentNamedTarget,
    AgentNamesListOutcome, AgentNamesRequest, AgentNamesResult, MAXIMUM_NAME_BINDINGS,
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
    } else if let Ok(request) = serde_json::from_slice::<AgentChangesRequest>(encoded) {
        AgentRequest::Changes(request)
    } else if let Ok(request) = serde_json::from_slice::<AgentNamesRequest>(encoded) {
        AgentRequest::Names(request)
    } else {
        AgentRequest::Launch(
            serde_json::from_slice::<AgentLaunchRequest>(encoded)
                .map_err(|_error| AgentContractError::InvalidDocument)?,
        )
    };
    validate_request(&request)?;
    Ok(request)
}

pub fn parse_agent_result(encoded: &[u8]) -> Result<AgentResult, AgentContractError> {
    let result = if let Ok(result) = serde_json::from_slice::<AgentReadResult>(encoded) {
        AgentResult::Read(result)
    } else if let Ok(result) = serde_json::from_slice::<AgentChangesResult>(encoded) {
        AgentResult::Changes(result)
    } else if let Ok(result) = serde_json::from_slice::<AgentNamesResult>(encoded) {
        AgentResult::Names(result)
    } else {
        AgentResult::Launch(
            serde_json::from_slice::<AgentLaunchResult>(encoded)
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
        AgentRequest::Launch(request) => serde_json::to_vec(request),
        AgentRequest::Changes(request) => serde_json::to_vec(request),
    };
    encoded.map_err(|_error| AgentContractError::InvalidDocument)
}

pub fn serialize_agent_result(result: &AgentResult) -> Result<Vec<u8>, AgentContractError> {
    validate_result(result)?;
    let encoded = match result {
        AgentResult::Read(result) => serde_json::to_vec(result),
        AgentResult::Names(result) => serde_json::to_vec(result),
        AgentResult::Launch(result) => serde_json::to_vec(result),
        AgentResult::Changes(result) => serde_json::to_vec(result),
    };
    encoded.map_err(|_error| AgentContractError::InvalidDocument)
}

fn validate_request(request: &AgentRequest) -> Result<(), AgentContractError> {
    match request {
        AgentRequest::Read(AgentReadRequest::List { limit, .. })
        | AgentRequest::Changes(AgentChangesRequest::Read { limit, .. }) => {
            if !(1..=MAXIMUM_LIST_LIMIT).contains(&limit.value()) {
                return Err(AgentContractError::InvalidDocument);
            }
        }
        AgentRequest::Launch(AgentLaunchRequest::Reserve { ttl_ms, .. }) => {
            if !(1..=MAXIMUM_LAUNCH_TTL_MS).contains(&ttl_ms.value()) {
                return Err(AgentContractError::InvalidDocument);
            }
        }
        AgentRequest::Launch(AgentLaunchRequest::TerminalStatus { binding, .. }) => {
            AgentHostEpoch::parse(binding.generation.as_str())
                .map_err(|_| AgentContractError::InvalidDocument)?;
        }
        AgentRequest::Launch(_) => {}
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
        AgentResult::Launch(result) => validate_launch_result(result),
        AgentResult::Changes(result) => validate_changes_result(result),
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
            if let AgentNamesListOutcome::ListedTargets { bindings, pending } = &result.outcome {
                validate_named_targets(bindings, pending)?;
            }
            if let AgentNamesListOutcome::Listed { bindings, pending } = &result.outcome {
                if bindings.len() + pending.len() > MAXIMUM_NAME_BINDINGS {
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
                let mut names: HashSet<_> = bindings.iter().map(|b| b.name.as_str()).collect();
                let mut presentations = HashSet::new();
                let mut reservations = HashSet::new();
                if pending.windows(2).any(|pair| pair[0].name >= pair[1].name) {
                    return Err(AgentContractError::InvalidDocument);
                }
                for entry in pending {
                    validate_reservation(entry)?;
                    if !names.insert(entry.name.as_str())
                        || !presentations.insert(entry.binding.presentation_id.as_str())
                        || !reservations.insert(entry.reservation_id.as_str())
                    {
                        return Err(AgentContractError::InvalidDocument);
                    }
                }
                let mut sessions = HashSet::with_capacity(bindings.len());
                if bindings.windows(2).any(|pair| pair[0].name >= pair[1].name)
                    || bindings
                        .iter()
                        .any(|binding| !sessions.insert(binding.session_id.as_str()))
                {
                    return Err(AgentContractError::InvalidDocument);
                }
            }
        }
        AgentNamesResult::NameAssign { result, .. } => {
            if let AgentNameAssignOutcome::RejectedTarget {
                reason,
                conflict_target,
            } = &result.outcome
                && (conflict_target.is_none()
                    || *reason != AgentNameAssignRejectionReason::NameInUse)
            {
                return Err(AgentContractError::InvalidDocument);
            }
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
        AgentNamesResult::Resolve { result, .. } => {
            if matches!(
                &result.outcome,
                crate::AgentResolveOutcome::ResolvedTarget {
                    target: AgentNameTarget::Terminal { .. },
                    resolved_by: crate::AgentResolvedBy::SessionId
                }
            ) {
                return Err(AgentContractError::InvalidDocument);
            }
        }
        AgentNamesResult::NameRelease { .. } => {}
    }
    Ok(())
}

fn validate_named_targets(
    bindings: &[AgentNamedTarget],
    pending: &[AgentLaunchReservation],
) -> Result<(), AgentContractError> {
    if bindings.len() + pending.len() > MAXIMUM_NAME_BINDINGS
        || bindings.windows(2).any(|pair| pair[0].name >= pair[1].name)
        || pending.windows(2).any(|pair| pair[0].name >= pair[1].name)
    {
        return Err(AgentContractError::InvalidDocument);
    }
    let mut names = HashSet::new();
    let mut sessions = HashSet::new();
    let mut presentations = HashSet::new();
    let mut reservations = HashSet::new();
    for entry in bindings {
        if !names.insert(entry.name.as_str()) {
            return Err(AgentContractError::InvalidDocument);
        }
        let unique = match &entry.target {
            AgentNameTarget::Session { session_id } => sessions.insert(session_id.as_str()),
            AgentNameTarget::Terminal { binding } => {
                presentations.insert(binding.presentation_id.as_str())
            }
        };
        if !unique {
            return Err(AgentContractError::InvalidDocument);
        }
    }
    for entry in pending {
        validate_reservation(entry)?;
        if !names.insert(entry.name.as_str())
            || !presentations.insert(entry.binding.presentation_id.as_str())
            || !reservations.insert(entry.reservation_id.as_str())
        {
            return Err(AgentContractError::InvalidDocument);
        }
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

fn validate_reservation(value: &AgentLaunchReservation) -> Result<(), AgentContractError> {
    if !(1..=MAXIMUM_LAUNCH_TTL_MS).contains(&value.remaining_ms.value())
        || value.settle_remaining_ms.value() > LAUNCH_SETTLE_MS
        || AgentHostEpoch::parse(value.binding.generation.as_str()).is_err()
    {
        return Err(AgentContractError::InvalidDocument);
    }
    Ok(())
}

fn validate_launch_result(result: &AgentLaunchResult) -> Result<(), AgentContractError> {
    match result.outcome() {
        AgentLaunchOutcome::TerminalStatus { binding, .. } => {
            if !matches!(result, AgentLaunchResult::TerminalStatus { .. }) {
                return Err(AgentContractError::InvalidDocument);
            }
            AgentHostEpoch::parse(binding.generation.as_str())
                .map(|_| ())
                .map_err(|_| AgentContractError::InvalidDocument)
        }
        AgentLaunchOutcome::Status { management, .. } => {
            if !matches!(result, AgentLaunchResult::Status { .. }) {
                return Err(AgentContractError::InvalidDocument);
            }
            match management {
                crate::AgentManagement::Unmanaged {} => Ok(()),
                crate::AgentManagement::Pending { reservation, .. } => {
                    validate_reservation(reservation)
                }
                crate::AgentManagement::Active { binding, .. } => {
                    AgentHostEpoch::parse(binding.generation.as_str())
                        .map(|_| ())
                        .map_err(|_| AgentContractError::InvalidDocument)
                }
            }
        }
        AgentLaunchOutcome::Reserved { reservation } => {
            if !matches!(result, AgentLaunchResult::Reserve { .. })
                || &reservation.reservation_id != result.request_id()
            {
                return Err(AgentContractError::InvalidDocument);
            }
            validate_reservation(reservation)
        }
        AgentLaunchOutcome::Pending { reservation, .. } => {
            if !matches!(result, AgentLaunchResult::Inspect { .. }) {
                return Err(AgentContractError::InvalidDocument);
            }
            validate_reservation(reservation)
        }
        AgentLaunchOutcome::Completed { .. }
            if matches!(result, AgentLaunchResult::Complete { .. }) =>
        {
            Ok(())
        }
        AgentLaunchOutcome::Released { .. }
            if matches!(result, AgentLaunchResult::Cancel { .. }) =>
        {
            Ok(())
        }
        AgentLaunchOutcome::Rejected { .. } | AgentLaunchOutcome::Unavailable { .. } => Ok(()),
        _ => Err(AgentContractError::InvalidDocument),
    }
}

fn validate_changes_result(result: &AgentChangesResult) -> Result<(), AgentContractError> {
    let AgentChangesResult::Read { result, .. } = result;
    let AgentChangesOutcome::Changes {
        changes,
        observed_through_revision,
        runtime_revision,
        has_more,
        ..
    } = &result.outcome
    else {
        return Ok(());
    };
    if changes.is_empty()
        || changes.len() > MAXIMUM_CHANGE_ITEMS
        || observed_through_revision.value() > runtime_revision.value()
        || *has_more != (observed_through_revision.value() < runtime_revision.value())
        || changes
            .last()
            .is_none_or(|change| change.revision != *observed_through_revision)
        || changes.iter().any(|change| {
            change.revision.value() == 0
                || change.session.pending_request_count.value() > MAXIMUM_PENDING_REQUESTS as u64
        })
        || changes
            .windows(2)
            .any(|pair| pair[0].revision.value() + 1 != pair[1].revision.value())
    {
        return Err(AgentContractError::InvalidDocument);
    }
    Ok(())
}
