//! The Agent integration contract values the CLI exchanges with the Session Worker and renders.
//! Provider inspection and mutation run only as Worker jobs; offline diagnostics and update
//! verification call the node-local writer's read-only helpers directly.

pub(crate) use orchbee_agent_integration::{
    AttemptedStep, CapsuleStatus, FailureCode, InspectionDetail, IntegrationId,
    IntegrationOperation, IntegrationReport, IntegrationRequest, Issue, MutationReport,
    PlannedStep, Refusal, SnapshotItem, State, StepKind, Verdict,
};

pub(crate) mod model {
    pub(crate) use orchbee_agent_integration::{Availability, ManagementMode};
}
