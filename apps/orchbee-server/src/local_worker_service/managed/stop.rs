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
mod handover;
#[cfg(any(windows, target_os = "macos"))]
mod idle;
mod worker_loss;

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
    /// Whether this generation adopted the Host its predecessor left running, whose terminals a
    /// failed start keeps for the next activation.
    pub(super) adopted: bool,
    /// The Worker endpoint, which admits a replacement Worker only while one is being started.
    pub(super) listener: orchbee_local_ipc::LocalListener,
    pub(super) registry: Arc<std::sync::Mutex<crate::worker_registry::WorkerRegistry>>,
    /// Wakes this owner's wait when an admitted Worker's connection ends.
    pub(super) waker: std::sync::Weak<SessionShutdown>,
    pub(super) worker_executable: std::path::PathBuf,
    /// The Worker `query` lists: the serving one, or none.
    pub(super) serving: local_client_service::ServingWorker,
    /// The Agent observation endpoint; none on Windows, which binds none.
    pub(super) ingress: Option<AgentObservationEndpoint>,
    pub(super) terminals: Arc<LocalTerminalService>,
    pub(super) agents: Arc<AgentService>,
    /// The checkpoint this Session keeps; none where the platform keeps none.
    pub(super) checkpoint: Option<super::checkpoint::Kept>,
    #[cfg(any(windows, target_os = "macos"))]
    pub(super) automatic_updates: Option<crate::automatic_updates::AutomaticUpdates>,
    /// What admitted Clients' checks for updates reach, from before the listener opens.
    pub(super) update_job: Arc<crate::update_job::UpdateJob>,
    pub(super) session_record: RetainedSessionRecord,
    /// The ticket of the restart that started this Server, which names its Worker endpoint for
    /// life; none for a Server an activation started.
    #[cfg(windows)]
    pub(super) worker_ticket: Option<orchbee_connection_handshake::HandoverTicket>,
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
    // Plugin startup commands may call back into the Session, so they run once it admits Clients.
    #[cfg(windows)]
    owners.terminals.start_plugin_hooks();
    // The schedule counts from the moment the Session admits Clients.
    #[cfg(any(windows, target_os = "macos"))]
    {
        owners.automatic_updates = crate::automatic_updates::AutomaticUpdates::start(
            Arc::clone(&owners.terminals),
            Arc::clone(&owners.update_job),
        );
    }
    // Only Windows and macOS keep a release store, so elsewhere no job ever runs.
    #[cfg(all(unix, not(target_os = "macos")))]
    owners.update_job.end();
    let mut worker_completed = false;
    #[cfg(windows)]
    let mut idle = idle::IdleMove::default();
    let mut loss = worker_loss::WorkerLoss::default();
    let mut supervision = super::replacement::Supervision::new(cohort);
    #[cfg(windows)]
    let mut restart = handover::Restart::default();
    // Every stop a POSIX Server begins uses its one identity, the idle move's too.
    #[cfg(target_os = "macos")]
    let mut idle = idle::IdleMove::under(own);
    loop {
        let watch = Watch {
            loss: &mut loss,
            supervision: &mut supervision,
        };
        // The Session record names this Server's successor, which holds the Session from now on.
        #[cfg(windows)]
        let Some(work) = next_work(
            &mut owners,
            &shutdown,
            &clients,
            &mut idle,
            watch,
            &mut restart,
        ) else {
            return handover::leave(owners, clients, &shutdown);
        };
        #[cfg(target_os = "macos")]
        let work = next_work(&mut owners, &shutdown, own, &mut idle, watch);
        #[cfg(all(unix, not(target_os = "macos")))]
        let work = next_work(&mut owners, &shutdown, own, watch);
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
            #[cfg(unix)]
            if shutdown.signalled() && matches!(owners.worker.try_wait(), Ok(Some(_))) {
                return leave(clients, &shutdown, error);
            }
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
        // A stop the idle move began names the version the Session moved to, so a Client that
        // lost this Server starts that version.
        #[cfg(any(windows, target_os = "macos"))]
        let moved_to = idle.moved_to(attempt.id);
        #[cfg(all(unix, not(target_os = "macos")))]
        let moved_to = None;
        owners
            .session_record
            .complete_moving(attempt.id.bytes(), moved_to)?;
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
    #[cfg(any(windows, target_os = "macos"))]
    owners.terminals.bind_agent_names(&owners.agents)?;
    owners.terminals.start_close_maintenance()?;
    owners.terminals.start_title_reads()?;
    owners.terminals.start_agent_catalog()?;
    owners.terminals.start_agent_start_delivery()?;
    #[cfg(any(windows, target_os = "macos"))]
    let mut restored = restored;
    #[cfg(any(windows, target_os = "macos"))]
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
    #[cfg(any(windows, target_os = "macos"))]
    owners.terminals.start_agent_resume(resumes)?;
    if let Some(kept) = &mut owners.checkpoint {
        kept.start_saver(&owners.terminals, checkpoint_worker(&owners.handle)?)?;
    }
    let identity = local_client_service::ServerIdentity::local(Some(cohort), owners.worker.id());
    owners.serving = identity.serving_worker();
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
        identity,
        Arc::clone(&owners.terminals),
        Arc::clone(&owners.agents),
        Arc::clone(shutdown),
        Arc::clone(&owners.update_job),
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
        settle_without_worker(owners, shutdown, attempt, deadline)?;
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
            kept.publish_final(&owners.terminals, &worker, &bytes)
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
    // The Worker's disposal, or the stop after that Worker exited, ended the Host, so no running
    // terminal is left to adopt.
    if let Some(kept) = &owners.checkpoint {
        kept.remove_bindings();
    }
    shutdown.phase(attempt, StopPhase::Ingress);
    stop_ingress(owners, deadline).map_err(|error| {
        let mut error = failure(StopPhase::Ingress, error);
        error.disposed = disposed;
        error
    })?;
    Ok(())
}

