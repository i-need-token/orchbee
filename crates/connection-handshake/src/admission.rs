//! The acceptor's ordered admission checks and the initiator's checks of the reply. Both are pure
//! decisions over explicit facts; the endpoint owns reading, slot counting, lifecycle, and the
//! effects of a served request.

use crate::{
    AcceptorMessage, Answer, Build, BuildEnvironment, CapabilitySet, ConnectionKind, DrainResult,
    EnvironmentName, ExitResult, Extensions, GenerationRange, InitiatorMessage, Offer,
    ProcessState, ProductVersion, QueryResult, Rejection, RejectionReason, RequestKind,
    Unsupported,
};

/// What an acceptor knows about itself when it judges a frame.
#[derive(Clone, Debug, PartialEq)]
pub struct AcceptorProfile {
    /// The connection kind this endpoint serves.
    pub connection: ConnectionKind,
    /// Every generation the acceptor supports.
    pub generations: GenerationRange,
    /// Every capability name the acceptor implements.
    pub capabilities: CapabilitySet,
    /// The acceptor's product version.
    pub product_version: ProductVersion,
    /// The acceptor's build identity.
    pub build: Build,
    /// The acceptor's environment and, outside `orchbee`, its fingerprint.
    pub environment: BuildEnvironment,
}

/// The acceptor's load when a frame arrives.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceptorLoad {
    /// Whether a slot for a negotiated connection is free.
    pub session_slot_free: bool,
    /// Whether a slot for a control connection is free.
    pub control_slot_free: bool,
    /// The acceptor's lifecycle state; any state but `Ready` refuses offers with `stopping`.
    pub state: ProcessState,
}

/// What the acceptor does with a decoded frame.
#[derive(Clone, Debug, PartialEq)]
pub enum Decision {
    /// Send the answer; the connection continues at its generation.
    Accept(Answer),
    /// Perform the request, send its one reply, and close.
    Serve(RequestKind),
    /// Send this reply to an unknown request type and close.
    Unsupported(Unsupported),
    /// Send the rejection and close.
    Reject(Rejection),
}

impl AcceptorProfile {
    /// This build's profile for `connection`: the registry's generations and capability names,
    /// and the environment identity `environment` implies.
    #[must_use]
    pub fn local(
        connection: ConnectionKind,
        product_version: ProductVersion,
        build: Build,
        environment: EnvironmentName,
    ) -> Self {
        Self {
            connection,
            generations: connection.supported_generations(),
            capabilities: connection.capabilities(),
            product_version,
            build,
            environment: BuildEnvironment::for_build(
                environment,
                connection.development_fingerprint(),
            ),
        }
    }

    /// The rejection carrying this acceptor's facts. Use it directly for `malformed`, when the
    /// frame does not decode, and for `timeout`.
    #[must_use]
    pub fn reject(&self, reason: RejectionReason) -> Rejection {
        Rejection {
            reason,
            generations: self.generations,
            product_version: self.product_version.clone(),
            environment: self.environment.name().clone(),
            extensions: Extensions::new(),
        }
    }

    /// Runs the checks after decoding, in order, and stops at the first failure: connection kind,
    /// environment, then in `orchbee` the generation ranges or elsewhere the fingerprint, then the
    /// slot, then the lifecycle. A control request checks only its environment and a control slot;
    /// it is served while draining or stopping.
    #[must_use]
    pub fn decide(&self, message: &InitiatorMessage, load: AcceptorLoad) -> Decision {
        match message {
            InitiatorMessage::Offer(offer) => self.decide_offer(offer, load),
            InitiatorMessage::Request(request) => {
                match self.check_request(&request.environment, load) {
                    Some(reason) => Decision::Reject(self.reject(reason)),
                    None => Decision::Serve(request.kind),
                }
            }
            InitiatorMessage::UnknownRequest(request) => {
                match self.check_request(request.environment(), load) {
                    Some(reason) => Decision::Reject(self.reject(reason)),
                    None => Decision::Unsupported(Unsupported {
                        request: request.request().clone(),
                        extensions: Extensions::new(),
                    }),
                }
            }
        }
    }

