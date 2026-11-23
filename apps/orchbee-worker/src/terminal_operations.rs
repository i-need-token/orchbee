//! Serializes admitted Server requests across the authorization kernel, the Hosts the Worker holds,
//! retained jobs, and managed lifecycle operations. A request goes to the Host that serves what it
//! names, and a launch to the Worker's own Host. This loop preserves one physical operation owner
//! without deciding Workbench membership or implementing PTY and VT behavior.

use orchbee_local_ipc::LocalStream as UnixStream;
use std::{
    cell::RefCell,
    error::Error,
    io,
    num::NonZeroU64,
    rc::Rc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use orchbee_capability_node_protocol::{
    CapabilityNodeMessage, DeclinedRequestKind, HostEpoch, LaunchPreferences,
    TerminalMoveTurnResult, TerminalViewerAction, WorkerRequestDeclined,
};
use orchbee_local_worker_protocol::WorkerId;
use orchbee_terminal_protocol::CorrelationId;

use crate::{
    agent_drain::AgentDrain,
    default_interactive_launch::SystemShell,
    diagnostic_events::{DeclineCause, Site},
    host_connection::HostBrokerConnection,
    terminal_authorization::{
        AuthorizedHostDispatch, AuthorizedHostOperation, AuthorizedHostOperationKernel,
        AuthorizedHostOperationResult, HostOperationAdmission,
    },
    terminal_host_operation::{HostOperationDisposition, PreparedHostOperation},
};

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
mod adopt;
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
pub(crate) use adopt::run as run_adopt;
mod background_jobs;
mod exit_cause;
mod host_query;
mod host_set;
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
mod host_start;
mod launch_facts;
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
mod lazy;
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
pub(crate) use lazy::run as run_lazy;
#[cfg(windows)]
mod handoff;
#[cfg(any(windows, target_os = "macos"))]
mod keeper_care;
#[cfg(windows)]
mod moves;
#[cfg(windows)]
mod recovery;
mod server_transport;
mod shutdown;
mod title_changes;
mod titles;

#[cfg(test)]
mod tests;

pub(crate) fn run(
    server: &UnixStream,
    host: HostBrokerConnection,
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
            plugin_jobs: &plugin_jobs,
            session_runtime: None,
        },
    );
    background_jobs::BackgroundJobs {
        discovery: &worktree_slot,
        integrations: &agent_integrations,
        probes: &agent_probes,
        mutations: &mutation_slot,
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
    pub(crate) plugin_jobs: &'a crate::plugin_jobs::PluginJobs,
    pub(crate) session_runtime: Option<&'a std::path::Path>,
}

/// Owns the admitted Server wire and preserves its physical owners across an explicit handoff.
pub(crate) fn run_admitted(
    server: UnixStream,
    host: HostBrokerConnection,
    worker_id: WorkerId,
    generation: NonZeroU64,
    kernel: AuthorizedHostOperationKernel,
    aux: AdmittedAux<'_>,
) -> Result<(), Box<dyn Error>> {
    #[cfg(windows)]
    let handoff = handoff::Handoff::new(aux.session_runtime);
    run_admitted_with_handoff(
        server,
        host_set::HostSet::unmanaged(host),
        worker_id,
        generation,
        kernel,
        aux,
        #[cfg(windows)]
        handoff,
    )
}

/// Serves the admitted connection and attaches the classified cause to the error that ends it,
/// named for the Host whose failure ended it. On Windows a Host that left an exchange unanswered
/// for its whole bound is checked for a hang before the Worker exits, so a hang a request met first
/// is recovered as one the care found.
fn run_admitted_with_handoff(
    server: UnixStream,
    mut hosts: host_set::HostSet<'_>,
    worker_id: WorkerId,
    generation: NonZeroU64,
    kernel: AuthorizedHostOperationKernel,
    aux: AdmittedAux<'_>,
    #[cfg(windows)] handoff: handoff::Handoff,
) -> Result<(), Box<dyn Error>> {
    let outcome = serve(
        server,
        &mut hosts,
        worker_id,
        generation,
        kernel,
        aux,
        #[cfg(windows)]
        handoff,
    );
    let failed = hosts.failed();
    #[cfg(windows)]
    if let Err(error) = &outcome
        && exit_cause::host_unanswered(error.as_ref(), hosts.connection(failed))
    {
        hosts.after_unanswered(failed);
    }
    outcome.map_err(|error| exit_cause::attach(error, hosts.connection(failed)))
}

