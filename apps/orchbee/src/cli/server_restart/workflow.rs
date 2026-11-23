//! The requester of a Server restart that keeps terminals, on Windows and macOS. It asks the running
//! Server whether its restart keeps terminals, then, holding the Session's activation lock so that no
//! activation starts a Server beside the restart, starts the successor Server suspended (on Windows
//! outside its own Job), names it in the Session record with a fresh ticket and the activation identifier it
//! commits, and resumes it. It waits until the Session's Client endpoint answers from that
//! successor. A successor that ended names a refusal in its exit code; otherwise, once what the
//! attempt left in the record is cleared, the record and the Client endpoint tell where the restart
//! ended: in the successor's Check, after it started its Worker with the running Server keeping or
//! taking back the Session, or with no Server serving it. Only an endpoint found gone proves that
//! no Server serves: one that does not answer in time may still have its Server. Cancelled or out of
//! time once the successor runs, it stops waiting and leaves the restart to finish or fail by
//! itself. Run from a store version folder, it records the reserved generation's start and, when the
//! new pair failed to take over or open, its failure for the fallback after a failed move.

use super::{FailedPhase, Refusal, Restart};
use crate::cli::console_interrupt::ArmedSignals;
use orchbee_connection_handshake::{EnvironmentName, ProcessRole, ProcessState, QueryResult};
use orchbee_local_client_protocol::ConnectClientError;
use orchbee_local_runtime::{
    Cohort, CohortArtifact, DataRoot, FallbackEvent, FallbackRecorder, LockAttempt,
    ModeCheckedLocalSessionRendezvous as Rendezvous, ModeCheckedSessionLock, RendezvousError,
    RestartCleared, RestartRefusal, RestartReservation, SessionCatalog, SessionScope,
    activation_text,
};
#[cfg(windows)]
use orchbee_local_runtime::{JobPlacement, LogLevel, LogValue};
use std::{
    ffi::OsStr,
    io,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

/// How long the namespace and activation locks may stay with others before the restart is refused
/// as busy.
const LOCKS: Duration = Duration::from_secs(2);
/// How long one `query` may take.
const QUERY: Duration = Duration::from_secs(2);
/// How often the Client endpoint is asked which Server answers.
const ASK: Duration = Duration::from_millis(500);
/// How long clearing what an ended attempt left may take.
const CLEAR: Duration = Duration::from_secs(10);
/// How many queries one look at the Client endpoint makes before a Server that does not answer is
/// judged.
const LOOKS: u32 = 3;
/// The pause between two queries of one look.
const LOOK_PAUSE: Duration = Duration::from_millis(250);
/// How long the look once the successor ended may take.
const LAST_LOOK: Duration = Duration::from_secs(7);

/// Runs one restart until `deadline`. A signal ends the wait as [`Restart::TimedOut`], which the
/// caller reports as cancelled.
pub(super) fn restart(deadline: Instant, signals: &mut ArmedSignals) -> Restart {
    let (base, session) = match session() {
        Ok(found) => found,
        // Without its runtime folder no Server can listen for the Session.
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Restart::Refused(Refusal::NotRunning);
        }
        Err(_) => return Restart::Failed(FailedPhase::Start),
    };
    let environment = orchbee_local_runtime::environment_name();
    match look(&session, &environment, deadline, Some(&mut *signals)) {
        Endpoint::Answered(answer) if answer.restart_keeps_terminals() == Some(true) => {}
        Endpoint::Answered(_) | Endpoint::Predates => return Restart::SessionTooOld,
        Endpoint::Refused => return Restart::Refused(Refusal::Busy),
        Endpoint::Gone => return Restart::Refused(Refusal::NotRunning),
        Endpoint::Cancelled => return Restart::TimedOut,
        // A Server may hold the endpoint without answering in time; the successor's Check asks it
        // again and decides, whatever its version.
        Endpoint::Silent => {}
    }
    let Some(start) = Start::prepare(deadline) else {
        return Restart::Failed(FailedPhase::Start);
    };
    let locks = deadline.min(Instant::now() + LOCKS);
    let Some(catalog) = catalog(&base, &start.root, locks) else {
        return Restart::Refused(Refusal::Busy);
    };
    let activation = match activation_lock(&session, locks) {
        Ok(activation) => activation,
        Err(refused) => return refused,
    };
    if cancelled(signals) {
        return Restart::TimedOut;
    }
    let arguments = start.arguments(&session);
    #[cfg(windows)]
    let spawned = catalog.spawn_successor_outside_job(
        &session,
        &activation,
        start.reservation,
        &start.executable,
        &arguments,
        deadline,
    );
    #[cfg(target_os = "macos")]
    let spawned = catalog.spawn_successor(
        &session,
        &activation,
        start.reservation,
        &start.executable,
        &arguments,
        deadline,
    );
    // Only the reservation needs the namespace; the activation lock stays until the end.
    drop(catalog);
    #[cfg(windows)]
    let (mut successor, placement) = match spawned {
        Ok(spawned) => spawned,
        // Another restart's successor still runs.
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            return Restart::Refused(Refusal::Busy);
        }
        // The record names no running generation of this Session: it stopped, or a stop is
        // ending it while its Server still answers. A Server that serves as usual beside it
        // leaves a record no restart can use.
        Err(error) if error.kind() == io::ErrorKind::InvalidData => {
            return match look(&session, &environment, deadline, Some(&mut *signals)) {
                Endpoint::Gone => Restart::Refused(Refusal::NotRunning),
                Endpoint::Cancelled => Restart::TimedOut,
                Endpoint::Answered(answer) if ready(&answer) => Restart::Refused(Refusal::Records),
                _ => Restart::Refused(Refusal::Busy),
            };
        }
        Err(_) => return Restart::Failed(FailedPhase::Start),
    };
    #[cfg(target_os = "macos")]
    let mut successor = match spawned {
        Ok(successor) => successor,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            return Restart::Refused(Refusal::Busy);
        }
        Err(error) if error.kind() == io::ErrorKind::InvalidData => {
            return match look(&session, &environment, deadline, Some(&mut *signals)) {
                Endpoint::Gone => Restart::Refused(Refusal::NotRunning),
                Endpoint::Cancelled => Restart::TimedOut,
                Endpoint::Answered(answer) if ready(&answer) => Restart::Refused(Refusal::Records),
                _ => Restart::Refused(Refusal::Busy),
            };
        }
        Err(_) => return Restart::Failed(FailedPhase::Start),
    };
    #[cfg(windows)]
    record_server_job(placement);
    // The reserved generation is a start of this version, so its failure can count toward the
    // fallback after a move; see `counts_as_failure`.
    let generation = activation_text(&start.reservation.activation_id);
    record_fallback(FallbackEvent::SessionStart {
        activation: &generation,
    });
    let pid = successor.id();
    let mut ask = Instant::now();
    loop {
        if cancelled(signals) {
            return Restart::TimedOut;
        }
        // A successor whose end cannot be observed is judged by the endpoint alone.
        if let Ok(Some(status)) = successor.try_wait() {
            let refusal = status
                .code()
                .and_then(|code| u32::try_from(code).ok())
                .and_then(RestartRefusal::from_code);
            let outcome = ended(&start, &session, &environment, &activation, refusal);
            if counts_as_failure(&outcome) {
                record_fallback(FallbackEvent::SessionFailure {
                    activation: &generation,
                });
            }
            return outcome;
        }
        if Instant::now() >= deadline {
            return Restart::TimedOut;
        }
        if Instant::now() >= ask {
            ask = Instant::now() + ASK;
            if query(&session, &environment, deadline, signals)
                .is_some_and(|answer| answered_by(&answer, pid))
            {
                return Restart::Restarted;
            }
        }
        thread::sleep(
            deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(50)),
        );
    }
}

