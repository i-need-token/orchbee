//! Admits this process to the Server that requested its start: the token the start passed on
//! standard input, the same-user connection to the endpoint the start named, the hello naming this
//! build, the Server's one answer, and the readiness line that follows admission. Nothing else is
//! written to the Server before it admits the hello.

use std::{
    io::{self, Read, Write},
    path::Path,
    time::Duration,
};

use orchbee_connection_handshake::{ConnectionKind, SourceCommit};
use orchbee_files_protocol::{
    BuildIdentity, LinkHello, LinkMessage, LinkSide, LinkToken, read_link_message,
    write_link_message,
};
use orchbee_local_ipc::LocalStream;
use orchbee_local_peer_credentials::require_same_effective_user;

use crate::exit::{Outcome, Stage};

/// How long the Server may take to answer the hello. The start's own 10-second deadline, after
/// which the Server has the Worker end this process, bounds the whole admission.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(5);
/// 64 lowercase hexadecimal digits and a line feed.
const TOKEN_LINE_BYTES: usize = 65;

/// A hello the Server did not admit.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum AdmissionError {
    /// Writing the hello or reading the answer failed, or the answer was not one.
    Handshake(io::ErrorKind),
    /// The Server refused this build.
    Refused,
}

impl AdmissionError {
    /// The process's end after this refusal.
    pub(crate) const fn outcome(&self) -> Outcome {
        match self {
            Self::Handshake(error) => Outcome::failed(Stage::Handshake, Some(*error)),
            Self::Refused => Outcome::failed(Stage::Refused, None),
        }
    }
}

/// Reads the token line the start wrote: exactly 64 lowercase hexadecimal digits and a line
/// feed. The Worker writes it at once and closes the stream, so the start's deadline bounds the
/// read.
pub(crate) fn read_token(input: &mut impl Read) -> io::Result<LinkToken> {
    let mut line = [0; TOKEN_LINE_BYTES];
    input.read_exact(&mut line)?;
    let (digits, newline) = line.split_at(TOKEN_LINE_BYTES - 1);
    std::str::from_utf8(digits)
        .ok()
        .filter(|_| newline == b"\n")
        .and_then(|digits| LinkToken::parse(digits).ok())
        .ok_or_else(|| io::ErrorKind::InvalidData.into())
}

/// Dials `endpoint` and proves the Server is this user before any byte is exchanged.
pub(crate) fn connect(endpoint: &Path) -> io::Result<LocalStream> {
    let stream = LocalStream::connect(endpoint)?;
    require_same_effective_user(&stream)?;
    Ok(stream)
}

/// Sends the hello carrying `token` and this build, and reads the Server's one answer.
pub(crate) fn admit(stream: &LocalStream, token: LinkToken) -> Result<(), AdmissionError> {
    let failed = |error: io::Error| AdmissionError::Handshake(error.kind());
    stream
        .set_read_timeout(Some(ANSWER_TIMEOUT))
        .map_err(failed)?;
    stream
        .set_write_timeout(Some(ANSWER_TIMEOUT))
        .map_err(failed)?;
    let hello = LinkMessage::Hello(LinkHello {
        build: this_build(),
        token,
    });
    write_link_message(&mut &*stream, &hello).map_err(failed)?;
    match read_link_message(&mut &*stream, LinkSide::Server).map_err(failed)? {
        LinkMessage::Admitted => {}
        LinkMessage::Refused(_) => return Err(AdmissionError::Refused),
        _ => return Err(AdmissionError::Handshake(io::ErrorKind::InvalidData)),
    }
    // Admitted, the link idles until the Server relays work or drains it.
    stream.set_read_timeout(None).map_err(failed)?;
    stream.set_write_timeout(None).map_err(failed)
}

/// Tells a launcher that reads standard output that the Server admitted this process. Readiness is
/// the Server's admission, so a launcher that keeps no reader loses nothing.
pub(crate) fn write_ready_marker(output: &mut impl Write) {
    let _ = output.write_all(b"ready\n").and_then(|()| output.flush());
}

/// This build as a hello names it: the product version, the commit release packaging compiled
/// in, the environment, and the development fingerprint of the Client-Server protocol inputs,
/// which include this link's codec.
pub(crate) fn this_build() -> BuildIdentity {
    BuildIdentity::new(
        env!("CARGO_PKG_VERSION"),
        option_env!("ORCHBEE_SOURCE_COMMIT")
            .and_then(|commit| SourceCommit::parse(commit).ok())
            .map(|commit| *commit.as_bytes()),
        orchbee_local_runtime::environment_stem(),
        *ConnectionKind::ClientServer
            .development_fingerprint()
            .as_bytes(),
    )
    .expect("the package version and environment stem follow the hello's grammar")
}

#[cfg(test)]
mod tests;
