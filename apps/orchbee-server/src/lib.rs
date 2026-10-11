//! Private Server authority and projection operations shared with its binary and conformance
//! harnesses. This target owns deterministic state transitions, not endpoint admission, worker
//! process supervision or a stable application-library API.

// Canonical Agent model: the production consumer lives in the `orchbee-server`
// binary (agent_service); the library target itself has no consumer, so the
// module compiles here only for the private conformance harness. See
// docs/agent-canonical-model.md.
#[cfg(test)]
mod agent;
mod pane_graphics;
mod semantic_events;
pub use pane_graphics::{
    DecodedGraphicsImage, PaneGraphicsAuthority, PaneGraphicsFrameCommit, PaneGraphicsOpen,
    PaneGraphicsOpenPreparation,
};
mod workbench_authority;
mod workbench_projection;
pub use semantic_events::{
    SiteEventJournal, SiteEventPage, matches_filter, semantic_pane, semantic_panes_for_binding,
};

pub use workbench_authority::{
    CloseContinuationNext, CloseStepObservation, CloseStepPrequeueFailure, CloseStepToken,
    CloseStepTokenMismatch, CommittedWorkbenchPanelCreateMetadata, CommittedWorkbenchTabCreate,
    CommittedWorkbenchTabCreateMetadata, LaunchProfileUse, MAXIMUM_WORKBENCH_CHECKPOINT_BYTES,
    ManagedAgentRestart, PendingWorkbenchPanelCreate, PendingWorkbenchTabCreate, PluginPaneLaunch,
    PreparedManagedAgentClear, PreparedStandaloneIntent, PreparedWorkbenchPanelCreate,
    PreparedWorkbenchPanelCreateSettlement, PreparedWorkbenchPanelPlacement,
    PreparedWorkbenchPanelRelocate, PreparedWorkbenchRestoreBinding, PreparedWorkbenchTabClose,
    PreparedWorkbenchTabClosePublication, PreparedWorkbenchTabCloseResume,
    PreparedWorkbenchTabCreate, PreparedWorkbenchTabMove, PreparedWorkbenchTabSettlement,
    PreparedWorktreeRebind, StagedWorkbenchPanelCreateMetadata, StagedWorkbenchTabCreateMetadata,
    StandaloneIntentError, StandaloneIntentMetadata, TerminalAgentBinding,
    TerminalAgentExplainContext, TerminalAgentNameOwner, TerminalAgentProcessObservation,
    TerminalAgentProcessOutcome, TerminalAgentScreenObservation, WorkbenchAuthority,
    WorkbenchBindingExposure, WorkbenchCheckpoint, WorkbenchCheckpointError, WorkbenchGrant,
    WorkbenchPanelCreateAdmission, WorkbenchPanelCreateBegin, WorkbenchPanelCreatePreparation,
    WorkbenchPanelCreateSettlementTokenMismatch, WorkbenchPanelPlacementAdmission,
    WorkbenchPanelPlacementBegin, WorkbenchPanelPlacementPreparation,
    WorkbenchPanelRelocateAdmission, WorkbenchPanelRelocateBegin,
    WorkbenchPanelRelocatePreparation, WorkbenchReadOrNavigationRequest, WorkbenchRestoreError,
    WorkbenchRestoreTarget, WorkbenchSettlementTokenMismatch, WorkbenchTabCloseAdmission,
    WorkbenchTabCloseBegin, WorkbenchTabCloseMemberRuntime, WorkbenchTabClosePreparation,
    WorkbenchTabClosePrimaryAdmission, WorkbenchTabClosePublicationError,
    WorkbenchTabCloseResolution, WorkbenchTabCloseResumeAdmission, WorkbenchTabCloseResumeBegin,
    WorkbenchTabCloseResumePreparation, WorkbenchTabCloseRuntimeTarget,
    WorkbenchTabCloseRuntimeUnavailableReason, WorkbenchTabCloseStepTarget,
    WorkbenchTabCloseTarget, WorkbenchTabCreateAdmission, WorkbenchTabCreateBegin,
    WorkbenchTabCreatePreparation, WorkbenchTabCreateRuntimeUnavailableReason,
    WorkbenchTabMoveAdmission, WorkbenchTabMoveBegin, WorkbenchTabMovePreparation,
    WorkbenchTerminalInvariantError, WorkbenchTerminalMembership, WorkbenchTerminalTab,
    WorkbenchWorkspaceCloseAdmission, WorkbenchWorkspaceCloseBegin,
    WorkbenchWorkspaceCloseResumeAdmission, WorkbenchWorkspaceCloseResumeBegin,
    WorkbenchWorkspaceCloseResumePreparation, WorkspaceGroupCloseMemberPlan,
    WorkspaceGroupClosePlan, WorktreeListResolution, WorktreeListTarget, WorktreeOperationPlan,
    WorktreeRebindError, WorktreeSourceTarget,
};
