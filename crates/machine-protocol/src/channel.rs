//! The state machine of one end of a machine connection: channel ids and states, credit in both
//! directions, the request and `ping` ledgers, the link's `facts` rule, and the fair sender. It
//! performs no I/O and keeps no clock: the caller feeds it the frames it reads, writes the frames
//! [`Connection::poll_transmit`] yields, and reports what its consumers took. Carriers, timers,
//! liveness, and relaying between connections belong to the endpoints.

mod outbox;
mod stream;

use std::{collections::BTreeMap, error::Error, fmt};

use orchbee_connection_handshake::Extensions;

use crate::{
    ByeReason, EncodeError, Frame, FrameKind, Id, MAXIMUM_CREDIT, MAXIMUM_DATA_PAYLOAD_BYTES,
    RecordError, ResetReason, Role,
    hub::Hub,
    link::{Event as LinkEvent, Facts, Link},
    protocol::Protocol,
    record::{Close, Message, Open, Record},
    value::name_enum,
};
use outbox::Outbox;
use stream::Stream;

/// The credit each direction of a channel holds at `open`.
pub const INITIAL_CREDIT: u32 = 262_144;
/// The taken, not yet regranted bytes at which a receiver sends `credit`.
pub const CREDIT_REGRANT_THRESHOLD: u32 = 131_072;
/// The most channels one end keeps open of those it opened.
pub const MAXIMUM_OPEN_CHANNELS: usize = 256;
/// The most requests one sender keeps unanswered.
pub const MAXIMUM_UNANSWERED_REQUESTS: usize = 64;

name_enum!(
    /// A violation: a frame that ends the connection, never one channel. The end that detects it
    /// writes only `bye` with reason `violation` and closes the carrier. The names are the
    /// vocabulary of `sequences.json`.
    pub enum Violation {
        /// A header that fails a frame check.
        Frame = "frame",
        /// A payload that breaks the JSON rules.
        Encoding = "encoding",
        /// A known record that is not exactly its closed object or travels in a direction the
        /// contract does not give it.
        Record = "record",
        /// `open` with an id of the receiver's parity.
        Parity = "parity",
        /// `open` with an id not greater than the sender's earlier ids.
        OpenIdNotIncreasing = "open-id-not-increasing",
        /// A frame other than `open` for an id never opened.
        NeverOpened = "never-opened",
        /// `accept` or `close` from the opener.
        OpenerAcceptOrClose = "opener-accept-or-close",
        /// A second `accept`.
        SecondAccept = "second-accept",
        /// A responder frame before `accept` other than `reset`.
        BeforeAccept = "before-accept",
        /// `data` after that side's `end`.
        DataAfterEnd = "data-after-end",
        /// A second `end`.
        SecondEnd = "second-end",
        /// `data` in a direction the channel kind gives none.
        DataDirection = "data-direction",
        /// `data` beyond the credit granted.
        CreditExceeded = "credit-exceeded",
        /// A `credit` frame that raises outstanding credit above 4,194,304.
        CreditOverflow = "credit-overflow",
        /// Responder `data` beyond its channel kind's bound.
        DataBound = "data-bound",
        /// A request id not greater than the sender's earlier ones.
        RequestIdNotIncreasing = "request-id-not-increasing",
        /// A 65th unanswered request.
        TooManyRequests = "too-many-requests",
        /// A second unanswered `ping`.
        SecondPing = "second-ping",
        /// A reply that answers no outstanding request of its type.
        UnmatchedReply = "unmatched-reply",
        /// A `pong` that answers no outstanding `ping`.
        UnmatchedPong = "unmatched-pong",
        /// On `machine-link`, a first acceptor frame that is not `facts`.
        FactsMissing = "facts-missing",
        /// On `machine-link`, a second `facts`.
        FactsRepeated = "facts-repeated",
    }
);

impl fmt::Display for Violation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "machine connection violation: {}", self.as_str())
    }
}

impl Error for Violation {}

impl From<RecordError> for Violation {
    fn from(error: RecordError) -> Self {
        match error {
            RecordError::Encoding => Self::Encoding,
            RecordError::Record => Self::Record,
        }
    }
}

name_enum!(
    /// How the state machine took a frame that broke no rule. The names are the vocabulary of
    /// `sequences.json`.
    pub enum Verdict {
        /// The frame took effect.
        Accepted = "accepted",
        /// A late frame for a closed channel, discarded without effect.
        Discarded = "discarded",
        /// The state machine queued its own answer: a `pong`, an `unsupported` reply, or a `reset`.
        Reply = "reply",
    }
);

