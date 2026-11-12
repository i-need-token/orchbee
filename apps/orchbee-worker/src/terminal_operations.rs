//! Serializes admitted Server requests across the authorization kernel, Host connection,
//! retained jobs, and managed lifecycle operations. This loop preserves one physical operation
//! owner without deciding Workbench membership or implementing PTY and VT behavior.

use orchbee_local_ipc::LocalStream as UnixStream;
use std::{
    cell::RefCell,
    error::Error,
    io,
    num::NonZeroU64,
    rc::Rc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use orchbee_capability_node_protocol::{CapabilityNodeMessage, HostEpoch, LaunchPreferences};
use orchbee_local_worker_protocol::WorkerId;
use orchbee_terminal_protocol::CorrelationId;

use crate::{
    agent_drain::AgentDrain,
    default_interactive_launch::SystemShell,
    host_connection::HostBrokerConnection,
    terminal_authorization::{
        AuthorizedHostDispatch, AuthorizedHostOperation, AuthorizedHostOperationKernel,
        AuthorizedHostOperationResult, HostOperationAdmission,
    },
    terminal_host_operation::{HostOperationDisposition, PreparedHostOperation},
};

mod background_jobs;
mod host_query;
mod launch_facts;
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
mod lazy;
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
pub(crate) use lazy::run as run_lazy;
#[cfg(windows)]
mod handoff;
mod server_transport;
#[cfg(windows)]
mod shutdown;
mod title_changes;
#[cfg(windows)]
mod titles;

#[cfg(test)]
mod tests;

// Leave room for result delivery within the Server carrier's response budget.
const HOST_OPERATION_BUDGET: Duration = Duration::from_secs(1);

pub(crate) fn run(
    server: &UnixStream,
    host: &mut HostBrokerConnection,
    worker_id: WorkerId,
    generation: NonZeroU64,
    drain: &Rc<RefCell<AgentDrain>>,
    system_shell: &SystemShell,
) -> Result<(), Box<dyn Error>> {
    server.set_nonblocking(true)?;
    host.stream().set_nonblocking(true)?;
    let kernel = AuthorizedHostOperationKernel::new(worker_id, generation)?;
    let worktree_slot = crate::worktree_discovery_job::WorktreeDiscoverySlot::new();
    let agent_integrations = crate::agent_integration_jobs::AgentIntegrationJobs::new();
    let agent_probes = crate::agent_probe_jobs::AgentProbeJobs::new();
    #[cfg(windows)]
    let plugin_jobs = crate::plugin_jobs::PluginJobs::new();
    let mutation_slot =
        crate::worktree_mutation_job::WorktreeMutationSlot::new(crate::worktree_mutation_git::run);
    let outcome = run_admitted(
        server.try_clone()?,
        host,
        worker_id,
        generation,
        kernel,
        AdmittedAux {
            drain,
            system_shell,
            worktree_slot: &worktree_slot,
            agent_integrations: &agent_integrations,
            agent_probes: &agent_probes,
            mutation_slot: &mutation_slot,
            #[cfg(windows)]
            plugin_jobs: &plugin_jobs,
            session_runtime: None,
            #[cfg(windows)]
            managed_host: None,
        },
    );
    background_jobs::BackgroundJobs {
        discovery: &worktree_slot,
        integrations: &agent_integrations,
        probes: &agent_probes,
        mutations: &mutation_slot,
        #[cfg(windows)]
        plugins: &plugin_jobs,
    }
    .settle_exit(outcome)
}

/// Auxiliary channels admitted alongside the operation kernel: the Agent
/// drain, this generation's System shell resolution, the WorktreeJob discovery
/// slot, and the managed session runtime directory (`None` for unmanaged
/// launches).
pub(crate) struct AdmittedAux<'a> {
    pub(crate) drain: &'a Rc<RefCell<AgentDrain>>,
    pub(crate) system_shell: &'a SystemShell,
    pub(crate) worktree_slot: &'a crate::worktree_discovery_job::WorktreeDiscoverySlot,
    pub(crate) agent_integrations: &'a crate::agent_integration_jobs::AgentIntegrationJobs,
    pub(crate) agent_probes: &'a crate::agent_probe_jobs::AgentProbeJobs,
    pub(crate) mutation_slot: &'a crate::worktree_mutation_job::WorktreeMutationSlot,
    #[cfg(windows)]
    pub(crate) plugin_jobs: &'a crate::plugin_jobs::PluginJobs,
    pub(crate) session_runtime: Option<&'a std::path::Path>,
    #[cfg(windows)]
    pub(super) managed_host: Option<&'a mut shutdown::ManagedHost>,
}

