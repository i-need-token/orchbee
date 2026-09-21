//! `transition_agent_runtime` — the canonical pure reducer.
//!
//! Every transition builds a raw candidate session, then validates it at the
//! same point the TypeScript source parses schemas: `session-observed` and
//! `observation-reconnected` map a failed candidate check to `invalid-event`,
//! while every other path lets the failure surface as a panic inside
//! `applied()`, mirroring the thrown `ZodError`.

use super::AgentObservationEvent;
use super::events::AgentSemanticEvent;
use super::model::{
    MAX_AGENT_PENDING_REQUESTS, MAX_AGENT_PRESENTATION_ATTACHMENTS, MAX_AGENT_SESSIONS,
    MAX_SAFE_INTEGER, RawAttachment, RawSessionSnapshot, RawTurnOutcome, RawTurnState,
    compare_agent_identity, valid_ordered_identity,
};
use super::{
    ATTENTION_ACTION_PATH, AgentCapability, AgentFreshness, AgentLifecycle, AgentObservationSource,
    AgentRuntimeSnapshot, AgentSessionSnapshot, AgentTurnState, RawAttentionRequest, RawPresence,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentTransitionRejection {
    CapabilityNotDeclared,
    CapacityExceeded,
    GenerationMismatch,
    IdentityConflict,
    InvalidEvent,
    InvalidParent,
    PresentationGenerationMismatch,
    PresentationNotFound,
    RequestConflict,
    RequestNotFound,
    RuntimeDisposed,
    SessionEnded,
    SessionNotFound,
    SourceCapabilityExceeded,
    TurnConflict,
    TurnNotFound,
}

impl AgentTransitionRejection {
    #[cfg(test)]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CapabilityNotDeclared => "capability-not-declared",
            Self::CapacityExceeded => "capacity-exceeded",
            Self::GenerationMismatch => "generation-mismatch",
            Self::IdentityConflict => "identity-conflict",
            Self::InvalidEvent => "invalid-event",
            Self::InvalidParent => "invalid-parent",
            Self::PresentationGenerationMismatch => "presentation-generation-mismatch",
            Self::PresentationNotFound => "presentation-not-found",
            Self::RequestConflict => "request-conflict",
            Self::RequestNotFound => "request-not-found",
            Self::RuntimeDisposed => "runtime-disposed",
            Self::SessionEnded => "session-ended",
            Self::SessionNotFound => "session-not-found",
            Self::SourceCapabilityExceeded => "source-capability-exceeded",
            Self::TurnConflict => "turn-conflict",
            Self::TurnNotFound => "turn-not-found",
        }
    }
}

#[derive(Clone, Debug)]
pub enum AgentTransition {
    Applied {
        #[cfg_attr(
            not(test),
            expect(
                dead_code,
                reason = "canonical transition payload; test accessors/future consumers only"
            )
        )]
        semantic_events: Vec<AgentSemanticEvent>,
        snapshot: AgentRuntimeSnapshot,
    },
    Unchanged {
        #[cfg_attr(
            not(test),
            expect(
                dead_code,
                reason = "canonical transition payload; test accessors/future consumers only"
            )
        )]
        snapshot: AgentRuntimeSnapshot,
    },
    Rejected {
        #[cfg_attr(
            not(test),
            expect(
                dead_code,
                reason = "canonical transition payload; test accessors/future consumers only"
            )
        )]
        reason: AgentTransitionRejection,
        #[cfg_attr(
            not(test),
            expect(
                dead_code,
                reason = "canonical transition payload; test accessors/future consumers only"
            )
        )]
        snapshot: AgentRuntimeSnapshot,
    },
}

impl AgentTransition {
    #[cfg(test)]
    pub fn snapshot(&self) -> &AgentRuntimeSnapshot {
        match self {
            Self::Applied { snapshot, .. }
            | Self::Unchanged { snapshot }
            | Self::Rejected { snapshot, .. } => snapshot,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Applied { .. } => "applied",
            Self::Unchanged { .. } => "unchanged",
            Self::Rejected { .. } => "rejected",
        }
    }

    #[cfg(test)]
    pub fn semantic_events(&self) -> &[AgentSemanticEvent] {
        match self {
            Self::Applied {
                semantic_events, ..
            } => semantic_events,
            Self::Unchanged { .. } | Self::Rejected { .. } => &[],
        }
    }
}

