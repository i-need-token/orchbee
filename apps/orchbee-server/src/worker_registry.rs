//! Owns Worker admission generations, the registered carrier of the local Worker and of each remote
//! node, the lookup of a Worker's carrier by its id, and exact-generation retirement with Agent
//! Drain identity minting. It does not authenticate transport peers or let a stale registration
//! retire a replacement Worker.

use std::{
    collections::BTreeMap,
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
use orchbee_machine_protocol::MachineId;
use orchbee_workbench_protocol::PresentationLaunchProgram;

use crate::agent::native_observation::CarrierIdentity;
use crate::agent_service::AgentService;
#[cfg(test)]
use crate::worker_obcn_carrier::WorkerWireExit;
use crate::worker_obcn_carrier::{
    WorkerCarrier, WorkerCarrierShutdown, WorkerRequestHandle, WorkerWireEnd, WorkerWireOwner,
};
use crate::{
    agent_catalog::AgentCatalog, terminal_titles::TerminalTitles, viewer_ready::ViewerReadyRoutes,
};

#[derive(Debug)]
struct ActiveCarrier {
    _shutdown: WorkerCarrierShutdown,
    request_handle: WorkerRequestHandle,
    drain_mint: Arc<AgentDrainMint>,
    /// Whether the Session's jobs may use this carrier. A replacement Worker serves only once it
    /// has adopted its Host, so `AdoptHost` is the first request it receives.
    serving: bool,
}

#[derive(Debug)]
struct ActiveWorker {
    carrier: Option<ActiveCarrier>,
    generation: NonZeroU64,
    worker_id: WorkerId,
    /// The System shell this generation resolved and reported before admission, held until its
    /// carrier registers and then carried by that carrier's request handle.
    system_shell: Option<PresentationLaunchProgram>,
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
    /// Serves the wire until it ends; the generation retires before this returns.
    pub(crate) fn run_to_end(mut self, stream: orchbee_local_ipc::LocalStream) -> WorkerWireEnd {
        let wire_owner = self
            .wire_owner
            .take()
            .unwrap_or_else(|| unreachable!("registered Worker must own its wire"));
        wire_owner.run_to_end(stream)
    }

    #[cfg(test)]
    pub(crate) fn run(self, stream: orchbee_local_ipc::LocalStream) -> WorkerWireExit {
        self.run_to_end(stream).exit
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
            // retirement and slot release. A faulted retirement fold is
            // reported inside `native_retire_carrier`'s typed result, and its
            // rebuild forgets the carrier; it cannot block registry retirement.
            if let Some(leaf) = self.leaf.take()
                && let Some(agents) = leaf.agents.upgrade()
            {
                let _retired = agents.native_retire_carrier(leaf.identity, Instant::now());
            }
            // The generation check makes delayed guard destruction harmless to a replacement.
            let _retired = registry.retire_carrier(self.generation);
            registry.titles.retire(self.generation);
            registry.catalog.retire(self.generation);
        }
    }
}

#[derive(Debug)]
pub(crate) struct WorkerRegistry {
    /// The local Worker.
    active: Option<ActiveWorker>,
    /// The Worker each borrowed machine runs for the Session, at most one per machine. Their
    /// generations come from the local Worker's counter, so a generation names one Worker of the
    /// Session, and no two Workers share an id.
    nodes: BTreeMap<MachineId, ActiveWorker>,
    next_generation: Option<NonZeroU64>,
    viewer_ready: Arc<ViewerReadyRoutes>,
    titles: Arc<TerminalTitles>,
    catalog: Arc<AgentCatalog>,
    /// Whether a Server restart holds the Worker for its successor: no request or job reaches it,
    /// as while it is replaced, except the exact capture's observation.
    held: bool,
}

impl WorkerRegistry {
    pub(crate) fn new() -> Self {
        Self {
            active: None,
            nodes: BTreeMap::new(),
            next_generation: Some(NonZeroU64::MIN),
            viewer_ready: Arc::default(),
            titles: Arc::default(),
            catalog: Arc::default(),
            held: false,
        }
    }

