//! The record of one frame on either machine connection, decoded for its sender: control messages
//! by class — request, reply, `unsupported`, event, `bye`, and the two unknown cases — channel
//! records, `data`, `credit`, and `ping`/`pong`. Decoding is stateless: it checks the encoding
//! layer, closed objects, value grammars, and which end may send a known record. Whether a frame
//! fits the channels and ledgers of a running connection is the channel layer's check.

pub(crate) mod hub;
pub(crate) mod link;

use std::{error::Error, fmt};

use orchbee_connection_handshake::Extensions;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Map, Value};

use crate::{
    Frame, FrameError, FrameKind, Id, Name,
    payload::{self, EXTENSIONS},
    protocol::{Members, Protocol, Refused},
    value::{ResetReason, name_enum},
};

/// The largest increment of one `credit` frame and the largest outstanding credit of a channel
/// direction.
pub const MAXIMUM_CREDIT: u32 = 4_194_304;
/// The suffix of every reply type but `unsupported`.
const REPLY_SUFFIX: &str = "-result";
const UNSUPPORTED: &str = "unsupported";
const BYE: &str = "bye";

/// An end of a connection: the `OBHS` initiator or acceptor.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Role {
    /// The end that sent the offer: it opens odd channel ids.
    Initiator,
    /// The end that answered: it opens even channel ids.
    Acceptor,
}

impl Role {
    /// The fixture name, `initiator` or `acceptor`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Initiator => "initiator",
            Self::Acceptor => "acceptor",
        }
    }

    /// The role `name` denotes, if any.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        [Self::Initiator, Self::Acceptor]
            .into_iter()
            .find(|role| role.name() == name)
    }

    /// The other end.
    #[must_use]
    pub const fn peer(self) -> Self {
        match self {
            Self::Initiator => Self::Acceptor,
            Self::Acceptor => Self::Initiator,
        }
    }

    /// The end whose parity a nonzero channel id has.
    #[must_use]
    pub const fn opener_of(channel: u32) -> Self {
        if channel % 2 == 1 {
            Self::Initiator
        } else {
            Self::Acceptor
        }
    }
}

/// The first layer that refuses a payload. Every variant is a violation that ends the connection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordError {
    /// The payload is not one strict JSON object under the payload rules, or holds `null` or a
    /// number that is not a plain integer token outside `extensions`.
    Encoding,
    /// The object is not exactly a record of its connection, or travels in a direction the
    /// contract does not give it.
    Record,
}

impl fmt::Display for RecordError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Encoding => "frame payload breaks the JSON payload rules",
            Self::Record => "frame payload is not a record of its connection and direction",
        })
    }
}

impl Error for RecordError {}

impl From<Refused> for RecordError {
    fn from(Refused: Refused) -> Self {
        Self::Record
    }
}

/// Why a record cannot be encoded as one frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EncodeError {
    /// The JSON payload exceeds 65,536 bytes.
    PayloadTooLarge,
    /// The channel or payload does not fit the frame kind.
    Frame(FrameError),
    /// A `credit` value outside 1 to 4,194,304.
    Credit,
}

impl fmt::Display for EncodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PayloadTooLarge => formatter.write_str("encoded payload exceeds the frame bound"),
            Self::Frame(error) => error.fmt(formatter),
            Self::Credit => formatter.write_str("credit is outside 1 to 4,194,304"),
        }
    }
}

impl Error for EncodeError {}

name_enum!(
    /// Why an end closes its carrier; it grows with fallback `unknown`.
    pub enum ByeReason {
        /// The end finished its work.
        Done = "done",
        /// The end is stopping.
        Stopping = "stopping",
        /// The end detected a violation.
        Violation = "violation",
        /// The helper heard nothing for 90 seconds.
        Idle = "idle",
        /// A reason this build does not know.
        Unknown = "unknown",
    }
);

/// A known channel kind with its `open` members, or a kind this build does not know, which the
/// responder resets as `unsupported` after reading only `kind`.
#[derive(Clone, Debug, PartialEq)]
pub enum Open<P: Protocol> {
    /// A known kind.
    Known {
        /// The kind and its members.
        channel: P::Channel,
        /// The payload's `extensions`.
        extensions: Extensions,
    },
    /// A kind this build does not know.
    Unknown {
        /// The kind's name.
        kind: Name,
    },
}

/// The responder's result of a channel: its outcome with result members.
#[derive(Clone, Debug, PartialEq)]
pub struct Close<P: Protocol> {
    /// The outcome; a name the channel kind does not list reads as the fallback.
    pub outcome: P::Outcome,
    /// The payload's `extensions`.
    pub extensions: Extensions,
}