/// What a received frame gives the endpoint.
#[derive(Clone, Debug, PartialEq)]
pub enum Delivery<P: Protocol> {
    /// A known request, reply, or event, an `unsupported` reply, or `bye`. Unknown requests are
    /// answered and unknown events ignored by the state machine.
    Message(Message<P>),
    /// The peer opened a channel; this end is its responder.
    Open {
        /// The channel id.
        channel: u32,
        /// The kind and its members.
        open: P::Channel,
        /// The payload's `extensions`.
        extensions: Extensions,
    },
    /// The responder accepted a channel this end opened.
    Accept {
        /// The channel id.
        channel: u32,
        /// The payload's `extensions`.
        extensions: Extensions,
    },
    /// Channel bytes; the consumer reports them through [`Connection::take`] as it takes them.
    Data {
        /// The channel id.
        channel: u32,
        /// The bytes.
        bytes: Vec<u8>,
    },
    /// The peer granted credit, so [`Connection::write`] may take more bytes.
    Writable {
        /// The channel id.
        channel: u32,
    },
    /// The peer sends no more `data` on the channel.
    End {
        /// The channel id.
        channel: u32,
    },
    /// The responder's result; the channel is closed.
    Close {
        /// The channel id.
        channel: u32,
        /// The outcome, read among the outcomes of the channel's kind.
        outcome: P::Outcome,
        /// The payload's `extensions`.
        extensions: Extensions,
    },
    /// The peer reset the channel; it is closed.
    Reset {
        /// The channel id.
        channel: u32,
        /// The reason, read among the reasons of the channel's kind.
        reason: ResetReason,
        /// The payload's `extensions`.
        extensions: Extensions,
    },
    /// The opener's `data` exceeded the kind's bound, so the state machine reset the channel with
    /// `too-large`; it is closed.
    TooLarge {
        /// The channel id.
        channel: u32,
    },
    /// The peer answered this end's `ping`.
    Pong,
}

/// The state machine's account of one received frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Receipt<P: Protocol> {
    /// How the frame was taken.
    pub verdict: Verdict,
    /// What it gives the endpoint, if anything.
    pub delivery: Option<Delivery<P>>,
}

impl<P: Protocol> Receipt<P> {
    const fn accepted(delivery: Delivery<P>) -> Self {
        Self {
            verdict: Verdict::Accepted,
            delivery: Some(delivery),
        }
    }

    const fn bare(verdict: Verdict) -> Self {
        Self {
            verdict,
            delivery: None,
        }
    }
}

/// Why this end refuses to send something; the connection is unaffected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SendError {
    /// The connection ended in a violation, or this end queued `bye`.
    Closed,
    /// Generation 1 does not give this end that record.
    Direction,
    /// This end already keeps 256 channels it opened open.
    Capacity,
    /// This end has used its last channel or request id.
    IdsExhausted,
    /// No such channel is open at this end.
    UnknownChannel,
    /// The responder must send `accept` first.
    NotAccepted,
    /// This end already accepted the channel.
    AlreadyAccepted,
    /// This end already sent `end` or `close` on the channel.
    Ended,
    /// The channel kind gives this end's direction no `data`.
    DataDirection,
    /// This end reached its `data` bound on the channel.
    DataBound,
    /// The channel kind does not list the `close` outcome.
    Outcome,
    /// The channel kind does not list the `reset` reason, or a `bye` reason is the fallback.
    Reason,
    /// This end already keeps 64 requests unanswered.
    TooManyRequests,
    /// No request of the peer with that id awaits a reply.
    UnknownRequest,
    /// The reply does not answer the request's type.
    ReplyMismatch,
    /// This end's `ping` is still unanswered.
    PingOutstanding,
    /// More bytes than were delivered and not yet taken.
    NotDelivered,
    /// The link's `facts` is sent once, by the constructor.
    FactsRepeated,
    /// The record does not fit one frame.
    Encode(EncodeError),
}

impl fmt::Display for SendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Closed => "machine connection is closed for sending",
            Self::Direction => "generation 1 does not give this end that record",
            Self::Capacity => "this end already keeps 256 channels it opened open",
            Self::IdsExhausted => "this end has used its last id",
            Self::UnknownChannel => "no such channel is open",
            Self::NotAccepted => "the responder has not accepted the channel",
            Self::AlreadyAccepted => "the channel is already accepted",
            Self::Ended => "this end already ended the channel",
            Self::DataDirection => "the channel kind gives this direction no data",
            Self::DataBound => "this end reached its data bound on the channel",
            Self::Outcome => "the channel kind does not list the outcome",
            Self::Reason => "the reason is not one this end may send",
            Self::TooManyRequests => "64 requests are already unanswered",
            Self::UnknownRequest => "no such request awaits a reply",
            Self::ReplyMismatch => "the reply does not answer the request's type",
            Self::PingOutstanding => "a ping is already unanswered",
            Self::NotDelivered => "more bytes than were delivered",
            Self::FactsRepeated => "facts is sent once",
            Self::Encode(error) => return error.fmt(formatter),
        })
    }
}

