//! Bounded, carrier-neutral requests and receipts between Server and Capability Nodes. These
//! types describe authorized terminal and job operations; runtime owners retain peer admission,
//! authorization state, process lifetime and recovery policy.

mod agent_drain;
mod codec;
mod error;
mod identity;
mod plugin_job;
pub use plugin_job::{
    ConfiguredCommandIntent, MAXIMUM_PLUGIN_INTENT_BYTES, MAXIMUM_PLUGIN_LOG_PAGE,
    MAXIMUM_PLUGIN_OUTPUT_BYTES, PluginCommandFailure, PluginCommandIntent, PluginCommandLog,
    PluginJobAction, PluginJobOutcome, PluginJobRefusal, PluginJobRequest, PluginJobResult,
};
mod worker_handoff;
mod worker_shutdown;
pub use worker_handoff::{
    WorkerHandoffAction, WorkerHandoffOutcome, WorkerHandoffPeer, WorkerHandoffRefusal,
    WorkerHandoffRequest, WorkerHandoffResult, WorkerHandoffTarget,
};
pub use worker_shutdown::{
    ShutdownBudget, ShutdownFailure, ShutdownRefusal, ShutdownStage, WorkerShutdownOutcome,
    WorkerShutdownRequest, WorkerShutdownResult,
};
mod terminal_cwd_validation;
mod terminal_editor;
pub use terminal_editor::{
    TerminalEditorCommandOutcome, TerminalEditorCommandRefusal, TerminalEditorCommandRequest,
    TerminalEditorCommandResult, TerminalEditorCommandSource, TerminalEditorSource,
};
mod terminal_graphics_decode;
mod terminal_launch;
mod terminal_lifecycle;
mod terminal_send;
mod terminal_titles;
mod terminal_viewer;
pub use terminal_graphics_decode::{MAXIMUM_GRAPHICS_PNG_BYTES, TerminalGraphicsDecodeRequest};
pub use terminal_titles::{
    MAXIMUM_TERMINAL_TITLE_BYTES, MAXIMUM_TERMINAL_TITLE_ROWS, TerminalTitleObservation,
    TerminalTitleRow, TerminalTitlesRequest, TerminalTitlesResult,
};

pub use terminal_viewer::{
    MAXIMUM_READ_ROWS, MAXIMUM_READ_ROWS_BYTES, MAXIMUM_VIEWER_RECORD_BYTES,
    MINIMUM_READ_ROWS_BYTES, TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest,
    TerminalViewerResult, ViewerHistoryDemand, ViewerProjectionDemand, ViewerRejection,
    ViewerRowDetail, ViewerRowsRejectedReason, ViewerSelectionExportCancelReason,
    ViewerSelectionExportFormat, ViewerSelectionGestureAction, ViewerSelectionGestureGeometry,
    ViewerSelectionGesturePosition, ViewerSelectionViewportPoint, ViewerSnapshotReason,
    ViewportRowOrigin,
};

pub use agent_drain::{
    AGENT_DRAIN_REPLY_KIND, AGENT_DRAIN_REQUEST_KIND, AgentDrainEntry, AgentDrainReply,
    AgentDrainRequest, EVENT_ENTRY_OVERHEAD_BYTES, GAP_ENTRY_BYTES, MAXIMUM_REPLY_ENTRIES,
    MAXIMUM_SIGNAL_PAYLOAD_BYTES, REPLY_ENTRY_BYTES, REPLY_HEADER_BYTES, REQUEST_PAYLOAD_BYTES,
};
pub use codec::{
    CapabilityNodeMessage, MAXIMUM_FRAME_PAYLOAD_BYTES, PROTOCOL_VERSION, decode_message,
    encode_message, read_message, write_message,
};
pub use error::{ProtocolError, ReadMessageError, WriteMessageError};
pub use identity::{
    HostCreationId, HostEpoch, HostTerminalId, LeaseId, OperationId, TerminalId, WorkerHandoffId,
    WorkerId,
};
pub use terminal_cwd_validation::{
    TerminalCwdValidationOutcome, TerminalCwdValidationRequest, TerminalCwdValidationResult,
};
pub use terminal_launch::{
    AbsoluteCwd, AuthorizedTerminalLaunch, AuthorizedTerminalStart, HostLaunchAccepted,
    HostLaunchState, HostStartAccepted, HostTerminalIdentity, LaunchSelection,
    LeaseExpiresAtUnixMilliseconds, MAXIMUM_AUTHORIZED_CWD_BYTES,
    MAXIMUM_LAUNCH_SELECTION_ARGUMENTS, MAXIMUM_LAUNCH_SELECTION_BYTES, StartTransition,
    TERMINAL_LAUNCH_CAPABILITY_VERSION, TerminalFence, TerminalLaunchCapabilityVersion,
    TerminalLaunchFailure, TerminalLaunchMode, TerminalLaunchOutcome, TerminalLaunchRejection,
    TerminalLaunchResult, TerminalOperationAuthorization, TerminalOperationLease,
    TerminalPixelGeometry, TerminalStartFailure, TerminalStartOutcome, TerminalStartRejection,
    TerminalStartResult, WorkerGeneration,
};
pub use terminal_lifecycle::{
    LIFECYCLE_INVENTORY_PAGE_TERMINALS, LifecycleCloseActivity, LifecycleHostFailure,
    LifecycleInventoryCursor, LifecycleInventoryFailure, LifecycleInventoryPage,
    LifecycleRejection, LifecycleStepOrdinal, LifecycleTargetObservation, LifecycleTerminalPhase,
    LifecycleUnknownReason, TERMINAL_LIFECYCLE_CAPABILITY_VERSION, TerminalLifecycleAction,
    TerminalLifecycleAuthorization, TerminalLifecycleCapabilityVersion, TerminalLifecycleOutcome,
    TerminalLifecycleRequest, TerminalLifecycleResult,
};