pub fn transition_agent_runtime(
    snapshot: &AgentRuntimeSnapshot,
    event: &AgentObservationEvent,
) -> AgentTransition {
    if let AgentObservationEvent::SessionObserved { .. } = event {
        return observe_session(snapshot, event);
    }
    let Some((generation, sequence, session_id)) = event.session_event_base() else {
        unreachable!("non-session-observed events always carry the base fields");
    };
    if !valid_ordered_identity(generation) || !valid_ordered_identity(sequence) {
        return rejected(snapshot, AgentTransitionRejection::InvalidEvent);
    }
    let Some(index) = snapshot
        .sessions
        .iter()
        .position(|session| session.session_id.as_str() == session_id)
    else {
        return rejected(snapshot, AgentTransitionRejection::SessionNotFound);
    };
    let session = &snapshot.sessions[index];
    if generation != session.observation.generation as f64 {
        if let AgentObservationEvent::ObservationReconnected { .. } = event
            && generation > session.observation.generation as f64
        {
            return reconnect_observation(snapshot, index, session, event);
        }
        return rejected(snapshot, AgentTransitionRejection::GenerationMismatch);
    }
    if sequence <= session.observation.sequence as f64 {
        return unchanged(snapshot);
    }
    match event {
        AgentObservationEvent::SessionObserved { .. } => {
            unreachable!("session-observed is handled before the preamble")
        }
        AgentObservationEvent::TurnStarted {
            activity,
            phase,
            turn_id,
            ..
        } => start_turn(snapshot, index, session, activity, phase, turn_id, sequence),
        AgentObservationEvent::TurnActivityChanged {
            activity,
            phase,
            turn_id,
            ..
        } => change_turn_activity(snapshot, index, session, activity, phase, turn_id, sequence),
        AgentObservationEvent::LimitedActivityObserved { activity, .. } => {
            observe_limited_activity(snapshot, index, session, activity, sequence)
        }
        AgentObservationEvent::AttentionRequested {
            event_id, request, ..
        } => request_attention(snapshot, index, session, event_id, request, sequence),
        AgentObservationEvent::AttentionResolved {
            event_id,
            request_id,
            ..
        } => resolve_attention(snapshot, index, session, event_id, request_id, sequence),
        AgentObservationEvent::TurnFinished {
            event_id,
            outcome,
            turn_id,
            ..
        } => finish_turn(
            snapshot, index, session, event_id, outcome, turn_id, sequence,
        ),
        AgentObservationEvent::SessionIdleObserved { .. } => {
            observe_idle(snapshot, index, session, sequence)
        }
        AgentObservationEvent::ObservationBecameStale { .. } => {
            become_stale(snapshot, index, session, sequence)
        }
        AgentObservationEvent::ObservationReconnected { .. } => {
            rejected(snapshot, AgentTransitionRejection::GenerationMismatch)
        }
        AgentObservationEvent::PresentationAttached { attachment, .. } => {
            attach_presentation(snapshot, index, session, attachment, sequence)
        }
        AgentObservationEvent::PresentationVisibilityChanged {
            presentation_generation,
            presentation_id,
            visibility,
            ..
        } => change_presentation_visibility(
            snapshot,
            index,
            session,
            presentation_id,
            presentation_generation,
            visibility,
            sequence,
        ),
        AgentObservationEvent::PresentationDetached {
            presentation_generation,
            presentation_id,
            ..
        } => detach_presentation(
            snapshot,
            index,
            session,
            presentation_id,
            presentation_generation,
            sequence,
        ),
        AgentObservationEvent::SessionEnded { event_id, .. } => {
            end_session(snapshot, index, session, event_id, sequence)
        }
    }
}

