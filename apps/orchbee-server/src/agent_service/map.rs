//! The explicit canonical→wire mapper for the Agent first-read. Every
//! conversion is typed and total over canonical-valid state; anything that
//! cannot be represented under the strict native wire profile fails loudly
//! with `AgentReadFault` — observations are never silently filtered and
//! never fabricated. Token enums map through their canonical schema token:
//! the canonical and wire vocabularies share one schema, so an unknown token
//! is a loud fault, never a silent drop.

use orchbee_agent_observation_protocol::{
    AgentActivity, AgentAttachmentGeneration, AgentAttentionKind, AgentAttentionRequest,
    AgentAttentionRequestId, AgentCapability, AgentEventId, AgentExecutionLocation,
    AgentExecutionLocationId, AgentFreshnessMeaning, AgentLifecycle, AgentObservation,
    AgentObservationFidelity, AgentObservationFreshness, AgentObservationSource,
    AgentPositiveInteger, AgentPresentationAttachment, AgentPresentationId,
    AgentPresentationVisibility, AgentProviderId, AgentSafeInteger, AgentSemanticStatus,
    AgentSessionDetail, AgentSessionExplanation, AgentSessionId, AgentSessionSnapshot,
    AgentSessionSummary, AgentStatusBasis, AgentTurnId, AgentTurnOutcome, AgentTurnOutcomeKind,
    AgentTurnPhase, AgentTurnState, ContentUnavailable, MAXIMUM_SAFE_INTEGER, NavigateOnly,
};

use super::AgentReadFault;
use crate::agent::{self, AgentSemanticStatus as CanonicalStatus};

fn fault(reason: &'static str) -> AgentReadFault {
    AgentReadFault(reason)
}

fn safe_integer(value: u64) -> Result<AgentSafeInteger, AgentReadFault> {
    if value > MAXIMUM_SAFE_INTEGER {
        return Err(fault("canonical safe integer exceeds the wire bound"));
    }
    Ok(AgentSafeInteger::new(value))
}

fn session_id(value: &agent::OpaqueId) -> Result<AgentSessionId, AgentReadFault> {
    AgentSessionId::parse(value.as_str())
        .map_err(|_error| fault("canonical session id is not wire-representable"))
}

fn turn_id(value: &agent::OpaqueId) -> Result<AgentTurnId, AgentReadFault> {
    AgentTurnId::parse(value.as_str())
        .map_err(|_error| fault("canonical turn id is not wire-representable"))
}

fn request_id(value: &agent::OpaqueId) -> Result<AgentAttentionRequestId, AgentReadFault> {
    AgentAttentionRequestId::parse(value.as_str())
        .map_err(|_error| fault("canonical attention request id is not wire-representable"))
}

fn event_id(value: &agent::OpaqueId) -> Result<AgentEventId, AgentReadFault> {
    AgentEventId::parse(value.as_str())
        .map_err(|_error| fault("canonical event id is not wire-representable"))
}

fn execution_location_id(
    value: &agent::OpaqueId,
) -> Result<AgentExecutionLocationId, AgentReadFault> {
    AgentExecutionLocationId::parse(value.as_str())
        .map_err(|_error| fault("canonical execution location id is not wire-representable"))
}

fn attachment_generation(
    value: &agent::OpaqueId,
) -> Result<AgentAttachmentGeneration, AgentReadFault> {
    AgentAttachmentGeneration::parse(value.as_str())
        .map_err(|_error| fault("canonical attachment generation is not wire-representable"))
}

fn presentation_id(value: &agent::PresentationId) -> Result<AgentPresentationId, AgentReadFault> {
    AgentPresentationId::parse(value.as_str())
        .map_err(|_error| fault("canonical presentation id is outside the native wire profile"))
}

fn provider_id(value: &agent::AgentProviderId) -> Result<AgentProviderId, AgentReadFault> {
    AgentProviderId::parse(value.as_str())
        .map_err(|_error| fault("canonical provider id is not wire-representable"))
}

fn activity(value: agent::AgentActivity) -> Result<AgentActivity, AgentReadFault> {
    activity_token(value.as_str())
}

fn activity_token(value: &str) -> Result<AgentActivity, AgentReadFault> {
    Ok(match value {
        "thinking" => AgentActivity::Thinking,
        "tool" => AgentActivity::Tool,
        "editing" => AgentActivity::Editing,
        "command" => AgentActivity::Command,
        "subagent" => AgentActivity::Subagent,
        "compacting" => AgentActivity::Compacting,
        "retrying" => AgentActivity::Retrying,
        "other" => AgentActivity::Other,
        _ => return Err(fault("canonical activity is not wire-representable")),
    })
}

