mod carrier;
mod command;
mod directory;
mod identity;
mod model;
mod panel_label;
mod validation;
mod workspace_order;
pub use workspace_order::{
    MAXIMUM_ENCODED_WORKSPACE_ORDER_BYTES, MAXIMUM_WORKSPACE_ORDER_LENGTH,
    WorkbenchWorkspaceInsertIndex, WorkbenchWorkspaceList,
};

pub use panel_label::{PanelCustomLabel, PanelLabelUpdate};

pub use carrier::{
    WORKBENCH_MAGIC, WorkbenchWireError, read_workbench_request, read_workbench_response,
    write_workbench_request, write_workbench_response,
};
pub use command::{
    ClientAgentAttentionEntry, ClientAgentAttentionSnapshot, ClientAgentAttentionVersion,
    ClientCellRect, ClientContextSelection, ClientLayoutAdjustment, ClientLayoutChange,
    ClientNavigationSnapshot, ClientPanelArea, ClientPanelDirection, ClientPanelLayout,
    ClientPanelNeighbors, ClientPanelViewport, ClientResizeAmount, ClientScrollMetrics,
    ClientSplitArea, ClientSplitChange, ClientSplitIdentity, ClientSplitPath, ClientSplitRatio,
    ClientViewAction, ClientViewAvailability, ClientViewCompletion, ClientViewCompletionOutcome,
    ClientViewControl, ClientViewControlReason, ClientViewControlStatus, ClientViewIntent,
    ClientViewObservation, ClientViewOperation, ClientViewOutcome, ClientViewPhase,
    ClientViewRecord, ClientViewSelection, ClientViewSequence, ClientViewUnavailable,
    ClientViewportMode, ClientViewportObservation, ClientViewportPending, ClientViewportSnapshot,
    ClientViewportSource, ClientViewportUnavailable, LaunchProfileCatalogOutcome,
    LaunchProfileCatalogRevision, LaunchProfileEntry, LaunchProfileId, LaunchProfileName,
    LaunchProfileSelector, LaunchSelection, MAXIMUM_CLIENT_ATTENTION_SESSIONS,
    MAXIMUM_CLIENT_SELECTION_CONTEXTS, MAXIMUM_CLIENT_VIEWPORT_PANELS, MAXIMUM_CLIENT_VIEWS,
    MAXIMUM_ENCODED_CLIENT_ATTENTION_BYTES, MAXIMUM_ENCODED_CLIENT_LAYOUT_BYTES,
    MAXIMUM_ENCODED_CLIENT_NAVIGATION_BYTES, MAXIMUM_ENCODED_CLIENT_VIEW_RESULT_BYTES,
    MAXIMUM_ENCODED_CLIENT_VIEWPORT_BYTES, MAXIMUM_ENCODED_LAUNCH_COMMAND_BYTES,
    MAXIMUM_ENCODED_LAUNCH_PROFILE_CATALOG_RESULT_BYTES, MAXIMUM_ENCODED_SNAPSHOT_RESULT_BYTES,
    MAXIMUM_ENCODED_WORKSPACE_CLOSE_READ_RESULT_BYTES,
    MAXIMUM_ENCODED_WORKSPACE_GROUP_CLOSE_RESULT_BYTES, MAXIMUM_ENCODED_WORKTREE_LIST_RESULT_BYTES,
    MAXIMUM_ENCODED_WORKTREE_OPERATION_BYTES, MAXIMUM_ENCODED_WORKTREE_QUERY_RESULT_BYTES,
    MAXIMUM_LAUNCH_PROFILE_CANDIDATES, MAXIMUM_LAUNCH_PROFILE_CATALOG_BYTES,
    MAXIMUM_LAUNCH_PROFILE_ENTRIES, MAXIMUM_LAUNCH_SELECTION_ARGUMENTS,
    MAXIMUM_LAUNCH_SELECTION_BYTES, MAXIMUM_RESULT_PUBLICATIONS,
    MAXIMUM_WORKSPACE_GROUP_CLOSE_ATTEMPTS, MAXIMUM_WORKSPACE_GROUP_CLOSE_MEMBERS,
    MAXIMUM_WORKTREE_OPERATION_ATTEMPTS, PublicationReadIntent, WorkbenchAction,
    WorkbenchClientViewRequest, WorkbenchClientViewResult, WorkbenchCommand,
    WorkbenchCommandOutcome, WorkbenchCommandResult, WorkbenchCommandUnavailableReason,
    WorkbenchLabel, WorkbenchLaunchProfileCatalogRead, WorkbenchLaunchProfileCatalogResult,
    WorkbenchPanelRelocateDestination, WorkbenchPanelSplitDirection, WorkbenchPanelTransfer,
    WorkbenchPublicationOutcome, WorkbenchPublicationResult, WorkbenchRejectionReason,
    WorkbenchRequest, WorkbenchResponse, WorkbenchSnapshotOutcome, WorkbenchSnapshotResult,
    WorkbenchSnapshotUnavailableReason, WorkbenchTabCloseAttemptGeneration,
    WorkbenchTabCloseBlockedReason, WorkbenchTabClosePhase, WorkbenchTabClosePublicationReason,
    WorkbenchTabCloseRead, WorkbenchTabCloseReadOutcome, WorkbenchTabCloseReadResult,
    WorkbenchTabCloseResume, WorkbenchTabCloseResumeOutcome,
    WorkbenchTabCloseResumeRejectionReason, WorkbenchTabCloseResumeResult, WorkbenchTabCloseStatus,
    WorkbenchTabCloseUnavailableReason, WorkbenchTabCloseUnknownPhase,
    WorkbenchTabCloseUnknownReason, WorkbenchTabInsertIndex, WorkbenchTabMoveDirection,
    WorkbenchTerminalFailureReason, WorkbenchTerminalGeometry, WorkbenchTerminalOutcome,
    WorkbenchTerminalUnavailableReason, WorkbenchWorkspaceCloseCounts,
    WorkbenchWorkspaceCloseMember, WorkbenchWorkspaceCloseMemberStatus,
    WorkbenchWorkspaceCloseRead, WorkbenchWorkspaceCloseReadOutcome,
    WorkbenchWorkspaceCloseReadResult, WorkbenchWorkspaceCloseResume,
    WorkbenchWorkspaceCloseResumeOutcome, WorkbenchWorkspaceCloseResumeResult,
    WorkbenchWorkspaceGroupCloseRequest, WorkbenchWorkspaceGroupCloseResult,
    WorkbenchWorktreeAction, WorkbenchWorktreeListOutcome, WorkbenchWorktreeListRead,
    WorkbenchWorktreeListResult, WorkbenchWorktreeOutcome, WorkbenchWorktreeQuery,
    WorkbenchWorktreeQueryOutcome, WorkbenchWorktreeQueryResult, WorkbenchWorktreeRequest,
    WorkbenchWorktreeResult, WorkspaceGroupCloseAction, WorkspaceGroupCloseChild,
    WorkspaceGroupCloseFailure, WorkspaceGroupCloseMember, WorkspaceGroupCloseObservation,
    WorkspaceGroupCloseOutcome, WorkspaceGroupCloseStatus, WorkspaceGroupCloseUnavailable,
    WorktreeAdmissionFailure, WorktreeCheckoutFacts, WorktreeEntry, WorktreeEntryText,
    WorktreeGitEffect, WorktreeGitError, WorktreeGitStage, WorktreeIntent, WorktreeListStage,
    WorktreeListTooLargeOrigin, WorktreeListUnavailableReason, WorktreeOperationFailure,
    WorktreeOperationPhase, WorktreeOperationStatus, WorktreeProgress, WorktreeQueryEntry,
    WorktreeQueryFailure, WorktreeQuerySource, WorktreeReference, WorktreeSelector,
    WorktreeShellRestoration, WorktreeSource, WorktreeWorkspaceAssociation,
    WorktreeWorkspaceDisposition, parse_workbench_request, parse_workbench_response,
    serialize_workbench_request, serialize_workbench_response,
};

