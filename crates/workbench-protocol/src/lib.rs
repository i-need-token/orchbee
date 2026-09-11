mod carrier;
mod command;
mod directory;
mod identity;
mod model;
mod validation;

pub use carrier::{
    WORKBENCH_MAGIC, WorkbenchWireError, read_workbench_request, read_workbench_response,
    write_workbench_request, write_workbench_response,
};
pub use command::{
    MAXIMUM_ENCODED_SNAPSHOT_RESULT_BYTES, MAXIMUM_RESULT_PUBLICATIONS, PublicationReadIntent,
    WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchCommandResult,
    WorkbenchCommandUnavailableReason, WorkbenchLabel, WorkbenchPanelSplitDirection,
    WorkbenchPublicationOutcome, WorkbenchPublicationResult, WorkbenchRejectionReason,
    WorkbenchRequest, WorkbenchResponse, WorkbenchSnapshotOutcome, WorkbenchSnapshotResult,
    WorkbenchSnapshotUnavailableReason, WorkbenchTabCloseAttemptGeneration,
    WorkbenchTabCloseBlockedReason, WorkbenchTabClosePhase, WorkbenchTabClosePublicationReason,
    WorkbenchTabCloseRead, WorkbenchTabCloseReadOutcome, WorkbenchTabCloseReadResult,
    WorkbenchTabCloseResume, WorkbenchTabCloseResumeOutcome,
    WorkbenchTabCloseResumeRejectionReason, WorkbenchTabCloseResumeResult, WorkbenchTabCloseStatus,
    WorkbenchTabCloseUnavailableReason, WorkbenchTabCloseUnknownPhase,
    WorkbenchTabCloseUnknownReason, WorkbenchTabMoveDirection, WorkbenchTerminalFailureReason,
    WorkbenchTerminalGeometry, WorkbenchTerminalOutcome, WorkbenchTerminalUnavailableReason,
    parse_workbench_request, parse_workbench_response, serialize_workbench_request,
    serialize_workbench_response,
};

pub use directory::{
    MAXIMUM_DIRECTORY_PATH_BYTES, WorkbenchDirectoryPath, WorkbenchDirectoryPathError,
};

pub use identity::{
    IdentityError, ProtocolVersion, RevisionError, SiteId, TerminalId, TerminalPresentationId,
    WorkbenchDirectoryId, WorkbenchPanelId, WorkbenchPanelSlotId, WorkbenchProjectId,
    WorkbenchRequestId, WorkbenchRevision, WorkbenchSplitId, WorkbenchTabGroupId, WorkbenchTabId,
    WorkbenchTerminalContextId, WorkbenchWorkspaceId, WorkerId,
};
pub use model::{
    Delta, DeltaChange, DeltaRejectionReason, Directory, Navigation, PanelNode, PanelOrientation,
    PanelSlot, Presentation, PresentationBinding, Project, Snapshot, SnapshotReadIntent, Tab,
    TabGroup, TabGroupColor, TerminalContext, WorkbenchProtocolDocument,
    WorkbenchPublicationDocument, Workspace,
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