/// What the stop loop watches its Worker with: the loss the fallback counts, and the supervision
/// that replaces a lost Worker.
struct Watch<'a> {
    loss: &'a mut worker_loss::WorkerLoss,
    supervision: &'a mut super::replacement::Supervision,
}

impl Watch<'_> {
    /// One look at the Worker. Returns whether the Server should begin its own stop: its Worker
    /// was lost without a serving replacement and that Worker's Host is gone, so no terminal can
    /// run here again. While that Host still runs, the Server keeps running beside it.
    fn look(&mut self, owners: &mut Owners, shutdown: &SessionShutdown) -> bool {
        let exited = matches!(owners.worker.try_wait(), Ok(Some(_)));
        self.loss.look(exited, shutdown.stopping(), || {
            worker_loss::record(&owners.session_record);
        });
        self.supervision.look(owners, shutdown)
    }
}

/// Waits for a stop request or an `exit`, answering the `handover` steps of a restart meanwhile.
/// Each tick looks at the Worker, which replaces a lost one, and feeds the idle move; a Worker exit
/// no stop explains is recorded as this generation's failure first. While a restart holds the
/// Session it watches only the restart. `None` means the Session record names the successor.
#[cfg(windows)]
fn next_work(
    owners: &mut Owners,
    shutdown: &SessionShutdown,
    clients: &local_client_service::ManagedClients,
    idle: &mut idle::IdleMove,
    mut watch: Watch<'_>,
    restart: &mut handover::Restart,
) -> Option<Work> {
    loop {
        let tick = if restart.holding() {
            handover::HELD_TICK
        } else {
            Duration::from_secs(1)
        };
        if let Some(work) = shutdown.take_within(tick) {
            // An `exit` decides a held restart first, the hold refusing stops, and a stop or an
            // `exit` clears one that never got so far.
            if restart.settle_before_ending(owners, shutdown) == handover::Next::Leave {
                return None;
            }
            return Some(work);
        }
        if let Some((step, ticket)) = shutdown.take_handover() {
            if restart.step(owners, shutdown, clients, &mut watch, step, ticket)
                == handover::Next::Leave
            {
                return None;
            }
            continue;
        }
        if restart.holding() {
            if restart.tick(owners, shutdown, &mut watch) == handover::Next::Leave {
                return None;
            }
            continue;
        }
        restart.look(owners);
        if watch.look(owners, shutdown)
            && !shutdown.stopping()
            && let Some(id) = own_stop_id()
        {
            shutdown.begin(id);
        }
        idle.tick(owners, shutdown);
    }
}

/// [`next_work`] as on Windows, except that a failed stop this Server began after losing its Worker
/// and Host is tried again once that Worker and Host are gone. Every stop this Server begins uses
/// `own`, the identity its signal handler uses too. On macOS each tick also feeds the idle move.
#[cfg(unix)]
fn next_work(
    owners: &mut Owners,
    shutdown: &SessionShutdown,
    own: StopId,
    #[cfg(target_os = "macos")] idle: &mut idle::IdleMove,
    mut watch: Watch<'_>,
) -> Work {
    loop {
        if let Some(work) = shutdown.take_within(Duration::from_secs(1)) {
            return work;
        }
        if watch.look(owners, shutdown) {
            begin_own(shutdown, own);
        }
        #[cfg(target_os = "macos")]
        idle.tick(owners, shutdown);
    }
}

