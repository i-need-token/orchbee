//! Opens the Host broker connection with the `worker-host` handshake before any OBTP record:
//! this build's offer, the Host's one reply, and the retry that a Host still ending another Worker
//! session asks for. The caller dials, bounds the reply with `CANDIDATE_DEADLINE`, and owns the
//! Hello that follows an answer.

use std::{
    error::Error,
    io::{self, Read, Write},
    time::Duration,
};

use orchbee_connection_handshake::{
    Build, ConnectionKind, EnvironmentName, InitiatorMessage, Offer, OfferFailure, ProductVersion,
    ReadError, RejectionReason, SourceCommit, conclude_offer, encode_initiator_frame,
    read_acceptor_message,
};

/// How long a Worker waits before it dials a busy Host again.
pub(super) const RETRY_INTERVAL: Duration = Duration::from_millis(20);

/// How the Host received one offer.
#[derive(Debug, Eq, PartialEq)]
pub(super) enum Outcome {
    /// The Host admitted this Worker; OBTP follows on the same connection.
    Answered,
    /// The Host still holds another Worker session, answering `capacity` or closing before any
    /// reply while that session ends; the caller dials again until its deadline.
    Busy,
}

/// This build's `worker-host` offer: the registry's generations and capabilities, the package
/// version, the source commit release packaging compiled in, and the compiled environment.
pub(super) fn local() -> Result<Offer, Box<dyn Error>> {
    Ok(Offer::local(
        ConnectionKind::WorkerHost,
        ProductVersion::new(env!("CARGO_PKG_VERSION"))?,
        Build {
            commit: source_commit(),
            cohort: None,
        },
        EnvironmentName::new(orchbee_local_runtime::environment_stem())?,
    ))
}

/// The commit `ORCHBEE_SOURCE_COMMIT` named at compile time; a build without a valid one sends
/// none, as a development build does.
fn source_commit() -> Option<SourceCommit> {
    option_env!("ORCHBEE_SOURCE_COMMIT").and_then(|commit| SourceCommit::parse(commit).ok())
}

/// Sends `offer` and reads the Host's reply through `stream`, whose read deadline the caller set.
///
/// # Errors
///
/// Returns an error for a Host that predates the handshake, refuses this Worker for any reason
/// but `capacity`, answers outside the offer, or fails the stream other than by closing.
pub(super) fn exchange(
    stream: &mut (impl Read + Write),
    offer: &Offer,
) -> Result<Outcome, Box<dyn Error>> {
    let frame = encode_initiator_frame(&InitiatorMessage::Offer(offer.clone()))?;
    match stream.write_all(&frame) {
        Ok(()) => {}
        Err(error) if closed(&error) => return Ok(Outcome::Busy),
        Err(error) => return Err(error.into()),
    }
    let reply = match read_acceptor_message(stream) {
        Ok(reply) => reply,
        Err(ReadError::Closed) => return Ok(Outcome::Busy),
        Err(ReadError::Io(error)) if closed(&error) => return Ok(Outcome::Busy),
        Err(ReadError::Io(error)) => return Err(error.into()),
        Err(ReadError::ForeignProtocol) => {
            return Err(refused("Terminal Host predates the connection handshake"));
        }
        Err(ReadError::Malformed(_)) => {
            return Err(refused("Terminal Host sent a malformed handshake reply"));
        }
    };
    match conclude_offer(offer, reply) {
        Ok(_) => Ok(Outcome::Answered),
        Err(OfferFailure::Rejected(rejection)) if rejection.reason == RejectionReason::Capacity => {
            Ok(Outcome::Busy)
        }
        Err(OfferFailure::Rejected(rejection)) => Err(refused(&format!(
            "Terminal Host refused the Worker connection: {}",
            rejection.reason.as_str()
        ))),
        Err(OfferFailure::InvalidAnswer | OfferFailure::UnexpectedReply) => Err(refused(
            "Terminal Host answered outside the Worker connection offer",
        )),
    }
}

/// The error for a Host that stayed busy until the caller's deadline.
pub(super) fn still_busy() -> Box<dyn Error> {
    io::Error::new(
        io::ErrorKind::TimedOut,
        "Terminal Host kept another Worker session until the connection deadline",
    )
    .into()
}

/// Whether a stream failure means the Host closed this connection, as it does to a connection
/// that reached a control lane while the previous Worker session ended.
fn closed(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::BrokenPipe
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::UnexpectedEof
    )
}

fn refused(message: &str) -> Box<dyn Error> {
    io::Error::new(io::ErrorKind::InvalidData, message.to_owned()).into()
}

/// The answer a fake Host in a test sends to this build's offer, so a test peer speaks the
/// handshake before its scripted OBTP records.
#[cfg(test)]
pub(crate) fn answer_test_offer(stream: &mut (impl Read + Write)) -> io::Result<()> {
    use orchbee_connection_handshake::{
        AcceptorLoad, AcceptorMessage, AcceptorProfile, Decision, ProcessState,
        encode_acceptor_frame, read_initiator_message,
    };

    let message = read_initiator_message(stream).map_err(io::Error::other)?;
    let product_version = ProductVersion::new("0.1.0").map_err(io::Error::other)?;
    let environment = EnvironmentName::new(orchbee_local_runtime::environment_stem())
        .map_err(io::Error::other)?;
    let profile = AcceptorProfile::local(
        ConnectionKind::WorkerHost,
        product_version,
        Build::default(),
        environment,
    );
    let load = AcceptorLoad {
        session_slot_free: true,
        control_slot_free: true,
        state: ProcessState::Ready,
    };
    let Decision::Accept(answer) = profile.decide(&message, load) else {
        return Err(io::Error::other("the test Host refused this build's offer"));
    };
    let frame =
        encode_acceptor_frame(&AcceptorMessage::Answer(answer)).map_err(io::Error::other)?;
    stream.write_all(&frame)
}
