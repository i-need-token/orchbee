//! Reads the Host's own process entry with one `query` control request on a new connection to the
//! Host's endpoint, under the caller's deadline. The Host answers on a control lane from facts it
//! publishes, leaving the broker connection untouched. The entry is relayed without capabilities,
//! which only the answering process states.

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
    Ok(entry)
}

#[cfg(all(test, windows))]
mod tests;
