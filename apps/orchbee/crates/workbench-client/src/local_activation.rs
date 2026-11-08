//! The one app-private local-Session activation algorithm, shared by the TUI
//! startup path and the headless `server start` command. Cohort validation
//! precedes runtime-directory preparation; one absolute caller-supplied
//! deadline bounds the whole attempt and is never reset per phase. The
//! lock/marker/spawn order is identical for both callers; only the failure
//! projection differs.

use crate::local_client::{CLIENT_VERSION, LocalClient, LocalClientConnectError, client_offer};
use crate::startup_status::RefusedVersions;
use orchbee_connection_handshake::RejectionReason as Refusal;
use orchbee_local_client_protocol::{ConnectClientError, unix_socket_path_fits};
use orchbee_local_runtime::{
    Cohort, CohortArtifact, LockAttempt, ModeCheckedLocalSessionRendezvous, SessionScope,
};
use std::{io, path::PathBuf, thread, time::Instant};

mod restore_feedback;

#[cfg(windows)]
mod windows_selection;
#[cfg(windows)]
pub use windows_selection::inside_selected_session;
#[cfg(windows)]
pub use windows_selection::{select_session, selected_session_name, target_session_name};
#[cfg(not(windows))]
pub fn selected_session_name() -> Option<&'static str> {
    None
}

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
    #[cfg(windows)]
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
    /// Server spawn or post-spawn liveness probe failure — a wedge-class
    /// fault: the activation marker may already be committed, so the verdict
    /// is unconditionally `RecoveryRequired` and the io detail is not
    /// classified further.
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
    /// The scope string parsed for this attempt (`orchbee-native-<euid>`).
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
    let cohort = Cohort::current_with_diagnostic(deadline).map_err(|error| {
        ActivationFailure::caused(
            failure(error.kind(), StartupFailure::Cohort),
            ActivationCause::Cohort(error),
        )
    })?;
    #[cfg(windows)]
    let selection = windows_selection::location().map_err(runtime_location_failure)?;
    // The default base lives in the data root, which may not exist yet; an explicit
    // `ORCHBEE_RUNTIME_BASE` must already exist, as before.
    #[cfg(windows)]
    if selection.default_base {
        orchbee_local_runtime::prepare_default_runtime_base(|name| std::env::var_os(name))
            .map_err(runtime_location_failure)?;
    }
    #[cfg(windows)]
    let (base, scope_text) = (selection.base.clone(), selection.scope().to_owned());
    #[cfg(not(windows))]
    let (base, scope_text) = runtime_location().map_err(runtime_location_failure)?;
    let scope = SessionScope::parse(&scope_text)
        .map_err(|_| ActivationFailure::bare(StartupFailure::Runtime))?;
    let scope_directory = base.join(&scope_text);
    // Every Windows Session is one catalog entry, named by its folders.
    #[cfg(windows)]
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
        catalog
            .prepare_activation(&selection.name)
            .map_err(recovery_failure)?;
        catalog
    };
    let session =
        ModeCheckedLocalSessionRendezvous::prepare_mode_checked(&base, scope).map_err(|error| {
            ActivationFailure::caused(
                StartupFailure::Runtime,
                ActivationCause::Rendezvous(error, scope_directory),
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
    let (client, spawned) = connect_or_activate(
        &session,
        &cohort,
        deadline,
        #[cfg(windows)]
        &catalog,
    )?;
    let restore_notice = match restore_feedback::read(&session) {
        Ok(notice) => notice.map_err(runtime_failure)?,
        Err(restore_feedback::Unsupported) => None,
    };
    Ok(StartedSession {
        client,
        spawned,
        scope: scope_text,
        runtime_base: base,
        runtime_directory: session.runtime_directory().to_owned(),
        restore_notice,
    })
}

fn connect_or_activate(
    session: &ModeCheckedLocalSessionRendezvous,
    cohort: &Cohort,
    deadline: Instant,
    #[cfg(windows)] catalog: &orchbee_local_runtime::SessionCatalog,
) -> Result<(LocalClient, bool), ActivationFailure> {
    loop {
        check_deadline(deadline)?;
        if let Some(client) = connect(session, cohort, deadline)? {
            return Ok((client, false));
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
        if let Some(client) = connect(session, cohort, deadline)? {
            return Ok((client, false));
        }
        let mut child = match session
            .try_acquire_session_lifetime_lock()
            .map_err(lock_failure)?
        {
            LockAttempt::Acquired(lifetime) => {
                #[cfg(windows)]
                catalog
                    .recover_inactive(session, &_activation, &lifetime)
                    .map_err(recovery_failure)?;
                let marker = session.activation_started().map_err(runtime_failure)?;
                if marker
                    || session.client_socket_exists().map_err(runtime_failure)?
                    || session.worker_socket_exists().map_err(runtime_failure)?
                {
                    return Err(ActivationFailure::bare(StartupFailure::RecoveryRequired));
                }
                // Activation rejects an unreadable file and freezes its location; the Server
                // reads that location itself as the single configuration authority.
                #[cfg(windows)]
                let settings =
                    orchbee_local_runtime::read_user_settings(|name| std::env::var_os(name))
                        .map_err(|error| {
                            ActivationFailure::caused(
                                StartupFailure::Configuration,
                                ActivationCause::Configuration(error),
                            )
                        })?;
                // The Server freezes its preference scope from the data root and exits without
                // one, so an environment that names no data root is a configuration failure here,
                // before the marker commits, rather than a Server exit that reads as a wedge.
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
                #[cfg(windows)]
                {
                    let mut activation_id = [0; 16];
                    getrandom::fill(&mut activation_id)
                        .map_err(|error| runtime_failure(io::Error::other(error.to_string())))?;
                    catalog
                        .begin_activation(session, activation_id)
                        .map_err(runtime_failure)?;
                }
                drop(lifetime);
                check_deadline(deadline)?;
                session.commit_activation().map_err(runtime_failure)?;
                check_deadline(deadline)?;
                let executable = cohort.executable(CohortArtifact::Server);
                let arguments = [
                    std::ffi::OsStr::new("--runtime-directory"),
                    session.runtime_directory().as_os_str(),
                    #[cfg(windows)]
                    std::ffi::OsStr::new("--configuration-file"),
                    #[cfg(windows)]
                    settings
                        .path
                        .as_deref()
                        .map(std::path::Path::as_os_str)
                        .unwrap_or(std::ffi::OsStr::new("")),
                    #[cfg(windows)]
                    std::ffi::OsStr::new("--configuration-file-explicit"),
                    #[cfg(windows)]
                    std::ffi::OsStr::new(if settings.explicit { "true" } else { "false" }),
                ];
                #[cfg(windows)]
                let child =
                    catalog.spawn_server(session, &_activation, &executable, &arguments, deadline);
                #[cfg(not(windows))]
                let child = orchbee_local_runtime::spawn_retained(&executable, &arguments);
                Some(child.map_err(|_| {
                    ActivationFailure::caused(
                        StartupFailure::RecoveryRequired,
                        ActivationCause::SpawnIo,
                    )
                })?)
            }
            LockAttempt::Contended => {
                if session.activation_started().map_err(runtime_failure)? {
                    None
                } else {
                    return Err(ActivationFailure::bare(StartupFailure::RecoveryRequired));
                }
            }
        };
        loop {
            check_deadline(deadline)?;
            if let Some(client) = connect(session, cohort, deadline)? {
                return Ok((client, child.is_some()));
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
                    return Err(ActivationFailure::bare(StartupFailure::RecoveryRequired));
                }
            } else if matches!(
                session
                    .try_acquire_session_lifetime_lock()
                    .map_err(lock_failure)?,
                LockAttempt::Acquired(_)
            ) {
                return Err(ActivationFailure::bare(StartupFailure::RecoveryRequired));
            }
            pause(deadline)?;
        }
    }
}

fn connect(
    session: &ModeCheckedLocalSessionRendezvous,
    cohort: &Cohort,
    deadline: Instant,
) -> Result<Option<LocalClient>, ActivationFailure> {
    session.client_socket_exists().map_err(runtime_failure)?;
    let offer = client_offer(Some(cohort.identity()));
    match LocalClient::connect_before(session.client_socket_path(), &offer, deadline) {
        Ok(client) => Ok(Some(client)),
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
                return Ok(None);
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
    ActivationFailure::caused(StartupFailure::Runtime, ActivationCause::RuntimeIo(error))
}

fn runtime_location_failure(error: io::Error) -> ActivationFailure {
    ActivationFailure::caused(
        StartupFailure::Runtime,
        ActivationCause::RuntimeLocation(error),
    )
}

#[cfg(windows)]
fn recovery_failure(error: io::Error) -> ActivationFailure {
    match error.kind() {
        io::ErrorKind::InvalidData | io::ErrorKind::WouldBlock => {
            ActivationFailure::bare(StartupFailure::RecoveryRequired)
        }
        _ => runtime_failure(error),
    }
}

/// A lock acquisition keeps the existing `Runtime` verdict; the raw
/// rendezvous error rides along for CLI kind classification.
fn lock_failure(error: orchbee_local_runtime::RendezvousError) -> ActivationFailure {
    ActivationFailure::caused(StartupFailure::Runtime, ActivationCause::LockFailure(error))
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
    check_deadline(deadline)?;
    thread::sleep(
        std::time::Duration::from_millis(10)
            .min(deadline.saturating_duration_since(Instant::now())),
    );
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
    #[cfg(windows)]
    {
        let location = windows_selection::location()?;
        Ok(location.default_base
            && matches!(
                std::fs::symlink_metadata(&location.base),
                Err(error) if error.kind() == io::ErrorKind::NotFound
            ))
    }
    #[cfg(unix)]
    {
        Ok(false)
    }
}

pub fn runtime_location() -> io::Result<(PathBuf, String)> {
    #[cfg(windows)]
    {
        let location = windows_selection::location()?;
        let scope = location.scope().to_owned();
        Ok((location.base, scope))
    }
    #[cfg(unix)]
    {
        let environment =
            orchbee_local_runtime::session_route_environment(|key| std::env::var_os(key));
        let fallback = orchbee_local_runtime::default_runtime_base(|key| std::env::var_os(key))
            .ok_or(io::ErrorKind::NotFound)?;
        let scope = format!("orchbee-native-{}", nix::unistd::geteuid());
        Ok((
            environment("ORCHBEE_RUNTIME_BASE")
                .map(PathBuf::from)
                .unwrap_or(fallback),
            scope,
        ))
    }
}
