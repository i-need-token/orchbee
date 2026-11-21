//! The Agent integration contract values shared by the dormant integration engine and `uninstall`.
//! No command enables or lists integrations. `uninstall` runs the writer's disable in its own
//! process once every Session, and so every Worker, has stopped, so hooks that earlier versions
//! wrote are still removed.

// module-layer: values

pub(crate) use orchbee_agent_integration::{IntegrationId, Refusal, State, Verdict};

pub(crate) mod model {
    pub(crate) use orchbee_agent_integration::Availability;
}
