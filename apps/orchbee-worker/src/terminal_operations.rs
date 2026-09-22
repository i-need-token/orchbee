use orchbee_local_ipc::LocalStream as UnixStream;
use std::{
    cell::RefCell,
    error::Error,
    io,
    num::NonZeroU64,
    rc::Rc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use orchbee_capability_node_protocol::{CapabilityNodeMessage, HostEpoch};
use orchbee_local_site_protocol::WorkerId;
use orchbee_terminal_protocol::CorrelationId;

use crate::{
    agent_drain::AgentDrain,
    host_connection::HostBrokerConnection,
    terminal_authorization::{
        AuthorizedHostDispatch, AuthorizedHostOperation, AuthorizedHostOperationKernel,
        AuthorizedHostOperationResult, HostOperationAdmission,
    },
    terminal_host_operation::{HostOperationDisposition, PreparedHostOperation},
};

mod launch_facts;
#[cfg(any(target_os = "linux", windows))]
mod lazy;
#[cfg(any(target_os = "linux", windows))]
pub(crate) use lazy::run as run_lazy;
mod server_transport;

#[cfg(test)]
mod drain_tests;

// Leave room for result delivery within the Server carrier's response budget.
const HOST_OPERATION_BUDGET: Duration = Duration::from_secs(1);

pub(crate) fn run(
    server: &UnixStream,
    host: &mut HostBrokerConnection,
    worker_id: WorkerId,
    generation: NonZeroU64,
    drain: &Rc<RefCell<AgentDrain>>,
) -> Result<(), Box<dyn Error>> {
    server.set_nonblocking(true)?;
    host.stream().set_nonblocking(true)?;
    let kernel = AuthorizedHostOperationKernel::new(worker_id, generation)?;
    let worktree_slot = crate::worktree_discovery_job::WorktreeDiscoverySlot::new();
    run_admitted(
        server,
        host,
        worker_id,
        generation,
        kernel,
        AdmittedAux {
            drain,
            worktree_slot: &worktree_slot,
            site_runtime: None,
        },
    )
}

/// Auxiliary channels admitted alongside the operation kernel: the Agent
/// drain, the WorktreeJob discovery slot, and the managed site runtime
/// directory (`None` for unmanaged launches).
pub(crate) struct AdmittedAux<'a> {
    pub(crate) drain: &'a Rc<RefCell<AgentDrain>>,
    pub(crate) worktree_slot: &'a crate::worktree_discovery_job::WorktreeDiscoverySlot,
    pub(crate) site_runtime: Option<&'a std::path::Path>,
}

