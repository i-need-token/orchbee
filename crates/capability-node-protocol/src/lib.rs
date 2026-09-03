mod codec;
mod error;
mod identity;
mod terminal_launch;

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
    TerminalFence, TerminalLaunchCapabilityVersion, TerminalLaunchMode, TerminalLaunchOutcome,
    TerminalLaunchResult, TerminalOperationAuthorization, TerminalOperationFailure,
    TerminalOperationLease, TerminalOperationRejection, TerminalPixelGeometry,
    TerminalStartOutcome, TerminalStartResult, WorkerGeneration,
};

#[cfg(test)]
mod tests;
