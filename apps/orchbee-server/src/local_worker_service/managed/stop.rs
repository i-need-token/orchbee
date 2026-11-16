//! Managed Session shutdown settles creation-owned resources before publishing restart authority;
//! an `exit` settles the same work but ends only this Server and its Worker. A Session that keeps a
//! checkpoint restores it before the Client listener opens, saves it while it runs and publishes a
//! final one before its Worker disposes any terminal.
use super::super::agent_observation_endpoint::AgentObservationEndpoint;
use crate::{
    agent_service::AgentService,
    local_client_service,
    local_terminal_service::LocalTerminalService,
    session_shutdown::{Attempt, SessionShutdown, Work},
    worker_obcn_carrier::{
        WorkerOperationRequest, WorkerOperationResult, WorkerRequestError, WorkerRequestHandle,
    },
};
use orchbee_capability_node_protocol::{
    OperationId, ShutdownBudget, ShutdownFailure, ShutdownStage, ShutdownTerminals,
    WorkerGeneration, WorkerId, WorkerShutdownOutcome, WorkerShutdownRequest,
};
use orchbee_local_client_protocol::{StopFailure, StopId, StopPhase};
use orchbee_local_runtime::{
    LockAttempt, ModeCheckedLocalSessionRendezvous, ModeCheckedSessionLock,
    RetainedActivationRecord, RetainedChild, RetainedSessionRecord,
};
use std::{
    io,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

#[cfg(windows)]
mod idle;

/// One stop attempt's cooperative deadline.
const STOP_ATTEMPT_BUDGET: Duration = Duration::from_secs(60);
/// The most of an attempt the Worker may spend disposing terminals and stopping its Host. The rest
/// of the attempt stays for the stages before and after it.
const WORKER_STOP_BUDGET: Duration = Duration::from_secs(45);

pub(super) struct Owners {
    pub(super) session: ModeCheckedLocalSessionRendezvous,
    pub(super) lifetime: ModeCheckedSessionLock,
    /// Keeps a temporary-file cleaner that honours locks away from the socket folder.
    #[cfg(unix)]
    pub(super) socket_root: ModeCheckedSessionLock,
    pub(super) record: RetainedActivationRecord,
    pub(super) worker: RetainedChild,
    pub(super) carrier: thread::JoinHandle<()>,
    pub(super) handle: WorkerRequestHandle,
    pub(super) ingress: AgentObservationEndpoint,
    pub(super) terminals: Arc<LocalTerminalService>,
    pub(super) agents: Arc<AgentService>,
    /// The checkpoint this Session keeps; none where the platform keeps none.
    pub(super) checkpoint: Option<super::checkpoint::Kept>,
    #[cfg(windows)]
    pub(super) automatic_updates: Option<crate::automatic_updates::AutomaticUpdates>,
    pub(super) session_record: RetainedSessionRecord,
}

struct Failure {
    phase: StopPhase,
    failure: StopFailure,
    disposed: u32,
}
fn failure(phase: StopPhase, error: io::Error) -> Failure {
    Failure {
        phase,
        failure: if error.kind() == io::ErrorKind::TimedOut {
            StopFailure::Deadline
        } else {
            StopFailure::Unavailable
        },
        disposed: 0,
    }
}

/// Starts the Session's jobs and Client listener, then serves stops and an `exit` until one ends
/// the process. `shutdown` may already hold a stop the Server's signals began under `own`.
pub(super) fn run(
    mut owners: Owners,
    shutdown: Arc<SessionShutdown>,
    #[cfg(unix)] own: StopId,
    cohort: &str,
    restored: crate::local_terminal_service::CheckpointRestore,
    startup_deadline: Instant,
    configuration: orchbee_local_runtime::UserSettingsLoad,
) -> io::Result<()> {
    let started = start(
        &mut owners,
        &shutdown,
        cohort,
        restored,
        startup_deadline,
        configuration,
    );
    let mut clients: local_client_service::ManagedClients = match started {
        Ok(clients) => clients,
        Err(error) => return Err(abort_startup(owners, error)),
    };
    #[cfg(windows)]
    {
        // Plugin startup commands may call back into the Session, so they run once it admits
        // Clients.
        owners.terminals.start_plugin_hooks();
        // The schedule counts from the moment the Session admits Clients.
        owners.automatic_updates =
            crate::automatic_updates::AutomaticUpdates::start(Arc::clone(&owners.terminals));
    }
    let mut worker_completed = false;
    #[cfg(windows)]
    let mut idle = idle::IdleMove::default();
    loop {
        #[cfg(windows)]
        let work = next_work(&mut owners, &shutdown, &clients, &mut idle);
        #[cfg(unix)]
        let work = next_work(&mut owners, &shutdown, own);
        let attempt = match work {
            Work::Stop(attempt) => attempt,
            Work::Exit => return exit(owners, clients, &shutdown),
        };
        let deadline = Instant::now() + STOP_ATTEMPT_BUDGET;
        let result = settle(
            &mut owners,
            &shutdown,
            attempt,
            deadline,
            &mut worker_completed,
        );
        if let Err(error) = result {
            shutdown.fail(attempt, error.phase, error.failure, error.disposed);
            continue;
        }
        // Hold activation through listener closure, receipt publication and lifetime release.
        let activation = loop {
            match owners
                .session
                .try_acquire_activation_lock()
                .map_err(io::Error::other)
            {
                Ok(LockAttempt::Acquired(lock)) => break Ok(lock),
                Ok(LockAttempt::Contended) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(10))
                }
                Ok(LockAttempt::Contended) => break Err(io::ErrorKind::TimedOut.into()),
                Err(error) => break Err(error),
            }
        };
        let activation = match activation {
            Ok(lock) => lock,
            Err(error) => {
                let error = failure(StopPhase::Completion, error);
                shutdown.fail(attempt, error.phase, error.failure, 0);
                continue;
            }
        };
        owners
            .carrier
            .join()
            .map_err(|_| io::Error::other("Worker carrier panicked"))?;
        shutdown.phase(attempt, StopPhase::Clients);
        // The process ends from here on, so a waiting `exit` requester replies first.
        shutdown.end(deadline);
        // After control-listener closure, only a valid durable record can prove success. A failure
        // exits with the active/invalid record intact; it never silently authorizes restart.
        clients.stop_before(deadline)?;
        // A POSIX socket file outlives its listener, so the stop removes its own before the
        // receipt; whatever a failed removal leaves, the next owner of the lifetime lock removes.
        #[cfg(unix)]
        owners.session.remove_endpoints(&owners.lifetime)?;
        shutdown.phase(attempt, StopPhase::Completion);
        if Instant::now() >= deadline {
            return Err(io::ErrorKind::TimedOut.into());
        }
        owners.session_record.complete(attempt.id.bytes())?;
        owners.record.complete(attempt.id.bytes())?;
        drop(owners.lifetime);
        #[cfg(unix)]
        drop(owners.socket_root);
        drop(activation);
        return Ok(());
    }
}

