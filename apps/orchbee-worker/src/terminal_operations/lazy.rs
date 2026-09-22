use super::{server_transport, unix_milliseconds};
use crate::{
    agent_drain::AgentDrain,
    host_connection::HostBrokerConnection,
    terminal_authorization::{AuthorizedHostOperationKernel, HostOperationAdmission},
    terminal_host_operation::HostOperationDisposition,
};
use orchbee_capability_node_protocol::CapabilityNodeMessage;
use orchbee_local_ipc::LocalStream as UnixStream;
use orchbee_local_site_protocol::WorkerId;
#[cfg(unix)]
use orchbee_local_site_runtime::HOST_ENDPOINT_ID;
use orchbee_local_site_runtime::{
    HOST_SCOPE_NAME, ModeCheckedLocalSiteRendezvous, NativeArtifact, NativeCohort, SiteScope,
};
#[cfg(unix)]
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::{
    cell::RefCell,
    error::Error,
    io,
    num::NonZeroU64,
    path::Path,
    rc::Rc,
    thread,
    time::{Duration, Instant},
};

pub(crate) fn run(
    server: &UnixStream,
    worker_id: WorkerId,
    generation: NonZeroU64,
    cohort: &NativeCohort,
    runtime: &Path,
) -> Result<(), Box<dyn Error>> {
    server.set_nonblocking(true)?;
    // The Agent Drain queue is owned by this admitted Server connection and
    // exists before any Host: pre-Launch requests get a truthful empty page,
    // and the same watermarks survive into the admitted loop unchanged.
    let drain = Rc::new(RefCell::new(AgentDrain::new()));
    let mut kernel = AuthorizedHostOperationKernel::new(worker_id, generation)?;
    // The discovery slot outlives the pre-Host phase: jobs admitted here stay
    // owned across the first Launch instead of being orphaned at handoff.
    let worktree_slot = crate::worktree_discovery_job::WorktreeDiscoverySlot::new();
    loop {
        let request = server_transport::receive(server, None)?;
        if let CapabilityNodeMessage::AgentDrainRequest(request) = request {
            // Without a Host, send always reports host_closed=true; that flag
            // is expected here and does not retire the Server connection.
            let _no_host = super::dispatch_drain_request(
                server, None, &drain, &request, worker_id, generation,
            )?;
            continue;
        }
        if let CapabilityNodeMessage::WorktreeJobRequest(request) = request {
            // Worktree jobs need no Host; they answer directly in this loop.
            let result = crate::worktree_job_operation::execute(
                request,
                worker_id,
                generation,
                &worktree_slot,
            );
            let _no_host = server_transport::send(
                server,
                None,
                &CapabilityNodeMessage::WorktreeJobResult(result),
            )?;
            continue;
        }
        if let CapabilityNodeMessage::TerminalCwdValidationRequest(request) = request {
            let result = crate::terminal_cwd_validation::validate(request, worker_id, generation);
            // The send result reports Host closure, not Server closure. This pre-Host loop has no
            // Host to observe, so only a result-write error retires the Server connection.
            let _no_host = server_transport::send(
                server,
                None,
                &CapabilityNodeMessage::TerminalCwdValidationResult(result),
            )?;
            continue;
        }
        let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = request else {
            // No binding exists before the first Launch. Other non-Launch traffic retires this
            // connection and cannot acquire Host-start authority.
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "expected first authorized Launch",
            )
            .into());
        };
        let dispatch = match kernel.admit_launch(launch, unix_milliseconds())? {
            HostOperationAdmission::Dispatch(dispatch) => dispatch,
            HostOperationAdmission::ResultReady => {
                let result = super::take_result(&mut kernel)?;
                let _no_host = server_transport::send(server, None, &result)?;
                continue;
            }
            HostOperationAdmission::SlotOccupied => {
                return Err(io::Error::other("occupied operation slot").into());
            }
        };
        // Preserve this exact kernel and dispatch across preparation and startup.
        let prepared = super::prepare(&dispatch, Some(runtime));
        let started = prepared.and_then(|prepared| {
            start_host(cohort, runtime, Rc::clone(&drain)).map(|host| (prepared, host))
        });
        let (prepared, mut host) = match started {
            Ok(started) => started,
            Err(error) => {
                kernel.record_pre_handoff_failure(&dispatch, unix_milliseconds())?;
                let result = super::take_result(&mut kernel)?;
                let _delivery = server_transport::send(server, None, &result);
                return Err(error);
            }
        };
        host.stream().set_nonblocking(true)?;
        let disposition = prepared.execute(
            &mut kernel,
            &mut host,
            server,
            unix_milliseconds,
            super::HOST_OPERATION_BUDGET,
        )?;
        if disposition == HostOperationDisposition::ServerClosed {
            return Err(super::peer_closed().into());
        }
        let result = super::take_result(&mut kernel)?;
        let closed = server_transport::send(
            server,
            (disposition == HostOperationDisposition::ResultReady).then_some(&host),
            &result,
        )?;
        if closed {
            return Err(super::peer_closed().into());
        }
        return super::run_admitted(
            server,
            &mut host,
            worker_id,
            generation,
            kernel,
            super::AdmittedAux {
                drain: &drain,
                worktree_slot: &worktree_slot,
                site_runtime: Some(runtime),
            },
        );
    }
}

