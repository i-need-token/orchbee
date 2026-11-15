//! Windows observation-only request. No Host startup, viewer, input or Agent acknowledgement.
use super::{Error, HostBrokerConnection, NonZeroU64, UnixStream, WorkerId};
use orchbee_capability_node_protocol::{
    TerminalTitleObservation, TerminalTitleRow, TerminalTitlesRequest, TerminalTitlesResult,
};
pub(super) fn execute(
    server: &UnixStream,
    host: Option<&HostBrokerConnection>,
    request: TerminalTitlesRequest,
    worker: WorkerId,
    generation: NonZeroU64,
) -> Result<TerminalTitlesResult, Box<dyn Error>> {
    request.validate()?;
    let current = request.worker_id.as_bytes() == worker.as_bytes()
        && request.worker_generation.get() == generation.get();
    if current && let Some(host) = host {
        let observation = crate::host_connection::HostExchange::open(host, server);
        observation
            .observe_pending(32)
            .map_err(|class| observation.error(class))?;
    }
    let rows = request
        .identities
        .into_iter()
        .map(|identity| {
            let observation = host
                .filter(|host| {
                    current
                        && identity.host_epoch().as_bytes() == *host.host_epoch().as_bytes()
                        && !host.disposed_terminal(identity.terminal_id())
                })
                .map_or(TerminalTitleObservation::Unavailable, |host| {
                    host.terminal_title(identity)
                });
            TerminalTitleRow {
                identity,
                observation,
            }
        })
        .collect();
    Ok(TerminalTitlesResult {
        request_id: request.request_id,
        rows,
    })
}
