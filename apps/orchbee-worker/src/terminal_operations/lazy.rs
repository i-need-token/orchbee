//! Owns the managed lazy-Host loop, starting the verified Host only after an admitted launch
//! and retaining its child and connection for subsequent operations. It does not adopt an
//! existing Host or replay a launch whose delivery is uncertain.

use super::{server_transport, unix_milliseconds};
use crate::{
    agent_drain::AgentDrain,
    default_interactive_launch::SystemShell,
    diagnostic_events::{DeclineCause, ExitCause, Peer, Reason, Site},
    host_connection::{DialFailure, HostBrokerConnection},
    terminal_authorization::{AuthorizedHostOperationKernel, HostOperationAdmission},
    terminal_host_operation::HostOperationDisposition,
};
use orchbee_capability_node_protocol::{
    CapabilityNodeMessage, DeclinedRequestKind, WorkerRequestDeclined,
};
use orchbee_local_ipc::LocalStream as UnixStream;
use orchbee_local_runtime::{
    Cohort, CohortArtifact, HOST_SCOPE_NAME, ModeCheckedLocalSessionRendezvous, SessionScope,
};
use orchbee_local_worker_protocol::WorkerId;
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
                handoff.expire(Instant::now());
            };
            #[cfg(windows)]
            handoff.expire(std::time::Instant::now());
            #[cfg(windows)]
            if let CapabilityNodeMessage::WorkerHandoffRequest(request) = request {
                site.set(Site::Handoff);
                handoff.execute(&mut server, None, request, worker_id, generation)?;
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
                let result = super::host_query::execute(None, request, worker_id, generation);
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
                let result = super::titles::execute(&server, None, request, worker_id, generation)?;
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
            let started =
                start_host(cohort, runtime, Rc::clone(&drain)).map(|host| (prepared, host));
            let (prepared, (mut host, child)) = match started {
                Ok(started) => {
                    crate::diagnostic_events::host_started();
                    started
                }
                Err(failure) => {
                    crate::diagnostic_events::host_start_failed(
                        failure.stage.token(),
                        failure.error.as_ref(),
                        failure.host_exit,
                        failure.elapsed,
                    );
                    kernel.record_pre_handoff_failure(&dispatch, unix_milliseconds())?;
                    let result = super::take_result(&mut kernel)?;
                    // The failure fails only this Launch while a later one can start a Host. A
                    // Worker that can start none again, or has used its starts, can serve no
                    // terminal: it ends after its answer, so the Server stops this Session and
                    // cleans up what the start left.
                    failed_starts += 1;
                    if !failure.retryable || failed_starts >= HOST_START_ATTEMPTS {
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
                    prepared.execute(&mut kernel, &mut host, &server, unix_milliseconds, bound)?;
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
            let mut managed_host = super::shutdown::ManagedHost::new(child);
            return super::run_admitted_with_handoff(
                server,
                &mut host,
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
                    managed_host: Some(&mut managed_host),
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

/// Where a managed Host start failed, named in its diagnostic record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HostStartStage {
    /// Preparing the Host's runtime folder.
    Directory,
    /// Checking the endpoint address, or an endpoint that existed before the start.
    Endpoint,
    /// Registering the Host in the Session record before creating it; only Windows registers.
    #[cfg(windows)]
    Registration,
    /// Creating the Host process.
    Spawn,
    /// Publishing the created Host's identity and releasing its initial thread.
    #[cfg(windows)]
    Release,
    /// The Host exited, or could not be observed, before its endpoint appeared.
    Exited,
    /// Dialing the Host's endpoint, which never reached the Host.
    Connect,
    /// The `worker-host` offer or the protocol `Hello` with a reached Host.
    Handshake,
    /// The shared deadline passed before the endpoint appeared.
    Deadline,
}

impl HostStartStage {
    /// The closed token the diagnostic record names this stage by.
    fn token(self) -> &'static str {
        match self {
            Self::Directory => "directory",
            Self::Endpoint => "endpoint",
            #[cfg(windows)]
            Self::Registration => "registration",
            Self::Spawn => "spawn",
            #[cfg(windows)]
            Self::Release => "release",
            Self::Exited => "exited",
            Self::Connect => "connect",
            Self::Handshake => "handshake",
            Self::Deadline => "deadline",
        }
    }
}

/// A failed managed Host start: its stage, the time since the start began, the Host's exit code
/// when it had exited, the failure itself, and whether this Worker may start a Host again.
#[derive(Debug)]
struct HostStartFailure {
    stage: HostStartStage,
    elapsed: Duration,
    host_exit: Option<u32>,
    error: Box<dyn Error>,
    /// No Host process was registered or created, or the one created has exited and left no
    /// endpoint, so a later start cannot meet it; and the failure is not one every later start
    /// would repeat, such as an endpoint present before the start. A Windows Host's registration
    /// outlives its process, so there only a failure before registration leaves a retry.
    retryable: bool,
}

impl HostStartFailure {
    /// The loop's end after a failure that leaves no retry, or the last of its starts: the Host
    /// never answered in time, ended before it was reached, or a local step failed.
    fn into_exit_cause(self) -> Box<dyn Error> {
        let (reason, peer) = match self.stage {
            HostStartStage::Deadline => (Reason::Deadline, Peer::Host),
            HostStartStage::Exited | HostStartStage::Connect | HostStartStage::Handshake => {
                (Reason::HostClosed, Peer::Host)
            }
            _ => (Reason::Local, Peer::None),
        };
        Box::new(ExitCause {
            site: Site::HostStart,
            reason,
            peer,
            waited: None,
            source: self.error,
        })
    }
}

/// How long a failed dial waits for the Host to exit, so the record carries the exit code of a
/// Host that ended the connection by ending itself.
const HOST_EXIT_GRACE: Duration = Duration::from_millis(250);

/// Host starts a Worker makes. A start that fails in a way a later one could pass leaves a retry,
/// but only this many in a row: the last failure ends the Worker after its answer, as a permanent
/// one does.
const HOST_START_ATTEMPTS: u32 = 3;

fn start_host(
    cohort: &Cohort,
    runtime: &Path,
    drain: Rc<RefCell<AgentDrain>>,
) -> Result<(HostBrokerConnection, orchbee_local_runtime::RetainedChild), HostStartFailure> {
    let began = Instant::now();
    let deadline = began + Duration::from_secs(5);
    let failed = |stage, error: Box<dyn Error>| HostStartFailure {
        stage,
        elapsed: began.elapsed(),
        host_exit: None,
        error,
        retryable: true,
    };
    // The Host binds `<id>-host.sock` relatively inside the socket folder while the Worker
    // connects through the derived absolute path; validate that address before the Host
    // directory or process can appear.
    let endpoint = ModeCheckedLocalSessionRendezvous::host_endpoint_path(runtime);
    // A failure no later start could pass leaves no retry: the address is the same every time.
    let permanent = |stage, error: Box<dyn Error>| HostStartFailure {
        retryable: false,
        ..failed(stage, error)
    };
    #[cfg(unix)]
    nix::sys::socket::UnixAddr::new(&endpoint)
        .map_err(|error| permanent(HostStartStage::Endpoint, error.into()))?;
    let directory = SessionScope::parse(HOST_SCOPE_NAME)
        .map_err(Box::<dyn Error>::from)
        .and_then(|scope| {
            ModeCheckedLocalSessionRendezvous::prepare_mode_checked(runtime, scope)
                .map_err(Box::<dyn Error>::from)
        })
        .map_err(|error| failed(HostStartStage::Directory, error))?;
    // An endpoint already present belongs to a Host this Worker did not start, live or dead, and
    // stays until the Session cleans up: every later start would meet it too.
    let recovery_required = || -> Box<dyn Error> {
        io::Error::new(io::ErrorKind::AlreadyExists, "Host recovery is required").into()
    };
    #[cfg(unix)]
    match std::fs::symlink_metadata(&endpoint) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Ok(_) => return Err(permanent(HostStartStage::Endpoint, recovery_required())),
        Err(error) => return Err(failed(HostStartStage::Endpoint, error.into())),
    }
    #[cfg(windows)]
    match orchbee_local_ipc::endpoint_exists(&endpoint) {
        Ok(false) => {}
        Ok(true) => return Err(permanent(HostStartStage::Endpoint, recovery_required())),
        Err(error) => return Err(failed(HostStartStage::Endpoint, error.into())),
    }
    let host_endpoint_id = ModeCheckedLocalSessionRendezvous::host_endpoint_id(runtime);
    let executable = cohort.executable(CohortArtifact::TerminalHost);
    let arguments = [
        std::ffi::OsStr::new("serve"),
        std::ffi::OsStr::new("--runtime-directory"),
        directory.runtime_directory().as_os_str(),
        std::ffi::OsStr::new("--endpoint-id"),
        std::ffi::OsStr::new(&host_endpoint_id),
    ];
    // A macOS or Linux Host binds its endpoint in the socket folder, which the Session admitted.
    #[cfg(unix)]
    let socket_root = ModeCheckedLocalSessionRendezvous::socket_root_path();
    #[cfg(unix)]
    let arguments = [
        &arguments[..],
        &[
            std::ffi::OsStr::new("--endpoint-directory"),
            socket_root.as_os_str(),
        ],
    ]
    .concat();
    #[cfg(windows)]
    let mut child = orchbee_local_runtime::DataRoot::resolve(|name| std::env::var_os(name))
        .map_err(|_| io::Error::from(io::ErrorKind::NotFound))
        .and_then(|root| {
            orchbee_local_runtime::spawn_session_child(
                &root,
                runtime,
                orchbee_local_runtime::SessionChild::Host,
                &executable,
                &arguments,
                deadline,
            )
        })
        .map_err(|error| {
            use orchbee_local_runtime::SessionChildStep;
            let stage = match SessionChildStep::of(&error) {
                Some(SessionChildStep::Creation) => HostStartStage::Spawn,
                Some(SessionChildStep::Release) => HostStartStage::Release,
                Some(SessionChildStep::Registration) | None => HostStartStage::Registration,
            };
            HostStartFailure {
                retryable: false,
                ..failed(stage, error.into())
            }
        })?;
    #[cfg(not(windows))]
    let mut child = orchbee_local_runtime::spawn_retained(&executable, &arguments)
        .map_err(|error| failed(HostStartStage::Spawn, error.into()))?;
    let dialed = |child: &mut orchbee_local_runtime::RetainedChild,
                  dial: Result<HostBrokerConnection, DialFailure>| {
        let failure = match dial {
            Ok(host) => return Ok(host),
            Err(DialFailure::Connect(error)) => failed(HostStartStage::Connect, error),
            Err(DialFailure::Handshake(error)) => failed(HostStartStage::Handshake, error),
        };
        // The elapsed time stays the dial's own; the wait only observes the Host's exit.
        Err(HostStartFailure {
            host_exit: exit_after_failure(child),
            ..failure
        })
    };
    let ready = (|| loop {
        if Instant::now() >= deadline {
            return Err(failed(
                HostStartStage::Deadline,
                io::Error::from(io::ErrorKind::TimedOut).into(),
            ));
        }
        match child.try_wait() {
            Ok(None) => {}
            Ok(Some(status)) => {
                return Err(HostStartFailure {
                    host_exit: exit_code(status),
                    ..failed(
                        HostStartStage::Exited,
                        io::Error::other("Terminal Host exited before readiness").into(),
                    )
                });
            }
            Err(error) => return Err(failed(HostStartStage::Exited, error.into())),
        }
        #[cfg(windows)]
        {
            match orchbee_local_ipc::endpoint_exists(&endpoint) {
                Ok(true) => {
                    let dial = HostBrokerConnection::connect_before(
                        &endpoint,
                        deadline,
                        Rc::clone(&drain),
                    );
                    return dialed(&mut child, dial);
                }
                Ok(false) => thread::sleep(Duration::from_millis(10)),
                Err(error) => return Err(failed(HostStartStage::Connect, error.into())),
            }
        }
        #[cfg(unix)]
        match std::fs::symlink_metadata(&endpoint) {
            Ok(status)
                if status.file_type().is_socket()
                    && status.uid() == nix::unistd::geteuid().as_raw()
                    && status.mode() & 0o7777 == 0o600 =>
            {
                let dial =
                    HostBrokerConnection::connect_before(&endpoint, deadline, Rc::clone(&drain));
                return dialed(&mut child, dial);
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
                return Err(failed(
                    HostStartStage::Endpoint,
                    io::Error::new(io::ErrorKind::PermissionDenied, "invalid Host endpoint").into(),
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                thread::sleep(Duration::from_millis(10))
            }
            Err(error) => return Err(failed(HostStartStage::Endpoint, error.into())),
        }
    })();
    match ready {
        Ok(host) => Ok((host, child)),
        // A Host still running could later bind the endpoint a second start dials, one that bound
        // its endpoint before it ended leaves that endpoint behind, and a Windows registration
        // outlives the process; each leaves this Worker no further start.
        Err(failure) => Err(HostStartFailure {
            retryable: cfg!(not(windows))
                && matches!(child.try_wait(), Ok(Some(_)))
                && endpoint_absent(&endpoint),
            ..failure
        }),
    }
}

/// Whether no endpoint, not even one a Host that ended left behind, is at the Host's address.
#[cfg(unix)]
fn endpoint_absent(endpoint: &Path) -> bool {
    matches!(std::fs::symlink_metadata(endpoint), Err(error) if error.kind() == io::ErrorKind::NotFound)
}

#[cfg(windows)]
fn endpoint_absent(endpoint: &Path) -> bool {
    matches!(orchbee_local_ipc::endpoint_exists(endpoint), Ok(false))
}

/// The Host's exit code once it exits within [`HOST_EXIT_GRACE`]; a Host still running keeps none.
fn exit_after_failure(child: &mut orchbee_local_runtime::RetainedChild) -> Option<u32> {
    let until = Instant::now() + HOST_EXIT_GRACE;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return exit_code(status),
            Ok(None) if Instant::now() < until => thread::sleep(Duration::from_millis(10)),
            Ok(None) | Err(_) => return None,
        }
    }
}

/// A Windows exit code keeps all 32 bits, so a crash reads as its status value; a POSIX Host ended
/// by a signal has none.
fn exit_code(status: std::process::ExitStatus) -> Option<u32> {
    status
        .code()
        .map(|code| u32::from_ne_bytes(code.to_ne_bytes()))
}

#[cfg(all(test, unix))]
mod tests;