fn observe_session(
    snapshot: &AgentRuntimeSnapshot,
    event: &AgentObservationEvent,
) -> AgentTransition {
    let AgentObservationEvent::SessionObserved {
        capabilities,
        execution_location,
        observation,
        parent_session_id,
        presentation_attachments,
        provider_id,
        session_id,
        ..
    } = event
    else {
        unreachable!();
    };
    if !valid_ordered_identity(observation.generation)
        || !valid_ordered_identity(observation.sequence)
        || !observation_source_is_valid(&observation.source, &observation.fidelity)
        || !capabilities_fit_source(capabilities, &observation.source)
    {
        return rejected(snapshot, AgentTransitionRejection::SourceCapabilityExceeded);
    }
    if let Some(existing) = snapshot
        .sessions
        .iter()
        .find(|session| session.session_id.as_str() == session_id.as_str())
    {
        if existing.provider_id.as_str() == provider_id.as_str()
            && existing.execution_location.execution_location_id.as_str()
                == execution_location.execution_location_id.as_str()
            && attachments_equal(&existing.presentation_attachments, presentation_attachments)
        {
            return unchanged(snapshot);
        }
        return rejected(snapshot, AgentTransitionRejection::IdentityConflict);
    }
    if snapshot.sessions.len() >= MAX_AGENT_SESSIONS {
        return rejected(snapshot, AgentTransitionRejection::CapacityExceeded);
    }
    if presentation_attachments.len() > MAX_AGENT_PRESENTATION_ATTACHMENTS {
        return rejected(snapshot, AgentTransitionRejection::CapacityExceeded);
    }
    if presentation_attachments
        .iter()
        .any(|attachment| presentation_generation_conflicts(snapshot, attachment))
    {
        return rejected(
            snapshot,
            AgentTransitionRejection::PresentationGenerationMismatch,
        );
    }
    // `event.parentSessionId !== null` — every non-null presence (including a
    // missing key, read as `undefined`) enters the parent guards.
    if parent_session_id.is_not_null() {
        let parent_found = matches!(parent_session_id, RawPresence::Value(parent)
            if snapshot
                .sessions
                .iter()
                .any(|session| session.session_id.as_str() == parent.as_str()));
        if !parent_found {
            return rejected(snapshot, AgentTransitionRejection::InvalidParent);
        }
        if !capabilities
            .iter()
            .any(|capability| capability == "subagents")
        {
            return rejected(snapshot, AgentTransitionRejection::CapabilityNotDeclared);
        }
    }
    let candidate = RawSessionSnapshot {
        activity_hint: RawPresence::Null,
        capabilities: unique_capabilities(capabilities),
        execution_location: execution_location.clone(),
        last_outcome: RawPresence::Null,
        lifecycle: "live".to_string(),
        observation: observation.clone(),
        parent_session_id: parent_session_id.clone(),
        pending_requests: Vec::new(),
        presentation_attachments: sort_attachments(presentation_attachments.clone()),
        provider_id: provider_id.clone(),
        session_id: session_id.clone(),
        turn: RawTurnState {
            activity: RawPresence::Absent,
            kind: "unknown".to_string(),
            phase: RawPresence::Absent,
            turn_id: RawPresence::Absent,
        },
    };
    let Ok(session) = AgentSessionSnapshot::try_from_raw(candidate) else {
        return rejected(snapshot, AgentTransitionRejection::InvalidEvent);
    };
    let mut sessions = snapshot.sessions.clone();
    sessions.push(session);
    applied(snapshot, sessions, Vec::new())
}

fn start_turn(
    snapshot: &AgentRuntimeSnapshot,
    index: usize,
    session: &AgentSessionSnapshot,
    activity: &RawPresence<String>,
    phase: &str,
    turn_id: &str,
    sequence: f64,
) -> AgentTransition {
    if let Some(unavailable) =
        require_live_capability(snapshot, session, AgentCapability::TurnLifecycle)
    {
        return unavailable;
    }
    if let AgentTurnState::Active {
        turn_id: active, ..
    } = &session.turn
    {
        return if active.as_str() == turn_id {
            unchanged(snapshot)
        } else {
            rejected(snapshot, AgentTransitionRejection::TurnConflict)
        };
    }
    let mut candidate = session.to_raw();
    candidate.activity_hint = RawPresence::Null;
    candidate.last_outcome = RawPresence::Null;
    candidate.observation = advance_observation(session, sequence);
    candidate.turn = RawTurnState {
        activity: activity.clone(),
        kind: "active".to_string(),
        phase: RawPresence::Value(phase.to_string()),
        turn_id: RawPresence::Value(turn_id.to_string()),
    };
    replace_session(snapshot, index, candidate, Vec::new())
}

fn observe_limited_activity(
    snapshot: &AgentRuntimeSnapshot,
    index: usize,
    session: &AgentSessionSnapshot,
    activity: &RawPresence<String>,
    sequence: f64,
) -> AgentTransition {
    if let Some(unavailable) = require_live_capability(snapshot, session, AgentCapability::Activity)
    {
        return unavailable;
    }
    if session.observation.source != AgentObservationSource::Terminal {
        return rejected(snapshot, AgentTransitionRejection::SourceCapabilityExceeded);
    }
    let mut candidate = session.to_raw();
    candidate.activity_hint = activity.clone();
    candidate.observation = advance_observation(session, sequence);
    replace_session(snapshot, index, candidate, Vec::new())
}

fn change_turn_activity(
    snapshot: &AgentRuntimeSnapshot,
    index: usize,
    session: &AgentSessionSnapshot,
    activity: &RawPresence<String>,
    phase: &str,
    turn_id: &str,
    sequence: f64,
) -> AgentTransition {
    if let Some(unavailable) = require_live_capability(snapshot, session, AgentCapability::Activity)
    {
        return unavailable;
    }
    let AgentTurnState::Active {
        turn_id: active, ..
    } = &session.turn
    else {
        return rejected(snapshot, AgentTransitionRejection::TurnNotFound);
    };
    if active.as_str() != turn_id {
        return rejected(snapshot, AgentTransitionRejection::TurnNotFound);
    }
    let mut candidate = session.to_raw();
    candidate.observation = advance_observation(session, sequence);
    candidate.turn = RawTurnState {
        activity: activity.clone(),
        kind: "active".to_string(),
        phase: RawPresence::Value(phase.to_string()),
        turn_id: RawPresence::Value(active.as_str().to_string()),
    };
    replace_session(snapshot, index, candidate, Vec::new())
}

