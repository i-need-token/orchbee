//! The Client side of the `OBHS` exchange against the spawned Server.

use std::io::{self, Read, Write};

use orchbee_connection_handshake::{
    AcceptorMessage, Build, ConnectionKind, InitiatorMessage, Offer, ProductVersion,
    encode_initiator_frame, read_acceptor_message,
};

/// This test build's Client offer, in the spawned Server's environment.
pub(super) fn offer() -> Offer {
    Offer::local(
        ConnectionKind::ClientServer,
        ProductVersion::new(env!("CARGO_PKG_VERSION")).expect("version"),
        Build::default(),
        orchbee_local_runtime::environment_name(),
    )
}

/// The encoded offer, for tests that coalesce it with later bytes.
pub(super) fn offer_frame() -> io::Result<Vec<u8>> {
    encode_initiator_frame(&InitiatorMessage::Offer(offer())).map_err(io::Error::other)
}

/// Offers this build's connection and reads the reply.
pub(super) fn send_offer(stream: &mut (impl Read + Write)) -> io::Result<AcceptorMessage> {
    stream.write_all(&offer_frame()?)?;
    read_acceptor_message(stream).map_err(io::Error::other)
}

/// Offers and requires an answer; the stream is then in the Client phase.
pub(super) fn answered(stream: &mut (impl Read + Write)) -> io::Result<()> {
    match send_offer(stream)? {
        AcceptorMessage::Answer(_) => Ok(()),
        other => Err(io::Error::other(format!(
            "offer was not answered: {other:?}"
        ))),
    }
}
