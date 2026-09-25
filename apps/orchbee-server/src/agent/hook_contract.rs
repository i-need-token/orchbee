//! Production port of the Desktop agent-hook observation contract and the
//! adapter event gate — `agent-hook-observation-contract.ts` and
//! `resolveAgentHookAdapter.ts` (fingerprints recorded in the hook fixture).
//!
//! Parsing mirrors the Zod `strictObject` boundary exactly: every declared key
//! is required, any extra key fails, `null` is accepted only where the schema
//! is `.nullable()`, and identity strings keep their length/regex bounds.

use serde_json::Value;

use super::{AgentActivity, AgentCapability, AgentPresentationVisibility};

pub const MAX_CORRELATIONS_PER_EVENT: usize = 32;
pub const MAX_OBSERVATION_BODY_BYTES: usize = 16 * 1024;

const TRANSPORT_IDENTITY_MIN: usize = 16;
const TRANSPORT_IDENTITY_MAX: usize = 64;
const HOOK_IDENTITY_MIN: usize = 1;
const HOOK_IDENTITY_MAX: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HookObservationEvent {
    SessionIdentified,
    SessionStarted,
    SessionIdle,
    TurnStarted,
    ToolStarted,
    ToolFinished,
    ToolFailed,
    QuestionRequested,
    ApprovalRequested,
    AttentionResolved,
    CompactionStarted,
    CompactionFinished,
    SubagentStarted,
    SubagentStopped,
    TurnStopped,
    TurnFailed,
    TurnCanceled,
    TurnInterrupted,
    SessionEnded,
}

impl HookObservationEvent {
    #[cfg(test)]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SessionIdentified => "session-identified",
            Self::SessionStarted => "session-started",
            Self::SessionIdle => "session-idle",
            Self::TurnStarted => "turn-started",
            Self::ToolStarted => "tool-started",
            Self::ToolFinished => "tool-finished",
            Self::ToolFailed => "tool-failed",
            Self::QuestionRequested => "question-requested",
            Self::ApprovalRequested => "approval-requested",
            Self::AttentionResolved => "attention-resolved",
            Self::CompactionStarted => "compaction-started",
            Self::CompactionFinished => "compaction-finished",
            Self::SubagentStarted => "subagent-started",
            Self::SubagentStopped => "subagent-stopped",
            Self::TurnStopped => "turn-stopped",
            Self::TurnFailed => "turn-failed",
            Self::TurnCanceled => "turn-canceled",
            Self::TurnInterrupted => "turn-interrupted",
            Self::SessionEnded => "session-ended",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "session-identified" => Self::SessionIdentified,
            "session-started" => Self::SessionStarted,
            "session-idle" => Self::SessionIdle,
            "turn-started" => Self::TurnStarted,
            "tool-started" => Self::ToolStarted,
            "tool-finished" => Self::ToolFinished,
            "tool-failed" => Self::ToolFailed,
            "question-requested" => Self::QuestionRequested,
            "approval-requested" => Self::ApprovalRequested,
            "attention-resolved" => Self::AttentionResolved,
            "compaction-started" => Self::CompactionStarted,
            "compaction-finished" => Self::CompactionFinished,
            "subagent-started" => Self::SubagentStarted,
            "subagent-stopped" => Self::SubagentStopped,
            "turn-stopped" => Self::TurnStopped,
            "turn-failed" => Self::TurnFailed,
            "turn-canceled" => Self::TurnCanceled,
            "turn-interrupted" => Self::TurnInterrupted,
            "session-ended" => Self::SessionEnded,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HookCorrelation {
    pub correlation_id: String,
    pub visibility: AgentPresentationVisibility,
}

/// `AgentHookSanitizedObservation` — `version` is a literal and is validated
/// at parse, not stored.
#[derive(Clone, Debug, PartialEq)]
pub struct HookObservation {
    pub activity: Option<AgentActivity>,
    pub agent_key: Option<String>,
    pub event: HookObservationEvent,
    pub operation_key: Option<String>,
    pub session_key: String,
    pub turn_key: Option<String>,
}

/// `AgentHookRichObservationEnvelope` — `version` is a literal, validated at
/// parse. Field-wise equality replaces the TypeScript `JSON.stringify`
/// comparison: the strict boundary produces one canonical shape.
#[derive(Clone, Debug, PartialEq)]
pub struct HookEnvelope {
    pub adapter_id: String,
    pub correlations: Vec<HookCorrelation>,
    pub event_id: String,
    pub observation: HookObservation,
}

/// `AgentHookAdapterDefinition` — raw strings so corpus-injected adapter
/// objects (empty capabilities, synthetic provider ids) stay representable.
#[derive(Clone, Debug, PartialEq)]
pub struct AdapterDefinition {
    pub adapter_id: String,
    pub capabilities: Vec<String>,
    pub provider_id: String,
}

/// `AgentHookPresentationCorrelationHint` payload — `kind`/`version` are
/// literals validated at parse; `presentationId` and `generation` are stamped
/// by the consumer from channel identity, never by the producer.
#[derive(Clone, Debug, PartialEq)]
pub struct HookHint {
    pub correlation_id: String,
    pub event_id: String,
}

fn is_transport_identity(value: &str) -> bool {
    (TRANSPORT_IDENTITY_MIN..=TRANSPORT_IDENTITY_MAX).contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn is_hook_identity(value: &str) -> bool {
    (HOOK_IDENTITY_MIN..=HOOK_IDENTITY_MAX).contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b':' || b == b'-')
}

/// strictObject member count + key set. Returns the object map or None when
/// the value is not an object, carries extra keys, or misses a required key.
fn strict_object<'a>(
    value: &'a Value,
    keys: &[&str],
) -> Option<&'a serde_json::Map<String, Value>> {
    let object = value.as_object()?;
    if object.len() != keys.len() {
        return None;
    }
    for key in keys {
        if !object.contains_key(*key) {
            return None;
        }
    }
    Some(object)
}

