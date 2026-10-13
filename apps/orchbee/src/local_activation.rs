//! The one app-private local-Site activation algorithm, shared by the TUI
//! startup path and the headless `server start` command. Cohort validation
//! precedes runtime-directory preparation; one absolute caller-supplied
//! deadline bounds the whole attempt and is never reset per phase. The
//! lock/marker/spawn order is identical for both callers; only the failure
//! projection differs.

use crate::local_client::{LocalClient, LocalClientConnectError};
use orchbee_local_client_protocol::{ConnectClientError, RejectionReason, unix_socket_path_fits};
use orchbee_local_site_runtime::{
    LockAttempt, ModeCheckedLocalSiteRendezvous, NativeArtifact, NativeCohort, SiteScope,
};
use std::{io, path::PathBuf, thread, time::Instant};

#[cfg(windows)]
mod windows_selection;
#[cfg(windows)]
pub(crate) use windows_selection::select_site;

/// The single local-startup verdict vocabulary, owned here by the shared
/// activation core so the core never depends on the TUI adapter. The core
/// itself only ever emits `Cohort`, `Runtime`, `SocketPathTooLong`,
/// `RecoveryRequired`, `Incompatible`, `Admission`, or `TimedOut`;
/// `Terminal`, `TerminalUnavailable`, `OutcomeUnknown`, and `Interface` are
/// produced exclusively by the TUI adapter's own terminal stages and live
/// in this one enum so both callers classify the same verdict type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StartupFailure {
    Terminal,
    Cohort,
    Runtime,
    SocketPathTooLong,
    RecoveryRequired,
    Incompatible,
    Admission,
    TimedOut,
    TerminalUnavailable,
    OutcomeUnknown,
    Interface,
    #[cfg(windows)]
    Configuration,
}

/// The shared activation verdict. `failure` is exactly the `StartupFailure`
/// the TUI adapter has always derived; `cause` retains the raw typed error
/// so the CLI can classify precisely — variant alone would collapse
/// protocol-invalid/internal admission arms into a generic refusal.
pub(crate) struct ActivationFailure {
    pub(crate) failure: StartupFailure,
    pub(crate) cause: Option<ActivationCause>,
}