impl Error for SendError {}

impl From<EncodeError> for SendError {
    fn from(error: EncodeError) -> Self {
        Self::Encode(error)
    }
}

enum Status {
    Open,
    Closed,
    NeverOpened,
}

/// One end of a `machine-link` or `machine-hub` connection after the `OBHS` answer.
#[derive(Debug)]
pub struct Connection<P: Protocol> {
    role: Role,
    channels: BTreeMap<u32, Stream<P::ChannelKind>>,
    next_open: Option<u32>,
    own_highest: u32,
    peer_highest: u32,
    sent_requests: BTreeMap<u32, &'static str>,
    next_request: Option<u32>,
    received_requests: BTreeMap<u32, &'static str>,
    peer_request_highest: u32,
    ping: Option<[u8; 8]>,
    pong_owed: bool,
    facts_received: bool,
    farewell: bool,
    violation: Option<Violation>,
    outbox: Outbox,
}

impl Connection<Hub> {
    /// The hub's end of a client connection.
    #[must_use]
    pub fn acceptor() -> Self {
        Self::new(Role::Acceptor)
    }
}

impl Connection<Link> {
    /// The remote helper's end, with `facts` queued as its first frame.
    #[must_use]
    pub fn acceptor(facts: Facts) -> Self {
        let mut connection = Self::new(Role::Acceptor);
        let frame = Record::<Link>::Message(Message::Event {
            event: LinkEvent::Facts(facts),
            extensions: Extensions::new(),
        })
        .encode(0)
        .expect("facts fit one frame: two paths of at most 4,096 bytes and two tokens");
        connection.outbox.push(frame);
        connection
    }
}

impl<P: Protocol> Connection<P> {
    /// The initiator's end: a client of the hub, or the hub on a link.
    #[must_use]
    pub fn initiator() -> Self {
        Self::new(Role::Initiator)
    }

    fn new(role: Role) -> Self {
        Self {
            role,
            channels: BTreeMap::new(),
            next_open: Some(match role {
                Role::Initiator => 1,
                Role::Acceptor => 2,
            }),
            own_highest: 0,
            peer_highest: 0,
            sent_requests: BTreeMap::new(),
            next_request: Some(1),
            received_requests: BTreeMap::new(),
            peer_request_highest: 0,
            ping: None,
            pong_owed: false,
            facts_received: false,
            farewell: false,
            violation: None,
            outbox: Outbox::default(),
        }
    }

    /// This end's role.
    #[must_use]
    pub const fn role(&self) -> Role {
        self.role
    }

    /// The violation that ended the connection, if any.
    #[must_use]
    pub const fn violation(&self) -> Option<Violation> {
        self.violation
    }

    /// The channels open at this end, of either opener.
    #[must_use]
    pub fn open_channels(&self) -> usize {
        self.channels
            .values()
            .filter(|stream| !stream.closing)
            .count()
    }

    /// Whether this end's `ping` is unanswered.
    #[must_use]
    pub const fn ping_outstanding(&self) -> bool {
        self.ping.is_some()
    }

    /// Whether frames wait to be written.
    #[must_use]
    pub fn has_pending(&self) -> bool {
        !self.outbox.is_empty()
    }

    /// The frames other than `data` and `bye` that wait to be written, so a carrier can stop
    /// reading while a peer that does not read keeps provoking replies.
    #[must_use]
    pub fn queued_frames(&self) -> usize {
        self.outbox.control_len()
    }

    /// The committed `data` bytes of `channel` that [`Connection::poll_transmit`] has not yet
    /// handed out; 0 for a channel that is not open.
    #[must_use]
    pub fn queued_bytes(&self, channel: u32) -> usize {
        self.channels.get(&channel).map_or(0, |stream| {
            stream
                .pending
                .iter()
                .map(|frame| frame.payload().len())
                .sum()
        })
    }

    /// Whether `bye` was written, so the caller closes the carrier.
    #[must_use]
    pub const fn is_finished(&self) -> bool {
        self.outbox.is_finished()
    }

    /// Takes one frame the peer sent.
    ///
    /// # Errors
    ///
    /// Returns the [`Violation`] the frame commits, or the earlier one that ended the connection.
    /// After a violation the connection only writes `bye` with reason `violation`.
    pub fn receive(&mut self, frame: &Frame) -> Result<Receipt<P>, Violation> {
        if let Some(violation) = self.violation {
            return Err(violation);
        }
        let receipt = self.admit(frame);
        if let Err(violation) = receipt {
            self.fail(violation);
        }
        receipt
    }

