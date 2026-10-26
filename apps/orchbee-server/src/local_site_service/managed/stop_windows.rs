//! Managed Windows shutdown settles creation-owned resources before publishing restart authority.
use super::super::agent_observation_endpoint::AgentObservationEndpoint;
use crate::{
    agent_service::AgentService,
    local_client_service,
    local_terminal_service::LocalTerminalService,
    site_shutdown::{Attempt, SiteShutdown},
    worker_obcn_carrier::{
        WorkerOperationRequest, WorkerOperationResult, WorkerRequestError, WorkerRequestHandle,
    },
};
use orchbee_capability_node_protocol::{
    OperationId, ShutdownBudget, ShutdownFailure, ShutdownStage, WorkerGeneration, WorkerId,
    WorkerShutdownOutcome, WorkerShutdownRequest,
};
use orchbee_local_client_protocol::{StopFailure, StopPhase};
use orchbee_local_site_runtime::{
    LockAttempt, ModeCheckedLocalSiteRendezvous, ModeCheckedSiteLock, RetainedActivationRecord,
    RetainedChild, WindowsRetainedSessionRecord, WindowsSessionStore,
};
use std::{
    io,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

pub(super) struct Owners {
    pub(super) site: ModeCheckedLocalSiteRendezvous,
    pub(super) lifetime: ModeCheckedSiteLock,
    pub(super) record: RetainedActivationRecord,
    pub(super) worker: RetainedChild,
    pub(super) carrier: thread::JoinHandle<()>,
    pub(super) handle: WorkerRequestHandle,
    pub(super) ingress: AgentObservationEndpoint,
    pub(super) terminals: Arc<LocalTerminalService>,
    pub(super) agents: Arc<AgentService>,
    pub(super) checkpoint_store: Arc<Mutex<WindowsSessionStore>>,
    pub(super) checkpoint_saver: Option<super::checkpoint_windows::autosave::Saver>,
    pub(super) suppress_initial_empty: bool,
    pub(super) session_record: Option<WindowsRetainedSessionRecord>,
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
    restored: crate::local_terminal_service::CheckpointRestore,
    startup_deadline: Instant,
    settings: orchbee_local_site_runtime::WindowsUserSettings,
) -> io::Result<()> {
    let shutdown = Arc::new(SiteShutdown::default());
    let started = (|| {
        owners.terminals.bind_agent_names(&owners.agents)?;
        owners.terminals.start_close_maintenance()?;
        owners
            .terminals
            .launch_checkpoint_terminals(restored, startup_deadline)?;
        owners
            .terminals
            .start_process_observer_with_settings(settings, true)?;
        let worker = checkpoint_worker(&owners.handle)?;
        owners.checkpoint_saver = Some(super::checkpoint_windows::autosave::Saver::start(
            Arc::clone(&owners.checkpoint_store),
            Arc::clone(&owners.terminals),
            worker,
            owners.suppress_initial_empty,
        )?);
        local_client_service::start_managed_windows(
            owners.site.client_socket_path(),
            cohort,
            Arc::clone(&owners.terminals),
            Arc::clone(&owners.agents),
            Arc::clone(&shutdown),
        )
    })();
    let mut clients: local_client_service::ManagedClients = match started {
        Ok(clients) => clients,
        Err(error) => return Err(abort_startup(owners, error)),
    };
    let mut worker_completed = false;
    loop {
        let attempt = shutdown.take();
        let deadline = Instant::now() + Duration::from_secs(60);
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
                .site
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
        // After control-listener closure, only a valid durable record can prove success. A failure
        // exits with the active/invalid record intact; it never silently authorizes restart.
        clients.stop_before(deadline)?;
        shutdown.phase(attempt, StopPhase::Completion);
        if Instant::now() >= deadline {
            return Err(io::ErrorKind::TimedOut.into());
        }
        if let Some(record) = &mut owners.session_record {
            record.complete(attempt.id.bytes())?;
        }
        owners.record.complete(attempt.id.bytes())?;
        drop(owners.lifetime);
        drop(activation);
        return Ok(());
    }
}

fn settle(
    owners: &mut Owners,
    shutdown: &SiteShutdown,
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
    if let Some(saver) = &mut owners.checkpoint_saver {
        saver
            .stop_before(deadline)
            .map_err(|error| failure(StopPhase::BackgroundJobs, error))?;
    }
    owners
        .terminals
        .stop_process_observer_before(deadline)
        .map_err(|error| failure(StopPhase::BackgroundJobs, error))?;
    owners
        .terminals
        .stop_worktrees_before(deadline)
        .map_err(|error| failure(StopPhase::BackgroundJobs, error))?;
    owners
        .terminals
        .stop_workspace_groups_before(deadline)
        .map_err(|error| failure(StopPhase::BackgroundJobs, error))?;
    owners
        .terminals
        .stop_close_before(deadline)
        .map_err(|error| failure(StopPhase::BackgroundJobs, error))?;
    owners
        .terminals
        .stop_graphics_before(deadline)
        .map_err(|error| failure(StopPhase::BackgroundJobs, error))?;
    if !*worker_completed {
        let worker = checkpoint_worker(&owners.handle)
            .map_err(|error| failure(StopPhase::BackgroundJobs, error))?;
        let bytes = owners
            .terminals
            .checkpoint(&worker, deadline)
            .map_err(|error| failure(StopPhase::BackgroundJobs, error))?;
        super::checkpoint_windows::autosave::publish_final(&owners.checkpoint_store, &bytes)
            .map_err(|error| failure(StopPhase::BackgroundJobs, error))?;
        shutdown.phase(attempt, StopPhase::Worker);
        let budget = deadline
            .saturating_duration_since(Instant::now())
            .as_millis()
            .min(15_000) as u32;
        let request = WorkerShutdownRequest {
            operation_id: OperationId::from_bytes(attempt.id.bytes())
                .expect("nonzero stop identity"),
            worker_id: WorkerId::from_bytes(owners.handle.worker_id().as_bytes())
                .expect("admitted Worker"),
            worker_generation: WorkerGeneration::new(owners.handle.generation()),
            budget: ShutdownBudget::new(budget)
                .map_err(|_| failure(StopPhase::Worker, io::ErrorKind::TimedOut.into()))?,
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
        if let Some(saver) = &mut owners.checkpoint_saver {
            saver.stop_before(deadline)?;
        }
        owners.terminals.stop_process_observer_before(deadline)?;
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
