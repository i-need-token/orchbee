//! Production port of the Desktop agent-hook observation contract and the
//! adapter event gate — `agent-hook-observation-contract.ts` and
//! `resolveAgentHookAdapter.ts` (fingerprints recorded in the hook fixture).
//!
//! Parsing mirrors the Zod `strictObject` boundary exactly: every declared key
//! is required, any extra key fails, `null` is accepted only where the schema
//! is `.nullable()`, and identity strings keep their length/regex bounds.
//! Every `version` member carries one generation of the `agent-hook` delivery,
//! and the optional `extensions` object is the only open slot.

use orchbee_agent_observation_protocol::AgentRunToken;
use orchbee_connection_handshake::{Delivery, Extensions, Generation};
use serde::Deserialize;
use serde_json::{Map, Value};

use super::{AgentActivity, AgentCapability, AgentPresentationVisibility};

pub const MAX_CORRELATIONS_PER_EVENT: usize = 32;
pub const MAX_OBSERVATION_BODY_BYTES: usize = 16 * 1024;

const TRANSPORT_IDENTITY_MIN: usize = 16;
const TRANSPORT_IDENTITY_MAX: usize = 64;
const HOOK_IDENTITY_MIN: usize = 1;
const HOOK_IDENTITY_MAX: usize = 128;

/// The acknowledgement of an accepted envelope of `generation`, or `None` for a generation this
/// build never defined. A released generation's acknowledgement bytes never change.
const fn acknowledgement_of(generation: u16) -> Option<&'static str> {
    match generation {
        1 => Some("{\"accepted\":true,\"version\":1}\n"),
        _ => None,
    }
}

// Every generation the registry admits has its own acknowledgement, so widening the
// `agent-hook` window without defining one fails the build.
const _: () = {
    let window = Delivery::AgentHook.supported_generations();
    let mut generation = window.lowest().get();
    loop {
        assert!(
            acknowledgement_of(generation).is_some(),
            "every agent-hook generation in the window needs an acknowledgement"
        );
        if generation == window.highest().get() {
            break;
        }
        generation += 1;
    }
};

/// A generation of the `agent-hook` delivery inside this build's window, with the
/// acknowledgement Server writes for an accepted envelope of that generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HookGeneration {
    acknowledgement: &'static str,
    generation: Generation,
}

impl HookGeneration {
    /// The generation a `version` member carries: a plain integer inside the window.
    fn admit(value: &Value) -> Option<Self> {
        let generation = Generation::new(u16::try_from(value.as_u64()?).ok()?)?;
        if !Delivery::AgentHook
            .supported_generations()
            .contains(generation)
        {
            return None;
        }
        Some(Self {
            acknowledgement: acknowledgement_of(generation.get())?,
            generation,
        })
    }

    /// The bytes that acknowledge an accepted envelope of this generation.
    pub fn acknowledgement(self) -> &'static str {
        self.acknowledgement
    }
}

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

/// `AgentHookSanitizedObservation` — `version` must equal the envelope's
/// generation; it is validated at parse, not stored.
#[derive(Clone, Debug, PartialEq)]
pub struct HookObservation {
    pub activity: Option<AgentActivity>,
    pub agent_key: Option<String>,
    pub event: HookObservationEvent,
    pub operation_key: Option<String>,
    pub session_key: String,
    pub turn_key: Option<String>,
}

/// `AgentHookRichObservationEnvelope`. Field-wise equality replaces the
/// TypeScript `JSON.stringify` comparison: the strict boundary produces one
/// canonical shape, and admitted extensions are not retained.
#[derive(Clone, Debug, PartialEq)]
pub struct HookEnvelope {
    pub adapter_id: String,
    pub correlations: Vec<HookCorrelation>,
    pub event_id: String,
    /// The generation both the envelope's and its observation's `version` carry.
    pub generation: HookGeneration,
    pub observation: HookObservation,
    /// The Server-minted run the hook's Agent inherited; absent from older integrations.
    pub run_token: Option<AgentRunToken>,
}

/// `AgentHookAdapterDefinition` — raw strings so corpus-injected adapter
/// objects (empty capabilities, synthetic provider ids) stay representable.
#[derive(Clone, Debug, PartialEq)]
pub struct AdapterDefinition {
    pub adapter_id: String,
    pub capabilities: Vec<String>,
    pub provider_id: String,
}

