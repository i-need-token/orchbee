//! Canonical Agent value types, identity validation, UTF-16 ordering, and the
//! raw-to-typed construction that mirrors the TypeScript schema parse points.
//!
//! Raw structs carry event-facing fields exactly as the schema-less TypeScript
//! event union does (strings and f64 numbers), so reducer guards stay reachable
//! in the same order. Typed snapshots are produced by `try_from_raw`, which
//! performs the same checks as the zod schemas including the `superRefine`
//! invariants. Raw construction and validation are the production admission
//! path: envelopes are parsed to typed snapshots, and reads mirror them back.

use std::cmp::Ordering;
use std::collections::BTreeSet;

pub const MAX_AGENT_SESSIONS: usize = 512;
pub const MAX_AGENT_PENDING_REQUESTS: usize = 128;
pub const MAX_AGENT_PRESENTATION_ATTACHMENTS: usize = 32;
pub const MAX_AGENT_CAPABILITIES: usize = 10;
pub const MAX_PRESENTATION_ID_UTF16_UNITS: usize = 256;
pub const MAX_OPAQUE_IDENTITY_UNITS: usize = 128;
pub const MAX_PROVIDER_ID_UNITS: usize = 64;
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

/// `Number.isSafeInteger(value) && value >= 0`.
pub fn valid_ordered_identity(value: f64) -> bool {
    value.is_finite() && value.fract() == 0.0 && value >= 0.0 && value <= MAX_SAFE_INTEGER as f64
}

/// `compareAgentIdentity`: JavaScript string comparison is a UTF-16 code-unit
/// comparison. Rust `str::cmp` would compare Unicode scalar values instead, so
/// the comparator iterates `encode_utf16` units explicitly.
pub fn compare_agent_identity(left: &str, right: &str) -> Ordering {
    left.encode_utf16().cmp(right.encode_utf16())
}

#[derive(Clone, Debug)]
pub struct AgentModelError(pub &'static str);

impl std::fmt::Display for AgentModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for AgentModelError {}

fn utf16_len(value: &str) -> usize {
    value.encode_utf16().count()
}

fn is_opaque_identity(value: &str) -> bool {
    let len = utf16_len(value);
    (1..=MAX_OPAQUE_IDENTITY_UNITS).contains(&len)
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'-'))
}

fn is_provider_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    let len = utf16_len(value);
    if !(1..=MAX_PROVIDER_ID_UNITS).contains(&len) || bytes.is_empty() {
        return false;
    }
    let lower_alnum = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit();
    if !lower_alnum(bytes[0]) || !lower_alnum(bytes[bytes.len() - 1]) {
        return false;
    }
    bytes
        .iter()
        .all(|&b| lower_alnum(b) || matches!(b, b'.' | b'-'))
}

/// Opaque ASCII identity shared by session/turn/request/event/execution-location
/// ids. One newtype is used because the TypeScript brands share one schema.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpaqueId(String);

