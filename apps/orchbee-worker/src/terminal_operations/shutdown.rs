//! Managed shutdown retains creation-owned resources across explicit attempts.
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
    /// The Session's Keeper, which a stop ends once the Host is gone.
    #[cfg(windows)]
    keeper: Option<crate::session_keeper::SessionKeeper>,
    /// The Session runtime folder and the Terminal Host program a Keeper for this Host starts from.
    #[cfg(windows)]
    keeper_program: Option<(std::path::PathBuf, std::path::PathBuf)>,
}

impl ManagedHost {
    pub(super) const fn new(child: RetainedChild) -> Self {
        Self {
            owner: HostOwner::Started(child),
            state: HostState::Connected,
            #[cfg(windows)]
            keeper: None,
            #[cfg(windows)]
            keeper_program: None,
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    pub(super) const fn adopted(host: crate::host_adoption::AdoptedHost) -> Self {
        Self {
            owner: HostOwner::Adopted(host),
            state: HostState::Connected,
            #[cfg(windows)]
            keeper: None,
            #[cfg(windows)]
            keeper_program: None,
        }
    }

    /// Names where a Keeper for this Host starts from: the Session runtime folder `runtime` and
    /// the Terminal Host program `executable`.
    #[cfg(windows)]
    pub(super) fn keep_from(&mut self, runtime: &std::path::Path, executable: std::path::PathBuf) {
        self.keeper_program = Some((runtime.to_owned(), executable));
    }

    /// Where a Keeper for this Host starts from, once named.
    #[cfg(windows)]
    pub(super) fn keeper_program(&self) -> Option<(&std::path::Path, &std::path::Path)> {
        self.keeper_program
            .as_ref()
            .map(|(runtime, executable)| (runtime.as_path(), executable.as_path()))
    }

    /// Keeps `keeper`, the Session's Keeper this Worker started or adopted, until a stop ends it.
    #[cfg(windows)]
    pub(super) fn keep(&mut self, keeper: Option<crate::session_keeper::SessionKeeper>) {
        self.keeper = keeper;
    }

    /// The Session's Keeper, while this Worker answers for one.
    #[cfg(windows)]
    pub(super) fn keeper(&mut self) -> Option<&mut crate::session_keeper::SessionKeeper> {
        self.keeper.as_mut()
    }

    /// Takes the Session's Keeper from this owner, for its replacement.
    #[cfg(windows)]
    pub(super) fn take_keeper(&mut self) -> Option<crate::session_keeper::SessionKeeper> {
        self.keeper.take()
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
                    // The Host released every holding before it ended its terminals, so the
                    // Keeper holds nothing and ending it ends no program.
                    #[cfg(windows)]
                    if let Some(mut keeper) = self.keeper.take() {
                        keeper.end(deadline);
                    }
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
    mut managed: Option<(&HostBrokerConnection, &mut ManagedHost)>,
) -> Result<(), Box<dyn Error>> {
    loop {
        let deadline =
            Instant::now() + Duration::from_millis(u64::from(request.budget.milliseconds()));
        let outcome = if let Some(reason) = refusal(request, worker_id, generation, true) {
            WorkerShutdownOutcome::Refused(reason)
        } else if let Err(error) = jobs.stop_before(deadline) {
            WorkerShutdownOutcome::Incomplete {
                host_epoch: managed.as_ref().map(|(host, _)| {
                    HostEpoch::from_bytes(*host.host_epoch().as_bytes())
                        .expect("admitted nonzero epoch")
                }),
                disposed_terminals: 0,
                stage: ShutdownStage::BackgroundJobs,
                failure: if error.kind() == io::ErrorKind::TimedOut {
                    ShutdownFailure::Deadline
                } else {
                    ShutdownFailure::ProcessWait
                },
            }
        } else if let Some((host, owner)) = managed.as_mut() {
            match request.terminals {
                ShutdownTerminals::Dispose => owner.stop(host, server, request, deadline),
                ShutdownTerminals::Keep => release(host),
            }
        } else {
            WorkerShutdownOutcome::NoHost
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

/// A Server `exit` ends this Worker without ending any terminal: closing the broker connection
/// leaves the Host with every terminal unchanged, and the retained Host child or adopted Host
/// owner is dropped without a wait or a termination.
fn release(host: &HostBrokerConnection) -> WorkerShutdownOutcome {
    let _closed = host.stream().shutdown(Shutdown::Both);
    WorkerShutdownOutcome::Released {
        host_epoch: HostEpoch::from_bytes(*host.host_epoch().as_bytes())
            .expect("admitted nonzero epoch"),
    }
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
