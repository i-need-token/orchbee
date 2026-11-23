//! Owns the managed lazy-Host loop, starting the verified Host only after an admitted launch
//! and retaining its child and connection for subsequent operations. It does not adopt an
//! existing Host, which only a replacement Worker's adoption does, or replay a launch whose
//! delivery is uncertain.

use super::host_start::{HOST_START_ATTEMPTS, HostStart, start_host};
use super::{server_transport, unix_milliseconds};
use crate::{
    agent_drain::AgentDrain,
    default_interactive_launch::SystemShell,
    diagnostic_events::{DeclineCause, Site},
    terminal_authorization::{AuthorizedHostOperationKernel, HostOperationAdmission},
    terminal_host_operation::HostOperationDisposition,
};
use orchbee_capability_node_protocol::{
    CapabilityNodeMessage, DeclinedRequestKind, WorkerRequestDeclined,
};
use orchbee_local_ipc::LocalStream as UnixStream;
use orchbee_local_runtime::Cohort;
use orchbee_local_worker_protocol::WorkerId;
use std::{cell::RefCell, error::Error, num::NonZeroU64, path::Path, rc::Rc};

pub(crate) fn run(
    server: UnixStream,
    worker_id: WorkerId,
    generation: NonZeroU64,
    cohort: &Cohort,
    runtime: &Path,
    system_shell: &SystemShell,
) -> Result<(), Box<dyn Error>> {
    #[cfg(windows)]
    let mut server = server;
    #[cfg(windows)]
    let mut handoff = super::handoff::Handoff::new(Some(runtime));
    server.set_nonblocking(true)?;
    // The Agent Drain queue is owned by this admitted Server connection and
    // exists before any Host: pre-Launch requests get a truthful empty page,
    // and the same watermarks survive into the admitted loop unchanged.
    let drain = Rc::new(RefCell::new(AgentDrain::new()));
    let mut kernel = AuthorizedHostOperationKernel::new(worker_id, generation)?;
    // The discovery slot outlives the pre-Host phase: jobs admitted here stay
    // owned across the first Launch instead of being orphaned at handoff.
    let worktree_slot = crate::worktree_discovery_job::WorktreeDiscoverySlot::new();
    let agent_integrations = crate::agent_integration_jobs::AgentIntegrationJobs::new();
    let agent_probes = crate::agent_probe_jobs::AgentProbeJobs::new();
    let plugin_jobs = crate::plugin_jobs::PluginJobs::new();
    let mutation_slot =
        crate::worktree_mutation_job::WorktreeMutationSlot::new(crate::worktree_mutation_git::run);
    // Where the pre-Host loop was when it ended; the admitted loop classifies its own end.
    let site = crate::diagnostic_events::ServingSite::new(Site::ServerReceive);
    // Host starts that failed so far; every one is consecutive, since a started Host leaves this loop.
    let mut failed_starts = 0_u32;
    let outcome = (|| {
        loop {
            site.set(Site::ServerReceive);
            #[cfg(not(windows))]
            let (request, arrived) = server_transport::receive_timed(&server, None)?;
            #[cfg(windows)]
            let (request, arrived) = loop {
                if let Some(received) =
                    server_transport::receive_until_timed(&server, None, handoff.deadline())?
                {
                    break received;
                }
                handoff.expire(std::time::Instant::now());
            };
            #[cfg(windows)]
            handoff.expire(std::time::Instant::now());
            #[cfg(windows)]
            if let CapabilityNodeMessage::WorkerHandoffRequest(request) = request {
                site.set(Site::Handoff);
                handoff.execute(&mut server, None, false, request, worker_id, generation)?;
                continue;
            }
            // As in the admitted loop, an ordinary request under a prepared handoff is declined.
            #[cfg(windows)]
            if handoff.is_prepared() {
                super::decline(&server, None, &request, DeclineCause::Handoff)?;
                continue;
            }
            if let CapabilityNodeMessage::TerminalGraphicsDecodeRequest(request) = request {
                let refusal = if request.worker_id.as_bytes() != worker_id.as_bytes() {
                    orchbee_capability_node_protocol::ViewerRejection::WrongWorker
                } else if request.worker_generation.get() != generation.get() {
                    orchbee_capability_node_protocol::ViewerRejection::StaleWorkerGeneration
                } else {
                    orchbee_capability_node_protocol::ViewerRejection::AttachmentUnavailable
                };
                let _no_host = server_transport::send(
                    &server,
                    None,
                    &CapabilityNodeMessage::TerminalViewerResult(
                        orchbee_capability_node_protocol::TerminalViewerResult {
                            request_id: request.request_id,
                            attachment_id: request.attachment_id,
                            outcome:
                                orchbee_capability_node_protocol::TerminalViewerOutcome::Rejected(
                                    refusal,
                                ),
                        },
                    ),
                )?;
                continue;
            }
            if let CapabilityNodeMessage::HostQueryRequest(request) = request {
                // No Host exists before the first Launch starts one.
                let result = super::host_query::execute(&[], request, worker_id, generation);
                let _no_host = server_transport::send(
                    &server,
                    None,
                    &CapabilityNodeMessage::HostQueryResult(result),
                )?;
                continue;
            }
            if let CapabilityNodeMessage::WorkerShutdownRequest(request) = request {
                site.set(Site::Shutdown);
                if let Some(reason) = super::shutdown::refusal(request, worker_id, generation, true)
                {
                    let _no_host = server_transport::send(
                        &server,
                        None,
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
                return super::shutdown::run(
                    &server,
                    request,
                    worker_id,
                    generation,
                    super::background_jobs::BackgroundJobs {
                        discovery: &worktree_slot,
                        integrations: &agent_integrations,
                        probes: &agent_probes,
                        mutations: &mutation_slot,
                        plugins: &plugin_jobs,
                    },
                    None,
                );
            }
            if let CapabilityNodeMessage::TerminalTitlesRequest(request) = request {
                site.set(Site::Titles);
                let result = super::titles::execute(&server, &[], request, worker_id, generation)?;
                let _no_host = server_transport::send(
                    &server,
                    None,
                    &CapabilityNodeMessage::TerminalTitlesResult(result),
                )?;
                continue;
            }
            if let CapabilityNodeMessage::TerminalTitleChangesRequest(request) = request {
                let result = super::title_changes::execute(None, request, worker_id, generation);
                let _no_host = server_transport::send(
                    &server,
                    None,
                    &CapabilityNodeMessage::TerminalTitleChangesResult(result),
                )?;
                continue;
            }
            if let CapabilityNodeMessage::AgentDrainRequest(request) = request {
                site.set(Site::Drain);
                // Without a Host, send always reports host_closed=true; that flag
                // is expected here and does not retire the Server connection.
                let _no_host = super::dispatch_drain_request(
                    &server, None, &drain, &request, worker_id, generation,
                )?;
                continue;
            }
            if let CapabilityNodeMessage::AgentIntegrationJobRequest(request) = request {
                // Agent integration jobs need no Host; they answer directly in this loop.
                let result = agent_integrations.execute(request, worker_id, generation);
                let _no_host = server_transport::send(
                    &server,
                    None,
                    &CapabilityNodeMessage::AgentIntegrationJobResult(result),
                )?;
                continue;
            }
            if let CapabilityNodeMessage::AgentProbeJobRequest(request) = request {
                // Agent probe jobs need no Host; they answer directly in this loop.
                let result = agent_probes.execute(request, worker_id, generation);
                let _no_host = server_transport::send(
                    &server,
                    None,
                    &CapabilityNodeMessage::AgentProbeJobResult(result),
                )?;
                continue;
            }
            if let CapabilityNodeMessage::PluginJobRequest(request) = request {
                let result = crate::plugin_job_operation::execute(
                    request,
                    worker_id,
                    generation,
                    &plugin_jobs,
                    Some(runtime),
                    None,
                );
                let _no_host = server_transport::send(
                    &server,
                    None,
                    &CapabilityNodeMessage::PluginJobResult(result),
                )?;
                continue;
            }
            if let CapabilityNodeMessage::WorktreeMutationRequest(request) = request {
                let result = crate::worktree_mutation_operation::execute(
                    request,
                    worker_id,
                    generation,
                    &mutation_slot,
                );
                let _no_host = server_transport::send(
                    &server,
                    None,
                    &CapabilityNodeMessage::WorktreeMutationResult(result),
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
                    &server,
                    None,
                    &CapabilityNodeMessage::WorktreeJobResult(result),
                )?;
                continue;
            }
            if let CapabilityNodeMessage::TerminalCwdValidationRequest(request) = request {
                let result =
                    crate::terminal_cwd_validation::validate(request, worker_id, generation);
                // The send result reports Host closure, not Server closure. This pre-Host loop has no
                // Host to observe, so only a result-write error retires the Server connection.
                let _no_host = server_transport::send(
                    &server,
                    None,
                    &CapabilityNodeMessage::TerminalCwdValidationResult(result),
                )?;
                continue;
            }
            let CapabilityNodeMessage::AuthorizedTerminalLaunch(mut launch) = request else {
                // No binding exists before the first Launch, so any other terminal request is
                // declined and cannot acquire Host-start authority.
                super::decline(&server, None, &request, DeclineCause::Unserved)?;
                continue;
            };
            site.set(Site::Launch);
            let declined = WorkerRequestDeclined::operation(
                DeclinedRequestKind::Launch,
                launch.authorization().operation_id(),
            );
            // The kernel retains its launch for replay; the earlier output travels apart.
            let output_seed = launch.take_output_seed();
            let dispatch = match kernel.admit_launch(launch, unix_milliseconds()) {
                Ok(HostOperationAdmission::Dispatch(dispatch)) => dispatch,
                Ok(HostOperationAdmission::ResultReady) => {
                    let result = super::take_result(&mut kernel)?;
                    let _no_host = server_transport::send(&server, None, &result)?;
                    continue;
                }
                // The kernel refused it before any effect, so only this Launch is declined.
                Ok(HostOperationAdmission::SlotOccupied) | Err(_) => {
                    super::send_decline(&server, None, declined, DeclineCause::State)?;
                    continue;
                }
            };
            // Preserve this exact kernel and dispatch across preparation and startup.
            let prepared = super::prepare_or_fail(
                &mut kernel,
                &dispatch,
                Site::Launch,
                Some(runtime),
                None,
                system_shell,
            )?
            .map(|prepared| prepared.with_output_seed(output_seed));
            let Some(prepared) = prepared else {
                let result = super::take_result(&mut kernel)?;
                let _no_host = server_transport::send(&server, None, &result)?;
                continue;
            };
            site.set(Site::HostStart);
            let fresh = HostStart::Fresh(orchbee_local_runtime::HostSlot::FIRST);
            let started = start_host(cohort, runtime, Rc::clone(&drain), fresh)
                .map(|host| (prepared, host));
            let (prepared, (host, child)) = match started {
                Ok(started) => {
                    crate::diagnostic_events::host_started();
                    started
                }
                Err(failure) => {
                    failure.record();
                    kernel.record_pre_handoff_failure(&dispatch, unix_milliseconds())?;
                    let result = super::take_result(&mut kernel)?;
                    // The failure fails only this Launch while a later one can start a Host. A
                    // Worker that can start none again, or has used its starts, can serve no
                    // terminal: it ends after its answer, so the Server stops this Session and
                    // cleans up what the start left.
                    failed_starts += 1;
                    if !failure.retryable() || failed_starts >= HOST_START_ATTEMPTS {
                        let _delivery = server_transport::send(&server, None, &result);
                        return Err(failure.into_exit_cause());
                    }
                    let _no_host = server_transport::send(&server, None, &result)?;
                    continue;
                }
            };
            host.stream().set_nonblocking(true)?;
            // The Launch's bound runs from when it became readable; the Host start used part of it.
            host.pacing.begin_request_at(arrived);
            host.pacing.set_site(Site::Launch);
            let bound = host.pacing.exchange_bound();
            let delivered = (|| -> Result<bool, Box<dyn Error>> {
                let disposition =
                    prepared.execute(&mut kernel, &host, &server, unix_milliseconds, bound)?;
                if disposition == HostOperationDisposition::ServerClosed {
                    return Err(super::peer_closed().into());
                }
                let result = super::take_result(&mut kernel)?;
                Ok(server_transport::send(
                    &server,
                    (disposition == HostOperationDisposition::ResultReady).then_some(&host),
                    &result,
                )?)
            })();
            match delivered {
                Ok(false) => {}
                Ok(true) => {
                    return Err(super::exit_cause::attach(
                        super::peer_closed().into(),
                        &host,
                    ));
                }
                Err(error) => return Err(super::exit_cause::attach(error, &host)),
            }
            let mut hosts = super::host_set::HostSet::managed(super::host_set::Starter {
                cohort,
                runtime,
                drain: Rc::clone(&drain),
            });
            // The Keeper's care starts the Session's Keeper between requests, first right after
            // this answer, so the start takes none of the Launch's bound.
            hosts.add(
                orchbee_local_runtime::HostSlot::FIRST,
                host,
                super::shutdown::ManagedHost::new(child),
                false,
            );
            return super::run_admitted_with_handoff(
                server,
                hosts,
                worker_id,
                generation,
                kernel,
                super::AdmittedAux {
                    drain: &drain,
                    system_shell,
                    worktree_slot: &worktree_slot,
                    agent_integrations: &agent_integrations,
                    agent_probes: &agent_probes,
                    mutation_slot: &mutation_slot,
                    plugin_jobs: &plugin_jobs,
                    session_runtime: Some(runtime),
                },
                #[cfg(windows)]
                handoff,
            );
        }
    })()
    .map_err(|error| super::exit_cause::attach_without_host(error, site.get()));
    super::background_jobs::BackgroundJobs {
        discovery: &worktree_slot,
        integrations: &agent_integrations,
        probes: &agent_probes,
        mutations: &mutation_slot,
        plugins: &plugin_jobs,
    }
    .settle_exit(outcome)
}

#[cfg(all(test, unix))]
mod tests;