fn request_attention(
    snapshot: &AgentRuntimeSnapshot,
    index: usize,
    session: &AgentSessionSnapshot,
    event_id: &str,
    request: &RawAttentionRequest,
    sequence: f64,
) -> AgentTransition {
    if session.lifecycle != AgentLifecycle::Live {
        return rejected(snapshot, AgentTransitionRejection::SessionEnded);
    }
    let required = attention_capability(&request.kind);
    let admitted = match required {
        AttentionRequirement::Any => has_any_attention_capability(&session.capabilities),
        AttentionRequirement::Exact(capability) => session.capabilities.contains(&capability),
        AttentionRequirement::Unsatisfiable => false,
    };
    if !admitted {
        return rejected(snapshot, AgentTransitionRejection::CapabilityNotDeclared);
    }
    // `request.turnId !== null` — a missing or mistyped key also takes the
    // turn-scoped branch and can never equal the active Turn.
    if request.turn_id.is_not_null() {
        let belongs = match (&session.turn, &request.turn_id) {
            (
                AgentTurnState::Active {
                    turn_id: active, ..
                },
                RawPresence::Value(turn_id),
            ) => active.as_str() == turn_id.as_str(),
            _ => false,
        };
        if !belongs {
            return rejected(snapshot, AgentTransitionRejection::TurnNotFound);
        }
    }
    if let Some(existing) = session
        .pending_requests
        .iter()
        .find(|candidate| candidate.request_id.as_str() == request.request_id.as_str())
    {
        return if requests_equal(existing, request) {
            unchanged(snapshot)
        } else {
            rejected(snapshot, AgentTransitionRejection::RequestConflict)
        };
    }
    if session.pending_requests.len() >= MAX_AGENT_PENDING_REQUESTS {
        return rejected(snapshot, AgentTransitionRejection::CapacityExceeded);
    }
    let mut candidate = session.to_raw();
    candidate.observation = advance_observation(session, sequence);
    candidate.pending_requests.push(request.clone());
    candidate
        .pending_requests
        .sort_by(|left, right| compare_agent_identity(&left.request_id, &right.request_id));
    replace_session(
        snapshot,
        index,
        candidate,
        vec![AgentSemanticEvent::AttentionRequested {
            event_id: event_id.to_string(),
            request: request.clone(),
            session_id: session.session_id.as_str().to_string(),
        }],
    )
}

fn resolve_attention(
    snapshot: &AgentRuntimeSnapshot,
    index: usize,
    session: &AgentSessionSnapshot,
    event_id: &str,
    request_id: &str,
    sequence: f64,
) -> AgentTransition {
    if session.lifecycle != AgentLifecycle::Live {
        return rejected(snapshot, AgentTransitionRejection::SessionEnded);
    }
    if !session
        .pending_requests
        .iter()
        .any(|request| request.request_id.as_str() == request_id)
    {
        return rejected(snapshot, AgentTransitionRejection::RequestNotFound);
    }
    let mut candidate = session.to_raw();
    candidate.observation = advance_observation(session, sequence);
    candidate
        .pending_requests
        .retain(|request| request.request_id != request_id);
    replace_session(
        snapshot,
        index,
        candidate,
        vec![AgentSemanticEvent::AttentionResolved {
            event_id: event_id.to_string(),
            request_id: request_id.to_string(),
            session_id: session.session_id.as_str().to_string(),
        }],
    )
}

fn finish_turn(
    snapshot: &AgentRuntimeSnapshot,
    index: usize,
    session: &AgentSessionSnapshot,
    event_id: &str,
    outcome: &str,
    turn_id: &str,
    sequence: f64,
) -> AgentTransition {
    if let Some(unavailable) =
        require_live_capability(snapshot, session, AgentCapability::TurnLifecycle)
    {
        return unavailable;
    }
    let AgentTurnState::Active {
        turn_id: active, ..
    } = &session.turn
    else {
        return rejected(snapshot, AgentTransitionRejection::TurnNotFound);
    };
    if active.as_str() != turn_id {
        return rejected(snapshot, AgentTransitionRejection::TurnNotFound);
    }
    let mut candidate = session.to_raw();
    candidate.last_outcome = RawPresence::Value(RawTurnOutcome {
        event_id: event_id.to_string(),
        kind: outcome.to_string(),
        turn_id: turn_id.to_string(),
    });
    candidate.observation = advance_observation(session, sequence);
    candidate
        .pending_requests
        .retain(|request| !matches!(&request.turn_id, RawPresence::Value(id) if id == turn_id));
    candidate.turn = RawTurnState {
        activity: RawPresence::Absent,
        kind: "idle".to_string(),
        phase: RawPresence::Absent,
        turn_id: RawPresence::Absent,
    };
    replace_session(
        snapshot,
        index,
        candidate,
        vec![AgentSemanticEvent::TurnOutcome {
            event_id: event_id.to_string(),
            outcome: outcome.to_string(),
            session_id: session.session_id.as_str().to_string(),
            turn_id: turn_id.to_string(),
        }],
    )
}