    /// Ends the connection for a frame the caller's decoder refused.
    pub fn frame_failed(&mut self) -> Violation {
        self.fail(Violation::Frame);
        self.violation.unwrap_or(Violation::Frame)
    }

    fn fail(&mut self, violation: Violation) {
        if self.violation.is_some() {
            return;
        }
        self.violation = Some(violation);
        self.channels.clear();
        self.ping = None;
        self.pong_owed = false;
        self.outbox.fail(bye_frame::<P>(ByeReason::Violation));
    }

    fn admit(&mut self, frame: &Frame) -> Result<Receipt<P>, Violation> {
        let sender = self.role.peer();
        if P::FACTS_FIRST && self.role == Role::Initiator && !self.facts_received {
            return self.admit_facts(frame);
        }
        let channel = frame.channel();
        match frame.kind() {
            FrameKind::Open => self.admit_open_id(channel)?,
            kind if !kind.on_channel_zero() => match self.status(channel) {
                Status::Open => {}
                Status::Closed => return Ok(Receipt::bare(Verdict::Discarded)),
                Status::NeverOpened => return Err(Violation::NeverOpened),
            },
            _ => {}
        }
        match Record::<P>::decode(sender, frame)? {
            Record::Message(message) => self.admit_message(message),
            Record::Ping(bytes) => self.admit_ping(bytes),
            Record::Pong(bytes) => {
                if self.ping != Some(bytes) {
                    return Err(Violation::UnmatchedPong);
                }
                self.ping = None;
                Ok(Receipt::accepted(Delivery::Pong))
            }
            Record::Open(open) => self.admit_open(channel, open),
            Record::Accept { extensions } => {
                let stream = self.stream_of(channel)?;
                if !stream.mine {
                    return Err(Violation::OpenerAcceptOrClose);
                }
                if stream.accepted {
                    return Err(Violation::SecondAccept);
                }
                stream.accepted = true;
                Ok(Receipt::accepted(Delivery::Accept {
                    channel,
                    extensions,
                }))
            }
            Record::Data(bytes) => self.admit_data(channel, bytes),
            Record::Credit(value) => {
                let stream = self.stream_of(channel)?;
                if stream.awaits_accept() {
                    return Err(Violation::BeforeAccept);
                }
                let granted = stream.peer_granted + u64::from(value);
                if granted - stream.committed > u64::from(MAXIMUM_CREDIT) {
                    return Err(Violation::CreditOverflow);
                }
                stream.peer_granted = granted;
                Ok(Receipt::accepted(Delivery::Writable { channel }))
            }
            Record::End => {
                let stream = self.stream_of(channel)?;
                if stream.awaits_accept() {
                    return Err(Violation::BeforeAccept);
                }
                if stream.peer_ended {
                    return Err(Violation::SecondEnd);
                }
                stream.peer_ended = true;
                Ok(Receipt::accepted(Delivery::End { channel }))
            }
            Record::Close(Close {
                outcome,
                extensions,
            }) => {
                let stream = self.stream_of(channel)?;
                if !stream.mine {
                    return Err(Violation::OpenerAcceptOrClose);
                }
                if !stream.accepted {
                    return Err(Violation::BeforeAccept);
                }
                let kind = stream.kind;
                self.channels.remove(&channel);
                Ok(Receipt::accepted(Delivery::Close {
                    channel,
                    outcome: P::outcome_for(outcome, kind),
                    extensions,
                }))
            }
            Record::Reset { reason, extensions } => {
                let kind = self.stream_of(channel)?.kind;
                self.channels.remove(&channel);
                Ok(Receipt::accepted(Delivery::Reset {
                    channel,
                    reason: reason.within(P::reset_reasons(kind)),
                    extensions,
                }))
            }
        }
    }

    fn admit_facts(&mut self, frame: &Frame) -> Result<Receipt<P>, Violation> {
        if frame.kind() != FrameKind::Message {
            return Err(Violation::FactsMissing);
        }
        match Record::<P>::decode(self.role.peer(), frame)? {
            Record::Message(Message::Event { event, extensions }) if P::is_facts(&event) => {
                self.facts_received = true;
                Ok(Receipt::accepted(Delivery::Message(Message::Event {
                    event,
                    extensions,
                })))
            }
            _ => Err(Violation::FactsMissing),
        }
    }

    fn status(&self, channel: u32) -> Status {
        if let Some(stream) = self.channels.get(&channel) {
            return if stream.closing {
                Status::Closed
            } else {
                Status::Open
            };
        }
        let highest = if Role::opener_of(channel) == self.role {
            self.own_highest
        } else {
            self.peer_highest
        };
        if channel <= highest {
            Status::Closed
        } else {
            Status::NeverOpened
        }
    }

