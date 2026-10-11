//! Bounded local Client admission and control messages with a same-user connecting adapter.
//! Callers own endpoint selection, invocation deadlines and the accepted stream; Server owns
//! Session admission and lifecycle decisions. This crate neither activates a Site nor interprets
//! the application protocol that follows admission.

mod codec;
mod server_metadata;
pub use server_metadata::{
    MAXIMUM_SERVER_VERSION_BYTES, SERVER_FEATURE_AGENT_OBSERVATION, SERVER_FEATURE_CLIENT_VIEW,
    SERVER_FEATURE_LIVE_HANDOFF, SERVER_FEATURE_RENDER_ROWS, SERVER_KNOWN_FEATURES, ServerMetadata,
    ServerMode,
};
mod site_stop;
pub use site_stop::{
    SiteStopRequest, SiteStopResult, SiteStopStatus, StopAction, StopAttempt, StopFailure, StopId,
    StopPhase,
};

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
#[cfg(windows)]
pub use connection::{
    SiteStopExchangeError, exchange_server_metadata_before, exchange_site_stop_before,
};