/// Owns the admitted Server wire and preserves its physical owners across an explicit handoff.
pub(crate) fn run_admitted(
    server: UnixStream,
    host: &mut HostBrokerConnection,
    worker_id: WorkerId,
    generation: NonZeroU64,
    kernel: AuthorizedHostOperationKernel,
    aux: AdmittedAux<'_>,
) -> Result<(), Box<dyn Error>> {
    #[cfg(windows)]
    let handoff = handoff::Handoff::new(aux.session_runtime);
    run_admitted_with_handoff(
        server,
        host,
        worker_id,
        generation,
        kernel,
        aux,
        #[cfg(windows)]
        handoff,
    )
}

fn run_admitted_with_handoff(
    server: UnixStream,
    host: &mut HostBrokerConnection,
    worker_id: WorkerId,
    generation: NonZeroU64,
    mut kernel: AuthorizedHostOperationKernel,
    aux: AdmittedAux<'_>,
    #[cfg(windows)] mut handoff: handoff::Handoff,
) -> Result<(), Box<dyn Error>> {
    #[cfg(windows)]
    let mut server = server;
    let epoch = HostEpoch::from_bytes(*host.host_epoch().as_bytes())?;
    loop {
        #[cfg(not(windows))]
        let request = server_transport::receive(&server, Some(host))?;
        #[cfg(windows)]
        let request = loop {
            if let Some(request) =
                server_transport::receive_until(&server, Some(host), handoff.deadline())?
            {
                break request;
            }
            handoff.expire(std::time::Instant::now());
        };
        #[cfg(windows)]
        handoff.expire(std::time::Instant::now());
        #[cfg(windows)]
        if let CapabilityNodeMessage::WorkerHandoffRequest(request) = request {
            handoff.execute(&mut server, Some(host), request, worker_id, generation)?;
            continue;
        }
        #[cfg(windows)]
        if handoff.is_prepared() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "ordinary request during prepared Server handoff",
            )
            .into());
        }
        if let CapabilityNodeMessage::TerminalEditorCommandRequest(request) = request {
            let result = crate::terminal_editor_operation::execute(request, worker_id, generation);
            if server_transport::send(
                &server,
                Some(host),
                &CapabilityNodeMessage::TerminalEditorCommandResult(result),
            )? {
                return Err(peer_closed().into());
            }
            continue;
        }
        if let CapabilityNodeMessage::HostQueryRequest(request) = request {
            let result = host_query::execute(Some(host), request, worker_id, generation);
            if server_transport::send(
                &server,
                Some(host),
                &CapabilityNodeMessage::HostQueryResult(result),
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
                    &server,
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
                &server,
                request,
                worker_id,
                generation,
                background_jobs::BackgroundJobs {
                    discovery: aux.worktree_slot,
                    integrations: aux.agent_integrations,
                    probes: aux.agent_probes,
                    mutations: aux.mutation_slot,
                    plugins: aux.plugin_jobs,
                },
                aux.managed_host.map(|owner| (&*host, owner)),
            );
        }
        #[cfg(windows)]
        if let CapabilityNodeMessage::TerminalTitlesRequest(request) = request {
            let result = titles::execute(&server, Some(host), request, worker_id, generation)?;
            if server_transport::send(
                &server,
                Some(host),
                &CapabilityNodeMessage::TerminalTitlesResult(result),
            )? {
                return Err(peer_closed().into());
            }
            continue;
        }
        if let CapabilityNodeMessage::TerminalTitleChangesRequest(request) = request {
            let result = title_changes::execute(Some(host), request, worker_id, generation);
            if server_transport::send(
                &server,
                Some(host),
                &CapabilityNodeMessage::TerminalTitleChangesResult(result),
            )? {
                return Err(peer_closed().into());
            }
            continue;
        }
        if let CapabilityNodeMessage::AgentDrainRequest(request) = request {
            let host_closed = dispatch_drain_request(
                &server,
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
        if let CapabilityNodeMessage::AgentIntegrationJobRequest(request) = request {
            // Start only admits the job owner; it never waits for the writer.
            let result = aux
                .agent_integrations
                .execute(request, worker_id, generation);
            if server_transport::send(
                &server,
                Some(host),
                &CapabilityNodeMessage::AgentIntegrationJobResult(result),
            )? {
                return Err(peer_closed().into());
            }
            continue;
        }
        if let CapabilityNodeMessage::AgentProbeJobRequest(request) = request {
            // Start only admits the job owner; it never waits for the lookup.
            let result = aux.agent_probes.execute(request, worker_id, generation);
            if server_transport::send(
                &server,
                Some(host),
                &CapabilityNodeMessage::AgentProbeJobResult(result),
            )? {
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
                aux.session_runtime,
            );
            if server_transport::send(
                &server,
                Some(host),
                &CapabilityNodeMessage::PluginJobResult(result),
            )? {
                return Err(peer_closed().into());
            }
            continue;
        }
        if let CapabilityNodeMessage::WorktreeMutationRequest(request) = request {
            let result = crate::worktree_mutation_operation::execute(
                request,
                worker_id,
                generation,
                aux.mutation_slot,
            );
            if server_transport::send(
                &server,
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
                &server,
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
                &server,
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
                        &server,
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
                &server,
                (disposition == HostOperationDisposition::ResultReady).then_some(&*host),
                &CapabilityNodeMessage::TerminalLifecycleResult(result),
            )? {
                return Err(peer_closed().into());
            }
            continue;
        }
        crate::terminal_viewer_operation::cleanup_closed(host, &server)?;
        if let CapabilityNodeMessage::TerminalControlRequest(request) = request {
            let result = crate::terminal_control_operation::execute(
                request, worker_id, generation, host, &server,
            )?;
            if server_transport::send(
                &server,
                Some(host),
                &CapabilityNodeMessage::TerminalControlResult(result),
            )? {
                return Err(peer_closed().into());
            }
            continue;
        }
        if let CapabilityNodeMessage::TerminalGraphicsDecodeRequest(request) = request {
            let result = crate::terminal_viewer_operation::decode_graphics(
                request, worker_id, generation, host, &server,
            )?;
            if server_transport::send(
                &server,
                Some(host),
                &CapabilityNodeMessage::TerminalViewerResult(result),
            )? {
                return Err(peer_closed().into());
            }
            continue;
        }
        if let CapabilityNodeMessage::TerminalViewerRequest(request) = request {
            let result = crate::terminal_viewer_operation::execute(
                request, worker_id, generation, host, &server,
            )?;
            if server_transport::send(
                &server,
                Some(host),
                &CapabilityNodeMessage::TerminalViewerResult(result),
            )? {
                return Err(peer_closed().into());
            }
            continue;
        }
        if let CapabilityNodeMessage::TerminalSendRequest(request) = request {
            let execution = crate::terminal_send_operation::execute(
                request, worker_id, generation, host, &server,
            )?;
            // A failed Host carrier still gets its result written; that write
            // is the last record this session produces and the loop exits
            // explicitly instead of discovering the dead stream later.
            let host_closed = server_transport::send(
                &server,
                (!execution.host_failed).then_some(&*host),
                &CapabilityNodeMessage::TerminalSendResult(execution.result),
            )?;
            if host_closed || execution.host_failed {
                return Err(peer_closed().into());
            }
            continue;
        }
        let now = unix_milliseconds();
        let mut output_seed = None;
        let admission = match request {
            CapabilityNodeMessage::AuthorizedTerminalLaunch(mut launch) => {
                // The kernel retains its launch for replay; the earlier output travels apart.
                output_seed = launch.take_output_seed();
                kernel.admit_launch(launch, now)?
            }
            CapabilityNodeMessage::AuthorizedTerminalStart(start) => {
                kernel.admit_start(start, now, epoch)?
            }
            CapabilityNodeMessage::WorkerHandoffRequest(_)
            | CapabilityNodeMessage::WorkerHandoffResult(_)
            | CapabilityNodeMessage::WorkerHandoffHello(_)
            | CapabilityNodeMessage::WorkerHandoffReady(_)
            | CapabilityNodeMessage::WorkerHandoffOwned(_)
            | CapabilityNodeMessage::WorkerShutdownRequest(_)
            | CapabilityNodeMessage::WorkerShutdownResult(_)
            | CapabilityNodeMessage::AgentDrainRequest(_)
            | CapabilityNodeMessage::AgentDrainReply(_)
            | CapabilityNodeMessage::TerminalTitlesRequest(_)
            | CapabilityNodeMessage::TerminalTitleChangesRequest(_)
            | CapabilityNodeMessage::TerminalEditorCommandRequest(_)
            | CapabilityNodeMessage::TerminalEditorCommandResult(_)
            | CapabilityNodeMessage::TerminalTitlesResult(_)
            | CapabilityNodeMessage::TerminalTitlesChanged(_)
            | CapabilityNodeMessage::TerminalTitleChangesResult(_)
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
            | CapabilityNodeMessage::TerminalGraphicsDecodeRequest(_)
            | CapabilityNodeMessage::TerminalViewerRequest(_)
            | CapabilityNodeMessage::TerminalViewerResult(_)
            | CapabilityNodeMessage::TerminalViewerRecordReady(_)
            | CapabilityNodeMessage::WorktreeJobRequest(_)
            | CapabilityNodeMessage::WorktreeJobResult(_)
            | CapabilityNodeMessage::WorktreeMutationRequest(_)
            | CapabilityNodeMessage::WorktreeMutationResult(_)
            | CapabilityNodeMessage::PluginJobRequest(_)
            | CapabilityNodeMessage::PluginJobResult(_)
            | CapabilityNodeMessage::AgentIntegrationJobRequest(_)
            | CapabilityNodeMessage::AgentIntegrationJobResult(_)
            | CapabilityNodeMessage::AgentProbeJobRequest(_)
            | CapabilityNodeMessage::AgentProbeJobResult(_)
            | CapabilityNodeMessage::HostQueryRequest(_)
            | CapabilityNodeMessage::HostQueryResult(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Server sent a Worker-to-Server result",
                )
                .into());
            }
        };
        let observed_colors = host.host_colors.borrow().latest();
        let disposition = match admission {
            HostOperationAdmission::Dispatch(dispatch) => {
                match prepare_or_fail(
                    &mut kernel,
                    &dispatch,
                    aux.session_runtime,
                    observed_colors,
                    aux.system_shell,
                )?
                .map(|prepared| prepared.with_output_seed(output_seed))
                {
                    Some(prepared) => prepared.execute(
                        &mut kernel,
                        host,
                        &server,
                        unix_milliseconds,
                        HOST_OPERATION_BUDGET,
                    )?,
                    None => HostOperationDisposition::ResultReady,
                }
            }
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
            &server,
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
    session_runtime: Option<&std::path::Path>,
    observed_colors: Option<orchbee_local_runtime::UserTerminalColors>,
    system_shell: &SystemShell,
) -> Result<Option<PreparedHostOperation>, Box<dyn Error>> {
    match prepare(dispatch, session_runtime, observed_colors, system_shell) {
        Ok(prepared) => Ok(Some(prepared)),
        Err(_) => {
            kernel.record_pre_handoff_failure(dispatch, unix_milliseconds())?;
            Ok(None)
        }
    }
}