/// Restores the checkpoint's terminals and starts every job before the Client listener opens. A
/// file that cannot be read starts the Session with the defaults, published as unreadable.
fn start(
    owners: &mut Owners,
    shutdown: &Arc<SessionShutdown>,
    cohort: &str,
    restored: crate::local_terminal_service::CheckpointRestore,
    startup_deadline: Instant,
    configuration: orchbee_local_runtime::UserSettingsLoad,
) -> io::Result<local_client_service::ManagedClients> {
    #[cfg(windows)]
    owners.terminals.bind_agent_names(&owners.agents)?;
    owners.terminals.start_close_maintenance()?;
    owners.terminals.start_title_reads()?;
    owners.terminals.start_agent_catalog()?;
    owners.terminals.start_agent_start_delivery()?;
    #[cfg(windows)]
    let mut restored = restored;
    #[cfg(windows)]
    let resumes = restored.take_agent_resume();
    // Restored launches read their kept output through the same store.
    if let Some(kept) = &owners.checkpoint {
        owners.terminals.install_terminal_output(Arc::new(
            crate::terminal_output::TerminalOutput::new(kept.output_store()),
        ));
    }
    owners
        .terminals
        .launch_checkpoint_terminals(restored, startup_deadline)?;
    owners
        .terminals
        .start_process_observer(move || Ok(configuration), true)?;
    // Resumes read through the observer and wait for a Client, so no listener may open first.
    #[cfg(windows)]
    owners.terminals.start_agent_resume(resumes)?;
    if let Some(kept) = &mut owners.checkpoint {
        kept.start_saver(&owners.terminals, checkpoint_worker(&owners.handle)?)?;
    }
    local_client_service::start_managed(
        #[cfg(windows)]
        owners.session.client_socket_path(),
        #[cfg(unix)]
        (
            owners.session.client_socket_path(),
            &ModeCheckedLocalSessionRendezvous::client_pending_socket_path(
                owners.session.runtime_directory(),
            ),
        ),
        local_client_service::ServerIdentity::local(Some(cohort), owners.worker.id()),
        Arc::clone(&owners.terminals),
        Arc::clone(&owners.agents),
        Arc::clone(shutdown),
    )
}