impl OpaqueId {
    pub fn new(value: &str) -> Result<Self, AgentModelError> {
        if is_opaque_identity(value) {
            Ok(Self(value.to_string()))
        } else {
            Err(AgentModelError("invalid opaque identity"))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// `presentationId`: 1..=256 UTF-16 code units. Lone surrogates cannot exist in
/// a Rust `String`, so the TypeScript well-formedness refinement is discharged
/// by construction; only the code-unit bound is enforced here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentationId(String);

impl PresentationId {
    pub fn new(value: &str) -> Result<Self, AgentModelError> {
        let len = utf16_len(value);
        if (1..=MAX_PRESENTATION_ID_UTF16_UNITS).contains(&len) {
            Ok(Self(value.to_string()))
        } else {
            Err(AgentModelError("invalid presentation identity"))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentProviderId(String);

impl AgentProviderId {
    pub fn new(value: &str) -> Result<Self, AgentModelError> {
        if is_provider_id(value) {
            Ok(Self(value.to_string()))
        } else {
            Err(AgentModelError("invalid provider identity"))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

macro_rules! token_enum {
    ($name:ident { $($variant:ident => $text:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
                        pub fn parse(value: &str) -> Result<Self, AgentModelError> {
                match value {
                    $($text => Ok(Self::$variant),)+
                    _ => Err(AgentModelError(concat!("invalid ", stringify!($name)))),
                }
            }

            pub fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $text,)+
                }
            }
        }
    };
}

token_enum!(AgentCapability {
    SessionLifecycle => "sessionLifecycle",
    TurnLifecycle => "turnLifecycle",
    Questions => "questions",
    Approvals => "approvals",
    Authentication => "authentication",
    Activity => "activity",
    Subagents => "subagents",
    Progress => "progress",
});

token_enum!(AgentActivity {
    Thinking => "thinking",
    Tool => "tool",
    Editing => "editing",
    Command => "command",
    Subagent => "subagent",
    Compacting => "compacting",
    Retrying => "retrying",
    Other => "other",
});

token_enum!(AgentPresentationVisibility {
    Visible => "visible",
    Hidden => "hidden",
    Unknown => "unknown",
});

token_enum!(AgentFreshness {
    Fresh => "fresh",
    Stale => "stale",
    Disconnected => "disconnected",
});

token_enum!(AgentObservationFidelity {
    Rich => "rich",
    Basic => "basic",
    Heuristic => "heuristic",
});

token_enum!(AgentObservationSource {
    Native => "native",
    Hook => "hook",
    Terminal => "terminal",
    Heuristic => "heuristic",
});

token_enum!(AgentTurnPhase {
    Queued => "queued",
    Working => "working",
    WaitingExternal => "waiting-external",
});

token_enum!(AgentAttentionKind {
    Question => "question",
    Approval => "approval",
    Authentication => "authentication",
    Unclassified => "unclassified",
});

token_enum!(AgentTurnOutcomeKind {
    Succeeded => "succeeded",
    Failed => "failed",
    Canceled => "canceled",
    Interrupted => "interrupted",
});

token_enum!(AgentLifecycle {
    Live => "live",
    Ended => "ended",
});

pub const ATTENTION_ACTION_PATH: &str = "navigate-only";

// ---- typed snapshots ----

#[derive(Clone, Debug)]
pub struct AgentObservation {
    pub fidelity: AgentObservationFidelity,
    pub freshness: AgentFreshness,
    pub generation: u64,
    pub sequence: u64,
    pub source: AgentObservationSource,
}

#[derive(Clone, Debug)]
pub struct AgentExecutionLocation {
    pub execution_location_id: OpaqueId,
}

#[derive(Clone, Debug)]
pub struct AgentPresentationAttachment {
    pub generation: OpaqueId,
    pub navigable: bool,
    pub presentation_id: PresentationId,
    pub visibility: AgentPresentationVisibility,
}

#[derive(Clone, Debug)]
pub struct AgentAttentionRequest {
    pub kind: AgentAttentionKind,
    pub request_id: OpaqueId,
    pub turn_id: Option<OpaqueId>,
}

#[derive(Clone, Debug)]
pub struct AgentTurnOutcome {
    pub event_id: OpaqueId,
    pub kind: AgentTurnOutcomeKind,
    pub turn_id: OpaqueId,
}

#[derive(Clone, Debug)]
pub enum AgentTurnState {
    Unknown,
    Idle,
    Active {
        activity: Option<AgentActivity>,
        phase: AgentTurnPhase,
        turn_id: OpaqueId,
    },
}

#[derive(Clone, Debug)]
pub struct AgentSessionSnapshot {
    pub activity_hint: Option<AgentActivity>,
    pub capabilities: Vec<AgentCapability>,
    pub execution_location: AgentExecutionLocation,
    pub last_outcome: Option<AgentTurnOutcome>,
    pub lifecycle: AgentLifecycle,
    pub observation: AgentObservation,
    pub parent_session_id: Option<OpaqueId>,
    pub pending_requests: Vec<AgentAttentionRequest>,
    pub presentation_attachments: Vec<AgentPresentationAttachment>,
    pub provider_id: AgentProviderId,
    pub session_id: OpaqueId,
    pub turn: AgentTurnState,
}

#[derive(Clone, Debug)]
pub struct AgentRuntimeSnapshot {
    pub revision: u64,
    pub sessions: Vec<AgentSessionSnapshot>,
}

impl AgentRuntimeSnapshot {
    pub fn create() -> Self {
        Self {
            revision: 0,
            sessions: Vec::new(),
        }
    }
}

// ---- raw mirrors of the schema-less TypeScript shapes ----

/// Presence tracking for raw object keys where absent, `null`, and a value
/// are observably different states under the TypeScript `strictObject`
/// members: required-nullable keys (`turn.activity`, `request.turnId`,
/// `activityHint`, `lastOutcome`, `parentSessionId` and their event-level
/// feeds) and union-member residue keys (`activity`/`phase`/`turnId` on
/// `unknown`/`idle` turns, which are forbidden in any form).
///
/// `Other` marks a present value whose JSON type the field's schema cannot
/// read; only the private fixture decoder can produce it. Keys whose schema
/// requires a non-nullable value keep plain raw fields — a missing key is
/// unrepresentable there and remains the decoder's documented obligation.
#[derive(Clone, Debug)]
pub enum RawPresence<T> {
    Absent,
    Null,
    /// Unrepresentable present value — only the private fixture decoder can
    /// produce it; no production ingress constructs it.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "fixture-decoder-only variant; no production ingress constructs it"
        )
    )]
    Other,
    Value(T),
}

impl<T> RawPresence<T> {
    /// `raw !== null` — every present form (including `Absent`, which reads as
    /// `undefined` in TypeScript) satisfies the inequality.
    pub fn is_not_null(&self) -> bool {
        !matches!(self, Self::Null)
    }
}

#[derive(Clone, Debug)]
pub struct RawObservation {
    pub fidelity: String,
    pub freshness: String,
    pub generation: f64,
    pub sequence: f64,
    pub source: String,
}

#[derive(Clone, Debug)]
pub struct RawExecutionLocation {
    pub execution_location_id: String,
}

#[derive(Clone, Debug)]
pub struct RawAttachment {
    pub generation: String,
    pub navigable: bool,
    pub presentation_id: String,
    pub visibility: String,
}

#[derive(Clone, Debug)]
pub struct RawAttentionRequest {
    pub action_path: String,
    pub kind: String,
    pub request_id: String,
    /// Required-nullable key: `Absent`/`Other` fail candidate validation.
    pub turn_id: RawPresence<String>,
}

#[derive(Clone, Debug)]
pub struct RawTurnOutcome {
    pub event_id: String,
    pub kind: String,
    pub turn_id: String,
}

#[derive(Clone, Debug)]
pub struct RawTurnState {
    /// `active` member: required-nullable. `unknown`/`idle`: forbidden key.
    pub activity: RawPresence<String>,
    pub kind: String,
    /// `active` member: required string. `unknown`/`idle`: forbidden key.
    pub phase: RawPresence<String>,
    /// `active` member: required string. `unknown`/`idle`: forbidden key.
    pub turn_id: RawPresence<String>,
}

#[derive(Clone, Debug)]
pub struct RawSessionSnapshot {
    pub activity_hint: RawPresence<String>,
    pub capabilities: Vec<String>,
    pub execution_location: RawExecutionLocation,
    pub last_outcome: RawPresence<RawTurnOutcome>,
    pub lifecycle: String,
    pub observation: RawObservation,
    pub parent_session_id: RawPresence<String>,
    pub pending_requests: Vec<RawAttentionRequest>,
    pub presentation_attachments: Vec<RawAttachment>,
    pub provider_id: String,
    pub session_id: String,
    pub turn: RawTurnState,
}

impl AgentSessionSnapshot {
    /// Convert the typed snapshot back to its raw mirror so transitions can
    /// rebuild candidates exactly like the TypeScript structural spreads.
    pub fn to_raw(&self) -> RawSessionSnapshot {
        RawSessionSnapshot {
            activity_hint: match self.activity_hint {
                Some(activity) => RawPresence::Value(activity.as_str().to_string()),
                None => RawPresence::Null,
            },
            capabilities: self
                .capabilities
                .iter()
                .map(|capability| capability.as_str().to_string())
                .collect(),
            execution_location: RawExecutionLocation {
                execution_location_id: self
                    .execution_location
                    .execution_location_id
                    .as_str()
                    .to_string(),
            },
            last_outcome: match self.last_outcome.as_ref() {
                Some(outcome) => RawPresence::Value(RawTurnOutcome {
                    event_id: outcome.event_id.as_str().to_string(),
                    kind: outcome.kind.as_str().to_string(),
                    turn_id: outcome.turn_id.as_str().to_string(),
                }),
                None => RawPresence::Null,
            },
            lifecycle: self.lifecycle.as_str().to_string(),
            observation: RawObservation {
                fidelity: self.observation.fidelity.as_str().to_string(),
                freshness: self.observation.freshness.as_str().to_string(),
                generation: self.observation.generation as f64,
                sequence: self.observation.sequence as f64,
                source: self.observation.source.as_str().to_string(),
            },
            parent_session_id: match self.parent_session_id.as_ref() {
                Some(id) => RawPresence::Value(id.as_str().to_string()),
                None => RawPresence::Null,
            },
            pending_requests: self
                .pending_requests
                .iter()
                .map(|request| RawAttentionRequest {
                    action_path: ATTENTION_ACTION_PATH.to_string(),
                    kind: request.kind.as_str().to_string(),
                    request_id: request.request_id.as_str().to_string(),
                    turn_id: match request.turn_id.as_ref() {
                        Some(id) => RawPresence::Value(id.as_str().to_string()),
                        None => RawPresence::Null,
                    },
                })
                .collect(),
            presentation_attachments: self
                .presentation_attachments
                .iter()
                .map(|attachment| RawAttachment {
                    generation: attachment.generation.as_str().to_string(),
                    navigable: attachment.navigable,
                    presentation_id: attachment.presentation_id.as_str().to_string(),
                    visibility: attachment.visibility.as_str().to_string(),
                })
                .collect(),
            provider_id: self.provider_id.as_str().to_string(),
            session_id: self.session_id.as_str().to_string(),
            turn: match &self.turn {
                AgentTurnState::Unknown => RawTurnState {
                    activity: RawPresence::Absent,
                    kind: "unknown".to_string(),
                    phase: RawPresence::Absent,
                    turn_id: RawPresence::Absent,
                },
                AgentTurnState::Idle => RawTurnState {
                    activity: RawPresence::Absent,
                    kind: "idle".to_string(),
                    phase: RawPresence::Absent,
                    turn_id: RawPresence::Absent,
                },
                AgentTurnState::Active {
                    activity,
                    phase,
                    turn_id,
                } => RawTurnState {
                    activity: match activity {
                        Some(activity) => RawPresence::Value(activity.as_str().to_string()),
                        None => RawPresence::Null,
                    },
                    kind: "active".to_string(),
                    phase: RawPresence::Value(phase.as_str().to_string()),
                    turn_id: RawPresence::Value(turn_id.as_str().to_string()),
                },
            },
        }
    }

    /// The `AgentSessionSnapshotSchema` equivalent: field validation plus the
    /// `superRefine` invariants, in the same check order.
    pub fn try_from_raw(raw: RawSessionSnapshot) -> Result<Self, AgentModelError> {
        let session = Self {
            activity_hint: match &raw.activity_hint {
                RawPresence::Value(activity) => Some(AgentActivity::parse(activity)?),
                RawPresence::Null => None,
                RawPresence::Absent | RawPresence::Other => {
                    return Err(AgentModelError("missing required activityHint"));
                }
            },
            capabilities: raw
                .capabilities
                .iter()
                .map(|capability| AgentCapability::parse(capability))
                .collect::<Result<Vec<_>, _>>()?,
            execution_location: AgentExecutionLocation {
                execution_location_id: OpaqueId::new(
                    &raw.execution_location.execution_location_id,
                )?,
            },
            last_outcome: match &raw.last_outcome {
                RawPresence::Value(outcome) => Some(AgentTurnOutcome {
                    event_id: OpaqueId::new(&outcome.event_id)?,
                    kind: AgentTurnOutcomeKind::parse(&outcome.kind)?,
                    turn_id: OpaqueId::new(&outcome.turn_id)?,
                }),
                RawPresence::Null => None,
                RawPresence::Absent | RawPresence::Other => {
                    return Err(AgentModelError("missing required lastOutcome"));
                }
            },
            lifecycle: AgentLifecycle::parse(&raw.lifecycle)?,
            observation: AgentObservation {
                fidelity: AgentObservationFidelity::parse(&raw.observation.fidelity)?,
                freshness: AgentFreshness::parse(&raw.observation.freshness)?,
                generation: ordered_identity_to_u64(raw.observation.generation)?,
                sequence: ordered_identity_to_u64(raw.observation.sequence)?,
                source: AgentObservationSource::parse(&raw.observation.source)?,
            },
            parent_session_id: match &raw.parent_session_id {
                RawPresence::Value(id) => Some(OpaqueId::new(id)?),
                RawPresence::Null => None,
                RawPresence::Absent | RawPresence::Other => {
                    return Err(AgentModelError("missing required parentSessionId"));
                }
            },
            pending_requests: raw
                .pending_requests
                .iter()
                .map(|request| {
                    if request.action_path != ATTENTION_ACTION_PATH {
                        return Err(AgentModelError("invalid attention action path"));
                    }
                    Ok(AgentAttentionRequest {
                        kind: AgentAttentionKind::parse(&request.kind)?,
                        request_id: OpaqueId::new(&request.request_id)?,
                        turn_id: match &request.turn_id {
                            RawPresence::Value(turn_id) => Some(OpaqueId::new(turn_id)?),
                            RawPresence::Null => None,
                            RawPresence::Absent | RawPresence::Other => {
                                return Err(AgentModelError("missing required request turnId"));
                            }
                        },
                    })
                })
                .collect::<Result<Vec<_>, AgentModelError>>()?,
            presentation_attachments: raw
                .presentation_attachments
                .iter()
                .map(|attachment| {
                    Ok(AgentPresentationAttachment {
                        generation: OpaqueId::new(&attachment.generation)?,
                        navigable: attachment.navigable,
                        presentation_id: PresentationId::new(&attachment.presentation_id)?,
                        visibility: AgentPresentationVisibility::parse(&attachment.visibility)?,
                    })
                })
                .collect::<Result<Vec<_>, AgentModelError>>()?,
            provider_id: AgentProviderId::new(&raw.provider_id)?,
            session_id: OpaqueId::new(&raw.session_id)?,
            turn: match raw.turn.kind.as_str() {
                // zod strictObject: non-active kinds admit no extra keys in
                // any form — a present `null` residue also fails the member.
                "unknown" | "idle"
                    if !matches!(raw.turn.activity, RawPresence::Absent)
                        || !matches!(raw.turn.phase, RawPresence::Absent)
                        || !matches!(raw.turn.turn_id, RawPresence::Absent) =>
                {
                    return Err(AgentModelError("non-active turn must not carry fields"));
                }
                "unknown" => AgentTurnState::Unknown,
                "idle" => AgentTurnState::Idle,
                "active" => AgentTurnState::Active {
                    activity: match &raw.turn.activity {
                        RawPresence::Value(activity) => Some(AgentActivity::parse(activity)?),
                        RawPresence::Null => None,
                        RawPresence::Absent | RawPresence::Other => {
                            return Err(AgentModelError("active turn requires an activity"));
                        }
                    },
                    phase: AgentTurnPhase::parse(match &raw.turn.phase {
                        RawPresence::Value(phase) => phase,
                        _ => return Err(AgentModelError("active turn requires a phase")),
                    })?,
                    turn_id: OpaqueId::new(match &raw.turn.turn_id {
                        RawPresence::Value(turn_id) => turn_id,
                        _ => return Err(AgentModelError("active turn requires a turnId")),
                    })?,
                },
                _ => return Err(AgentModelError("invalid turn kind")),
            },
        };
        session.validate()?;
        Ok(session)
    }

    /// The `AgentSessionSnapshotSchema.superRefine` invariants.
    fn validate(&self) -> Result<(), AgentModelError> {
        if self.capabilities.len() > MAX_AGENT_CAPABILITIES {
            return Err(AgentModelError("Agent capabilities exceed the bound"));
        }
        if self.pending_requests.len() > MAX_AGENT_PENDING_REQUESTS {
            return Err(AgentModelError("Agent pending requests exceed the bound"));
        }
        if self.presentation_attachments.len() > MAX_AGENT_PRESENTATION_ATTACHMENTS {
            return Err(AgentModelError(
                "Agent presentation attachments exceed the bound",
            ));
        }
        if self
            .capabilities
            .iter()
            .map(|capability| capability.as_str())
            .collect::<BTreeSet<_>>()
            .len()
            != self.capabilities.len()
        {
            return Err(AgentModelError("Agent capabilities must be unique"));
        }
        if self
            .presentation_attachments
            .iter()
            .map(|attachment| {
                (
                    attachment.presentation_id.as_str(),
                    attachment.generation.as_str(),
                )
            })
            .collect::<BTreeSet<_>>()
            .len()
            != self.presentation_attachments.len()
        {
            return Err(AgentModelError(
                "Agent presentation attachments must be unique",
            ));
        }
        if self
            .presentation_attachments
            .iter()
            .filter(|attachment| attachment.navigable)
            .map(|attachment| attachment.presentation_id.as_str())
            .collect::<BTreeSet<_>>()
            .len()
            != self
                .presentation_attachments
                .iter()
                .filter(|attachment| attachment.navigable)
                .count()
        {
            return Err(AgentModelError(
                "An Agent Session can have only one navigable generation per presentation",
            ));
        }
        if self.activity_hint.is_some()
            && (!self.capabilities.contains(&AgentCapability::Activity)
                || self.observation.source != AgentObservationSource::Terminal)
        {
            return Err(AgentModelError(
                "An Agent activity hint requires terminal-scoped activity capability",
            ));
        }
        if self
            .pending_requests
            .iter()
            .map(|request| request.request_id.as_str())
            .collect::<BTreeSet<_>>()
            .len()
            != self.pending_requests.len()
        {
            return Err(AgentModelError("Agent request identities must be unique"));
        }
        if self.lifecycle == AgentLifecycle::Ended
            && (self.activity_hint.is_some()
                || matches!(self.turn, AgentTurnState::Active { .. })
                || !self.pending_requests.is_empty())
        {
            return Err(AgentModelError(
                "An ended Agent Session cannot retain active work, activity hints, or requests",
            ));
        }
        for request in &self.pending_requests {
            if let Some(turn_id) = &request.turn_id {
                let belongs = match &self.turn {
                    AgentTurnState::Active {
                        turn_id: active, ..
                    } => active == turn_id,
                    _ => false,
                };
                if !belongs {
                    return Err(AgentModelError(
                        "A Turn-scoped request must belong to the active Turn",
                    ));
                }
            }
        }
        Ok(())
    }
}

impl AgentRuntimeSnapshot {
    /// Validate runtime-level invariants on an already-typed snapshot; mirrors
    /// the runtime `superRefine` checks.
    pub fn validate_runtime(&self) -> Result<(), AgentModelError> {
        if self.revision > MAX_SAFE_INTEGER {
            return Err(AgentModelError(
                "Agent runtime revision exceeds safe integer",
            ));
        }
        if self.sessions.len() > MAX_AGENT_SESSIONS {
            return Err(AgentModelError("Agent Sessions exceed the bound"));
        }
        if self
            .sessions
            .iter()
            .map(|session| session.session_id.as_str())
            .collect::<BTreeSet<_>>()
            .len()
            != self.sessions.len()
        {
            return Err(AgentModelError("Agent Session identities must be unique"));
        }
        for session in &self.sessions {
            let Some(mut parent) = session.parent_session_id.as_ref() else {
                continue;
            };
            if !self
                .sessions
                .iter()
                .any(|candidate| &candidate.session_id == parent)
            {
                return Err(AgentModelError(
                    "An Agent Session parent must exist in the same snapshot",
                ));
            }
            let mut visited = BTreeSet::from([session.session_id.as_str()]);
            loop {
                if !visited.insert(parent.as_str()) {
                    return Err(AgentModelError("Agent Session parentage must be acyclic"));
                }
                match self
                    .sessions
                    .iter()
                    .find(|candidate| &candidate.session_id == parent)
                    .and_then(|candidate| candidate.parent_session_id.as_ref())
                {
                    Some(next) => parent = next,
                    None => break,
                }
            }
        }
        Ok(())
    }
}

fn ordered_identity_to_u64(value: f64) -> Result<u64, AgentModelError> {
    if valid_ordered_identity(value) {
        Ok(value as u64)
    } else {
        Err(AgentModelError("invalid nonnegative safe integer"))
    }
}
