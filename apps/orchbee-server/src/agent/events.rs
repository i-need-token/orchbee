//! The thirteen `AgentObservationEvent` kinds and the four `AgentSemanticEvent`
//! kinds. Event fields mirror the schema-less TypeScript union: raw strings and
//! `f64` numbers so every reducer guard stays reachable in source order.

use super::model::{RawAttachment, RawTurnOutcome, RawTurnState};
use super::{RawAttentionRequest, RawExecutionLocation, RawObservation, RawPresence};

#[derive(Clone, Debug)]
pub enum AgentObservationEvent {
    SessionObserved {
        capabilities: Vec<String>,
        #[cfg_attr(
            not(test),
            expect(
                dead_code,
                reason = "shared event provenance validated by conformance; Server does not read this field"
            )
        )]
        event_id: String,
        execution_location: RawExecutionLocation,
        observation: RawObservation,
        /// `AgentSessionId | null` — an absent key is not `null` here: the
        /// `!== null` parent guards treat it as a (missing) value.
        parent_session_id: RawPresence<String>,
        presentation_attachments: Vec<RawAttachment>,
        provider_id: String,
        session_id: String,
    },
    TurnStarted {
        activity: RawPresence<String>,
        #[cfg_attr(
            not(test),
            expect(
                dead_code,
                reason = "shared event provenance validated by conformance; Server does not read this field"
            )
        )]
        event_id: String,
        generation: f64,
        phase: String,
        sequence: f64,
        session_id: String,
        turn_id: String,
    },
    TurnActivityChanged {
        activity: RawPresence<String>,
        #[cfg_attr(
            not(test),
            expect(
                dead_code,
                reason = "shared event provenance validated by conformance; Server does not read this field"
            )
        )]
        event_id: String,
        generation: f64,
        phase: String,
        sequence: f64,
        session_id: String,
        turn_id: String,
    },
    AttentionRequested {
        event_id: String,
        generation: f64,
        request: RawAttentionRequest,
        sequence: f64,
        session_id: String,
    },
    AttentionResolved {
        event_id: String,
        generation: f64,
        request_id: String,
        sequence: f64,
        session_id: String,
    },
    TurnFinished {
        event_id: String,
        generation: f64,
        outcome: String,
        sequence: f64,
        session_id: String,
        turn_id: String,
    },
    SessionIdleObserved {
        #[cfg_attr(
            not(test),
            expect(
                dead_code,
                reason = "shared event provenance validated by conformance; Server does not read this field"
            )
        )]
        event_id: String,
        generation: f64,
        sequence: f64,
        session_id: String,
    },
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "Desktop produces this shared variant; Server retains cross-language conformance"
        )
    )]
    ObservationBecameStale {
        event_id: String,
        generation: f64,
        sequence: f64,
        session_id: String,
    },
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "Desktop produces this shared variant; Server retains cross-language conformance"
        )
    )]
    ObservationReconnected {
        activity_hint: RawPresence<String>,
        capabilities: Vec<String>,
        event_id: String,
        generation: f64,
        last_outcome: RawPresence<RawTurnOutcome>,
        pending_requests: Vec<RawAttentionRequest>,
        sequence: f64,
        session_id: String,
        turn: RawTurnState,
    },
    PresentationAttached {
        attachment: RawAttachment,
        #[cfg_attr(
            not(test),
            expect(
                dead_code,
                reason = "shared event provenance validated by conformance; Server does not read this field"
            )
        )]
        event_id: String,
        generation: f64,
        sequence: f64,
        session_id: String,
    },
    PresentationVisibilityChanged {
        #[cfg_attr(
            not(test),
            expect(
                dead_code,
                reason = "shared event provenance validated by conformance; Server does not read this field"
            )
        )]
        event_id: String,
        generation: f64,
        presentation_generation: String,
        presentation_id: String,
        sequence: f64,
        session_id: String,
        visibility: String,
    },
    PresentationDetached {
        #[cfg_attr(
            not(test),
            expect(
                dead_code,
                reason = "shared event provenance validated by conformance; Server does not read this field"
            )
        )]
        event_id: String,
        generation: f64,
        presentation_generation: String,
        presentation_id: String,
        sequence: f64,
        session_id: String,
    },
    SessionEnded {
        event_id: String,
        generation: f64,
        sequence: f64,
        session_id: String,
    },
}