fn settle(
    owners: &mut Owners,
    shutdown: &SessionShutdown,
    attempt: Attempt,
    deadline: Instant,
    worker_completed: &mut bool,
) -> Result<(), Failure> {
    let mut disposed = 0;
    if !shutdown.settle_records(deadline) {
        return Err(failure(
            StopPhase::Admission,
            io::ErrorKind::TimedOut.into(),
        ));
    }
    stop_background_jobs(owners, deadline)
        .map_err(|error| failure(StopPhase::BackgroundJobs, error))?;
    let worker_exited = owners
        .worker
        .try_wait()
        .map_err(|_| Failure {
            phase: StopPhase::BackgroundJobs,
            failure: StopFailure::ProcessWait,
            disposed: 0,
        })?
        .is_some();
    if !*worker_completed && worker_exited {
        settle_without_worker(owners, deadline)?;
        *worker_completed = true;
    }
    if !*worker_completed {
        if let Some(kept) = &owners.checkpoint {
            let worker = checkpoint_worker(&owners.handle)
                .map_err(|error| failure(StopPhase::BackgroundJobs, error))?;
            let bytes = owners
                .terminals
                .checkpoint(&worker, deadline)
                .map_err(|error| failure(StopPhase::BackgroundJobs, error))?;
            kept.publish_final(&owners.terminals, &bytes)
                .map_err(|error| failure(StopPhase::BackgroundJobs, error))?;
        }
        shutdown.phase(attempt, StopPhase::Worker);
        let budget = deadline
            .saturating_duration_since(Instant::now())
            .min(WORKER_STOP_BUDGET)
            .as_millis() as u32;
        let request = WorkerShutdownRequest {
            operation_id: OperationId::from_bytes(attempt.id.bytes())
                .expect("nonzero stop identity"),
            worker_id: WorkerId::from_bytes(owners.handle.worker_id().as_bytes())
                .expect("admitted Worker"),
            worker_generation: WorkerGeneration::new(owners.handle.generation()),
            budget: ShutdownBudget::new(budget)
                .map_err(|_| failure(StopPhase::Worker, io::ErrorKind::TimedOut.into()))?,
            terminals: ShutdownTerminals::Dispose,
        };
        match owners
            .handle
            .request(WorkerOperationRequest::Shutdown(request))
        {
            Ok(WorkerOperationResult::Shutdown(result)) => match result.outcome {
                WorkerShutdownOutcome::NoHost => *worker_completed = true,
                WorkerShutdownOutcome::Stopped {
                    disposed_terminals, ..
                } => {
                    *worker_completed = true;
                    disposed = disposed_terminals;
                }
                WorkerShutdownOutcome::Refused(_) => {
                    return Err(Failure {
                        phase: StopPhase::Worker,
                        failure: StopFailure::Refused,
                        disposed: 0,
                    });
                }
                // Only a Keep shutdown is released, and this request disposes.
                WorkerShutdownOutcome::Released { .. } => {
                    return Err(Failure {
                        phase: StopPhase::Worker,
                        failure: StopFailure::Protocol,
                        disposed: 0,
                    });
                }
                WorkerShutdownOutcome::Incomplete {
                    disposed_terminals,
                    stage,
                    failure,
                    ..
                } => {
                    return Err(Failure {
                        phase: worker_phase(stage),
                        failure: worker_failure(failure),
                        disposed: disposed_terminals,
                    });
                }
            },
            Ok(_) => {
                return Err(Failure {
                    phase: StopPhase::Worker,
                    failure: StopFailure::Protocol,
                    disposed: 0,
                });
            }
            Err(error) => {
                return Err(Failure {
                    phase: StopPhase::Worker,
                    failure: if matches!(error, WorkerRequestError::PostDeliveryOutcomeUnknown(_)) {
                        StopFailure::OutcomeUnknown
                    } else {
                        StopFailure::Unavailable
                    },
                    disposed: 0,
                });
            }
        }
    }
    shutdown.phase(attempt, StopPhase::WorkerExit);
    loop {
        let exited = owners
            .worker
            .try_wait()
            .map_err(|_| Failure {
                phase: StopPhase::WorkerExit,
                failure: StopFailure::ProcessWait,
                disposed,
            })?
            .is_some();
        if exited && owners.carrier.is_finished() {
            break;
        }
        if Instant::now() >= deadline {
            return Err(Failure {
                phase: StopPhase::WorkerExit,
                failure: StopFailure::Deadline,
                disposed,
            });
        }
        thread::sleep(Duration::from_millis(10));
    }
    owners
        .terminals
        .finish_process_observer_after_worker_exit(&owners.handle)
        .map_err(|error| failure(StopPhase::WorkerExit, error))?;
    shutdown.phase(attempt, StopPhase::Ingress);
    owners.ingress.stop_before(deadline).map_err(|error| {
        let mut error = failure(StopPhase::Ingress, error);
        error.disposed = disposed;
        error
    })?;
    Ok(())
}

