//! The acceptor's ordered checks and the initiator's checks of a reply, driven by
//! `admission.json`.

use std::collections::BTreeMap;

use orchbee_connection_handshake::{
    AcceptorLoad, AcceptorMessage, AcceptorProfile, Build, BuildEnvironment, CapabilitySet,
    ConnectionKind, Decision, EnvironmentName, Fingerprint, GenerationRange, InitiatorMessage,
    OfferFailure, ProcessState, ProductVersion, RejectionReason, RequestFailure, RequestKind,
    conclude_offer, conclude_request, decode_acceptor_frame, decode_initiator_frame,
    encode_acceptor_frame, encode_initiator_frame,
};
use serde::Deserialize;

use crate::support::{self, TestResult};

const ADMISSION: &str = include_str!("../../../schemas/connection/v1/fixtures/admission.json");

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Fixtures {
    acceptors: BTreeMap<String, AcceptorFixture>,
    cases: Vec<AdmissionCase>,
    replies: Vec<OfferReplyCase>,
    request_replies: Vec<RequestReplyCase>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct AcceptorFixture {
    connection: String,
    generations: GenerationRange,
    capabilities: CapabilitySet,
    product_version: ProductVersion,
    build: Build,
    environment: EnvironmentName,
    fingerprint: Option<Fingerprint>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct LoadFixture {
    session_slot_free: bool,
    control_slot_free: bool,
    state: ProcessState,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AdmissionCase {
    name: String,
    acceptor: String,
    load: LoadFixture,
    payload: String,
    decision: ExpectedDecision,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
enum ExpectedDecision {
    Accept {
        generation: u16,
        capabilities: Vec<String>,
    },
    Reject {
        reason: String,
    },
    Serve {
        request: String,
    },
    Unsupported {
        request: String,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OfferReplyCase {
    name: String,
    offer: String,
    reply: String,
    outcome: OfferOutcome,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
enum OfferOutcome {
    Accepted,
    Close,
    Rejected { reason: String },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestReplyCase {
    name: String,
    request: String,
    reply: String,
    outcome: RequestOutcome,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
enum RequestOutcome {
    Reply,
    Close,
    Rejected { reason: String },
    Unsupported,
}

impl AcceptorFixture {
    fn profile(&self) -> Result<AcceptorProfile, Box<dyn std::error::Error>> {
        let environment = match self.fingerprint {
            None => BuildEnvironment::release(),
            Some(fingerprint) => {
                BuildEnvironment::development(self.environment.clone(), fingerprint)?
            }
        };
        if environment.name() != &self.environment {
            return Err(format!("{} needs a fingerprint", self.environment).into());
        }
        Ok(AcceptorProfile {
            connection: ConnectionKind::from_name(&self.connection)
                .ok_or_else(|| format!("{} is no connection kind", self.connection))?,
            generations: self.generations,
            capabilities: self.capabilities.clone(),
            product_version: self.product_version.clone(),
            build: self.build.clone(),
            environment,
        })
    }
}

#[test]
fn acceptor_decisions_follow_the_check_order() -> TestResult {
    let fixtures: Fixtures = serde_json::from_str(ADMISSION)?;
    assert!(!fixtures.cases.is_empty());
    for case in fixtures.cases {
        let name = &case.name;
        let profile = fixtures
            .acceptors
            .get(&case.acceptor)
            .ok_or_else(|| format!("{name}: no acceptor {}", case.acceptor))?
            .profile()?;
        let load = AcceptorLoad {
            session_slot_free: case.load.session_slot_free,
            control_slot_free: case.load.control_slot_free,
            state: case.load.state,
        };
        let message = decode_initiator_frame(&support::frame(case.payload.as_bytes())?);
        let decision = match &message {
            Ok(message) => profile.decide(message, load),
            Err(_) => Decision::Reject(profile.reject(RejectionReason::Malformed)),
        };
        let reply = match (&decision, &case.decision) {
            (
                Decision::Accept(answer),
                ExpectedDecision::Accept {
                    generation,
                    capabilities,
                },
            ) => {
                assert_eq!(answer.generation.get(), *generation, "{name}");
                let selected: Vec<&str> = answer
                    .capabilities
                    .iter()
                    .map(|name| name.as_str())
                    .collect();
                assert_eq!(selected, *capabilities, "{name}");
                assert_eq!(answer.product_version, profile.product_version, "{name}");
                assert_eq!(answer.build, profile.build, "{name}");
                let reply = AcceptorMessage::Answer(answer.clone());
                let Ok(InitiatorMessage::Offer(offer)) = &message else {
                    return Err(format!("{name}: only an offer is accepted").into());
                };
                assert_eq!(
                    conclude_offer(offer, reply.clone()).as_ref(),
                    Ok(answer),
                    "{name}"
                );
                reply
            }
            (Decision::Reject(rejection), ExpectedDecision::Reject { reason }) => {
                assert_eq!(rejection.reason.as_str(), reason, "{name}");
                assert_eq!(rejection, &profile.reject(rejection.reason), "{name}");
                AcceptorMessage::Rejection(rejection.clone())
            }
            (Decision::Serve(kind), ExpectedDecision::Serve { request }) => {
                assert_eq!(kind.as_str(), request, "{name}");
                continue;
            }
            (Decision::Unsupported(reply), ExpectedDecision::Unsupported { request }) => {
                assert_eq!(reply.request.as_str(), request, "{name}");
                AcceptorMessage::Unsupported(reply.clone())
            }
            (actual, expected) => {
                return Err(format!("{name}: decided {actual:?}, expected {expected:?}").into());
            }
        };
        assert_eq!(
            decode_acceptor_frame(&encode_acceptor_frame(&reply)?)?,
            reply,
            "{name}"
        );
    }
    Ok(())
}

#[test]
fn initiator_closes_on_every_reply_outside_its_offer_or_request() -> TestResult {
    let fixtures: Fixtures = serde_json::from_str(ADMISSION)?;
    assert!(!fixtures.replies.is_empty() && !fixtures.request_replies.is_empty());
    for case in fixtures.replies {
        let name = &case.name;
        let InitiatorMessage::Offer(offer) =
            decode_initiator_frame(&support::frame(case.offer.as_bytes())?)?
        else {
            return Err(format!("{name}: offer must be an offer").into());
        };
        let reply = decode_acceptor_frame(&support::frame(case.reply.as_bytes())?)?;
        match (conclude_offer(&offer, reply), &case.outcome) {
            (Ok(_), OfferOutcome::Accepted)
            | (
                Err(OfferFailure::InvalidAnswer | OfferFailure::UnexpectedReply),
                OfferOutcome::Close,
            ) => {}
            (Err(OfferFailure::Rejected(rejection)), OfferOutcome::Rejected { reason })
                if rejection.reason.as_str() == reason => {}
            (actual, expected) => {
                return Err(format!("{name}: concluded {actual:?}, expected {expected:?}").into());
            }
        }
    }
    for case in fixtures.request_replies {
        let name = &case.name;
        let kind = [RequestKind::Query, RequestKind::Drain, RequestKind::Exit]
            .into_iter()
            .find(|kind| kind.as_str() == case.request)
            .ok_or_else(|| format!("{name}: {} is no request", case.request))?;
        let reply = decode_acceptor_frame(&support::frame(case.reply.as_bytes())?)?;
        match (conclude_request(kind, reply), &case.outcome) {
            (Ok(_), RequestOutcome::Reply)
            | (Err(RequestFailure::UnexpectedReply), RequestOutcome::Close)
            | (Err(RequestFailure::Unsupported(_)), RequestOutcome::Unsupported) => {}
            (Err(RequestFailure::Rejected(rejection)), RequestOutcome::Rejected { reason })
                if rejection.reason.as_str() == reason => {}
            (actual, expected) => {
                return Err(format!("{name}: concluded {actual:?}, expected {expected:?}").into());
            }
        }
    }
    Ok(())
}

#[test]
fn local_offer_meets_local_acceptor_in_release_and_development() -> TestResult {
    let idle = AcceptorLoad {
        session_slot_free: true,
        control_slot_free: true,
        state: ProcessState::Ready,
    };
    for connection in ConnectionKind::ALL {
        for environment in ["orchbee", "ob-dev-0123456789abcdef"] {
            let environment = EnvironmentName::new(environment)?;
            let version = ProductVersion::new("0.0.0-dev")?;
            let offer = orchbee_connection_handshake::Offer::local(
                connection,
                version.clone(),
                Build::default(),
                environment.clone(),
            );
            let fingerprint = offer.environment.fingerprint().copied();
            assert_eq!(
                fingerprint,
                (!environment.is_release()).then(|| connection.development_fingerprint())
            );
            let message = decode_initiator_frame(&encode_initiator_frame(
                &InitiatorMessage::Offer(offer.clone()),
            )?)?;
            let acceptor =
                AcceptorProfile::local(connection, version, Build::default(), environment);
            let Decision::Accept(answer) = acceptor.decide(&message, idle) else {
                return Err(format!("{} must admit its own offer", connection.name()).into());
            };
            assert_eq!(
                answer.generation,
                connection.supported_generations().highest()
            );
            assert_eq!(
                conclude_offer(&offer, AcceptorMessage::Answer(answer.clone())),
                Ok(answer)
            );
        }
    }
    Ok(())
}
