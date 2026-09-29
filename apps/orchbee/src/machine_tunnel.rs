//! Persistent Windows SSH carriers. Native admission remains end-to-end; no frame is replayed.
mod program;
mod relay;
#[cfg(test)]
mod tests;
use crate::{local_client::LocalClient, machine_ssh, machines::Profile};
use orchbee_local_ipc::{LocalListener, LocalStream};
use orchbee_local_site_runtime::{ModeCheckedLocalSiteRendezvous, NativeCohort, SiteScope};
use std::{
    io,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

/// Retained by every Client origin, including secondary lanes and reconnects.
#[derive(Debug)]
pub(crate) struct Tunnel {
    stop: Arc<AtomicBool>,
    task: Option<JoinHandle<io::Result<()>>>,
}
impl Drop for Tunnel {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(task) = self.task.take() {
            let _settled = task.join();
        }
    }
}
/// An admitted remote Site carrier. Cancellation belongs to the connection's owner.
pub(crate) fn connect(
    profile: &Profile,
    deadline: Instant,
    cancelled: &AtomicU8,
) -> io::Result<LocalClient> {
    if !profile.enabled {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    let admitted = machine_ssh::probe(&profile.draft, deadline, || {
        Ok(cancelled.load(Ordering::Acquire) != 0)
    })?;
    if cancelled.load(Ordering::Acquire) != 0 {
        return Err(io::ErrorKind::Interrupted.into());
    }
    let ssh = machine_ssh::executable()?;
    let remote = program::command(
        &admitted.executable,
        profile.draft.session.as_str(),
        &admitted.cohort,
    );
    let target = profile.draft.target.clone();
    let (endpoint, guard) = start(move || Ok(machine_ssh::command(&ssh, &target, &remote)))?;
    // The invocation-owned wake is observed throughout this single admission.
    let client = LocalClient::connect_before_cancellable(
        &endpoint,
        &admitted.cohort,
        deadline,
        Some(orchbee_local_ipc::LocalIo::Wake(cancelled, 1)),
    )
    .map_err(admission_error)?;
    if cancelled.load(Ordering::Acquire) != 0 {
        return Err(io::ErrorKind::Interrupted.into());
    }
    Ok(client.with_tunnel(guard))
}
pub(crate) fn admission_error(error: crate::local_client::LocalClientConnectError) -> io::Error {
    use crate::local_client::LocalClientConnectError;
    use orchbee_local_client_protocol::ConnectClientError;
    match error {
        LocalClientConnectError::RequestIoConfiguration(error)
        | LocalClientConnectError::Admission(
            ConnectClientError::ConnectFailed(error)
            | ConnectClientError::Io(error)
            | ConnectClientError::CancellationSourceFault(error),
        ) => error,
        LocalClientConnectError::Admission(ConnectClientError::CancellationReady) => {
            io::ErrorKind::Interrupted.into()
        }
        LocalClientConnectError::Admission(ConnectClientError::Rejected(
            orchbee_local_client_protocol::RejectionReason::CapacityExceeded
            | orchbee_local_client_protocol::RejectionReason::HandshakeTimeout
            | orchbee_local_client_protocol::RejectionReason::Stopping,
        )) => io::ErrorKind::ConnectionRefused.into(),
        LocalClientConnectError::Admission(ConnectClientError::Rejected(_)) => {
            io::ErrorKind::PermissionDenied.into()
        }
        LocalClientConnectError::Admission(
            ConnectClientError::Protocol(_) | ConnectClientError::UnexpectedResponse,
        ) => io::ErrorKind::InvalidData.into(),
    }
}
fn start(
    command: impl Fn() -> io::Result<std::process::Command> + Send + 'static,
) -> io::Result<(PathBuf, Arc<Tunnel>)> {
    let id = crate::machines::ProfileId::generate()
        .map_err(|_| io::Error::other("machine endpoint identity unavailable"))?;
    let endpoint = PathBuf::from(format!(r"\\.\pipe\orchbee-machine-{}", id.as_str()));
    let listener = LocalListener::bind(&endpoint)?;
    listener.set_nonblocking(true)?;
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    let task = thread::Builder::new()
        .name("orchbee-machine-relay".into())
        .spawn(move || {
            let mut lanes = Vec::new();
            while !worker_stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((stream, _)) if lanes.len() < 16 => {
                        if let Ok(lane) =
                            command().and_then(|command| relay::Lane::command(stream, command))
                        {
                            lanes.push(lane);
                        }
                    }
                    Ok(_) => {} // Capacity refusal closes only that candidate lane.
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                    Err(_) => break,
                }
                let mut index = 0;
                while index < lanes.len() {
                    if matches!(lanes[index].step(), Ok(true)) {
                        index += 1;
                    } else {
                        let mut lane = lanes.swap_remove(index);
                        let _settled = lane.settle();
                    }
                }
                thread::sleep(Duration::from_millis(5));
            }
            let mut failure = None;
            for lane in &mut lanes {
                if let Err(error) = lane.settle() {
                    failure = Some(error);
                }
            }
            failure.map_or(Ok(()), Err)
        })?;
    Ok((
        endpoint,
        Arc::new(Tunnel {
            stop,
            task: Some(task),
        }),
    ))
}
/// Private binary helper: only the selected existing same-user Site is admitted.
pub(crate) fn serve(expected_cohort: &str) -> io::Result<()> {
    use std::io::IsTerminal;
    if io::stdin().is_terminal() || io::stdout().is_terminal() {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let deadline = Instant::now() + Duration::from_secs(20);
    let cohort = NativeCohort::current(deadline)?;
    if cohort.identity() != expected_cohort {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    let (base, scope) = crate::local_activation::runtime_location()?;
    let site = ModeCheckedLocalSiteRendezvous::validate_mode_checked(
        &base,
        SiteScope::parse(&scope).map_err(io::Error::other)?,
    )
    .map_err(io::Error::other)?;
    let stream = LocalStream::connect_before(site.client_socket_path(), deadline)?;
    stream.require_same_user()?;
    let mut lane = relay::Lane::stdio(stream)?;
    let result = loop {
        match lane.step() {
            Ok(true) => thread::sleep(Duration::from_millis(5)),
            Ok(false) => break Ok(()),
            Err(error) => break Err(error),
        }
    };
    let settled = lane.settle();
    result.and(settled)
}
