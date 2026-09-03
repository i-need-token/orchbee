mod codec;

#[cfg(unix)]
mod posix_peer;

pub use codec::{
    LocalSiteMessage, MAXIMUM_COHORT_BYTES, MAXIMUM_FRAME_PAYLOAD_BYTES, PROTOCOL_VERSION,
    ProtocolError, ReadMessageError, RejectionReason, WorkerHello, WorkerId, WriteMessageError,
    read_message, write_message,
};

#[cfg(unix)]
pub use posix_peer::require_same_effective_user;
