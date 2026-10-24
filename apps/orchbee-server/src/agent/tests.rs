pub(super) mod conformance_support;
mod corpus;
mod hook_conformance;
mod oracle;
mod snapshot_copy_probe;
mod targeted;

use super::events::AgentSemanticEvent;
use super::{AgentRuntimeSnapshot, AgentTransition};
impl AgentTransition {
    pub fn snapshot(&self) -> &AgentRuntimeSnapshot {
        match self {
            Self::Applied { snapshot, .. }
            | Self::Unchanged { snapshot }
            | Self::Rejected { snapshot, .. } => snapshot,
        }
    }
    pub fn semantic_events(&self) -> &[AgentSemanticEvent] {
        match self {
            Self::Applied {
                semantic_events, ..
            } => semantic_events,
            Self::Unchanged { .. } | Self::Rejected { .. } => &[],
        }
    }
}

impl super::reducer::AgentTransitionRejection {
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

impl super::AgentObservationEvent {
    /// Every observation event carries its own `eventId` provenance.
    pub fn event_id(&self) -> &str {
        match self {
            Self::SessionObserved { event_id, .. }
            | Self::TurnStarted { event_id, .. }
            | Self::TurnActivityChanged { event_id, .. }
            | Self::AttentionRequested { event_id, .. }
            | Self::AttentionResolved { event_id, .. }
            | Self::TurnFinished { event_id, .. }
            | Self::SessionIdleObserved { event_id, .. }
            | Self::ObservationBecameStale { event_id, .. }
            | Self::ObservationReconnected { event_id, .. }
            | Self::PresentationAttached { event_id, .. }
            | Self::PresentationVisibilityChanged { event_id, .. }
            | Self::PresentationDetached { event_id, .. }
            | Self::SessionEnded { event_id, .. } => event_id,
        }
    }
}
