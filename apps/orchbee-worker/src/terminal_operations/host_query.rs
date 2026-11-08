//! Answers the Server's Host query with the entry the connected Host reports for itself. The Host
//! is asked, not described from the Worker's records, because only the Host knows its state, its
//! live terminal count and the generations it supports. A Host that has not answered within the
//! budget is left out, and a request fenced to another Worker or generation lists no Host.

use std::{
    num::NonZeroU64,
    time::{Duration, Instant},
};

use orchbee_capability_node_protocol::{HostQueryRequest, HostQueryResult};
use orchbee_local_worker_protocol::WorkerId;

use crate::host_connection::HostBrokerConnection;

/// Leaves the rest of the Server's wait for its reply to arrive.
const HOST_QUERY_BUDGET: Duration = Duration::from_millis(500);

pub(super) fn execute(
    host: Option<&HostBrokerConnection>,
    request: HostQueryRequest,
    worker: WorkerId,
    generation: NonZeroU64,
) -> HostQueryResult {
    let current = request.worker_id.as_bytes() == worker.as_bytes()
        && request.worker_generation.get() == generation.get();
    let deadline = Instant::now() + HOST_QUERY_BUDGET;
    let hosts = host
        .filter(|_| current)
        .and_then(|host| host.query_process(deadline))
        .into_iter()
        .collect();
    HostQueryResult { request, hosts }
}
