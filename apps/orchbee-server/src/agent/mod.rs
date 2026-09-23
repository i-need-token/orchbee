//! Canonical Agent observation model — bounded Rust port of the frozen
//! TypeScript `packages/agent` sources (base commit
//! `0d4b685a1b7db9405ca94908385e0534ddd41dc9`).
//!
//! The production surface is the first-read subset only: typed canonical
//! state, one `AgentRuntime`, and the status/summary/explanation projections
//! the OBAP `list`/`get`/`explain` adapter consumes. The mutation surface —
//! events, reducer, change journal, raw construction, and every conformance
//! harness — stays `#[cfg(test)]`. See `docs/agent-canonical-model.md` for
//! claims, traceability, and the activation boundary.

mod model;
mod projection;
mod runtime;

mod changes;
mod conformance_support;
#[cfg(test)]
mod corpus;
mod events;
#[cfg(test)]
mod hook_conformance;
mod hook_contract;
pub(crate) mod hook_ingress;
mod join;
pub(crate) mod native_observation;
#[cfg(test)]
mod oracle;
mod reducer;
#[cfg(test)]
mod targeted;

pub(crate) use model::AgentModelError;
pub(crate) use model::{
    ATTENTION_ACTION_PATH, AgentActivity, AgentCapability, AgentFreshness, AgentLifecycle,
    AgentObservation, AgentObservationSource, AgentPresentationAttachment,
    AgentPresentationVisibility, AgentProviderId, AgentRuntimeSnapshot, AgentSessionSnapshot,
    AgentTurnOutcomeKind, AgentTurnState, OpaqueId,
};
// Consumed only by the bin crate (`agent_service::map`); the cfg(test) lib
// harness has no such consumer and would report them as unused.
#[cfg_attr(test, allow(unused_imports))]
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
// Consumed by the bin-only Agent service tests; the library conformance target
// compiles this module without that consumer.
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use model::RawAttachment;
pub(crate) use model::{RawAttentionRequest, RawExecutionLocation, RawObservation, RawPresence};
pub(crate) use reducer::AgentTransition;

// The translator and its join options back the `AgentState` cell's
// production entry seams (endpoint admission, tick, deadline, native
// drain application); corpus/test helpers stay gated inside the modules.
pub(crate) use hook_ingress::AgentHookObservationIngress;
pub(crate) use join::JoinOptions;