/// Waits for a stop request or an `exit`. The managed Session never enrolls a replacement Worker,
/// so once its Worker has exited and that Worker's Host is gone too, no terminal can run here again
/// and the Server begins its own stop. While that Host still runs, the Server keeps running beside
/// it. Each tick also feeds the idle move.
#[cfg(windows)]
fn next_work(
    owners: &mut Owners,
    shutdown: &SessionShutdown,
    clients: &local_client_service::ManagedClients,
    idle: &mut idle::IdleMove,
) -> Work {
    loop {
        if let Some(work) = shutdown.take_within(Duration::from_secs(1)) {
            return work;
        }
        if !shutdown.stopping()
            && matches!(owners.worker.try_wait(), Ok(Some(_)))
            && host_absent(owners)
            && let Some(id) = own_stop_id()
        {
            shutdown.begin(id);
        }
        idle.tick(owners, shutdown, clients);
    }
}

/// [`next_work`] as on Windows, except that a failed stop this Server began, on a signal or after
/// losing its Worker and Host, is tried again once that Worker and Host are gone. A logout or
/// shutdown signals the Worker and Host too, so a first attempt can find the Host still exiting.
/// Every stop this Server begins uses `own`, the identity its signal handler uses too.
#[cfg(unix)]
fn next_work(owners: &mut Owners, shutdown: &SessionShutdown, own: StopId) -> Work {
    loop {
        if let Some(work) = shutdown.take_within(Duration::from_secs(1)) {
            return work;
        }
        if matches!(owners.worker.try_wait(), Ok(Some(_))) && host_absent(owners) {
            begin_own(shutdown, own);
        }
    }
}

/// Begins the stop this Server owns under `own`, or queues its next attempt once one failed. A
/// stop a Client recorded stays that Client's to retry.
#[cfg(unix)]
pub(super) fn begin_own(shutdown: &SessionShutdown, own: StopId) {
    if !shutdown.begin(own) {
        shutdown.request(&orchbee_local_client_protocol::SessionStopRequest {
            id: own,
            action: orchbee_local_client_protocol::StopAction::Retry,
        });
    }
}

