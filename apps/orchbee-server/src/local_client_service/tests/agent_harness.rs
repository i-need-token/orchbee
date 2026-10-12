//! Reusable test-only transport harness for the Agent first-read. It seeds a
//! real `AgentService` through canonical event dispatch, then serves it over
//! either an in-process `UnixStream` pair driven by `handle_candidate` or a
//! real filesystem listener driven by `serve` — the same code paths the
//! production listener uses. CLI subprocess fixtures reuse `listen` plus the
//! event builders; no production seed flag, environment variable, or wire
//! operation exists anywhere in this path.

use orchbee_local_ipc::{LocalListener as UnixListener, LocalStream as UnixStream};
#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    io,
    path::Path,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};

use orchbee_local_client_protocol::{ClientHello, LocalClientMessage, read_message, write_message};

use super::super::{ConnectionSlots, handle_candidate, serve};
use crate::{
    agent::{
        AgentObservationEvent, AgentTransition, RawAttentionRequest, RawExecutionLocation,
        RawObservation, RawPresence,
    },
    agent_service::{AgentPolicy, AgentService},
    client_session_registry::ClientSessionRegistry,
    local_terminal_service::LocalTerminalService,
    workbench_service::{WorkbenchPolicy, WorkbenchService},
    worker_registry::WorkerRegistry,
};

const IO_BUDGET: Duration = Duration::from_secs(5);

/// `session-observed` with a native rich observation and the lifecycle,
/// turn-lifecycle, and question capabilities declared.
pub(crate) fn session_observed(session_id: &str, generation: f64) -> AgentObservationEvent {
    AgentObservationEvent::SessionObserved {
        capabilities: vec![
            "sessionLifecycle".to_string(),
            "turnLifecycle".to_string(),
            "questions".to_string(),
        ],
        event_id: format!("seed-observe-{session_id}"),
        execution_location: RawExecutionLocation {
            execution_location_id: format!("loc-{session_id}"),
        },
        observation: RawObservation {
            fidelity: "rich".to_string(),
            freshness: "fresh".to_string(),
            generation,
            sequence: generation,
            source: "native".to_string(),
        },
        parent_session_id: RawPresence::Null,
        presentation_attachments: Vec::new(),
        provider_id: "seed-provider".to_string(),
        session_id: session_id.to_string(),
    }
}

/// `turn-started` with a working phase and an optional activity hint.
/// Follow-up events must repeat the session's stored observation
/// `generation` and advance `sequence` — that is the canonical ordering
/// contract the preamble enforces.
pub(crate) fn turn_started(
    session_id: &str,
    turn_id: &str,
    activity: &str,
    generation: f64,
    sequence: f64,
) -> AgentObservationEvent {
    AgentObservationEvent::TurnStarted {
        activity: RawPresence::Value(activity.to_string()),
        event_id: format!("seed-turn-{turn_id}"),
        generation,
        phase: "working".to_string(),
        sequence,
        session_id: session_id.to_string(),
        turn_id: turn_id.to_string(),
    }
}

/// `attention-requested` carrying a `navigate-only` request without a turn.
pub(crate) fn attention_requested(
    session_id: &str,
    request_id: &str,
    kind: &str,
    generation: f64,
    sequence: f64,
) -> AgentObservationEvent {
    AgentObservationEvent::AttentionRequested {
        event_id: format!("seed-attention-{request_id}"),
        generation,
        request: RawAttentionRequest {
            action_path: "navigate-only".to_string(),
            kind: kind.to_string(),
            request_id: request_id.to_string(),
            turn_id: RawPresence::Null,
        },
        sequence,
        session_id: session_id.to_string(),
    }
}

/// `session-ended` for a session without pending work.
pub(crate) fn session_ended(
    session_id: &str,
    generation: f64,
    sequence: f64,
) -> AgentObservationEvent {
    AgentObservationEvent::SessionEnded {
        event_id: format!("seed-ended-{session_id}"),
        generation,
        sequence,
        session_id: session_id.to_string(),
    }
}

