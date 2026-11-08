//! The closed `OBHS` messages and their growable enums. An initiator sends one offer or one control
//! request; the acceptor answers once. A growable enum maps an unknown name to `Unknown`, which
//! encodes as `unknown`.

use crate::{
    Build, BuildEnvironment, CapabilitySet, ContractName, EnvironmentName, Extensions, Generation,
    GenerationRange, InvalidValue, ProcessState, ProductVersion, QueryResult, RequestName,
    name::{growable_name, growable_serde, growable_value},
};

/// An initiator's proposal for a negotiated connection.
#[derive(Clone, Debug, PartialEq)]
pub struct Offer {
    /// The connection kind the initiator expects at this endpoint, possibly one unknown here.
    pub connection: ContractName,
    /// Every generation the initiator supports.
    pub generations: GenerationRange,
    /// Every capability name the initiator implements.
    pub capabilities: CapabilitySet,
    /// The initiator's product version.
    pub product_version: ProductVersion,
    /// The initiator's build identity.
    pub build: Build,
    /// The initiator's environment and, outside `orchbee`, its fingerprint.
    pub environment: BuildEnvironment,
    /// Extension members.
    pub extensions: Extensions,
}

/// The acceptor's admission of an offer.
#[derive(Clone, Debug, PartialEq)]
pub struct Answer {
    /// The highest generation in both ranges; records after the answer carry it.
    pub generation: Generation,
    /// The capability names both sides listed.
    pub capabilities: CapabilitySet,
    /// The acceptor's product version.
    pub product_version: ProductVersion,
    /// The acceptor's build identity.
    pub build: Build,
    /// Extension members.
    pub extensions: Extensions,
}

/// The acceptor's refusal of an offer or request; the acceptor closes after sending it.
#[derive(Clone, Debug, PartialEq)]
pub struct Rejection {
    /// Why the acceptor refused.
    pub reason: RejectionReason,
    /// Every generation the acceptor supports.
    pub generations: GenerationRange,
    /// The acceptor's product version.
    pub product_version: ProductVersion,
    /// The acceptor's environment.
    pub environment: EnvironmentName,
    /// Extension members.
    pub extensions: Extensions,
}

/// Why an acceptor refused, listed in the order the acceptor checks; `Timeout` and `Unknown` stand
/// outside that order.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RejectionReason {
    /// The frame does not decode as an offer or a control request.
    Malformed,
    /// The offer names another connection kind than this endpoint's.
    ConnectionUnsupported,
    /// The initiator's environment differs from the acceptor's.
    EnvironmentMismatch,
    /// In `orchbee`, the initiator's highest generation is below the acceptor's lowest.
    InitiatorTooOld,
    /// In `orchbee`, the initiator's lowest generation is above the acceptor's highest.
    InitiatorTooNew,
    /// Outside `orchbee`, the fingerprints differ.
    FingerprintMismatch,
    /// No connection slot is free.
    Capacity,
    /// The acceptor is draining or stopping.
    Stopping,
    /// No complete frame arrived within [`CANDIDATE_DEADLINE`](crate::CANDIDATE_DEADLINE).
    Timeout,
    /// A reason this build does not know.
    Unknown,
}

impl RejectionReason {
    const NAMES: [(Self, &'static str); 10] = [
        (Self::Malformed, "malformed"),
        (Self::ConnectionUnsupported, "connection-unsupported"),
        (Self::EnvironmentMismatch, "environment-mismatch"),
        (Self::InitiatorTooOld, "initiator-too-old"),
        (Self::InitiatorTooNew, "initiator-too-new"),
        (Self::FingerprintMismatch, "fingerprint-mismatch"),
        (Self::Capacity, "capacity"),
        (Self::Stopping, "stopping"),
        (Self::Timeout, "timeout"),
        (Self::Unknown, "unknown"),
    ];

    /// The wire name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        growable_name(&Self::NAMES, self)
    }

    /// The reason `name` denotes, `Unknown` for any other grammatical name.
    #[must_use]
    pub fn from_name(name: &str) -> Self {
        growable_value(&Self::NAMES, name, Self::Unknown)
    }
}

