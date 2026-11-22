//! The closed `OBHS` messages and their growable enums. An initiator sends one offer or one control
//! request; the acceptor answers once. A growable enum maps an unknown name to `Unknown`, which
//! encodes as `unknown`.

use crate::{
    Build, BuildEnvironment, CapabilitySet, ContractName, EnvironmentName, Extensions, Generation,
    GenerationRange, HandoverTicket, InvalidValue, ProcessState, ProductVersion, QueryResult,
    RequestName,
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

/// A control request type this build knows. `query`, `drain`, and `exit` carry no members, and
/// every version answers them; only a Server serves `handover`, and any other acceptor answers it
/// `unsupported`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RequestKind {
    /// Report versions and state; changes nothing.
    Query,
    /// Stop accepting new work while existing work continues; idempotent.
    Drain,
    /// Settle in-flight work, reply, and end the process.
    Exit,
    /// One step of a Server restart that keeps terminals, for the successor `ticket` names.
    Handover {
        /// The step.
        step: HandoverStep,
        /// The ticket of the successor recorded in the Session record.
        ticket: HandoverTicket,
    },
}

impl RequestKind {
    /// The wire `type`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Query => "query",
            Self::Drain => "drain",
            Self::Exit => "exit",
            Self::Handover { .. } => "handover",
        }
    }
}

// The name of a known request type, for the `unsupported` reply of an acceptor that does not
// serve it.
impl From<RequestKind> for RequestName {
    fn from(kind: RequestKind) -> Self {
        Self::known(kind.as_str())
    }
}

/// A step of a `handover` request.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HandoverStep {
    /// Hold new intents, record the Session exactly, and release the Session store and the Worker
    /// under a lease.
    Prepare,
    /// End the Server once the Session record names its successor.
    Commit,
    /// Take back what `prepare` released.
    Abort,
    /// A step this build does not know.
    Unknown,
}

impl HandoverStep {
    const NAMES: [(Self, &'static str); 4] = [
        (Self::Prepare, "prepare"),
        (Self::Commit, "commit"),
        (Self::Abort, "abort"),
        (Self::Unknown, "unknown"),
    ];

    /// The wire name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        growable_name(&Self::NAMES, self)
    }

    /// The step `name` denotes, `Unknown` for any other grammatical name.
    #[must_use]
    pub fn from_name(name: &str) -> Self {
        growable_value(&Self::NAMES, name, Self::Unknown)
    }
}

/// Why the Server refused a `handover` step.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HandoverRefusal {
    /// The Server is already handing over, stopping, or replacing its Worker.
    Busy,
    /// Its Worker runs a background program.
    BackgroundPrograms,
    /// It has no serving Worker.
    WorkerUnavailable,
    /// A connected Client, or one being admitted, did not offer `reconnect`: it may not cope with
    /// its Server being replaced.
    Clients,
    /// The Session record does not name this Server with a successor holding the ticket.
    NotRecorded,
    /// The step is one the Server does not know.
    UnknownStep,
    /// A reason this build does not know.
    Unknown,
}

impl HandoverRefusal {
    const NAMES: [(Self, &'static str); 7] = [
        (Self::Busy, "busy"),
        (Self::BackgroundPrograms, "background-programs"),
        (Self::WorkerUnavailable, "worker-unavailable"),
        (Self::Clients, "clients"),
        (Self::NotRecorded, "not-recorded"),
        (Self::UnknownStep, "unknown-step"),
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

/// The wire `state` of a `handover-result`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum HandoverState {
    Prepared,
    Committed,
    Aborted,
    Refused,
    Unknown,
}

impl HandoverState {
    const NAMES: [(Self, &'static str); 5] = [
        (Self::Prepared, "prepared"),
        (Self::Committed, "committed"),
        (Self::Aborted, "aborted"),
        (Self::Refused, "refused"),
        (Self::Unknown, "unknown"),
    ];

    /// The wire name.
    pub(crate) fn as_str(self) -> &'static str {
        growable_name(&Self::NAMES, self)
    }

    /// The state `name` denotes, `Unknown` for any other grammatical name.
    pub(crate) fn from_name(name: &str) -> Self {
        growable_value(&Self::NAMES, name, Self::Unknown)
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

/// What a `handover` step did.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HandoverOutcome {
    /// `prepare` released the Session store and the Worker; the lease runs.
    Prepared,
    /// `commit` found the successor recorded; the Server ends.
    Committed,
    /// `abort` took back what `prepare` released, or found nothing released.
    Aborted,
    /// The Server refused the step and changed nothing.
    Refused(HandoverRefusal),
    /// A state this build does not know.
    Unknown,
}

/// The reply to `handover`.
#[derive(Clone, Debug, PartialEq)]
pub struct HandoverResult {
    /// What the step did; only a refusal carries a reason.
    pub outcome: HandoverOutcome,
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
    /// Replies to `handover`.
    HandoverResult(HandoverResult),
    /// Replies to an unknown request type.
    Unsupported(Unsupported),
}

/// Every message `type` this build knows, in both directions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MessageType {
    Offer,
    // Only the kinds without members; `handover` has its own variant.
    Request(RequestKind),
    Handover,
    Answer,
    Rejection,
    QueryResult,
    DrainResult,
    ExitResult,
    HandoverResult,
    Unsupported,
}

impl MessageType {
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "offer" => Self::Offer,
            "query" => Self::Request(RequestKind::Query),
            "drain" => Self::Request(RequestKind::Drain),
            "exit" => Self::Request(RequestKind::Exit),
            "handover" => Self::Handover,
            "answer" => Self::Answer,
            "rejection" => Self::Rejection,
            "query-result" => Self::QueryResult,
            "drain-result" => Self::DrainResult,
            "exit-result" => Self::ExitResult,
            "handover-result" => Self::HandoverResult,
            "unsupported" => Self::Unsupported,
            _ => return None,
        })
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Offer => "offer",
            Self::Request(kind) => kind.as_str(),
            Self::Handover => "handover",
            Self::Answer => "answer",
            Self::Rejection => "rejection",
            Self::QueryResult => "query-result",
            Self::DrainResult => "drain-result",
            Self::ExitResult => "exit-result",
            Self::HandoverResult => "handover-result",
            Self::Unsupported => "unsupported",
        }
    }
}

growable_serde!(RejectionReason);
growable_serde!(HandoverStep);
growable_serde!(HandoverRefusal);
growable_serde!(HandoverState);