/// A control message, classified by its shape.
#[derive(Clone, Debug, PartialEq)]
pub enum Message<P: Protocol> {
    /// A known request.
    Request {
        /// Greater than every earlier request id of its sender.
        id: Id,
        /// The request.
        request: P::Request,
        /// The payload's `extensions`.
        extensions: Extensions,
    },
    /// A request of a type this build does not know, answered with `unsupported`; only `type` and
    /// `id` were read.
    UnknownRequest {
        /// The request's id.
        id: Id,
        /// The request's type.
        request: Name,
    },
    /// A known reply.
    Reply {
        /// The id of the request it answers.
        id: Id,
        /// The reply.
        reply: P::Reply,
        /// The payload's `extensions`.
        extensions: Extensions,
    },
    /// The reply to a request of a type the receiver does not know.
    Unsupported {
        /// The id of the request it answers.
        id: Id,
        /// The unknown request type.
        request: Name,
        /// The payload's `extensions`.
        extensions: Extensions,
    },
    /// A known event.
    Event {
        /// The event.
        event: P::Event,
        /// The payload's `extensions`.
        extensions: Extensions,
    },
    /// An event of a type this build does not know, ignored; only `type` was read.
    UnknownEvent {
        /// The event's type.
        event: Name,
    },
    /// The best-effort last frame of an end before it closes the carrier.
    Bye {
        /// Why the end closes.
        reason: ByeReason,
        /// The payload's `extensions`.
        extensions: Extensions,
    },
}

/// The record of one frame.
#[derive(Clone, Debug, PartialEq)]
pub enum Record<P: Protocol> {
    /// A control message on channel 0.
    Message(Message<P>),
    /// A channel's opening record.
    Open(Open<P>),
    /// A channel's accepting record.
    Accept {
        /// The payload's `extensions`.
        extensions: Extensions,
    },
    /// Channel bytes.
    Data(Vec<u8>),
    /// A credit grant from 1 to 4,194,304 bytes.
    Credit(u32),
    /// Its sender sends no more `data` on the channel.
    End,
    /// The responder's result.
    Close(Close<P>),
    /// An immediate close of the channel.
    Reset {
        /// The reason, read among the connection's reasons.
        reason: ResetReason,
        /// The payload's `extensions`.
        extensions: Extensions,
    },
    /// Eight bytes to echo.
    Ping([u8; 8]),
    /// The echo of a `ping`.
    Pong([u8; 8]),
}

impl<P: Protocol> Record<P> {
    /// Decodes `frame` as `sender` sent it on connection `P`.
    ///
    /// # Errors
    ///
    /// Returns [`RecordError::Encoding`] for a JSON payload that breaks the payload rules and
    /// [`RecordError::Record`] for an object that is not exactly a record of the connection or
    /// travels in a direction the contract does not give it.
    pub fn decode(sender: Role, frame: &Frame) -> Result<Self, RecordError> {
        let bytes = frame.payload();
        match frame.kind() {
            FrameKind::Data => Ok(Self::Data(bytes.to_vec())),
            FrameKind::End => Ok(Self::End),
            FrameKind::Credit => {
                let value = u32::from_be_bytes(fixed(bytes)?);
                if (1..=MAXIMUM_CREDIT).contains(&value) {
                    Ok(Self::Credit(value))
                } else {
                    Err(RecordError::Record)
                }
            }
            FrameKind::Ping => Ok(Self::Ping(fixed(bytes)?)),
            FrameKind::Pong => Ok(Self::Pong(fixed(bytes)?)),
            FrameKind::Message => decode_message(sender, object(bytes)?).map(Self::Message),
            FrameKind::Open => decode_open(sender, object(bytes)?).map(Self::Open),
            FrameKind::Accept => {
                let mut members = object(bytes)?;
                let extensions = take_extensions(&mut members)?;
                closed::<NoMembers>(members)?;
                Ok(Self::Accept { extensions })
            }
            FrameKind::Close => {
                let mut members = object(bytes)?;
                let extensions = take_extensions(&mut members)?;
                Ok(Self::Close(Close {
                    outcome: P::decode_outcome(members)?,
                    extensions,
                }))
            }
            FrameKind::Reset => {
                let mut members = object(bytes)?;
                let extensions = take_extensions(&mut members)?;
                let reason = closed::<WireReset>(members)?.reason;
                Ok(Self::Reset {
                    reason: ResetReason::read(reason.as_str(), P::RESET_REASONS),
                    extensions,
                })
            }
        }
    }

