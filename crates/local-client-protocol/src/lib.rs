//! Bounded local Client phase and Session stop messages that follow the `OBHS` handshake, with a
//! same-user connecting adapter that runs the handshake first. Callers own the offer, endpoint
//! selection, invocation deadlines and the accepted stream; Server owns Connection admission and
//! lifecycle decisions. This crate neither activates a Session nor interprets the application
//! protocol that follows admission.

mod codec;
mod session_stop;
pub use session_stop::{
    SessionStopRequest, SessionStopResult, SessionStopStatus, StopAction, StopAttempt, StopFailure,
    StopId, StopPhase,
};

#[cfg(any(unix, windows))]
mod connection;

pub use codec::{
    LocalClientMessage, MAXIMUM_FRAME_PAYLOAD_BYTES, PROTOCOL_VERSION, ProtocolError,
    ReadMessageError, RejectionReason, WriteMessageError, read_message, write_message,
};

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
pub use connection::exchange_query_before;
#[cfg(any(unix, windows))]
pub use connection::{
    ClientConnection, ConnectClientError, ExitReply, UNIX_SOCKET_PATH_CAPACITY,
    unix_socket_path_fits,
};
#[cfg(windows)]
pub use connection::{
    SessionStopExchangeError, exchange_exit_before, exchange_session_stop_before,
};