fn capability(value: agent::AgentCapability) -> Result<AgentCapability, AgentReadFault> {
    Ok(match value.as_str() {
        "sessionLifecycle" => AgentCapability::SessionLifecycle,
        "turnLifecycle" => AgentCapability::TurnLifecycle,
        "questions" => AgentCapability::Questions,
        "approvals" => AgentCapability::Approvals,
        "authentication" => AgentCapability::Authentication,
        "activity" => AgentCapability::Activity,
        "subagents" => AgentCapability::Subagents,
        "progress" => AgentCapability::Progress,
        _ => return Err(fault("canonical capability is not wire-representable")),
    })
}

fn visibility(
    value: agent::AgentPresentationVisibility,
) -> Result<AgentPresentationVisibility, AgentReadFault> {
    Ok(match value.as_str() {
        "visible" => AgentPresentationVisibility::Visible,
        "hidden" => AgentPresentationVisibility::Hidden,
        "unknown" => AgentPresentationVisibility::Unknown,
        _ => return Err(fault("canonical visibility is not wire-representable")),
    })
}

fn fidelity(
    value: agent::AgentObservationFidelity,
) -> Result<AgentObservationFidelity, AgentReadFault> {
    Ok(match value.as_str() {
        "rich" => AgentObservationFidelity::Rich,
        "basic" => AgentObservationFidelity::Basic,
        "heuristic" => AgentObservationFidelity::Heuristic,
        _ => return Err(fault("canonical fidelity is not wire-representable")),
    })
}

fn freshness(value: agent::AgentFreshness) -> Result<AgentObservationFreshness, AgentReadFault> {
    Ok(match value.as_str() {
        "fresh" => AgentObservationFreshness::Fresh,
        "stale" => AgentObservationFreshness::Stale,
        "disconnected" => AgentObservationFreshness::Disconnected,
        _ => return Err(fault("canonical freshness is not wire-representable")),
    })
}

fn source(value: agent::AgentObservationSource) -> Result<AgentObservationSource, AgentReadFault> {
    Ok(match value.as_str() {
        "native" => AgentObservationSource::Native,
        "hook" => AgentObservationSource::Hook,
        "terminal" => AgentObservationSource::Terminal,
        "heuristic" => AgentObservationSource::Heuristic,
        _ => return Err(fault("canonical source is not wire-representable")),
    })
}

fn phase(value: agent::AgentTurnPhase) -> Result<AgentTurnPhase, AgentReadFault> {
    phase_token(value.as_str())
}

fn phase_token(value: &str) -> Result<AgentTurnPhase, AgentReadFault> {
    Ok(match value {
        "queued" => AgentTurnPhase::Queued,
        "working" => AgentTurnPhase::Working,
        "waiting-external" => AgentTurnPhase::WaitingExternal,
        _ => return Err(fault("canonical turn phase is not wire-representable")),
    })
}

fn attention_kind(value: agent::AgentAttentionKind) -> Result<AgentAttentionKind, AgentReadFault> {
    Ok(match value.as_str() {
        "question" => AgentAttentionKind::Question,
        "approval" => AgentAttentionKind::Approval,
        "authentication" => AgentAttentionKind::Authentication,
        "unclassified" => AgentAttentionKind::Unclassified,
        _ => return Err(fault("canonical attention kind is not wire-representable")),
    })
}

fn outcome_kind(
    value: agent::AgentTurnOutcomeKind,
) -> Result<AgentTurnOutcomeKind, AgentReadFault> {
    Ok(match value.as_str() {
        "succeeded" => AgentTurnOutcomeKind::Succeeded,
        "failed" => AgentTurnOutcomeKind::Failed,
        "canceled" => AgentTurnOutcomeKind::Canceled,
        "interrupted" => AgentTurnOutcomeKind::Interrupted,
        _ => return Err(fault("canonical outcome kind is not wire-representable")),
    })
}

fn lifecycle(value: agent::AgentLifecycle) -> Result<AgentLifecycle, AgentReadFault> {
    Ok(match value.as_str() {
        "live" => AgentLifecycle::Live,
        "ended" => AgentLifecycle::Ended,
        _ => return Err(fault("canonical lifecycle is not wire-representable")),
    })
}

fn observation(value: &agent::AgentObservation) -> Result<AgentObservation, AgentReadFault> {
    Ok(AgentObservation {
        fidelity: fidelity(value.fidelity)?,
        freshness: freshness(value.freshness)?,
        generation: safe_integer(value.generation)?,
        sequence: safe_integer(value.sequence)?,
        source: source(value.source)?,
    })
}

fn turn(value: &agent::AgentTurnState) -> Result<AgentTurnState, AgentReadFault> {
    Ok(match value {
        agent::AgentTurnState::Unknown => AgentTurnState::Unknown {},
        agent::AgentTurnState::Idle => AgentTurnState::Idle {},
        agent::AgentTurnState::Active {
            activity: turn_activity,
            phase: turn_phase,
            turn_id: active,
        } => AgentTurnState::Active {
            activity: turn_activity.map(activity).transpose()?,
            phase: phase(*turn_phase)?,
            turn_id: turn_id(active)?,
        },
    })
}