/// After its Worker exited, nothing can sample a terminal's directory or stop its Host. The stop
/// finishes only when that Host is gone too, keeping each terminal's last sampled directory; a live
/// Host is refused, because nothing here may end it.
fn settle_without_worker(owners: &Owners, deadline: Instant) -> Result<(), Failure> {
    let worker = checkpoint_worker(&owners.handle)
        .map_err(|error| failure(StopPhase::BackgroundJobs, error))?;
    if !host_absent(owners) {
        return Err(Failure {
            phase: StopPhase::HostExit,
            failure: StopFailure::Refused,
            disposed: 0,
        });
    }
    owners
        .terminals
        .finish_process_observer_after_worker_exit(&owners.handle)
        .map_err(|error| failure(StopPhase::BackgroundJobs, error))?;
    if let Some(kept) = &owners.checkpoint {
        let bytes = owners
            .terminals
            .checkpoint_without_worker(&worker)
            .map_err(|error| failure(StopPhase::BackgroundJobs, error))?;
        if Instant::now() >= deadline {
            return Err(failure(
                StopPhase::BackgroundJobs,
                io::ErrorKind::TimedOut.into(),
            ));
        }
        kept.publish_final(&owners.terminals, &bytes)
            .map_err(|error| failure(StopPhase::BackgroundJobs, error))?;
    }
    Ok(())
}

/// Stops every Server background job that could start new work through the Worker. Hooks submit
/// Worker jobs, so dispatch stops before any Worker owner; admitted hook jobs end with the Worker.
fn stop_background_jobs(owners: &mut Owners, deadline: Instant) -> io::Result<()> {
    #[cfg(windows)]
    {
        // An automatic update holds no Session resource, so its stop waits for nothing and never
        // fails.
        if let Some(updates) = owners.automatic_updates.take() {
            updates.stop();
        }
        owners.terminals.stop_plugin_hooks_before(deadline)?;
        // A resume types through the Worker and reads through the observer, so it stops first; one
        // not yet attempted stays pending for the final capture.
        owners.terminals.stop_agent_resume_before(deadline)?;
        // A start line types through the Worker and reads through the observer the same way.
        owners
            .terminals
            .stop_agent_start_delivery_before(deadline)?;
    }
    if let Some(kept) = &mut owners.checkpoint {
        kept.stop_saver_before(deadline)?;
    }
    owners.terminals.stop_process_observer_before(deadline)?;
    owners.terminals.stop_worktrees_before(deadline)?;
    owners.terminals.stop_workspace_groups_before(deadline)?;
    // A duplication launches terminals through the Worker, so it stops while the Worker runs.
    owners.terminals.stop_tab_duplicates_before(deadline)?;
    // A Tab set waits on its members' Closes, so it stops before Close maintenance freezes them.
    owners.terminals.stop_tab_sets_before(deadline)?;
    owners.terminals.stop_title_reads_before(deadline)?;
    owners.terminals.stop_agent_catalog_before(deadline)?;
    owners.terminals.stop_close_before(deadline)?;
    owners.terminals.stop_graphics_before(deadline)
}

/// How long an `exit` may spend settling before the process ends regardless.
const EXIT_BUDGET: Duration = Duration::from_secs(30);
/// What an `exit` asks of the Worker's terminals.
#[cfg(windows)]
const EXIT_TERMINALS: ShutdownTerminals = ShutdownTerminals::Keep;
#[cfg(unix)]
const EXIT_TERMINALS: ShutdownTerminals = ShutdownTerminals::Dispose;

