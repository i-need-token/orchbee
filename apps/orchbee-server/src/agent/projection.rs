//! Derives Agent session status, summaries, explanations, and presentation ordering from
//! admitted domain snapshots, preserving semantic priority, freshness, and stable identity
//! ordering. It does not mutate Agent state, read terminal contents, or perform transport
//! effects.

#[cfg(test)]
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[cfg(test)]
use super::AgentRuntimeSnapshot;
#[cfg(test)]
use super::model::compare_agent_identity;
use super::{
    AgentCapability, AgentFreshness, AgentLifecycle, AgentPresentationAttachment,
    AgentSessionSnapshot, AgentTurnState, OpaqueId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentSemanticStatus {
    NeedsAttention {
        count: u64,
    },
    Failed,
    Succeeded,
    Canceled,
    Interrupted,
    Working {
        activity: Option<String>,
        phase: String,
    },
    Idle,
    Unknown {
        activity_hint: Option<String>,
    },
    Ended,
}

impl AgentSemanticStatus {
    #[cfg(test)]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::NeedsAttention { .. } => "needs-attention",
            Self::Failed => "failed",
            Self::Succeeded => "succeeded",
            Self::Canceled => "canceled",
            Self::Interrupted => "interrupted",
            Self::Working { .. } => "working",
            Self::Idle => "idle",
            Self::Unknown { .. } => "unknown",
            Self::Ended => "ended",
        }
    }
}

#[derive(Clone, Debug)]
pub struct AgentSessionSummary {
    pub execution_location_id: OpaqueId,
    pub lifecycle: AgentLifecycle,
    pub observation: super::model::AgentObservation,
    pub parent_session_id: Option<OpaqueId>,
    pub pending_request_count: u64,
    pub presentation_attachments: Vec<AgentPresentationAttachment>,
    pub provider_id: super::model::AgentProviderId,
    pub session_id: OpaqueId,
    pub status: AgentSemanticStatus,
}

#[derive(Clone, Debug)]
pub struct AgentSessionExplanation {
    pub action_path: &'static str,
    pub capability_ceiling: Vec<AgentCapability>,
    pub content_available: bool,
    pub fidelity: super::model::AgentObservationFidelity,
    pub freshness: AgentFreshness,
    pub freshness_meaning: &'static str,
    pub generation: u64,
    pub sequence: u64,
    pub source: super::model::AgentObservationSource,
    pub status: AgentSemanticStatus,
    pub status_basis: &'static str,
}

#[cfg(test)]
#[derive(Clone, Debug)]
pub struct AgentAggregatePresentation {
    pub active_count: u64,
    pub attention_count: u64,
    pub freshness: AgentFreshness,
    pub provider_ids: Vec<super::model::AgentProviderId>,
    pub session_count: u64,
    pub status: AgentSemanticStatus,
}

#[cfg(test)]
#[derive(Clone, Debug)]
pub struct AgentLocationPresentation {
    pub aggregate: AgentAggregatePresentation,
    pub location: AgentPresentationAttachment,
}

/// `projectAgentSession(session).status` — the staged consumers (change
/// journal, summaries, explanations, wait matching) only ever read the status
/// arm of the TypeScript presentation wrapper, so it is inlined here.
pub fn project_agent_session(session: &AgentSessionSnapshot) -> AgentSemanticStatus {
    resolve_status(session)
}

fn resolve_status(session: &AgentSessionSnapshot) -> AgentSemanticStatus {
    if session.lifecycle == AgentLifecycle::Ended {
        return AgentSemanticStatus::Ended;
    }
    if !session.pending_requests.is_empty() {
        return AgentSemanticStatus::NeedsAttention {
            count: session.pending_requests.len() as u64,
        };
    }
    if let Some(outcome) = &session.last_outcome {
        return match outcome.kind.as_str() {
            "failed" => AgentSemanticStatus::Failed,
            "succeeded" => AgentSemanticStatus::Succeeded,
            "canceled" => AgentSemanticStatus::Canceled,
            "interrupted" => AgentSemanticStatus::Interrupted,
            other => unreachable!("canonical lastOutcome kind, got {other}"),
        };
    }
    match &session.turn {
        AgentTurnState::Active {
            activity, phase, ..
        } => AgentSemanticStatus::Working {
            activity: activity.map(|activity| activity.as_str().to_string()),
            phase: phase.as_str().to_string(),
        },
        AgentTurnState::Idle => AgentSemanticStatus::Idle,
        AgentTurnState::Unknown => AgentSemanticStatus::Unknown {
            activity_hint: session
                .activity_hint
                .map(|activity| activity.as_str().to_string()),
        },
    }
}

