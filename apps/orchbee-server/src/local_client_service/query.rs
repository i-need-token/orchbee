//! Answers the handshake's `query` request before allocating any Client Connection state: this
//! Server first, then its Worker when the Server started it. A Worker comes from the Server's own
//! installation, so it shares the Server's version and build. Hosts join the reply once the Worker
//! reports them.

use std::num::NonZeroU32;

use orchbee_connection_handshake::{
    AcceptorMessage, CapabilitySet, ConnectionEntry, ConnectionKind, ContractName, Delivery,
    Extensions, FrameFormat, GenerationRange, InvalidValue, ProcessEntry, ProcessRole,
    ProcessState, QueryResult,
};
use orchbee_local_ipc::LocalStream;

use super::{ServerIdentity, diagnostic, send_handshake};

/// Sends the `query-result`; the caller closes the connection.
pub(super) fn respond(
    stream: &mut LocalStream,
    identity: &ServerIdentity,
    state: ProcessState,
    terminals: u32,
) {
    match result(identity, state, terminals) {
        Ok(result) => {
            let _delivered = send_handshake(stream, &AcceptorMessage::QueryResult(result));
        }
        Err(InvalidValue) => {
            diagnostic::report(&diagnostic::Diagnostic::new("query", "build-result"));
        }
    }
}

/// The reply for `identity` in `state`, which its Worker shares; Restart Session does not keep
/// terminals yet.
pub(super) fn result(
    identity: &ServerIdentity,
    state: ProcessState,
    terminals: u32,
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
    }
    QueryResult::new(processes, Some(false), Extensions::new())
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