/// Ends this Server and its Worker for an `exit`. On Windows the Host and every terminal keep
/// running; a POSIX Worker cannot leave its Host to a later Server, so it disposes them and stops
/// the Host, and a Session that keeps a checkpoint relaunches them on its next start. In-flight
/// records and background jobs settle and the final checkpoint is published first; a settling
/// failure is reported and the process still ends. No receipt is published, so until P1 records
/// this end the next activation recovers the Session as an unclean end.
fn exit(
    mut owners: Owners,
    mut clients: local_client_service::ManagedClients,
    shutdown: &SessionShutdown,
) -> io::Result<()> {
    let deadline = Instant::now() + EXIT_BUDGET;
    if let Err(error) = settle_for_exit(&mut owners, shutdown, deadline) {
        eprintln!(
            "orchbee-server diagnostic component=lifecycle operation=exit stage=settle cause=io:{:?}",
            error.kind()
        );
    }
    shutdown.end(deadline);
    clients.stop_before(deadline)
}

fn settle_for_exit(
    owners: &mut Owners,
    shutdown: &SessionShutdown,
    deadline: Instant,
) -> io::Result<()> {
    if !shutdown.settle_records(deadline) {
        return Err(io::ErrorKind::TimedOut.into());
    }
    stop_background_jobs(owners, deadline)?;
    let worker = checkpoint_worker(&owners.handle)?;
    if owners.worker.try_wait()?.is_some() {
        owners
            .terminals
            .finish_process_observer_after_worker_exit(&owners.handle)?;
        if let Some(kept) = &owners.checkpoint {
            let bytes = owners.terminals.checkpoint_without_worker(&worker)?;
            kept.publish_final(&owners.terminals, &bytes)?;
        }
    } else {
        if let Some(kept) = &owners.checkpoint {
            let bytes = owners.terminals.checkpoint(&worker, deadline)?;
            kept.publish_final(&owners.terminals, &bytes)?;
        }
        let mut operation = [0; 16];
        getrandom::fill(&mut operation).map_err(|error| io::Error::other(error.to_string()))?;
        let budget = deadline
            .saturating_duration_since(Instant::now())
            .as_millis()
            .min(15_000) as u32;
        let request = WorkerShutdownRequest {
            operation_id: OperationId::from_bytes(operation).map_err(io::Error::other)?,
            worker_id: WorkerId::from_bytes(owners.handle.worker_id().as_bytes())
                .map_err(io::Error::other)?,
            worker_generation: WorkerGeneration::new(owners.handle.generation()),
            budget: ShutdownBudget::new(budget).map_err(|_| io::ErrorKind::TimedOut)?,
            terminals: EXIT_TERMINALS,
        };
        match owners
            .handle
            .request(WorkerOperationRequest::Shutdown(request))
        {
            #[cfg(windows)]
            Ok(WorkerOperationResult::Shutdown(result))
                if matches!(
                    result.outcome,
                    WorkerShutdownOutcome::NoHost | WorkerShutdownOutcome::Released { .. }
                ) => {}
            #[cfg(unix)]
            Ok(WorkerOperationResult::Shutdown(result))
                if matches!(
                    result.outcome,
                    WorkerShutdownOutcome::NoHost | WorkerShutdownOutcome::Stopped { .. }
                ) => {}
            _ => return Err(io::Error::other("Worker exit unconfirmed")),
        }
        while owners.worker.try_wait()?.is_none() || !owners.carrier.is_finished() {
            if Instant::now() >= deadline {
                return Err(io::ErrorKind::TimedOut.into());
            }
            thread::sleep(Duration::from_millis(10));
        }
        owners
            .terminals
            .finish_process_observer_after_worker_exit(&owners.handle)?;
    }
    owners.ingress.stop_before(deadline)?;
    // A Host left without a terminal ends once its broker is gone; waiting for it lets the next
    // activation recover the Session at once instead of finding a live process.
    if owners.terminals.terminal_count() == 0 {
        let host_deadline = deadline.min(Instant::now() + Duration::from_secs(3));
        while !host_absent(owners) && Instant::now() < host_deadline {
            thread::sleep(Duration::from_millis(10));
        }
    }
    Ok(())
}

/// Only an exact observation that the Host is gone, or that it never started, counts.
fn host_absent(owners: &Owners) -> bool {
    matches!(owners.session_record.host_absent(), Ok(true))
}

#[cfg(windows)]
fn own_stop_id() -> Option<StopId> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).ok()?;
    StopId::new(bytes).ok()
}

