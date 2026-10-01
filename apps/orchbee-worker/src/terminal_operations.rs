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
#[cfg(windows)]
mod shutdown;
#[cfg(windows)]
mod titles;

#[cfg(test)]
mod drain_tests;
#[cfg(all(test, windows))]
mod shell_tests;

// Leave room for result delivery within the Server carrier's response budget.
const HOST_OPERATION_BUDGET: Duration = Duration::from_secs(1);

pub(crate) fn run(
    server: &UnixStream,
    host: &mut HostBrokerConnection,
    worker_id: WorkerId,
    generation: NonZeroU64,
    drain: &Rc<RefCell<AgentDrain>>,
    #[cfg(windows)] shell: &orchbee_local_site_runtime::WindowsShellSettings,
) -> Result<(), Box<dyn Error>> {
    server.set_nonblocking(true)?;
    host.stream().set_nonblocking(true)?;
    let kernel = AuthorizedHostOperationKernel::new(worker_id, generation)?;
    let worktree_slot = crate::worktree_discovery_job::WorktreeDiscoverySlot::new();
    #[cfg(windows)]
    let plugin_jobs = crate::plugin_jobs::PluginJobs::new();
    #[cfg(windows)]
    let mutation_slot = crate::worktree_mutation_job::WorktreeMutationSlot::new(
        crate::worktree_mutation_windows::run,
    );
    let outcome = run_admitted(
        server,
        host,
        worker_id,
        generation,
        kernel,
        AdmittedAux {
            #[cfg(windows)]
            shell,
            drain,
            worktree_slot: &worktree_slot,
            #[cfg(windows)]
            mutation_slot: &mutation_slot,
            #[cfg(windows)]
            plugin_jobs: &plugin_jobs,
            site_runtime: None,
            #[cfg(windows)]
            managed_host: None,
        },
    );
    #[cfg(windows)]
    plugin_jobs.stop_before(std::time::Instant::now() + Duration::from_secs(6))?;
    #[cfg(windows)]
    mutation_slot.stop_before(std::time::Instant::now() + Duration::from_secs(6))?;
    outcome
}

