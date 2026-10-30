//! Closed vocabularies for the Agent integration surface, owned by the language-neutral
//! operation contract in `orchbee-agent-integration`. Kilo extends the native registry with
//! Windows-only managed onboarding; Desktop management keeps its own registry.

pub use orchbee_agent_integration::{
    Availability, IntegrationId, Issue, ManagementMode, Provenance, SnapshotItem, State,
};
