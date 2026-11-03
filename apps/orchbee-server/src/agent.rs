//! Canonical Agent observation model — bounded Rust port of the removed
//! TypeScript `packages/agent` reference, pinned by the frozen conformance corpus.
//!
//! The production surface includes canonical state, admitted observation
//! transitions, projections and bounded change reads. Test-only seeding and
//! conformance harnesses remain gated. See `docs/agent-canonical-model.md`.

mod model;
mod projection;
mod runtime;

mod changes;
mod events;
mod hook_contract;
pub(crate) mod hook_ingress;
mod join;
pub(crate) mod native_observation;
mod reducer;
#[cfg(test)]
mod tests;

pub(crate) use changes::{AgentChangeReadInput, AgentRuntimeChange, AgentRuntimeChangeReadResult};
pub(crate) use model::AgentModelError;
pub(crate) use model::{
    ATTENTION_ACTION_PATH, AgentActivity, AgentCapability, AgentFreshness, AgentLifecycle,
    AgentObservation, AgentObservationSource, AgentPresentationAttachment,
    AgentPresentationVisibility, AgentProviderId, AgentRuntimeSnapshot, AgentSessionSnapshot,
    AgentTurnOutcomeKind, AgentTurnState, OpaqueId,
};
pub(crate) use model::{
    AgentAttentionKind, AgentAttentionRequest, AgentObservationFidelity, AgentTurnOutcome,
    AgentTurnPhase, PresentationId,
};
pub(crate) use projection::{
    AgentSemanticStatus, explain_agent_session, project_agent_session,
    project_agent_session_summary,
};
pub(crate) use runtime::AgentRuntime;

// Narrowly scoped seeding support: tests dispatch real canonical events
// through the same entry the corpus uses.
pub(crate) use events::AgentObservationEvent;
#[cfg(test)]
pub(crate) use model::RawAttachment;
pub(crate) use model::{RawAttentionRequest, RawExecutionLocation, RawObservation, RawPresence};
pub(crate) use reducer::AgentTransition;

// The translator and its join options back the `AgentState` cell's
// production entry seams (endpoint admission, tick, deadline, native
// drain application); corpus/test helpers stay gated inside the modules.
pub(crate) use hook_ingress::AgentHookObservationIngress;
pub(crate) use join::JoinOptions;
