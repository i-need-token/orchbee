//! Answers the handshake's `query` request before allocating any Client Connection state: this
//! Server first, then its Worker when the Server started it, then the Terminal Hosts connected to
//! that Worker. A Worker comes from the Server's own installation, so it shares the Server's
//! version and build. Each Host reports its own entry through the Worker; a Host that has not
//! answered by the time the reply is due is left out rather than listed with guessed facts.

use std::{
    num::NonZeroU32,
    time::{Duration, Instant},
};

use orchbee_connection_handshake::{
    AcceptorMessage, CapabilitySet, ConnectionEntry, ConnectionKind, ContractName, Delivery,
    Extensions, FrameFormat, GenerationRange, InvalidValue, MAXIMUM_QUERY_PROCESSES, ProcessEntry,
    ProcessRole, ProcessState, QueryResult,
};
use orchbee_local_ipc::LocalStream;

use super::{ServerIdentity, diagnostic, send_handshake};
use crate::local_terminal_service::LocalTerminalService;

/// How long after accepting the connection the Server waits for its Worker's Hosts, leaving the
/// rest of the candidate deadline for the reply.
const HOSTS_DUE: Duration = Duration::from_secs(1);

/// Sends the `query-result` for the connection accepted at `accepted`; the caller closes the
/// connection.
pub(super) fn respond(
    stream: &mut LocalStream,
    identity: &ServerIdentity,
    state: ProcessState,
    terminals: &LocalTerminalService,
    accepted: Instant,
) {
    // Hosts follow the Worker they are connected to, so a Server that lists no Worker lists none.
    let hosts = if identity.worker.is_some() {
        terminals.connected_hosts(accepted + HOSTS_DUE)
    } else {
        Vec::new()
    };
    match result(identity, state, terminals.terminal_count(), hosts) {
        Ok(result) => {
            let _delivered = send_handshake(stream, &AcceptorMessage::QueryResult(result));
        }
        Err(InvalidValue) => {
            diagnostic::report(&diagnostic::Diagnostic::new("query", "build-result"));
        }
    }
}

/// The reply for `identity` in `state`, which its Worker shares, followed by at most 256 of `hosts`
/// without capabilities; Restart Session does not keep terminals yet.
pub(super) fn result(
    identity: &ServerIdentity,
    state: ProcessState,
    terminals: u32,
    hosts: Vec<ProcessEntry>,
) -> Result<QueryResult, InvalidValue> {
    let profile = &identity.profile;
    let process = |role, pid, connections| ProcessEntry {
        role,
        product_version: profile.product_version.clone(),
        build: profile.build.clone(),
        environment: profile.environment.name().clone(),
        pid,
        state,
        terminals,
        connections,
    };
    let server_pid = NonZeroU32::new(std::process::id()).ok_or(InvalidValue)?;
    let mut server = vec![
        entry(
            ConnectionKind::ClientServer.name(),
            profile.generations,
            Some(profile.capabilities.clone()),
        )?,
        entry(
            Delivery::AgentHook.name(),
            Delivery::AgentHook.supported_generations(),
            Some(CapabilitySet::default()),
        )?,
    ];
    let mut worker = vec![entry(
        ConnectionKind::WorkerHost.name(),
        ConnectionKind::WorkerHost.supported_generations(),
        None,
    )?];
    for format in FrameFormat::ALL {
        server.push(entry(
            format.name(),
            format.supported_generations(),
            Some(CapabilitySet::default()),
        )?);
        worker.push(entry(format.name(), format.supported_generations(), None)?);
    }
    let mut processes = vec![process(ProcessRole::Server, server_pid, sorted(server))];
    if let Some(pid) = identity.worker {
        processes.push(process(ProcessRole::Worker, pid, sorted(worker)));
        let room = MAXIMUM_QUERY_PROCESSES - processes.len();
        processes.extend(hosts.into_iter().take(room).map(host));
    }
    QueryResult::new(processes, Some(false), Extensions::new())
}

/// A Host's entry as a Server lists it: only the answering process states capabilities.
fn host(mut entry: ProcessEntry) -> ProcessEntry {
    for connection in &mut entry.connections {
        connection.capabilities = None;
    }
    entry.connections = sorted(entry.connections);
    entry
}

fn entry(
    name: &str,
    generations: GenerationRange,
    capabilities: Option<CapabilitySet>,
) -> Result<ConnectionEntry, InvalidValue> {
    Ok(ConnectionEntry {
        kind: ContractName::new(name)?,
        generations,
        capabilities,
    })
}

/// The reply lists each process's connections in ascending bytewise `kind` order.
fn sorted(mut entries: Vec<ConnectionEntry>) -> Vec<ConnectionEntry> {
    entries.sort_by(|left, right| left.kind.cmp(&right.kind));
    entries
}

#[cfg(test)]
mod tests;
