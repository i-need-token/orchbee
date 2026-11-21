//! The one app-private local-Session activation algorithm, shared by the TUI
//! startup path and the headless `server start` command. Cohort validation
//! precedes runtime-directory preparation; one absolute caller-supplied
//! deadline bounds the whole attempt and is never reset per phase. The
//! lock/marker/spawn order is identical for both callers; only the failure
//! projection differs. On Windows and macOS a release or test Client outside
//! the release store first hands a start to the store's active version (see
//! `store_start`), then runs this same sequence to join. A Session whose Server
//! is ending by a stop is waited out without the activation lock, and the whole
//! sequence, the store's start included, then runs again, so a Client opened
//! during a stop starts the next Session instead of failing.

use crate::local_client::{CLIENT_VERSION, LocalClient, LocalClientConnectError, client_offer};
use crate::startup_status::RefusedVersions;
use orchbee_connection_handshake::RejectionReason as Refusal;
use orchbee_local_client_protocol::{
    ConnectClientError, RejectionReason as ClientRejection, unix_socket_path_fits,
};
use orchbee_local_runtime::{
    Cohort, CohortArtifact, LockAttempt, ModeCheckedLocalSessionRendezvous, SessionScope,
};
use std::{
    io,
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

/// How long an activation that met a Server ending by a stop waits before it looks again. The stop
/// refuses every look until it ends, so a shorter pause would only add work to the ending Server.
const STOPPING_PAUSE: Duration = Duration::from_millis(100);

mod fallback;
mod restore_feedback;
mod session_selection;

#[cfg(any(windows, target_os = "macos"))]
mod store_start;
pub use session_selection::{
    inside_selected_session, select_session, selected_session_name, target_session_name,
};

pub use crate::startup_status::StartupFailure;

/// The shared activation verdict. `failure` is exactly the `StartupFailure`
/// the TUI adapter has always derived; `cause` retains the raw typed error
/// so the CLI can classify precisely — variant alone would collapse
/// protocol-invalid/internal admission arms into a generic refusal.
pub struct ActivationFailure {
    pub failure: StartupFailure,
    pub cause: Option<ActivationCause>,
}

/// The raw cause behind an activation failure, kept for typed CLI
/// classification only — the TUI adapter never reads it.
pub enum ActivationCause {
    #[cfg(any(windows, target_os = "macos"))]
    Configuration(orchbee_local_runtime::SettingsError),
    /// `Cohort::current_with_diagnostic` failure: its I/O kind selects
    /// the CLI exit class and its artifact-open evidence can identify one
    /// exact missing sibling.
    Cohort(orchbee_local_runtime::CohortError),
    /// Runtime-directory preparation failure: the rendezvous error plus the
    /// scope directory the typed path split classifies against.
    Rendezvous(orchbee_local_runtime::RendezvousError, PathBuf),
    /// An admission/connect failure in its full wire form.
    Connect(LocalClientConnectError),
    /// Activation-lifetime lock acquisition failure — the rendezvous error
    /// a `try_acquire_*_lock` call carries.
    LockFailure(orchbee_local_runtime::RendezvousError),
    /// Selection or initial catalog preparation failed before activation effects.
    RuntimeLocation(io::Error),
    /// Marker, socket-presence probe, or marker-commit I/O.
    RuntimeIo(io::Error),
    /// A Session record failure around the Server's creation, or a post-spawn
    /// liveness probe failure — a wedge-class fault: the activation marker may
    /// already be committed, so the verdict is unconditionally
    /// `RecoveryRequired` and the io detail is not classified further. A
    /// creation the system refused is `ServerNotStarted` instead.
    SpawnIo,
}

impl ActivationFailure {
    fn bare(failure: StartupFailure) -> Self {
        Self {
            failure,
            cause: None,
        }
    }

    fn caused(failure: StartupFailure, cause: ActivationCause) -> Self {
        Self {
            failure,
            cause: Some(cause),
        }
    }
}

/// The result of a successful activation attempt: an admitted client plus
/// whether THIS invocation spawned the Server child — an actual branch
/// fact, never inferred from marker state.
pub struct StartedSession {
    pub client: LocalClient,
    pub spawned: bool,
    /// The scope string parsed for this attempt: the selected Session's name.
    pub scope: String,
    /// The resolved runtime base (`ORCHBEE_RUNTIME_BASE` or the environment default), for
    /// machine-readable output.
    pub runtime_base: PathBuf,
    /// The admitted canonical runtime, used for per-Session Client preferences.
    pub runtime_directory: PathBuf,
    pub restore_notice: Option<orchbee_local_runtime::CheckpointRestoreNotice>,
}

/// Connect-or-activate under one absolute deadline. The sequence is the
/// existing TUI order, unchanged: cohort validation first, then
/// runtime-base/scope resolution, `prepare_mode_checked`, the derived
/// socket-path budget check, then the activation loop.
pub fn activate_local_session(deadline: Instant) -> Result<StartedSession, ActivationFailure> {
    activate(
        deadline,
        #[cfg(windows)]
        selected_session_name(),
        #[cfg(windows)]
        session_selection::location,
        #[cfg(windows)]
        ServerJob::Inherit,
        #[cfg(unix)]
        None,
    )
}

/// [`activate_local_session`] for the Session `name` rather than the invocation's selection, as the
/// link helper's `ensure-session` asks.
#[cfg(unix)]
pub fn activate_named_session(
    name: &orchbee_local_runtime::SessionName,
    deadline: Instant,
) -> Result<StartedSession, ActivationFailure> {
    activate(deadline, Some(name))
}

/// [`activate_local_session`] for the Session `name` rather than the invocation's selection, so one
/// process can start several Sessions: the link helper serves one `ensure-session` per channel.
/// The helper runs in the Job Win32-OpenSSH ends with the `ssh` connection, so a Server this
/// starts leaves that Job when the Job allows it and keeps running after the connection ends.
#[cfg(windows)]
pub fn activate_named_session(
    name: &orchbee_local_runtime::SessionName,
    deadline: Instant,
) -> Result<StartedSession, ActivationFailure> {
    activate(
        deadline,
        Some(name.as_str()),
        || session_selection::location_of(Some(name)),
        ServerJob::Leave,
    )
}

/// Whether a Server this activation starts stays in the Job the activating process runs in.
#[cfg(windows)]
#[derive(Clone, Copy)]
enum ServerJob {
    /// It stays, as every local start's Server does.
    Inherit,
    /// It leaves when the Job allows it, and writes where it was created to the daily log.
    Leave,
}

/// The Client endpoint of the existing Session `name` of this user and environment, found as a
/// local Client finds it. A Session whose runtime folder is absent fails with `NotFound`.
pub fn session_client_endpoint(name: &orchbee_local_runtime::SessionName) -> io::Result<PathBuf> {
    let location = session_selection::location_of(Some(name))?;
    let scope = SessionScope::parse(location.scope()).map_err(io::Error::other)?;
    let session = ModeCheckedLocalSessionRendezvous::validate_mode_checked(&location.base, scope)
        .map_err(|error| match error {
        orchbee_local_runtime::RendezvousError::System { source, .. } => source,
        other => io::Error::other(other),
    })?;
    Ok(session.client_socket_path().to_owned())
}

/// `explicit` is the Session name this activation was given, by `--session` or by its caller, which
/// a store start passes on; an inherited or default selection reaches the store's Client through the
/// same environment.
fn activate(
    deadline: Instant,
    #[cfg(windows)] explicit: Option<&str>,
    #[cfg(windows)] locate: impl FnOnce() -> io::Result<session_selection::Location>,
    #[cfg(windows)] server_job: ServerJob,
    #[cfg(unix)] named: Option<&orchbee_local_runtime::SessionName>,
) -> Result<StartedSession, ActivationFailure> {
    let cohort = Cohort::current_with_diagnostic(deadline).map_err(|error| {
        // A damaged store version would otherwise fail here on every start without counting, and
        // stay active; recorded only from a store version folder while its move counts.
        let mut activation = [0; 16];
        if fallback::damaged(error.kind()) && getrandom::fill(&mut activation).is_ok() {
            fallback::observe(&activation, fallback::Seen::Damaged, fallback::record);
        }
        ActivationFailure::caused(
            failure(error.kind(), StartupFailure::Cohort),
            ActivationCause::Cohort(error),
        )
    })?;
    #[cfg(windows)]
    let selection = locate().map_err(runtime_location_failure)?;
    // The default base lives in the data root, which may not exist yet; an explicit
    // `ORCHBEE_RUNTIME_BASE` must already exist, as before.
    #[cfg(windows)]
    if selection.default_base {
        orchbee_local_runtime::prepare_default_runtime_base(|name| std::env::var_os(name))
            .map_err(runtime_location_failure)?;
    }
    #[cfg(windows)]
    let (base, scope_text) = (selection.base.clone(), selection.scope().to_owned());
    // The default base lives in the data root, which may not exist yet; it is created, admitted,
    // and named through the data root's canonical path. An explicit `ORCHBEE_RUNTIME_BASE` must
    // already exist.
    #[cfg(unix)]
    let (base, scope_text, selected) = {
        let session_selection::Location {
            base,
            default_base,
            name,
        } = named
            .map_or_else(session_selection::location, |name| {
                session_selection::location_of(Some(name))
            })
            .map_err(runtime_location_failure)?;
        let base = if default_base {
            orchbee_local_runtime::prepare_default_runtime_base(|name| std::env::var_os(name))
                .map_err(runtime_location_failure)?
        } else {
            base
        };
        (base, name.as_str().to_owned(), name)
    };
    #[cfg(windows)]
    let name = &selection.name;
    #[cfg(unix)]
    let name = &selected;
    // The name given by `--session` or by the caller, which a store start passes on as on Windows.
    #[cfg(target_os = "macos")]
    let explicit = match named {
        Some(name) => Some(name.as_str()),
        None => selected_session_name(),
    };
    let scope = SessionScope::parse(&scope_text)
        .map_err(|_| ActivationFailure::bare(StartupFailure::Runtime))?;
    let scope_directory = base.join(&scope_text);
    // Each pass ends joined, failed, or at a Server ending by a stop, which the next pass outlasts.
    loop {
        // A start the store's Client takes runs before the catalog lock, which that Client's own
        // activation takes in turn; this activation then joins its Session as any other.
        #[cfg(any(windows, target_os = "macos"))]
        let started_by_store = store_start::start_if_absent(
            &base,
            &scope_text,
            explicit,
            &cohort.executable(CohortArtifact::Client),
            deadline,
        )?;
        // Every Session is one catalog entry, named by its folders.
        let catalog = {
            let root = orchbee_local_runtime::DataRoot::resolve(|name| std::env::var_os(name))
                .map_err(|_| runtime_location_failure(io::ErrorKind::NotFound.into()))?;
            let catalog = loop {
                check_deadline(deadline)?;
                match orchbee_local_runtime::SessionCatalog::try_lock(&base, &root) {
                    Ok(owner) => break owner,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => pause(deadline)?,
                    Err(error) => return Err(runtime_location_failure(error)),
                }
            };
            catalog.prepare_activation(name).map_err(recovery_failure)?;
            catalog
        };
        let session = ModeCheckedLocalSessionRendezvous::prepare_mode_checked(&base, scope.clone())
            .map_err(|error| {
                ActivationFailure::caused(
                    runtime_verdict(error.refusal()),
                    ActivationCause::Rendezvous(error, scope_directory.clone()),
                )
            })?;
        // Every endpoint a Session derives that cannot fit this platform's socket
        // address ABI can never bind or connect; report the derived-path budget
        // before any admission attempt collapses it into a generic refusal.
        if !ModeCheckedLocalSessionRendezvous::socket_budget_paths(session.runtime_directory())
            .iter()
            .all(|path| unix_socket_path_fits(path))
        {
            return Err(ActivationFailure::bare(StartupFailure::SocketPathTooLong));
        }
        let (client, spawned) = match connect_or_activate(
            &session,
            &cohort,
            deadline,
            &catalog,
            #[cfg(windows)]
            server_job,
        )? {
            Activated::Joined(client, spawned) => (client, spawned),
            // The catalog and the Session's folder handles go too, and the next pass starts with
            // the store's start, so the next Session runs the store's active version.
            Activated::Stopping => {
                drop((catalog, session));
                wait(deadline, STOPPING_PAUSE)?;
                continue;
            }
        };
        #[cfg(any(windows, target_os = "macos"))]
        let spawned = spawned || started_by_store;
        // Reading the notice leaves it in place, so a Client that joins a Session the store's Client
        // started reads the same notice that Client reported.
        let restore_notice = match restore_feedback::read(&session) {
            Ok(notice) => notice.map_err(runtime_failure)?,
            Err(restore_feedback::Unsupported) => None,
        };
        return Ok(StartedSession {
            client,
            spawned,
            scope: scope_text,
            runtime_base: base,
            runtime_directory: session.runtime_directory().to_owned(),
            restore_notice,
        });
    }
}

/// How one pass of [`connect_or_activate`] ended.
enum Activated {
    /// An admitted Client, and whether this pass spawned the Server.
    Joined(LocalClient, bool),
    /// A Server this pass did not start is ending by a stop; the pass let go of the activation
    /// lock so the stop can complete.
    Stopping,
}

/// What one connection attempt reached.
enum Reached {
    Client(LocalClient),
    /// No Server listens.
    Absent,
    /// A Server listens but is ending: a recorded Session stop refuses every Client until the
    /// Server exits, and an exiting Server refuses every offer.
    Stopping,
}

fn connect_or_activate(
    session: &ModeCheckedLocalSessionRendezvous,
    cohort: &Cohort,
    deadline: Instant,
    catalog: &orchbee_local_runtime::SessionCatalog,
    #[cfg(windows)] server_job: ServerJob,
) -> Result<Activated, ActivationFailure> {
    loop {
        check_deadline(deadline)?;
        match connect(session, cohort, deadline)? {
            Reached::Client(client) => return Ok(Activated::Joined(client, false)),
            Reached::Stopping => return Ok(Activated::Stopping),
            Reached::Absent => {}
        }
        let _activation = match session
            .try_acquire_activation_lock()
            .map_err(lock_failure)?
        {
            LockAttempt::Acquired(lock) => lock,
            LockAttempt::Contended => {
                pause(deadline)?;
                continue;
            }
        };
        // A stopping Server takes the activation lock to complete its stop, so this pass never
        // waits for one while holding it.
        match connect(session, cohort, deadline)? {
            Reached::Client(client) => return Ok(Activated::Joined(client, false)),
            Reached::Stopping => return Ok(Activated::Stopping),
            Reached::Absent => {}
        }
        // The generation this attempt publishes, whose failure to start the fallback counts.
        let mut generation = None;
        // An observer probes the lifetime lock for an instant without the activation lock; only a
        // lock still held past that has an owner.
        let mut child = match orchbee_local_runtime::acquire_past_probes(|| {
            session.try_acquire_session_lifetime_lock()
        })
        .map_err(lock_failure)?
        {
            LockAttempt::Acquired(lifetime) => {
                recover_inactive(catalog, session, &_activation, &lifetime, deadline)?;
                let marker = session.activation_started().map_err(runtime_failure)?;
                // A POSIX socket file outlives its listener, so only the marker refuses there; the
                // Server removes stale sockets under the lifetime lock before it binds.
                #[cfg(windows)]
                let refused = marker
                    || session.client_socket_exists().map_err(runtime_failure)?
                    || session.worker_socket_exists().map_err(runtime_failure)?;
                #[cfg(unix)]
                let refused = marker;
                if refused {
                    return Err(ActivationFailure::bare(StartupFailure::RecoveryRequired));
                }
                // Activation refuses only a location it cannot admit, and freezes it: a file that
                // cannot be read still starts the Session, so the person has a terminal to repair
                // it. The Server reads that location itself as the single configuration authority.
                #[cfg(any(windows, target_os = "macos"))]
                let settings =
                    orchbee_local_runtime::read_startup_settings(|name| std::env::var_os(name))
                        .map_err(|error| {
                            ActivationFailure::caused(
                                StartupFailure::Configuration,
                                ActivationCause::Configuration(error),
                            )
                        })?;
                // The Server starts its Worker under the data root and exits without one, so an
                // environment that names no data root is a configuration failure here, before the
                // marker commits, rather than a Server exit that reads as a wedge.
                #[cfg(windows)]
                if let Err(error) =
                    orchbee_local_runtime::DataRoot::resolve(|name| std::env::var_os(name))
                {
                    return Err(ActivationFailure::caused(
                        StartupFailure::Configuration,
                        ActivationCause::Configuration(orchbee_local_runtime::SettingsError::new(
                            error.variable,
                            error.reason,
                        )),
                    ));
                }
                let mut activation_id = [0; 16];
                getrandom::fill(&mut activation_id)
                    .map_err(|error| runtime_failure(io::Error::other(error.to_string())))?;
                catalog
                    .begin_activation(session, activation_id)
                    .map_err(runtime_failure)?;
                drop(lifetime);
                check_deadline(deadline)?;
                session.commit_activation().map_err(runtime_failure)?;
                check_deadline(deadline)?;
                // Recorded only now, so a generation that never starts its Server counts nothing
                // when recovery later proves it dead.
                fallback::observe(&activation_id, fallback::Seen::Started, fallback::record);
                generation = Some(activation_id);
                let executable = cohort.executable(CohortArtifact::Server);
                let arguments = [
                    std::ffi::OsStr::new("--runtime-directory"),
                    session.runtime_directory().as_os_str(),
                    #[cfg(any(windows, target_os = "macos"))]
                    std::ffi::OsStr::new("--configuration-file"),
                    #[cfg(any(windows, target_os = "macos"))]
                    settings
                        .path
                        .as_deref()
                        .map(std::path::Path::as_os_str)
                        .unwrap_or(std::ffi::OsStr::new("")),
                    #[cfg(any(windows, target_os = "macos"))]
                    std::ffi::OsStr::new("--configuration-file-explicit"),
                    #[cfg(any(windows, target_os = "macos"))]
                    std::ffi::OsStr::new(if settings.explicit { "true" } else { "false" }),
                ];
                #[cfg(windows)]
                let child = match server_job {
                    ServerJob::Inherit => catalog.spawn_server(
                        session,
                        &_activation,
                        &executable,
                        &arguments,
                        deadline,
                    ),
                    ServerJob::Leave => catalog
                        .spawn_server_outside_job(
                            session,
                            &_activation,
                            &executable,
                            &arguments,
                            deadline,
                        )
                        .map(|(child, placement)| {
                            record_server_job(placement);
                            child
                        }),
                };
                // The Session's terminals inherit a `PATH` that ends with this cohort's folder.
                #[cfg(not(windows))]
                let child = orchbee_local_runtime::spawn_session_server(&executable, &arguments);
                Some(child.map_err(|error| {
                    fallback::observe(
                        &activation_id,
                        fallback::Seen::FailedToStart,
                        fallback::record,
                    );
                    spawn_failure(&error)
                })?)
            }
            // A POSIX Server that holds the lifetime lock without an active marker is completing
            // its stop; the wait below starts again once it releases the lock.
            LockAttempt::Contended => {
                #[cfg(windows)]
                if !session.activation_started().map_err(runtime_failure)? {
                    return Err(ActivationFailure::bare(StartupFailure::RecoveryRequired));
                }
                None
            }
        };
        loop {
            check_deadline(deadline)?;
            match connect(session, cohort, deadline)? {
                Reached::Client(client) => {
                    return Ok(Activated::Joined(client, child.is_some()));
                }
                // The stop of a Server this pass did not start needs the activation lock to
                // complete, so the pass lets go of it and starts again.
                Reached::Stopping if child.is_none() => return Ok(Activated::Stopping),
                Reached::Stopping | Reached::Absent => {}
            }
            if let Some(child) = child.as_mut() {
                if child
                    .try_wait()
                    .map_err(|_| {
                        ActivationFailure::caused(
                            StartupFailure::RecoveryRequired,
                            ActivationCause::SpawnIo,
                        )
                    })?
                    .is_some()
                {
                    if let Some(activation_id) = &generation {
                        fallback::observe(
                            activation_id,
                            fallback::Seen::FailedToStart,
                            fallback::record,
                        );
                    }
                    return Err(ActivationFailure::bare(StartupFailure::RecoveryRequired));
                }
            } else if matches!(
                session
                    .try_acquire_session_lifetime_lock()
                    .map_err(lock_failure)?,
                LockAttempt::Acquired(_)
            ) {
                // A POSIX Server that ended, by its stop or a signal it could not handle, left a
                // stopped or recoverable Session, which this activation starts again from the top.
                #[cfg(unix)]
                break;
                #[cfg(windows)]
                return Err(ActivationFailure::bare(StartupFailure::RecoveryRequired));
            }
            pause(deadline)?;
        }
    }
}

/// How long a POSIX activation waits for the Worker and Host of a Server that just ended to
/// follow it before the Host's lock refuses recovery.
#[cfg(unix)]
const HOST_EXIT_GRACE: std::time::Duration = std::time::Duration::from_secs(3);

/// Recovers a Session whose Server ended without its stop. A POSIX Worker ends once its Server wire
/// does, and a Host that retains no terminal once its Worker leaves, so a Host lock that frees
/// within [`HOST_EXIT_GRACE`] still admits recovery; one that stays held belongs to a Host that
/// keeps terminals running.
#[cfg(unix)]
fn recover_inactive(
    catalog: &orchbee_local_runtime::SessionCatalog,
    session: &ModeCheckedLocalSessionRendezvous,
    activation: &orchbee_local_runtime::ModeCheckedSessionLock,
    lifetime: &orchbee_local_runtime::ModeCheckedSessionLock,
    deadline: Instant,
) -> Result<(), ActivationFailure> {
    let grace = Instant::now() + HOST_EXIT_GRACE;
    loop {
        match catalog.recover_inactive(session, activation, lifetime) {
            Ok(recovered) => {
                if let Some(activation_id) = &recovered {
                    fallback::observe(activation_id, fallback::Seen::Crashed, fallback::record);
                }
                return Ok(());
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock && Instant::now() < grace => {
                pause(deadline)?;
            }
            Err(error) => return Err(recovery_failure(error)),
        }
    }
}

/// Recovers a Session whose Server ended without its stop once every registered process is gone.
#[cfg(windows)]
fn recover_inactive(
    catalog: &orchbee_local_runtime::SessionCatalog,
    session: &ModeCheckedLocalSessionRendezvous,
    activation: &orchbee_local_runtime::ModeCheckedSessionLock,
    lifetime: &orchbee_local_runtime::ModeCheckedSessionLock,
    _deadline: Instant,
) -> Result<(), ActivationFailure> {
    let recovered = catalog
        .recover_inactive(session, activation, lifetime)
        .map_err(recovery_failure)?;
    if let Some(activation_id) = &recovered {
        fallback::observe(activation_id, fallback::Seen::Crashed, fallback::record);
    }
    Ok(())
}

/// Writes `server-job`: `outcome` `outside` for a Server created outside the activating process's
/// Job, or `inside` with the refused request's error kind as `reason`, when the Server shares that
/// Job and ends with it.
#[cfg(windows)]
fn record_server_job(placement: orchbee_local_runtime::JobPlacement) {
    use orchbee_local_runtime::{JobPlacement, LogLevel, LogValue};
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

fn connect(
    session: &ModeCheckedLocalSessionRendezvous,
    cohort: &Cohort,
    deadline: Instant,
) -> Result<Reached, ActivationFailure> {
    session.client_socket_exists().map_err(runtime_failure)?;
    let offer = client_offer(Some(cohort.identity()));
    match LocalClient::connect_before(session.client_socket_path(), &offer, deadline) {
        Ok(client) => Ok(Reached::Client(client)),
        Err(error) => {
            // An absent or refusing socket inside the activation loop is
            // retryable; every other connect failure surfaces verbatim.
            if let LocalClientConnectError::Admission(ConnectClientError::ConnectFailed(source)) =
                &error
                && matches!(
                    source.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
                )
            {
                return Ok(Reached::Absent);
            }
            if let LocalClientConnectError::Admission(admission) = &error
                && stopping(admission)
            {
                return Ok(Reached::Stopping);
            }
            let failure = match &error {
                LocalClientConnectError::Admission(error) => match refusal(error) {
                    Some(failure) => failure,
                    None => admission_failure(error),
                },
                LocalClientConnectError::RequestIoConfiguration(_) => StartupFailure::Admission,
            };
            Err(ActivationFailure::caused(
                failure,
                ActivationCause::Connect(error),
            ))
        }
    }
}

/// Whether the Server refused because it is ending: a recorded Session stop refuses the Client phase
/// and an exiting Server refuses the offer, both with `stopping`.
fn stopping(error: &ConnectClientError) -> bool {
    match error {
        ConnectClientError::Rejected(reason) => *reason == ClientRejection::Stopping,
        ConnectClientError::Refused(rejection) => rejection.reason == Refusal::Stopping,
        _ => false,
    }
}

/// The verdict for a handshake refusal, or `None` for any other admission failure. A Session that
/// predates the handshake is the older side.
pub fn refusal(error: &ConnectClientError) -> Option<StartupFailure> {
    let versions = |session: Option<&str>| {
        Box::new(RefusedVersions {
            client: CLIENT_VERSION.to_owned(),
            session: session.map(str::to_owned),
        })
    };
    match error {
        ConnectClientError::PredatesHandshake => {
            Some(StartupFailure::SessionTooOld(versions(None)))
        }
        ConnectClientError::Refused(rejection) => {
            let session = Some(rejection.product_version.as_str());
            match rejection.reason {
                Refusal::InitiatorTooOld => Some(StartupFailure::ClientTooOld(versions(session))),
                Refusal::InitiatorTooNew => Some(StartupFailure::SessionTooOld(versions(session))),
                Refusal::EnvironmentMismatch => Some(StartupFailure::OtherEnvironment),
                Refusal::FingerprintMismatch => Some(StartupFailure::DevelopmentBuildMismatch),
                _ => None,
            }
        }
        _ => None,
    }
}

fn admission_failure(error: &ConnectClientError) -> StartupFailure {
    if error.refused_by_elevated_session() {
        return StartupFailure::ElevatedSession;
    }
    match error {
        ConnectClientError::ConnectFailed(source) | ConnectClientError::Io(source) => {
            failure(source.kind(), StartupFailure::Admission)
        }
        // Only the explicitly recognized protocol failures have a more specific transport classification here.
        _ => StartupFailure::Admission,
    }
}

/// Marker/probe/commit I/O keeps the existing `Runtime` verdict; the raw io
/// error rides along for CLI kind classification.
fn runtime_failure(error: io::Error) -> ActivationFailure {
    ActivationFailure::caused(
        runtime_verdict(orchbee_local_runtime::AdmissionRefusal::of(&error).cloned()),
        ActivationCause::RuntimeIo(error),
    )
}

fn runtime_location_failure(error: io::Error) -> ActivationFailure {
    ActivationFailure::caused(
        runtime_verdict(orchbee_local_runtime::AdmissionRefusal::of(&error).cloned()),
        ActivationCause::RuntimeLocation(error),
    )
}

/// A folder or file refused by the ownership, permission, link, or file-lock rules is reported with
/// the item at fault; every other runtime failure keeps the `Runtime` verdict.
fn runtime_verdict(refusal: Option<orchbee_local_runtime::AdmissionRefusal>) -> StartupFailure {
    refusal.map_or(StartupFailure::Runtime, |refusal| {
        StartupFailure::Refused(Box::new(refusal))
    })
}

/// A recovery refused because an earlier start's process still runs names that process; any other
/// refusal keeps the recovery verdict.
fn recovery_failure(error: io::Error) -> ActivationFailure {
    if let Some(blocker) = orchbee_local_runtime::RecoveryBlocker::of(&error) {
        return ActivationFailure::bare(StartupFailure::RecoveryBlocked(blocker));
    }
    match error.kind() {
        io::ErrorKind::InvalidData | io::ErrorKind::WouldBlock => {
            ActivationFailure::bare(StartupFailure::RecoveryRequired)
        }
        _ => runtime_failure(error),
    }
}

/// A Server the system would not create keeps the system's answer, so the person sees what was
/// refused, such as a missing or blocked program file. Nothing ran, so the next start recovers by
/// itself. On Windows only the creation step is the system's answer; a failure in the Session
/// record around it keeps the recovery verdict.
fn spawn_failure(error: &io::Error) -> ActivationFailure {
    #[cfg(windows)]
    if orchbee_local_runtime::SessionChildStep::of(error)
        != Some(orchbee_local_runtime::SessionChildStep::Creation)
    {
        return ActivationFailure::caused(
            StartupFailure::RecoveryRequired,
            ActivationCause::SpawnIo,
        );
    }
    ActivationFailure::bare(StartupFailure::ServerNotStarted(
        crate::startup_status::ServerSpawnFailure {
            kind: error.kind(),
            os_error: os_error(error),
        },
    ))
}

/// The first operating-system error code in `error` or the errors it wraps.
fn os_error(error: &io::Error) -> Option<i32> {
    let mut current: &(dyn std::error::Error + 'static) = error;
    loop {
        current = match current.downcast_ref::<io::Error>() {
            Some(error) => match (error.raw_os_error(), error.get_ref()) {
                (Some(code), _) => return Some(code),
                // A wrapping I/O error's own source skips the error it wraps.
                (None, Some(wrapped)) => wrapped,
                (None, None) => return None,
            },
            None => current.source()?,
        };
    }
}

/// A lock acquisition keeps the existing `Runtime` verdict; the raw
/// rendezvous error rides along for CLI kind classification.
fn lock_failure(error: orchbee_local_runtime::RendezvousError) -> ActivationFailure {
    ActivationFailure::caused(
        runtime_verdict(error.refusal()),
        ActivationCause::LockFailure(error),
    )
}

/// Shared deadline check — the TUI readiness loop consumes the identical
/// verdict via the adapter's `failure` projection.
pub fn check_deadline(deadline: Instant) -> Result<(), ActivationFailure> {
    if Instant::now() >= deadline {
        return Err(ActivationFailure::bare(StartupFailure::TimedOut));
    }
    Ok(())
}

pub fn pause(deadline: Instant) -> Result<(), ActivationFailure> {
    wait(deadline, Duration::from_millis(10))
}

/// Sleeps `duration`, or until `deadline` when that comes first.
fn wait(deadline: Instant, duration: Duration) -> Result<(), ActivationFailure> {
    check_deadline(deadline)?;
    thread::sleep(duration.min(deadline.saturating_duration_since(Instant::now())));
    Ok(())
}

/// The existing deadline-sensitive fallback: a TimedOut io kind means the
/// operation expired against the shared budget rather than failing its own
/// class.
fn failure(kind: io::ErrorKind, fallback: StartupFailure) -> StartupFailure {
    if kind == io::ErrorKind::TimedOut {
        StartupFailure::TimedOut
    } else {
        fallback
    }
}

/// Whether the runtime base is this environment's default, `internal/runtime` in its data root, and
/// nothing has created it yet; activation creates it. Read-only commands then report an unstarted
/// environment instead of failing. An explicit base never qualifies.
pub fn runtime_base_unstarted() -> io::Result<bool> {
    let location = session_selection::location()?;
    Ok(location.default_base
        && matches!(
            std::fs::symlink_metadata(&location.base),
            Err(error) if error.kind() == io::ErrorKind::NotFound
        ))
}

pub fn runtime_location() -> io::Result<(PathBuf, String)> {
    let location = session_selection::location()?;
    let scope = location.scope().to_owned();
    Ok((location.base, scope))
}