fn nullable_string(value: &Value, check: fn(&str) -> bool) -> Option<Option<String>> {
    match value {
        Value::Null => Some(None),
        Value::String(s) if check(s) => Some(Some(s.clone())),
        _ => None,
    }
}

/// `AgentHookRichObservationEnvelopeSchema.safeParse` — `None` covers every
/// rejection (the boundary records no reason).
pub fn parse_envelope(value: &Value) -> Option<HookEnvelope> {
    let object = strict_object(
        value,
        &[
            "adapterId",
            "correlations",
            "eventId",
            "observation",
            "version",
        ],
    )?;
    let adapter_id = object["adapterId"].as_str()?;
    if !is_adapter_id(adapter_id) {
        return None;
    }
    if object["version"] != 3 {
        return None;
    }
    let event_id = object["eventId"].as_str()?;
    if !is_transport_identity(event_id) {
        return None;
    }
    let correlations_value = object["correlations"].as_array()?;
    if correlations_value.is_empty() || correlations_value.len() > MAX_CORRELATIONS_PER_EVENT {
        return None;
    }
    let mut correlations = Vec::with_capacity(correlations_value.len());
    for entry in correlations_value {
        let entry = strict_object(entry, &["correlationId", "visibility"])?;
        let correlation_id = entry["correlationId"].as_str()?;
        if !is_transport_identity(correlation_id) {
            return None;
        }
        let visibility = AgentPresentationVisibility::parse(entry["visibility"].as_str()?).ok()?;
        correlations.push(HookCorrelation {
            correlation_id: correlation_id.to_string(),
            visibility,
        });
    }
    // superRefine: correlation identities must be unique.
    for index in 1..correlations.len() {
        if correlations[..index]
            .iter()
            .any(|prior| prior.correlation_id == correlations[index].correlation_id)
        {
            return None;
        }
    }
    let observation = strict_object(
        &object["observation"],
        &[
            "activity",
            "agentKey",
            "event",
            "operationKey",
            "sessionKey",
            "turnKey",
            "version",
        ],
    )?;
    if observation["version"] != 1 {
        return None;
    }
    let event = HookObservationEvent::parse(observation["event"].as_str()?)?;
    let activity = match &observation["activity"] {
        Value::Null => None,
        Value::String(s) => Some(AgentActivity::parse(s).ok()?),
        _ => return None,
    };
    let agent_key = nullable_string(&observation["agentKey"], is_hook_identity)?;
    let operation_key = nullable_string(&observation["operationKey"], is_hook_identity)?;
    let turn_key = nullable_string(&observation["turnKey"], is_hook_identity)?;
    let session_key = observation["sessionKey"].as_str()?;
    if !is_hook_identity(session_key) {
        return None;
    }
    Some(HookEnvelope {
        adapter_id: adapter_id.to_string(),
        correlations,
        event_id: event_id.to_string(),
        observation: HookObservation {
            activity,
            agent_key,
            event,
            operation_key,
            session_key: session_key.to_string(),
            turn_key,
        },
    })
}

