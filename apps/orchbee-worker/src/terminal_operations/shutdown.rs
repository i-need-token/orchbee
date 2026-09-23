//! Windows managed shutdown retains creation-owned resources across explicit attempts.
use super::server_transport;
use crate::{host_connection::HostBrokerConnection, worktree_discovery_job::WorktreeDiscoverySlot};
use orchbee_capability_node_protocol::{
    CapabilityNodeMessage, HostEpoch, ShutdownFailure, ShutdownRefusal, ShutdownStage,
    WorkerShutdownOutcome, WorkerShutdownRequest, WorkerShutdownResult,
};
use orchbee_local_ipc::LocalStream;
use orchbee_local_site_protocol::WorkerId;
use orchbee_local_site_runtime::RetainedChild;
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

/// Exact managed Host ownership; incomplete attempts never drop the child handle.
pub(crate) struct ManagedHost {
    child: RetainedChild,
    state: HostState,
}

impl ManagedHost {
    pub(super) const fn new(child: RetainedChild) -> Self {
        Self {
            child,
            state: HostState::Connected,
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
            match self.child.try_wait() {
                Ok(Some(_)) => {
                    return WorkerShutdownOutcome::Stopped {
                        host_epoch: epoch,
                        disposed_terminals: disposed,
                    };
                }
                Ok(None) => {}
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
    slot: &WorktreeDiscoverySlot,
    mut managed: Option<(&HostBrokerConnection, &mut ManagedHost)>,
) -> Result<(), Box<dyn Error>> {
    loop {
        let deadline =
            Instant::now() + Duration::from_millis(u64::from(request.budget.milliseconds()));
        let outcome = if let Some(reason) = refusal(request, worker_id, generation, true) {
            WorkerShutdownOutcome::Refused(reason)
        } else if let Err(error) = slot.stop_before(deadline) {
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
            owner.stop(host, server, request, deadline)
        } else {
            WorkerShutdownOutcome::NoHost
        };
        let completed = matches!(
            outcome,
            WorkerShutdownOutcome::NoHost | WorkerShutdownOutcome::Stopped { .. }
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
            _ => {
                return Err(
                    io::Error::new(io::ErrorKind::InvalidData, "Worker is stopping").into(),
                );
            }
        }
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

#[cfg(test)]
mod tests;
