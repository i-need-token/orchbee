//! Agent integration ownership shared by every native process: the language-neutral operation
//! contract and managed provider identities. Clients and Server consume only this contract; the
//! node-local writer executes it under Server authorization.

mod contract;
mod identity;

pub use contract::{
    AttemptedStep, Availability, CAPSULE_SOURCE_KINDS, CapsuleStatus, ContractError, Direction,
    FILESYSTEM_OPERATIONS, FailureCode, InspectionDetail, IntegrationOperation, IntegrationReport,
    IntegrationRequest, Issue, MAXIMUM_INTEGRATION_BUDGET_MILLISECONDS,
    MAXIMUM_INTEGRATION_REPORT_BYTES, MAXIMUM_INTEGRATION_REQUEST_BYTES, ManagementMode,
    Marketplace, MutationReport, PlannedStep, Provenance, Refusal, SnapshotItem, State, StepKind,
    StepOutcome, TargetState, Verdict, decode_report, decode_request, encode_report,
    encode_request,
};
pub use identity::IntegrationId;
