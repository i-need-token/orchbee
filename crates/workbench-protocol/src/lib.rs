pub mod agent_view;
pub use agent_view::{
    AgentViewAction, AgentViewBuiltinField, AgentViewBuiltinSortField, AgentViewContextOperand,
    AgentViewContextValue, AgentViewFailure, AgentViewField, AgentViewFilter, AgentViewOutcome,
    AgentViewRequest, AgentViewResult, AgentViewSnapshot, AgentViewSort, AgentViewSortField,
    AgentViewSortOrder, AgentViewSpec, AgentViewToken, AgentViewValue, MAXIMUM_AGENT_VIEW_BYTES,
    MAXIMUM_AGENT_VIEW_SPEC_BYTES, parse_agent_view_spec,
};
mod carrier;
mod command;
pub use command::{
    ConfiguredCommandAction, ConfiguredCommandDescriptor, ConfiguredCommandFailure,
    ConfiguredCommandInvocation, ConfiguredCommandKind, ConfiguredCommandOutcome,
    ConfiguredCommandPopupSpec, ConfiguredCommandRequest, ConfiguredCommandResult,
    ConfiguredCommandSelection, ConfiguredCommandSource, ConfiguredCommandTextPoint,
    MAXIMUM_CONFIGURED_COMMAND_BYTES,
};
pub use command::{
    EditorPopupCapture, MAXIMUM_TERMINAL_EDITOR_BYTES, ScrollbackEditorSpec, TerminalEditorAction,
    TerminalEditorFailure, TerminalEditorOutcome, TerminalEditorRequest, TerminalEditorResult,
};
pub use command::{
    GitDivergence, MAXIMUM_WORKSPACE_GIT_BYTES, WorkspaceGitFailure, WorkspaceGitOutcome,
    WorkspaceGitRequest, WorkspaceGitResult,
};
pub use command::{
    MAXIMUM_EVENT_FILTERS, MAXIMUM_EVENT_PAGE, MAXIMUM_EVENTS_BYTES, SemanticEvent,
    SemanticEventCursor, SemanticEventData, SemanticEventFilter, SemanticEventGap,
    SemanticEventKind, SemanticEventMatch, SemanticEventPosition, SemanticEventScope,
    SemanticEventSource, SemanticEventsAction, SemanticEventsFailure, SemanticEventsOutcome,
    SemanticEventsRequest, SemanticEventsResult, SemanticOutputMatch, SemanticOutputSource,
    SemanticPane, matches_semantic_filter,
};
mod directory;
mod identity;
mod layout_template;
mod model;
pub use layout_template::{
    MAXIMUM_ENCODED_LAYOUT_COMMAND_BYTES, MAXIMUM_LAYOUT_CHILDREN, MAXIMUM_LAYOUT_DEPTH,
    MAXIMUM_LAYOUT_PANELS, WorkbenchLayoutNode, WorkbenchLayoutPresentation,
    WorkbenchLayoutTemplate,
};
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
    ClientNavigationSnapshot, ClientOverlayMember, ClientPanelArea, ClientPanelDirection,
    ClientPanelLayout, ClientPanelNeighbors, ClientPanelViewport, ClientPresentationSelection,
    ClientPresentationSnapshot, ClientResizeAmount, ClientScrollMetrics, ClientSplitArea,
    ClientSplitChange, ClientSplitIdentity, ClientSplitPath, ClientSplitRatio,
    ClientTerminalAgentAttentionEntry, ClientTerminalAgentAttentionSnapshot,
    ClientTerminalAgentAttentionVersion, ClientViewAction, ClientViewAvailability,
    ClientViewCompletion, ClientViewCompletionOutcome, ClientViewControl, ClientViewControlReason,
    ClientViewControlStatus, ClientViewIntent, ClientViewObservation, ClientViewOperation,
    ClientViewOutcome, ClientViewPhase, ClientViewRecord, ClientViewSelection, ClientViewSequence,
    ClientViewUnavailable, ClientViewportMode, ClientViewportObservation, ClientViewportPending,
    ClientViewportSnapshot, ClientViewportSource, ClientViewportUnavailable,
    LaunchProfileCatalogOutcome, LaunchProfileCatalogRevision, LaunchProfileEntry, LaunchProfileId,
    LaunchProfileName, LaunchProfileSelector, LaunchSelection, MAXIMUM_CLIENT_ATTENTION_SESSIONS,
    MAXIMUM_CLIENT_SELECTION_CONTEXTS, MAXIMUM_CLIENT_VIEWPORT_PANELS, MAXIMUM_CLIENT_VIEWS,
    MAXIMUM_ENCODED_CLIENT_ATTENTION_BYTES, MAXIMUM_ENCODED_CLIENT_LAYOUT_BYTES,
    MAXIMUM_ENCODED_CLIENT_NAVIGATION_BYTES, MAXIMUM_ENCODED_CLIENT_PRESENTATION_BYTES,
    MAXIMUM_ENCODED_CLIENT_VIEW_RESULT_BYTES, MAXIMUM_ENCODED_CLIENT_VIEWPORT_BYTES,
    MAXIMUM_ENCODED_LAUNCH_COMMAND_BYTES, MAXIMUM_ENCODED_LAUNCH_PROFILE_CATALOG_RESULT_BYTES,
    MAXIMUM_ENCODED_SNAPSHOT_RESULT_BYTES, MAXIMUM_ENCODED_WORKSPACE_CLOSE_READ_RESULT_BYTES,
    MAXIMUM_ENCODED_WORKSPACE_GROUP_CLOSE_RESULT_BYTES,
    MAXIMUM_ENCODED_WORKSPACE_METADATA_RESULT_BYTES, MAXIMUM_ENCODED_WORKTREE_LIST_RESULT_BYTES,
    MAXIMUM_ENCODED_WORKTREE_OPERATION_BYTES, MAXIMUM_ENCODED_WORKTREE_QUERY_RESULT_BYTES,
    MAXIMUM_LAUNCH_PROFILE_CANDIDATES, MAXIMUM_LAUNCH_PROFILE_CATALOG_BYTES,
    MAXIMUM_LAUNCH_PROFILE_ENTRIES, MAXIMUM_LAUNCH_SELECTION_ARGUMENTS,
    MAXIMUM_LAUNCH_SELECTION_BYTES, MAXIMUM_METADATA_PATCH_KEYS, MAXIMUM_METADATA_TOKEN_KEYS,
    MAXIMUM_METADATA_TTL_MILLISECONDS, MAXIMUM_RESULT_PUBLICATIONS,
    MAXIMUM_WORKSPACE_GROUP_CLOSE_ATTEMPTS, MAXIMUM_WORKSPACE_GROUP_CLOSE_MEMBERS,
    MAXIMUM_WORKSPACE_METADATA_READ_ROWS, MAXIMUM_WORKTREE_OPERATION_ATTEMPTS,
    MetadataReportDisposition, MetadataSource, MetadataTokenKey, MetadataTokenPatch,
    MetadataTokenValue, MetadataTokens, PublicationReadIntent, WorkbenchAction,
    WorkbenchClientViewRequest, WorkbenchClientViewResult, WorkbenchCommand,
    WorkbenchCommandOutcome, WorkbenchCommandResult, WorkbenchCommandUnavailableReason,
    WorkbenchLabel, WorkbenchLaunchProfileCatalogRead, WorkbenchLaunchProfileCatalogResult,
    WorkbenchPanelRelocateDestination, WorkbenchPanelSplitDirection, WorkbenchPanelTerminalTarget,
    WorkbenchPanelTransfer, WorkbenchPublicationOutcome, WorkbenchPublicationResult,
    WorkbenchRejectionReason, WorkbenchRequest, WorkbenchResponse, WorkbenchSnapshotOutcome,
    WorkbenchSnapshotResult, WorkbenchSnapshotUnavailableReason,
    WorkbenchTabCloseAttemptGeneration, WorkbenchTabCloseBlockedReason, WorkbenchTabClosePhase,
    WorkbenchTabClosePublicationReason, WorkbenchTabCloseRead, WorkbenchTabCloseReadOutcome,
    WorkbenchTabCloseReadResult, WorkbenchTabCloseResume, WorkbenchTabCloseResumeOutcome,
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
    WorkspaceMetadataAction, WorkspaceMetadataFailure, WorkspaceMetadataOutcome,
    WorkspaceMetadataRequest, WorkspaceMetadataResult, WorkspaceMetadataRow,
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
pub use launch_options::{LaunchOptions, PluginPaneSelector};
mod launch_environment;
pub use launch_environment::{
    LaunchEnvironment, MAXIMUM_LAUNCH_ENVIRONMENT_BYTES, MAXIMUM_LAUNCH_ENVIRONMENT_ENTRIES,
};