#[cfg(test)]
fn status_priority(status: &AgentSemanticStatus) -> i64 {
    match status {
        AgentSemanticStatus::NeedsAttention { .. } => 6,
        AgentSemanticStatus::Failed
        | AgentSemanticStatus::Interrupted
        | AgentSemanticStatus::Canceled => 5,
        AgentSemanticStatus::Succeeded => 4,
        AgentSemanticStatus::Working { .. } => 3,
        AgentSemanticStatus::Idle => 2,
        AgentSemanticStatus::Unknown { .. } => 1,
        AgentSemanticStatus::Ended => 0,
    }
}

#[cfg(test)]
fn freshness_rank(freshness: AgentFreshness) -> i64 {
    match freshness {
        AgentFreshness::Fresh => 2,
        AgentFreshness::Stale => 1,
        AgentFreshness::Disconnected => 0,
    }
}

#[cfg(test)]
struct SessionPresentation<'a> {
    freshness: AgentFreshness,
    session_id: &'a str,
    status: AgentSemanticStatus,
}

#[cfg(test)]
fn project_session_presentation(session: &AgentSessionSnapshot) -> SessionPresentation<'_> {
    SessionPresentation {
        freshness: session.observation.freshness,
        session_id: session.session_id.as_str(),
        status: resolve_status(session),
    }
}

#[cfg(test)]
fn compare_session_presentation(
    left: &SessionPresentation<'_>,
    right: &SessionPresentation<'_>,
) -> std::cmp::Ordering {
    let priority = status_priority(&right.status).cmp(&status_priority(&left.status));
    if priority != std::cmp::Ordering::Equal {
        return priority;
    }
    let freshness = freshness_rank(right.freshness).cmp(&freshness_rank(left.freshness));
    if freshness != std::cmp::Ordering::Equal {
        return freshness;
    }
    let hint = |status: &AgentSemanticStatus| -> bool {
        matches!(
            status,
            AgentSemanticStatus::Unknown {
                activity_hint: Some(_)
            }
        )
    };
    let hint_priority = hint(&right.status).cmp(&hint(&left.status));
    if hint_priority != std::cmp::Ordering::Equal {
        return hint_priority;
    }
    compare_agent_identity(left.session_id, right.session_id)
}

#[cfg(test)]
fn aggregate_freshness(observations: &[AgentFreshness]) -> AgentFreshness {
    if observations.contains(&AgentFreshness::Disconnected) {
        return AgentFreshness::Disconnected;
    }
    if observations.contains(&AgentFreshness::Stale) {
        AgentFreshness::Stale
    } else {
        AgentFreshness::Fresh
    }
}

#[cfg(test)]
fn resolve_aggregate(sessions: &[&AgentSessionSnapshot]) -> (AgentFreshness, AgentSemanticStatus) {
    let mut presentations: Vec<SessionPresentation<'_>> = sessions
        .iter()
        .map(|session| project_session_presentation(session))
        .collect();
    presentations.sort_by(compare_session_presentation);
    let Some(highest) = presentations.first() else {
        return (
            AgentFreshness::Disconnected,
            AgentSemanticStatus::Unknown {
                activity_hint: None,
            },
        );
    };
    if !matches!(highest.status, AgentSemanticStatus::NeedsAttention { .. }) {
        return (highest.freshness, highest.status.clone());
    }
    let attention: Vec<&SessionPresentation<'_>> = presentations
        .iter()
        .filter(|presentation| {
            matches!(
                presentation.status,
                AgentSemanticStatus::NeedsAttention { .. }
            )
        })
        .collect();
    (
        aggregate_freshness(
            &attention
                .iter()
                .map(|presentation| presentation.freshness)
                .collect::<Vec<_>>(),
        ),
        AgentSemanticStatus::NeedsAttention {
            count: attention
                .iter()
                .map(|presentation| match presentation.status {
                    AgentSemanticStatus::NeedsAttention { count } => count,
                    _ => 0,
                })
                .sum(),
        },
    )
}