/// `AgentHookPresentationCorrelationHint` payload — `kind` is a literal and
/// `version` a generation inside the window, both validated at parse;
/// `presentationId` and `generation` are stamped by the consumer from channel
/// identity, never by the producer.
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
    strict_object_with_optional(value, keys, &[])
}

/// strictObject whose `optional` keys may be absent; any other extra key still fails.
fn strict_object_with_optional<'a>(
    value: &'a Value,
    keys: &[&str],
    optional: &[&str],
) -> Option<&'a serde_json::Map<String, Value>> {
    let object = value.as_object()?;
    let present = optional
        .iter()
        .filter(|key| object.contains_key(**key))
        .count();
    if object.len() != keys.len() + present {
        return None;
    }
    for key in keys {
        if !object.contains_key(*key) {
            return None;
        }
    }
    Some(object)
}

/// Whether the optional `extensions` member is well formed: an object whose names follow the
/// capability-name grammar and whose values stay within the nesting bound. This build knows no
/// extension, so every admitted name is ignored.
fn extensions_are_well_formed(object: &Map<String, Value>) -> bool {
    object
        .get("extensions")
        .is_none_or(|extensions| Extensions::deserialize(extensions).is_ok())
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
    let object = strict_object_with_optional(
        value,
        &[
            "adapterId",
            "correlations",
            "eventId",
            "observation",
            "version",
        ],
        &["extensions", "runToken"],
    )?;
    if !extensions_are_well_formed(object) {
        return None;
    }
    // Present means well formed: a malformed token rejects the envelope; absence is the fallback.
    let run_token = match object.get("runToken") {
        None => None,
        Some(token) => Some(AgentRunToken::parse(token.as_str()?).ok()?),
    };
    let adapter_id = object["adapterId"].as_str()?;
    if !is_adapter_id(adapter_id) {
        return None;
    }
    let generation = HookGeneration::admit(&object["version"])?;
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
    if HookGeneration::admit(&observation["version"]) != Some(generation) {
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
        generation,
        observation: HookObservation {
            activity,
            agent_key,
            event,
            operation_key,
            session_key: session_key.to_string(),
            turn_key,
        },
        run_token,
    })
}

/// `AgentHookPresentationCorrelationHintSchema.safeParse`.
pub fn parse_hint(value: &Value) -> Option<HookHint> {
    let object = strict_object_with_optional(
        value,
        &["correlationId", "eventId", "kind", "version"],
        &["extensions"],
    )?;
    if object["kind"] != "correlation"
        || HookGeneration::admit(&object["version"]).is_none()
        || !extensions_are_well_formed(object)
    {
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
            | "qodercli"
            | "antigravity-cli"
            | "mastracode"
            | "droid"
            | "kimi"
            | "letta"
            | "devin"
            | "hermes"
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
/// The closed registry associates each adapter with its accepted events,
/// capabilities, and provider identity.
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
            "mastracode" => (
                &[
                    SessionIdentified,
                    SessionEnded,
                    TurnStarted,
                    TurnStopped,
                    TurnCanceled,
                    TurnFailed,
                    ToolStarted,
                    ToolFinished,
                    ToolFailed,
                    ApprovalRequested,
                    AttentionResolved,
                    SubagentStarted,
                    SubagentStopped,
                ],
                &[
                    SessionLifecycle,
                    TurnLifecycle,
                    Activity,
                    Approvals,
                    Subagents,
                ],
                "mastracode",
            ),
            "antigravity-cli" => (&[SessionIdentified], &[SessionLifecycle], "antigravity"),
            "letta" => (&[SessionIdentified], &[SessionLifecycle], "letta"),
            "devin" => (&[SessionIdentified], &[SessionLifecycle], "devin"),
            "hermes" => (&[SessionIdentified], &[SessionLifecycle], "hermes"),
            "kimi" => (
                &[SessionIdentified, SessionEnded],
                &[SessionLifecycle],
                "kimi",
            ),
            "droid" => (&[SessionIdentified], &[SessionLifecycle], "droid"),
            "qodercli" => (&[SessionIdentified], &[SessionLifecycle], "qodercli"),
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
