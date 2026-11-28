//! The Session's `query` control request for CLI observations; it never enters the Client phase.

use orchbee_connection_handshake::{ProcessRole, QueryResult};
use orchbee_local_client_protocol::ConnectClientError;
use orchbee_local_ipc::LocalIo;
use orchbee_local_runtime::compare_product_versions;
use std::{cmp::Ordering, path::Path, time::Instant};

/// Asks the Server at `endpoint` for its processes, as this build's environment.
pub(super) fn read(
    endpoint: &Path,
    deadline: Instant,
    wake: Option<LocalIo<'_>>,
) -> Result<QueryResult, ConnectClientError> {
    orchbee_local_client_protocol::exchange_query_before(
        endpoint,
        orchbee_local_runtime::environment_name(),
        deadline,
        wake,
    )
}

/// The query extension naming the failed move that stopped the Session's Worker's moves.
const MOVES_STOPPED: &str = "moves-stopped";

/// What a Session's `query` answer says about the terminals its Worker moves onto its own Terminal
/// Host: those that Hosts older than the Worker still run, the oldest such Host's version, and the
/// failed move that stopped every later move, when one did.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct Moves {
    pub(super) terminals: u32,
    pub(super) version: String,
    pub(super) stopped: Option<MovesStopped>,
}

/// The step of a failed move and its reason, as the Worker's diagnostics name them.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct MovesStopped {
    pub(super) step: String,
    pub(super) reason: String,
}

/// The moves `result` describes; `None` while it lists no Worker, or no Host older than the Worker
/// runs a terminal. A `moves-stopped` member outside its grammar reads as absent.
pub(super) fn moves(result: &QueryResult) -> Option<Moves> {
    let processes = result.processes();
    let worker = processes
        .iter()
        .find(|entry| entry.role == ProcessRole::Worker)?;
    let older = processes.iter().filter(|entry| {
        entry.role == ProcessRole::TerminalHost
            && entry.terminals > 0
            && compare_product_versions(
                entry.product_version.as_str(),
                worker.product_version.as_str(),
            ) == Ordering::Less
    });
    let version = older
        .clone()
        .map(|entry| entry.product_version.as_str())
        .min_by(|left, right| compare_product_versions(left, right))?
        .to_owned();
    let token = |value: &serde_json::Value| {
        value
            .as_str()
            .filter(|token| {
                (1..=32).contains(&token.len())
                    && token
                        .bytes()
                        .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'-'))
            })
            .map(str::to_owned)
    };
    let stopped = result
        .extensions()
        .get(MOVES_STOPPED)
        .and_then(serde_json::Value::as_object)
        .filter(|fields| fields.len() == 2)
        .and_then(|fields| {
            Some(MovesStopped {
                step: token(fields.get("step")?)?,
                reason: token(fields.get("reason")?)?,
            })
        });
    Some(Moves {
        terminals: older
            .map(|entry| entry.terminals)
            .fold(0, u32::saturating_add),
        version,
        stopped,
    })
}