    fn stream_of(&mut self, channel: u32) -> Result<&mut Stream<P::ChannelKind>, Violation> {
        self.channels
            .get_mut(&channel)
            .ok_or(Violation::NeverOpened)
    }

    fn admit_open_id(&mut self, channel: u32) -> Result<(), Violation> {
        if Role::opener_of(channel) == self.role {
            return Err(Violation::Parity);
        }
        if channel <= self.peer_highest {
            return Err(Violation::OpenIdNotIncreasing);
        }
        self.peer_highest = channel;
        Ok(())
    }

    fn admit_open(&mut self, channel: u32, open: Open<P>) -> Result<Receipt<P>, Violation> {
        let Open::Known {
            channel: kind_channel,
            extensions,
        } = open
        else {
            self.queue_reset(channel, ResetReason::Unsupported);
            return Ok(Receipt::bare(Verdict::Reply));
        };
        if self.opened_by(self.role.peer()) >= MAXIMUM_OPEN_CHANNELS {
            self.queue_reset(channel, ResetReason::Capacity);
            return Ok(Receipt::bare(Verdict::Reply));
        }
        let kind = P::channel_kind(&kind_channel);
        self.channels
            .insert(channel, Stream::new(kind, false, P::rules(kind)));
        Ok(Receipt::accepted(Delivery::Open {
            channel,
            open: kind_channel,
            extensions,
        }))
    }

    fn admit_data(&mut self, channel: u32, bytes: Vec<u8>) -> Result<Receipt<P>, Violation> {
        let stream = self.stream_of(channel)?;
        if stream.awaits_accept() {
            return Err(Violation::BeforeAccept);
        }
        if stream.peer_ended {
            return Err(Violation::DataAfterEnd);
        }
        let Some(bound) = stream.peer_bound() else {
            return Err(Violation::DataDirection);
        };
        let received = stream.received + bytes.len() as u64;
        if received > stream.granted {
            return Err(Violation::CreditExceeded);
        }
        if received > bound {
            if stream.mine {
                return Err(Violation::DataBound);
            }
            self.channels.remove(&channel);
            self.queue_reset(channel, ResetReason::TooLarge);
            return Ok(Receipt {
                verdict: Verdict::Reply,
                delivery: Some(Delivery::TooLarge { channel }),
            });
        }
        stream.received = received;
        stream.untaken += bytes.len() as u64;
        Ok(Receipt::accepted(Delivery::Data { channel, bytes }))
    }

    fn admit_ping(&mut self, bytes: [u8; 8]) -> Result<Receipt<P>, Violation> {
        if self.pong_owed {
            return Err(Violation::SecondPing);
        }
        self.pong_owed = true;
        self.outbox.push_pong(encode(&Record::<P>::Pong(bytes), 0));
        Ok(Receipt::bare(Verdict::Reply))
    }

    fn admit_message(&mut self, message: Message<P>) -> Result<Receipt<P>, Violation> {
        match message {
            Message::Request {
                id,
                request,
                extensions,
            } => {
                self.admit_request_id(id)?;
                self.received_requests
                    .insert(id.get(), P::request_name(&request));
                Ok(Receipt::accepted(Delivery::Message(Message::Request {
                    id,
                    request,
                    extensions,
                })))
            }
            Message::UnknownRequest { id, request } => {
                self.admit_request_id(id)?;
                let reply = Record::<P>::Message(Message::Unsupported {
                    id,
                    request,
                    extensions: Extensions::new(),
                });
                self.outbox.push(encode(&reply, 0));
                Ok(Receipt::bare(Verdict::Reply))
            }
            Message::Reply {
                id,
                reply,
                extensions,
            } => {
                self.answer_sent_request(id, P::reply_request(&reply))?;
                Ok(Receipt::accepted(Delivery::Message(Message::Reply {
                    id,
                    reply,
                    extensions,
                })))
            }
            Message::Unsupported {
                id,
                request,
                extensions,
            } => {
                self.answer_sent_request(id, request.as_str())?;
                Ok(Receipt::accepted(Delivery::Message(Message::Unsupported {
                    id,
                    request,
                    extensions,
                })))
            }
            Message::Event { event, extensions } => {
                if P::is_facts(&event) {
                    return Err(Violation::FactsRepeated);
                }
                Ok(Receipt::accepted(Delivery::Message(Message::Event {
                    event,
                    extensions,
                })))
            }
            Message::UnknownEvent { .. } => Ok(Receipt::bare(Verdict::Accepted)),
            Message::Bye { reason, extensions } => {
                Ok(Receipt::accepted(Delivery::Message(Message::Bye {
                    reason,
                    extensions,
                })))
            }
        }
    }