fn attention_request(
    value: &agent::AgentAttentionRequest,
) -> Result<AgentAttentionRequest, AgentReadFault> {
    Ok(AgentAttentionRequest {
        action_path: NavigateOnly,
        kind: attention_kind(value.kind)?,
        request_id: request_id(&value.request_id)?,
        turn_id: value.turn_id.as_ref().map(turn_id).transpose()?,
    })
}

fn turn_outcome(value: &agent::AgentTurnOutcome) -> Result<AgentTurnOutcome, AgentReadFault> {
    Ok(AgentTurnOutcome {
        event_id: event_id(&value.event_id)?,
        kind: outcome_kind(value.kind)?,
        turn_id: turn_id(&value.turn_id)?,
    })
}

fn attachment(
    value: &agent::AgentPresentationAttachment,
) -> Result<AgentPresentationAttachment, AgentReadFault> {
    Ok(AgentPresentationAttachment {
        generation: attachment_generation(&value.generation)?,
        navigable: value.navigable,
        presentation_id: presentation_id(&value.presentation_id)?,
        visibility: visibility(value.visibility)?,
    })
}

fn snapshot(value: &agent::AgentSessionSnapshot) -> Result<AgentSessionSnapshot, AgentReadFault> {
    Ok(AgentSessionSnapshot {
        activity_hint: value.activity_hint.map(activity).transpose()?,
        capabilities: value
            .capabilities
            .iter()
            .map(|item| capability(*item))
            .collect::<Result<_, _>>()?,
        execution_location: AgentExecutionLocation {
            execution_location_id: execution_location_id(
                &value.execution_location.execution_location_id,
            )?,
        },
        last_outcome: value.last_outcome.as_ref().map(turn_outcome).transpose()?,
        lifecycle: lifecycle(value.lifecycle)?,
        observation: observation(&value.observation)?,
        parent_session_id: value
            .parent_session_id
            .as_ref()
            .map(session_id)
            .transpose()?,
        pending_requests: value
            .pending_requests
            .iter()
            .map(attention_request)
            .collect::<Result<_, _>>()?,
        presentation_attachments: value
            .presentation_attachments
            .iter()
            .map(attachment)
            .collect::<Result<_, _>>()?,
        provider_id: provider_id(&value.provider_id)?,
        session_id: session_id(&value.session_id)?,
        turn: turn(&value.turn)?,
    })
}

fn status(value: &CanonicalStatus) -> Result<AgentSemanticStatus, AgentReadFault> {
    Ok(match value {
        CanonicalStatus::NeedsAttention { count } => AgentSemanticStatus::NeedsAttention {
            count: AgentPositiveInteger::new(*count)
                .ok_or(fault("canonical attention count is not a positive integer"))?,
        },
        CanonicalStatus::Failed => AgentSemanticStatus::Failed {},
        CanonicalStatus::Succeeded => AgentSemanticStatus::Succeeded {},
        CanonicalStatus::Canceled => AgentSemanticStatus::Canceled {},
        CanonicalStatus::Interrupted => AgentSemanticStatus::Interrupted {},
        CanonicalStatus::Working {
            activity: working_activity,
            phase: working_phase,
        } => AgentSemanticStatus::Working {
            activity: working_activity
                .as_deref()
                .map(activity_token)
                .transpose()?,
            phase: phase_token(working_phase)?,
        },
        CanonicalStatus::Idle => AgentSemanticStatus::Idle {},
        CanonicalStatus::Unknown { activity_hint } => AgentSemanticStatus::Unknown {
            activity_hint: activity_hint.as_deref().map(activity_token).transpose()?,
        },
        CanonicalStatus::Ended => AgentSemanticStatus::Ended {},
    })
}

/// `get`/`explain` session detail: the full wire snapshot plus resolved status.
pub(super) fn session_detail(
    session: &agent::AgentSessionSnapshot,
) -> Result<AgentSessionDetail, AgentReadFault> {
    Ok(AgentSessionDetail {
        snapshot: snapshot(session)?,
        status: status(&agent::project_agent_session(session))?,
    })
}

/// `list` item: the canonical summary projection mapped field by field.
pub(super) fn session_summary(
    session: &agent::AgentSessionSnapshot,
) -> Result<AgentSessionSummary, AgentReadFault> {
    let summary = agent::project_agent_session_summary(session);
    Ok(AgentSessionSummary {
        execution_location: AgentExecutionLocation {
            execution_location_id: execution_location_id(&summary.execution_location_id)?,
        },
        lifecycle: lifecycle(summary.lifecycle)?,
        observation: observation(&summary.observation)?,
        parent_session_id: summary
            .parent_session_id
            .as_ref()
            .map(session_id)
            .transpose()?,
        pending_request_count: safe_integer(summary.pending_request_count)?,
        presentation_attachments: summary
            .presentation_attachments
            .iter()
            .map(attachment)
            .collect::<Result<_, _>>()?,
        provider_id: provider_id(&summary.provider_id)?,
        session_id: session_id(&summary.session_id)?,
        status: status(&summary.status)?,
    })
}

