//! The Agent integration contract values the CLI exchanges with the Session Worker and renders.
//! `explain`, enable and disable run as Worker jobs; `list`, offline diagnostics and update
//! verification call the node-local writer's read-only helpers directly, and `uninstall` runs the
//! writer's disable in its own process once every Session, and so every Worker, has stopped.

pub(crate) use orchbee_agent_integration::{
    AttemptedStep, CapsuleStatus, FailureCode, InspectionDetail, IntegrationId,
    IntegrationOperation, IntegrationReport, IntegrationRequest, Issue, MutationReport,
    PlannedStep, Refusal, SnapshotItem, State, StepKind, Verdict,
};

pub(crate) mod model {
    pub(crate) use orchbee_agent_integration::Availability;
}
