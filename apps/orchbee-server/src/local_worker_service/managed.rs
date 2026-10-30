//! Composes admitted runtime locks, verified cohort artifacts, retained Worker startup, Client
//! service, and checkpoint lifecycle for a managed Session. It does not define domain
//! transitions or adopt an unregistered process as the Session owner.

#[cfg(not(windows))]
use crate::local_client_service;
use crate::{
    agent_service::{AgentPolicy, AgentService},
    local_terminal_service::LocalTerminalService,
    worker_registry::WorkerRegistry,
};
use orchbee_local_ipc::LocalListener as UnixListener;
use orchbee_local_runtime::{
    Cohort, CohortArtifact, LockAttempt, ModeCheckedLocalSessionRendezvous,
};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    io,
    path::Path,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
#[cfg(windows)]
mod checkpoint_windows;
#[cfg(windows)]
mod stop_windows;

pub(super) fn run(
    runtime: &Path,
    #[cfg(windows)] configuration: orchbee_local_runtime::UserSettingsLoad,
) -> io::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(20);
    let cohort = Cohort::current(deadline)?;
    let session = ModeCheckedLocalSessionRendezvous::from_runtime_directory(runtime)
        .map_err(io::Error::other)?;
    if !session.activation_started()? {
        return Err(io::Error::other("Session activation was not committed"));
    }
    let LockAttempt::Acquired(_lifetime) = session
        .try_acquire_session_lifetime_lock()
        .map_err(io::Error::other)?
    else {
        return Err(io::Error::other("Session lifetime is already owned"));
    };
    #[cfg(windows)]
    let record = session.retain_active_record()?;
    #[cfg(windows)]
    let (checkpoint_store, checkpoint, session_record, suppress_initial_empty) =
        checkpoint_windows::load(&session)?;
    #[cfg(unix)]
    nix::unistd::setsid()?;
    // The observation endpoint binds and starts before the Worker socket
    // or any emitter can exist; a bind/start failure fails the whole Session
    // start loudly under the held lifetime lock rather than publishing a
    // ready Session without its observation channel.
    let bound = super::agent_observation_endpoint::bind(runtime)?;
    // The bound endpoint is fully functional on any runtime path; only the
    // ingress's stored overlay copy is a String, which cannot carry
    // non-UTF-8 bytes — a non-UTF-8 base therefore stores no path, the same
    // degradation the Worker applies to its own env overlay derivation.
    let socket_path = bound.path().to_str().map(str::to_owned);
    let agents = Arc::new(AgentService::open_observing(
        AgentPolicy {
            read: true,
            manage: true,
        },
        socket_path,
    )?);
    let _endpoint = bound.start(Arc::clone(&agents));
    let listener = UnixListener::bind(session.worker_socket_path())?;
    #[cfg(unix)]
    std::fs::set_permissions(
        session.worker_socket_path(),
        std::fs::Permissions::from_mode(0o600),
    )?;
    listener.set_nonblocking(true)?;
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let executable = cohort.executable(CohortArtifact::Worker);
    let arguments = [
        std::ffi::OsStr::new("--runtime-directory"),
        runtime.as_os_str(),
    ];
    #[cfg(windows)]
    let mut worker = orchbee_local_runtime::spawn_session_child(
        &std::env::var_os("LOCALAPPDATA")
            .map(std::path::PathBuf::from)
            .ok_or(io::ErrorKind::NotFound)?,
        runtime,
        orchbee_local_runtime::SessionChild::Worker,
        &executable,
        &arguments,
        deadline,
    )?;
    #[cfg(not(windows))]
    let mut worker = orchbee_local_runtime::spawn_retained(&executable, &arguments)?;
    #[cfg(windows)]
    let mut carrier = None;
    let admitted = (|| -> io::Result<()> {
        loop {
            if Instant::now() >= deadline {
                return Err(io::ErrorKind::TimedOut.into());
            }
            if worker.try_wait()?.is_some() {
                return Err(io::Error::other("local Worker exited before admission"));
            }
            match listener.accept() {
                Ok((stream, _)) => {
                    #[cfg(not(windows))]
                    super::handle_candidate(stream, &registry, cohort.identity(), Some(&agents));
                    #[cfg(windows)]
                    {
                        carrier = super::admit_candidate(
                            stream,
                            &registry,
                            cohort.identity(),
                            Some(&agents),
                        );
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10))
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
            if registry
                .lock()
                .map_err(|_| io::Error::other("Worker registry failed"))?
                .request_handle()
                .is_some()
            {
                break Ok(());
            }
        }
    })();
    // The managed Session never enrolls a replacement Worker, even after this one disappears.
    drop(listener);
    #[cfg(windows)]
    if let Err(error) = admitted {
        return Err(checkpoint_windows::abort_before_launch(
            worker, carrier, error,
        ));
    }
    #[cfg(not(windows))]
    admitted?;
    #[cfg(not(windows))]
    drop(worker);
    #[cfg(windows)]
    let initialized = (|| {
        let handle = registry
            .lock()
            .map_err(|_| io::Error::other("Worker registry failed"))?
            .request_handle()
            .ok_or_else(|| io::Error::other("Worker carrier disappeared"))?;
        let (mut terminals, pending) = LocalTerminalService::from_checkpoint(
            registry,
            checkpoint,
            LocalTerminalService::preferences_scope(&configuration)?,
        )?;
        terminals.configure_graphics_files(runtime);
        Ok::<_, io::Error>((handle, Arc::new(terminals), pending))
    })();
    // File frames refuse at the frame directory adapter here; creation stays lazy.
    #[cfg(not(windows))]
    let terminals = {
        let mut terminals = LocalTerminalService::new(registry);
        terminals.configure_graphics_files(runtime);
        Arc::new(terminals)
    };
    #[cfg(not(windows))]
    terminals.initialize_preferences(
        orchbee_local_runtime::UserPreferencesScope::from_environment(|name| {
            std::env::var_os(name)
        })
        .map_err(|_| io::Error::other("Settings scope is unavailable"))?,
    )?;
    #[cfg(not(windows))]
    terminals.start_close_maintenance()?;
    // Hosts here cannot sample processes, so the observer refuses before reading settings.
    #[cfg(not(windows))]
    match terminals.start_process_observer(
        || {
            orchbee_local_runtime::read_user_settings(|name| std::env::var_os(name))
                .map_err(io::Error::other)
        },
        true,
    ) {
        Err(error) if error.kind() == io::ErrorKind::Unsupported => {}
        started => started?,
    }
    #[cfg(windows)]
    {
        let (handle, terminals, pending) = match initialized {
            Ok(initialized) => initialized,
            Err(error) => {
                return Err(checkpoint_windows::abort_before_launch(
                    worker, carrier, error,
                ));
            }
        };
        let Some(carrier) = carrier else {
            return Err(checkpoint_windows::abort_before_launch(
                worker,
                None,
                io::Error::other("Worker wire owner did not start"),
            ));
        };
        stop_windows::run(
            stop_windows::Owners {
                session,
                lifetime: _lifetime,
                record,
                worker,
                carrier,
                handle,
                ingress: _endpoint,
                terminals,
                agents,
                checkpoint_store: Arc::new(Mutex::new(checkpoint_store)),
                checkpoint_saver: None,
                suppress_initial_empty,
                session_record,
            },
            cohort.identity(),
            pending,
            deadline,
            configuration,
        )
    }
    #[cfg(not(windows))]
    {
        let _clients = local_client_service::start_managed(
            session.client_socket_path(),
            cohort.identity(),
            terminals,
            agents,
        )?;
        loop {
            thread::park();
        }
    }
}