/// The raw cause behind an activation failure, kept for typed CLI
/// classification only — the TUI adapter never reads it.
pub(crate) enum ActivationCause {
    #[cfg(windows)]
    Configuration(orchbee_local_site_runtime::WindowsSettingsError),
    /// `NativeCohort::current` failure: an io error whose kind selects the
    /// CLI exit class.
    Cohort(io::Error),
    /// Runtime-directory preparation failure: the rendezvous error plus the
    /// scope directory the typed path split classifies against.
    Rendezvous(orchbee_local_site_runtime::RendezvousError, PathBuf),
    /// An admission/connect failure in its full wire form.
    Connect(LocalClientConnectError),
    /// Activation-lifetime lock acquisition failure — the rendezvous error
    /// a `try_acquire_*_lock` call carries.
    LockFailure(orchbee_local_site_runtime::RendezvousError),
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
pub(crate) struct StartedSite {
    pub(crate) client: LocalClient,
    pub(crate) spawned: bool,
    /// The scope string parsed for this attempt (`orchbee-native-<euid>`).
    pub(crate) scope: String,
    /// The resolved runtime base (`ORCHBEE_RUNTIME_BASE` or `/tmp`), for
    /// machine-readable output.
    pub(crate) runtime_base: PathBuf,
    /// The admitted canonical runtime, used for per-Site Client preferences.
    #[cfg(windows)]
    pub(crate) runtime_directory: PathBuf,
    #[cfg(windows)]
    pub(crate) restore_notice: Option<orchbee_local_site_runtime::WindowsCheckpointRestoreNotice>,
}

/// Connect-or-activate under one absolute deadline. The sequence is the
/// existing TUI order, unchanged: cohort validation first, then
/// runtime-base/scope resolution, `prepare_mode_checked`, the derived
/// socket-path budget check, then the activation loop.
pub(crate) fn activate_local_site(deadline: Instant) -> Result<StartedSite, ActivationFailure> {
    let cohort = NativeCohort::current(deadline).map_err(|error| {
        ActivationFailure::caused(
            failure(&error, StartupFailure::Cohort),
            ActivationCause::Cohort(error),
        )
    })?;
    #[cfg(windows)]
    let mut selection = windows_selection::location().map_err(runtime_failure)?;
    #[cfg(windows)]
    let (base, scope_text) = (selection.base.clone(), selection.scope.clone());
    #[cfg(not(windows))]
    let (base, scope_text) = runtime_location().map_err(runtime_failure)?;
    let scope = SiteScope::parse(&scope_text)
        .map_err(|_| ActivationFailure::bare(StartupFailure::Runtime))?;
    let scope_directory = base.join(&scope_text);
    #[cfg(windows)]
    let _named_catalog =
        if orchbee_local_site_runtime::WindowsNamedSiteCatalog::supports_scope(&scope_text)
            .map_err(runtime_failure)?
        {
            let local = std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .ok_or_else(|| runtime_failure(io::ErrorKind::NotFound.into()))?;
            loop {
                check_deadline(deadline)?;
                match orchbee_local_site_runtime::WindowsNamedSiteCatalog::try_lock(&base, &local) {
                    Ok(owner) => break Some(owner),
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => pause(deadline)?,
                    Err(error) => return Err(runtime_failure(error)),
                }
            }
        } else {
            None
        };
    #[cfg(windows)]
    if let Some(catalog) = &_named_catalog {
        selection.name = catalog
            .prepare_activation(&scope_text, selection.name.as_ref())
            .map_err(recovery_failure)?;
    }
    let site =
        ModeCheckedLocalSiteRendezvous::prepare_mode_checked(&base, scope).map_err(|error| {
            ActivationFailure::caused(
                StartupFailure::Runtime,
                ActivationCause::Rendezvous(error, scope_directory),
            )
        })?;
    // Every endpoint a Site derives that cannot fit this platform's socket
    // address ABI can never bind or connect; report the derived-path budget
    // before any admission attempt collapses it into a generic refusal.
    if !ModeCheckedLocalSiteRendezvous::socket_budget_paths(site.runtime_directory())
        .iter()
        .all(|path| unix_socket_path_fits(path))
    {
        return Err(ActivationFailure::bare(StartupFailure::SocketPathTooLong));
    }
    #[cfg(windows)]
    if let Some(name) = selection.name {
        loop {
            check_deadline(deadline)?;
            match site.try_acquire_activation_lock().map_err(lock_failure)? {
                LockAttempt::Acquired(lock) => {
                    site.register_session_name(&lock, &name)
                        .map_err(runtime_failure)?;
                    break;
                }
                LockAttempt::Contended => pause(deadline)?,
            }
        }
    } else {
        // An inherited route names an already registered runtime, never an implicit new name.
        ModeCheckedLocalSiteRendezvous::from_runtime_directory(site.runtime_directory())
            .map_err(lock_failure)?;
    }
    let (client, spawned) = connect_or_activate(
        &site,
        &cohort,
        deadline,
        #[cfg(windows)]
        _named_catalog.as_ref(),
    )?;
    Ok(StartedSite {
        client,
        spawned,
        scope: scope_text,
        runtime_base: base,
        #[cfg(windows)]
        runtime_directory: site.runtime_directory().to_owned(),
        #[cfg(windows)]
        restore_notice: site
            .checkpoint_restore_notice(
                &std::env::var_os("LOCALAPPDATA")
                    .map(PathBuf::from)
                    .ok_or_else(|| runtime_failure(io::ErrorKind::NotFound.into()))?,
            )
            .map_err(runtime_failure)?,
    })
}

fn connect_or_activate(
    site: &ModeCheckedLocalSiteRendezvous,
    cohort: &NativeCohort,
    deadline: Instant,
    #[cfg(windows)] catalog: Option<&orchbee_local_site_runtime::WindowsNamedSiteCatalog>,
) -> Result<(LocalClient, bool), ActivationFailure> {
    loop {
        check_deadline(deadline)?;
        if let Some(client) = connect(site, cohort, deadline)? {
            return Ok((client, false));
        }
        let _activation = match site.try_acquire_activation_lock().map_err(lock_failure)? {
            LockAttempt::Acquired(lock) => lock,
            LockAttempt::Contended => {
                pause(deadline)?;
                continue;
            }
        };
        if let Some(client) = connect(site, cohort, deadline)? {
            return Ok((client, false));
        }
        let mut child = match site
            .try_acquire_site_lifetime_lock()
            .map_err(lock_failure)?
        {
            LockAttempt::Acquired(lifetime) => {
                #[cfg(windows)]
                if let Some(catalog) = catalog {
                    catalog
                        .recover_inactive(site, &_activation, &lifetime)
                        .map_err(recovery_failure)?;
                }
                let marker = site.activation_started().map_err(runtime_failure)?;
                if marker
                    || site.client_socket_exists().map_err(runtime_failure)?
                    || site.worker_socket_exists().map_err(runtime_failure)?
                {
                    return Err(ActivationFailure::bare(StartupFailure::RecoveryRequired));
                }
                #[cfg(windows)]
                let settings = orchbee_local_site_runtime::read_windows_user_settings(|name| {
                    std::env::var_os(name)
                })
                .map_err(|error| {
                    ActivationFailure::caused(
                        StartupFailure::Configuration,
                        ActivationCause::Configuration(error),
                    )
                })?;
                #[cfg(windows)]
                if let Some(catalog) = catalog {
                    let mut activation_id = [0; 16];
                    getrandom::fill(&mut activation_id)
                        .map_err(|error| runtime_failure(io::Error::other(error.to_string())))?;
                    catalog
                        .begin_activation(site, activation_id)
                        .map_err(runtime_failure)?;
                }
                drop(lifetime);
                check_deadline(deadline)?;
                site.commit_activation().map_err(runtime_failure)?;
                check_deadline(deadline)?;
                let executable = cohort.executable(NativeArtifact::Server);
                let arguments = [
                    std::ffi::OsStr::new("--local-site"),
                    site.runtime_directory().as_os_str(),
                    #[cfg(windows)]
                    std::ffi::OsStr::new("--resume-agents-on-restore"),
                    #[cfg(windows)]
                    std::ffi::OsStr::new(if settings.resume_agents_on_restore {
                        "true"
                    } else {
                        "false"
                    }),
                    #[cfg(windows)]
                    std::ffi::OsStr::new("--manifest-check"),
                    #[cfg(windows)]
                    std::ffi::OsStr::new(if settings.manifest_check {
                        "true"
                    } else {
                        "false"
                    }),
                    #[cfg(windows)]
                    std::ffi::OsStr::new("--manifest-check-source"),
                    #[cfg(windows)]
                    std::ffi::OsStr::new(settings.manifest_check_source),
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
                    #[cfg(windows)]
                    std::ffi::OsStr::new("--default-shell"),
                    #[cfg(windows)]
                    std::ffi::OsStr::new(settings.shell.executable()),
                    #[cfg(windows)]
                    std::ffi::OsStr::new("--shell-mode"),
                    #[cfg(windows)]
                    std::ffi::OsStr::new(settings.shell.mode().as_str()),
                ];
                #[cfg(windows)]
                let child = match catalog {
                    Some(catalog) => {
                        catalog.spawn_server(site, &_activation, &executable, &arguments, deadline)
                    }
                    None => orchbee_local_site_runtime::spawn_retained(&executable, &arguments),
                };
                #[cfg(not(windows))]
                let child = orchbee_local_site_runtime::spawn_retained(&executable, &arguments);
                Some(child.map_err(|_| {
                    ActivationFailure::caused(
                        StartupFailure::RecoveryRequired,
                        ActivationCause::SpawnIo,
                    )
                })?)
            }
            LockAttempt::Contended => {
                if site.activation_started().map_err(runtime_failure)? {
                    None
                } else {
                    return Err(ActivationFailure::bare(StartupFailure::RecoveryRequired));
                }
            }
        };
        loop {
            check_deadline(deadline)?;
            if let Some(client) = connect(site, cohort, deadline)? {
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
                site.try_acquire_site_lifetime_lock()
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
    site: &ModeCheckedLocalSiteRendezvous,
    cohort: &NativeCohort,
    deadline: Instant,
) -> Result<Option<LocalClient>, ActivationFailure> {
    site.client_socket_exists().map_err(runtime_failure)?;
    match LocalClient::connect_before(site.client_socket_path(), cohort.identity(), deadline) {
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
                LocalClientConnectError::Admission(ConnectClientError::Rejected(
                    RejectionReason::IncompatibleCohort | RejectionReason::IncompatibleProtocol,
                )) => StartupFailure::Incompatible,
                LocalClientConnectError::Admission(
                    ConnectClientError::ConnectFailed(source) | ConnectClientError::Io(source),
                ) => failure(source, StartupFailure::Admission),
                _ => StartupFailure::Admission,
            };
            Err(ActivationFailure::caused(
                failure,
                ActivationCause::Connect(error),
            ))
        }
    }
}

/// Marker/probe/commit I/O keeps the existing `Runtime` verdict; the raw io
/// error rides along for CLI kind classification.
fn runtime_failure(error: io::Error) -> ActivationFailure {
    ActivationFailure::caused(StartupFailure::Runtime, ActivationCause::RuntimeIo(error))
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
fn lock_failure(error: orchbee_local_site_runtime::RendezvousError) -> ActivationFailure {
    ActivationFailure::caused(StartupFailure::Runtime, ActivationCause::LockFailure(error))
}

/// Shared deadline check — the TUI readiness loop consumes the identical
/// verdict via the adapter's `failure` projection.
pub(crate) fn check_deadline(deadline: Instant) -> Result<(), ActivationFailure> {
    if Instant::now() >= deadline {
        return Err(ActivationFailure::bare(StartupFailure::TimedOut));
    }
    Ok(())
}

pub(crate) fn pause(deadline: Instant) -> Result<(), ActivationFailure> {
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
fn failure(error: &io::Error, fallback: StartupFailure) -> StartupFailure {
    if error.kind() == io::ErrorKind::TimedOut {
        StartupFailure::TimedOut
    } else {
        fallback
    }
}

pub(crate) fn runtime_location() -> io::Result<(PathBuf, String)> {
    #[cfg(windows)]
    {
        let location = windows_selection::location()?;
        Ok((location.base, location.scope))
    }
    #[cfg(unix)]
    {
        #[cfg(target_os = "macos")]
        let fallback = PathBuf::from("/private/tmp");
        #[cfg(not(target_os = "macos"))]
        let fallback = PathBuf::from("/tmp");
        let scope = format!("orchbee-native-{}", nix::unistd::geteuid());
        Ok((
            std::env::var_os("ORCHBEE_RUNTIME_BASE")
                .map(PathBuf::from)
                .unwrap_or(fallback),
            scope,
        ))
    }
}