/// Each Server request bounds its Host exchanges from when its bytes first became readable
/// ([`Pacing`]), and names the site its failure would be recorded at. Cleanup of what the
/// connection no longer uses runs after the request's own work, so it never takes the request's
/// room, and before the result, while the Server sends nothing.
///
/// A request that names a terminal goes to the Host that serves its epoch, one that names an
/// attachment to the Host that holds it, a launch to the Worker's own Host, and a request that
/// reads every Host to each that still serves. Between requests the loop also keeps the Keeper's
/// care on Windows and macOS, and the explicit handoff on Windows.
///
/// [`Pacing`]: crate::host_connection::Pacing
fn serve(
    server: UnixStream,
    hosts: &mut host_set::HostSet<'_>,
    worker_id: WorkerId,
    generation: NonZeroU64,
    mut kernel: AuthorizedHostOperationKernel,
    aux: AdmittedAux<'_>,
    #[cfg(windows)] mut handoff: handoff::Handoff,
) -> Result<(), Box<dyn Error>> {
    #[cfg(windows)]
    let mut server = server;
    #[cfg(windows)]
    let mut moving = moves::Moves::default();
    loop {
        hosts.end_request();
        let (request, readable) = loop {
            // The Keeper's care runs between requests, never under a prepared handoff, so nothing
            // changes under one. A request that arrived while it ran waited for it, so that
            // request's bound runs from when the care began.
            #[cfg(windows)]
            let (wake, cared) = if handoff.is_prepared() {
                (handoff.deadline(), None)
            } else if hosts.is_managed() {
                let cared = hosts.care(&server);
                (Some(hosts.care_due()), cared)
            } else {
                (None, None)
            };
            // A macOS Worker takes no handoff.
            #[cfg(target_os = "macos")]
            let (wake, cared) = if hosts.is_managed() {
                let cared: Option<Instant> = hosts.care(&server);
                (Some(hosts.care_due()), cared)
            } else {
                (None, None)
            };
            #[cfg(not(any(windows, target_os = "macos")))]
            let (wake, cared) = (None, None::<Instant>);
            if let Some((request, readable)) = hosts.receive(&server, wake)? {
                break (request, cared.map_or(readable, |began| readable.min(began)));
            }
            #[cfg(windows)]
            handoff.expire(Instant::now());
        };
        hosts.begin_request_at(readable);
        #[cfg(windows)]
        handoff.expire(Instant::now());
        #[cfg(windows)]
        if let CapabilityNodeMessage::WorkerHandoffRequest(request) = request {
            let host = hosts.primary();
            host.pacing.set_site(Site::Handoff);
            let attached = hosts.serving().iter().any(|host| {
                !host.viewers.borrow().is_empty() || !host.controllers.borrow().is_empty()
            });
            handoff.execute(
                &mut server,
                Some(host),
                attached,
                request,
                worker_id,
                generation,
            )?;
            continue;
        }
        // A prepared handoff hands this connection to another Server: an ordinary request now is
        // declined, so nothing changes under the handoff, and the connection serves on.
        #[cfg(windows)]
        if handoff.is_prepared() {
            decline(
                &server,
                Some(hosts.primary()),
                &request,
                DeclineCause::Handoff,
            )?;
            continue;
        }
        // A request the Server wrote before it recorded a move runs against the terminal as it
        // runs now.
        #[cfg(windows)]
        let mut request = request;
        #[cfg(windows)]
        let named = moving.before(&mut request, hosts);
        if let CapabilityNodeMessage::TerminalMoveTurnRequest(turn) = request {
            if turn.worker_id.as_bytes() != worker_id.as_bytes()
                || turn.worker_generation.get() != generation.get()
            {
                decline(
                    &server,
                    Some(hosts.primary()),
                    &request,
                    DeclineCause::Fence,
                )?;
                continue;
            }
            #[cfg(windows)]
            let outcome = {
                hosts.set_site(Site::Move);
                moving.turn(hosts, &mut kernel, &server, readable)?
            };
            #[cfg(not(windows))]
            let outcome = orchbee_capability_node_protocol::TerminalMoveTurnOutcome::Done;
            let result = TerminalMoveTurnResult {
                request: turn,
                outcome,
            };
            clean_and_send(
                &server,
                hosts.primary(),
                &CapabilityNodeMessage::TerminalMoveTurnResult(result),
            )?;
            continue;
        }
        let primary = hosts.primary();
        if let CapabilityNodeMessage::HostQueryRequest(request) = request {
            let result = host_query::execute(&hosts.serving(), request, worker_id, generation);
            answer(
                &server,
                primary,
                &CapabilityNodeMessage::HostQueryResult(result),
            )?;
            continue;
        }
        if let CapabilityNodeMessage::WorkerShutdownRequest(request) = request {
            hosts.set_site(Site::Shutdown);
            if let Some(reason) =
                shutdown::refusal(request, worker_id, generation, hosts.is_managed())
            {
                answer(
                    &server,
                    hosts.primary(),
                    &CapabilityNodeMessage::WorkerShutdownResult(
                        orchbee_capability_node_protocol::WorkerShutdownResult {
                            request,
                            outcome:
                                orchbee_capability_node_protocol::WorkerShutdownOutcome::Refused(
                                    reason,
                                ),
                        },
                    ),
                )?;
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
                hosts.stop_hosts(),
            );
        }
        if let CapabilityNodeMessage::TerminalTitlesRequest(request) = request {
            hosts.set_site(Site::Titles);
            let result =
                titles::execute(&server, &hosts.serving(), request, worker_id, generation)?;
            answer(
                &server,
                primary,
                &CapabilityNodeMessage::TerminalTitlesResult(result),
            )?;
            continue;
        }
        if let CapabilityNodeMessage::TerminalTitleChangesRequest(request) = request {
            let result = title_changes::execute(Some(primary), request, worker_id, generation);
            answer(
                &server,
                primary,
                &CapabilityNodeMessage::TerminalTitleChangesResult(result),
            )?;
            continue;
        }
        if let CapabilityNodeMessage::AgentDrainRequest(request) = request {
            primary.pacing.set_site(Site::Drain);
            let host_closed = dispatch_drain_request(
                &server,
                Some(primary),
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
            answer(
                &server,
                primary,
                &CapabilityNodeMessage::AgentIntegrationJobResult(result),
            )?;
            continue;
        }
        if let CapabilityNodeMessage::AgentProbeJobRequest(request) = request {
            // Start only admits the job owner; it never waits for the lookup.
            let result = aux.agent_probes.execute(request, worker_id, generation);
            answer(
                &server,
                primary,
                &CapabilityNodeMessage::AgentProbeJobResult(result),
            )?;
            continue;
        }
        if let CapabilityNodeMessage::PluginJobRequest(request) = request {
            // A program starts on the Worker's own Host and is reached on the Host that runs it.
            let programs = hosts.programs(&server);
            let result = crate::plugin_job_operation::execute(
                request,
                worker_id,
                generation,
                aux.plugin_jobs,
                aux.session_runtime,
                programs
                    .as_ref()
                    .map(|programs| programs as &dyn crate::plugin_jobs::ProgramHost),
            );
            answer(
                &server,
                primary,
                &CapabilityNodeMessage::PluginJobResult(result),
            )?;
            continue;
        }
        if let CapabilityNodeMessage::WorktreeMutationRequest(request) = request {
            let result = crate::worktree_mutation_operation::execute(
                request,
                worker_id,
                generation,
                aux.mutation_slot,
            );
            answer(
                &server,
                primary,
                &CapabilityNodeMessage::WorktreeMutationResult(result),
            )?;
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
            answer(
                &server,
                primary,
                &CapabilityNodeMessage::WorktreeJobResult(result),
            )?;
            continue;
        }
        if let CapabilityNodeMessage::TerminalCwdValidationRequest(request) = request {
            let result = crate::terminal_cwd_validation::validate(request, worker_id, generation);
            answer(
                &server,
                primary,
                &CapabilityNodeMessage::TerminalCwdValidationResult(result),
            )?;
            continue;
        }
        if let CapabilityNodeMessage::TerminalLifecycleRequest(request) = request {
            use crate::terminal_authorization::LifecycleAdmission;
            let host = hosts.connection(hosts.by_epoch(request.target.host_epoch()));
            let epoch = HostEpoch::from_bytes(*host.host_epoch().as_bytes())?;
            host.pacing.set_site(Site::Lifecycle);
            let declined = WorkerRequestDeclined::operation(
                DeclinedRequestKind::Lifecycle,
                request.request_id,
            );
            let disposition = match kernel.admit_lifecycle(request, unix_milliseconds(), epoch) {
                Ok(LifecycleAdmission::Dispatch(dispatch)) => {
                    match crate::terminal_lifecycle_operation::PreparedLifecycleOperation::new(
                        dispatch, host,
                    ) {
                        Ok(prepared) => prepared.execute(
                            &mut kernel,
                            host,
                            &server,
                            unix_milliseconds,
                            host.pacing.exchange_bound(),
                        )?,
                        // A request this Worker cannot put into a Host record reaches no Host.
                        Err(error) => {
                            crate::diagnostic_events::request_unprepared(
                                Site::Lifecycle,
                                error.as_ref(),
                            );
                            kernel.lifecycle_pre_handoff_failure(dispatch, unix_milliseconds())?;
                            HostOperationDisposition::ResultReady
                        }
                    }
                }
                Ok(LifecycleAdmission::ResultReady) => HostOperationDisposition::ResultReady,
                // The kernel refused it before any effect, so only this request is declined.
                Ok(LifecycleAdmission::SlotOccupied) | Err(_) => {
                    send_decline(&server, Some(host), declined, DeclineCause::State)?;
                    continue;
                }
            };
            if disposition == HostOperationDisposition::ServerClosed {
                return Err(peer_closed().into());
            }
            // Every dispatched path above settles the slot before it returns; the loop is
            // serialized and Host events never touch the kernel, so a missing result is this
            // Worker's own broken invariant.
            let result = kernel
                .take_lifecycle_result()
                .ok_or_else(|| io::Error::other("Worker lifecycle result is missing"))?;
            #[cfg(windows)]
            let result = moves::lifecycle_answer(result, named.as_ref())?;
            // A retired terminal is never named again, so its cleanup cell frees for another Close.
            if result.outcome()
                == orchbee_capability_node_protocol::TerminalLifecycleOutcome::Retired
            {
                host.forget_retired_target(request.target);
            }
            if server_transport::send(
                &server,
                (disposition == HostOperationDisposition::ResultReady).then_some(host),
                &CapabilityNodeMessage::TerminalLifecycleResult(result),
            )? {
                return Err(peer_closed().into());
            }
            continue;
        }
        if let CapabilityNodeMessage::TerminalControlRequest(request) = request {
            let host = hosts.connection(hosts.by_attachment(request.viewer_id));
            host.pacing.set_site(Site::Control);
            let result = crate::terminal_control_operation::execute(
                request, worker_id, generation, host, &server,
            )?;
            clean_and_send(
                &server,
                host,
                &CapabilityNodeMessage::TerminalControlResult(result),
            )?;
            continue;
        }
        if let CapabilityNodeMessage::TerminalGraphicsDecodeRequest(request) = request {
            let host = hosts.connection(hosts.by_attachment(request.attachment_id));
            host.pacing.set_site(Site::Graphics);
            let result = crate::terminal_viewer_operation::decode_graphics(
                request, worker_id, generation, host, &server,
            )?;
            clean_and_send(
                &server,
                host,
                &CapabilityNodeMessage::TerminalViewerResult(result),
            )?;
            continue;
        }
        if let CapabilityNodeMessage::TerminalViewerRequest(request) = request {
            let index = match &request.action {
                TerminalViewerAction::Attach { host_epoch, .. } => hosts.by_epoch(*host_epoch),
                TerminalViewerAction::ReadLifecycle { target } => {
                    hosts.by_epoch(target.host_epoch())
                }
                _ => hosts.by_attachment(request.attachment_id),
            };
            let host = hosts.connection(index);
            host.pacing.set_site(Site::Viewer);
            #[cfg_attr(
                not(windows),
                expect(unused_mut, reason = "only moves rewrite an answer")
            )]
            let mut result = crate::terminal_viewer_operation::execute(
                request, worker_id, generation, host, &server,
            )?;
            #[cfg(windows)]
            moves::viewer_answer(&mut result.outcome, named.as_ref());
            clean_and_send(
                &server,
                host,
                &CapabilityNodeMessage::TerminalViewerResult(result),
            )?;
            continue;
        }
        if let CapabilityNodeMessage::TerminalSendRequest(request) = request {
            let host = hosts.connection(hosts.by_epoch(request.target.host_epoch()));
            host.pacing.set_site(Site::Send);
            let execution = crate::terminal_send_operation::execute(
                request, worker_id, generation, host, &server,
            )?;
            let result = CapabilityNodeMessage::TerminalSendResult(execution.result);
            if execution.host_failed {
                // A failed Host carrier still gets its result written; that write
                // is the last record this session produces and the loop exits
                // explicitly instead of discovering the dead stream later.
                let _host_closed = server_transport::send(&server, None, &result)?;
                return Err(peer_closed().into());
            }
            clean_and_send(&server, host, &result)?;
            continue;
        }
        let now = unix_milliseconds();
        let mut output_seed = None;
        // A Start goes to the Host that serves its terminal; a Launch finds its Host once admitted.
        let (admission, declined, site, routed) = match request {
            CapabilityNodeMessage::AuthorizedTerminalLaunch(mut launch) => {
                let declined = WorkerRequestDeclined::operation(
                    DeclinedRequestKind::Launch,
                    launch.authorization().operation_id(),
                );
                // The kernel retains its launch for replay; the earlier output travels apart.
                output_seed = launch.take_output_seed();
                (
                    kernel.admit_launch(launch, now),
                    declined,
                    Site::Launch,
                    None,
                )
            }
            CapabilityNodeMessage::AuthorizedTerminalStart(start) => {
                let declined = WorkerRequestDeclined::operation(
                    DeclinedRequestKind::Start,
                    start.authorization().operation_id(),
                );
                let index = hosts.by_epoch(start.identity().host_epoch());
                let epoch =
                    HostEpoch::from_bytes(*hosts.connection(index).host_epoch().as_bytes())?;
                (
                    kernel.admit_start(start, now, epoch),
                    declined,
                    Site::Start,
                    Some(index),
                )
            }
            // Every request served above has continued; what remains is declined.
            other => {
                decline(&server, Some(primary), &other, DeclineCause::Unserved)?;
                continue;
            }
        };
        let mut answered = routed.unwrap_or_else(|| hosts.primary_index());
        hosts.connection(answered).pacing.set_site(site);
        let disposition = match admission {
            Ok(HostOperationAdmission::Dispatch(dispatch)) => {
                // A new terminal runs on the Worker's own Host, which starts here when it holds
                // none; a start that fails fails only this launch.
                let routed = match routed {
                    Some(index) => Ok(index),
                    None => hosts.own_or_start(readable),
                };
                match routed {
                    Err(error) => {
                        crate::diagnostic_events::request_unprepared(site, error.as_ref());
                        kernel.record_pre_handoff_failure(&dispatch, unix_milliseconds())?;
                        HostOperationDisposition::ResultReady
                    }
                    Ok(index) => {
                        answered = index;
                        let host = hosts.connection(index);
                        host.pacing.set_site(site);
                        let bound = host.pacing.exchange_bound();
                        match prepare_or_fail(
                            &mut kernel,
                            &dispatch,
                            site,
                            aux.session_runtime,
                            hosts.observed_colors(),
                            aux.system_shell,
                        )?
                        .map(|prepared| prepared.with_output_seed(output_seed))
                        {
                            Some(prepared) => prepared.execute(
                                &mut kernel,
                                host,
                                &server,
                                unix_milliseconds,
                                bound,
                            )?,
                            None => HostOperationDisposition::ResultReady,
                        }
                    }
                }
            }
            Ok(HostOperationAdmission::ResultReady) => HostOperationDisposition::ResultReady,
            // The kernel refused it before any effect, so only this request is declined.
            Ok(HostOperationAdmission::SlotOccupied) | Err(_) => {
                send_decline(
                    &server,
                    Some(hosts.connection(answered)),
                    declined,
                    DeclineCause::State,
                )?;
                continue;
            }
        };
        if disposition == HostOperationDisposition::ServerClosed {
            return Err(peer_closed().into());
        }
        let result = take_result(&mut kernel)?;
        // No next admission occurs while this exact result is being delivered. A
        // failed write abandons the process; it never causes a Host retry.
        if disposition == HostOperationDisposition::ResultReady {
            clean_and_send(&server, hosts.connection(answered), &result)?;
        } else {
            let _host_closed = server_transport::send(&server, None, &result)?;
            return Err(peer_closed().into());
        }
    }
}