/// A control request type every version answers.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RequestKind {
    /// Report versions and state; changes nothing.
    Query,
    /// Stop accepting new work while existing work continues; idempotent.
    Drain,
    /// Settle in-flight work, reply, and end the process.
    Exit,
}

impl RequestKind {
    /// The wire `type`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Query => "query",
            Self::Drain => "drain",
            Self::Exit => "exit",
        }
    }
}

/// A known control request, sent in place of an offer.
#[derive(Clone, Debug, PartialEq)]
pub struct ControlRequest {
    /// The request type.
    pub kind: RequestKind,
    /// The requester's environment, checked as for an offer.
    pub environment: EnvironmentName,
    /// Extension members.
    pub extensions: Extensions,
}

/// A control request of a type this build does not know. Only `type` and `environment` are read;
/// its other members belong to a newer contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnknownRequest {
    request: RequestName,
    environment: EnvironmentName,
}

impl UnknownRequest {
    /// A request named `request`, which must not be a message type this build knows.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidValue`] when `request` names a known message type.
    pub fn new(request: RequestName, environment: EnvironmentName) -> Result<Self, InvalidValue> {
        if MessageType::from_name(request.as_str()).is_some() {
            return Err(InvalidValue);
        }
        Ok(Self {
            request,
            environment,
        })
    }

    /// The requested type.
    #[must_use]
    pub fn request(&self) -> &RequestName {
        &self.request
    }

    /// The requester's environment.
    #[must_use]
    pub fn environment(&self) -> &EnvironmentName {
        &self.environment
    }
}

/// The reply to `drain`.
#[derive(Clone, Debug, PartialEq)]
pub struct DrainResult {
    /// The process state after the request.
    pub state: ProcessState,
    /// Extension members.
    pub extensions: Extensions,
}

/// The reply to `exit`, sent before the process ends.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExitResult {
    /// Extension members.
    pub extensions: Extensions,
}

/// The reply to a control request type the acceptor does not know.
#[derive(Clone, Debug, PartialEq)]
pub struct Unsupported {
    /// The requested type.
    pub request: RequestName,
    /// Extension members.
    pub extensions: Extensions,
}

/// A frame an initiator sends.
#[derive(Clone, Debug, PartialEq)]
pub enum InitiatorMessage {
    /// An offer for a negotiated connection.
    Offer(Offer),
    /// A known control request.
    Request(ControlRequest),
    /// A control request of an unknown type.
    UnknownRequest(UnknownRequest),
}

/// A frame an acceptor sends.
#[derive(Clone, Debug, PartialEq)]
pub enum AcceptorMessage {
    /// Admits an offer.
    Answer(Answer),
    /// Refuses an offer or request.
    Rejection(Rejection),
    /// Replies to `query`.
    QueryResult(QueryResult),
    /// Replies to `drain`.
    DrainResult(DrainResult),
    /// Replies to `exit`.
    ExitResult(ExitResult),
    /// Replies to an unknown request type.
    Unsupported(Unsupported),
}

/// Every message `type` this build knows, in both directions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MessageType {
    Offer,
    Request(RequestKind),
    Answer,
    Rejection,
    QueryResult,
    DrainResult,
    ExitResult,
    Unsupported,
}

impl MessageType {
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "offer" => Self::Offer,
            "query" => Self::Request(RequestKind::Query),
            "drain" => Self::Request(RequestKind::Drain),
            "exit" => Self::Request(RequestKind::Exit),
            "answer" => Self::Answer,
            "rejection" => Self::Rejection,
            "query-result" => Self::QueryResult,
            "drain-result" => Self::DrainResult,
            "exit-result" => Self::ExitResult,
            "unsupported" => Self::Unsupported,
            _ => return None,
        })
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Offer => "offer",
            Self::Request(kind) => kind.as_str(),
            Self::Answer => "answer",
            Self::Rejection => "rejection",
            Self::QueryResult => "query-result",
            Self::DrainResult => "drain-result",
            Self::ExitResult => "exit-result",
            Self::Unsupported => "unsupported",
        }
    }
}

growable_serde!(RejectionReason);