fn observe_idle(
    snapshot: &AgentRuntimeSnapshot,
    index: usize,
    session: &AgentSessionSnapshot,
    sequence: f64,
) -> AgentTransition {
    if let Some(unavailable) =
        require_live_capability(snapshot, session, AgentCapability::TurnLifecycle)
    {
        return unavailable;
    }
    let active_turn_id = match &session.turn {
        AgentTurnState::Active { turn_id, .. } => Some(turn_id.as_str().to_string()),
        _ => None,
    };
    let mut candidate = session.to_raw();
    candidate.observation = advance_observation(session, sequence);
    if let Some(active_turn_id) = active_turn_id {
        candidate.pending_requests.retain(
            |request| !matches!(&request.turn_id, RawPresence::Value(id) if id == &active_turn_id),
        );
    }
    candidate.turn = RawTurnState {
        activity: RawPresence::Absent,
        kind: "idle".to_string(),
        phase: RawPresence::Absent,
        turn_id: RawPresence::Absent,
    };
    replace_session(snapshot, index, candidate, Vec::new())
}

fn become_stale(
    snapshot: &AgentRuntimeSnapshot,
    index: usize,
    session: &AgentSessionSnapshot,
    sequence: f64,
) -> AgentTransition {
    if session.observation.freshness == AgentFreshness::Disconnected {
        return unchanged(snapshot);
    }
    let mut candidate = session.to_raw();
    candidate.observation = advance_observation(session, sequence);
    candidate.observation.freshness = "stale".to_string();
    replace_session(snapshot, index, candidate, Vec::new())
}

fn reconnect_observation(
    snapshot: &AgentRuntimeSnapshot,
    index: usize,
    session: &AgentSessionSnapshot,
    event: &AgentObservationEvent,
) -> AgentTransition {
    let AgentObservationEvent::ObservationReconnected {
        activity_hint,
        capabilities,
        generation,
        last_outcome,
        pending_requests,
        sequence,
        turn,
        ..
    } = event
    else {
        unreachable!();
    };
    if *sequence <= session.observation.sequence as f64 || session.lifecycle != AgentLifecycle::Live
    {
        return if *sequence <= session.observation.sequence as f64 {
            unchanged(snapshot)
        } else {
            rejected(snapshot, AgentTransitionRejection::SessionEnded)
        };
    }
    if !capabilities_fit_source(capabilities, session.observation.source.as_str()) {
        return rejected(snapshot, AgentTransitionRejection::SourceCapabilityExceeded);
    }
    let mut candidate = session.to_raw();
    candidate.activity_hint = activity_hint.clone();
    candidate.capabilities = unique_capabilities(capabilities);
    candidate.last_outcome = last_outcome.clone();
    candidate.observation.freshness = "fresh".to_string();
    candidate.observation.generation = *generation;
    candidate.observation.sequence = *sequence;
    candidate.pending_requests = pending_requests.clone();
    candidate.turn = turn.clone();
    if !session_claims_fit_capabilities(&candidate) {
        return rejected(snapshot, AgentTransitionRejection::CapabilityNotDeclared);
    }
    let Ok(session_candidate) = AgentSessionSnapshot::try_from_raw(candidate) else {
        return rejected(snapshot, AgentTransitionRejection::InvalidEvent);
    };
    let mut sessions = snapshot.sessions.clone();
    sessions[index] = session_candidate;
    if (AgentRuntimeSnapshot {
        revision: snapshot.revision,
        sessions: sessions.clone(),
    })
    .validate_runtime()
    .is_err()
    {
        return rejected(snapshot, AgentTransitionRejection::InvalidEvent);
    }
    applied(snapshot, sessions, Vec::new())
}