    fn admit_request_id(&mut self, id: Id) -> Result<(), Violation> {
        if id.get() <= self.peer_request_highest {
            return Err(Violation::RequestIdNotIncreasing);
        }
        if self.received_requests.len() >= MAXIMUM_UNANSWERED_REQUESTS {
            return Err(Violation::TooManyRequests);
        }
        self.peer_request_highest = id.get();
        Ok(())
    }

    fn answer_sent_request(&mut self, id: Id, request: &str) -> Result<(), Violation> {
        match self.sent_requests.get(&id.get()) {
            Some(&name) if name == request => {
                self.sent_requests.remove(&id.get());
                Ok(())
            }
            _ => Err(Violation::UnmatchedReply),
        }
    }

    fn opened_by(&self, opener: Role) -> usize {
        let mine = opener == self.role;
        self.channels
            .values()
            .filter(|stream| stream.mine == mine && !stream.closing)
            .count()
    }

    fn queue_reset(&mut self, channel: u32, reason: ResetReason) {
        let reset = Record::<P>::Reset {
            reason,
            extensions: Extensions::new(),
        };
        self.outbox.push(encode(&reset, channel));
    }

    fn sendable(&self) -> Result<(), SendError> {
        if self.violation.is_some() || self.farewell {
            Err(SendError::Closed)
        } else {
            Ok(())
        }
    }

    fn stream_to_send(&mut self, channel: u32) -> Result<&mut Stream<P::ChannelKind>, SendError> {
        self.sendable()?;
        self.channels
            .get_mut(&channel)
            .filter(|stream| !stream.closing)
            .ok_or(SendError::UnknownChannel)
    }

    /// Opens a channel and returns its id. The opener may write `data` and `end` before `accept`
    /// arrives, inside the initial credit.
    ///
    /// # Errors
    ///
    /// Returns [`SendError`] when the connection is closed for sending, this end is not the
    /// initiator, it keeps 256 channels it opened open, or it has used its last id.
    pub fn open(&mut self, channel: P::Channel) -> Result<u32, SendError> {
        self.sendable()?;
        if self.role != Role::Initiator {
            return Err(SendError::Direction);
        }
        if self.opened_by(self.role) >= MAXIMUM_OPEN_CHANNELS {
            return Err(SendError::Capacity);
        }
        let id = self.next_open.ok_or(SendError::IdsExhausted)?;
        let kind = P::channel_kind(&channel);
        let frame = Record::<P>::Open(Open::Known {
            channel,
            extensions: Extensions::new(),
        })
        .encode(id)?;
        self.next_open = id.checked_add(2);
        self.own_highest = id;
        self.channels
            .insert(id, Stream::new(kind, true, P::rules(kind)));
        self.outbox.push(frame);
        Ok(id)
    }

    /// Accepts a channel the peer opened, and sends the credit its consumer already earned.
    ///
    /// # Errors
    ///
    /// Returns [`SendError`] when the channel is not open, this end opened it, or it was already
    /// accepted.
    pub fn accept(&mut self, channel: u32) -> Result<(), SendError> {
        let stream = self.stream_to_send(channel)?;
        if stream.mine {
            return Err(SendError::Direction);
        }
        if stream.accepted {
            return Err(SendError::AlreadyAccepted);
        }
        stream.accepted = true;
        let frame = encode(
            &Record::<P>::Accept {
                extensions: Extensions::new(),
            },
            channel,
        );
        self.outbox.push(frame);
        self.regrant(channel);
        Ok(())
    }

    /// Commits as many leading bytes of `bytes` as the channel's credit and `data` bound admit and
    /// returns their count; the caller offers the rest again after [`Delivery::Writable`].
    ///
    /// # Errors
    ///
    /// Returns [`SendError`] when the channel is not open, this end is a responder that has not
    /// accepted, already ended the channel, has no `data` direction on it, or reached its bound.
    pub fn write(&mut self, channel: u32, bytes: &[u8]) -> Result<usize, SendError> {
        let stream = self.stream_to_send(channel)?;
        if !stream.may_grant() {
            return Err(SendError::NotAccepted);
        }
        if stream.ended {
            return Err(SendError::Ended);
        }
        let bound = stream.own_bound().ok_or(SendError::DataDirection)?;
        let bound_room = bound - stream.committed;
        if bound_room == 0 && !bytes.is_empty() {
            return Err(SendError::DataBound);
        }
        let room = (stream.peer_granted - stream.committed).min(bound_room);
        let taken = bytes.len().min(usize::try_from(room).unwrap_or(usize::MAX));
        let (accepted, _) = bytes.split_at(taken);
        for piece in accepted.chunks(MAXIMUM_DATA_PAYLOAD_BYTES) {
            let frame = Frame::new(FrameKind::Data, channel, piece.to_vec())
                .map_err(|error| SendError::Encode(EncodeError::Frame(error)))?;
            stream.pending.push_back(frame);
        }
        stream.committed += taken as u64;
        if taken > 0 && !stream.ready {
            stream.ready = true;
            self.outbox.mark_ready(channel);
        }
        Ok(taken)
    }

