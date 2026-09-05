use std::{
    error::Error,
    io,
    num::NonZeroU64,
    os::unix::net::UnixStream,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use orchbee_capability_node_protocol::{CapabilityNodeMessage, HostEpoch};
use orchbee_local_site_protocol::WorkerId;
use orchbee_terminal_protocol::CorrelationId;

use crate::{
    host_connection::HostBrokerConnection,
    terminal_authorization::{
        AuthorizedHostDispatch, AuthorizedHostOperation, AuthorizedHostOperationKernel,
        AuthorizedHostOperationResult, HostOperationAdmission,
    },
    terminal_host_operation::{HostOperationDisposition, PreparedHostOperation},
};

mod launch_facts;
mod server_transport;

// Leave room for result delivery within the Server carrier's response budget.
const HOST_OPERATION_BUDGET: Duration = Duration::from_secs(1);

pub(crate) fn run(
    server: &UnixStream,
    host: &mut HostBrokerConnection,
    worker_id: WorkerId,
    generation: NonZeroU64,
) -> Result<(), Box<dyn Error>> {
    server.set_nonblocking(true)?;
    host.stream().set_nonblocking(true)?;
    let mut kernel = AuthorizedHostOperationKernel::new(worker_id, generation)?;
    let epoch = HostEpoch::from_bytes(*host.host_epoch().as_bytes())?;
    loop {
        let request = server_transport::receive(server, host)?;
        crate::terminal_viewer_operation::cleanup_closed(host, server)?;
        if let CapabilityNodeMessage::TerminalControlRequest(request) = request {
            let result = crate::terminal_control_operation::execute(
                request, worker_id, generation, host, server,
            )?;
            if server_transport::send(
                server,
                Some(host),
                &CapabilityNodeMessage::TerminalControlResult(result),
            )? {
                return Err(peer_closed().into());
            }
            continue;
        }
        if let CapabilityNodeMessage::TerminalViewerRequest(request) = request {
            let result = crate::terminal_viewer_operation::execute(
                request, worker_id, generation, host, server,
            )?;
            if server_transport::send(
                server,
                Some(host),
                &CapabilityNodeMessage::TerminalViewerResult(result),
            )? {
                return Err(peer_closed().into());
            }
            continue;
        }
        let now = unix_milliseconds();
        let admission = match request {
            CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) => {
                kernel.admit_launch(launch, now)?
            }
            CapabilityNodeMessage::AuthorizedTerminalStart(start) => {
                kernel.admit_start(start, now, epoch)?
            }
            CapabilityNodeMessage::TerminalControlRequest(_)
            | CapabilityNodeMessage::TerminalControlResult(_)
            | CapabilityNodeMessage::TerminalLaunchResult(_)
            | CapabilityNodeMessage::TerminalStartResult(_)
            | CapabilityNodeMessage::TerminalViewerRequest(_)
            | CapabilityNodeMessage::TerminalViewerResult(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Server sent a Worker-to-Server result",
                )
                .into());
            }
        };
        let disposition = match admission {
            HostOperationAdmission::Dispatch(dispatch) => prepare(&dispatch)?.execute(
                &mut kernel,
                host,
                server,
                unix_milliseconds,
                HOST_OPERATION_BUDGET,
            )?,
            HostOperationAdmission::ResultReady => HostOperationDisposition::ResultReady,
            HostOperationAdmission::SlotOccupied => {
                return Err(io::Error::other("Worker operation slot is still occupied").into());
            }
        };
        if disposition == HostOperationDisposition::ServerClosed {
            return Err(peer_closed().into());
        }
        let result = match kernel
            .take_result()
            .ok_or_else(|| io::Error::other("Worker operation has no result"))?
        {
            AuthorizedHostOperationResult::Launch(result) => {
                CapabilityNodeMessage::TerminalLaunchResult(result)
            }
            AuthorizedHostOperationResult::Start(result) => {
                CapabilityNodeMessage::TerminalStartResult(result)
            }
        };
        // No next admission occurs while this exact result is being delivered. A
        // failed write abandons the process; it never causes a Host retry.
        let host_closed = server_transport::send(
            server,
            (disposition == HostOperationDisposition::ResultReady).then_some(&*host),
            &result,
        )?;
        if host_closed {
            return Err(peer_closed().into());
        }
    }
}

fn prepare(dispatch: &AuthorizedHostDispatch) -> Result<PreparedHostOperation, Box<dyn Error>> {
    let correlation = CorrelationId::from_bytes(crate::worker_identity::fresh_identity()?)?;
    match dispatch.operation() {
        AuthorizedHostOperation::Launch(launch) => Ok(PreparedHostOperation::launch(
            dispatch,
            launch_facts::resolve(launch.cwd())?,
            correlation,
        )?),
        AuthorizedHostOperation::Start(_) => {
            Ok(PreparedHostOperation::start(dispatch, correlation)?)
        }
    }
}

fn unix_milliseconds() -> u64 {
    // An unavailable or unrepresentable wall clock expires every lease.
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|elapsed| u64::try_from(elapsed.as_millis()).ok())
        .unwrap_or(u64::MAX)
}

fn peer_closed() -> io::Error {
    io::Error::new(
        io::ErrorKind::ConnectionAborted,
        "Worker peer connection ended",
    )
}
