//! Sends the Host one control request on a new connection to its endpoint, under the caller's
//! deadline: `query` reads its own process entry, and `drain` stops it taking new terminals. The
//! Host answers on a control lane, leaving the broker connection untouched. A queried entry is
//! relayed without capabilities, which only the answering process states.

use std::{error::Error, io::Write, path::Path, time::Instant};

use orchbee_connection_handshake::{
    ControlRequest, EnvironmentName, Extensions, InitiatorMessage, ProcessEntry, ProcessRole,
    RequestKind, RequestReply, conclude_request, encode_initiator_frame, read_acceptor_message,
};

use super::admission::Admission;

/// The entry the Host at `endpoint` reports for itself.
///
/// # Errors
///
/// Returns an error when the Host cannot be reached, refuses, answers with anything but its own
/// Terminal Host entry, or has not answered by `deadline`.
pub(super) fn query(endpoint: &Path, deadline: Instant) -> Result<ProcessEntry, Box<dyn Error>> {
    query_with_progress(endpoint, deadline).map(|(entry, _progress)| entry)
}

/// The entry the Host at `endpoint` reports for itself, with the `dispatch-progress` its answer
/// carries when it carries one.
///
/// # Errors
///
/// As [`query`].
pub(super) fn query_with_progress(
    endpoint: &Path,
    deadline: Instant,
) -> Result<(ProcessEntry, Option<u64>), Box<dyn Error>> {
    let mut transport = Admission::connect(endpoint, deadline)?;
    let request = ControlRequest {
        kind: RequestKind::Query,
        environment: EnvironmentName::new(orchbee_local_runtime::environment_stem())?,
        extensions: Extensions::new(),
    };
    transport.write_all(&encode_initiator_frame(&InitiatorMessage::Request(
        request,
    ))?)?;
    let reply = read_acceptor_message(&mut transport)?;
    let Ok(RequestReply::Query(result)) = conclude_request(RequestKind::Query, reply) else {
        return Err("Terminal Host did not answer the process query".into());
    };
    let mut entry = result
        .processes()
        .first()
        .filter(|entry| entry.role == ProcessRole::TerminalHost)
        .cloned()
        .ok_or("Terminal Host answered the process query with another process")?;
    for connection in &mut entry.connections {
        connection.capabilities = None;
    }
    let progress = result
        .extensions()
        .get(DISPATCH_PROGRESS)
        .and_then(|progress| progress.as_u64());
    Ok((entry, progress))
}

/// Has the Host at `endpoint` drain: it takes no new terminal and ends once it holds none.
///
/// # Errors
///
/// Returns an error when the Host cannot be reached, refuses, answers with anything but a
/// `drain-result`, or has not answered by `deadline`.
pub(super) fn drain(endpoint: &Path, deadline: Instant) -> Result<(), Box<dyn Error>> {
    let mut transport = Admission::connect(endpoint, deadline)?;
    let request = ControlRequest {
        kind: RequestKind::Drain,
        environment: EnvironmentName::new(orchbee_local_runtime::environment_stem())?,
        extensions: Extensions::new(),
    };
    transport.write_all(&encode_initiator_frame(&InitiatorMessage::Request(
        request,
    ))?)?;
    let reply = read_acceptor_message(&mut transport)?;
    match conclude_request(RequestKind::Drain, reply) {
        Ok(RequestReply::Drain(_)) => Ok(()),
        _ => Err("Terminal Host did not answer the drain".into()),
    }
}

/// The `query-result` extension that counts the Host's completed dispatch-loop turns.
const DISPATCH_PROGRESS: &str = "dispatch-progress";

#[cfg(all(test, windows))]
mod tests;
