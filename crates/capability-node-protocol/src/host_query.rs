//! The Server's read of the Terminal Hosts connected to its Worker, for the handshake `query`
//! answer: each Host's own process entry, relayed without capabilities. How the Worker learns an
//! entry, and which Hosts it lists, stay with the Worker.

use orchbee_connection_handshake::{MAXIMUM_PROCESS_CONNECTIONS, ProcessEntry, ProcessRole};

use crate::{OperationId, ProtocolError, WorkerGeneration, WorkerId};

/// The most Hosts one reply lists: the handshake's process bound less the Server and its Worker.
pub const MAXIMUM_QUERY_HOSTS: usize = 256;

/// Asks the Worker for the process entry of every Host connected to it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostQueryRequest {
    pub request_id: OperationId,
    pub worker_id: WorkerId,
    pub worker_generation: WorkerGeneration,
}

/// The entries of the Hosts that answered the Worker in time, in the Worker's order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostQueryResult {
    /// The request this result answers, echoed whole.
    pub request: HostQueryRequest,
    pub hosts: Vec<ProcessEntry>,
}

impl HostQueryResult {
    /// # Errors
    ///
    /// Returns [`ProtocolError::InvalidPayload`] for more than [`MAXIMUM_QUERY_HOSTS`] entries, or
    /// for an entry that is not a Terminal Host's, states capabilities, or lists more than
    /// [`MAXIMUM_PROCESS_CONNECTIONS`] connections or lists them out of strictly ascending `kind`
    /// order.
    pub fn validate(&self) -> Result<(), ProtocolError> {
        let valid = self.hosts.len() <= MAXIMUM_QUERY_HOSTS
            && self.hosts.iter().all(|host| {
                host.role == ProcessRole::TerminalHost
                    && host.connections.len() <= MAXIMUM_PROCESS_CONNECTIONS
                    && host.connections.windows(2).all(|pair| match pair {
                        [earlier, later] => earlier.kind < later.kind,
                        _ => false,
                    })
                    && host
                        .connections
                        .iter()
                        .all(|connection| connection.capabilities.is_none())
            });
        if valid {
            Ok(())
        } else {
            Err(ProtocolError::InvalidPayload)
        }
    }
}
