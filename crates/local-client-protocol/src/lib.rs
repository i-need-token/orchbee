mod codec;

#[cfg(any(unix, windows))]
mod connection;

pub use codec::{
    ClientHello, LocalClientMessage, MAXIMUM_COHORT_BYTES, MAXIMUM_FRAME_PAYLOAD_BYTES,
    PROTOCOL_VERSION, ProtocolError, ReadMessageError, RejectionReason, WriteMessageError,
    read_message, write_message,
};

#[cfg(any(unix, windows))]
pub use connection::{
    ClientSessionConnection, ConnectClientError, UNIX_SOCKET_PATH_CAPACITY, unix_socket_path_fits,
};