/// Begins the stop this Server owns under `own`, or queues its next attempt once one failed. A
/// stop a Client recorded stays that Client's to retry; only a signal retries it.
#[cfg(unix)]
fn begin_own(shutdown: &SessionShutdown, own: StopId) {
    if !shutdown.begin(own) {
        shutdown.request(&orchbee_local_client_protocol::SessionStopRequest {
            id: own,
            action: orchbee_local_client_protocol::StopAction::Retry,
        });
    }
}

/// After its Worker exited, nothing can sample a terminal's directory, and the final capture keeps
/// each terminal's last sampled directory. The stop then ends that Worker's Host after the final
/// capture, as the Worker's own disposal would have, on every platform alike: a stop is never
/// refused only because its Worker is gone.
fn settle_without_worker(
    owners: &Owners,
    shutdown: &SessionShutdown,
    attempt: Attempt,
    deadline: Instant,
) -> Result<(), Failure> {
    let worker = checkpoint_worker(&owners.handle)
        .map_err(|error| failure(StopPhase::BackgroundJobs, error))?;
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
        kept.publish_final(&owners.terminals, &worker, &bytes)
            .map_err(|error| failure(StopPhase::BackgroundJobs, error))?;
    }
    shutdown.phase(attempt, StopPhase::HostExit);
    end_orphaned_host(owners, deadline).map_err(|error| failure(StopPhase::HostExit, error))?;
    // A Keeper may hold the last handles of a program whose Host ended, so the stop ends it once
    // the Host is gone, checked by its recorded identity; stopping still ends every program.
    #[cfg(windows)]
    orchbee_local_runtime::end_recorded_keeper(&owners.session_record, deadline)
        .map_err(|error| failure(StopPhase::HostExit, error))?;
    Ok(())
}

/// The longest a stop, or a POSIX `exit`, waits for an orphaned Host to end every terminal and exit.
const ORPHANED_HOST_BUDGET: Duration = WORKER_STOP_BUDGET;

/// Ends the Host a lost Worker left behind, the only way left to end its terminals: the Host's
/// `exit` control request ends every retained terminal through the Host's ordinary teardown and
/// then the Host. It returns once [`host_absent`] observes the Host gone, its lock free on Linux and
/// macOS and its registered identity gone on Windows, and fails at `deadline` or
/// [`ORPHANED_HOST_BUDGET`], whichever comes first. A Host that is already gone, or that ends while
/// the request is lost, counts as ended; the Host is never ended by force.
fn end_orphaned_host(owners: &Owners, deadline: Instant) -> io::Result<()> {
    if host_absent(owners) {
        return Ok(());
    }
    let deadline = deadline.min(Instant::now() + ORPHANED_HOST_BUDGET);
    let endpoint =
        ModeCheckedLocalSessionRendezvous::host_endpoint_path(owners.session.runtime_directory());
    let environment = orchbee_connection_handshake::EnvironmentName::new(
        orchbee_local_runtime::environment_stem(),
    )
    .map_err(io::Error::other)?;
    let reply =
        orchbee_local_client_protocol::exchange_exit_before(&endpoint, environment, deadline, None);
    let submitted = match reply {
        Ok(orchbee_local_client_protocol::ExitReply::Ending) => true,
        Ok(orchbee_local_client_protocol::ExitReply::Unsupported) => {
            return Err(io::ErrorKind::Unsupported.into());
        }
        Err(error) => error.submitted,
    };
    loop {
        if host_absent(owners) {
            return Ok(());
        }
        // A request that never reached the Host leaves it running.
        if !submitted {
            return Err(io::ErrorKind::NotConnected.into());
        }
        if Instant::now() >= deadline {
            return Err(io::ErrorKind::TimedOut.into());
        }
        thread::sleep(Duration::from_millis(10));
    }
}

/// After a stop signal, a failed attempt with no Worker left ends the process, so a signal always
/// ends the Server within one attempt's budget: nothing here can still change a terminal, and a
/// logout or a shutdown would end the process anyway. No receipt is published, so the next start
/// recovers the Session as after a crash once the Host's lock is free, and refuses while a Host
/// that could not be ended still runs.
#[cfg(unix)]
fn leave(
    mut clients: local_client_service::ManagedClients,
    shutdown: &SessionShutdown,
    failed: Failure,
) -> io::Result<()> {
    eprintln!(
        "orchbee-server diagnostic component=lifecycle operation=stop stage=leave phase={:?} failure={:?}",
        failed.phase, failed.failure
    );
    let deadline = Instant::now() + LEAVE_BUDGET;
    shutdown.end(deadline);
    clients.stop_before(deadline)?;
    Err(io::Error::other(
        "Session stop failed after its Worker exited",
    ))
}