fn start_host(
    cohort: &NativeCohort,
    runtime: &Path,
    drain: Rc<RefCell<AgentDrain>>,
) -> Result<HostBrokerConnection, Box<dyn Error>> {
    let deadline = Instant::now() + Duration::from_secs(5);
    // The Host binds `<id>.s` relatively inside `host/` — a 34-byte leaf that
    // always fits — while the Worker connects through the derived absolute
    // path. That absolute address can outgrow this platform's socket ABI, so
    // validate it before the Host directory or process can appear.
    let endpoint = ModeCheckedLocalSiteRendezvous::host_endpoint_path(runtime);
    #[cfg(unix)]
    nix::sys::socket::UnixAddr::new(&endpoint)?;
    let directory = ModeCheckedLocalSiteRendezvous::prepare_mode_checked(
        runtime,
        SiteScope::parse(HOST_SCOPE_NAME)?,
    )?;
    #[cfg(unix)]
    match std::fs::symlink_metadata(&endpoint) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Ok(_) => {
            return Err(
                io::Error::new(io::ErrorKind::AlreadyExists, "Host recovery is required").into(),
            );
        }
        Err(error) => return Err(error.into()),
    }
    #[cfg(windows)]
    if orchbee_local_ipc::endpoint_exists(&endpoint)? {
        return Err(
            io::Error::new(io::ErrorKind::AlreadyExists, "Host recovery is required").into(),
        );
    }
    #[cfg(windows)]
    let host_endpoint_id = ModeCheckedLocalSiteRendezvous::host_endpoint_id(runtime);
    #[cfg(unix)]
    let host_endpoint_id = HOST_ENDPOINT_ID;
    let mut child = orchbee_local_site_runtime::spawn_retained(
        &cohort.executable(NativeArtifact::TerminalHost),
        &[
            std::ffi::OsStr::new("serve"),
            std::ffi::OsStr::new("--runtime-directory"),
            directory.runtime_directory().as_os_str(),
            std::ffi::OsStr::new("--endpoint-id"),
            std::ffi::OsStr::new(&host_endpoint_id),
        ],
    )?;
    loop {
        if Instant::now() >= deadline {
            return Err(io::Error::from(io::ErrorKind::TimedOut).into());
        }
        if child.try_wait()?.is_some() {
            return Err(io::Error::other("Terminal Host exited before readiness").into());
        }
        #[cfg(windows)]
        {
            if orchbee_local_ipc::endpoint_exists(&endpoint)? {
                return HostBrokerConnection::connect_before(&endpoint, deadline, drain);
            }
            thread::sleep(Duration::from_millis(10));
        }
        #[cfg(unix)]
        match std::fs::symlink_metadata(&endpoint) {
            Ok(status)
                if status.file_type().is_socket()
                    && status.uid() == nix::unistd::geteuid().as_raw()
                    && status.mode() & 0o7777 == 0o600 =>
            {
                return HostBrokerConnection::connect_before(&endpoint, deadline, drain);
            }
            Ok(status)
                if status.file_type().is_socket()
                    && status.uid() == nix::unistd::geteuid().as_raw() =>
            {
                // This endpoint did not exist before our one spawn. Wait for the Host's
                // final chmod; never connect to the intermediate socket mode.
                thread::sleep(Duration::from_millis(10));
            }
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "invalid Host endpoint",
                )
                .into());
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                thread::sleep(Duration::from_millis(10))
            }
            Err(error) => return Err(error.into()),
        }
    }
}

#[cfg(all(test, unix))]
mod tests;
