//! Bounded runtime change journal and its paging read.
//!
//! `read_agent_runtime_changes` mirrors the TypeScript arithmetic exactly;
//! malformed private input returns a typed `AgentChangeReadError` — the Rust
//! mapping of the `ZodError` the TypeScript parse throws — never a transition
//! result.

use super::AgentObservationEvent;
use super::AgentSemanticStatus;
use super::AgentSessionSnapshot;
use super::model::valid_ordered_identity;
use super::project_agent_session;
use super::{AgentLifecycle, AgentObservation, AgentProviderId, AgentRuntimeSnapshot, OpaqueId};

pub const AGENT_RUNTIME_CHANGE_RETENTION: usize = 512;
pub const MAX_AGENT_RUNTIME_CHANGE_PAGE_SIZE: u64 = 100;

// The journal payload is shared by the canonical corpus and native change reads.
#[derive(Clone, Debug)]
pub struct AgentRuntimeChangeSession {
    pub lifecycle: AgentLifecycle,
    pub observation: AgentObservation,
    pub parent_session_id: Option<OpaqueId>,
    pub pending_request_count: u64,
    pub provider_id: AgentProviderId,
    pub session_id: OpaqueId,
    pub status: AgentSemanticStatus,
}

#[derive(Clone, Debug)]
pub struct AgentRuntimeChange {
    pub revision: u64,
    pub session: AgentRuntimeChangeSession,
    /// `AgentObservationTransitionKind` — one shared observation event type.
    pub transition: &'static str,
}

/// Raw read input: `f64` fields keep the invalid-input path reachable exactly
/// like the TypeScript `number` schema check.
#[derive(Clone, Copy, Debug)]
pub struct AgentChangeReadInput {
    pub after_revision: f64,
    pub limit: f64,
}

#[derive(Clone, Debug)]
pub enum AgentRuntimeChangeReadResult {
    Changes {
        changes: Vec<AgentRuntimeChange>,
        has_more: bool,
        observed_through_revision: u64,
        runtime_revision: u64,
    },
    Current {
        observed_through_revision: u64,
        runtime_revision: u64,
    },
    ResetRequired {
        observed_through_revision: u64,
        runtime_revision: u64,
    },
    RevisionAhead {
        observed_through_revision: u64,
        runtime_revision: u64,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct AgentChangeReadError(pub &'static str);

impl std::fmt::Display for AgentChangeReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for AgentChangeReadError {}

pub fn create_agent_runtime_change(
    snapshot: &AgentRuntimeSnapshot,
    event: &AgentObservationEvent,
) -> AgentRuntimeChange {
    let Some(session_id) = event_session_id(event) else {
        panic!("An applied Agent transition must carry a Session identity.");
    };
    let Some(session) = snapshot
        .sessions
        .iter()
        .find(|candidate| candidate.session_id.as_str() == session_id)
    else {
        panic!("An applied Agent transition must retain its target Session.");
    };
    AgentRuntimeChange {
        revision: snapshot.revision,
        session: project_change_session(session),
        transition: event.transition_kind(),
    }
}

fn event_session_id(event: &AgentObservationEvent) -> Option<&str> {
    match event {
        AgentObservationEvent::SessionObserved { session_id, .. } => Some(session_id),
        _ => event
            .session_event_base()
            .map(|(_, _, session_id)| session_id),
    }
}

/// Append while enforcing revision contiguity and the retention bound; the
/// TypeScript throws on a non-contiguous append, so this panics identically.
pub fn append_agent_runtime_change(
    changes: &mut Vec<AgentRuntimeChange>,
    change: AgentRuntimeChange,
) {
    if let Some(previous) = changes.last()
        && change.revision != previous.revision + 1
    {
        panic!("Agent runtime change revisions must remain contiguous.");
    }
    changes.push(change);
    if changes.len() > AGENT_RUNTIME_CHANGE_RETENTION {
        let overflow = changes.len() - AGENT_RUNTIME_CHANGE_RETENTION;
        changes.drain(0..overflow);
    }
}

pub fn read_agent_runtime_changes(
    runtime_revision: u64,
    changes: &[AgentRuntimeChange],
    input: AgentChangeReadInput,
) -> Result<AgentRuntimeChangeReadResult, AgentChangeReadError> {
    let input = validate_read_input(input)?;
    if input.after_revision > runtime_revision {
        return Ok(AgentRuntimeChangeReadResult::RevisionAhead {
            observed_through_revision: input.after_revision,
            runtime_revision,
        });
    }
    let first = changes.first();
    if input.after_revision < runtime_revision
        && first.is_none_or(|first| input.after_revision < first.revision.saturating_sub(1))
    {
        return Ok(AgentRuntimeChangeReadResult::ResetRequired {
            observed_through_revision: runtime_revision,
            runtime_revision,
        });
    }
    let available: Vec<&AgentRuntimeChange> = changes
        .iter()
        .filter(|change| change.revision > input.after_revision)
        .collect();
    if available.is_empty() {
        return Ok(AgentRuntimeChangeReadResult::Current {
            observed_through_revision: input.after_revision,
            runtime_revision,
        });
    }
    let page: Vec<AgentRuntimeChange> = available
        .iter()
        .take(input.limit as usize)
        .map(|change| (*change).clone())
        .collect();
    let observed_through_revision = page
        .last()
        .expect("An Agent change page must contain one change.")
        .revision;
    Ok(AgentRuntimeChangeReadResult::Changes {
        has_more: available.len() > page.len(),
        changes: page,
        observed_through_revision,
        runtime_revision,
    })
}

struct ValidatedReadInput {
    after_revision: u64,
    limit: u64,
}

fn validate_read_input(
    input: AgentChangeReadInput,
) -> Result<ValidatedReadInput, AgentChangeReadError> {
    if !valid_ordered_identity(input.after_revision) {
        return Err(AgentChangeReadError("invalid change read cursor"));
    }
    if !(input.limit.is_finite()
        && input.limit.fract() == 0.0
        && input.limit >= 1.0
        && input.limit <= MAX_AGENT_RUNTIME_CHANGE_PAGE_SIZE as f64)
    {
        return Err(AgentChangeReadError("invalid change page limit"));
    }
    Ok(ValidatedReadInput {
        after_revision: input.after_revision as u64,
        limit: input.limit as u64,
    })
}

fn project_change_session(session: &AgentSessionSnapshot) -> AgentRuntimeChangeSession {
    AgentRuntimeChangeSession {
        lifecycle: session.lifecycle,
        observation: session.observation.clone(),
        parent_session_id: session.parent_session_id.clone(),
        pending_request_count: session.pending_requests.len() as u64,
        provider_id: session.provider_id.clone(),
        session_id: session.session_id.clone(),
        status: project_agent_session(session),
    }
}
