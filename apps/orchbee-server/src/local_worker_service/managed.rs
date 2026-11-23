//! Composes admitted runtime locks, verified cohort artifacts, retained Worker startup, Client
//! service, and checkpoint lifecycle for a managed Session. A Windows generation that kept its
//! predecessor's running Host adopts it before serving. It does not define domain transitions or
//! adopt an unregistered process as the Session owner.

use crate::{
    agent_service::{AgentPolicy, AgentService},
    local_terminal_service::LocalTerminalService,
    session_shutdown::SessionShutdown,
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
#[cfg(any(windows, target_os = "macos"))]
mod adoption;
mod checkpoint;
mod replacement;
#[cfg(unix)]
mod signals;
mod stop;
#[cfg(windows)]
mod successor;
#[cfg(windows)]
pub(super) use successor::{Refused, run as run_successor};

/// Wakes the lifecycle owner when a Worker connection ends, so its loss is seen at once rather
/// than at the owner's next tick.
fn wake_on_end(shutdown: &std::sync::Weak<SessionShutdown>) -> Box<dyn FnOnce() + Send> {
    let shutdown = shutdown.clone();
    Box::new(move || {
        if let Some(shutdown) = shutdown.upgrade() {
            shutdown.wake();
        }
    })
}

pub(super) fn run(
    runtime: &Path,
    #[cfg(windows)] configuration: orchbee_local_runtime::UserSettingsLoad,
    #[cfg(unix)] configuration: super::startup_configuration::Location,
) -> io::Result<()> {
    // First, so a stop signal that arrives during startup waits for the signal thread.
    #[cfg(unix)]
    let signals = signals::catch()?;
    // Only a location that cannot be admitted stops the Session; any file starts it.
    #[cfg(unix)]
    let configuration = super::read_configuration(configuration).map_err(io::Error::other)?;
    let deadline = Instant::now() + Duration::from_secs(20);
    let cohort = Cohort::current(deadline)?;
    let session = ModeCheckedLocalSessionRendezvous::from_runtime_directory(runtime)
        .map_err(io::Error::other)?;
    if !session.activation_started()? {
        return Err(io::Error::other("Session activation was not committed"));
    }
    // An observer probes this lock for an instant; only a lock still held past that is owned.
    let LockAttempt::Acquired(lifetime) =
        orchbee_local_runtime::acquire_past_probes(|| session.try_acquire_session_lifetime_lock())
            .map_err(io::Error::other)?
    else {
        return Err(io::Error::other("Session lifetime is already owned"));
    };
    // A temporary-file cleaner that honours locks leaves the socket folder alone while this
    // Session lives.
    #[cfg(unix)]
    let socket_root = session.hold_socket_root()?;
    let record = session.retain_active_record()?;
    let (session_record, kept, checkpoint) = checkpoint::load(&session)?;
    // A generation that kept its predecessor's running Host adopts it; one that cannot fails here,
    // before anything starts, and leaves that Host running.
    #[cfg(any(windows, target_os = "macos"))]
    let adopting = adoption::Adopting::find(&session_record, kept.as_ref())?;
    // A Session that ended without its stop left its socket files, and no listener can serve
    // them while this Server holds the lifetime lock.
    #[cfg(unix)]
    session.remove_endpoints(&lifetime)?;
    #[cfg(unix)]
    nix::unistd::setsid()?;
    let shutdown = Arc::new(SessionShutdown::default());
    // Every stop this Server begins, on a signal or after losing its Worker, uses one identity.
    #[cfg(unix)]
    let own = {
        let mut bytes = [0; 16];
        getrandom::fill(&mut bytes).map_err(|error| io::Error::other(error.to_string()))?;
        orchbee_local_client_protocol::StopId::new(bytes).map_err(io::Error::other)?
    };
    #[cfg(unix)]
    signals.forward(Arc::clone(&shutdown), own)?;
    // The observation endpoint binds and starts before the Worker socket
    // or any emitter can exist; a bind/start failure fails the whole Session
    // start loudly under the held lifetime lock rather than publishing a
    // ready Session without its observation channel. Windows withdrew that
    // channel, so a Session there binds none and starts without it.
    let bound = super::agent_observation_endpoint::bind_for_session(runtime)?;
    // The bound endpoint is fully functional on any runtime path; only the
    // ingress's stored overlay copy is a String, which cannot carry
    // non-UTF-8 bytes — a non-UTF-8 base therefore stores no path, the same
    // degradation the Worker applies to its own env overlay derivation.
    let socket_path = bound
        .as_ref()
        .and_then(|bound| bound.path().to_str().map(str::to_owned));
    let agents = Arc::new(AgentService::open_observing(
        AgentPolicy {
            read: true,
            manage: true,
        },
        socket_path,
    )?);
    let endpoint = bound.map(|bound| bound.start(Arc::clone(&agents)));
    let listener = UnixListener::bind(session.worker_socket_path())?;
    #[cfg(unix)]
    std::fs::set_permissions(
        session.worker_socket_path(),
        std::fs::Permissions::from_mode(0o600),
    )?;
    listener.set_nonblocking(true)?;
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let executable = cohort.executable(CohortArtifact::Worker);
    // An adopting Worker keeps the Session Worker id its predecessor's bindings name.
    #[cfg(any(windows, target_os = "macos"))]
    let (worker_argument, replacing) = (
        adopting.as_ref().map(adoption::Adopting::worker_argument),
        adopting.as_ref().map(adoption::Adopting::worker_id),
    );
    #[cfg(not(any(windows, target_os = "macos")))]
    let (worker_argument, replacing): (Option<String>, _) = (None, None);
    let mut arguments = vec![
        std::ffi::OsStr::new("--runtime-directory"),
        runtime.as_os_str(),
    ];
    if let Some(worker_id) = &worker_argument {
        arguments.extend([
            std::ffi::OsStr::new("--worker-id"),
            std::ffi::OsStr::new(worker_id),
        ]);
    }
    #[cfg(any(windows, target_os = "macos"))]
    let mut worker = orchbee_local_runtime::spawn_session_child(
        &orchbee_local_runtime::DataRoot::resolve(|name| std::env::var_os(name))
            .map_err(|_| io::Error::from(io::ErrorKind::NotFound))?,
        runtime,
        orchbee_local_runtime::SessionChild::Worker,
        &executable,
        &arguments,
        deadline,
    )?;
    #[cfg(not(any(windows, target_os = "macos")))]
    let mut worker = orchbee_local_runtime::spawn_retained(&executable, &arguments)?;
    let mut carrier = None;
    let admitted = admit_worker(
        &listener,
        &mut worker,
        Admitting {
            registry: &registry,
            cohort: cohort.identity(),
            agents: &agents,
            replacing,
            shutdown: &shutdown,
        },
        deadline,
        &mut carrier,
    );
    // The listener stays open, admitting a replacement Worker only while the Server starts one.
    #[cfg(any(windows, target_os = "macos"))]
    if let Err(error) = admitted {
        return Err(checkpoint::abort_before_launch(worker, carrier, error));
    }
    // No launch was sent, so this Worker owns no Host; it stops once its Server connection closes.
    #[cfg(not(any(windows, target_os = "macos")))]
    admitted?;
    let initialized = (|| {
        #[cfg(any(windows, target_os = "macos"))]
        if let Some(adopting) = &adopting {
            let (handle, mut terminals, pending) =
                adopting.restore(&registry, checkpoint, &configuration)?;
            terminals.configure_graphics_files(runtime);
            return Ok((handle, Arc::new(terminals), pending));
        }
        let handle = registry
            .lock()
            .map_err(|_| io::Error::other("Worker registry failed"))?
            .request_handle()
            .ok_or_else(|| io::Error::other("Worker carrier disappeared"))?;
        let (mut terminals, pending) = LocalTerminalService::from_checkpoint(
            Arc::clone(&registry),
            checkpoint,
            &configuration,
        )?;
        terminals.configure_graphics_files(runtime);
        Ok::<_, io::Error>((handle, Arc::new(terminals), pending))
    })();
    #[cfg(any(windows, target_os = "macos"))]
    let (handle, terminals, pending) = match initialized {
        Ok(initialized) => initialized,
        Err(error) => {
            return Err(checkpoint::abort_before_launch(worker, carrier, error));
        }
    };
    #[cfg(not(any(windows, target_os = "macos")))]
    let (handle, terminals, pending) = initialized?;
    #[cfg(any(windows, target_os = "macos"))]
    let Some(carrier) = carrier else {
        return Err(checkpoint::abort_before_launch(
            worker,
            None,
            io::Error::other("Worker wire owner did not start"),
        ));
    };
    #[cfg(not(any(windows, target_os = "macos")))]
    let carrier = carrier.ok_or_else(|| io::Error::other("Worker wire owner did not start"))?;
    stop::run(
        stop::Owners {
            session,
            lifetime,
            #[cfg(unix)]
            socket_root,
            record,
            worker,
            carrier,
            handle,
            #[cfg(any(windows, target_os = "macos"))]
            adopted: adopting.is_some(),
            #[cfg(not(any(windows, target_os = "macos")))]
            adopted: false,
            listener,
            registry,
            waker: Arc::downgrade(&shutdown),
            worker_executable: executable,
            serving: crate::local_client_service::ServingWorker::default(),
            ingress: endpoint,
            terminals,
            agents,
            checkpoint: kept,
            #[cfg(any(windows, target_os = "macos"))]
            automatic_updates: None,
            update_job: Arc::default(),
            session_record,
            #[cfg(windows)]
            worker_ticket: None,
        },
        shutdown,
        #[cfg(unix)]
        own,
        cohort.identity(),
        pending,
        deadline,
        configuration,
    )
}

/// What admits a starting Worker on its Server's Worker endpoint.
struct Admitting<'a> {
    registry: &'a Arc<Mutex<WorkerRegistry>>,
    cohort: &'a str,
    agents: &'a Arc<AgentService>,
    /// The Session Worker id an adopting Worker keeps; none for a Worker that adopts nothing.
    replacing: Option<orchbee_local_worker_protocol::WorkerId>,
    shutdown: &'a Arc<SessionShutdown>,
}

