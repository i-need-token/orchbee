mod codec;

#[cfg(unix)]
mod connection;

pub use codec::{
    ClientHello, LocalClientMessage, MAXIMUM_COHORT_BYTES, MAXIMUM_FRAME_PAYLOAD_BYTES,
    PROTOCOL_VERSION, ProtocolError, ReadMessageError, RejectionReason, WriteMessageError,
    read_message, write_message,
};

#[cfg(unix)]
pub use connection::{ClientSessionConnection, ConnectClientError};