    /// Sends `end`, after the channel's pending `data`.
    ///
    /// # Errors
    ///
    /// Returns [`SendError`] when the channel is not open, this end is a responder that has not
    /// accepted, or it already ended the channel.
    pub fn end(&mut self, channel: u32) -> Result<(), SendError> {
        let stream = self.stream_to_send(channel)?;
        if !stream.may_grant() {
            return Err(SendError::NotAccepted);
        }
        if stream.ended {
            return Err(SendError::Ended);
        }
        stream.ended = true;
        let frame = encode(&Record::<P>::End, channel);
        self.queue_after_data(channel, frame);
        Ok(())
    }

    /// Sends the responder's `close` with `outcome`, after the channel's pending `data`; the
    /// channel is closed for receiving at once.
    ///
    /// # Errors
    ///
    /// Returns [`SendError`] when the channel is not open, this end opened it or has not accepted
    /// it, or the channel's kind does not list `outcome`.
    pub fn close(&mut self, channel: u32, outcome: P::Outcome) -> Result<(), SendError> {
        let stream = self.stream_to_send(channel)?;
        if stream.mine {
            return Err(SendError::Direction);
        }
        if !stream.accepted {
            return Err(SendError::NotAccepted);
        }
        if !P::kind_lists(&outcome, stream.kind) {
            return Err(SendError::Outcome);
        }
        let frame = Record::<P>::Close(Close {
            outcome,
            extensions: Extensions::new(),
        })
        .encode(channel)?;
        let Some(stream) = self.channels.get_mut(&channel) else {
            return Err(SendError::UnknownChannel);
        };
        stream.ended = true;
        stream.closing = true;
        self.queue_after_data(channel, frame);
        Ok(())
    }

    /// Resets the channel at once with `reason`, dropping its pending `data`.
    ///
    /// # Errors
    ///
    /// Returns [`SendError`] when the channel is not open or its kind does not list `reason`.
    pub fn reset(&mut self, channel: u32, reason: ResetReason) -> Result<(), SendError> {
        let stream = self.stream_to_send(channel)?;
        if reason == ResetReason::Unknown || !P::reset_reasons(stream.kind).contains(&reason) {
            return Err(SendError::Reason);
        }
        self.channels.remove(&channel);
        self.queue_reset(channel, reason);
        Ok(())
    }

    /// Reports that the consumer took `bytes` delivered bytes of the channel, and sends `credit`
    /// once the taken, not yet regranted bytes reach 131,072. Taking from a channel that is no
    /// longer open does nothing.
    ///
    /// # Errors
    ///
    /// Returns [`SendError::NotDelivered`] for more bytes than were delivered and not yet taken.
    pub fn take(&mut self, channel: u32, bytes: usize) -> Result<(), SendError> {
        let Some(stream) = self
            .channels
            .get_mut(&channel)
            .filter(|stream| !stream.closing)
        else {
            return Ok(());
        };
        let bytes = bytes as u64;
        if bytes > stream.untaken {
            return Err(SendError::NotDelivered);
        }
        stream.untaken -= bytes;
        stream.unregranted += bytes;
        self.regrant(channel);
        Ok(())
    }

    fn regrant(&mut self, channel: u32) {
        if self.violation.is_some() {
            return;
        }
        let Some(stream) = self.channels.get_mut(&channel) else {
            return;
        };
        if stream.unregranted < u64::from(CREDIT_REGRANT_THRESHOLD)
            || stream.peer_ended
            || stream.closing
            || !stream.may_grant()
        {
            return;
        }
        let value = stream.unregranted;
        stream.unregranted = 0;
        stream.granted += value;
        let value = u32::try_from(value).expect("taken bytes never exceed one window");
        self.outbox
            .push(encode(&Record::<P>::Credit(value), channel));
    }

    fn queue_after_data(&mut self, channel: u32, frame: Frame) {
        let Some(stream) = self.channels.get_mut(&channel) else {
            return;
        };
        if stream.pending.is_empty() {
            self.outbox.push(frame);
            if stream.closing {
                self.channels.remove(&channel);
            }
        } else {
            stream.after_data.push_back(frame);
        }
    }