/// Opens a read-enabled service and seeds it through real canonical event
/// dispatch; a seed that does not apply fails the fixture loudly.
pub(crate) fn seeded(events: &[AgentObservationEvent]) -> io::Result<Arc<AgentService>> {
    let service = Arc::new(AgentService::open(AgentPolicy {
        read: true,
        manage: true,
    })?);
    for event in events {
        assert!(
            matches!(service.seed(event), Ok(AgentTransition::Applied { .. })),
            "seed event {} was not applied",
            event.event_id()
        );
    }
    Ok(service)
}

/// The denied-everything workbench policy used by the Agent fixtures — the
/// admitted session only exercises the Agent arm.
fn workbench(terminals: &Arc<LocalTerminalService>) -> WorkbenchService {
    WorkbenchService::new(
        WorkbenchPolicy {
            read: false,
            manage_navigation: false,
            associate_directory: false,
            create_terminal: false,
            manage_terminal_lifecycle: false,
        },
        Arc::clone(terminals),
    )
}

fn terminals() -> Arc<LocalTerminalService> {
    Arc::new(LocalTerminalService::new(Arc::new(Mutex::new(
        WorkerRegistry::new(),
    ))))
}

/// An admitted client side of a real `UnixStream` pair; the server half is
/// owned by the spawned `handle_candidate` thread.
pub(crate) struct AdmittedClient {
    stream: UnixStream,
    handler: JoinHandle<()>,
}

impl AdmittedClient {
    /// The admitted record stream; OBAP requests go in, results come back.
    pub(crate) fn stream(&mut self) -> &mut UnixStream {
        &mut self.stream
    }

    /// Closes the client and joins the handler thread.
    pub(crate) fn join(self) {
        drop(self.stream);
        let _ended = self.handler.join();
    }
}

/// Opens a real `UnixStream` pair, spawns `handle_candidate`, and completes
/// the OBLC admission handshake; the returned stream is ready for OBAP.
pub(crate) fn admitted(agents: Arc<AgentService>, cohort: &str) -> io::Result<AdmittedClient> {
    let terminals = terminals();
    let workbench = workbench(&terminals);
    let registry = Arc::new(Mutex::new(ClientSessionRegistry::new()));
    let slots = Arc::new(ConnectionSlots::new());
    let slot = slots.acquire().ok_or(io::ErrorKind::WouldBlock)?;
    let (mut client, server) = UnixStream::pair()?;
    client.set_read_timeout(Some(IO_BUDGET))?;
    client.set_write_timeout(Some(IO_BUDGET))?;
    let expected_cohort = cohort.to_string();
    let handler = thread::Builder::new()
        .name("orchbee-test-agent-candidate".to_owned())
        .spawn(move || {
            handle_candidate(
                server,
                &registry,
                &expected_cohort,
                slot,
                &terminals,
                &workbench,
                &agents,
            );
        })?;
    write_message(
        &mut client,
        &LocalClientMessage::ClientHello(ClientHello::new(cohort).map_err(io::Error::other)?),
    )
    .map_err(io::Error::other)?;
    match read_message(&mut client).map_err(io::Error::other)? {
        LocalClientMessage::ClientAccepted { .. } => {}
        _other => return Err(io::Error::other("local client admission was not accepted")),
    }
    Ok(AdmittedClient {
        stream: client,
        handler,
    })
}

/// Starts a real OBLC listener at `endpoint` (mode 0600) whose Agent reads
/// are served by `agents`; the real `serve` loop owns the runtime disposal
/// guard. This is the fixture recovery's CLI subprocess tests reuse.
pub(crate) fn listen(
    endpoint: &Path,
    cohort: &str,
    agents: Arc<AgentService>,
) -> io::Result<JoinHandle<io::Result<()>>> {
    let listener = UnixListener::bind(endpoint)?;
    #[cfg(unix)]
    fs::set_permissions(endpoint, fs::Permissions::from_mode(0o600))?;
    let registry = Arc::new(Mutex::new(ClientSessionRegistry::new()));
    let connection_slots = Arc::new(ConnectionSlots::new());
    let expected_cohort: Arc<str> = Arc::from(cohort);
    let terminals = terminals();
    let workbench = Arc::new(workbench(&terminals));
    thread::Builder::new()
        .name("orchbee-test-agent-listener".to_owned())
        .spawn(move || {
            serve(
                listener,
                registry,
                connection_slots,
                expected_cohort,
                terminals,
                workbench,
                agents,
            )
        })
}