pub use command::{
    MAXIMUM_ENCODED_PANEL_METADATA_RESULT_BYTES, MAXIMUM_PANEL_METADATA_READ_ROWS,
    MAXIMUM_PANEL_METADATA_SOURCES, MetadataState, PanelMetadataAction, PanelMetadataEffective,
    PanelMetadataFailure, PanelMetadataField, PanelMetadataOutcome, PanelMetadataPatch,
    PanelMetadataPresentation, PanelMetadataRequest, PanelMetadataResult, PanelMetadataRow,
    PresentationMetadataRow,
};

pub use command::{
    AgentDetectionEvaluation, AgentDetectionEvidence, AgentDetectionManifest, AgentDetectionRule,
    AgentDetectionSample, AgentManifestSource, AgentManifestWarning, AgentManifestWarningKind,
    AgentScreenExplanation, MAXIMUM_ENCODED_PANEL_AGENT_RESULT_BYTES,
    MAXIMUM_PANEL_AGENT_READ_ROWS, MAXIMUM_TERMINAL_AGENT_CHANGES,
    MAXIMUM_TERMINAL_AGENT_STATE_ROWS, MAXIMUM_TERMINAL_TITLE_BYTES, MAXIMUM_TERMINAL_TITLE_ROWS,
    PanelAgentAction, PanelAgentDisposition, PanelAgentFailure, PanelAgentOutcome,
    PanelAgentReference, PanelAgentReport, PanelAgentRequest, PanelAgentResult, PanelAgentRow,
    PanelAgentSession, PanelAgentState, PanelAgentView, PresentationAgentRow, TerminalAgentChange,
    TerminalAgentExplanation, TerminalAgentObservation, TerminalAgentStateRow,
    TerminalAgentStateSnapshot, TerminalTitleObservation, TerminalTitleRow,
};