/// Delivers `result` while reading `host`; a Host that ended meanwhile ends the connection.
fn answer(
    server: &UnixStream,
    host: &HostBrokerConnection,
    result: &CapabilityNodeMessage,
) -> Result<(), Box<dyn Error>> {
    if server_transport::send(server, Some(host), result)? {
        return Err(peer_closed().into());
    }
    Ok(())
}

/// Answers a whole Server request this Worker will not serve with a decline, records its `cause`,
/// and keeps serving, so one request without a branch, or one this Worker refuses before any
/// effect, never ends the Worker and its terminals. A message only a Worker sends answers no Server
/// request, so it is dropped unanswered and recorded.
fn decline(
    server: &UnixStream,
    host: Option<&HostBrokerConnection>,
    request: &CapabilityNodeMessage,
    cause: DeclineCause,
) -> Result<(), Box<dyn Error>> {
    let Some(declined) = WorkerRequestDeclined::of(request) else {
        crate::diagnostic_events::server_unexpected(
            crate::diagnostic_events::ServerUnexpected::WorkerOnly,
        );
        return Ok(());
    };
    send_decline(server, host, declined, cause)
}

/// Sends `declined`, which tells the Server that this Worker did nothing for that request.
fn send_decline(
    server: &UnixStream,
    host: Option<&HostBrokerConnection>,
    declined: WorkerRequestDeclined,
    cause: DeclineCause,
) -> Result<(), Box<dyn Error>> {
    crate::diagnostic_events::request_declined(declined.kind, cause);
    let host_closed = server_transport::send(
        server,
        host,
        &CapabilityNodeMessage::WorkerRequestDeclined(declined),
    )?;
    // Before the first Launch there is no Host, and the send reports it as closed.
    if host_closed && host.is_some() {
        return Err(peer_closed().into());
    }
    Ok(())
}