#[cfg(test)]
mod tests;

mod worktree_job;
mod worktree_mutation;
pub use worktree_job::{
    MAXIMUM_WORKTREE_JOB_DIRECTORY_BYTES, MAXIMUM_WORKTREE_JOB_RESULT_FIELDS_BYTES,
    WorktreeJobAction, WorktreeJobCancelOutcome, WorktreeJobOutcome, WorktreeJobPollOutcome,
    WorktreeJobRequest, WorktreeJobResult, WorktreeJobStage, WorktreeJobStartOutcome,
};
pub use worktree_mutation::{
    GitReference, MAXIMUM_GIT_REFERENCE_BYTES, MAXIMUM_WORKTREE_DIAGNOSTIC_BYTES,
    MAXIMUM_WORKTREE_MUTATION_FIELDS_BYTES, WorktreeCheckout, WorktreeCheckoutSelector,
    WorktreeMutation, WorktreeMutationCancelOutcome, WorktreeMutationCompletion,
    WorktreeMutationEffect, WorktreeMutationError, WorktreeMutationFailure, WorktreeMutationIntent,
    WorktreeMutationJobAction, WorktreeMutationOutcome, WorktreeMutationPollOutcome,
    WorktreeMutationRequest, WorktreeMutationResult, WorktreeMutationStage,
    WorktreeMutationStartOutcome,
};

mod send_stroke;
mod terminal_control;
pub use send_stroke::{SendKeyStroke, SendStrokeKey};
pub use terminal_control::{
    ControlInput, ControlRejection, HostDefaultColors, MAXIMUM_CONTROL_INPUT_BYTES,
    MAXIMUM_PASTE_CONTENT_BYTES, PasteAcceptedBytes, PasteContent, TerminalControlAction,
    TerminalControlOutcome, TerminalControlRequest, TerminalControlResult,
};
pub use terminal_send::{
    MAXIMUM_SEND_ACCEPTED_BYTES, MAXIMUM_SEND_ACCEPTED_ENCODED_BYTES, MAXIMUM_SEND_AGGREGATE_BYTES,
    MAXIMUM_SEND_BYTES_BYTES, MAXIMUM_SEND_PASTE_BYTES, MAXIMUM_SEND_RECORDS,
    MAXIMUM_SEND_TEXT_BYTES, SendAcceptedBytes, SendBytes, SendCleanupStatus, SendKey,
    SendPasteContent, SendProgress, SendRecord, SendRecords, SendRejection, SendText,
    SendTextInput, SendUnavailable, TerminalSendOutcome, TerminalSendRequest, TerminalSendResult,
};

mod launch_environment;
pub use launch_environment::{
    LaunchEnvironment, MAXIMUM_LAUNCH_ENVIRONMENT_BYTES, MAXIMUM_LAUNCH_ENVIRONMENT_ENTRIES,
};

mod plugin_terminal_environment;
pub use plugin_terminal_environment::{
    MAXIMUM_PLUGIN_TERMINAL_ENVIRONMENT_BYTES, PLUGIN_TERMINAL_ENVIRONMENT_NAMES,
    PluginTerminalEnvironment,
};
