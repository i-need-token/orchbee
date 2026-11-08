//! The Client side of the `OBHS` exchange for in-process admission tests.

use std::io::{self, Read, Write};

use orchbee_connection_handshake::{
    AcceptorMessage, Answer, Build, ConnectionKind, ControlRequest, EnvironmentName, Extensions,
    InitiatorMessage, Offer, ProductVersion, QueryResult, RequestKind, encode_initiator_frame,
    read_acceptor_message,
};
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};

/// This test build's Client offer.
pub(crate) fn offer() -> Offer {
    Offer::local(
        ConnectionKind::ClientServer,
        ProductVersion::new("0.1.0").expect("version"),
        Build::default(),
        orchbee_local_runtime::environment_name(),
    )
}

/// Sends one handshake frame and reads the one reply.
pub(crate) fn exchange(
    stream: &mut (impl Read + Write),
    message: &InitiatorMessage,
) -> io::Result<AcceptorMessage> {
    stream.write_all(&encode_initiator_frame(message).map_err(io::Error::other)?)?;
    stream.flush()?;
    read_acceptor_message(stream).map_err(io::Error::other)
}

/// Offers this build's connection and requires an answer.
pub(crate) fn answered(stream: &mut (impl Read + Write)) -> io::Result<Answer> {
    match exchange(stream, &InitiatorMessage::Offer(offer()))? {
        AcceptorMessage::Answer(answer) => Ok(answer),
        other => Err(io::Error::other(format!(
            "offer was not answered: {other:?}"
        ))),
    }
}

/// Completes the handshake and the Client phase; the stream is then ready for application
/// records.
pub(crate) fn admit(stream: &mut (impl Read + Write)) -> io::Result<()> {
    answered(stream)?;
    write_message(stream, &LocalClientMessage::ClientHello).map_err(io::Error::other)?;
    match read_message(stream).map_err(io::Error::other)? {
        LocalClientMessage::ClientAccepted { .. } => Ok(()),
        other => Err(io::Error::other(format!(
            "Client phase was not accepted: {other:?}"
        ))),
    }
}

/// Sends a `query` request in `environment` and reads its reply.
pub(crate) fn query(
    stream: &mut (impl Read + Write),
    environment: EnvironmentName,
) -> io::Result<AcceptorMessage> {
    exchange(
        stream,
        &InitiatorMessage::Request(ControlRequest {
            kind: RequestKind::Query,
            environment,
            extensions: Extensions::new(),
        }),
    )
}

/// Requires a `query-result`.
pub(crate) fn query_result(reply: AcceptorMessage) -> io::Result<QueryResult> {
    match reply {
        AcceptorMessage::QueryResult(result) => Ok(result),
        other => Err(io::Error::other(format!(
            "expected a query result: {other:?}"
        ))),
    }
}