/// How long [`leave`] gives the Client listeners to close.
#[cfg(unix)]
const LEAVE_BUDGET: Duration = Duration::from_secs(5);

/// Stops the Agent observation endpoint where the Session bound one.
fn stop_ingress(owners: &mut Owners, deadline: Instant) -> io::Result<()> {
    owners
        .ingress
        .as_mut()
        .map_or(Ok(()), |ingress| ingress.stop_before(deadline))
}

/// Stops every Server background job that could start new work through the Worker. Hooks submit
/// Worker jobs, so dispatch stops before any Worker owner; admitted hook jobs end with the Worker.
fn stop_background_jobs(owners: &mut Owners, deadline: Instant) -> io::Result<()> {
    // An automatic update holds no Session resource, so its stop never fails.
    #[cfg(any(windows, target_os = "macos"))]
    if let Some(updates) = owners.automatic_updates.take() {
        updates.stop();
    }
    #[cfg(windows)]
    owners.terminals.stop_plugin_hooks_before(deadline)?;
    #[cfg(any(windows, target_os = "macos"))]
    {
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
/// the Host, or, once that Worker exited, the Server ends its Host, and a Session that keeps a
/// checkpoint relaunches them on its next start. In-flight
/// records and background jobs settle and the final checkpoint is published first; a settling
/// failure is reported and the process still ends. No receipt is published: once the Worker is
/// gone the Session record names this generation as exited, so the next activation recovers the
/// Session without counting a crash.
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
            kept.publish_final(&owners.terminals, &worker, &bytes)?;
        }
        // A POSIX `exit` disposes the terminals, which only the Worker's Host can still end.
        #[cfg(unix)]
        end_orphaned_host(owners, deadline)?;
    } else {
        if let Some(kept) = &owners.checkpoint {
            let bytes = owners.terminals.checkpoint(&worker, deadline)?;
            kept.publish_final(&owners.terminals, &worker, &bytes)?;
        }
        end_worker(owners, deadline)?;
        owners
            .terminals
            .finish_process_observer_after_worker_exit(&owners.handle)?;
    }
    // A failed write leaves the next activation to count this end as a crash, as before.
    if let Err(error) = owners.session_record.record_exit(deadline) {
        eprintln!(
            "orchbee-server diagnostic component=lifecycle operation=exit stage=record cause=io:{:?}",
            error.kind()
        );
    }
    stop_ingress(owners, deadline)?;
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

/// Ends the Worker as an `exit` does, its terminals kept on Windows and disposed on POSIX, and
/// waits until it and its connection's thread are gone or `deadline` passes.
fn end_worker(owners: &mut Owners, deadline: Instant) -> io::Result<()> {
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
    Ok(())
}

/// Only an exact observation that the Host is gone, or that it never started, counts.
pub(super) fn host_absent(owners: &Owners) -> bool {
    matches!(owners.session_record.host_absent(), Ok(true))
}

/// Only an exact observation that the Keeper is gone, or that none started, counts; a running
/// Keeper keeps the programs of a Host that ended. POSIX starts none.
pub(super) fn keeper_absent(owners: &Owners) -> bool {
    matches!(
        owners.session_record.keeper_presence(),
        Ok(orchbee_local_runtime::HostPresence::Gone
            | orchbee_local_runtime::HostPresence::NotStarted)
    )
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

/// Startup has no Client requests, but restoration may already have created a Host. A generation
/// that adopted its predecessor's Host keeps it and its terminals, which the next activation adopts
/// again; any other ends its Host.
fn abort_startup(mut owners: Owners, startup: io::Error) -> io::Error {
    let deadline = Instant::now() + Duration::from_secs(30);
    let cleanup = (|| -> io::Result<()> {
        #[cfg(any(windows, target_os = "macos"))]
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
            terminals: if owners.adopted {
                ShutdownTerminals::Keep
            } else {
                ShutdownTerminals::Dispose
            },
        };
        match owners
            .handle
            .request(WorkerOperationRequest::Shutdown(request))
        {
            Ok(WorkerOperationResult::Shutdown(result))
                if matches!(
                    result.outcome,
                    WorkerShutdownOutcome::NoHost
                        | WorkerShutdownOutcome::Stopped { .. }
                        | WorkerShutdownOutcome::Released { .. }
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
        stop_ingress(&mut owners, deadline)?;
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
pub(super) fn checkpoint_worker(
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