    /// Sends a request and returns its id.
    ///
    /// # Errors
    ///
    /// Returns [`SendError`] when the connection is closed for sending, this end is not the
    /// initiator, 64 requests are unanswered, ids are exhausted, or the request does not fit.
    pub fn request(&mut self, request: P::Request) -> Result<Id, SendError> {
        self.sendable()?;
        if self.role != Role::Initiator {
            return Err(SendError::Direction);
        }
        if self.sent_requests.len() >= MAXIMUM_UNANSWERED_REQUESTS {
            return Err(SendError::TooManyRequests);
        }
        let id = self
            .next_request
            .and_then(Id::new)
            .ok_or(SendError::IdsExhausted)?;
        let name = P::request_name(&request);
        let frame = Record::<P>::Message(Message::Request {
            id,
            request,
            extensions: Extensions::new(),
        })
        .encode(0)?;
        self.next_request = id.get().checked_add(1);
        self.sent_requests.insert(id.get(), name);
        self.outbox.push(frame);
        Ok(id)
    }

    /// Sends the reply to the peer's request `id`.
    ///
    /// # Errors
    ///
    /// Returns [`SendError`] when the connection is closed for sending, no request `id` awaits a
    /// reply, the reply answers another type, or it does not fit.
    pub fn reply(&mut self, id: Id, reply: P::Reply) -> Result<(), SendError> {
        self.sendable()?;
        match self.received_requests.get(&id.get()) {
            Some(&name) if name == P::reply_request(&reply) => {}
            Some(_) => return Err(SendError::ReplyMismatch),
            None => return Err(SendError::UnknownRequest),
        }
        let frame = Record::<P>::Message(Message::Reply {
            id,
            reply,
            extensions: Extensions::new(),
        })
        .encode(0)?;
        self.received_requests.remove(&id.get());
        self.outbox.push(frame);
        Ok(())
    }

    /// Sends an event; a state event replaces the undelivered one of its subject in place.
    ///
    /// # Errors
    ///
    /// Returns [`SendError`] when the connection is closed for sending, this end is not the
    /// acceptor, the event is the link's `facts`, or it does not fit.
    pub fn event(&mut self, event: P::Event) -> Result<(), SendError> {
        self.sendable()?;
        if self.role != Role::Acceptor {
            return Err(SendError::Direction);
        }
        if P::is_facts(&event) {
            return Err(SendError::FactsRepeated);
        }
        let subject = P::event_subject(&event);
        let frame = Record::<P>::Message(Message::Event {
            event,
            extensions: Extensions::new(),
        })
        .encode(0)?;
        self.outbox.push_event(frame, subject);
        Ok(())
    }

    /// Sends `ping` with `bytes`.
    ///
    /// # Errors
    ///
    /// Returns [`SendError`] when the connection is closed for sending or this end's `ping` is
    /// still unanswered.
    pub fn ping(&mut self, bytes: [u8; 8]) -> Result<(), SendError> {
        self.sendable()?;
        if self.ping.is_some() {
            return Err(SendError::PingOutstanding);
        }
        self.ping = Some(bytes);
        self.outbox.push(encode(&Record::<P>::Ping(bytes), 0));
        Ok(())
    }

    /// Queues `bye` with `reason` as the last frame, after every frame already queued; nothing
    /// more can be sent.
    ///
    /// # Errors
    ///
    /// Returns [`SendError`] when the connection is closed for sending or `reason` is the
    /// fallback `unknown`.
    pub fn bye(&mut self, reason: ByeReason) -> Result<(), SendError> {
        self.sendable()?;
        if reason == ByeReason::Unknown {
            return Err(SendError::Reason);
        }
        self.farewell = true;
        self.outbox.set_bye(bye_frame::<P>(reason));
        Ok(())
    }

    /// The next frame to write: every frame other than `data` first, then one `data` frame of
    /// each ready channel per round, and `bye` last.
    pub fn poll_transmit(&mut self) -> Option<Frame> {
        if let Some(queued) = self.outbox.pop_control() {
            if queued.pong {
                self.pong_owed = false;
            }
            return Some(queued.frame);
        }
        while let Some(channel) = self.outbox.pop_ready() {
            let Some(stream) = self.channels.get_mut(&channel) else {
                continue;
            };
            stream.ready = false;
            let Some(frame) = stream.pending.pop_front() else {
                continue;
            };
            if stream.pending.is_empty() {
                let closing = stream.closing;
                for tail in stream.after_data.drain(..) {
                    self.outbox.push(tail);
                }
                if closing {
                    self.channels.remove(&channel);
                }
            } else {
                stream.ready = true;
                self.outbox.mark_ready(channel);
            }
            return Some(frame);
        }
        self.outbox.take_bye()
    }
}

fn encode<P: Protocol>(record: &Record<P>, channel: u32) -> Frame {
    record
        .encode(channel)
        .expect("records the state machine builds fit one frame")
}

fn bye_frame<P: Protocol>(reason: ByeReason) -> Frame {
    encode(
        &Record::<P>::Message(Message::Bye {
            reason,
            extensions: Extensions::new(),
        }),
        0,
    )
}
