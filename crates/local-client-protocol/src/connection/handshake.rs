//! The `OBHS` exchange that opens every Client connection, shared by the platform adapters: one
//! offer or control request and its one reply, then the OBLC Client phase. The adapters own the
//! transport, its deadline, and the cancellation source.

use std::{
    io::{self, Read, Write},
    num::NonZeroU64,
};

use orchbee_connection_handshake::{
    AcceptorMessage, CapabilitySet, InitiatorMessage, Offer, OfferFailure, ReadError,
    conclude_offer, encode_initiator_frame, read_acceptor_message,
};
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
use orchbee_connection_handshake::{
    ControlRequest, EnvironmentName, Extensions, QueryResult, RequestFailure, RequestKind,
    RequestReply, conclude_request,
};

use super::ConnectClientError;
use crate::{LocalClientMessage, read_message, write_message};

/// Sends `offer` and accepts only an answer inside it, returning the capabilities both sides
/// listed. A rejection keeps the Server's facts; a reply that is not `OBHS` comes from a Server
/// that predates the handshake.
pub(super) fn offer(
    transport: &mut (impl Read + Write),
    offer: &Offer,
) -> Result<CapabilitySet, ConnectClientError> {
    send(transport, &InitiatorMessage::Offer(offer.clone()))?;
    conclude_offer(offer, receive(transport)?)
        .map(|answer| answer.capabilities)
        .map_err(|failure| match failure {
            OfferFailure::Rejected(rejection) => ConnectClientError::Refused(Box::new(rejection)),
            OfferFailure::InvalidAnswer | OfferFailure::UnexpectedReply => {
                ConnectClientError::UnexpectedResponse
            }
        })
}

/// Sends one `query` request and accepts only its `query-result`.
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
pub(super) fn query(
    transport: &mut (impl Read + Write),
    environment: EnvironmentName,
) -> Result<QueryResult, ConnectClientError> {
    send(
        transport,
        &InitiatorMessage::Request(ControlRequest {
            kind: RequestKind::Query,
            environment,
            extensions: Extensions::new(),
        }),
    )?;
    match conclude_request(RequestKind::Query, receive(transport)?) {
        Ok(RequestReply::Query(result)) => Ok(result),
        Err(RequestFailure::Rejected(rejection)) => {
            Err(ConnectClientError::Refused(Box::new(rejection)))
        }
        Ok(RequestReply::Drain(_) | RequestReply::Exit(_))
        | Err(RequestFailure::Unsupported(_) | RequestFailure::UnexpectedReply) => {
            Err(ConnectClientError::UnexpectedResponse)
        }
    }
}

/// Sends one `exit` request; its reply follows the Server's settlement, so the caller reads it
/// with [`exit_reply`] under its own deadline.
#[cfg(windows)]
pub(super) fn request_exit(
    transport: &mut impl Write,
    environment: EnvironmentName,
) -> Result<(), ConnectClientError> {
    send(
        transport,
        &InitiatorMessage::Request(ControlRequest {
            kind: RequestKind::Exit,
            environment,
            extensions: Extensions::new(),
        }),
    )
}

/// Accepts the `exit-result`, or the Server's `unsupported` reply from a build that serves no
/// `exit`.
#[cfg(windows)]
pub(super) fn exit_reply(
    transport: &mut impl Read,
) -> Result<super::ExitReply, ConnectClientError> {
    match conclude_request(RequestKind::Exit, receive(transport)?) {
        Ok(RequestReply::Exit(_)) => Ok(super::ExitReply::Ending),
        Err(RequestFailure::Unsupported(_)) => Ok(super::ExitReply::Unsupported),
        Err(RequestFailure::Rejected(rejection)) => {
            Err(ConnectClientError::Refused(Box::new(rejection)))
        }
        Ok(RequestReply::Query(_) | RequestReply::Drain(_))
        | Err(RequestFailure::UnexpectedReply) => Err(ConnectClientError::UnexpectedResponse),
    }
}

/// Enters the Client phase on an answered connection and returns the Server's connection
/// generation.
pub(super) fn enter_client_phase(
    transport: &mut (impl Read + Write),
) -> Result<NonZeroU64, ConnectClientError> {
    write_message(transport, &LocalClientMessage::ClientHello)?;
    match read_message(transport)? {
        LocalClientMessage::ClientAccepted { generation } => Ok(generation),
        LocalClientMessage::Rejected { reason } => Err(ConnectClientError::Rejected(reason)),
        LocalClientMessage::ClientHello
        | LocalClientMessage::SessionStopRequest(_)
        | LocalClientMessage::SessionStopResult(_) => Err(ConnectClientError::UnexpectedResponse),
    }
}

fn send(transport: &mut impl Write, message: &InitiatorMessage) -> Result<(), ConnectClientError> {
    // An offer or request of this build is far below the frame bound.
    let frame = encode_initiator_frame(message)
        .map_err(|_| ConnectClientError::Io(io::ErrorKind::InvalidInput.into()))?;
    transport.write_all(&frame)?;
    transport.flush()?;
    Ok(())
}

fn receive(transport: &mut impl Read) -> Result<AcceptorMessage, ConnectClientError> {
    read_acceptor_message(transport).map_err(|error| match error {
        ReadError::Io(error) => ConnectClientError::Io(error),
        ReadError::Closed => ConnectClientError::Io(io::ErrorKind::UnexpectedEof.into()),
        ReadError::ForeignProtocol => ConnectClientError::PredatesHandshake,
        ReadError::Malformed(_) => ConnectClientError::UnexpectedResponse,
    })
}