pub use command::agent_manifests::{
    AgentManifestActivation, AgentManifestSummary, AgentManifestsAction, AgentManifestsFailure,
    AgentManifestsOperation, AgentManifestsOutcome, AgentManifestsRequest, AgentManifestsResult,
    AgentManifestsSnapshot, MAXIMUM_ENCODED_AGENT_MANIFESTS_BYTES,
};

pub use command::agent_manifest_update::{
    AgentManifestAutomatic, AgentManifestCheckSource, AgentManifestMaintenance,
    AgentManifestProviderUpdate, AgentManifestUpdateDisposition, AgentManifestUpdateFailure,
    AgentManifestUpdateReport, valid_agent_manifest_catalog_url,
};

pub use command::configuration::{
    ConfigurationAction, ConfigurationFailure, ConfigurationOutcome, ConfigurationRequest,
    ConfigurationRestart, ConfigurationResult, ConfigurationShellMode, ConfigurationSnapshot,
    MAXIMUM_CONFIGURATION_BYTES,
};

pub use command::notification::{
    Notification, NotificationAction, NotificationFailure, NotificationOutcome,
    NotificationPosition, NotificationReason, NotificationRequest, NotificationResult,
    NotificationSound,
};

pub use command::{
    WindowTitle, WindowTitleAction, WindowTitleContext, WindowTitleFailure, WindowTitleOutcome,
    WindowTitleReason, WindowTitleRequest, WindowTitleResult, WindowTitleSnapshot,
};

pub use command::{
    ClientPopupSource, MAXIMUM_ENCODED_PLUGIN_REQUEST_BYTES, MAXIMUM_ENCODED_PLUGIN_RESULT_BYTES,
    PluginAction, PluginCommandFailure, PluginCommandLog, PluginFailure, PluginInvocationContext,
    PluginLogCursor, PluginOutcome, PluginPaneInfo, PluginPopupSize, PluginPopupSpec,
    PluginRequest, PluginResult, PluginWorker, PluginWorktreeContext, plugin_identifier,
    plugin_job_identifier,
};