    /// Encodes the record as one frame on `channel`; `message`, `ping`, and `pong` ignore it and
    /// travel on channel 0.
    ///
    /// # Errors
    ///
    /// Returns [`EncodeError`] for a payload above its bound, a channel the kind does not admit, or
    /// a credit outside its range.
    pub fn encode(&self, channel: u32) -> Result<Frame, EncodeError> {
        let (kind, channel, bytes) = match self {
            Self::Message(message) => (FrameKind::Message, 0, encode_message(message)?),
            Self::Open(open) => (FrameKind::Open, channel, encode_open(open)?),
            Self::Accept { extensions } => (
                FrameKind::Accept,
                channel,
                encode_payload(with_extensions(Map::new(), extensions))?,
            ),
            Self::Data(bytes) => (FrameKind::Data, channel, bytes.clone()),
            Self::Credit(value) => {
                if !(1..=MAXIMUM_CREDIT).contains(value) {
                    return Err(EncodeError::Credit);
                }
                (FrameKind::Credit, channel, value.to_be_bytes().to_vec())
            }
            Self::End => (FrameKind::End, channel, Vec::new()),
            Self::Close(close) => (
                FrameKind::Close,
                channel,
                encode_payload(with_extensions(
                    P::encode_outcome(&close.outcome),
                    &close.extensions,
                ))?,
            ),
            Self::Reset { reason, extensions } => (
                FrameKind::Reset,
                channel,
                encode_payload(with_extensions(reason_members(*reason), extensions))?,
            ),
            Self::Ping(bytes) => (FrameKind::Ping, 0, bytes.to_vec()),
            Self::Pong(bytes) => (FrameKind::Pong, 0, bytes.to_vec()),
        };
        Frame::new(kind, channel, bytes).map_err(EncodeError::Frame)
    }
}

fn fixed<const BYTES: usize>(bytes: &[u8]) -> Result<[u8; BYTES], RecordError> {
    <[u8; BYTES]>::try_from(bytes).map_err(|_| RecordError::Record)
}

fn object(bytes: &[u8]) -> Result<Members, RecordError> {
    payload::parse(bytes).ok_or(RecordError::Encoding)
}

fn encode_payload(members: Members) -> Result<Vec<u8>, EncodeError> {
    payload::encode(members).ok_or(EncodeError::PayloadTooLarge)
}

/// Takes the top-level `extensions` member, an empty set when absent.
fn take_extensions(members: &mut Members) -> Result<Extensions, Refused> {
    members
        .remove(EXTENSIONS)
        .map_or(Ok(Extensions::new()), |value| {
            serde_json::from_value(value).map_err(|_| Refused)
        })
}

/// Takes the string member `name` when it follows the name grammar.
fn take_name(members: &mut Members, name: &str) -> Result<Name, Refused> {
    match members.remove(name) {
        Some(Value::String(text)) => Name::new(text).map_err(|_| Refused),
        _ => Err(Refused),
    }
}

/// Decodes `members` as the closed wire object `Wire`.
pub(crate) fn closed<Wire: DeserializeOwned>(members: Members) -> Result<Wire, Refused> {
    serde_json::from_value(Value::Object(members)).map_err(|_| Refused)
}

/// The members of the wire object `wire`.
pub(crate) fn members_of(wire: &impl Serialize) -> Members {
    match serde_json::to_value(wire) {
        Ok(Value::Object(members)) => members,
        _ => unreachable!("record wire structs serialize to JSON objects"),
    }
}

fn with_extensions(mut members: Members, extensions: &Extensions) -> Members {
    if !extensions.is_empty() {
        members.insert(
            EXTENSIONS.to_owned(),
            serde_json::to_value(extensions).expect("extensions serialize to a JSON object"),
        );
    }
    members
}

fn reason_members(reason: ResetReason) -> Members {
    let mut members = Map::new();
    members.insert(
        "reason".to_owned(),
        Value::String(reason.as_str().to_owned()),
    );
    members
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct NoMembers {}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct WireReset {
    reason: Name,
}

#[derive(serde::Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireId {
    id: Id,
}

#[derive(serde::Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireUnsupported {
    request: Name,
}

#[derive(serde::Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireBye {
    reason: Name,
}

fn decode_open<P: Protocol>(sender: Role, mut members: Members) -> Result<Open<P>, RecordError> {
    let kind = take_name(&mut members, "kind")?;
    let extensions = members.remove(EXTENSIONS);
    match P::decode_open(kind.as_str(), members)? {
        Some(channel) if sender == Role::Initiator => Ok(Open::Known {
            channel,
            extensions: extensions.map_or(Ok(Extensions::new()), |value| {
                serde_json::from_value(value).map_err(|_| RecordError::Record)
            })?,
        }),
        Some(_) => Err(RecordError::Record),
        None => Ok(Open::Unknown { kind }),
    }
}

