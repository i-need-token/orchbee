//! The records of `machine-link` generation 1, the connection from the hub to the remote helper:
//! the helper's `facts` event and the channel kinds `session`, `route`, and `ensure-session` with
//! their outcomes and reasons. The capability-gated `install` channel and the `facts` members
//! `origin` and `install` are not part of this build: an `install` open is an unknown kind and a
//! gated member is an unknown member.

use std::convert::Infallible;

use serde::{Deserialize, Serialize};
use serde_json::Map;

use crate::{
    Name, PathText, ResetReason, SessionName, Token,
    protocol::{KindRules, Members, Protocol, Refused, sealed::Codec},
    record::{closed, members_of},
    value::{SHARED_RESET_REASONS, name_enum},
};

/// The `machine-link` connection: initiated by the hub, accepted by `orchbee --machine-link-v1`
/// on the remote machine.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Link {}

impl Protocol for Link {}

name_enum!(
    /// A link channel kind; the hub opens every one.
    pub enum ChannelKind {
        /// Bytes relayed unread to and from a Session's Client endpoint.
        Session = "session",
        /// One `machine-route` request packet and its reply packet.
        Route = "route",
        /// Starts a Session's Server when it is not running and waits for it to report ready.
        EnsureSession = "ensure-session",
    }
);

/// A link channel kind with its `open` members.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Channel {
    /// Relay to the Client endpoint of `session` of the helper's user and environment.
    Session {
        /// The Session.
        session: SessionName,
    },
    /// Run one routed command.
    Route,
    /// Make sure `session`'s Server runs.
    EnsureSession {
        /// The Session.
        session: SessionName,
    },
}

impl Channel {
    /// The channel's kind.
    #[must_use]
    pub const fn kind(&self) -> ChannelKind {
        match self {
            Self::Session { .. } => ChannelKind::Session,
            Self::Route => ChannelKind::Route,
            Self::EnsureSession { .. } => ChannelKind::EnsureSession,
        }
    }
}

/// A link channel's `close` outcome. Each kind lists its own and reads any other as `Unknown`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Outcome {
    /// `session`: the endpoint closed and every byte it sent was relayed.
    Ended,
    /// `session`: an endpoint I/O error; `ensure-session`: the Server did not start.
    Failed,
    /// `route`: the reply packet was sent.
    Replied,
    /// `ensure-session`: the Server reported ready.
    Ready {
        /// Whether this channel started it.
        started: bool,
    },
    /// `ensure-session`: a start was attempted and readiness was not confirmed; for every kind,
    /// an outcome this build does not know.
    Unknown,
}

const OUTCOME_NAMES: [&str; 5] = ["ended", "failed", "replied", "ready", "unknown"];

/// The helper's first frame after the answer: its own build and executable. The hub may open
/// channels before it arrives.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Facts {
    /// The helper's platform token, such as `windows`, `macos`, or `linux`. It grows without a
    /// named fallback: the hub treats a platform it does not know as unsupported.
    pub platform: Token,
    /// The helper's architecture token, such as `x86_64` or `aarch64`, under the same rule.
    pub architecture: Token,
    /// The running helper's path.
    pub executable: PathText,
    /// The store root this helper resolves, when it resolves one.
    pub store_root: Option<PathText>,
}

/// A link event other than `bye`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Event {
    /// The helper's facts.
    Facts(Facts),
}

const FACTS: &str = "facts";