fn attach_presentation(
    snapshot: &AgentRuntimeSnapshot,
    index: usize,
    session: &AgentSessionSnapshot,
    attachment: &RawAttachment,
    sequence: f64,
) -> AgentTransition {
    if session.lifecycle != AgentLifecycle::Live {
        return rejected(snapshot, AgentTransitionRejection::SessionEnded);
    }
    if !attachment.navigable {
        return rejected(snapshot, AgentTransitionRejection::InvalidEvent);
    }
    if presentation_generation_conflicts(snapshot, attachment) {
        return rejected(
            snapshot,
            AgentTransitionRejection::PresentationGenerationMismatch,
        );
    }
    if let Some(attachment_index) = find_attachment_index(
        &session.presentation_attachments,
        &attachment.presentation_id,
        &attachment.generation,
    ) {
        let existing = &session.presentation_attachments[attachment_index];
        if presentation_attachment_matches(existing, attachment) {
            return unchanged(snapshot);
        }
        let mut candidate = session.to_raw();
        candidate.observation = advance_observation(session, sequence);
        candidate.presentation_attachments[attachment_index] = attachment.clone();
        candidate.presentation_attachments = sort_attachments(candidate.presentation_attachments);
        return replace_session(snapshot, index, candidate, Vec::new());
    }
    if session.presentation_attachments.len() >= MAX_AGENT_PRESENTATION_ATTACHMENTS {
        return rejected(snapshot, AgentTransitionRejection::CapacityExceeded);
    }
    let mut candidate = session.to_raw();
    candidate.observation = advance_observation(session, sequence);
    candidate.presentation_attachments.push(attachment.clone());
    candidate.presentation_attachments = sort_attachments(candidate.presentation_attachments);
    replace_session(snapshot, index, candidate, Vec::new())
}

fn change_presentation_visibility(
    snapshot: &AgentRuntimeSnapshot,
    index: usize,
    session: &AgentSessionSnapshot,
    presentation_id: &str,
    presentation_generation: &str,
    visibility: &str,
    sequence: f64,
) -> AgentTransition {
    let Some(attachment_index) = find_attachment_index(
        &session.presentation_attachments,
        presentation_id,
        presentation_generation,
    ) else {
        return rejected(snapshot, AgentTransitionRejection::PresentationNotFound);
    };
    let attachment = &session.presentation_attachments[attachment_index];
    if !attachment.navigable {
        return rejected(snapshot, AgentTransitionRejection::PresentationNotFound);
    }
    if attachment.visibility.as_str() == visibility {
        return unchanged(snapshot);
    }
    let mut candidate = session.to_raw();
    candidate.observation = advance_observation(session, sequence);
    candidate.presentation_attachments[attachment_index].visibility = visibility.to_string();
    replace_session(snapshot, index, candidate, Vec::new())
}

fn detach_presentation(
    snapshot: &AgentRuntimeSnapshot,
    index: usize,
    session: &AgentSessionSnapshot,
    presentation_id: &str,
    presentation_generation: &str,
    sequence: f64,
) -> AgentTransition {
    let Some(attachment_index) = find_attachment_index(
        &session.presentation_attachments,
        presentation_id,
        presentation_generation,
    ) else {
        return rejected(snapshot, AgentTransitionRejection::PresentationNotFound);
    };
    if !session.presentation_attachments[attachment_index].navigable {
        return unchanged(snapshot);
    }
    let mut candidate = session.to_raw();
    candidate.observation = advance_observation(session, sequence);
    candidate.presentation_attachments[attachment_index].navigable = false;
    candidate.presentation_attachments[attachment_index].visibility = "unknown".to_string();
    let has_navigable_attachment = candidate
        .presentation_attachments
        .iter()
        .any(|attachment| attachment.navigable);
    // TS keeps the stored freshness while any navigable attachment remains;
    // losing the last one forces `disconnected`.
    candidate.observation.freshness = if has_navigable_attachment {
        session.observation.freshness.as_str().to_string()
    } else {
        "disconnected".to_string()
    };
    replace_session(snapshot, index, candidate, Vec::new())
}

