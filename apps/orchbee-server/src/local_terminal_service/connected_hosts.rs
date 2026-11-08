//! Reads the process entries of the Terminal Hosts connected to the current Worker for the
//! handshake `query` answer. The read waits only until the caller's deadline: no Worker, a Worker
//! between generations, a full queue, or a reply past the deadline all read as no Host, and the
//! carrier discards a late reply.

use super::LocalTerminalService;
use crate::worker_obcn_carrier::{ExchangeWait, WorkerOperationResult};
use orchbee_capability_node_protocol::{HostQueryRequest, WorkerGeneration, WorkerId};
use orchbee_connection_handshake::ProcessEntry;
use std::time::Instant;

impl LocalTerminalService {
    /// The entries of the Hosts that answered the Worker, in its order, read before `deadline`.
    pub(crate) fn connected_hosts(&self, deadline: Instant) -> Vec<ProcessEntry> {
        self.read_connected_hosts(deadline).unwrap_or_default()
    }

    fn read_connected_hosts(&self, deadline: Instant) -> Option<Vec<ProcessEntry>> {
        // A reply already due asks the Worker for nothing.
        if Instant::now() >= deadline {
            return None;
        }
        let handle = self.workers.lock().ok()?.request_handle()?;
        let request = HostQueryRequest {
            request_id: self.next_viewer_operation()?,
            worker_id: WorkerId::from_bytes(handle.worker_id().as_bytes()).ok()?,
            worker_generation: WorkerGeneration::new(handle.generation()),
        };
        match handle
            .admit_host_query(request)
            .ok()?
            .recv_deadline(deadline)
        {
            ExchangeWait::Ready(result) => match *result {
                Ok(WorkerOperationResult::HostQuery(result)) => Some(result.hosts),
                _ => None,
            },
            ExchangeWait::Elapsed | ExchangeWait::Lost(_) => None,
        }
    }
}

#[cfg(test)]
mod tests;