/// Auxiliary channels admitted alongside the operation kernel: the Agent
/// drain, the WorktreeJob discovery slot, and the managed site runtime
/// directory (`None` for unmanaged launches).
pub(crate) struct AdmittedAux<'a> {
    #[cfg(windows)]
    pub(crate) shell: &'a orchbee_local_site_runtime::WindowsShellSettings,
    pub(crate) drain: &'a Rc<RefCell<AgentDrain>>,
    pub(crate) worktree_slot: &'a crate::worktree_discovery_job::WorktreeDiscoverySlot,
    #[cfg(windows)]
    pub(crate) mutation_slot: &'a crate::worktree_mutation_job::WorktreeMutationSlot,
    #[cfg(windows)]
    pub(crate) plugin_jobs: &'a crate::plugin_jobs::PluginJobs,
    pub(crate) site_runtime: Option<&'a std::path::Path>,
    #[cfg(windows)]
    pub(super) managed_host: Option<&'a mut shutdown::ManagedHost>,
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
        if let CapabilityNodeMessage::TerminalEditorCommandRequest(request) = request {
            let result = crate::terminal_editor_operation::execute(request, worker_id, generation);
            if server_transport::send(
                server,
                Some(host),
                &CapabilityNodeMessage::TerminalEditorCommandResult(result),
            )? {
                return Err(peer_closed().into());
            }
            continue;
        }
        #[cfg(windows)]
        if let CapabilityNodeMessage::WorkerShutdownRequest(request) = request {
            if let Some(reason) =
                shutdown::refusal(request, worker_id, generation, aux.managed_host.is_some())
            {
                if server_transport::send(
                    server,
                    Some(host),
                    &CapabilityNodeMessage::WorkerShutdownResult(
                        orchbee_capability_node_protocol::WorkerShutdownResult {
                            request,
                            outcome:
                                orchbee_capability_node_protocol::WorkerShutdownOutcome::Refused(
                                    reason,
                                ),
                        },
                    ),
                )? {
                    return Err(peer_closed().into());
                }
                continue;
            }
            return shutdown::run(
                server,
                request,
                worker_id,
                generation,
                shutdown::BackgroundJobs {
                    discovery: aux.worktree_slot,
                    mutations: aux.mutation_slot,
                    plugins: aux.plugin_jobs,
                },
                aux.managed_host.map(|owner| (&*host, owner)),
            );
        }
        #[cfg(windows)]
        if let CapabilityNodeMessage::TerminalTitlesRequest(request) = request {
            let result = titles::execute(server, Some(host), request, worker_id, generation)?;
            if server_transport::send(
                server,
                Some(host),
                &CapabilityNodeMessage::TerminalTitlesResult(result),
            )? {
                return Err(peer_closed().into());
            }
            continue;
        }
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
        if let CapabilityNodeMessage::PluginJobRequest(request) = request {
            let result = crate::plugin_job_operation::execute(
                request,
                worker_id,
                generation,
                #[cfg(windows)]
                aux.plugin_jobs,
                #[cfg(windows)]
                aux.site_runtime,
            );
            if server_transport::send(
                server,
                Some(host),
                &CapabilityNodeMessage::PluginJobResult(result),
            )? {
                return Err(peer_closed().into());
            }
            continue;
        }
        if let CapabilityNodeMessage::WorktreeMutationRequest(request) = request {
            #[cfg(windows)]
            let result = crate::worktree_mutation_operation::execute(
                request,
                worker_id,
                generation,
                aux.mutation_slot,
            );
            #[cfg(not(windows))]
            let result =
                crate::worktree_mutation_operation::execute(request, worker_id, generation);
            if server_transport::send(
                server,
                Some(host),
                &CapabilityNodeMessage::WorktreeMutationResult(result),
            )? {
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
            CapabilityNodeMessage::WorkerShutdownRequest(_)
            | CapabilityNodeMessage::WorkerShutdownResult(_)
            | CapabilityNodeMessage::AgentDrainRequest(_)
            | CapabilityNodeMessage::AgentDrainReply(_)
            | CapabilityNodeMessage::TerminalTitlesRequest(_)
            | CapabilityNodeMessage::TerminalEditorCommandRequest(_)
            | CapabilityNodeMessage::TerminalEditorCommandResult(_)
            | CapabilityNodeMessage::TerminalTitlesResult(_)
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
            | CapabilityNodeMessage::WorktreeJobResult(_)
            | CapabilityNodeMessage::WorktreeMutationRequest(_)
            | CapabilityNodeMessage::WorktreeMutationResult(_)
            | CapabilityNodeMessage::PluginJobRequest(_)
            | CapabilityNodeMessage::PluginJobResult(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Server sent a Worker-to-Server result",
                )
                .into());
            }
        };
        let disposition = match admission {
            HostOperationAdmission::Dispatch(dispatch) => match prepare_or_fail(
                &mut kernel,
                &dispatch,
                aux.site_runtime,
                #[cfg(windows)]
                aux.shell,
            )? {
                Some(prepared) => prepared.execute(
                    &mut kernel,
                    host,
                    server,
                    unix_milliseconds,
                    HOST_OPERATION_BUDGET,
                )?,
                None => HostOperationDisposition::ResultReady,
            },
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

fn prepare_or_fail(
    kernel: &mut AuthorizedHostOperationKernel,
    dispatch: &AuthorizedHostDispatch,
    site_runtime: Option<&std::path::Path>,
    #[cfg(windows)] shell: &orchbee_local_site_runtime::WindowsShellSettings,
) -> Result<Option<PreparedHostOperation>, Box<dyn Error>> {
    match prepare(
        dispatch,
        site_runtime,
        #[cfg(windows)]
        shell,
    ) {
        Ok(prepared) => Ok(Some(prepared)),
        Err(_) => {
            kernel.record_pre_handoff_failure(dispatch, unix_milliseconds())?;
            Ok(None)
        }
    }
}

fn prepare(
    dispatch: &AuthorizedHostDispatch,
    site_runtime: Option<&std::path::Path>,
    #[cfg(windows)] shell: &orchbee_local_site_runtime::WindowsShellSettings,
) -> Result<PreparedHostOperation, Box<dyn Error>> {
    let correlation = CorrelationId::from_bytes(crate::worker_identity::fresh_identity()?)?;
    match dispatch.operation() {
        AuthorizedHostOperation::Launch(launch) => {
            #[cfg(not(windows))]
            if launch.default_shell().is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "Windows shell policy is unsupported",
                )
                .into());
            }
            #[cfg(windows)]
            let selected = launch
                .default_shell()
                .map(|policy| {
                    orchbee_local_site_runtime::WindowsShellSettings::new(
                        policy.executable(),
                        policy.mode().as_str(),
                    )
                })
                .transpose()?;
            #[cfg(windows)]
            let shell = selected.as_ref().unwrap_or(shell);
            Ok(PreparedHostOperation::launch(
                dispatch,
                launch_facts::resolve(
                    launch.cwd(),
                    launch.launch(),
                    launch.environment(),
                    launch.plugin_environment(),
                    #[cfg(windows)]
                    shell,
                )?,
                correlation,
                site_runtime,
            )?)
        }
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
