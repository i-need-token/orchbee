//! The Server side of the `OBHS` exchange for fake Servers, which then script the Client phase.
use orchbee_connection_handshake::{
    AcceptorMessage, Answer, Build, CapabilitySet, ConnectionKind, EnvironmentName, Extensions,
    InitiatorMessage, Offer, ProductVersion, Rejection, RejectionReason, encode_acceptor_frame,
    read_initiator_message,
};
use orchbee_local_client_protocol::{LocalClientMessage, read_message};
use std::io::{self, Read, Write};

/// The test environment fake Servers share with the Clients they admit.
const TEST_ENVIRONMENT: &str = "ob-test-0000000000000000";

/// A Client offer in the fake Servers' environment.
pub fn offer() -> Offer {
    Offer::local(
        ConnectionKind::ClientServer,
        ProductVersion::new("0.1.0").expect("version"),
        Build::default(),
        EnvironmentName::new(TEST_ENVIRONMENT).expect("environment"),
    )
}

/// Reads the candidate's handshake frame and requires an offer.
pub fn read_offer(stream: &mut impl Read) -> io::Result<Offer> {
    match read_initiator_message(stream).map_err(io::Error::other)? {
        InitiatorMessage::Offer(offer) => Ok(offer),
        other => Err(io::Error::other(format!("expected an offer: {other:?}"))),
    }
}

/// Answers the candidate's offer at its highest generation; the Client phase follows.
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

/// Answers the offer and requires the Client phase's `ClientHello`.
pub fn expect_hello(stream: &mut (impl Read + Write)) -> io::Result<()> {
    answer_offer(stream)?;
    match read_message(stream).map_err(io::Error::other)? {
        LocalClientMessage::ClientHello => Ok(()),
        other => Err(io::Error::other(format!("expected ClientHello: {other:?}"))),
    }
}

/// Refuses the candidate's offer as a Server of `product_version` would.
pub fn refuse(
    stream: &mut (impl Read + Write),
    reason: RejectionReason,
    product_version: &str,
) -> io::Result<Rejection> {
    let offer = read_offer(stream)?;
    let rejection = Rejection {
        reason,
        generations: offer.generations,
        product_version: ProductVersion::new(product_version).map_err(io::Error::other)?,
        environment: offer.environment.name().clone(),
        extensions: Extensions::new(),
    };
    let reply = AcceptorMessage::Rejection(rejection.clone());
    stream.write_all(&encode_acceptor_frame(&reply).map_err(io::Error::other)?)?;
    stream.flush()?;
    Ok(rejection)
}
