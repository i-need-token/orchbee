//! Read process-owned diagnostics before allocating any Client Connection state.
use orchbee_local_client_protocol::{
    ClientHello, LocalClientMessage, SERVER_FEATURE_AGENT_OBSERVATION, SERVER_FEATURE_CLIENT_VIEW,
    SERVER_FEATURE_RENDER_ROWS, ServerMetadata, ServerMode, write_message,
};
use orchbee_local_ipc::LocalStream;
use std::num::{NonZeroU8, NonZeroU32};

pub(super) fn respond(stream: &mut LocalStream, cohort: &str, mode: ServerMode) {
    let Ok(cohort) = ClientHello::new(cohort) else {
        return;
    };
    let metadata = ServerMetadata::new(
        env!("CARGO_PKG_VERSION"),
        cohort,
        NonZeroU32::new(std::process::id()).expect("Windows process ID is nonzero"),
        NonZeroU8::new(orchbee_client_server_protocol::PROTOCOL_VERSION)
            .expect("OBCS version is nonzero"),
        mode,
        SERVER_FEATURE_RENDER_ROWS | SERVER_FEATURE_CLIENT_VIEW | SERVER_FEATURE_AGENT_OBSERVATION,
    );
    if let Ok(metadata) = metadata {
        let _delivery = write_message(stream, &LocalClientMessage::ServerMetadata(metadata));
    }
}

#[cfg(test)]
mod tests;
