mod agent_drain;
mod codec;
mod error;
mod identity;
mod terminal_cwd_validation;
mod terminal_launch;
mod terminal_lifecycle;
mod terminal_send;
mod terminal_viewer;

pub use terminal_viewer::{
    MAXIMUM_READ_ROWS, MAXIMUM_READ_ROWS_BYTES, MAXIMUM_VIEWER_RECORD_BYTES,
    MINIMUM_READ_ROWS_BYTES, TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest,
    TerminalViewerResult, ViewerHistoryDemand, ViewerProjectionDemand, ViewerRejection,
    ViewerRowsRejectedReason, ViewerSnapshotReason, ViewportRowOrigin,
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
    HostCreationId, HostEpoch, HostTerminalId, LeaseId, OperationId, TerminalId, WorkerId,
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
    LIFECYCLE_INVENTORY_PAGE_TERMINALS, LifecycleHostFailure, LifecycleInventoryCursor,
    LifecycleInventoryFailure, LifecycleInventoryPage, LifecycleRejection, LifecycleStepOrdinal,
    LifecycleTargetObservation, LifecycleTerminalPhase, LifecycleUnknownReason,
    TERMINAL_LIFECYCLE_CAPABILITY_VERSION, TerminalLifecycleAction, TerminalLifecycleAuthorization,
    TerminalLifecycleCapabilityVersion, TerminalLifecycleOutcome, TerminalLifecycleRequest,
    TerminalLifecycleResult,
};

#[cfg(test)]
mod tests;

mod worktree_job;
pub use worktree_job::{
    MAXIMUM_WORKTREE_JOB_DIRECTORY_BYTES, MAXIMUM_WORKTREE_JOB_RESULT_FIELDS_BYTES,
    WorktreeJobAction, WorktreeJobCancelOutcome, WorktreeJobOutcome, WorktreeJobPollOutcome,
    WorktreeJobRequest, WorktreeJobResult, WorktreeJobStage, WorktreeJobStartOutcome,
};

mod terminal_control;
pub use terminal_control::{
    ControlInput, ControlRejection, MAXIMUM_CONTROL_INPUT_BYTES, MAXIMUM_PASTE_CONTENT_BYTES,
    PasteAcceptedBytes, PasteContent, TerminalControlAction, TerminalControlOutcome,
    TerminalControlRequest, TerminalControlResult,
};
pub use terminal_send::{
    MAXIMUM_SEND_ACCEPTED_BYTES, MAXIMUM_SEND_TEXT_BYTES, SendAcceptedBytes, SendCleanupStatus,
    SendKey, SendRejection, SendText, SendTextInput, SendUnavailable, TerminalSendOutcome,
    TerminalSendPayload, TerminalSendRequest, TerminalSendResult,
};