/// Admits `worker`, which this Server started, on `listener` until it serves or, adopting, until it
/// waits for the Host it adopts; `carrier` receives its connection's owner as soon as one starts,
/// so a failure still hands it to the cleanup.
fn admit_worker(
    listener: &UnixListener,
    worker: &mut orchbee_local_runtime::RetainedChild,
    admitting: Admitting<'_>,
    deadline: Instant,
    carrier: &mut Option<thread::JoinHandle<()>>,
) -> io::Result<()> {
    loop {
        if Instant::now() >= deadline {
            return Err(io::ErrorKind::TimedOut.into());
        }
        if worker.try_wait()?.is_some() {
            return Err(io::Error::other("local Worker exited before admission"));
        }
        match listener.accept() {
            Ok((stream, _)) => {
                *carrier = super::admit_candidate(
                    stream,
                    admitting.registry,
                    admitting.cohort,
                    Some(admitting.agents),
                    worker.id(),
                    admitting.replacing,
                    Some(wake_on_end(&Arc::downgrade(admitting.shutdown))),
                );
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10))
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
        // An adopting Worker serves no job until it adopted the Host.
        let registry = admitting
            .registry
            .lock()
            .map_err(|_| io::Error::other("Worker registry failed"))?;
        if registry.request_handle().is_some() || registry.adopting_handle().is_some() {
            return Ok(());
        }
    }
}