/// `AgentHookPresentationCorrelationHintSchema.safeParse`.
pub fn parse_hint(value: &Value) -> Option<HookHint> {
    let object = strict_object(value, &["correlationId", "eventId", "kind", "version"])?;
    if object["kind"] != "correlation" || object["version"] != 3 {
        return None;
    }
    let correlation_id = object["correlationId"].as_str()?;
    let event_id = object["eventId"].as_str()?;
    if !is_transport_identity(correlation_id) || !is_transport_identity(event_id) {
        return None;
    }
    Some(HookHint {
        correlation_id: correlation_id.to_string(),
        event_id: event_id.to_string(),
    })
}

fn is_adapter_id(value: &str) -> bool {
    matches!(
        value,
        "claude-code"
            | "codex"
            | "grok"
            | "opencode"
            | "kilo"
            | "cursor"
            | "pi"
            | "omp"
            | "gemini"
            | "github-copilot"
            | "cline"
            | "qwen-code"
    )
}

/// `resolveAgentHookAdapter(adapterId, event)` — the event-aware gate. Returns
/// `None` for an unknown adapter id or an event the adapter does not accept.
/// Registry ported verbatim from `resolveAgentHookAdapter.ts` (all twelve
/// adapters, per-event accepted sets, capabilities, provider ids).
pub fn resolve_adapter(adapter_id: &str, event: HookObservationEvent) -> Option<AdapterDefinition> {
    use AgentCapability::{
        Activity, Approvals, Questions, SessionLifecycle, Subagents, TurnLifecycle,
    };
    use HookObservationEvent::*;
    const CAP_SESSION_TURN_QA_AS: &[AgentCapability] = &[
        SessionLifecycle,
        TurnLifecycle,
        Questions,
        Approvals,
        Activity,
        Subagents,
    ];
    const CAP_TURN_QA: &[AgentCapability] = &[TurnLifecycle, Questions, Activity];
    let (accepted, capabilities, provider_id): (&[HookObservationEvent], &[AgentCapability], &str) =
        match adapter_id {
            "cursor" => (
                &[SessionIdentified, SessionStarted],
                &[SessionLifecycle],
                "cursor",
            ),
            "claude-code" => (
                &[
                    SessionStarted,
                    TurnStarted,
                    ToolStarted,
                    ToolFinished,
                    ToolFailed,
                    QuestionRequested,
                    ApprovalRequested,
                    CompactionStarted,
                    CompactionFinished,
                    SubagentStarted,
                    SubagentStopped,
                    TurnStopped,
                    TurnFailed,
                    SessionEnded,
                ],
                CAP_SESSION_TURN_QA_AS,
                "claude-code",
            ),
            "codex" => (
                &[
                    SessionStarted,
                    TurnStarted,
                    ToolStarted,
                    ToolFinished,
                    QuestionRequested,
                    ApprovalRequested,
                    CompactionStarted,
                    CompactionFinished,
                    SubagentStarted,
                    SubagentStopped,
                    TurnStopped,
                    SessionEnded,
                ],
                CAP_SESSION_TURN_QA_AS,
                "codex",
            ),
            "grok" => (
                &[
                    SessionStarted,
                    SessionIdle,
                    TurnStarted,
                    ToolStarted,
                    ToolFinished,
                    ToolFailed,
                    ApprovalRequested,
                    AttentionResolved,
                    CompactionStarted,
                    CompactionFinished,
                    SubagentStarted,
                    SubagentStopped,
                    TurnStopped,
                    TurnFailed,
                    TurnCanceled,
                    TurnInterrupted,
                    SessionEnded,
                ],
                &[
                    SessionLifecycle,
                    TurnLifecycle,
                    Approvals,
                    Activity,
                    Subagents,
                ],
                "grok",
            ),
            "opencode" | "kilo" => (
                &[
                    SessionStarted,
                    SessionIdle,
                    TurnStarted,
                    ToolStarted,
                    ToolFinished,
                    ToolFailed,
                    QuestionRequested,
                    ApprovalRequested,
                    AttentionResolved,
                    TurnFailed,
                    SessionEnded,
                ],
                &[
                    SessionLifecycle,
                    TurnLifecycle,
                    Questions,
                    Approvals,
                    Activity,
                ],
                adapter_id,
            ),
            "pi" => (
                &[
                    SessionStarted,
                    SessionIdle,
                    TurnStarted,
                    ToolStarted,
                    ToolFinished,
                    ToolFailed,
                    QuestionRequested,
                    AttentionResolved,
                    CompactionStarted,
                    CompactionFinished,
                    SessionEnded,
                ],
                &[SessionLifecycle, TurnLifecycle, Questions, Activity],
                "pi",
            ),
            "omp" => (
                &[
                    SessionStarted,
                    SessionIdle,
                    TurnStarted,
                    ToolStarted,
                    ToolFinished,
                    ToolFailed,
                    QuestionRequested,
                    ApprovalRequested,
                    AttentionResolved,
                    CompactionStarted,
                    CompactionFinished,
                    SessionEnded,
                ],
                &[
                    SessionLifecycle,
                    TurnLifecycle,
                    Questions,
                    Approvals,
                    Activity,
                ],
                "omp",
            ),
            "gemini" => (
                &[
                    SessionStarted,
                    SessionIdle,
                    TurnStarted,
                    ToolStarted,
                    ToolFinished,
                    ToolFailed,
                    QuestionRequested,
                    ApprovalRequested,
                    CompactionStarted,
                    TurnStopped,
                    SessionEnded,
                ],
                &[
                    SessionLifecycle,
                    TurnLifecycle,
                    Questions,
                    Approvals,
                    Activity,
                ],
                "gemini",
            ),
            "github-copilot" => (
                &[
                    SessionStarted,
                    SessionIdle,
                    TurnStarted,
                    ToolStarted,
                    ToolFinished,
                    ToolFailed,
                    QuestionRequested,
                    ApprovalRequested,
                    CompactionStarted,
                    TurnStopped,
                    TurnFailed,
                    SessionEnded,
                ],
                &[
                    SessionLifecycle,
                    TurnLifecycle,
                    Questions,
                    Approvals,
                    Activity,
                ],
                "github-copilot",
            ),
            "cline" => (
                &[
                    TurnStarted,
                    ToolStarted,
                    ToolFinished,
                    ToolFailed,
                    QuestionRequested,
                    TurnStopped,
                    TurnFailed,
                    TurnInterrupted,
                ],
                CAP_TURN_QA,
                "cline",
            ),
            "qwen-code" => (
                &[
                    SessionStarted,
                    SessionIdle,
                    TurnStarted,
                    ToolStarted,
                    ToolFinished,
                    ToolFailed,
                    QuestionRequested,
                    ApprovalRequested,
                    AttentionResolved,
                    CompactionStarted,
                    CompactionFinished,
                    SubagentStarted,
                    SubagentStopped,
                    TurnStopped,
                    TurnFailed,
                    SessionEnded,
                ],
                CAP_SESSION_TURN_QA_AS,
                "qwen-code",
            ),
            _ => return None,
        };
    if !accepted.contains(&event) {
        return None;
    }
    Some(AdapterDefinition {
        adapter_id: adapter_id.to_string(),
        capabilities: capabilities
            .iter()
            .map(|capability| capability.as_str().to_string())
            .collect(),
        provider_id: provider_id.to_string(),
    })
}
