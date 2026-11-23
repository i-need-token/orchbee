//! Managed shutdown retains creation-owned resources across explicit attempts. A stop that
//! disposes ends every Host the Worker holds, one after another, and then the Session's Keeper; one
//! that keeps the terminals releases every Host.
use super::{background_jobs::BackgroundJobs, server_transport};
use crate::host_connection::HostBrokerConnection;
use orchbee_capability_node_protocol::{
    CapabilityNodeMessage, HostEpoch, ShutdownFailure, ShutdownRefusal, ShutdownStage,
    ShutdownTerminals, WorkerShutdownOutcome, WorkerShutdownRequest, WorkerShutdownResult,
};
use orchbee_local_ipc::LocalStream;
use orchbee_local_runtime::RetainedChild;
use orchbee_local_worker_protocol::WorkerId;
use std::{
    error::Error,
    io,
    net::Shutdown,
    num::NonZeroU64,
    thread,
    time::{Duration, Instant},
};

mod host;

enum HostState {
    Connected,
    Empty,
    /// The Host exited after its terminals ended; a later attempt finds it stopped.
    Ended,
    Unavailable {
        stage: ShutdownStage,
        failure: ShutdownFailure,
    },
}

/// The process whose exit completes a managed stop: the child this Worker started, or the Host a
/// replacement Worker adopted.
enum HostOwner {
    Started(RetainedChild),
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    Adopted(crate::host_adoption::AdoptedHost),
}

impl HostOwner {
    fn exited(&mut self) -> io::Result<bool> {
        match self {
            Self::Started(child) => child.try_wait().map(|status| status.is_some()),
            #[cfg(any(target_os = "linux", target_os = "macos", windows))]
            Self::Adopted(host) => host.exited(),
        }
    }
}

/// Exact managed Host ownership; incomplete attempts never drop the owner.
pub(crate) struct ManagedHost {
    owner: HostOwner,
    state: HostState,
}

/// What a stop ends: each Host the Worker holds with its owner, the epoch the outcome names, and
/// the Session's Keeper, which ends once every Host is gone.
pub(super) struct StopHosts<'a> {
    pub(super) hosts: Vec<(&'a HostBrokerConnection, &'a mut ManagedHost)>,
    pub(super) epoch: HostEpoch,
    #[cfg(windows)]
    pub(super) keeper: &'a mut Option<crate::session_keeper::SessionKeeper>,
    #[cfg(windows)]
    pub(super) retiring: &'a mut Option<crate::session_keeper::RetiringKeeper>,
}