#[cfg(test)]
fn project_aggregate(sessions: Vec<&AgentSessionSnapshot>) -> AgentAggregatePresentation {
    let live: Vec<&AgentSessionSnapshot> = sessions
        .iter()
        .filter(|session| session.lifecycle == AgentLifecycle::Live)
        .copied()
        .collect();
    let retained = if live.is_empty() { sessions } else { live };
    let mut sorted = retained;
    sorted.sort_by(|left, right| {
        compare_agent_identity(left.session_id.as_str(), right.session_id.as_str())
    });
    if sorted.is_empty() {
        panic!("An Agent aggregate projection requires a Session.");
    }
    let (freshness, status) = resolve_aggregate(&sorted);
    AgentAggregatePresentation {
        active_count: sorted
            .iter()
            .filter(|session| matches!(session.turn, AgentTurnState::Active { .. }))
            .count() as u64,
        attention_count: sorted
            .iter()
            .map(|session| session.pending_requests.len() as u64)
            .sum(),
        freshness,
        provider_ids: {
            let mut providers: Vec<&str> = sorted
                .iter()
                .map(|session| session.provider_id.as_str())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            // `[...new Set()].sort()` — canonical UTF-16 identity order.
            providers.sort_by(|left, right| compare_agent_identity(left, right));
            providers
                .into_iter()
                .map(|provider| {
                    super::model::AgentProviderId::new(provider).expect("stored provider id")
                })
                .collect()
        },
        session_count: sorted.len() as u64,
        status,
    }
}

#[cfg(test)]
pub fn project_agent_session_group(
    sessions: &[&AgentSessionSnapshot],
) -> Option<AgentAggregatePresentation> {
    if sessions.is_empty() {
        return None;
    }
    Some(project_aggregate(sessions.to_vec()))
}

#[cfg(test)]
fn location_key(location: &AgentPresentationAttachment) -> (String, String) {
    (
        location.presentation_id.as_str().to_string(),
        location.generation.as_str().to_string(),
    )
}

#[cfg(test)]
pub fn project_agent_locations(snapshot: &AgentRuntimeSnapshot) -> Vec<AgentLocationPresentation> {
    let mut by_location: BTreeMap<(String, String), Vec<&AgentSessionSnapshot>> = BTreeMap::new();
    let mut locations: HashMap<(String, String), &AgentPresentationAttachment> = HashMap::new();
    for session in &snapshot.sessions {
        for attachment in &session.presentation_attachments {
            let key = location_key(attachment);
            by_location.entry(key.clone()).or_default().push(session);
            locations.entry(key).or_insert(attachment);
        }
    }
    let mut presentations: Vec<AgentLocationPresentation> = by_location
        .into_iter()
        .map(|(key, sessions)| AgentLocationPresentation {
            aggregate: project_aggregate(sessions),
            location: (*locations.get(&key).expect("location key")).clone(),
        })
        .collect();
    presentations.sort_by(|left, right| {
        compare_agent_identity(
            left.location.presentation_id.as_str(),
            right.location.presentation_id.as_str(),
        )
        .then_with(|| {
            compare_agent_identity(
                left.location.generation.as_str(),
                right.location.generation.as_str(),
            )
        })
    });
    presentations
}

#[cfg(test)]
fn compare_location_preference(
    left: &AgentLocationPresentation,
    right: &AgentLocationPresentation,
) -> std::cmp::Ordering {
    let navigation = right.location.navigable.cmp(&left.location.navigable);
    if navigation != std::cmp::Ordering::Equal {
        return navigation;
    }
    let freshness =
        freshness_rank(right.aggregate.freshness).cmp(&freshness_rank(left.aggregate.freshness));
    if freshness != std::cmp::Ordering::Equal {
        return freshness;
    }
    let semantic =
        status_priority(&right.aggregate.status).cmp(&status_priority(&left.aggregate.status));
    if semantic != std::cmp::Ordering::Equal {
        return semantic;
    }
    compare_agent_identity(
        right.location.generation.as_str(),
        left.location.generation.as_str(),
    )
}

#[cfg(test)]
pub fn project_agent_location(
    snapshot: &AgentRuntimeSnapshot,
    presentation_id: &str,
) -> Option<AgentLocationPresentation> {
    let mut candidates: Vec<AgentLocationPresentation> = project_agent_locations(snapshot)
        .into_iter()
        .filter(|presentation| presentation.location.presentation_id.as_str() == presentation_id)
        .collect();
    candidates.sort_by(compare_location_preference);
    candidates.into_iter().next()
}

