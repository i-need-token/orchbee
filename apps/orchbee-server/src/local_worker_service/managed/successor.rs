//! The new side of a Server restart that keeps terminals, on Windows and macOS: the successor Server the
//! `orchbee` requesting the restart started suspended and named in the Session record's `successor`
//! entry. It checks that the running Server and its Terminal Host can hand the Session over, starts
//! its own Worker on the Worker endpoint the restart's ticket names, and asks the running Server to
//! `prepare`. From `prepared` it has 25 seconds to take the Session store and the exact capture,
//! let its Worker adopt the Host, write the binding record and commit under the record's writer
//! lock. After the commit it ends the replaced Server and opens the Session as any start does.
//! Until `prepared` a refusal ends its Worker and names the reason in its exit code; afterwards a
//! failure ends its Worker and asks the replaced Server to take the Session back. A macOS successor
//! first catches stop signals and leaves the requester's session, as a macOS start does, and
//! removes the socket files the replaced Server left before it opens the Session. It records
//! `handover-successor`.

use super::{Admitting, admit_worker, adoption, checkpoint, stop};
use crate::{
    agent_service::{AgentPolicy, AgentService},
    session_shutdown::SessionShutdown,
    worker_registry::WorkerRegistry,
};
use orchbee_connection_handshake::{
    CapabilitySet, ConnectionKind, EnvironmentName, Extensions, GenerationRange, HandoverOutcome,
    HandoverRefusal, HandoverStep, HandoverTicket, ProcessRole, ProcessState, RECONNECT,
};
use orchbee_local_client_protocol::{
    HandoverReply, exchange_handover_before, exchange_query_before,
};
use orchbee_local_ipc::LocalListener;
use orchbee_local_runtime::{
    Cohort, CohortArtifact, DataRoot, ExactProcess, HostPresence, LockAttempt, LogField, LogLevel,
    LogValue, ModeCheckedLocalSessionRendezvous as Rendezvous, ModeCheckedSessionLock,
    RestartRefusal, RetainedChild, RetainedSessionRecord, SuccessorRecord,
};
use orchbee_local_worker_protocol::WorkerId;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    ffi::OsStr,
    fmt, io,
    path::Path,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

/// What the checks, the Worker start and its admission get together.
const CHECK: Duration = Duration::from_secs(30);
/// How long one `query` may take.
const QUERY: Duration = Duration::from_secs(5);
/// How long the running Server may take to answer `prepare`; it answers within 40 seconds or not
/// at all.
const PREPARE: Duration = Duration::from_secs(45);
/// What the restart has from `prepared` to the commit, inside the replaced Server's 30-second
/// lease.
const COMMIT: Duration = Duration::from_secs(25);
/// How long the replaced Server may take to answer `commit` or `abort`.
const STEP: Duration = Duration::from_secs(10);
/// How long the replaced Server may take to leave after the commit before it is ended.
const LEAVE: Duration = Duration::from_secs(30);
/// How long the Session lifetime lock may stay held by the leaving Server.
const LIFETIME: Duration = Duration::from_secs(10);
/// What the opened Session's restored launches get, as at any start.
const OPEN: Duration = Duration::from_secs(20);
/// How long a Worker this Server ends may take to exit.
const END: Duration = Duration::from_secs(5);

/// The `query` extension that lists what each connected Client offered.
const CLIENTS: &str = "clients";
/// The `query` extension that counts the background programs the running Worker runs.
const BACKGROUND_PROGRAMS: &str = "background-programs";

const RECORD: &str = "handover-successor";

/// A refusal before the replaced Server prepared, which this Server's exit code names.
#[derive(Debug)]
pub(crate) struct Refused(pub(crate) RestartRefusal);

impl fmt::Display for Refused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "restart refused: {}", self.0.name())
    }
}

impl std::error::Error for Refused {}