pub use directory::{
    MAXIMUM_DIRECTORY_PATH_BYTES, WorkbenchDirectoryPath, WorkbenchDirectoryPathError,
};

pub use identity::{
    IdentityError, ProtocolVersion, RevisionError, SiteId, TerminalId, TerminalPresentationId,
    WorkbenchClientId, WorkbenchDirectoryId, WorkbenchPanelId, WorkbenchPanelSlotId,
    WorkbenchProjectId, WorkbenchRequestId, WorkbenchRevision, WorkbenchSplitId,
    WorkbenchTabGroupId, WorkbenchTabId, WorkbenchTerminalContextId, WorkbenchWorkspaceId,
    WorkerId,
};
pub use model::{
    Delta, DeltaChange, DeltaRejectionReason, Directory, Navigation, PanelNode, PanelOrientation,
    PanelRightClickTarget, PanelSlot, Presentation, PresentationBinding, Project, Snapshot,
    SnapshotReadIntent, Tab, TabGroup, TabGroupColor, TerminalContext, WorkbenchProtocolDocument,
    WorkbenchPublicationDocument, Workspace, tab_display_label,
};
pub use validation::{
    ContractError, MAXIMUM_CHILD_PRESENTATIONS_PER_PANEL, MAXIMUM_DELTA_CHANGES,
    MAXIMUM_DIRECTORIES_PER_PROJECT, MAXIMUM_ENCODED_INTENT_BYTES,
    MAXIMUM_ENCODED_PROJECTION_BYTES, MAXIMUM_LABEL_LENGTH, MAXIMUM_PANEL_NODE_DEPTH,
    MAXIMUM_PANEL_NODES_PER_CONTEXT, MAXIMUM_PROJECTS, MAXIMUM_TAB_GROUPS_PER_CONTEXT,
    MAXIMUM_TABS_PER_CONTEXT, MAXIMUM_TERMINAL_CONTEXTS, MAXIMUM_WORKSPACES_PER_PROJECT,
    apply_workbench_delta, apply_workbench_publication, parse_delta, parse_document,
    parse_snapshot, parse_snapshot_read_intent, serialize_document,
};

mod launch_options;
pub use launch_options::LaunchOptions;
mod launch_environment;
pub use launch_environment::{
    LaunchEnvironment, MAXIMUM_LAUNCH_ENVIRONMENT_BYTES, MAXIMUM_LAUNCH_ENVIRONMENT_ENTRIES,
};
