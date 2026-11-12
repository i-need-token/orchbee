//! The seam between the rules both machine connections share — frames, channels, credit, message
//! classes, and violations — and each connection's own records: channel kinds, open members,
//! close outcomes, requests, replies, and events. Generation 1 gives every known channel kind and
//! request to the initiator and every known event and reply to the acceptor, so the shared layer
//! checks directions and the seam only decodes and encodes members.

use std::fmt::Debug;

use serde_json::{Map, Value};

use crate::value::ResetReason;

/// One of the two machine connections, [`Link`](crate::link::Link) or [`Hub`](crate::hub::Hub).
/// Sealed: only this crate implements it.
pub trait Protocol: sealed::Codec {}

/// The members of a payload object after its `type`, `id`, `kind`, and `extensions` were taken.
pub(crate) type Members = Map<String, Value>;

/// A record that breaks its connection's rules. Nominally public because the sealed [`Codec`]
/// names it; this module is private, so nothing outside the crate can reach it.
///
/// [`Codec`]: sealed::Codec
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Refused;

/// What `data` a channel kind admits in each direction, in bytes per channel and direction.
/// Nominally public for the same reason as [`Refused`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KindRules {
    /// The opener's bound, beyond which the responder resets with `too-large`; `None` when the
    /// opener sends no `data`.
    pub(crate) opener_data: Option<u64>,
    /// The responder's bound, beyond which its `data` is a violation; `None` when the responder
    /// sends no `data`.
    pub(crate) responder_data: Option<u64>,
}

impl KindRules {
    /// Unbounded `data` both ways.
    pub(crate) const STREAM: Self = Self {
        opener_data: Some(u64::MAX),
        responder_data: Some(u64::MAX),
    };
    /// One request packet and one reply packet.
    pub(crate) const ROUTE: Self = Self {
        opener_data: Some(MAXIMUM_ROUTE_REQUEST_BYTES),
        responder_data: Some(MAXIMUM_ROUTE_REPLY_BYTES),
    };
    /// No `data` either way.
    pub(crate) const NONE: Self = Self {
        opener_data: None,
        responder_data: None,
    };
}

/// The largest `machine-route` request packet a `route` channel carries.
pub const MAXIMUM_ROUTE_REQUEST_BYTES: u64 = 2_097_152;
/// The largest `machine-route` reply packet a `route` channel carries.
pub const MAXIMUM_ROUTE_REPLY_BYTES: u64 = 25_165_824;

pub(crate) mod sealed {
    use super::{Debug, KindRules, Members, Refused, ResetReason};

    /// The per-connection records behind [`Protocol`](super::Protocol).
    pub trait Codec: Copy + Debug + Eq + Sized + 'static {
        /// Whether the acceptor's first frame is the event `facts`, sent once.
        const FACTS_FIRST: bool;
        /// The reset reasons of every channel kind of the connection.
        const RESET_REASONS: &'static [ResetReason];

        /// A known channel kind with its `open` members.
        type Channel: Clone + Debug + PartialEq;
        /// A known channel kind.
        type ChannelKind: Copy + Debug + Eq;
        /// A `close` outcome with its result members.
        type Outcome: Clone + Debug + PartialEq;
        /// A known request, initiator to acceptor.
        type Request: Clone + Debug + PartialEq;
        /// A known reply, acceptor to initiator.
        type Reply: Clone + Debug + PartialEq;
        /// A known event other than `bye`, acceptor to initiator.
        type Event: Clone + Debug + PartialEq;

        /// The channel `kind` names with its members; `Ok(None)` for a kind this build does not
        /// know.
        fn decode_open(kind: &str, members: Members) -> Result<Option<Self::Channel>, Refused>;
        /// The kind name and members of `channel`.
        fn encode_open(channel: &Self::Channel) -> (&'static str, Members);
        /// The kind of `channel`.
        fn channel_kind(channel: &Self::Channel) -> Self::ChannelKind;
        /// The `data` rules of `kind`.
        fn rules(kind: Self::ChannelKind) -> KindRules;
        /// The reset reasons of `kind`: the shared ones and the kind's own.
        fn reset_reasons(kind: Self::ChannelKind) -> &'static [ResetReason];

        /// The outcome of a `close` payload, unknown names read as the fallback.
        fn decode_outcome(members: Members) -> Result<Self::Outcome, Refused>;
        /// The members of `outcome`.
        fn encode_outcome(outcome: &Self::Outcome) -> Members;
        /// `outcome` as `kind` reads it: an outcome the kind does not list reads as the fallback.
        fn outcome_for(outcome: Self::Outcome, kind: Self::ChannelKind) -> Self::Outcome;
        /// Whether `outcome` is one `kind` lists, so the responder may send it.
        fn kind_lists(outcome: &Self::Outcome, kind: Self::ChannelKind) -> bool;

        /// The request `name` names with its members; `Ok(None)` for a type this build does not
        /// know.
        fn decode_request(name: &str, members: Members) -> Result<Option<Self::Request>, Refused>;
        /// The type name of `request`.
        fn request_name(request: &Self::Request) -> &'static str;
        /// The members of `request`.
        fn encode_request(request: &Self::Request) -> Members;
        /// The reply to request type `request`; `Ok(None)` when no known request has that type.
        fn decode_reply(request: &str, members: Members) -> Result<Option<Self::Reply>, Refused>;
        /// The type name of the request `reply` answers.
        fn reply_request(reply: &Self::Reply) -> &'static str;
        /// The members of `reply`.
        fn encode_reply(reply: &Self::Reply) -> Members;

        /// The event `name` names with its members; `Ok(None)` for a type this build does not
        /// know.
        fn decode_event(name: &str, members: Members) -> Result<Option<Self::Event>, Refused>;
        /// The type name of `event`.
        fn event_name(event: &Self::Event) -> &'static str;
        /// The members of `event`.
        fn encode_event(event: &Self::Event) -> Members;
        /// The subject of a state event, whose newest undelivered copy replaces older ones.
        fn event_subject(event: &Self::Event) -> Option<String>;
        /// Whether `event` is `facts`.
        fn is_facts(event: &Self::Event) -> bool;
    }
}
