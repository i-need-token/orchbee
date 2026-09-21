// Canonical Agent model: the production consumer lives in the `orchbee-server`
// binary (agent_service); the library target itself has no consumer, so the
// module compiles here only for the private conformance harness. See
// docs/agent-canonical-model.md.
#[cfg(test)]
mod agent;
mod workbench_authority;
mod workbench_projection;

pub use workbench_authority::{
    CloseContinuationNext, CloseStepObservation, CloseStepPrequeueFailure, CloseStepToken,
    CloseStepTokenMismatch, CommittedWorkbenchPanelSplitMetadata, CommittedWorkbenchTabCreate,
    CommittedWorkbenchTabCreateMetadata, LaunchProfileUse, PendingWorkbenchPanelSplit,
    PendingWorkbenchTabCreate, PreparedStandaloneIntent, PreparedWorkbenchPanelMove,
    PreparedWorkbenchPanelRelocate, PreparedWorkbenchPanelSplit,
    PreparedWorkbenchPanelSplitSettlement, PreparedWorkbenchTabClose,
    PreparedWorkbenchTabClosePublication, PreparedWorkbenchTabCloseResume,
    PreparedWorkbenchTabCreate, PreparedWorkbenchTabMove, PreparedWorkbenchTabSettlement,
    StagedWorkbenchPanelSplitMetadata, StagedWorkbenchTabCreateMetadata, StandaloneIntentError,
    StandaloneIntentMetadata, WorkbenchAuthority, WorkbenchBindingExposure, WorkbenchGrant,
    WorkbenchPanelMoveAdmission, WorkbenchPanelMoveBegin, WorkbenchPanelMovePreparation,
    WorkbenchPanelRelocateAdmission, WorkbenchPanelRelocateBegin,
    WorkbenchPanelRelocatePreparation, WorkbenchPanelSplitAdmission, WorkbenchPanelSplitBegin,
    WorkbenchPanelSplitPreparation, WorkbenchPanelSplitSettlementTokenMismatch,
    WorkbenchReadOrNavigationRequest, WorkbenchSettlementTokenMismatch, WorkbenchTabCloseAdmission,
    WorkbenchTabCloseBegin, WorkbenchTabClosePreparation, WorkbenchTabClosePrimaryAdmission,
    WorkbenchTabClosePublicationError, WorkbenchTabCloseResolution,
    WorkbenchTabCloseResumeAdmission, WorkbenchTabCloseResumeBegin,
    WorkbenchTabCloseResumePreparation, WorkbenchTabCloseRuntimeTarget,
    WorkbenchTabCloseRuntimeUnavailableReason, WorkbenchTabCloseStepTarget,
    WorkbenchTabCloseTarget, WorkbenchTabCreateAdmission, WorkbenchTabCreateBegin,
    WorkbenchTabCreatePreparation, WorkbenchTabCreateRuntimeUnavailableReason,
    WorkbenchTabMoveAdmission, WorkbenchTabMoveBegin, WorkbenchTabMovePreparation,
    WorkbenchTerminalInvariantError, WorkbenchTerminalMembership, WorkbenchTerminalTab,
    WorkbenchWorkspaceCloseAdmission, WorkbenchWorkspaceCloseBegin,
    WorkbenchWorkspaceCloseResumeAdmission, WorkbenchWorkspaceCloseResumeBegin,
    WorkbenchWorkspaceCloseResumePreparation, WorktreeListResolution, WorktreeListTarget,
};
