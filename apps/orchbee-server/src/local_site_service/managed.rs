use crate::{
    local_client_service, local_terminal_service::LocalTerminalService,
    worker_registry::WorkerRegistry,
};
use orchbee_local_site_runtime::{
    LockAttempt, ModeCheckedLocalSiteRendezvous, NativeArtifact, NativeCohort, RetainedChild,
};
use std::{
    io,
    os::unix::{fs::PermissionsExt, net::UnixListener},
    path::Path,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

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
    nix::unistd::setsid()?;
    let listener = UnixListener::bind(site.worker_socket_path())?;
    std::fs::set_permissions(
        site.worker_socket_path(),
        std::fs::Permissions::from_mode(0o600),
    )?;
    listener.set_nonblocking(true)?;
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let mut worker = RetainedChild::new(
        Command::new(cohort.executable(NativeArtifact::Worker))
            .arg("--local-site")
            .arg(runtime)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?,
    );
    loop {
        if Instant::now() >= deadline {
            return Err(io::ErrorKind::TimedOut.into());
        }
        if worker.try_wait()?.is_some() {
            return Err(io::Error::other("local Worker exited before admission"));
        }
        match listener.accept() {
            Ok((stream, _)) => super::handle_candidate(stream, &registry, cohort.identity()),
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
    drop(worker);
    let terminals = Arc::new(LocalTerminalService::new(registry));
    terminals.start_close_maintenance()?;
    let _clients = local_client_service::start_managed(
        site.client_socket_path(),
        cohort.identity(),
        terminals,
    )?;
    loop {
        thread::park();
    }
}
