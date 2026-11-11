//! Managed Windows shutdown settles creation-owned resources before publishing restart authority;
//! an `exit` settles the same work but ends only this Server and its Worker.
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
    RetainedActivationRecord, RetainedChild, RetainedSessionRecord, SessionStore,
};
use std::{
    io,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

/// One stop attempt's cooperative deadline.
const STOP_ATTEMPT_BUDGET: Duration = Duration::from_secs(60);
/// The most of an attempt the Worker may spend disposing terminals and stopping its Host. The rest
/// of the attempt stays for the stages before and after it.
const WORKER_STOP_BUDGET: Duration = Duration::from_secs(45);

pub(super) struct Owners {
    pub(super) session: ModeCheckedLocalSessionRendezvous,
    pub(super) lifetime: ModeCheckedSessionLock,
    pub(super) record: RetainedActivationRecord,
    pub(super) worker: RetainedChild,
    pub(super) carrier: thread::JoinHandle<()>,
    pub(super) handle: WorkerRequestHandle,
    pub(super) ingress: AgentObservationEndpoint,
    pub(super) terminals: Arc<LocalTerminalService>,
    pub(super) agents: Arc<AgentService>,
    pub(super) checkpoint_store: Arc<Mutex<SessionStore>>,
    pub(super) checkpoint_saver: Option<super::checkpoint_windows::autosave::Saver>,
    pub(super) suppress_initial_empty: bool,
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

pub(super) fn run(
    mut owners: Owners,
    cohort: &str,
    mut restored: crate::local_terminal_service::CheckpointRestore,
    startup_deadline: Instant,
    configuration: orchbee_local_runtime::UserSettingsLoad,
) -> io::Result<()> {
    let shutdown = Arc::new(SessionShutdown::default());
    let started = (|| {
        owners.terminals.bind_agent_names(&owners.agents)?;
        owners.terminals.start_close_maintenance()?;
        owners.terminals.start_title_reads()?;
        owners.terminals.start_agent_catalog()?;
        owners.terminals.start_agent_start_delivery()?;
        let resumes = restored.take_agent_resume();
        // Restored launches read their kept output through the same store.
        let output_store: Arc<dyn crate::terminal_output::TerminalOutputStore> =
            Arc::clone(&owners.checkpoint_store) as _;
        owners.terminals.install_terminal_output(Arc::new(
            crate::terminal_output::TerminalOutput::new(output_store),
        ));
        owners
            .terminals
            .launch_checkpoint_terminals(restored, startup_deadline)?;
        owners
            .terminals
            .start_process_observer(move || Ok(configuration), true)?;
        // Resumes read through the observer and wait for a Client, so no listener may open first.
        owners.terminals.start_agent_resume(resumes)?;
        let worker = checkpoint_worker(&owners.handle)?;
        // Earlier layouts are an aid, never a reason to refuse the Session.
        let store: Arc<dyn crate::layout_history::LayoutHistoryStore> =
            Arc::clone(&owners.checkpoint_store) as _;
        match crate::layout_history::LayoutHistory::load(store) {
            Ok(history) => {
                owners.terminals.install_layout_history(Arc::new(history));
            }
            Err(error) => eprintln!(
                "orchbee-server diagnostic component=checkpoint operation=layout-history stage=load cause=io:{:?}",
                error.kind()
            ),
        }
        owners.checkpoint_saver = Some(super::checkpoint_windows::autosave::Saver::start(
            Arc::clone(&owners.checkpoint_store),
            Arc::clone(&owners.terminals),
            worker,
            owners.suppress_initial_empty,
        )?);
        local_client_service::start_managed_windows(
            owners.session.client_socket_path(),
            local_client_service::ServerIdentity::local(Some(cohort), owners.worker.id()),
            Arc::clone(&owners.terminals),
            Arc::clone(&owners.agents),
            Arc::clone(&shutdown),
        )
    })();
    let mut clients: local_client_service::ManagedClients = match started {
        Ok(clients) => clients,
        Err(error) => return Err(abort_startup(owners, error)),
    };
    // Plugin startup commands may call back into the Session, so they run once it admits Clients.
    owners.terminals.start_plugin_hooks();
    let mut worker_completed = false;
    loop {
        let attempt = match next_work(&mut owners, &shutdown) {
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
        shutdown.phase(attempt, StopPhase::Completion);
        if Instant::now() >= deadline {
            return Err(io::ErrorKind::TimedOut.into());
        }
        owners.session_record.complete(attempt.id.bytes())?;
        owners.record.complete(attempt.id.bytes())?;
        drop(owners.lifetime);
        drop(activation);
        return Ok(());
    }
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
        let worker = checkpoint_worker(&owners.handle)
            .map_err(|error| failure(StopPhase::BackgroundJobs, error))?;
        settle_without_worker(owners, &worker, deadline)?;
        *worker_completed = true;
    }
    if !*worker_completed {
        let worker = checkpoint_worker(&owners.handle)
            .map_err(|error| failure(StopPhase::BackgroundJobs, error))?;
        let bytes = owners
            .terminals
            .checkpoint(&worker, deadline)
            .map_err(|error| failure(StopPhase::BackgroundJobs, error))?;
        publish_final_checkpoint(owners, &bytes)
            .map_err(|error| failure(StopPhase::BackgroundJobs, error))?;
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
/// it.
fn next_work(owners: &mut Owners, shutdown: &SessionShutdown) -> Work {
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
    }
}

/// After its Worker exited, nothing can sample a terminal's directory or stop its Host. The stop
/// finishes only when that Host is gone too, keeping each terminal's last sampled directory; a live
/// Host is refused, because nothing here may end it.
fn settle_without_worker(
    owners: &Owners,
    worker: &orchbee_workbench_protocol::WorkerId,
    deadline: Instant,
) -> Result<(), Failure> {
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
    let bytes = owners
        .terminals
        .checkpoint_without_worker(worker)
        .map_err(|error| failure(StopPhase::BackgroundJobs, error))?;
    if Instant::now() >= deadline {
        return Err(failure(
            StopPhase::BackgroundJobs,
            io::ErrorKind::TimedOut.into(),
        ));
    }
    publish_final_checkpoint(owners, &bytes)
        .map_err(|error| failure(StopPhase::BackgroundJobs, error))?;
    Ok(())
}

/// Stops every Server background job that could start new work through the Worker. Hooks submit
/// Worker jobs, so dispatch stops before any Worker owner; admitted hook jobs end with the Worker.
fn stop_background_jobs(owners: &mut Owners, deadline: Instant) -> io::Result<()> {
    owners.terminals.stop_plugin_hooks_before(deadline)?;
    // A resume types through the Worker and reads through the observer, so it stops first; one
    // not yet attempted stays pending for the final capture.
    owners.terminals.stop_agent_resume_before(deadline)?;
    // A start line types through the Worker and reads through the observer the same way.
    owners
        .terminals
        .stop_agent_start_delivery_before(deadline)?;
    if let Some(saver) = &mut owners.checkpoint_saver {
        saver.stop_before(deadline)?;
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

fn publish_final_checkpoint(owners: &Owners, bytes: &[u8]) -> io::Result<()> {
    super::checkpoint_windows::autosave::publish_final(&owners.checkpoint_store, bytes)?;
    owners.terminals.terminal_output_after_save(bytes);
    super::checkpoint_windows::autosave::record_history(&owners.terminals, bytes, &mut None);
    Ok(())
}

/// How long an `exit` may spend settling before the process ends regardless.
const EXIT_BUDGET: Duration = Duration::from_secs(30);

/// Ends this Server and its Worker for an `exit`, leaving the Host and every terminal running.
/// In-flight records and background jobs settle and the final checkpoint is published first; a
/// settling failure is reported and the process still ends. No receipt is published, so until P1
/// records this end the next activation recovers the Session as an unclean end.
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
        let bytes = owners.terminals.checkpoint_without_worker(&worker)?;
        publish_final_checkpoint(owners, &bytes)?;
    } else {
        let bytes = owners.terminals.checkpoint(&worker, deadline)?;
        publish_final_checkpoint(owners, &bytes)?;
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
            terminals: ShutdownTerminals::Keep,
        };
        match owners
            .handle
            .request(WorkerOperationRequest::Shutdown(request))
        {
            Ok(WorkerOperationResult::Shutdown(result))
                if matches!(
                    result.outcome,
                    WorkerShutdownOutcome::NoHost | WorkerShutdownOutcome::Released { .. }
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
        owners.terminals.stop_agent_resume_before(deadline)?;
        owners
            .terminals
            .stop_agent_start_delivery_before(deadline)?;
        if let Some(saver) = &mut owners.checkpoint_saver {
            saver.stop_before(deadline)?;
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
