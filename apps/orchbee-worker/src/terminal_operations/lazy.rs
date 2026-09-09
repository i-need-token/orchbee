use super::{server_transport, unix_milliseconds};
use crate::{
    host_connection::HostBrokerConnection,
    terminal_authorization::{AuthorizedHostOperationKernel, HostOperationAdmission},
    terminal_host_operation::HostOperationDisposition,
};
use orchbee_capability_node_protocol::CapabilityNodeMessage;
use orchbee_local_site_protocol::WorkerId;
use orchbee_local_site_runtime::{
    ModeCheckedLocalSiteRendezvous, NativeArtifact, NativeCohort, RetainedChild, SiteScope,
};
use std::{
    error::Error,
    io,
    num::NonZeroU64,
    os::unix::{
        fs::{FileTypeExt, MetadataExt},
        net::UnixStream,
    },
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const HOST_ENDPOINT_ID: &str = "00000000000000000000000000000001";

pub(crate) fn run(
    server: &UnixStream,
    worker_id: WorkerId,
    generation: NonZeroU64,
    cohort: &NativeCohort,
    runtime: &Path,
) -> Result<(), Box<dyn Error>> {
    server.set_nonblocking(true)?;
    let mut kernel = AuthorizedHostOperationKernel::new(worker_id, generation)?;
    loop {
        let request = server_transport::receive(server, None)?;
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
        let prepared = super::prepare(&dispatch);
        let started =
            prepared.and_then(|prepared| start_host(cohort, runtime).map(|host| (prepared, host)));
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
        return super::run_admitted(server, &mut host, worker_id, generation, kernel);
    }
}

fn start_host(
    cohort: &NativeCohort,
    runtime: &Path,
) -> Result<HostBrokerConnection, Box<dyn Error>> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let directory =
        ModeCheckedLocalSiteRendezvous::prepare_mode_checked(runtime, SiteScope::parse("host")?)?;
    let endpoint = directory
        .runtime_directory()
        .join(format!("{HOST_ENDPOINT_ID}.s"));
    match std::fs::symlink_metadata(&endpoint) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Ok(_) => {
            return Err(
                io::Error::new(io::ErrorKind::AlreadyExists, "Host recovery is required").into(),
            );
        }
        Err(error) => return Err(error.into()),
    }
    let mut child = RetainedChild::new(
        Command::new(cohort.executable(NativeArtifact::TerminalHost))
            .arg("serve")
            .arg("--runtime-directory")
            .arg(directory.runtime_directory())
            .arg("--endpoint-id")
            .arg(HOST_ENDPOINT_ID)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?,
    );
    loop {
        if Instant::now() >= deadline {
            return Err(io::Error::from(io::ErrorKind::TimedOut).into());
        }
        if child.try_wait()?.is_some() {
            return Err(io::Error::other("Terminal Host exited before readiness").into());
        }
        match std::fs::symlink_metadata(&endpoint) {
            Ok(status)
                if status.file_type().is_socket()
                    && status.uid() == nix::unistd::geteuid().as_raw()
                    && status.mode() & 0o7777 == 0o600 =>
            {
                return HostBrokerConnection::connect_before(&endpoint, deadline);
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

#[cfg(test)]
mod tests;