const SESSION_REASONS: [ResetReason; 7] = [
    SHARED_RESET_REASONS[0],
    SHARED_RESET_REASONS[1],
    SHARED_RESET_REASONS[2],
    SHARED_RESET_REASONS[3],
    SHARED_RESET_REASONS[4],
    ResetReason::SessionNotRunning,
    ResetReason::InvalidSession,
];
const ROUTE_REASONS: [ResetReason; 6] = [
    SHARED_RESET_REASONS[0],
    SHARED_RESET_REASONS[1],
    SHARED_RESET_REASONS[2],
    SHARED_RESET_REASONS[3],
    SHARED_RESET_REASONS[4],
    ResetReason::TooLarge,
];
const ENSURE_SESSION_REASONS: [ResetReason; 6] = [
    SHARED_RESET_REASONS[0],
    SHARED_RESET_REASONS[1],
    SHARED_RESET_REASONS[2],
    SHARED_RESET_REASONS[3],
    SHARED_RESET_REASONS[4],
    ResetReason::InvalidSession,
];
const ALL_REASONS: [ResetReason; 8] = [
    SHARED_RESET_REASONS[0],
    SHARED_RESET_REASONS[1],
    SHARED_RESET_REASONS[2],
    SHARED_RESET_REASONS[3],
    SHARED_RESET_REASONS[4],
    ResetReason::SessionNotRunning,
    ResetReason::InvalidSession,
    ResetReason::TooLarge,
];

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireSession {
    session: SessionName,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireEmpty {}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireOutcome {
    outcome: Name,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    started: Option<bool>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WireFacts {
    platform: Token,
    architecture: Token,
    executable: PathText,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    store_root: Option<PathText>,
}

impl Codec for Link {
    const FACTS_FIRST: bool = true;
    const RESET_REASONS: &'static [ResetReason] = &ALL_REASONS;

    type Channel = Channel;
    type ChannelKind = ChannelKind;
    type Outcome = Outcome;
    type Request = Infallible;
    type Reply = Infallible;
    type Event = Event;

    fn decode_open(kind: &str, members: Members) -> Result<Option<Channel>, Refused> {
        Ok(Some(match ChannelKind::from_name(kind) {
            None => return Ok(None),
            Some(ChannelKind::Session) => Channel::Session {
                session: closed::<WireSession>(members)?.session,
            },
            Some(ChannelKind::Route) => {
                closed::<WireEmpty>(members)?;
                Channel::Route
            }
            Some(ChannelKind::EnsureSession) => Channel::EnsureSession {
                session: closed::<WireSession>(members)?.session,
            },
        }))
    }

    fn encode_open(channel: &Channel) -> (&'static str, Members) {
        let members = match channel {
            Channel::Session { session } | Channel::EnsureSession { session } => {
                members_of(&WireSession {
                    session: session.clone(),
                })
            }
            Channel::Route => Map::new(),
        };
        (channel.kind().as_str(), members)
    }

    fn channel_kind(channel: &Channel) -> ChannelKind {
        channel.kind()
    }

    fn rules(kind: ChannelKind) -> KindRules {
        match kind {
            ChannelKind::Session => KindRules::STREAM,
            ChannelKind::Route => KindRules::ROUTE,
            ChannelKind::EnsureSession => KindRules::NONE,
        }
    }

    fn reset_reasons(kind: ChannelKind) -> &'static [ResetReason] {
        match kind {
            ChannelKind::Session => &SESSION_REASONS,
            ChannelKind::Route => &ROUTE_REASONS,
            ChannelKind::EnsureSession => &ENSURE_SESSION_REASONS,
        }
    }

    fn decode_outcome(members: Members) -> Result<Outcome, Refused> {
        let wire = closed::<WireOutcome>(members)?;
        let name = wire.outcome.as_str();
        let known = OUTCOME_NAMES.contains(&name);
        match (name, wire.started) {
            ("ready", Some(started)) => Ok(Outcome::Ready { started }),
            ("ready", None) => Err(Refused),
            (_, Some(_)) if known => Err(Refused),
            ("ended", None) => Ok(Outcome::Ended),
            ("failed", None) => Ok(Outcome::Failed),
            ("replied", None) => Ok(Outcome::Replied),
            _ => Ok(Outcome::Unknown),
        }
    }

    fn encode_outcome(outcome: &Outcome) -> Members {
        let (name, started) = match outcome {
            Outcome::Ended => ("ended", None),
            Outcome::Failed => ("failed", None),
            Outcome::Replied => ("replied", None),
            Outcome::Ready { started } => ("ready", Some(*started)),
            Outcome::Unknown => ("unknown", None),
        };
        members_of(&WireOutcome {
            outcome: Name::literal(name),
            started,
        })
    }

    fn outcome_for(outcome: Outcome, kind: ChannelKind) -> Outcome {
        if Self::kind_lists(&outcome, kind) {
            outcome
        } else {
            Outcome::Unknown
        }
    }

    fn kind_lists(outcome: &Outcome, kind: ChannelKind) -> bool {
        matches!(
            (kind, outcome),
            (ChannelKind::Session, Outcome::Ended | Outcome::Failed)
                | (ChannelKind::Route, Outcome::Replied)
                | (
                    ChannelKind::EnsureSession,
                    Outcome::Ready { .. } | Outcome::Failed | Outcome::Unknown
                )
        )
    }

    fn decode_request(_name: &str, _members: Members) -> Result<Option<Infallible>, Refused> {
        Ok(None)
    }

    fn request_name(request: &Infallible) -> &'static str {
        match *request {}
    }

    fn encode_request(request: &Infallible) -> Members {
        match *request {}
    }

    fn decode_reply(_request: &str, _members: Members) -> Result<Option<Infallible>, Refused> {
        Ok(None)
    }

    fn reply_request(reply: &Infallible) -> &'static str {
        match *reply {}
    }

    fn encode_reply(reply: &Infallible) -> Members {
        match *reply {}
    }

    fn decode_event(name: &str, members: Members) -> Result<Option<Event>, Refused> {
        if name != FACTS {
            return Ok(None);
        }
        let wire = closed::<WireFacts>(members)?;
        Ok(Some(Event::Facts(Facts {
            platform: wire.platform,
            architecture: wire.architecture,
            executable: wire.executable,
            store_root: wire.store_root,
        })))
    }

    fn event_name(event: &Event) -> &'static str {
        match event {
            Event::Facts(_) => FACTS,
        }
    }

    fn encode_event(event: &Event) -> Members {
        match event {
            Event::Facts(facts) => members_of(&WireFacts {
                platform: facts.platform.clone(),
                architecture: facts.architecture.clone(),
                executable: facts.executable.clone(),
                store_root: facts.store_root.clone(),
            }),
        }
    }

    fn event_subject(_event: &Event) -> Option<String> {
        None
    }

    fn is_facts(event: &Event) -> bool {
        matches!(event, Event::Facts(_))
    }
}