    /// Holds the serving Worker for a restart's successor, or lifts that hold.
    pub(crate) fn hold(&mut self, held: bool) {
        self.held = held;
    }

    pub(crate) fn viewer_ready_routes(&self) -> Arc<ViewerReadyRoutes> {
        Arc::clone(&self.viewer_ready)
    }

    /// The Session's published terminal titles, which follow the registered generation.
    pub(crate) fn terminal_titles(&self) -> Arc<TerminalTitles> {
        Arc::clone(&self.titles)
    }

    /// The Session's Agent catalog, which looks up each registered generation.
    pub(crate) fn agent_catalog(&self) -> Arc<AgentCatalog> {
        Arc::clone(&self.catalog)
    }

    /// Admits the local Worker.
    pub(crate) fn admit(
        &mut self,
        worker_id: WorkerId,
        system_shell: Option<PresentationLaunchProgram>,
    ) -> WorkerAdmission {
        if let Some(active) = self.active.as_ref() {
            return duplicate(active);
        }
        if let Some(node) = self.nodes.values().find(|node| node.worker_id == worker_id) {
            return duplicate(node);
        }
        let Some(worker) = self.next_worker(worker_id, system_shell) else {
            return WorkerAdmission::GenerationExhausted;
        };
        let generation = worker.generation;
        self.active = Some(worker);
        WorkerAdmission::Accepted { generation }
    }

