//! The permanent `OBHS` connection handshake: the strict frame and message codec, the acceptor's
//! ordered admission checks, the initiator's checks of the reply, and the Rust mirror of the
//! generation registry with this build's development fingerprints. The language-neutral authority
//! is `schemas/connection`. Transports, deadlines, peer credentials, slot counting, and the effects
//! of control requests stay with each endpoint.

mod admission;
mod codec;
mod error;
mod extensions;
mod generation;
mod identity;
mod message;
mod name;
mod process;
mod query;
mod registry;
mod strict_json;

pub use admission::{
    AcceptorLoad, AcceptorProfile, Decision, OfferFailure, RequestFailure, RequestReply,
    conclude_offer, conclude_request,
};
pub use codec::{
    CANDIDATE_DEADLINE, FRAME_HEADER_BYTES, FRAME_MAGIC, MAXIMUM_PAYLOAD_BYTES,
    decode_acceptor_frame, decode_initiator_frame, encode_acceptor_frame, encode_initiator_frame,
    read_acceptor_message, read_initiator_message,
};
pub use error::{DecodeError, EncodeError, InvalidValue, ReadError};
pub use extensions::Extensions;
pub use generation::{CapabilitySet, Generation, GenerationRange, MAXIMUM_CAPABILITIES};
pub use identity::{
    Build, BuildEnvironment, CohortIdentity, Fingerprint, ProductVersion, SourceCommit,
};
pub use message::{
    AcceptorMessage, Answer, ControlRequest, DrainResult, ExitResult, InitiatorMessage, Offer,
    Rejection, RejectionReason, RequestKind, UnknownRequest, Unsupported,
};
pub use name::{CapabilityName, ContractName, EnvironmentName, RELEASE_ENVIRONMENT, RequestName};
pub use process::{ProcessRole, ProcessState};
pub use query::{
    ConnectionEntry, MAXIMUM_PROCESS_CONNECTIONS, MAXIMUM_QUERY_PROCESSES, ProcessEntry,
    QueryResult,
};
pub use registry::{ConnectionKind, Delivery, FrameFormat, PROMPT_GATED_INPUT};
