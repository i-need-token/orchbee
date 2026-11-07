//! Private Server authority and projection operations shared with its binary and conformance
//! harnesses. This target owns deterministic state transitions, not endpoint admission, worker
//! process supervision or a stable application-library API.

mod pane_graphics;
mod semantic_events;
pub use pane_graphics::{
    DecodedGraphicsImage, PaneGraphicsAuthority, PaneGraphicsFrameCommit, PaneGraphicsOpen,
    PaneGraphicsOpenPreparation,
};
mod workbench_authority;
mod workbench_projection;
pub use semantic_events::{
    SessionChangeSignal, SessionChangeSource, SessionEventJournal, SessionEventPage,
    matches_filter, semantic_pane, semantic_panes_for_binding,
};

pub use workbench_authority::{
    AgentResumeProvider, CloseContinuationNext, CloseStepObservation, CloseStepPrequeueFailure,
    CloseStepToken, CloseStepTokenMismatch, CommittedWorkbenchPanelCreateMetadata,
    CommittedWorkbenchTabCreate, CommittedWorkbenchTabCreateMetadata, LaunchProfileUse,
    LayoutHistorySummary, LayoutReopenError, MAXIMUM_WORKBENCH_CHECKPOINT_BYTES, OpenLayout,
    PendingWorkbenchPanelCreate, PendingWorkbenchTabCreate, PluginPaneLaunch,
    PreparedWorkbenchPanelCreate, PreparedWorkbenchPanelCreateSettlement,
    PreparedWorkbenchPanelPlacement, PreparedWorkbenchPanelRelocate,
    PreparedWorkbenchRestoreBinding, PreparedWorkbenchTabClose,
    PreparedWorkbenchTabClosePublication, PreparedWorkbenchTabCloseResume,
    PreparedWorkbenchTabCreate, PreparedWorkbenchTabMove, PreparedWorkbenchTabSettlement,
    PreparedWorktreeRebind, ReopenedLayout, RetainedAgentSession, RetainedLayout,
    StagedWorkbenchPanelCreateMetadata, StagedWorkbenchTabCreateMetadata, TabDuplicatePlan,
    TabSetCloseAdmission, TabSetCloseBegin, TabSetCloseMemberPlan, TabSetCloseMemberState,
    TabSetClosePlan, TerminalAgentBinding, TerminalAgentExplainContext, TerminalAgentNameOwner,
    TerminalAgentProcessObservation, TerminalAgentProcessOutcome, TerminalAgentScreenObservation,
    WorkbenchAuthority, WorkbenchBindingExposure, WorkbenchCheckpoint, WorkbenchCheckpointError,
    WorkbenchGrant, WorkbenchPanelCreateAdmission, WorkbenchPanelCreateBegin,
    WorkbenchPanelCreatePreparation, WorkbenchPanelCreateSettlementTokenMismatch,
    WorkbenchPanelPlacementAdmission, WorkbenchPanelPlacementBegin,
    WorkbenchPanelPlacementPreparation, WorkbenchPanelRelocateAdmission,
    WorkbenchPanelRelocateBegin, WorkbenchPanelRelocatePreparation,
    WorkbenchProjectDissolveAdmission, WorkbenchProjectDissolveBegin,
    WorkbenchReadOrNavigationRequest, WorkbenchRestoreError, WorkbenchRestoreTarget,
    WorkbenchSettlementTokenMismatch, WorkbenchTabCloseAdmission, WorkbenchTabCloseBegin,
    WorkbenchTabCloseMemberRuntime, WorkbenchTabClosePreparation,
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
    WorktreeRebindError, WorktreeSourceTarget, same_directory, without_trailing_separator,
};

#[cfg(test)]
mod test_support;

#[cfg(test)]
extern crate self as orchbee_server;