/// `explain` outcome: the canonical explanation mapped field by field. The
/// canonical `action_path` and `content_available` are verified, not
/// paralleled — a canonical explanation that disagrees with the
/// navigate-only/no-content contract is a loud fault.
pub(super) fn session_explanation(
    session: &agent::AgentSessionSnapshot,
) -> Result<AgentSessionExplanation, AgentReadFault> {
    let explanation = agent::explain_agent_session(session);
    if explanation.action_path != agent::ATTENTION_ACTION_PATH {
        return Err(fault(
            "canonical explanation action path is not navigate-only",
        ));
    }
    if explanation.content_available {
        return Err(fault("canonical explanation unexpectedly reports content"));
    }
    Ok(AgentSessionExplanation {
        action_path: NavigateOnly,
        capability_ceiling: explanation
            .capability_ceiling
            .iter()
            .map(|item| capability(*item))
            .collect::<Result<_, _>>()?,
        content_available: ContentUnavailable,
        fidelity: fidelity(explanation.fidelity)?,
        freshness: freshness(explanation.freshness)?,
        freshness_meaning: match explanation.freshness_meaning {
            "current-observation" => AgentFreshnessMeaning::CurrentObservation,
            "last-observation-may-be-outdated" => {
                AgentFreshnessMeaning::LastObservationMayBeOutdated
            }
            "no-trusted-current-route" => AgentFreshnessMeaning::NoTrustedCurrentRoute,
            _ => {
                return Err(fault(
                    "canonical freshness meaning is not wire-representable",
                ));
            }
        },
        generation: safe_integer(explanation.generation)?,
        sequence: safe_integer(explanation.sequence)?,
        source: source(explanation.source)?,
        status: status(&explanation.status)?,
        status_basis: match explanation.status_basis {
            "attention-request" => AgentStatusBasis::AttentionRequest,
            "turn-outcome" => AgentStatusBasis::TurnOutcome,
            "active-turn" => AgentStatusBasis::ActiveTurn,
            "affirmative-idle" => AgentStatusBasis::AffirmativeIdle,
            "session-ended" => AgentStatusBasis::SessionEnded,
            "limited-activity-hint" => AgentStatusBasis::LimitedActivityHint,
            "unobserved-turn-state" => AgentStatusBasis::UnobservedTurnState,
            _ => {
                return Err(fault("canonical status basis is not wire-representable"));
            }
        },
    })
}

/// The canonical runtime revision carried on every successful outcome.
pub(super) fn revision(value: u64) -> Result<AgentSafeInteger, AgentReadFault> {
    safe_integer(value)
}

/// Journal records deliberately contain no reconstructed location or attachments.
pub(super) fn runtime_change(
    change: &agent::AgentRuntimeChange,
) -> Result<orchbee_agent_observation_protocol::AgentChange, AgentReadFault> {
    use orchbee_agent_observation_protocol::{
        AgentChange, AgentChangeSession, AgentChangeTransition as T,
    };
    let session = &change.session;
    Ok(AgentChange {
        revision: safe_integer(change.revision)?,
        transition: match change.transition {
            "session-observed" => T::SessionObserved,
            "turn-started" => T::TurnStarted,
            "turn-activity-changed" => T::TurnActivityChanged,
            "attention-requested" => T::AttentionRequested,
            "attention-resolved" => T::AttentionResolved,
            "turn-finished" => T::TurnFinished,
            "session-idle-observed" => T::SessionIdleObserved,
            "observation-became-stale" => T::ObservationBecameStale,
            "observation-reconnected" => T::ObservationReconnected,
            "presentation-attached" => T::PresentationAttached,
            "presentation-visibility-changed" => T::PresentationVisibilityChanged,
            "presentation-detached" => T::PresentationDetached,
            "session-ended" => T::SessionEnded,
            _ => return Err(fault("unknown canonical change transition")),
        },
        session: AgentChangeSession {
            lifecycle: lifecycle(session.lifecycle)?,
            observation: observation(&session.observation)?,
            parent_session_id: session
                .parent_session_id
                .as_ref()
                .map(session_id)
                .transpose()?,
            pending_request_count: safe_integer(session.pending_request_count)?,
            provider_id: provider_id(&session.provider_id)?,
            session_id: session_id(&session.session_id)?,
            status: status(&session.status)?,
        },
    })
}
