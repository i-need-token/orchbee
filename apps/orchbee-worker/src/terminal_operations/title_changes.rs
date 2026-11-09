//! Answers the Server's physical title change reads on every platform. Before a Host starts the
//! projection is empty; afterwards the Host connection's projection answers from memory. It never
//! starts a Host, drains Host events, attaches a viewer or waits.
use super::{HostBrokerConnection, NonZeroU64, WorkerId};
use orchbee_capability_node_protocol::{
    TerminalTitleChangePage, TerminalTitleChangesOutcome, TerminalTitleChangesRequest,
    TerminalTitleChangesResult,
};

pub(super) fn execute(
    host: Option<&HostBrokerConnection>,
    request: TerminalTitleChangesRequest,
    worker: WorkerId,
    generation: NonZeroU64,
) -> TerminalTitleChangesResult {
    let outcome = if request.worker_id.as_bytes() != worker.as_bytes()
        || request.worker_generation.get() != generation.get()
    {
        TerminalTitleChangesOutcome::RejectedFencing
    } else if let Some(host) = host {
        host.title_changes(request.after)
    } else if request.after == 0 {
        TerminalTitleChangesOutcome::Page(TerminalTitleChangePage {
            rows: Vec::new(),
            next: 0,
            complete: true,
        })
    } else {
        // No change exists before the Host starts, so no reader can hold this cursor.
        TerminalTitleChangesOutcome::Reset
    };
    TerminalTitleChangesResult { request, outcome }
}