/// Runs cleanup after a request's own work, then delivers its result. A cleanup that lost the
/// Host still delivers the result before the connection ends with that failure.
fn clean_and_send(
    server: &UnixStream,
    host: &HostBrokerConnection,
    result: &CapabilityNodeMessage,
) -> Result<(), Box<dyn Error>> {
    let cleaned = crate::terminal_viewer_operation::cleanup(host, server);
    let host_closed = server_transport::send(server, cleaned.is_ok().then_some(host), result)?;
    cleaned?;
    if host_closed {
        return Err(peer_closed().into());
    }
    Ok(())
}

/// One Agent Drain request dispatched on the serialized Server connection.
/// Shared by the pre-Host lazy loop and the admitted loop: the queue state is
/// the same object either way. A request fenced to another Worker or
/// generation, or one acknowledging more than this Worker offered, is declined
/// before it changes the queue. Returns the send host-closed flag: both `Ok` values prove the
/// reply was completely written, so the captured watermark commits first and
/// the caller decides whether a closed Host retires the connection.
pub(super) fn dispatch_drain_request(
    server: &UnixStream,
    host: Option<&HostBrokerConnection>,
    drain: &RefCell<AgentDrain>,
    request: &orchbee_capability_node_protocol::AgentDrainRequest,
    worker_id: WorkerId,
    generation: NonZeroU64,
) -> Result<bool, Box<dyn Error>> {
    let cause = if request.worker_id.as_bytes() != worker_id.as_bytes()
        || request.worker_generation.get() != generation.get()
    {
        Some(DeclineCause::Fence)
    } else if drain.borrow().acknowledges_unoffered(request.acknowledged) {
        Some(DeclineCause::Watermark)
    } else {
        None
    };
    if let Some(cause) = cause {
        let declined = WorkerRequestDeclined::agent_drain(request.token, request.acknowledged);
        crate::diagnostic_events::request_declined(declined.kind, cause);
        return Ok(server_transport::send(
            server,
            host,
            &CapabilityNodeMessage::WorkerRequestDeclined(declined),
        )?);
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

/// The result of the request just served. Every dispatched path settles the slot before it
/// returns; the loop is serialized and Host events never touch the kernel, so a missing result is
/// this Worker's own broken invariant, which ends it.
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

/// Prepares a dispatched Launch or Start, or fails it before handoff with a record at `site` when
/// this Worker cannot: nothing has reached the Host, so only this request fails.
fn prepare_or_fail(
    kernel: &mut AuthorizedHostOperationKernel,
    dispatch: &AuthorizedHostDispatch,
    site: Site,
    session_runtime: Option<&std::path::Path>,
    observed_colors: Option<orchbee_local_runtime::UserTerminalColors>,
    system_shell: &SystemShell,
) -> Result<Option<PreparedHostOperation>, Box<dyn Error>> {
    match prepare(dispatch, session_runtime, observed_colors, system_shell) {
        Ok(prepared) => Ok(Some(prepared)),
        Err(error) => {
            crate::diagnostic_events::request_unprepared(site, error.as_ref());
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