/// What starting the successor needs, read before any lock is taken.
struct Start {
    root: DataRoot,
    executable: PathBuf,
    /// The configuration location an activation would freeze, which the successor reads.
    configuration: Option<PathBuf>,
    explicit: bool,
    reservation: RestartReservation,
    /// The reservation's ticket as the successor's argument.
    ticket: String,
}

impl Start {
    fn prepare(deadline: Instant) -> Option<Self> {
        let root = DataRoot::resolve(|name| std::env::var_os(name)).ok()?;
        let executable = Cohort::current(deadline)
            .ok()?
            .executable(CohortArtifact::Server);
        let settings =
            orchbee_local_runtime::read_startup_settings(|name| std::env::var_os(name)).ok()?;
        let reservation = RestartReservation {
            ticket: entropy()?,
            activation_id: entropy()?,
        };
        Some(Self {
            root,
            executable,
            configuration: settings.path.clone(),
            explicit: settings.explicit,
            reservation,
            ticket: reservation
                .ticket
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
        })
    }

    fn arguments<'a>(&'a self, session: &'a Rendezvous) -> Vec<&'a OsStr> {
        vec![
            OsStr::new("--runtime-directory"),
            session.runtime_directory().as_os_str(),
            OsStr::new("--handover"),
            OsStr::new(&self.ticket),
            OsStr::new("--configuration-file"),
            self.configuration
                .as_deref()
                .map(Path::as_os_str)
                .unwrap_or(OsStr::new("")),
            OsStr::new("--configuration-file-explicit"),
            OsStr::new(if self.explicit { "true" } else { "false" }),
        ]
    }
}

