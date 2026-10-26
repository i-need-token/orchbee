//! Explicit transfer of the single Server wire; physical owners remain in the same loop.
use super::server_transport;
use crate::host_connection::HostBrokerConnection;
use orchbee_capability_node_protocol::{
    CapabilityNodeMessage as Message, WorkerHandoffAction as Action, WorkerHandoffId,
    WorkerHandoffOutcome as Outcome, WorkerHandoffRefusal as Refusal, WorkerHandoffRequest,
    WorkerHandoffResult, WorkerHandoffTarget,
};
use orchbee_local_ipc::LocalStream;
use orchbee_local_worker_protocol::WorkerId;
use std::{
    collections::BTreeSet,
    error::Error,
    net::Shutdown,
    num::NonZeroU64,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

mod standby;
#[cfg(test)]
mod tests;

const MAXIMUM_ATTEMPTS: usize = 64;
const PREPARED_LEASE: Duration = Duration::from_secs(15);
const HANDSHAKE_BUDGET: Duration = Duration::from_secs(2);

struct Prepared {
    target: WorkerHandoffTarget,
    expires: Instant,
    stream: LocalStream,
}

/// Owns one standby wire and nonreusable tickets within the existing Worker loop.
pub(super) struct Handoff {
    runtime: Option<PathBuf>,
    spent: BTreeSet<WorkerHandoffId>,
    prepared: Option<Prepared>,
}

impl Handoff {
    pub(super) fn new(runtime: Option<&Path>) -> Self {
        Self {
            runtime: runtime.map(Path::to_owned),
            spent: BTreeSet::new(),
            prepared: None,
        }
    }

    pub(super) fn deadline(&self) -> Option<Instant> {
        self.prepared.as_ref().map(|prepared| prepared.expires)
    }

    pub(super) fn expire(&mut self, now: Instant) {
        if self.deadline().is_some_and(|deadline| now >= deadline) {
            self.prepared.take();
        }
    }

    pub(super) fn is_prepared(&self) -> bool {
        self.prepared.is_some()
    }

    fn refusal(
        &self,
        request: WorkerHandoffRequest,
        worker_id: WorkerId,
        generation: NonZeroU64,
        attachments_empty: bool,
    ) -> Option<Refusal> {
        if request.worker_id.as_bytes() != worker_id.as_bytes()
            || request.worker_generation.get() != generation.get()
        {
            return Some(Refusal::WrongWorker);
        }
        if self.runtime.is_none() {
            return Some(Refusal::Unmanaged);
        }
        if let Some(prepared) = &self.prepared {
            return if request.target != prepared.target {
                Some(Refusal::TicketMismatch)
            } else if request.action == Action::Prepare {
                Some(Refusal::Busy)
            } else {
                None
            };
        }
        if request.action != Action::Prepare {
            return Some(Refusal::NotPrepared);
        }
        if !attachments_empty {
            return Some(Refusal::AttachmentsBusy);
        }
        if self.spent.contains(&request.target.handoff_id) {
            return Some(Refusal::ReusedTicket);
        }
        if self.spent.len() >= MAXIMUM_ATTEMPTS {
            return Some(Refusal::Capacity);
        }
        None
    }

    pub(super) fn execute(
        &mut self,
        server: &mut LocalStream,
        host: Option<&HostBrokerConnection>,
        request: WorkerHandoffRequest,
        worker_id: WorkerId,
        generation: NonZeroU64,
    ) -> Result<(), Box<dyn Error>> {
        self.expire(Instant::now());
        let empty = host.is_none_or(|host| {
            host.viewers.borrow().is_empty() && host.controllers.borrow().is_empty()
        });
        let refusal = self.refusal(request, worker_id, generation, empty);
        if let Some(reason) = refusal {
            return reply(server, host, request, Outcome::Refused(reason));
        }
        match request.action {
            Action::Prepare => {
                self.spent.insert(request.target.handoff_id);
                let runtime = self.runtime.as_deref().expect("managed admission");
                match standby::connect(
                    runtime,
                    request.peer(),
                    server,
                    host,
                    Instant::now() + HANDSHAKE_BUDGET,
                ) {
                    Ok(stream) => {
                        self.prepared = Some(Prepared {
                            target: request.target,
                            expires: Instant::now() + PREPARED_LEASE,
                            stream,
                        });
                        // If this delivery fails, the loop exits and drops the standby.
                        reply(server, host, request, Outcome::Prepared)
                    }
                    Err(reason) => reply(server, host, request, Outcome::Refused(reason)),
                }
            }
            Action::Abort => {
                self.prepared.take();
                reply(server, host, request, Outcome::Aborted)
            }
            Action::Commit => {
                if !standby::is_idle(
                    &self
                        .prepared
                        .as_ref()
                        .expect("matching prepared ticket")
                        .stream,
                )
                .unwrap_or(false)
                {
                    self.prepared.take();
                    return reply(
                        server,
                        host,
                        request,
                        Outcome::Refused(Refusal::TargetUnavailable),
                    );
                }
                // No switch is possible until this exact old-wire record completes.
                let expires = self
                    .prepared
                    .as_ref()
                    .expect("matching prepared ticket")
                    .expires;
                let committed = WorkerHandoffResult::new(request, Outcome::Committed)?;
                let closed = server_transport::send_until(
                    server,
                    host,
                    &Message::WorkerHandoffResult(committed),
                    expires,
                )?;
                if closed && host.is_some() {
                    return Err(super::peer_closed().into());
                }
                let prepared = self.prepared.take().expect("matching prepared ticket");
                let old = std::mem::replace(server, prepared.stream);
                old.shutdown(Shutdown::Both)?;
                drop(old);
                let closed = server_transport::send(
                    server,
                    host,
                    &Message::WorkerHandoffOwned(request.peer()),
                )?;
                if closed && host.is_some() {
                    return Err(super::peer_closed().into());
                }
                Ok(())
            }
        }
    }
}

fn reply(
    server: &LocalStream,
    host: Option<&HostBrokerConnection>,
    request: WorkerHandoffRequest,
    outcome: Outcome,
) -> Result<(), Box<dyn Error>> {
    let result = WorkerHandoffResult::new(request, outcome)?;
    let closed = server_transport::send(server, host, &Message::WorkerHandoffResult(result))?;
    if closed && host.is_some() {
        return Err(super::peer_closed().into());
    }
    Ok(())
}