    /// Admits the Worker of `machine`'s remote node, one per machine. Its id names it among the
    /// Session's Workers, so an id another Worker holds is a duplicate.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "a remote node joins only through the machine link, which carries no Worker yet"
        )
    )]
    pub(crate) fn admit_node(
        &mut self,
        machine: MachineId,
        worker_id: WorkerId,
        system_shell: Option<PresentationLaunchProgram>,
    ) -> WorkerAdmission {
        if let Some(node) = self.nodes.get(&machine) {
            return duplicate(node);
        }
        if let Some(other) = self.workers().find(|other| other.worker_id == worker_id) {
            return duplicate(other);
        }
        let Some(worker) = self.next_worker(worker_id, system_shell) else {
            return WorkerAdmission::GenerationExhausted;
        };
        let generation = worker.generation;
        self.nodes.insert(machine, worker);
        WorkerAdmission::Accepted { generation }
    }

    fn next_worker(
        &mut self,
        worker_id: WorkerId,
        system_shell: Option<PresentationLaunchProgram>,
    ) -> Option<ActiveWorker> {
        let generation = self.next_generation?;
        self.next_generation = generation.get().checked_add(1).and_then(NonZeroU64::new);
        Some(ActiveWorker {
            carrier: None,
            generation,
            worker_id,
            system_shell,
        })
    }

    /// Every admitted Worker: the local one first, then each remote node in machine order.
    fn workers(&self) -> impl Iterator<Item = &ActiveWorker> {
        self.active.iter().chain(self.nodes.values())
    }

    fn worker_mut(&mut self, generation: NonZeroU64) -> Option<&mut ActiveWorker> {
        match self.active.as_mut() {
            Some(active) if active.generation == generation => Some(active),
            _ => self
                .nodes
                .values_mut()
                .find(|node| node.generation == generation),
        }
    }

    pub(crate) fn register_carrier(
        registry: &Arc<Mutex<Self>>,
        generation: NonZeroU64,
        carrier: WorkerCarrier,
    ) -> Option<RegisteredWorkerWireOwner> {
        Self::register_carrier_impl(registry, generation, carrier, None, true)
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
        Self::register_carrier_impl(registry, generation, carrier, Some(agents), true)
    }

    /// [`Self::register_carrier_with_agents`] for a replacement Worker, whose carrier serves no
    /// job until [`Self::serve_adopted`]: until then only [`Self::adopting_handle`] reaches it.
    pub(crate) fn register_replacement_with_agents(
        registry: &Arc<Mutex<Self>>,
        generation: NonZeroU64,
        carrier: WorkerCarrier,
        agents: &Arc<AgentService>,
    ) -> Option<RegisteredWorkerWireOwner> {
        Self::register_carrier_impl(registry, generation, carrier, Some(agents), false)
    }

    /// Registers the carrier of a remote node's Worker. A replacement serves no job until
    /// [`Self::serve_adopted`]. Titles, the Agent catalog and the Agent leaf follow the local
    /// Worker only.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "a remote node joins only through the machine link, which carries no Worker yet"
        )
    )]
    pub(crate) fn register_node_carrier(
        registry: &Arc<Mutex<Self>>,
        generation: NonZeroU64,
        carrier: WorkerCarrier,
        serving: bool,
    ) -> Option<RegisteredWorkerWireOwner> {
        Self::register_carrier_impl(registry, generation, carrier, None, serving)
    }

    fn register_carrier_impl(
        registry: &Arc<Mutex<Self>>,
        generation: NonZeroU64,
        carrier: WorkerCarrier,
        agents: Option<&Arc<AgentService>>,
        serving: bool,
    ) -> Option<RegisteredWorkerWireOwner> {
        let mut registry_guard = registry.lock().ok()?;
        let viewer_ready = registry_guard.viewer_ready_routes();
        let titles = registry_guard.terminal_titles();
        let catalog = registry_guard.agent_catalog();
        let local = registry_guard
            .active
            .as_ref()
            .is_some_and(|active| active.generation == generation);
        // The Agent leaf admits one carrier, the local Worker's.
        if agents.is_some() && !local {
            return None;
        }
        let active = registry_guard.worker_mut(generation)?;
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
        // A Worker the title wire cannot name keeps no titles; registration is unaffected.
        let title_worker = CapabilityWorkerId::from_bytes(active.worker_id.as_bytes())
            .ok()
            .filter(|_| local);
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
                    // Contract refusal (carrier already active) or a fault
                    // in the admission fold, whose rebuild keeps no carrier,
                    // fail the registration closed: no Live record is
                    // published without its leaf.
                    Ok(Err(_)) | Err(_) => return None,
                }
            }
            None => None,
        };
        let (request_handle, shutdown, wire_owner) = carrier.into_parts();
        let wire_owner = wire_owner.with_viewer_ready(viewer_ready);
        let wire_owner = if local {
            wire_owner.with_titles(Arc::clone(&titles), generation)
        } else {
            wire_owner
        };
        // The carrier matched this admitted generation above, so its handle carries this
        // generation's reported shell and no other.
        let request_handle = request_handle.with_system_shell(active.system_shell.take());
        active.carrier = Some(ActiveCarrier {
            _shutdown: shutdown,
            request_handle,
            drain_mint,
            serving,
        });
        drop(registry_guard);
        // Before the wire owner runs, so no note of this generation precedes it.
        if let Some(worker) = title_worker {
            titles.begin(worker, generation);
            catalog.begin(worker, generation);
        }
        Some(RegisteredWorkerWireOwner {
            registration: Some(WorkerGenerationRegistration {
                generation,
                registry: Arc::downgrade(registry),
                leaf,
            }),
            wire_owner: Some(wire_owner),
        })
    }

    /// The serving local Worker's live carrier; none while a restart holds it.
    pub(crate) fn request_handle(&self) -> Option<WorkerRequestHandle> {
        if self.held {
            return None;
        }
        self.capture_handle()
    }

    /// The live carrier of the serving Worker `worker_id` names, local or remote; none while a
    /// restart holds the Session's Workers.
    pub(crate) fn handle_for_worker(
        &self,
        worker_id: CapabilityWorkerId,
    ) -> Option<WorkerRequestHandle> {
        if self.held {
            return None;
        }
        self.workers()
            .find(|worker| worker.worker_id.as_bytes() == worker_id.as_bytes())
            .and_then(serving_handle)
    }

    /// The serving carriers of all the Session's Workers, the local Worker's first; none while a
    /// restart holds the Session's Workers.
    pub(crate) fn serving_handles(&self) -> Vec<WorkerRequestHandle> {
        if self.held {
            return Vec::new();
        }
        self.workers().filter_map(serving_handle).collect()
    }

    /// The serving carrier of the Worker `handle` addresses, whichever generation serves it now.
    pub(crate) fn current_for(&self, handle: &WorkerRequestHandle) -> Option<WorkerRequestHandle> {
        if self.held {
            return None;
        }
        self.workers()
            .find(|worker| worker.worker_id == handle.worker_id())
            .and_then(serving_handle)
    }

    /// Whether `handle` is the live carrier its Worker serves through now.
    pub(crate) fn is_current(&self, handle: &WorkerRequestHandle) -> bool {
        handle.is_active()
            && self
                .current_for(handle)
                .is_some_and(|current| current.generation() == handle.generation())
    }

    /// [`Self::handle_for_worker`] for exactly that Worker's `generation`.
    pub(crate) fn handle_at(
        &self,
        worker_id: CapabilityWorkerId,
        generation: WorkerGeneration,
    ) -> Option<WorkerRequestHandle> {
        self.handle_for_worker(worker_id)
            .filter(|handle| handle.generation().get() == generation.get())
    }

    /// Whether the Worker `worker_id` names serves now through exactly `generation`.
    pub(crate) fn serves(
        &self,
        worker_id: CapabilityWorkerId,
        generation: WorkerGeneration,
    ) -> bool {
        self.handle_at(worker_id, generation).is_some()
    }

    /// The serving Worker's live carrier, also while a restart holds it: only the exact capture and
    /// the observation it and the background-program count make use it then, and they change
    /// nothing in the Session.
    pub(crate) fn capture_handle(&self) -> Option<WorkerRequestHandle> {
        self.active.as_ref().and_then(serving_handle)
    }

    /// The live carrier of a replacement Worker that has not adopted its Host yet.
    pub(crate) fn adopting_handle(&self) -> Option<WorkerRequestHandle> {
        self.active
            .as_ref()
            .and_then(|active| active.carrier.as_ref())
            .filter(|carrier| !carrier.serving)
            .map(|carrier| &carrier.request_handle)
            .filter(|request_handle| request_handle.is_active())
            .cloned()
    }

    /// Lets the Session's jobs use the replacement carrier of `generation` once it adopted its
    /// Host. Returns whether that carrier is still the registered one; a carrier that ended since
    /// still serves no job, since [`Self::request_handle`] skips an ended carrier.
    pub(crate) fn serve_adopted(&mut self, generation: NonZeroU64) -> bool {
        match self.worker_mut(generation) {
            Some(ActiveWorker {
                carrier: Some(carrier),
                ..
            }) => {
                carrier.serving = true;
                true
            }
            _ => false,
        }
    }

    pub(crate) fn cancel_pending_admission(&mut self, generation: NonZeroU64) -> bool {
        self.remove_worker(generation, false)
    }

    fn retire_carrier(&mut self, generation: NonZeroU64) -> bool {
        self.remove_worker(generation, true)
    }

    /// Removes the Worker of `generation` when its carrier is registered exactly as `registered`
    /// says.
    fn remove_worker(&mut self, generation: NonZeroU64, registered: bool) -> bool {
        let matches = |worker: &ActiveWorker| {
            worker.generation == generation && worker.carrier.is_some() == registered
        };
        if self.active.as_ref().is_some_and(matches) {
            self.active = None;
            return true;
        }
        let machine = self
            .nodes
            .iter()
            .find(|(_, node)| matches(node))
            .map(|(machine, _)| machine.clone());
        machine.is_some_and(|machine| self.nodes.remove(&machine).is_some())
    }
}

fn duplicate(worker: &ActiveWorker) -> WorkerAdmission {
    WorkerAdmission::Duplicate {
        active_generation: worker.generation,
        active_worker_id: worker.worker_id,
    }
}

/// The live carrier of `worker` once it serves the Session's jobs.
fn serving_handle(worker: &ActiveWorker) -> Option<WorkerRequestHandle> {
    worker
        .carrier
        .as_ref()
        .filter(|carrier| carrier.serving)
        .map(|carrier| &carrier.request_handle)
        .filter(|request_handle| request_handle.is_active())
        .cloned()
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
        if !carrier.serving || self.held || !carrier.request_handle.is_active() {
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