/// The Session this invocation names; `NotFound` when its runtime folder does not exist.
fn session() -> io::Result<(PathBuf, Rendezvous)> {
    let (base, scope) = crate::local_activation::runtime_location()?;
    let scope = SessionScope::parse(&scope).map_err(|_| io::ErrorKind::InvalidInput)?;
    let session = Rendezvous::validate_mode_checked(&base, scope).map_err(|error| match error {
        RendezvousError::System { source, .. } => source,
        _ => io::ErrorKind::InvalidData.into(),
    })?;
    Ok((base, session))
}

fn cancelled(signals: &mut ArmedSignals) -> bool {
    signals.take().is_ok_and(|signal| signal.is_some())
}

/// One `query` of the Session's Client endpoint; `None` when no Server answered in time or a
/// signal interrupted it.
fn query(
    session: &Rendezvous,
    environment: &EnvironmentName,
    deadline: Instant,
    signals: &ArmedSignals,
) -> Option<QueryResult> {
    orchbee_local_client_protocol::exchange_query_before(
        session.client_socket_path(),
        environment.clone(),
        deadline.min(Instant::now() + QUERY),
        signals.poll_fd(),
    )
    .ok()
}

/// What one look at the Session's Client endpoint found.
enum Endpoint {
    /// A Server answered `query`.
    Answered(QueryResult),
    /// A Server refused the query: it holds the endpoint and answers, only not this request now.
    Refused,
    /// The Server predates the handshake, so it cannot hand its terminals over.
    Predates,
    /// No Server holds the endpoint: two queries in a row found none listening.
    Gone,
    /// No Server answered in time, yet the endpoint was not found gone: its Server may be busy.
    Silent,
    /// A signal interrupted the look.
    Cancelled,
}

/// Looks at the Session's Client endpoint with up to [`LOOKS`] queries before `deadline`, until a
/// Server answers or the endpoint is found gone twice in a row. `signals`, when given, may end the
/// look.
fn look(
    session: &Rendezvous,
    environment: &EnvironmentName,
    deadline: Instant,
    mut signals: Option<&mut ArmedSignals>,
) -> Endpoint {
    let mut gone = 0;
    for attempt in 0..LOOKS {
        if attempt > 0 {
            thread::sleep(LOOK_PAUSE.min(deadline.saturating_duration_since(Instant::now())));
        }
        if Instant::now() >= deadline {
            break;
        }
        let answer = orchbee_local_client_protocol::exchange_query_before(
            session.client_socket_path(),
            environment.clone(),
            deadline.min(Instant::now() + QUERY),
            signals.as_deref().and_then(ArmedSignals::poll_fd),
        );
        match answer {
            Ok(answer) => return Endpoint::Answered(answer),
            Err(ConnectClientError::Refused(_) | ConnectClientError::Rejected(_)) => {
                return Endpoint::Refused;
            }
            Err(ConnectClientError::PredatesHandshake) => return Endpoint::Predates,
            Err(ConnectClientError::ConnectFailed(error)) if no_server(session, &error) => {
                gone += 1;
                if gone == 2 {
                    return Endpoint::Gone;
                }
                continue;
            }
            Err(_) if signals.as_deref_mut().is_some_and(cancelled) => {
                return Endpoint::Cancelled;
            }
            Err(_) => {}
        }
        gone = 0;
    }
    Endpoint::Silent
}

/// Whether a failed connect shows that no Server holds the Session: its endpoint is missing, or, on
/// macOS, where a socket file outlives its Server, the connect was refused and a probe takes the
/// Session lifetime lock, which every Server holds while it lives.
fn no_server(session: &Rendezvous, error: &io::Error) -> bool {
    match error.kind() {
        io::ErrorKind::NotFound => true,
        #[cfg(target_os = "macos")]
        io::ErrorKind::ConnectionRefused => matches!(
            session.try_acquire_session_lifetime_lock(),
            Ok(LockAttempt::Acquired(_))
        ),
        _ => {
            let _ = session;
            false
        }
    }
}

/// Whether the answering Server accepts new work, neither draining nor ending.
fn ready(answer: &QueryResult) -> bool {
    answer.processes().first().is_some_and(|server| {
        server.role == ProcessRole::Server && server.state == ProcessState::Ready
    })
}

/// Whether the Server answering is the successor this invocation started.
fn answered_by(answer: &QueryResult, successor: Option<u32>) -> bool {
    answer.processes().first().is_some_and(|server| {
        server.role == ProcessRole::Server && Some(server.pid.get()) == successor
    })
}

