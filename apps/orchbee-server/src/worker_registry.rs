//! Owns Worker admission generations, the active registered carrier, and exact-generation
//! retirement with Agent Drain identity minting. It does not authenticate transport peers or
//! let a stale registration retire a replacement Worker.

use std::{
    num::NonZeroU64,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

#[cfg(test)]
use std::io::{Read, Write};

use orchbee_capability_node_protocol::{WorkerGeneration, WorkerId as CapabilityWorkerId};
use orchbee_local_worker_protocol::WorkerId;

use crate::agent::native_observation::CarrierIdentity;
use crate::agent_service::AgentService;
use crate::worker_obcn_carrier::{
    WorkerCarrier, WorkerCarrierShutdown, WorkerRequestHandle, WorkerWireExit, WorkerWireOwner,
};

#[derive(Debug)]
struct ActiveCarrier {
    _shutdown: WorkerCarrierShutdown,
    request_handle: WorkerRequestHandle,
    drain_mint: Arc<AgentDrainMint>,
}

#[derive(Debug)]
struct ActiveWorker {
    carrier: Option<ActiveCarrier>,
    generation: NonZeroU64,
    worker_id: WorkerId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkerAdmission {
    Accepted {
        generation: NonZeroU64,
    },
    Duplicate {
        active_generation: NonZeroU64,
        active_worker_id: WorkerId,
    },
    GenerationExhausted,
}

pub(crate) struct RegisteredWorkerWireOwner {
    registration: Option<WorkerGenerationRegistration>,
    wire_owner: Option<WorkerWireOwner>,
}

impl RegisteredWorkerWireOwner {
    pub(crate) fn run(mut self, stream: orchbee_local_ipc::LocalStream) -> WorkerWireExit {
        let wire_owner = self
            .wire_owner
            .take()
            .unwrap_or_else(|| unreachable!("registered Worker must own its wire"));
        wire_owner.run(stream)
    }

    #[cfg(test)]
    pub(crate) fn run_configured_for_test<Stream>(mut self, stream: Stream) -> WorkerWireExit
    where
        Stream: orchbee_local_ipc::AsLocalIo + Read + Write,
    {
        let wire_owner = self
            .wire_owner
            .take()
            .unwrap_or_else(|| unreachable!("registered Worker must own its wire"));
        wire_owner.run_configured_for_test(stream)
    }
}

impl Drop for RegisteredWorkerWireOwner {
    fn drop(&mut self) {
        // Wire settlement must precede registry retirement so a replacement can never overlap an
        // old write or result publication, including during unwinding and thread-spawn failure.
        drop(self.wire_owner.take());
        drop(self.registration.take());
    }
}

/// The leaf attachment for the canonical AgentService native seam. The
/// registration carries the exact CNP carrier identity it admitted plus a
/// weak handle to the service so carrier teardown can retire the leaf inside
/// the same registry guard. A dead service cannot observe a stale leaf, so
/// the weak upgrade simply fails and the slot still retires.
struct LeafBinding {
    identity: CarrierIdentity,
    agents: Weak<AgentService>,
}

struct WorkerGenerationRegistration {
    generation: NonZeroU64,
    registry: Weak<Mutex<WorkerRegistry>>,
    leaf: Option<LeafBinding>,
}

impl Drop for WorkerGenerationRegistration {
    fn drop(&mut self) {
        let Some(registry) = self.registry.upgrade() else {
            return;
        };
        if let Ok(mut registry) = registry.lock() {
            // The exact leaf identity retires while the registry guard is
            // still held — a replacement can never be exposed between leaf
            // retirement and slot release. A poisoned AgentService cell is
            // reported inside `native_retire_carrier`'s typed result; it
            // cannot block registry retirement.
            if let Some(leaf) = self.leaf.take()
                && let Some(agents) = leaf.agents.upgrade()
            {
                let _retired = agents.native_retire_carrier(leaf.identity, Instant::now());
            }
            // The generation check makes delayed guard destruction harmless to a replacement.
            let _retired = registry.retire_carrier(self.generation);
        }
    }
}

#[derive(Debug)]
pub(crate) struct WorkerRegistry {
    active: Option<ActiveWorker>,
    next_generation: Option<NonZeroU64>,
}

impl WorkerRegistry {
    pub(crate) const fn new() -> Self {
        Self {
            active: None,
            next_generation: Some(NonZeroU64::MIN),
        }
    }

    pub(crate) fn admit(&mut self, worker_id: WorkerId) -> WorkerAdmission {
        if let Some(active) = self.active.as_ref() {
            return WorkerAdmission::Duplicate {
                active_generation: active.generation,
                active_worker_id: active.worker_id,
            };
        }
        let Some(generation) = self.next_generation else {
            return WorkerAdmission::GenerationExhausted;
        };
        self.active = Some(ActiveWorker {
            carrier: None,
            generation,
            worker_id,
        });
        self.next_generation = generation.get().checked_add(1).and_then(NonZeroU64::new);
        WorkerAdmission::Accepted { generation }
    }

    pub(crate) fn register_carrier(
        registry: &Arc<Mutex<Self>>,
        generation: NonZeroU64,
        carrier: WorkerCarrier,
    ) -> Option<RegisteredWorkerWireOwner> {
        Self::register_carrier_impl(registry, generation, carrier, None)
    }

    /// The managed consumer of the same registration path: identical checks and
    /// publication, plus the canonical AgentService native leaf admitted and
    /// carried by the registration. The leaf admits inside this registry
    /// guard before the carrier record is installed, so `request_handle` and
    /// `agent_drain_carrier` can never expose a Live carrier whose leaf was
    /// refused — and the leaf retires inside the registration guard before
    /// the slot frees. The service is held weakly; lifecycle authority stays
    /// at the session composition root for later wiring.
    pub(crate) fn register_carrier_with_agents(
        registry: &Arc<Mutex<Self>>,
        generation: NonZeroU64,
        carrier: WorkerCarrier,
        agents: &Arc<AgentService>,
    ) -> Option<RegisteredWorkerWireOwner> {
        Self::register_carrier_impl(registry, generation, carrier, Some(agents))
    }

    fn register_carrier_impl(
        registry: &Arc<Mutex<Self>>,
        generation: NonZeroU64,
        carrier: WorkerCarrier,
        agents: Option<&Arc<AgentService>>,
    ) -> Option<RegisteredWorkerWireOwner> {
        let mut registry_guard = registry.lock().ok()?;
        let active = registry_guard.active.as_mut()?;
        if active.generation != generation
            || carrier.generation() != generation
            || carrier.worker_id() != active.worker_id
            || active.carrier.is_some()
            || !carrier.is_active()
        {
            return None;
        }
        // Allocating pieces are constructed before the leaf fold so nothing
        // between the fold and the record install below can panic.
        let drain_mint = Arc::new(AgentDrainMint::new());
        let leaf = match agents {
            Some(agents) => {
                // The identity is derived under the guard from the SAME
                // admitted worker and generation — never an independent
                // caller value, and the same pair agent_drain_carrier()
                // re-derives.
                let worker_id = CapabilityWorkerId::from_bytes(active.worker_id.as_bytes()).ok()?;
                let identity = CarrierIdentity {
                    worker_id,
                    generation: WorkerGeneration::new(generation),
                };
                let weak_agents = Arc::downgrade(agents);
                match agents.native_admit_carrier(identity, Instant::now()) {
                    Ok(Ok(())) => Some(LeafBinding {
                        identity,
                        agents: weak_agents,
                    }),
                    // Contract refusal (carrier already active) or fold
                    // unavailability (poisoned cell) fail the registration
                    // closed: no Live record is published without its leaf.
                    Ok(Err(_)) | Err(_) => return None,
                }
            }
            None => None,
        };
        let (request_handle, shutdown, wire_owner) = carrier.into_parts();
        active.carrier = Some(ActiveCarrier {
            _shutdown: shutdown,
            request_handle,
            drain_mint,
        });
        drop(registry_guard);
        Some(RegisteredWorkerWireOwner {
            registration: Some(WorkerGenerationRegistration {
                generation,
                registry: Arc::downgrade(registry),
                leaf,
            }),
            wire_owner: Some(wire_owner),
        })
    }

    pub(crate) fn request_handle(&self) -> Option<WorkerRequestHandle> {
        self.active
            .as_ref()
            .and_then(|active| active.carrier.as_ref())
            .map(|carrier| &carrier.request_handle)
            .filter(|request_handle| request_handle.is_active())
            .cloned()
    }

    pub(crate) fn cancel_pending_admission(&mut self, generation: NonZeroU64) -> bool {
        if self
            .active
            .as_ref()
            .is_none_or(|active| active.generation != generation || active.carrier.is_some())
        {
            return false;
        }
        self.active = None;
        true
    }

    fn retire_carrier(&mut self, generation: NonZeroU64) -> bool {
        if self
            .active
            .as_ref()
            .is_none_or(|active| active.generation != generation || active.carrier.is_none())
        {
            return false;
        }
        self.active = None;
        true
    }
}

/// Per-carrier AgentDrain token mint. Drain tokens are carrier-scoped
/// correlation values: the mint is born at successful carrier registration
/// and dies with the record. Exhaustion is a value answer, never a lifecycle
/// event — it never retires the carrier and never reuses a token.
#[derive(Debug)]
pub(crate) struct AgentDrainMint {
    last_issued: AtomicU64,
}

impl AgentDrainMint {
    const fn new() -> Self {
        Self {
            last_issued: AtomicU64::new(0),
        }
    }

    /// Next token this mint would issue, without issuing it. `None` at
    /// exhaustion (`last_issued == u64::MAX`); never mutates.
    pub(crate) fn floor(&self) -> Option<u64> {
        self.last_issued.load(Ordering::Acquire).checked_add(1)
    }

    /// Issue the next drain token. At `u64::MAX` the checked CAS fails and
    /// `last_issued` is left untouched — no mutation, no retirement, no reuse.
    pub(crate) fn mint(&self) -> Option<NonZeroU64> {
        self.last_issued
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_add(1)
            })
            .ok()
            .and_then(|previous| NonZeroU64::new(previous + 1))
    }
}

