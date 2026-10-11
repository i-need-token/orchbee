//! Private matched-cohort Server-to-Worker admission framing over caller-owned byte streams.
//! Endpoint selection, peer authentication, admission state and process supervision remain with
//! the runtime owners; decoding a claim does not grant Worker authority.

mod codec;

pub use codec::{
    LocalSiteMessage, MAXIMUM_COHORT_BYTES, MAXIMUM_FRAME_PAYLOAD_BYTES, PROTOCOL_VERSION,
    ProtocolError, ReadMessageError, RejectionReason, WorkerHello, WorkerId, WriteMessageError,
    read_message, write_message,
};