/// The outcome once the successor ended, after what the attempt left in the record is cleared: the
/// refusal its exit code names, or what the record says.
fn ended(
    start: &Start,
    session: &Rendezvous,
    environment: &EnvironmentName,
    activation: &ModeCheckedSessionLock,
    refusal: Option<RestartRefusal>,
) -> Restart {
    let cleared = orchbee_local_runtime::clear_restart(
        &start.root,
        session,
        activation,
        start.reservation,
        Instant::now() + CLEAR,
    );
    if let Some(refusal) = refusal {
        return match refusal {
            RestartRefusal::Unsupported => Restart::SessionTooOld,
            // The successor could not reach the running Server, which may still serve: it ended in
            // its Check, and nothing changed.
            RestartRefusal::NotServing => {
                match look(session, environment, Instant::now() + LAST_LOOK, None) {
                    Endpoint::Gone => Restart::Refused(Refusal::NotRunning),
                    _ => Restart::Failed(FailedPhase::Check),
                }
            }
            RestartRefusal::Busy | RestartRefusal::NotRecorded => Restart::Refused(Refusal::Busy),
            RestartRefusal::BackgroundPrograms => Restart::Refused(Refusal::BackgroundPrograms),
            RestartRefusal::Clients => Restart::Refused(Refusal::Clients),
            RestartRefusal::WorkerUnavailable => Restart::Refused(Refusal::WorkerUnavailable),
            RestartRefusal::Host => Restart::Refused(Refusal::Host),
            RestartRefusal::Records => Restart::Refused(Refusal::Records),
        };
    }
    // The record names the successor, which ended before it served, or the running Server ended
    // during the restart too: no Server serves, and the next start brings the Session back with
    // every terminal.
    if matches!(cleared, Ok(RestartCleared::Committed))
        || matches!(
            look(session, environment, Instant::now() + LAST_LOOK, None),
            Endpoint::Gone
        )
    {
        return Restart::Failed(FailedPhase::Open);
    }
    Restart::Failed(match cleared {
        // The entry named no Worker: the successor ended in its Check, before the running Server
        // could hold the Session.
        Ok(RestartCleared::Cleared {
            worker_recorded: false,
        }) => FailedPhase::Check,
        // The successor had started its Worker, so the running Server may have held the Session:
        // it keeps it or took it back, whether this command or that Server removed the entry, or
        // the record cannot be read. The running Server serves either way.
        _ => FailedPhase::Takeover,
    })
}

/// Whether a successor's end counts as a failure of the reserved generation: it ended without
/// naming a refusal, which it does only before the running Server prepared, and after it had started
/// its Worker or committed, so the new version failed to take the Session over or to open it. A
/// successor still running at the deadline counts nothing, and the store counts one generation's
/// failure once, whoever else records it.
fn counts_as_failure(outcome: &Restart) -> bool {
    matches!(
        outcome,
        Restart::Failed(FailedPhase::Takeover | FailedPhase::Open)
    )
}

/// Records `event` for the store this program runs from; a program outside a store version folder
/// records nothing.
fn record_fallback(event: FallbackEvent<'_>) {
    if let Some(recorder) = FallbackRecorder::for_current_executable() {
        recorder.record(event);
    }
}

/// The namespace owner, waited for until `deadline`.
fn catalog(base: &Path, root: &DataRoot, deadline: Instant) -> Option<SessionCatalog> {
    loop {
        match SessionCatalog::try_lock(base, root) {
            Ok(catalog) => return Some(catalog),
            Err(error)
                if error.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline =>
            {
                thread::sleep(Duration::from_millis(25));
            }
            Err(_) => return None,
        }
    }
}

/// The Session's activation lock, waited for until `deadline`.
fn activation_lock(
    session: &Rendezvous,
    deadline: Instant,
) -> Result<ModeCheckedSessionLock, Restart> {
    loop {
        match session.try_acquire_activation_lock() {
            Ok(LockAttempt::Acquired(lock)) => return Ok(lock),
            Ok(LockAttempt::Contended) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(25));
            }
            Ok(LockAttempt::Contended) => return Err(Restart::Refused(Refusal::Busy)),
            Err(_) => return Err(Restart::Failed(FailedPhase::Start)),
        }
    }
}

fn entropy() -> Option<[u8; 16]> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).ok()?;
    (bytes != [0; 16]).then_some(bytes)
}

/// Writes `server-job` as an activation does: `outside` for a successor created outside this
/// process's Job, or `inside` with the refused request's error kind, when it shares that Job and
/// ends with it.
#[cfg(windows)]
fn record_server_job(placement: JobPlacement) {
    let (level, fields) = match placement {
        JobPlacement::Outside => (
            LogLevel::Info,
            vec![("outcome", LogValue::Token("outside"))],
        ),
        JobPlacement::Inside(kind) => (
            LogLevel::Warning,
            vec![
                ("outcome", LogValue::Token("inside")),
                ("reason", LogValue::from(kind)),
            ],
        ),
    };
    orchbee_local_runtime::log_event(level, "server-job", &fields);
}