fn prepare(
    dispatch: &AuthorizedHostDispatch,
    session_runtime: Option<&std::path::Path>,
    observed_colors: Option<orchbee_local_runtime::UserTerminalColors>,
    system_shell: &SystemShell,
) -> Result<PreparedHostOperation, Box<dyn Error>> {
    let correlation = CorrelationId::from_bytes(crate::worker_identity::fresh_identity()?)?;
    match dispatch.operation() {
        AuthorizedHostOperation::Launch(launch) => {
            // Admission has already retired duplicates. The Session captured this creation's
            // preferences before dispatch; an unavailable capture fails before any effect.
            let preferences = match launch.preferences() {
                LaunchPreferences::Absent => None,
                LaunchPreferences::Unavailable => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Shared terminal launch preferences are unavailable",
                    )
                    .into());
                }
                LaunchPreferences::Available(preferences) => Some(preferences),
            };
            #[cfg(not(windows))]
            if launch.editor_source().is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "Windows editor launch is unsupported",
                )
                .into());
            }
            // A Host this Worker started writes the document to the folder it was given; an
            // explicit-endpoint Host was given none and refuses the editor itself.
            #[cfg(windows)]
            if launch.editor_source().is_some() && session_runtime.is_some() {
                crate::editor_documents::prepare_launch()?;
            }
            // Host display mode follows the most recent Client's outer terminal, then the OS.
            let follows_host = preferences.is_none_or(|preferences| preferences.colors().is_none());
            let host_colors = if follows_host {
                observed_colors.unwrap_or_else(system_terminal_colors)
            } else {
                orchbee_local_runtime::UserTerminalColors::LIGHT
            };
            Ok(PreparedHostOperation::launch(
                dispatch,
                launch_facts::resolve(launch, preferences, system_shell, host_colors)?,
                correlation,
                session_runtime,
            )?)
        }
        AuthorizedHostOperation::Start(_) => {
            Ok(PreparedHostOperation::start(dispatch, correlation)?)
        }
    }
}

fn system_terminal_colors() -> orchbee_local_runtime::UserTerminalColors {
    #[cfg(windows)]
    if orchbee_local_runtime::read_windows_apps_use_light_theme() == Some(false) {
        return orchbee_local_runtime::UserTerminalColors::DARK;
    }
    orchbee_local_runtime::UserTerminalColors::LIGHT
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
