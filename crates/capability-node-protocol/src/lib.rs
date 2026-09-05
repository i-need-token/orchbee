mod codec;
mod error;
mod identity;
mod terminal_launch;
mod terminal_viewer;

pub use terminal_viewer::{
    MAXIMUM_VIEWER_RECORD_BYTES, TerminalViewerAction, TerminalViewerOutcome,
    TerminalViewerRequest, TerminalViewerResult, ViewerRejection, ViewerSnapshotReason,
};

pub use codec::{
    CapabilityNodeMessage, MAXIMUM_FRAME_PAYLOAD_BYTES, PROTOCOL_VERSION, decode_message,
    encode_message, read_message, write_message,
};
pub use error::{ProtocolError, ReadMessageError, WriteMessageError};
pub use identity::{
    HostCreationId, HostEpoch, HostTerminalId, LeaseId, OperationId, TerminalId, WorkerId,
};
pub use terminal_launch::{
    AbsoluteCwd, AuthorizedTerminalLaunch, AuthorizedTerminalStart, HostLaunchAccepted,
    HostLaunchState, HostStartAccepted, HostTerminalIdentity, LeaseExpiresAtUnixMilliseconds,
    MAXIMUM_AUTHORIZED_CWD_BYTES, StartTransition, TERMINAL_LAUNCH_CAPABILITY_VERSION,
    TerminalFence, TerminalLaunchCapabilityVersion, TerminalLaunchFailure, TerminalLaunchMode,
    TerminalLaunchOutcome, TerminalLaunchRejection, TerminalLaunchResult,
    TerminalOperationAuthorization, TerminalOperationLease, TerminalPixelGeometry,
    TerminalStartFailure, TerminalStartOutcome, TerminalStartRejection, TerminalStartResult,
    WorkerGeneration,
};

#[cfg(test)]
mod tests;

mod terminal_control;
pub use terminal_control::{
    ControlInput, ControlRejection, MAXIMUM_CONTROL_INPUT_BYTES, TerminalControlAction,
    TerminalControlOutcome, TerminalControlRequest, TerminalControlResult,
};