fn worker_phase(stage: ShutdownStage) -> StopPhase {
    match stage {
        ShutdownStage::BackgroundJobs => StopPhase::BackgroundJobs,
        ShutdownStage::Inventory => StopPhase::Inventory,
        ShutdownStage::Force => StopPhase::Force,
        ShutdownStage::Dispose => StopPhase::Dispose,
        ShutdownStage::HostExit => StopPhase::HostExit,
    }
}

/// Startup has no Client requests, but restoration may already have created a Host.
fn abort_startup(mut owners: Owners, startup: io::Error) -> io::Error {
    let deadline = Instant::now() + Duration::from_secs(30);
    let cleanup = (|| -> io::Result<()> {
        #[cfg(windows)]
        {
            owners.terminals.stop_agent_resume_before(deadline)?;
            owners
                .terminals
                .stop_agent_start_delivery_before(deadline)?;
        }
        if let Some(kept) = &mut owners.checkpoint {
            kept.stop_saver_before(deadline)?;
        }
        owners.terminals.stop_process_observer_before(deadline)?;
        owners.terminals.stop_title_reads_before(deadline)?;
        owners.terminals.stop_agent_catalog_before(deadline)?;
        owners.terminals.stop_close_before(deadline)?;
        owners.terminals.stop_graphics_before(deadline)?;
        let mut bytes = [0; 16];
        getrandom::fill(&mut bytes).map_err(|error| io::Error::other(error.to_string()))?;
        let operation_id = OperationId::from_bytes(bytes).map_err(io::Error::other)?;
        let request = WorkerShutdownRequest {
            operation_id,
            worker_id: WorkerId::from_bytes(owners.handle.worker_id().as_bytes())
                .map_err(io::Error::other)?,
            worker_generation: WorkerGeneration::new(owners.handle.generation()),
            budget: ShutdownBudget::new(15_000).map_err(io::Error::other)?,
            terminals: ShutdownTerminals::Dispose,
        };
        match owners
            .handle
            .request(WorkerOperationRequest::Shutdown(request))
        {
            Ok(WorkerOperationResult::Shutdown(result))
                if matches!(
                    result.outcome,
                    WorkerShutdownOutcome::NoHost | WorkerShutdownOutcome::Stopped { .. }
                ) => {}
            // Startup cleanup requires a complete shutdown receipt from this Worker; other responses do not prove it stopped.
            _ => return Err(io::Error::other("startup Worker shutdown unconfirmed")),
        }
        while owners.worker.try_wait()?.is_none() || !owners.carrier.is_finished() {
            if Instant::now() >= deadline {
                return Err(io::ErrorKind::TimedOut.into());
            }
            thread::sleep(Duration::from_millis(10));
        }
        owners
            .terminals
            .finish_process_observer_after_worker_exit(&owners.handle)?;
        owners.ingress.stop_before(deadline)?;
        Ok(())
    })();
    if cleanup.is_ok() && owners.carrier.join().is_err() {
        return io::Error::other(format!("{startup}; startup Worker carrier panicked"));
    }
    match cleanup {
        Ok(()) => startup,
        Err(cleanup) => {
            io::Error::other(format!("{startup}; startup cleanup unconfirmed: {cleanup}"))
        }
    }
}
fn checkpoint_worker(
    handle: &WorkerRequestHandle,
) -> io::Result<orchbee_workbench_protocol::WorkerId> {
    orchbee_workbench_protocol::WorkerId::parse(&format!(
        "{:032x}",
        u128::from_be_bytes(handle.worker_id().as_bytes())
    ))
    .map_err(io::Error::other)
}
fn worker_failure(failure: ShutdownFailure) -> StopFailure {
    match failure {
        ShutdownFailure::Deadline => StopFailure::Deadline,
        ShutdownFailure::Transport => StopFailure::OutcomeUnknown,
        ShutdownFailure::Protocol => StopFailure::Protocol,
        ShutdownFailure::HostRefused => StopFailure::Refused,
        ShutdownFailure::ProcessWait => StopFailure::ProcessWait,
    }
}