/// Runs the successor Server of the restart `ticket` names for the Session at `runtime`.
pub(in crate::local_worker_service) fn run(
    runtime: &Path,
    ticket: HandoverTicket,
    #[cfg(windows)] configuration: orchbee_local_runtime::UserSettingsLoad,
    #[cfg(unix)] configuration: super::super::startup_configuration::Location,
) -> io::Result<()> {
    // First, so a stop signal that arrives during the restart waits for the signal thread.
    #[cfg(unix)]
    let signals = super::signals::catch()?;
    // Out of the requester's session, so its terminal's signals no longer reach this Server.
    #[cfg(unix)]
    nix::unistd::setsid()?;
    #[cfg(unix)]
    let configuration =
        super::super::read_configuration(configuration).map_err(io::Error::other)?;
    let started = Instant::now();
    let deadline = started + CHECK;
    let cohort = Cohort::current(deadline)?;
    let session = Rendezvous::from_runtime_directory(runtime).map_err(io::Error::other)?;
    let root = DataRoot::resolve(|name| std::env::var_os(name))
        .map_err(|_| io::Error::from(io::ErrorKind::NotFound))?;
    let environment = orchbee_local_runtime::environment_name();
    let mut record = match SuccessorRecord::open(&root, &session, *ticket.as_bytes()) {
        Ok(record) => record,
        Err(_) => return Err(refused(RestartRefusal::NotRecorded, started)),
    };
    let replaced = match (session.activation_started(), record.open_replaced()) {
        (Ok(true), Ok(Some(replaced))) => replaced,
        _ => return Err(refused(RestartRefusal::NotServing, started)),
    };
    let worker_id = check(&root, &session, &record, &environment, deadline)
        .map_err(|refusal| refused(refusal, started))?;

    // The Worker connects to the endpoint the ticket names: the running Server still serves the
    // Session's own until it leaves.
    let endpoint = session.successor_worker_socket_path(*ticket.as_bytes());
    let listener = LocalListener::bind(&endpoint)?;
    #[cfg(unix)]
    let mut bound = BoundEndpoint(Some(endpoint));
    #[cfg(unix)]
    if let Some(path) = &bound.0 {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    listener.set_nonblocking(true)?;
    let agents = Arc::new(AgentService::open_observing(
        AgentPolicy {
            read: true,
            manage: true,
        },
        None,
    )?);
    let shutdown = Arc::new(SessionShutdown::default());
    #[cfg(unix)]
    let own = super::own_stop_identity()?;
    #[cfg(unix)]
    signals.forward(Arc::clone(&shutdown), own)?;
    #[cfg(unix)]
    let socket_root = session.hold_socket_root()?;
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let executable = cohort.executable(CohortArtifact::Worker);
    let worker_argument = format!("{:032x}", u128::from_be_bytes(worker_id.as_bytes()));
    let ticket_argument = ticket.to_string();
    let arguments = [
        OsStr::new("--runtime-directory"),
        runtime.as_os_str(),
        OsStr::new("--worker-id"),
        OsStr::new(&worker_argument),
        OsStr::new("--ticket"),
        OsStr::new(&ticket_argument),
    ];
    let mut worker = record
        .spawn_worker(&executable, &arguments, deadline)
        .inspect_err(|_| failed("start", started))?;
    let mut carrier = None;
    let admitted = admit_worker(
        &listener,
        &mut worker,
        Admitting {
            registry: &registry,
            cohort: cohort.identity(),
            agents: &agents,
            replacing: Some(worker_id),
            shutdown: &shutdown,
        },
        deadline,
        &mut carrier,
    );
    if let Err(error) = admitted {
        failed("start", started);
        return Err(checkpoint::abort_before_launch(worker, carrier, error));
    }
    let Some(carrier) = carrier else {
        failed("start", started);
        return Err(checkpoint::abort_before_launch(
            worker,
            None,
            io::Error::other("Worker wire owner did not start"),
        ));
    };

    let prepared = exchange_handover_before(
        session.client_socket_path(),
        environment.clone(),
        HandoverStep::Prepare,
        ticket,
        Instant::now() + PREPARE,
        None,
    );
    match prepared {
        Ok(HandoverReply::Result(HandoverOutcome::Prepared)) => {}
        Ok(HandoverReply::Result(HandoverOutcome::Refused(reason))) => {
            end_worker(worker);
            return Err(refused(refusal_of(reason), started));
        }
        Ok(HandoverReply::Unsupported) => {
            end_worker(worker);
            return Err(refused(RestartRefusal::Unsupported, started));
        }
        Err(error) if !error.submitted => {
            end_worker(worker);
            return Err(refused(RestartRefusal::NotServing, started));
        }
        // The running Server may hold the Session for this ticket.
        Ok(HandoverReply::Result(_)) | Err(_) => {
            end_worker(worker);
            abort(&session, &environment, ticket);
            failed("prepare", started);
            return Err(io::Error::other("the running Server did not prepare"));
        }
    }

    let commit_deadline = Instant::now() + COMMIT;
    let taken = take_over(
        &root,
        &session,
        &mut record,
        &registry,
        worker_id,
        &configuration,
        commit_deadline,
    );
    let (kept, handle, terminals, pending) = match taken {
        Ok(taken) => taken,
        Err((phase, error)) => {
            end_worker(worker);
            abort(&session, &environment, ticket);
            failed(phase, started);
            return Err(error);
        }
    };

    // The record names this Server: the replaced one leaves on `commit`, or once its lease ends.
    let _ = exchange_handover_before(
        session.client_socket_path(),
        environment,
        HandoverStep::Commit,
        ticket,
        Instant::now() + STEP,
        None,
    );
    leave(&replaced);
    let opened = (|| {
        let lifetime = lifetime_lock(&session)?;
        // The replaced Server leaves its socket files, which no listener serves once it ended.
        #[cfg(unix)]
        session.remove_endpoints(&lifetime)?;
        let activation = session.retain_active_record()?;
        let session_record = RetainedSessionRecord::retain(&root, &session)?;
        kept.committed(RetainedSessionRecord::retain(&root, &session)?)?;
        Ok::<_, io::Error>((lifetime, activation, session_record))
    })();
    let (lifetime, activation, session_record) = match opened {
        Ok(opened) => opened,
        Err(error) => {
            failed("open", started);
            return Err(error);
        }
    };
    record_outcome(
        LogLevel::Info,
        &[
            ("outcome", LogValue::Token("committed")),
            ("elapsedMs", LogValue::Count(milliseconds(started))),
        ],
    );
    // The Session's stop removes the endpoint from here on.
    #[cfg(unix)]
    bound.0.take();
    stop::run(
        stop::Owners {
            session,
            lifetime,
            #[cfg(unix)]
            socket_root,
            record: activation,
            worker,
            carrier,
            handle,
            adopted: true,
            listener,
            registry,
            waker: Arc::downgrade(&shutdown),
            worker_executable: executable,
            serving: crate::local_client_service::ServingWorker::default(),
            ingress: None,
            terminals,
            agents,
            checkpoint: Some(kept),
            automatic_updates: None,
            update_job: Arc::default(),
            session_record,
            worker_ticket: Some(ticket),
        },
        shutdown,
        #[cfg(unix)]
        own,
        cohort.identity(),
        pending,
        Instant::now() + OPEN,
        configuration,
    )
}

/// Checks that the running Server and its Terminal Host can hand the Session over, and answers the
/// Session Worker id the binding record names, which this Server's Worker keeps.
fn check(
    root: &DataRoot,
    session: &Rendezvous,
    record: &SuccessorRecord,
    environment: &EnvironmentName,
    deadline: Instant,
) -> Result<WorkerId, RestartRefusal> {
    let answered = exchange_query_before(
        session.client_socket_path(),
        environment.clone(),
        (Instant::now() + QUERY).min(deadline),
        None,
    )
    .map_err(|_| RestartRefusal::NotServing)?;
    if answered.restart_keeps_terminals() != Some(true) {
        return Err(RestartRefusal::Unsupported);
    }
    match answered.processes() {
        [server, ..]
            if server.role == ProcessRole::Server
                && server.pid.get() == record.replaced_process_id() =>
        {
            if server.state != ProcessState::Ready {
                return Err(RestartRefusal::Busy);
            }
        }
        _ => return Err(RestartRefusal::NotServing),
    }
    if !answered
        .processes()
        .iter()
        .any(|entry| entry.role == ProcessRole::Worker)
    {
        return Err(RestartRefusal::WorkerUnavailable);
    }
    // A count the running Worker did not give in time reads as not idle.
    match answered
        .extensions()
        .get(BACKGROUND_PROGRAMS)
        .and_then(serde_json::Value::as_u64)
    {
        Some(0) => {}
        Some(_) => return Err(RestartRefusal::BackgroundPrograms),
        None => return Err(RestartRefusal::WorkerUnavailable),
    }
    if !clients_rejoin(answered.extensions()) {
        return Err(RestartRefusal::Clients);
    }
    let slots = RetainedSessionRecord::retain(root, session)
        .and_then(|record| record.hosts())
        .map_err(|_| RestartRefusal::Records)?;
    let mut hosts = Vec::new();
    for slot in slots {
        match slot.presence {
            HostPresence::NotStarted => {}
            HostPresence::Running(Some(host)) => hosts.push((slot.slot, host)),
            _ => return Err(RestartRefusal::Host),
        }
    }
    // A restart from another installation may need a Host of its own, which a Session whose every
    // slot holds a Host leaves no room for.
    if hosts.is_empty() || hosts.len() == orchbee_local_runtime::MAXIMUM_HOSTS {
        return Err(RestartRefusal::Host);
    }
    let bindings = orchbee_local_runtime::read_session_bindings(root, session)
        .ok()
        .flatten()
        .and_then(|bytes| crate::binding_record::decode(&bytes))
        .ok_or(RestartRefusal::Records)?;
    let named = hosts.iter().all(|(_, host)| {
        bindings.hosts.iter().any(|named| {
            named.process.is_some_and(|process| {
                (process.process_id.get(), process.creation_time.get()) == *host
            })
        })
    });
    if !named {
        return Err(RestartRefusal::Records);
    }
    let ours = ConnectionKind::WorkerHost.supported_generations();
    for (slot, host) in hosts {
        #[cfg(windows)]
        let endpoint = Rendezvous::slot_endpoint_path(session.runtime_directory(), slot);
        // A macOS Session runs its one Host in the first slot.
        #[cfg(unix)]
        let endpoint = if slot == orchbee_local_runtime::HostSlot::FIRST {
            Rendezvous::host_endpoint_path(session.runtime_directory())
        } else {
            return Err(RestartRefusal::Host);
        };
        let answered = exchange_query_before(
            &endpoint,
            environment.clone(),
            (Instant::now() + QUERY).min(deadline),
            None,
        )
        .map_err(|_| RestartRefusal::Host)?;
        let ready = answered.processes().first().is_some_and(|entry| {
            entry.role == ProcessRole::TerminalHost
                && entry.pid.get() == host.0
                && entry.state == ProcessState::Ready
                && entry.connections.iter().any(|connection| {
                    connection.kind.as_str() == ConnectionKind::WorkerHost.name()
                        && connection.generations.highest_common(ours).is_some()
                })
        });
        if !ready {
            return Err(RestartRefusal::Host);
        }
    }
    Ok(WorkerId::from_bytes(bindings.worker_id.as_bytes()))
}

/// Whether every connected Client reconnects to this Server by itself and can join it: it offered
/// `reconnect` and a `client-server` range that meets this build's.
fn clients_rejoin(extensions: &Extensions) -> bool {
    let Some(serde_json::Value::Array(clients)) = extensions.get(CLIENTS) else {
        return false;
    };
    let ours = ConnectionKind::ClientServer.supported_generations();
    clients.iter().all(|client| {
        let generations = client
            .get("generations")
            .and_then(|value| serde_json::from_value::<GenerationRange>(value.clone()).ok());
        let capabilities = client
            .get("capabilities")
            .and_then(|value| serde_json::from_value::<CapabilitySet>(value.clone()).ok());
        generations.is_some_and(|generations| generations.highest_common(ours).is_some())
            && capabilities.is_some_and(|capabilities| capabilities.contains(RECONNECT))
    })
}

type Taken = (
    checkpoint::Kept,
    crate::worker_obcn_carrier::WorkerRequestHandle,
    Arc<crate::local_terminal_service::LocalTerminalService>,
    crate::local_terminal_service::CheckpointRestore,
);

/// Takes the Session from the prepared Server: the store and its exact capture, the Host through
/// this Server's Worker, the binding record, and the commit, all by `deadline`. A failure names its
/// phase.
fn take_over(
    root: &DataRoot,
    session: &Rendezvous,
    record: &mut SuccessorRecord,
    registry: &Arc<Mutex<WorkerRegistry>>,
    worker_id: WorkerId,
    configuration: &orchbee_local_runtime::UserSettingsLoad,
    deadline: Instant,
) -> Result<Taken, (&'static str, io::Error)> {
    let (retained, kept, capture) =
        checkpoint::take(root, session).map_err(|error| ("take", error))?;
    let adopted = (|| {
        let adopting = adoption::Adopting::find(&retained, Some(&kept))?
            .ok_or_else(|| io::Error::other("the Session keeps no Terminal Host"))?;
        if adopting.worker_id() != worker_id {
            return Err(io::Error::other(
                "the exact capture names another Session Worker",
            ));
        }
        let (handle, mut terminals, pending) =
            adopting.restore(registry, Some(capture), configuration)?;
        terminals.configure_graphics_files(session.runtime_directory());
        kept.write_bindings_exact(&terminals, &stop::checkpoint_worker(&handle)?)?;
        Ok::<_, io::Error>((handle, Arc::new(terminals), pending))
    })();
    let (handle, terminals, pending) = adopted.map_err(|error| ("adopt", error))?;
    if Instant::now() >= deadline {
        return Err(("commit", io::ErrorKind::TimedOut.into()));
    }
    record.commit(deadline).map_err(|error| ("commit", error))?;
    Ok((kept, handle, terminals, pending))
}

/// The socket file of the Worker endpoint a successor bound, which it removes when it fails before
/// it serves the Session; a socket file outlives its listener.
#[cfg(unix)]
struct BoundEndpoint(Option<std::path::PathBuf>);

#[cfg(unix)]
impl Drop for BoundEndpoint {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Ends the Worker this Server started; its connection's owner ends with it.
fn end_worker(mut worker: RetainedChild) {
    let _ = worker.terminate_before(Instant::now() + END);
}

/// Asks the replaced Server to take the Session back; its lease and the end of this Server's Worker
/// do the same when the request does not arrive.
fn abort(session: &Rendezvous, environment: &EnvironmentName, ticket: HandoverTicket) {
    let _ = exchange_handover_before(
        session.client_socket_path(),
        environment.clone(),
        HandoverStep::Abort,
        ticket,
        Instant::now() + STEP,
        None,
    );
}

/// Waits for the replaced Server to leave, and ends it once [`LEAVE`] has passed.
fn leave(replaced: &ExactProcess) {
    let deadline = Instant::now() + LEAVE;
    while Instant::now() < deadline {
        if replaced.exited().unwrap_or(false) {
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
    let _ = replaced.end_before(Instant::now() + END);
}

/// The Session lifetime lock, which the replaced Server holds until it ends.
fn lifetime_lock(session: &Rendezvous) -> io::Result<ModeCheckedSessionLock> {
    let deadline = Instant::now() + LIFETIME;
    loop {
        match orchbee_local_runtime::acquire_past_probes(|| {
            session.try_acquire_session_lifetime_lock()
        }) {
            Ok(LockAttempt::Acquired(lifetime)) => return Ok(lifetime),
            Ok(_) if Instant::now() < deadline => thread::sleep(Duration::from_millis(50)),
            Ok(_) => return Err(io::Error::other("Session lifetime is already owned")),
            Err(error) => return Err(io::Error::other(error)),
        }
    }
}

/// The refusal a `prepare` answer names.
fn refusal_of(reason: HandoverRefusal) -> RestartRefusal {
    match reason {
        HandoverRefusal::Busy => RestartRefusal::Busy,
        HandoverRefusal::BackgroundPrograms => RestartRefusal::BackgroundPrograms,
        HandoverRefusal::WorkerUnavailable => RestartRefusal::WorkerUnavailable,
        HandoverRefusal::Clients => RestartRefusal::Clients,
        HandoverRefusal::NotRecorded => RestartRefusal::NotRecorded,
        HandoverRefusal::UnknownStep | HandoverRefusal::Unknown => RestartRefusal::Unsupported,
    }
}

fn refused(refusal: RestartRefusal, started: Instant) -> io::Error {
    record_outcome(
        LogLevel::Warning,
        &[
            ("outcome", LogValue::Token("refused")),
            ("reason", LogValue::Token(refusal.name())),
            ("elapsedMs", LogValue::Count(milliseconds(started))),
        ],
    );
    io::Error::other(Refused(refusal))
}

fn failed(phase: &'static str, started: Instant) {
    record_outcome(
        LogLevel::Error,
        &[
            ("outcome", LogValue::Token("failed")),
            ("phase", LogValue::Token(phase)),
            ("elapsedMs", LogValue::Count(milliseconds(started))),
        ],
    );
}

fn record_outcome(level: LogLevel, fields: &[LogField]) {
    orchbee_local_runtime::log_event(level, RECORD, fields);
}

fn milliseconds(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}