impl AgentObservationEvent {
    /// `generation`, `sequence`, `sessionId` for the shared dispatch preamble.
    /// `session-observed` carries its ordered identity inside `observation` and
    /// returns `None` here because it bypasses that preamble entirely.
    pub fn session_event_base(&self) -> Option<(f64, f64, &str)> {
        match self {
            Self::SessionObserved { .. } => None,
            Self::TurnStarted {
                generation,
                sequence,
                session_id,
                ..
            }
            | Self::TurnActivityChanged {
                generation,
                sequence,
                session_id,
                ..
            }
            | Self::AttentionRequested {
                generation,
                sequence,
                session_id,
                ..
            }
            | Self::AttentionResolved {
                generation,
                sequence,
                session_id,
                ..
            }
            | Self::TurnFinished {
                generation,
                sequence,
                session_id,
                ..
            }
            | Self::SessionIdleObserved {
                generation,
                sequence,
                session_id,
                ..
            }
            | Self::ObservationBecameStale {
                generation,
                sequence,
                session_id,
                ..
            }
            | Self::PresentationAttached {
                generation,
                sequence,
                session_id,
                ..
            }
            | Self::PresentationVisibilityChanged {
                generation,
                sequence,
                session_id,
                ..
            }
            | Self::PresentationDetached {
                generation,
                sequence,
                session_id,
                ..
            }
            | Self::SessionEnded {
                generation,
                sequence,
                session_id,
                ..
            }
            | Self::ObservationReconnected {
                generation,
                sequence,
                session_id,
                ..
            } => Some((*generation, *sequence, session_id.as_str())),
        }
    }

    /// The `AgentObservationTransitionKind` recorded in journal entries.
    pub fn transition_kind(&self) -> &'static str {
        match self {
            Self::SessionObserved { .. } => "session-observed",
            Self::TurnStarted { .. } => "turn-started",
            Self::TurnActivityChanged { .. } => "turn-activity-changed",
            Self::AttentionRequested { .. } => "attention-requested",
            Self::AttentionResolved { .. } => "attention-resolved",
            Self::TurnFinished { .. } => "turn-finished",
            Self::SessionIdleObserved { .. } => "session-idle-observed",
            Self::ObservationBecameStale { .. } => "observation-became-stale",
            Self::ObservationReconnected { .. } => "observation-reconnected",
            Self::PresentationAttached { .. } => "presentation-attached",
            Self::PresentationVisibilityChanged { .. } => "presentation-visibility-changed",
            Self::PresentationDetached { .. } => "presentation-detached",
            Self::SessionEnded { .. } => "session-ended",
        }
    }
}

#[derive(Clone, Debug)]
pub enum AgentSemanticEvent {
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "shared reducer output validated by conformance; Server reads the resulting snapshot"
        )
    )]
    AttentionRequested {
        event_id: String,
        request: RawAttentionRequest,
        session_id: String,
    },
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "shared reducer output validated by conformance; Server reads the resulting snapshot"
        )
    )]
    AttentionResolved {
        event_id: String,
        request_id: String,
        session_id: String,
    },
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "shared reducer output validated by conformance; Server reads the resulting snapshot"
        )
    )]
    TurnOutcome {
        event_id: String,
        outcome: String,
        session_id: String,
        turn_id: String,
    },
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "shared reducer output validated by conformance; Server reads the resulting snapshot"
        )
    )]
    SessionEnded {
        event_id: String,
        session_id: String,
    },
}
