//! Answers the Server's Host query with the entry each Host the Worker serves reports for itself.
//! A Host is asked, not described from the Worker's records, because only the Host knows its state,
//! its live terminal count and the generations it supports. A Host that has not answered within the
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
    hosts: &[&HostBrokerConnection],
    request: HostQueryRequest,
    worker: WorkerId,
    generation: NonZeroU64,
) -> HostQueryResult {
    let current = request.worker_id.as_bytes() == worker.as_bytes()
        && request.worker_generation.get() == generation.get();
    let deadline = Instant::now() + HOST_QUERY_BUDGET;
    let hosts = hosts
        .iter()
        .filter(|_| current)
        .filter_map(|host| host.query_process(deadline))
        .collect();
    let mut result = HostQueryResult { request, hosts };
    // A Host reply the protocol refuses (connections out of kind order or over bound) is answered
    // as a query the Host did not answer, so the Worker keeps serving.
    if result.validate().is_err() {
        result.hosts.clear();
    }
    result
}