#[cfg(test)]
pub fn project_agent_presentation_group(
    snapshot: &AgentRuntimeSnapshot,
    presentation_ids: &[String],
) -> Option<AgentAggregatePresentation> {
    let mut selected_locations: HashMap<String, String> = HashMap::new();
    for presentation_id in presentation_ids.iter().collect::<BTreeSet<_>>() {
        if let Some(presentation) = project_agent_location(snapshot, presentation_id) {
            selected_locations.insert(
                presentation_id.clone(),
                presentation.location.generation.as_str().to_string(),
            );
        }
    }
    if selected_locations.is_empty() {
        return None;
    }
    let sessions: Vec<&AgentSessionSnapshot> = snapshot
        .sessions
        .iter()
        .filter(|session| {
            session.presentation_attachments.iter().any(|attachment| {
                selected_locations
                    .get(attachment.presentation_id.as_str())
                    .is_some_and(|generation| generation.as_str() == attachment.generation.as_str())
            })
        })
        .collect();
    if sessions.is_empty() {
        return None;
    }
    Some(project_aggregate(sessions))
}

// ---- semantic read model (semantic-commands/agent-semantic-read-model.ts) ----

pub fn project_agent_session_summary(session: &AgentSessionSnapshot) -> AgentSessionSummary {
    AgentSessionSummary {
        execution_location_id: session.execution_location.execution_location_id.clone(),
        lifecycle: session.lifecycle,
        observation: session.observation.clone(),
        parent_session_id: session.parent_session_id.clone(),
        pending_request_count: session.pending_requests.len() as u64,
        presentation_attachments: session.presentation_attachments.clone(),
        provider_id: session.provider_id.clone(),
        session_id: session.session_id.clone(),
        status: resolve_status(session),
    }
}

pub fn explain_agent_session(session: &AgentSessionSnapshot) -> AgentSessionExplanation {
    AgentSessionExplanation {
        action_path: super::model::ATTENTION_ACTION_PATH,
        capability_ceiling: session.capabilities.clone(),
        content_available: false,
        fidelity: session.observation.fidelity,
        freshness: session.observation.freshness,
        freshness_meaning: match session.observation.freshness {
            AgentFreshness::Fresh => "current-observation",
            AgentFreshness::Stale => "last-observation-may-be-outdated",
            AgentFreshness::Disconnected => "no-trusted-current-route",
        },
        generation: session.observation.generation,
        sequence: session.observation.sequence,
        source: session.observation.source,
        status: resolve_status(session),
        status_basis: status_basis(session),
    }
}

fn status_basis(session: &AgentSessionSnapshot) -> &'static str {
    if session.lifecycle == AgentLifecycle::Ended {
        return "session-ended";
    }
    if !session.pending_requests.is_empty() {
        return "attention-request";
    }
    if session.last_outcome.is_some() {
        return "turn-outcome";
    }
    match session.turn {
        AgentTurnState::Active { .. } => "active-turn",
        AgentTurnState::Idle => "affirmative-idle",
        AgentTurnState::Unknown => {
            if session.activity_hint.is_none() {
                "unobserved-turn-state"
            } else {
                "limited-activity-hint"
            }
        }
    }
}

/// The twelve `AgentWaitCondition` values; a status condition only matches on a
/// `fresh` observation, while the three freshness conditions match directly.
#[cfg(test)]
pub const ALL_AGENT_WAIT_CONDITIONS: [&str; 12] = [
    "needs-attention",
    "failed",
    "succeeded",
    "canceled",
    "interrupted",
    "working",
    "idle",
    "unknown",
    "ended",
    "fresh",
    "stale",
    "disconnected",
];

#[cfg(test)]
pub fn matching_agent_wait_conditions<'a>(
    session: &AgentSessionSnapshot,
    conditions: &'a [&'a str],
) -> Vec<&'a str> {
    let status = resolve_status(session);
    conditions
        .iter()
        .copied()
        .filter(|condition| {
            if matches!(*condition, "fresh" | "stale" | "disconnected") {
                return session.observation.freshness.as_str() == *condition;
            }
            session.observation.freshness == AgentFreshness::Fresh && status.kind() == *condition
        })
        .collect()
}
