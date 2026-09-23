#[cfg(not(windows))]
use crate::local_client_service;
use crate::{
    agent_service::{AgentPolicy, AgentService},
    local_terminal_service::LocalTerminalService,
    worker_registry::WorkerRegistry,
};
use orchbee_local_ipc::LocalListener as UnixListener;
use orchbee_local_site_runtime::{
    LockAttempt, ModeCheckedLocalSiteRendezvous, NativeArtifact, NativeCohort,
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
mod stop_windows;

pub(super) fn run(runtime: &Path) -> io::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(20);
    let cohort = NativeCohort::current(deadline)?;
    let site = ModeCheckedLocalSiteRendezvous::from_runtime_directory(runtime)
        .map_err(io::Error::other)?;
    if !site.activation_started()? {
        return Err(io::Error::other("Site activation was not committed"));
    }
    let LockAttempt::Acquired(_lifetime) = site
        .try_acquire_site_lifetime_lock()
        .map_err(io::Error::other)?
    else {
        return Err(io::Error::other("Site lifetime is already owned"));
    };
    #[cfg(windows)]
    let record = site.retain_active_record()?;
    #[cfg(unix)]
    nix::unistd::setsid()?;
    // The observation endpoint binds and starts before the Worker socket
    // or any emitter can exist; a bind/start failure fails the whole Site
    // start loudly under the held lifetime lock rather than publishing a
    // ready Site without its observation channel.
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
    let listener = UnixListener::bind(site.worker_socket_path())?;
    #[cfg(unix)]
    std::fs::set_permissions(
        site.worker_socket_path(),
        std::fs::Permissions::from_mode(0o600),
    )?;
    listener.set_nonblocking(true)?;
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let mut worker = orchbee_local_site_runtime::spawn_retained(
        &cohort.executable(NativeArtifact::Worker),
        &[std::ffi::OsStr::new("--local-site"), runtime.as_os_str()],
    )?;
    #[cfg(windows)]
    let mut carrier = None;
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
                    carrier =
                        super::admit_candidate(stream, &registry, cohort.identity(), Some(&agents));
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
            break;
        }
    }
    // The managed Site never enrolls a replacement Worker, even after this one disappears.
    drop(listener);
    #[cfg(not(windows))]
    drop(worker);
    #[cfg(windows)]
    let handle = registry
        .lock()
        .map_err(|_| io::Error::other("Worker registry failed"))?
        .request_handle()
        .ok_or_else(|| io::Error::other("Worker carrier disappeared"))?;
    let terminals = Arc::new(LocalTerminalService::new(registry));
    terminals.start_close_maintenance()?;
    #[cfg(windows)]
    {
        stop_windows::run(
            stop_windows::Owners {
                site,
                lifetime: _lifetime,
                record,
                worker,
                carrier: carrier
                    .ok_or_else(|| io::Error::other("Worker wire owner did not start"))?,
                handle,
                ingress: _endpoint,
                terminals,
                agents,
            },
            cohort.identity(),
        )
    }
    #[cfg(not(windows))]
    {
        let _clients = local_client_service::start_managed(
            site.client_socket_path(),
            cohort.identity(),
            terminals,
            agents,
        )?;
        loop {
            thread::park();
        }
    }
}
