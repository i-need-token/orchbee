//! A scripted Server side of the `OBHS` exchange, so adapter tests reach the OBLC phase.

use std::io::{self, Read, Write};

use orchbee_connection_handshake::{
    AcceptorMessage, Answer, Build, CapabilitySet, ConnectionKind, EnvironmentName, Extensions,
    InitiatorMessage, Offer, ProductVersion, encode_acceptor_frame, read_initiator_message,
};

/// A Client offer of a test environment.
pub fn offer() -> Offer {
    Offer::local(
        ConnectionKind::ClientServer,
        ProductVersion::new("0.1.0").expect("version"),
        Build::default(),
        EnvironmentName::new("ob-test-0000000000000000").expect("environment"),
    )
}

/// Reads the candidate's one handshake frame and requires an offer.
pub fn read_offer(stream: &mut impl Read) -> io::Result<Offer> {
    match read_initiator_message(stream).map_err(io::Error::other)? {
        InitiatorMessage::Offer(offer) => Ok(offer),
        other => Err(io::Error::other(format!("expected an offer: {other:?}"))),
    }
}

/// Answers the candidate's offer at its highest generation.
pub fn answer_offer(stream: &mut (impl Read + Write)) -> io::Result<()> {
    let offer = read_offer(stream)?;
    let answer = AcceptorMessage::Answer(Answer {
        generation: offer.generations.highest(),
        capabilities: CapabilitySet::default(),
        product_version: offer.product_version,
        build: Build::default(),
        extensions: Extensions::new(),
    });
    stream.write_all(&encode_acceptor_frame(&answer).map_err(io::Error::other)?)?;
    stream.flush()
}
