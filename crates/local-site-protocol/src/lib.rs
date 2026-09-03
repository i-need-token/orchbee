mod codec;

pub use codec::{
    LocalSiteMessage, MAXIMUM_COHORT_BYTES, MAXIMUM_FRAME_PAYLOAD_BYTES, PROTOCOL_VERSION,
    ProtocolError, ReadMessageError, RejectionReason, WorkerHello, WorkerId, WriteMessageError,
    read_message, write_message,
};