/// Typed answer to "is there a Worker carrier worth draining right now".
/// `active == None` is the only true-none; a pending admission, a dead
/// carrier record, or a carrier that cannot be named on the drain wire are
/// all `CarrierUnavailable`, never `NoCarrier`.
#[derive(Debug)]
pub(crate) enum AgentDrainCarrier {
    NoCarrier,
    CarrierUnavailable,
    Live(AgentDrainTarget),
}

/// One live drain target: the cloned request handle, the carrier's full
/// wire identity as a CNP WorkerId + WorkerGeneration pair, and the shared
/// per-carrier token mint. A cloned target may outlive retirement only with
/// a dead identity — the mint is not a liveness proof; the handle's
/// admission fence is.
#[derive(Debug)]
pub(crate) struct AgentDrainTarget {
    pub(crate) request: WorkerRequestHandle,
    pub(crate) worker_id: CapabilityWorkerId,
    pub(crate) worker_generation: WorkerGeneration,
    pub(crate) mint: Arc<AgentDrainMint>,
}

impl WorkerRegistry {
    /// Drain-scoped view of the active carrier, separating the states
    /// `request_handle()` folds into one `None`.
    pub(crate) fn agent_drain_carrier(&self) -> AgentDrainCarrier {
        let Some(active) = self.active.as_ref() else {
            return AgentDrainCarrier::NoCarrier;
        };
        let Some(carrier) = active.carrier.as_ref() else {
            return AgentDrainCarrier::CarrierUnavailable;
        };
        if !carrier.request_handle.is_active() {
            return AgentDrainCarrier::CarrierUnavailable;
        }
        let Ok(worker_id) = CapabilityWorkerId::from_bytes(active.worker_id.as_bytes()) else {
            return AgentDrainCarrier::CarrierUnavailable;
        };
        AgentDrainCarrier::Live(AgentDrainTarget {
            request: carrier.request_handle.clone(),
            worker_id,
            worker_generation: WorkerGeneration::new(active.generation),
            mint: Arc::clone(&carrier.drain_mint),
        })
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use tests::generation_is_retired;