impl ManagedHost {
    pub(super) const fn new(child: RetainedChild) -> Self {
        Self {
            owner: HostOwner::Started(child),
            state: HostState::Connected,
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    pub(super) const fn adopted(host: crate::host_adoption::AdoptedHost) -> Self {
        Self {
            owner: HostOwner::Adopted(host),
            state: HostState::Connected,
        }
    }

    /// Whether the Host runs `executable`, this Worker's cohort Terminal Host: one it started does,
    /// and an adopted one whose image is that program.
    #[cfg(windows)]
    pub(super) fn runs(&self, executable: &std::path::Path) -> bool {
        match &self.owner {
            HostOwner::Started(_) => true,
            HostOwner::Adopted(host) => host.runs(executable),
        }
    }

    /// Whether the Host process has exited.
    #[cfg(windows)]
    pub(super) fn exited(&mut self) -> io::Result<bool> {
        self.owner.exited()
    }

    /// The Host's process ID and creation time.
    #[cfg(windows)]
    pub(super) fn host_identity(&self) -> io::Result<(u32, u64)> {
        match &self.owner {
            HostOwner::Started(child) => child.exact_identity(),
            HostOwner::Adopted(host) => host.identity(),
        }
    }

    /// The Host's process, for the hang check that may end it.
    #[cfg(windows)]
    pub(super) fn end_host(&mut self, deadline: Instant) -> io::Result<()> {
        match &mut self.owner {
            HostOwner::Started(child) => child.terminate_before(deadline).map(drop),
            HostOwner::Adopted(host) => host.end_before(deadline),
        }
    }

    fn stop(
        &mut self,
        host: &HostBrokerConnection,
        server: &LocalStream,
        request: WorkerShutdownRequest,
        deadline: Instant,
    ) -> WorkerShutdownOutcome {
        let epoch =
            HostEpoch::from_bytes(*host.host_epoch().as_bytes()).expect("admitted nonzero epoch");
        let mut disposed = 0;
        let incomplete = |stage, failure, disposed_terminals| WorkerShutdownOutcome::Incomplete {
            host_epoch: Some(epoch),
            disposed_terminals,
            stage,
            failure,
        };
        match self.state {
            HostState::Ended => {
                return WorkerShutdownOutcome::Stopped {
                    host_epoch: epoch,
                    disposed_terminals: 0,
                };
            }
            HostState::Unavailable { stage, failure } => return incomplete(stage, failure, 0),
            HostState::Connected => {
                if let Err(error) =
                    host::drain(host, server, request.operation_id, deadline, &mut disposed)
                {
                    if error.retire {
                        let _closed = host.stream().shutdown(Shutdown::Both);
                        self.state = HostState::Unavailable {
                            stage: error.stage,
                            failure: error.failure,
                        };
                    }
                    return incomplete(error.stage, error.failure, disposed);
                }
                self.state = HostState::Empty;
            }
            HostState::Empty => {}
        }
        if host.stream().shutdown(Shutdown::Both).is_err() {
            return incomplete(
                ShutdownStage::HostExit,
                ShutdownFailure::Transport,
                disposed,
            );
        }
        loop {
            match self.owner.exited() {
                Ok(true) => {
                    self.state = HostState::Ended;
                    return WorkerShutdownOutcome::Stopped {
                        host_epoch: epoch,
                        disposed_terminals: disposed,
                    };
                }
                Ok(false) => {}
                Err(_) => {
                    return incomplete(
                        ShutdownStage::HostExit,
                        ShutdownFailure::ProcessWait,
                        disposed,
                    );
                }
            }
            if !pause_before(deadline) {
                return incomplete(ShutdownStage::HostExit, ShutdownFailure::Deadline, disposed);
            }
        }
    }
}

/// Identity refusal is checked before entering stop-only mode or touching any owner.
pub(super) fn refusal(
    request: WorkerShutdownRequest,
    worker_id: WorkerId,
    generation: NonZeroU64,
    managed: bool,
) -> Option<ShutdownRefusal> {
    if request.worker_id.as_bytes() != worker_id.as_bytes()
        || request.worker_generation.get() != generation.get()
    {
        Some(ShutdownRefusal::WrongWorker)
    } else if !managed {
        Some(ShutdownRefusal::Unmanaged)
    } else {
        None
    }
}

/// After a valid stop, only explicit stop attempts are accepted; no normal admission resumes.
pub(super) fn run(
    server: &LocalStream,
    mut request: WorkerShutdownRequest,
    worker_id: WorkerId,
    generation: NonZeroU64,
    jobs: BackgroundJobs<'_>,
    mut managed: Option<StopHosts<'_>>,
) -> Result<(), Box<dyn Error>> {
    loop {
        let deadline =
            Instant::now() + Duration::from_millis(u64::from(request.budget.milliseconds()));
        let outcome = if let Some(reason) = refusal(request, worker_id, generation, true) {
            WorkerShutdownOutcome::Refused(reason)
        } else {
            match jobs.stop_before(deadline) {
                Err(error) if request.terminals == ShutdownTerminals::Dispose => {
                    WorkerShutdownOutcome::Incomplete {
                        host_epoch: managed.as_ref().map(|stop| stop.epoch),
                        disposed_terminals: 0,
                        stage: ShutdownStage::BackgroundJobs,
                        failure: if error.kind() == io::ErrorKind::TimedOut {
                            ShutdownFailure::Deadline
                        } else {
                            ShutdownFailure::ProcessWait
                        },
                    }
                }
                // Keeping the terminals leaves every one running, which no background job's end
                // changes, so a job whose end cannot be proven is recorded and this Worker still
                // ends: otherwise one stuck job would keep its Server from ever ending.
                Err(error) => {
                    crate::diagnostic_events::cleanup_unproven(&error);
                    end(&mut managed, server, request, deadline)
                }
                Ok(()) => end(&mut managed, server, request, deadline),
            }
        };
        let completed = matches!(
            outcome,
            WorkerShutdownOutcome::NoHost
                | WorkerShutdownOutcome::Stopped { .. }
                | WorkerShutdownOutcome::Released { .. }
        );
        let _no_host_monitor = server_transport::send(
            server,
            None,
            &CapabilityNodeMessage::WorkerShutdownResult(WorkerShutdownResult { request, outcome }),
        )?;
        if completed {
            return Ok(());
        }
        match server_transport::receive(server, None)? {
            CapabilityNodeMessage::WorkerShutdownRequest(next) => request = next,
            // After shutdown admission, only shutdown continuation traffic can advance this exchange.
            _ => {
                return Err(
                    io::Error::new(io::ErrorKind::InvalidData, "Worker is stopping").into(),
                );
            }
        }
    }
}

/// Ends every Host's terminals and the Hosts, or releases them, once the background jobs are
/// settled. The outcome names the epoch `managed` gives and counts the terminals every Host ended
/// in this attempt; the first Host that does not stop names its own epoch.
fn end(
    managed: &mut Option<StopHosts<'_>>,
    server: &LocalStream,
    request: WorkerShutdownRequest,
    deadline: Instant,
) -> WorkerShutdownOutcome {
    let Some(stop) = managed.as_mut() else {
        return WorkerShutdownOutcome::NoHost;
    };
    if request.terminals == ShutdownTerminals::Keep {
        for (host, _) in &stop.hosts {
            release(host);
        }
        return WorkerShutdownOutcome::Released {
            host_epoch: stop.epoch,
        };
    }
    let mut disposed = 0_u32;
    for (host, owner) in &mut stop.hosts {
        match owner.stop(host, server, request, deadline) {
            WorkerShutdownOutcome::Stopped {
                disposed_terminals, ..
            } => disposed = disposed.saturating_add(disposed_terminals),
            WorkerShutdownOutcome::Incomplete {
                host_epoch,
                disposed_terminals,
                stage,
                failure,
            } => {
                return WorkerShutdownOutcome::Incomplete {
                    host_epoch,
                    disposed_terminals: disposed.saturating_add(disposed_terminals),
                    stage,
                    failure,
                };
            }
            other => return other,
        }
    }
    // Each Host released every holding before it ended its terminals, so the Keeper holds nothing
    // and ending it ends no program.
    #[cfg(windows)]
    if let Some(mut keeper) = stop.keeper.take() {
        keeper.end(deadline);
    }
    #[cfg(windows)]
    if let Some(mut retiring) = stop.retiring.take() {
        retiring.end_with_session(deadline);
    }
    WorkerShutdownOutcome::Stopped {
        host_epoch: stop.epoch,
        disposed_terminals: disposed,
    }
}

/// A Server `exit` ends this Worker without ending any terminal: closing the broker connection
/// leaves the Host with every terminal unchanged, and the retained Host child or adopted Host
/// owner is dropped without a wait or a termination.
fn release(host: &HostBrokerConnection) {
    let _closed = host.stream().shutdown(Shutdown::Both);
}

fn pause_before(deadline: Instant) -> bool {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return false;
    }
    thread::sleep(remaining.min(Duration::from_millis(10)));
    true
}

#[cfg(all(test, windows))]
mod tests;