fn encode_open<P: Protocol>(open: &Open<P>) -> Result<Vec<u8>, EncodeError> {
    let (kind, members) = match open {
        Open::Known {
            channel,
            extensions,
        } => {
            let (kind, members) = P::encode_open(channel);
            (kind.to_owned(), with_extensions(members, extensions))
        }
        Open::Unknown { kind } => (kind.as_str().to_owned(), Map::new()),
    };
    let mut members = members;
    members.insert("kind".to_owned(), Value::String(kind));
    encode_payload(members)
}

fn decode_message<P: Protocol>(
    sender: Role,
    mut members: Members,
) -> Result<Message<P>, RecordError> {
    let name = take_name(&mut members, "type")?;
    let name = name.as_str();
    if let Some(id) = members.remove("id") {
        let id: Id = serde_json::from_value(id).map_err(|_| RecordError::Record)?;
        if name == UNSUPPORTED {
            let extensions = take_extensions(&mut members)?;
            let request = closed::<WireUnsupported>(members)?.request;
            return Ok(Message::Unsupported {
                id,
                request,
                extensions,
            });
        }
        if let Some(request) = name.strip_suffix(REPLY_SUFFIX) {
            let extensions = take_extensions(&mut members)?;
            return match P::decode_reply(request, members)? {
                Some(reply) if sender == Role::Acceptor => Ok(Message::Reply {
                    id,
                    reply,
                    extensions,
                }),
                Some(_) | None => Err(RecordError::Record),
            };
        }
        let extensions = members.remove(EXTENSIONS);
        return match P::decode_request(name, members)? {
            Some(request) if sender == Role::Initiator => Ok(Message::Request {
                id,
                request,
                extensions: extensions.map_or(Ok(Extensions::new()), |value| {
                    serde_json::from_value(value).map_err(|_| RecordError::Record)
                })?,
            }),
            Some(_) => Err(RecordError::Record),
            None => Ok(Message::UnknownRequest {
                id,
                request: Name::new(name).map_err(|_| RecordError::Record)?,
            }),
        };
    }
    if name == BYE {
        let extensions = take_extensions(&mut members)?;
        let reason = closed::<WireBye>(members)?.reason;
        return Ok(Message::Bye {
            reason: ByeReason::from_name(reason.as_str()).unwrap_or(ByeReason::Unknown),
            extensions,
        });
    }
    let extensions = members.remove(EXTENSIONS);
    match P::decode_event(name, members)? {
        Some(event) if sender == Role::Acceptor => Ok(Message::Event {
            event,
            extensions: extensions.map_or(Ok(Extensions::new()), |value| {
                serde_json::from_value(value).map_err(|_| RecordError::Record)
            })?,
        }),
        Some(_) => Err(RecordError::Record),
        None => Ok(Message::UnknownEvent {
            event: Name::new(name).map_err(|_| RecordError::Record)?,
        }),
    }
}

fn encode_message<P: Protocol>(message: &Message<P>) -> Result<Vec<u8>, EncodeError> {
    let (name, id, members) = match message {
        Message::Request {
            id,
            request,
            extensions,
        } => (
            P::request_name(request).to_owned(),
            Some(*id),
            with_extensions(P::encode_request(request), extensions),
        ),
        Message::UnknownRequest { id, request } => {
            (request.as_str().to_owned(), Some(*id), Map::new())
        }
        Message::Reply {
            id,
            reply,
            extensions,
        } => (
            format!("{}{REPLY_SUFFIX}", P::reply_request(reply)),
            Some(*id),
            with_extensions(P::encode_reply(reply), extensions),
        ),
        Message::Unsupported {
            id,
            request,
            extensions,
        } => (
            UNSUPPORTED.to_owned(),
            Some(*id),
            with_extensions(
                members_of(&WireUnsupported {
                    request: request.clone(),
                }),
                extensions,
            ),
        ),
        Message::Event { event, extensions } => (
            P::event_name(event).to_owned(),
            None,
            with_extensions(P::encode_event(event), extensions),
        ),
        Message::UnknownEvent { event } => (event.as_str().to_owned(), None, Map::new()),
        Message::Bye { reason, extensions } => (
            BYE.to_owned(),
            None,
            with_extensions(
                members_of(&WireBye {
                    reason: Name::literal(reason.as_str()),
                }),
                extensions,
            ),
        ),
    };
    let mut members = members;
    members.insert("type".to_owned(), Value::String(name));
    if let Some(id) = id {
        members.extend(members_of(&WireId { id }));
    }
    encode_payload(members)
}