    fn decide_offer(&self, offer: &Offer, load: AcceptorLoad) -> Decision {
        if offer.connection.as_str() != self.connection.name() {
            return Decision::Reject(self.reject(RejectionReason::ConnectionUnsupported));
        }
        if offer.environment.name() != self.environment.name() {
            return Decision::Reject(self.reject(RejectionReason::EnvironmentMismatch));
        }
        let ours = self.generations;
        let theirs = offer.generations;
        let generation = match self.environment.fingerprint() {
            None if theirs.highest() < ours.lowest() => {
                return Decision::Reject(self.reject(RejectionReason::InitiatorTooOld));
            }
            None if theirs.lowest() > ours.highest() => {
                return Decision::Reject(self.reject(RejectionReason::InitiatorTooNew));
            }
            // Both checks passed, so the ranges overlap and the lower highest is common.
            None => theirs.highest().min(ours.highest()),
            Some(fingerprint) => {
                // Equal fingerprints imply equal registries; disjoint ranges disprove that.
                let common = (offer.environment.fingerprint() == Some(fingerprint))
                    .then(|| theirs.highest_common(ours))
                    .flatten();
                match common {
                    Some(generation) => generation,
                    None => {
                        return Decision::Reject(self.reject(RejectionReason::FingerprintMismatch));
                    }
                }
            }
        };
        if !load.session_slot_free {
            return Decision::Reject(self.reject(RejectionReason::Capacity));
        }
        if load.state != ProcessState::Ready {
            return Decision::Reject(self.reject(RejectionReason::Stopping));
        }
        Decision::Accept(Answer {
            generation,
            capabilities: offer.capabilities.intersection(&self.capabilities),
            product_version: self.product_version.clone(),
            build: self.build.clone(),
            extensions: Extensions::new(),
        })
    }

    fn check_request(
        &self,
        environment: &EnvironmentName,
        load: AcceptorLoad,
    ) -> Option<RejectionReason> {
        if environment != self.environment.name() {
            Some(RejectionReason::EnvironmentMismatch)
        } else if !load.control_slot_free {
            Some(RejectionReason::Capacity)
        } else {
            None
        }
    }
}

impl Offer {
    /// This build's offer for `connection`: the registry's generations and capability names, and
    /// the environment identity `environment` implies.
    #[must_use]
    pub fn local(
        connection: ConnectionKind,
        product_version: ProductVersion,
        build: Build,
        environment: EnvironmentName,
    ) -> Self {
        Self {
            connection: connection.contract_name(),
            generations: connection.supported_generations(),
            capabilities: connection.capabilities(),
            product_version,
            build,
            environment: BuildEnvironment::for_build(
                environment,
                connection.development_fingerprint(),
            ),
            extensions: Extensions::new(),
        }
    }
}

/// Why an offer did not produce a usable answer.
#[derive(Clone, Debug, PartialEq)]
pub enum OfferFailure {
    /// The acceptor refused.
    Rejected(Rejection),
    /// The answer selects a generation outside the offer's range or lists a name the offer did
    /// not; the initiator closes.
    InvalidAnswer,
    /// The reply is not an answer or a rejection; the initiator closes.
    UnexpectedReply,
}

/// Checks the acceptor's reply to `offer`.
///
/// # Errors
///
/// Returns [`OfferFailure`] for a rejection, an answer outside the offer, or another reply.
pub fn conclude_offer(offer: &Offer, reply: AcceptorMessage) -> Result<Answer, OfferFailure> {
    match reply {
        AcceptorMessage::Answer(answer)
            if offer.generations.contains(answer.generation)
                && answer.capabilities.is_subset(&offer.capabilities) =>
        {
            Ok(answer)
        }
        AcceptorMessage::Answer(_) => Err(OfferFailure::InvalidAnswer),
        AcceptorMessage::Rejection(rejection) => Err(OfferFailure::Rejected(rejection)),
        _ => Err(OfferFailure::UnexpectedReply),
    }
}

/// The reply that pairs with a served control request.
#[derive(Clone, Debug, PartialEq)]
pub enum RequestReply {
    /// The reply to `query`.
    Query(QueryResult),
    /// The reply to `drain`.
    Drain(DrainResult),
    /// The reply to `exit`.
    Exit(ExitResult),
}

/// Why a control request did not produce its reply.
#[derive(Clone, Debug, PartialEq)]
pub enum RequestFailure {
    /// The acceptor refused.
    Rejected(Rejection),
    /// The acceptor does not serve the request type.
    Unsupported(Unsupported),
    /// The reply belongs to another request or to an offer.
    UnexpectedReply,
}

/// Checks the acceptor's reply to a `kind` request.
///
/// # Errors
///
/// Returns [`RequestFailure`] for a rejection, an `unsupported` reply, or an unpaired reply.
pub fn conclude_request(
    kind: RequestKind,
    reply: AcceptorMessage,
) -> Result<RequestReply, RequestFailure> {
    match (kind, reply) {
        (RequestKind::Query, AcceptorMessage::QueryResult(result)) => {
            Ok(RequestReply::Query(result))
        }
        (RequestKind::Drain, AcceptorMessage::DrainResult(result)) => {
            Ok(RequestReply::Drain(result))
        }
        (RequestKind::Exit, AcceptorMessage::ExitResult(result)) => Ok(RequestReply::Exit(result)),
        (_, AcceptorMessage::Rejection(rejection)) => Err(RequestFailure::Rejected(rejection)),
        (_, AcceptorMessage::Unsupported(reply)) => Err(RequestFailure::Unsupported(reply)),
        _ => Err(RequestFailure::UnexpectedReply),
    }
}