fn end_session(
    snapshot: &AgentRuntimeSnapshot,
    index: usize,
    session: &AgentSessionSnapshot,
    event_id: &str,
    sequence: f64,
) -> AgentTransition {
    let retires_heuristic_presence = session.observation.source
        == AgentObservationSource::Heuristic
        && session.observation.fidelity == super::model::AgentObservationFidelity::Heuristic
        && session.capabilities.is_empty();
    if session.lifecycle != AgentLifecycle::Live {
        return rejected(snapshot, AgentTransitionRejection::SessionEnded);
    }
    if !retires_heuristic_presence
        && let Some(unavailable) =
            require_live_capability(snapshot, session, AgentCapability::SessionLifecycle)
    {
        return unavailable;
    }
    let interrupted = match &session.turn {
        AgentTurnState::Active { turn_id, .. } => Some(RawTurnOutcome {
            event_id: event_id.to_string(),
            kind: "interrupted".to_string(),
            turn_id: turn_id.as_str().to_string(),
        }),
        _ => session.last_outcome.as_ref().map(|outcome| RawTurnOutcome {
            event_id: outcome.event_id.as_str().to_string(),
            kind: outcome.kind.as_str().to_string(),
            turn_id: outcome.turn_id.as_str().to_string(),
        }),
    };
    let mut semantic_events = Vec::new();
    if !retires_heuristic_presence {
        if let AgentTurnState::Active { turn_id, .. } = &session.turn {
            semantic_events.push(AgentSemanticEvent::TurnOutcome {
                event_id: event_id.to_string(),
                outcome: "interrupted".to_string(),
                session_id: session.session_id.as_str().to_string(),
                turn_id: turn_id.as_str().to_string(),
            });
        }
        semantic_events.push(AgentSemanticEvent::SessionEnded {
            event_id: event_id.to_string(),
            session_id: session.session_id.as_str().to_string(),
        });
    }
    let mut candidate = session.to_raw();
    candidate.activity_hint = RawPresence::Null;
    candidate.last_outcome = match interrupted {
        Some(outcome) => RawPresence::Value(outcome),
        None => RawPresence::Null,
    };
    candidate.lifecycle = "ended".to_string();
    candidate.observation = advance_observation(session, sequence);
    if retires_heuristic_presence {
        candidate.observation.freshness = "disconnected".to_string();
    }
    candidate.pending_requests = Vec::new();
    candidate.turn = RawTurnState {
        activity: RawPresence::Absent,
        kind: "unknown".to_string(),
        phase: RawPresence::Absent,
        turn_id: RawPresence::Absent,
    };
    replace_session(snapshot, index, candidate, semantic_events)
}

fn require_live_capability(
    snapshot: &AgentRuntimeSnapshot,
    session: &AgentSessionSnapshot,
    capability: AgentCapability,
) -> Option<AgentTransition> {
    if session.lifecycle != AgentLifecycle::Live {
        return Some(rejected(snapshot, AgentTransitionRejection::SessionEnded));
    }
    if !session.capabilities.contains(&capability) {
        return Some(rejected(
            snapshot,
            AgentTransitionRejection::CapabilityNotDeclared,
        ));
    }
    None
}

fn replace_session(
    snapshot: &AgentRuntimeSnapshot,
    index: usize,
    candidate: RawSessionSnapshot,
    semantic_events: Vec<AgentSemanticEvent>,
) -> AgentTransition {
    // Mirrors the TypeScript `applied()` schema parse: a candidate that cannot
    // satisfy the canonical invariants is a reachable throw, not a rejection.
    let session = AgentSessionSnapshot::try_from_raw(candidate)
        .unwrap_or_else(|error| panic!("Agent session invariant violation: {error}"));
    let mut sessions = snapshot.sessions.clone();
    sessions[index] = session;
    applied(snapshot, sessions, semantic_events)
}

fn applied(
    previous: &AgentRuntimeSnapshot,
    sessions: Vec<AgentSessionSnapshot>,
    semantic_events: Vec<AgentSemanticEvent>,
) -> AgentTransition {
    if previous.revision >= MAX_SAFE_INTEGER {
        return rejected(previous, AgentTransitionRejection::CapacityExceeded);
    }
    let mut sessions = sessions;
    sessions.sort_by(|left, right| {
        compare_agent_identity(left.session_id.as_str(), right.session_id.as_str())
    });
    let snapshot = AgentRuntimeSnapshot {
        revision: previous.revision + 1,
        sessions,
    };
    snapshot
        .validate_runtime()
        .unwrap_or_else(|error| panic!("Agent runtime invariant violation: {error}"));
    AgentTransition::Applied {
        semantic_events,
        snapshot,
    }
}

fn unchanged(snapshot: &AgentRuntimeSnapshot) -> AgentTransition {
    AgentTransition::Unchanged {
        snapshot: snapshot.clone(),
    }
}

fn rejected(snapshot: &AgentRuntimeSnapshot, reason: AgentTransitionRejection) -> AgentTransition {
    AgentTransition::Rejected {
        reason,
        snapshot: snapshot.clone(),
    }
}

fn advance_observation(
    session: &AgentSessionSnapshot,
    sequence: f64,
) -> super::model::RawObservation {
    super::model::RawObservation {
        fidelity: session.observation.fidelity.as_str().to_string(),
        freshness: "fresh".to_string(),
        generation: session.observation.generation as f64,
        sequence,
        source: session.observation.source.as_str().to_string(),
    }
}

fn unique_capabilities(capabilities: &[String]) -> Vec<String> {
    let mut unique: Vec<String> = capabilities
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .cloned()
        .collect();
    unique.sort_by(|left, right| compare_agent_identity(left, right));
    unique
}

fn observation_source_is_valid(source: &str, fidelity: &str) -> bool {
    if source == "heuristic" {
        return fidelity == "heuristic";
    }
    if source == "terminal" {
        return fidelity != "rich";
    }
    fidelity != "heuristic"
}

