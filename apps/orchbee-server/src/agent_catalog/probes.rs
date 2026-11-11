//! The probe pump: one thread that exchanges the Agent probe Start, Poll and Cancel the catalog
//! asks for with the registered Worker. Each exchange is admitted to the carrier's low lane under a
//! fresh operation ID; the pump keeps at most one outstanding and stops waiting for it when the
//! lookup ends, its deadline passes or the Session stops, leaving the wire owner to consume a late
//! reply.
use super::{AgentCatalog, Step, Ticket};
use crate::{
    worker_obcn_carrier::{ExchangeWait, WorkerOperationResult},
    worker_registry::WorkerRegistry,
};
use orchbee_capability_node_protocol::{AgentProbeJobRequest, AgentProbeJobResult, OperationId};
use std::{
    io,
    sync::{Arc, Mutex, Weak},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// How often a pump waiting on an exchange checks whether the lookup still wants it.
const STOP_POLL: Duration = Duration::from_millis(100);

impl AgentCatalog {
    /// Starts the probe pump. Only Session composition starts it, once.
    pub(crate) fn start_probes(
        self: &Arc<Self>,
        workers: Weak<Mutex<WorkerRegistry>>,
    ) -> io::Result<()> {
        let mut slot = self
            .prober
            .lock()
            .map_err(|_| io::Error::other("Agent probe pump poisoned"))?;
        if slot.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Agent probe pump already started",
            ));
        }
        let catalog = Arc::clone(self);
        *slot = Some(
            thread::Builder::new()
                .name("orchbee-agent-catalog".into())
                .spawn(move || catalog.run(&workers))?,
        );
        Ok(())
    }

    /// Stops the probe pump and joins it within the Session owner's deadline. A Worker job still
    /// running is left to the Worker, which ends it at its own deadline or its exit.
    #[cfg(windows)]
    pub(crate) fn stop_probes_before(&self, deadline: Instant) -> io::Result<()> {
        self.stop();
        loop {
            {
                let mut slot = self
                    .prober
                    .lock()
                    .map_err(|_| io::Error::other("Agent probe pump poisoned"))?;
                if slot.as_ref().is_none_or(thread::JoinHandle::is_finished) {
                    if let Some(prober) = slot.take() {
                        prober
                            .join()
                            .map_err(|_| io::Error::other("Agent probe pump panicked"))?;
                    }
                    return Ok(());
                }
            }
            if Instant::now() >= deadline {
                return Err(io::ErrorKind::TimedOut.into());
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn run(&self, workers: &Weak<Mutex<WorkerRegistry>>) {
        loop {
            match self.next(Instant::now()) {
                Step::Stopped => return,
                Step::Wait(deadline) => self.wait(deadline),
                Step::Exchange {
                    ticket,
                    worker,
                    action,
                } => {
                    let result = operation_id().and_then(|operation_id| {
                        self.exchange(
                            workers,
                            ticket,
                            AgentProbeJobRequest {
                                operation_id,
                                worker_id: worker,
                                worker_generation: ticket.generation,
                                action,
                            },
                        )
                    });
                    self.apply(ticket, result, Instant::now(), unix_ms());
                }
            }
        }
    }

    /// `None` when the exchange could not be admitted or answered while the lookup wanted it.
    fn exchange(
        &self,
        workers: &Weak<Mutex<WorkerRegistry>>,
        ticket: Ticket,
        request: AgentProbeJobRequest,
    ) -> Option<AgentProbeJobResult> {
        let handle = {
            let workers = workers.upgrade()?;
            let workers = workers.lock().ok()?;
            workers.request_handle()?
        };
        if handle.generation().get() != ticket.generation.get() {
            return None;
        }
        let pending = handle.admit_agent_probe(request).ok()?;
        drop(handle);
        loop {
            match pending.recv_deadline(Instant::now() + STOP_POLL) {
                ExchangeWait::Ready(result) => {
                    return match *result {
                        Ok(WorkerOperationResult::AgentProbeJob(result)) => Some(result),
                        Ok(_) | Err(_) => None,
                    };
                }
                ExchangeWait::Lost(_) => return None,
                ExchangeWait::Elapsed if !self.awaits(ticket, Instant::now()) => return None,
                ExchangeWait::Elapsed => {}
            }
        }
    }
}

/// A fresh random operation identity; the Worker pairs nothing else with it.
fn operation_id() -> Option<OperationId> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).ok()?;
    OperationId::from_bytes(bytes).ok()
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
        })
}