pub(crate) fn run_admitted(
    server: &UnixStream,
    host: &mut HostBrokerConnection,
    worker_id: WorkerId,
    generation: NonZeroU64,
    mut kernel: AuthorizedHostOperationKernel,
    aux: AdmittedAux<'_>,
) -> Result<(), Box<dyn Error>> {
    let epoch = HostEpoch::from_bytes(*host.host_epoch().as_bytes())?;
    loop {
        let request = server_transport::receive(server, Some(host))?;
        if let CapabilityNodeMessage::AgentDrainRequest(request) = request {
            let host_closed = dispatch_drain_request(
                server,
                Some(host),
                aux.drain,
                &request,
                worker_id,
                generation,
            )?;
            if host_closed {
                return Err(peer_closed().into());
            }
            continue;
        }
        if let CapabilityNodeMessage::WorktreeJobRequest(request) = request {
            // Start only admits a background owner; it never waits for the
            // child. Every reply echoes operation identity and Job ID.
            let result = crate::worktree_job_operation::execute(
                request,
                worker_id,
                generation,
                aux.worktree_slot,
            );
            if server_transport::send(
                server,
                Some(host),
                &CapabilityNodeMessage::WorktreeJobResult(result),
            )? {
                return Err(peer_closed().into());
            }
            continue;
        }
        if let CapabilityNodeMessage::TerminalCwdValidationRequest(request) = request {
            let result = crate::terminal_cwd_validation::validate(request, worker_id, generation);
            if server_transport::send(
                server,
                Some(host),
                &CapabilityNodeMessage::TerminalCwdValidationResult(result),
            )? {
                return Err(peer_closed().into());
            }
            continue;
        }
        if let CapabilityNodeMessage::TerminalLifecycleRequest(request) = request {
            let disposition = match kernel.admit_lifecycle(request, unix_milliseconds(), epoch)? {
                crate::terminal_authorization::LifecycleAdmission::Dispatch(dispatch) => {
                    crate::terminal_lifecycle_operation::PreparedLifecycleOperation::new(
                        dispatch, host,
                    )?
                    .execute(
                        &mut kernel,
                        host,
                        server,
                        unix_milliseconds,
                        HOST_OPERATION_BUDGET,
                    )?
                }
                crate::terminal_authorization::LifecycleAdmission::ResultReady => {
                    HostOperationDisposition::ResultReady
                }
                crate::terminal_authorization::LifecycleAdmission::SlotOccupied => {
                    return Err(io::Error::other("Worker lifecycle slot is still occupied").into());
                }
            };
            if disposition == HostOperationDisposition::ServerClosed {
                return Err(peer_closed().into());
            }
            let result = kernel
                .take_lifecycle_result()
                .ok_or_else(|| io::Error::other("Worker lifecycle result is missing"))?;
            if server_transport::send(
                server,
                (disposition == HostOperationDisposition::ResultReady).then_some(&*host),
                &CapabilityNodeMessage::TerminalLifecycleResult(result),
            )? {
                return Err(peer_closed().into());
            }
            continue;
        }
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
        if let CapabilityNodeMessage::TerminalSendRequest(request) = request {
            let execution = crate::terminal_send_operation::execute(
                request, worker_id, generation, host, server,
            )?;
            // A failed Host carrier still gets its result written; that write
            // is the last record this session produces and the loop exits
            // explicitly instead of discovering the dead stream later.
            let host_closed = server_transport::send(
                server,
                (!execution.host_failed).then_some(&*host),
                &CapabilityNodeMessage::TerminalSendResult(execution.result),
            )?;
            if host_closed || execution.host_failed {
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
            CapabilityNodeMessage::AgentDrainRequest(_)
            | CapabilityNodeMessage::AgentDrainReply(_)
            | CapabilityNodeMessage::TerminalControlRequest(_)
            | CapabilityNodeMessage::TerminalCwdValidationRequest(_)
            | CapabilityNodeMessage::TerminalCwdValidationResult(_)
            | CapabilityNodeMessage::TerminalLifecycleRequest(_)
            | CapabilityNodeMessage::TerminalLifecycleResult(_)
            | CapabilityNodeMessage::TerminalControlResult(_)
            | CapabilityNodeMessage::TerminalLaunchResult(_)
            | CapabilityNodeMessage::TerminalSendRequest(_)
            | CapabilityNodeMessage::TerminalSendResult(_)
            | CapabilityNodeMessage::TerminalStartResult(_)
            | CapabilityNodeMessage::TerminalViewerRequest(_)
            | CapabilityNodeMessage::TerminalViewerResult(_)
            | CapabilityNodeMessage::WorktreeJobRequest(_)
            | CapabilityNodeMessage::WorktreeJobResult(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Server sent a Worker-to-Server result",
                )
                .into());
            }
        };
        let disposition = match admission {
            HostOperationAdmission::Dispatch(dispatch) => prepare(&dispatch, aux.site_runtime)?
                .execute(
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
        let result = take_result(&mut kernel)?;
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

/// One Agent Drain request dispatched on the serialized Server connection.
/// Shared by the pre-Host lazy loop and the admitted loop: the queue state is
/// the same object either way. A request fenced to another Worker or
/// generation is a protocol violation, not a typed rejection — the caller ends
/// the connection. Returns the send host-closed flag: both `Ok` values prove
/// the page was completely written, so the captured watermark commits first
/// and the caller decides whether a closed Host retires the connection.
pub(super) fn dispatch_drain_request(
    server: &UnixStream,
    host: Option<&HostBrokerConnection>,
    drain: &RefCell<AgentDrain>,
    request: &orchbee_capability_node_protocol::AgentDrainRequest,
    worker_id: WorkerId,
    generation: NonZeroU64,
) -> Result<bool, Box<dyn Error>> {
    if request.worker_id.as_bytes() != worker_id.as_bytes()
        || request.worker_generation.get() != generation.get()
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Agent drain request is fenced to another Worker or generation",
        )
        .into());
    }
    // The borrow ends before the bounded write: send consumes Host events
    // mid-write, and those events append to this same queue.
    let (reply, covered_hi) = drain
        .borrow_mut()
        .respond(request.token, request.acknowledged)?;
    let host_closed =
        server_transport::send(server, host, &CapabilityNodeMessage::AgentDrainReply(reply))?;
    // Only the captured watermark commits — never a queue re-read.
    if let Some(covered_hi) = covered_hi {
        drain.borrow_mut().commit_offered(covered_hi);
    }
    Ok(host_closed)
}

fn take_result(kernel: &mut AuthorizedHostOperationKernel) -> io::Result<CapabilityNodeMessage> {
    match kernel
        .take_result()
        .ok_or_else(|| io::Error::other("Worker operation has no result"))?
    {
        AuthorizedHostOperationResult::Launch(result) => {
            Ok(CapabilityNodeMessage::TerminalLaunchResult(result))
        }
        AuthorizedHostOperationResult::Start(result) => {
            Ok(CapabilityNodeMessage::TerminalStartResult(result))
        }
    }
}

fn prepare(
    dispatch: &AuthorizedHostDispatch,
    site_runtime: Option<&std::path::Path>,
) -> Result<PreparedHostOperation, Box<dyn Error>> {
    let correlation = CorrelationId::from_bytes(crate::worker_identity::fresh_identity()?)?;
    match dispatch.operation() {
        AuthorizedHostOperation::Launch(launch) => Ok(PreparedHostOperation::launch(
            dispatch,
            launch_facts::resolve(launch.cwd(), launch.launch())?,
            correlation,
            site_runtime,
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
