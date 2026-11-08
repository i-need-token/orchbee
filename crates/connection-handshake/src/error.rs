//! Matchable outcomes of constructing values, encoding, decoding, and reading `OBHS` frames. They
//! carry no record bytes; presentation belongs to each endpoint.

use std::{error::Error, fmt, io};

/// A value outside its wire grammar or bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidValue;

impl fmt::Display for InvalidValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("value violates its OBHS grammar or bounds")
    }
}

impl Error for InvalidValue {}

/// Why bytes are not exactly one valid `OBHS` frame for the expected direction. An acceptor
/// answers every variant with a `malformed` rejection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodeError {
    /// The bytes do not start with `OBHS`.
    Magic,
    /// The declared payload length is 0 or above [`MAXIMUM_PAYLOAD_BYTES`](crate::MAXIMUM_PAYLOAD_BYTES).
    Length,
    /// The bytes end before the header or the declared payload does.
    Truncated,
    /// Bytes follow the declared payload.
    TrailingBytes,
    /// The payload is not one strict UTF-8 JSON object: a syntax error, a duplicate member, or
    /// nesting deeper than the contract admits.
    Encoding,
    /// The object is not a valid message for the expected direction.
    Message,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Magic => "frame does not start with OBHS",
            Self::Length => "frame payload length is out of bounds",
            Self::Truncated => "frame ends before its declared payload",
            Self::TrailingBytes => "bytes follow the frame payload",
            Self::Encoding => "frame payload is not one strict UTF-8 JSON object",
            Self::Message => "frame payload is not a valid message for this direction",
        })
    }
}

impl Error for DecodeError {}

/// Why reading one frame from a stream failed.
#[derive(Debug)]
pub enum ReadError {
    /// The stream failed, including a read timeout the caller configured.
    Io(io::Error),
    /// The stream ended before a complete frame.
    Closed,
    /// The first bytes differ from `OBHS`. From a reply, the peer predates the handshake and is
    /// the older side.
    ForeignProtocol,
    /// The frame has an out-of-bounds length or an invalid payload; never
    /// [`DecodeError::Magic`], [`DecodeError::Truncated`], or [`DecodeError::TrailingBytes`].
    Malformed(DecodeError),
}

impl fmt::Display for ReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(_) => formatter.write_str("handshake stream failed"),
            Self::Closed => formatter.write_str("handshake stream ended before a complete frame"),
            Self::ForeignProtocol => formatter.write_str("peer does not speak OBHS"),
            Self::Malformed(error) => error.fmt(formatter),
        }
    }
}

impl Error for ReadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Malformed(error) => Some(error),
            Self::Closed | Self::ForeignProtocol => None,
        }
    }
}

/// Why a message cannot be encoded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EncodeError {
    /// The encoded payload exceeds [`MAXIMUM_PAYLOAD_BYTES`](crate::MAXIMUM_PAYLOAD_BYTES).
    PayloadTooLarge,
}

impl fmt::Display for EncodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("encoded OBHS payload exceeds the frame bound")
    }
}

impl Error for EncodeError {}