fn capabilities_fit_source(capabilities: &[String], source: &str) -> bool {
    if source == "heuristic" {
        return capabilities.is_empty();
    }
    if source == "terminal" {
        return capabilities
            .iter()
            .all(|capability| capability == "activity");
    }
    true
}

/// The attention capability a request kind requires. `unclassified` accepts any
/// attention capability; an unknown kind mirrors the TypeScript fall-through
/// (`requiredCapability` is `undefined`, which `includes()` never satisfies).
enum AttentionRequirement {
    Any,
    Exact(AgentCapability),
    Unsatisfiable,
}

fn attention_capability(kind: &str) -> AttentionRequirement {
    match kind {
        "question" => AttentionRequirement::Exact(AgentCapability::Questions),
        "approval" => AttentionRequirement::Exact(AgentCapability::Approvals),
        "authentication" => AttentionRequirement::Exact(AgentCapability::Authentication),
        "unclassified" => AttentionRequirement::Any,
        _ => AttentionRequirement::Unsatisfiable,
    }
}

fn has_any_attention_capability(capabilities: &[AgentCapability]) -> bool {
    capabilities.iter().any(|capability| {
        matches!(
            capability,
            AgentCapability::Questions
                | AgentCapability::Approvals
                | AgentCapability::Authentication
        )
    })
}

/// `sessionClaimsFitCapabilities` on a raw candidate — capability membership is
/// checked against the raw (already deduplicated) capability tokens exactly as
/// the TypeScript string list does.
fn session_claims_fit_capabilities(session: &RawSessionSnapshot) -> bool {
    let has = |capability: &str| session.capabilities.iter().any(|c| c == capability);
    if session.activity_hint.is_not_null()
        && (!has("activity") || session.observation.source != "terminal")
    {
        return false;
    }
    if (session.turn.kind != "unknown" || session.last_outcome.is_not_null())
        && !has("turnLifecycle")
    {
        return false;
    }
    session
        .pending_requests
        .iter()
        .all(|request| match attention_capability(&request.kind) {
            AttentionRequirement::Any => session.capabilities.iter().any(|capability| {
                matches!(
                    capability.as_str(),
                    "questions" | "approvals" | "authentication"
                )
            }),
            AttentionRequirement::Exact(capability) => has(capability.as_str()),
            AttentionRequirement::Unsatisfiable => false,
        })
}

fn requests_equal(left: &super::model::AgentAttentionRequest, right: &RawAttentionRequest) -> bool {
    right.action_path == ATTENTION_ACTION_PATH
        && left.kind.as_str() == right.kind
        && left.request_id.as_str() == right.request_id
        && match (&left.turn_id, &right.turn_id) {
            (None, RawPresence::Null) => true,
            (Some(id), RawPresence::Value(turn_id)) => id.as_str() == turn_id,
            _ => false,
        }
}

fn find_attachment_index(
    attachments: &[super::model::AgentPresentationAttachment],
    presentation_id: &str,
    generation: &str,
) -> Option<usize> {
    attachments.iter().position(|attachment| {
        attachment.presentation_id.as_str() == presentation_id
            && attachment.generation.as_str() == generation
    })
}

fn sort_attachments(attachments: Vec<RawAttachment>) -> Vec<RawAttachment> {
    let mut attachments = attachments;
    attachments.sort_by(|left, right| {
        compare_agent_identity(&left.presentation_id, &right.presentation_id)
            .then_with(|| compare_agent_identity(&left.generation, &right.generation))
    });
    attachments
}

fn presentation_attachment_matches(
    left: &super::model::AgentPresentationAttachment,
    right: &RawAttachment,
) -> bool {
    left.generation.as_str() == right.generation.as_str()
        && left.navigable == right.navigable
        && left.presentation_id.as_str() == right.presentation_id
        && left.visibility.as_str() == right.visibility
}

fn attachments_equal(
    left: &[super::model::AgentPresentationAttachment],
    right: &[RawAttachment],
) -> bool {
    if left.len() != right.len() {
        return false;
    }
    // Stored attachments are already in canonical order; sorting the raw side
    // yields the same pairwise comparison as the TypeScript double sort.
    let sorted_right = sort_attachments(right.to_vec());
    left.iter()
        .zip(sorted_right.iter())
        .all(|(l, r)| presentation_attachment_matches(l, r))
}

fn presentation_generation_conflicts(
    snapshot: &AgentRuntimeSnapshot,
    attachment: &RawAttachment,
) -> bool {
    if !attachment.navigable {
        return false;
    }
    snapshot.sessions.iter().any(|session| {
        session.presentation_attachments.iter().any(|candidate| {
            candidate.navigable
                && candidate.presentation_id.as_str() == attachment.presentation_id
                && candidate.generation.as_str() != attachment.generation.as_str()
        })
    })
}
