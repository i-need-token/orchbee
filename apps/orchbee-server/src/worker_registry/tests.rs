use orchbee_local_ipc::LocalStream as UnixStream;
use std::{
    num::NonZeroU64,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use orchbee_agent_observation_protocol::{
    AgentListOutcome, AgentReadRequest, AgentReadResult, AgentSafeInteger, AgentUnavailableReason,
    ObapRequestId,
};
use orchbee_capability_node_protocol::{
    AgentDrainReply, AgentDrainRequest, CapabilityNodeMessage, read_message, write_message,
};
use orchbee_local_worker_protocol::WorkerId;
use orchbee_workbench_protocol::PresentationLaunchProgram;

use crate::agent::native_observation::CarrierIdentity;
use crate::agent_service::{AgentPolicy, AgentService, DrainExpectation};
use crate::worker_obcn_carrier::{
    ExchangeWait, WorkerOperationResult, WorkerRequestError, WorkerWireExit, create_worker_carrier,
};

use super::{
    AgentDrainCarrier, AgentDrainMint, AgentDrainTarget, LeafBinding, WorkerAdmission,
    WorkerGenerationRegistration, WorkerRegistry,
};

const FIRST_WORKER: WorkerId = WorkerId::from_bytes([1; 16]);
const SECOND_WORKER: WorkerId = WorkerId::from_bytes([2; 16]);
const UNNAMEABLE_WORKER: WorkerId = WorkerId::from_bytes([0; 16]);
const DRAIN_TEST_TIMEOUT: Duration = Duration::from_secs(2);

pub(crate) fn generation_is_retired(registry: &WorkerRegistry, generation: NonZeroU64) -> bool {
    registry
        .active
        .as_ref()
        .is_none_or(|active| active.generation != generation)
}

#[test]
fn inactive_carrier_blocks_replacement_until_registration_retires() {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let generation = {
        let mut registry = registry.lock().unwrap_or_else(|error| error.into_inner());
        let WorkerAdmission::Accepted { generation } = registry.admit(FIRST_WORKER, None) else {
            panic!("first Worker was not admitted");
        };
        generation
    };
    let carrier = create_worker_carrier(FIRST_WORKER, generation);
    let observer = carrier.request_handle();
    let owner = WorkerRegistry::register_carrier(&registry, generation, carrier)
        .unwrap_or_else(|| panic!("first carrier was not registered"));

    // Holding the registry gate prevents the registration guard from retiring the generation
    // after the wire owner marks its handle inactive. No timing guess selects this window.
    let mut registry_guard = registry.lock().unwrap_or_else(|error| error.into_inner());
    let dropping_owner = thread::spawn(move || drop(owner));
    let deadline = Instant::now() + Duration::from_secs(2);
    while observer.is_active() && Instant::now() < deadline {
        thread::yield_now();
    }
    let became_inactive = !observer.is_active();
    let handle_unavailable = registry_guard.request_handle().is_none();
    let prematurely_retired = generation_is_retired(&registry_guard, generation);
    let premature_admission = registry_guard.admit(SECOND_WORKER, None);
    drop(registry_guard);
    dropping_owner
        .join()
        .unwrap_or_else(|_panic| panic!("carrier owner teardown panicked"));

    assert!(became_inactive, "carrier owner did not begin teardown");
    assert!(handle_unavailable);
    assert!(
        !prematurely_retired,
        "an inactive handle does not prove generation retirement"
    );
    assert_eq!(
        premature_admission,
        WorkerAdmission::Duplicate {
            active_generation: generation,
            active_worker_id: FIRST_WORKER,
        }
    );

    let mut registry_guard = registry.lock().unwrap_or_else(|error| error.into_inner());
    assert!(generation_is_retired(&registry_guard, generation));
    let WorkerAdmission::Accepted {
        generation: replacement_generation,
    } = registry_guard.admit(SECOND_WORKER, None)
    else {
        panic!("replacement Worker was not admitted after retirement");
    };
    assert!(replacement_generation > generation);
    assert!(generation_is_retired(&registry_guard, generation));
    assert!(!generation_is_retired(
        &registry_guard,
        replacement_generation
    ));
}

#[test]
fn admits_only_one_worker_and_assigns_increasing_generations() {
    let mut registry = WorkerRegistry::new();
    let WorkerAdmission::Accepted {
        generation: first_generation,
    } = registry.admit(FIRST_WORKER, None)
    else {
        panic!("first Worker was not admitted");
    };
    assert_eq!(first_generation, NonZeroU64::MIN);
    assert_eq!(
        registry.admit(SECOND_WORKER, None),
        WorkerAdmission::Duplicate {
            active_generation: first_generation,
            active_worker_id: FIRST_WORKER,
        }
    );
    assert!(registry.cancel_pending_admission(first_generation));

    let WorkerAdmission::Accepted {
        generation: second_generation,
    } = registry.admit(SECOND_WORKER, None)
    else {
        panic!("replacement Worker was not admitted");
    };
    assert!(second_generation > first_generation);
}

#[test]
fn each_generation_handle_carries_only_the_shell_that_generation_reported() {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let shell = |name| PresentationLaunchProgram::parse(name).ok();
    let admit_and_register = |worker_id, system_shell| {
        let generation = {
            let mut registry = registry.lock().unwrap_or_else(|error| error.into_inner());
            let WorkerAdmission::Accepted { generation } = registry.admit(worker_id, system_shell)
            else {
                panic!("Worker was not admitted");
            };
            generation
        };
        let carrier = create_worker_carrier(worker_id, generation);
        WorkerRegistry::register_carrier(&registry, generation, carrier)
            .unwrap_or_else(|| panic!("carrier was not registered"))
    };
    let current_handle = || {
        registry
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .request_handle()
            .unwrap_or_else(|| panic!("registered carrier has no handle"))
    };

    let first_owner = admit_and_register(FIRST_WORKER, shell("pwsh.exe"));
    let first = current_handle();
    assert_eq!(first.system_shell(), shell("pwsh.exe").as_ref());

    drop(first_owner);
    assert!(
        registry
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .request_handle()
            .is_none()
    );

    let second_owner = admit_and_register(SECOND_WORKER, shell("cmd.exe"));
    let second = current_handle();
    assert!(second.generation() > first.generation());
    assert_eq!(second.system_shell(), shell("cmd.exe").as_ref());
    // A handle read before the retirement keeps naming its own generation's shell.
    assert_eq!(first.system_shell(), shell("pwsh.exe").as_ref());

    // A generation that reported no shell inherits none from its predecessor.
    drop(second_owner);
    let _third_owner = admit_and_register(FIRST_WORKER, None);
    assert_eq!(current_handle().system_shell(), None);
}

#[test]
fn stale_registration_cleanup_cannot_remove_a_replacement() {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let first_generation = {
        let mut registry = registry.lock().unwrap_or_else(|error| error.into_inner());
        let WorkerAdmission::Accepted { generation } = registry.admit(FIRST_WORKER, None) else {
            panic!("first Worker was not admitted");
        };
        assert!(registry.cancel_pending_admission(generation));
        generation
    };
    let replacement_generation = {
        let mut registry = registry.lock().unwrap_or_else(|error| error.into_inner());
        let WorkerAdmission::Accepted { generation } = registry.admit(SECOND_WORKER, None) else {
            panic!("replacement Worker was not admitted");
        };
        generation
    };
    let replacement_carrier = create_worker_carrier(SECOND_WORKER, replacement_generation);
    let replacement_observer = replacement_carrier.request_handle();
    let replacement_owner =
        WorkerRegistry::register_carrier(&registry, replacement_generation, replacement_carrier)
            .unwrap_or_else(|| panic!("replacement carrier was not registered"));

    drop(WorkerGenerationRegistration {
        generation: first_generation,
        registry: Arc::downgrade(&registry),
        leaf: None,
    });

    let registry_guard = registry.lock().unwrap_or_else(|error| error.into_inner());
    assert!(replacement_observer.is_active());
    assert_eq!(
        registry_guard
            .request_handle()
            .map(|handle| handle.generation()),
        Some(replacement_generation)
    );
    drop(registry_guard);
    drop(replacement_owner);
}

#[test]
fn exposes_only_the_matching_active_generation_carrier() {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let generation = {
        let mut registry = registry.lock().unwrap_or_else(|error| error.into_inner());
        let WorkerAdmission::Accepted { generation } = registry.admit(FIRST_WORKER, None) else {
            panic!("Worker was not admitted");
        };
        assert!(registry.request_handle().is_none());
        generation
    };

    let wrong_carrier = create_worker_carrier(SECOND_WORKER, generation);
    let wrong_observer = wrong_carrier.request_handle();
    assert!(WorkerRegistry::register_carrier(&registry, generation, wrong_carrier).is_none());
    assert!(!wrong_observer.is_active());
    assert!(
        registry
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .request_handle()
            .is_none()
    );

    let carrier = create_worker_carrier(FIRST_WORKER, generation);
    let observer = carrier.request_handle();
    let registered_owner = WorkerRegistry::register_carrier(&registry, generation, carrier)
        .unwrap_or_else(|| panic!("matching carrier was not registered"));
    let exposed = registry
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .request_handle()
        .unwrap_or_else(|| panic!("active Worker carrier was not exposed"));
    assert_eq!(exposed.generation(), generation);
    assert_eq!(exposed.worker_id(), FIRST_WORKER);

    let duplicate_carrier = create_worker_carrier(FIRST_WORKER, generation);
    let duplicate_observer = duplicate_carrier.request_handle();
    assert!(WorkerRegistry::register_carrier(&registry, generation, duplicate_carrier).is_none());
    assert!(!duplicate_observer.is_active());
    assert!(observer.is_active());
    assert!(
        !registry
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .cancel_pending_admission(generation)
    );

    drop(registered_owner);

    assert!(!observer.is_active());
    assert!(!exposed.is_active());
    assert!(
        registry
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .request_handle()
            .is_none()
    );
}

#[test]
fn stale_carrier_registration_preserves_the_replacement() {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let first_generation = {
        let mut registry = registry.lock().unwrap_or_else(|error| error.into_inner());
        let WorkerAdmission::Accepted { generation } = registry.admit(FIRST_WORKER, None) else {
            panic!("first Worker was not admitted");
        };
        assert!(registry.cancel_pending_admission(generation));
        generation
    };
    let replacement_generation = {
        let mut registry = registry.lock().unwrap_or_else(|error| error.into_inner());
        let WorkerAdmission::Accepted { generation } = registry.admit(SECOND_WORKER, None) else {
            panic!("replacement Worker was not admitted");
        };
        generation
    };

    let stale_carrier = create_worker_carrier(FIRST_WORKER, first_generation);
    let stale_observer = stale_carrier.request_handle();
    assert!(WorkerRegistry::register_carrier(&registry, first_generation, stale_carrier).is_none());
    assert!(!stale_observer.is_active());

    let replacement_carrier = create_worker_carrier(SECOND_WORKER, replacement_generation);
    let replacement_observer = replacement_carrier.request_handle();
    let replacement_owner =
        WorkerRegistry::register_carrier(&registry, replacement_generation, replacement_carrier)
            .unwrap_or_else(|| panic!("replacement carrier was not registered"));

    assert!(replacement_observer.is_active());
    assert_eq!(
        registry
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .request_handle()
            .map(|handle| handle.generation()),
        Some(replacement_generation)
    );
    drop(replacement_owner);
}

#[test]
fn admits_the_last_nonzero_generation_before_exhaustion() {
    let maximum_generation = NonZeroU64::MAX;
    let mut registry = WorkerRegistry {
        active: None,
        next_generation: Some(maximum_generation),
        viewer_ready: Arc::default(),
    };

    assert_eq!(
        registry.admit(FIRST_WORKER, None),
        WorkerAdmission::Accepted {
            generation: maximum_generation,
        }
    );
    assert!(registry.cancel_pending_admission(maximum_generation));
    assert_eq!(
        registry.admit(SECOND_WORKER, None),
        WorkerAdmission::GenerationExhausted
    );
}

// ---- AgentDrain registry target: per-carrier mint + typed accessor ----

fn admit_first_worker(registry: &Arc<Mutex<WorkerRegistry>>, worker_id: WorkerId) -> NonZeroU64 {
    let mut registry = registry.lock().unwrap_or_else(|error| error.into_inner());
    let WorkerAdmission::Accepted { generation } = registry.admit(worker_id, None) else {
        panic!("Worker was not admitted");
    };
    generation
}

fn register_first_carrier(
    registry: &Arc<Mutex<WorkerRegistry>>,
    worker_id: WorkerId,
    generation: NonZeroU64,
) -> super::RegisteredWorkerWireOwner {
    let carrier = create_worker_carrier(worker_id, generation);
    WorkerRegistry::register_carrier(registry, generation, carrier)
        .unwrap_or_else(|| panic!("carrier was not registered"))
}

fn live_drain_target(registry: &Arc<Mutex<WorkerRegistry>>) -> AgentDrainTarget {
    match registry
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .agent_drain_carrier()
    {
        AgentDrainCarrier::Live(target) => target,
        other => panic!("expected a live drain target, got {other:?}"),
    }
}

fn drain_carrier_kind(registry: &Arc<Mutex<WorkerRegistry>>) -> AgentDrainCarrier {
    registry
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .agent_drain_carrier()
}

#[test]
fn drain_mint_orders_tokens_and_exhausts_without_mutation() {
    let mint = AgentDrainMint::new();
    assert_eq!(mint.floor(), Some(1));
    assert_eq!(mint.mint(), NonZeroU64::new(1));
    assert_eq!(mint.mint(), NonZeroU64::new(2));
    assert_eq!(mint.floor(), Some(3));

    let mint = AgentDrainMint {
        last_issued: AtomicU64::new(u64::MAX - 1),
    };
    assert_eq!(mint.floor(), Some(u64::MAX));
    assert_eq!(mint.mint(), NonZeroU64::new(u64::MAX));
    // Exhaustion is a value answer: the checked CAS leaves last_issued at
    // u64::MAX — no mutation, no reuse, and the mint stays callable.
    assert_eq!(mint.floor(), None);
    assert_eq!(mint.mint(), None);
    assert_eq!(mint.mint(), None);
    assert_eq!(mint.last_issued.load(Ordering::Acquire), u64::MAX);
}

#[test]
fn drain_mint_stays_monotone_under_concurrent_minting() {
    const THREADS: u64 = 4;
    const PER_THREAD: u64 = 8;
    let mint = Arc::new(AgentDrainMint::new());
    let workers: Vec<_> = (0..THREADS)
        .map(|_| {
            let mint = Arc::clone(&mint);
            thread::spawn(move || {
                (0..PER_THREAD)
                    .filter_map(|_| mint.mint())
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    let mut tokens: Vec<u64> = Vec::new();
    for worker in workers {
        tokens.extend(
            worker
                .join()
                .unwrap_or_else(|_panic| panic!("minting thread panicked"))
                .iter()
                .map(|token| token.get()),
        );
    }
    tokens.sort_unstable();
    assert_eq!(
        tokens,
        (1..=THREADS * PER_THREAD).collect::<Vec<_>>(),
        "concurrent mints must be distinct, contiguous, and nonzero"
    );
    assert_eq!(mint.floor(), Some(THREADS * PER_THREAD + 1));
}

#[test]
fn drain_accessor_separates_none_pending_dead_and_live() {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    // active == None is the only true-none.
    assert!(matches!(
        drain_carrier_kind(&registry),
        AgentDrainCarrier::NoCarrier
    ));

    let generation = admit_first_worker(&registry, FIRST_WORKER);
    {
        // Pending admission: present but unregistered — never NoCarrier.
        let registry_guard = registry.lock().unwrap_or_else(|error| error.into_inner());
        assert!(registry_guard.request_handle().is_none());
        assert!(matches!(
            registry_guard.agent_drain_carrier(),
            AgentDrainCarrier::CarrierUnavailable
        ));
    }

    let carrier = create_worker_carrier(FIRST_WORKER, generation);
    let observer = carrier.request_handle();
    let owner = WorkerRegistry::register_carrier(&registry, generation, carrier)
        .unwrap_or_else(|| panic!("carrier was not registered"));
    {
        let registry_guard = registry.lock().unwrap_or_else(|error| error.into_inner());
        assert!(matches!(
            registry_guard.agent_drain_carrier(),
            AgentDrainCarrier::Live(_)
        ));
    }

    // Dead record: the wire owner dies while the registry gate keeps the
    // registration guard from retiring the generation — the same window as
    // inactive_carrier_blocks_replacement_until_registration_retires.
    let registry_guard = registry.lock().unwrap_or_else(|error| error.into_inner());
    let dropping_owner = thread::spawn(move || drop(owner));
    let deadline = Instant::now() + DRAIN_TEST_TIMEOUT;
    while observer.is_active() && Instant::now() < deadline {
        thread::yield_now();
    }
    assert!(
        !observer.is_active(),
        "carrier owner did not begin teardown"
    );
    assert!(registry_guard.request_handle().is_none());
    assert!(matches!(
        registry_guard.agent_drain_carrier(),
        AgentDrainCarrier::CarrierUnavailable
    ));
    drop(registry_guard);
    dropping_owner
        .join()
        .unwrap_or_else(|_panic| panic!("carrier owner teardown panicked"));

    assert!(matches!(
        drain_carrier_kind(&registry),
        AgentDrainCarrier::NoCarrier
    ));
}

#[test]
fn live_drain_target_carries_full_identity_and_one_mint_cell() {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let generation = admit_first_worker(&registry, FIRST_WORKER);
    let owner = register_first_carrier(&registry, FIRST_WORKER, generation);

    let first = live_drain_target(&registry);
    let second = live_drain_target(&registry);
    assert_eq!(first.worker_id.as_bytes(), FIRST_WORKER.as_bytes());
    assert_eq!(first.worker_generation.get(), generation.get());
    assert_eq!(first.request.worker_id(), FIRST_WORKER);
    assert_eq!(first.request.generation(), generation);
    // Both reads share the record's single mint cell.
    assert_eq!(first.mint.mint(), NonZeroU64::new(1));
    assert_eq!(second.mint.mint(), NonZeroU64::new(2));
    assert_eq!(second.mint.floor(), Some(3));

    drop(owner);
}

#[test]
fn unnameable_carrier_is_unavailable_not_true_none() {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let generation = admit_first_worker(&registry, UNNAMEABLE_WORKER);
    let owner = register_first_carrier(&registry, UNNAMEABLE_WORKER, generation);

    let registry_guard = registry.lock().unwrap_or_else(|error| error.into_inner());
    // The carrier is present and live on the Option surface, but its
    // all-zero WorkerId cannot be named on the drain wire: classified as
    // CarrierUnavailable — present-but-unnameable, never NoCarrier.
    assert!(registry_guard.request_handle().is_some());
    assert!(matches!(
        registry_guard.agent_drain_carrier(),
        AgentDrainCarrier::CarrierUnavailable
    ));
    drop(registry_guard);
    drop(owner);
}

#[test]
fn retired_target_mint_is_not_a_liveness_proof() {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let generation = admit_first_worker(&registry, FIRST_WORKER);
    let owner = register_first_carrier(&registry, FIRST_WORKER, generation);
    let target = live_drain_target(&registry);
    assert_eq!(target.mint.mint(), NonZeroU64::new(1));

    drop(owner);
    assert!(matches!(
        drain_carrier_kind(&registry),
        AgentDrainCarrier::NoCarrier
    ));
    assert!(!target.request.is_active());
    // The stale mint still issues dead-identity tokens — harmless and
    // unmatched; it is not evidence the carrier lives.
    assert!(target.mint.mint().is_some());
    // The admission fence on the dead handle is the liveness proof.
    let stale_request = AgentDrainRequest {
        worker_id: target.worker_id,
        worker_generation: target.worker_generation,
        token: target
            .mint
            .mint()
            .unwrap_or_else(|| panic!("mint exhausted")),
        acknowledged: 0,
    };
    assert_eq!(
        target
            .request
            .admit_agent_drain(stale_request)
            .map(|_exchange| ()),
        Err(WorkerRequestError::Unavailable)
    );
}

#[test]
fn each_registration_starts_a_fresh_drain_mint() {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let first_generation = admit_first_worker(&registry, FIRST_WORKER);
    let first_owner = register_first_carrier(&registry, FIRST_WORKER, first_generation);
    let first = live_drain_target(&registry);
    assert_eq!(first.mint.mint(), NonZeroU64::new(1));
    drop(first_owner);

    let second_generation = admit_first_worker(&registry, SECOND_WORKER);
    let second_owner = register_first_carrier(&registry, SECOND_WORKER, second_generation);
    let second = live_drain_target(&registry);
    // The replacement carrier's mint is a new cell — no cross-carrier reuse.
    assert_eq!(second.mint.mint(), NonZeroU64::new(1));
    drop(second_owner);
}

#[test]
fn mint_exhaustion_is_a_value_answer_not_a_carrier_event() {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let generation = admit_first_worker(&registry, FIRST_WORKER);
    let owner = register_first_carrier(&registry, FIRST_WORKER, generation);
    {
        let mut registry_guard = registry.lock().unwrap_or_else(|error| error.into_inner());
        let record = registry_guard
            .active
            .as_mut()
            .and_then(|active| active.carrier.as_mut())
            .unwrap_or_else(|| panic!("carrier record was not installed"));
        record
            .drain_mint
            .last_issued
            .store(u64::MAX - 1, Ordering::Release);
    }

    let target = live_drain_target(&registry);
    assert_eq!(target.mint.mint(), NonZeroU64::new(u64::MAX));
    assert_eq!(target.mint.mint(), None);
    assert_eq!(target.mint.floor(), None);
    // Exhaustion did not retire or downgrade the carrier.
    assert!(matches!(
        drain_carrier_kind(&registry),
        AgentDrainCarrier::Live(_)
    ));
    assert!(target.request.is_active());
    drop(owner);
}

#[test]
fn drain_target_drives_a_real_low_lane_exchange() {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let generation = admit_first_worker(&registry, FIRST_WORKER);
    let owner = register_first_carrier(&registry, FIRST_WORKER, generation);
    let target = live_drain_target(&registry);
    let token = target
        .mint
        .mint()
        .unwrap_or_else(|| panic!("mint exhausted"));
    let request = AgentDrainRequest {
        worker_id: target.worker_id,
        worker_generation: target.worker_generation,
        token,
        acknowledged: 0,
    };
    // Admit through the real LOW-lane seam before the owner runs: the queue
    // is in-memory, so admission precedes dispatch deterministically.
    let exchange = target
        .request
        .admit_agent_drain(request)
        .unwrap_or_else(|error| panic!("drain admission failed: {error:?}"));

    let (server, mut worker) =
        UnixStream::pair().unwrap_or_else(|error| panic!("socket pair failed: {error}"));
    // The real socket needs the production entry point: `run` applies
    // `configure_idle_stream` (read timeouts the idle probe depends on),
    // while `run_configured_for_test` exists for fake streams only.
    let owner_thread = thread::spawn(move || owner.run(server));
    let worker_thread = thread::spawn(move || {
        let observed = read_message(&mut worker)
            .unwrap_or_else(|error| panic!("fake Worker read failed: {error}"));
        assert_eq!(observed, CapabilityNodeMessage::AgentDrainRequest(request));
        let reply = AgentDrainReply::new(token, 0, 0, Vec::new())
            .unwrap_or_else(|error| panic!("reply construction failed: {error}"));
        write_message(&mut worker, &CapabilityNodeMessage::AgentDrainReply(reply))
            .unwrap_or_else(|error| panic!("fake Worker write failed: {error}"));
    });

    match exchange.recv_deadline(Instant::now() + DRAIN_TEST_TIMEOUT) {
        ExchangeWait::Ready(result) => match *result {
            Ok(WorkerOperationResult::AgentDrain(reply)) => {
                assert_eq!(reply.token_echo(), token);
                assert_eq!(reply.acknowledged_echo(), 0);
                assert_eq!(reply.queue_tail(), 0);
            }
            other => panic!("drain exchange did not complete: {other:?}"),
        },
        other => panic!("drain exchange did not complete: {other:?}"),
    }
    worker_thread
        .join()
        .unwrap_or_else(|_panic| panic!("fake Worker panicked"));
    let exit = owner_thread
        .join()
        .unwrap_or_else(|_panic| panic!("wire owner panicked"));
    assert_eq!(exit, WorkerWireExit::PeerClosed);
}

// ---------------------------------------------------------------------------
// Staged canonical-leaf lifecycle: the real `register_carrier_impl` path with
// an `AgentService` attached. Every test drives the same registration and
// drop owner the production path uses — there is no second modeled
// implementation — and the leaf is observed only through the service's own
// fenced folds.
// ---------------------------------------------------------------------------

fn open_agents() -> Arc<AgentService> {
    Arc::new(
        AgentService::open(AgentPolicy {
            read: true,
            manage: true,
        })
        .unwrap_or_else(|error| panic!("AgentService failed to open: {error}")),
    )
}

fn test_identity(worker_id: WorkerId, generation: NonZeroU64) -> CarrierIdentity {
    CarrierIdentity {
        worker_id: orchbee_capability_node_protocol::WorkerId::from_bytes(worker_id.as_bytes())
            .unwrap_or_else(|_error| panic!("worker id is not nameable on the CNP wire")),
        generation: orchbee_capability_node_protocol::WorkerGeneration::new(generation),
    }
}

/// Probe contract — NOT a read-only peek: `native_fenced_final_snapshot`
/// runs ONE fold that (a) fences the live carrier tuple against `identity`,
/// (b) checks runtime disposal, then (c) applies a whole-snapshot
/// `reconcile_complete_bindings(&[])` — an EMPTY binding reconcile that
/// legitimately mutates leaf bindings/effects inside the same guard before
/// cloning the snapshot. `Ok(Ok(Some))` = exact identity live + runtime
/// alive; `Ok(Err(CarrierMismatch))` = absent or different identity;
/// `Ok(Ok(None))` = disposed runtime; `Err` = poisoned cell.
fn leaf_holds(agents: &AgentService, identity: CarrierIdentity) -> bool {
    matches!(
        agents.native_fenced_final_snapshot(
            DrainExpectation::Carrier {
                req_carrier: identity,
                barrier: 0,
            },
            Vec::new(),
            Instant::now(),
        ),
        Ok(Ok(Some(_)))
    )
}

/// Same mutating-probe contract as `leaf_holds`, under the `NoCarrier`
/// expectation: `Ok(Ok(Some))` = leaf holds no carrier + runtime alive;
/// `Ok(Err(CarrierAppeared))` = a carrier is still admitted; `Ok(Ok(None))`
/// = disposed runtime; `Err` = poisoned cell.
fn leaf_empty(agents: &AgentService) -> bool {
    matches!(
        agents.native_fenced_final_snapshot(
            DrainExpectation::NoCarrier,
            Vec::new(),
            Instant::now(),
        ),
        Ok(Ok(Some(_)))
    )
}

fn register_with_agents(
    registry: &Arc<Mutex<WorkerRegistry>>,
    worker_id: WorkerId,
    generation: NonZeroU64,
    agents: &Arc<AgentService>,
) -> Option<super::RegisteredWorkerWireOwner> {
    let carrier = create_worker_carrier(worker_id, generation);
    WorkerRegistry::register_carrier_with_agents(registry, generation, carrier, agents)
}

#[test]
fn live_carrier_implies_exact_native_leaf_admitted() {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let agents = open_agents();
    let generation = admit_first_worker(&registry, FIRST_WORKER);
    let identity = test_identity(FIRST_WORKER, generation);
    let owner = register_with_agents(&registry, FIRST_WORKER, generation, &agents)
        .unwrap_or_else(|| panic!("carrier was not registered"));

    assert!(leaf_holds(&agents, identity));
    // A Live handle is exposed only after the leaf admitted.
    assert!(
        registry
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .request_handle()
            .is_some()
    );
    // The leaf is the exact pair: a different generation's identity is refused.
    let wrong = CarrierIdentity {
        generation: orchbee_capability_node_protocol::WorkerGeneration::new(
            NonZeroU64::new(generation.get() + 1).unwrap_or_else(|| panic!("next generation")),
        ),
        ..identity
    };
    assert!(!leaf_holds(&agents, wrong));
    drop(owner);
}

#[test]
fn native_leaf_refusal_leaves_no_live_carrier() {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let agents = open_agents();
    let generation = admit_first_worker(&registry, FIRST_WORKER);
    let identity = test_identity(FIRST_WORKER, generation);
    // Occupy the leaf with a different carrier identity first.
    let occupant = test_identity(SECOND_WORKER, generation);
    assert!(matches!(
        agents.native_admit_carrier(occupant, Instant::now()),
        Ok(Ok(()))
    ));

    assert!(register_with_agents(&registry, FIRST_WORKER, generation, &agents).is_none());
    assert!(
        registry
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .request_handle()
            .is_none()
    );
    assert!(!leaf_holds(&agents, identity));
    assert!(leaf_holds(&agents, occupant));

    // A refused registration legitimately leaves the pending admission for
    // the EXISTING caller to cancel — ownership is not moved to satisfy the
    // test. After cancellation the occupant's leaf retires through the
    // service's own seam (the fact its owner would deliver), and the same
    // caller-owned path admits and registers the replacement.
    {
        let mut registry_guard = registry.lock().unwrap_or_else(|error| error.into_inner());
        assert!(registry_guard.cancel_pending_admission(generation));
    }
    assert!(matches!(
        agents.native_retire_carrier(occupant, Instant::now()),
        Ok(())
    ));
    let replacement_generation = admit_first_worker(&registry, SECOND_WORKER);
    let replacement_owner =
        register_with_agents(&registry, SECOND_WORKER, replacement_generation, &agents)
            .unwrap_or_else(|| panic!("readmitted carrier was not registered"));
    assert!(leaf_holds(
        &agents,
        test_identity(SECOND_WORKER, replacement_generation)
    ));
    drop(replacement_owner);
}

#[test]
fn native_fold_poison_leaves_no_live_carrier() {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let agents = open_agents();
    let generation = admit_first_worker(&registry, FIRST_WORKER);
    let identity = test_identity(FIRST_WORKER, generation);
    // A committed leaf mutation followed by a fold fault permanently poisons
    // the cell — every later fold reports fold-unavailability.
    let _ = agents.native_poison_after_mutation_for_test(identity, Instant::now());
    assert!(agents.cell_poisoned_for_test());

    assert!(register_with_agents(&registry, FIRST_WORKER, generation, &agents).is_none());
    assert!(
        registry
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .request_handle()
            .is_none()
    );
    // The pending admission stays owned by the existing caller: cancellation
    // frees it, and the plain registration path — which never touches the
    // leaf — readmits the replacement normally even with the service cell
    // permanently poisoned.
    {
        let mut registry_guard = registry.lock().unwrap_or_else(|error| error.into_inner());
        assert!(registry_guard.cancel_pending_admission(generation));
    }
    let replacement_generation = admit_first_worker(&registry, SECOND_WORKER);
    let replacement_owner =
        register_first_carrier(&registry, SECOND_WORKER, replacement_generation);
    assert!(matches!(
        drain_carrier_kind(&registry),
        AgentDrainCarrier::Live(_)
    ));
    drop(replacement_owner);
}

#[test]
fn owner_teardown_retires_leaf_then_registry_before_replacement() {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let agents = open_agents();
    let first_generation = admit_first_worker(&registry, FIRST_WORKER);
    let first_identity = test_identity(FIRST_WORKER, first_generation);
    let owner = register_with_agents(&registry, FIRST_WORKER, first_generation, &agents)
        .unwrap_or_else(|| panic!("first carrier was not registered"));
    assert!(leaf_holds(&agents, first_identity));

    drop(owner);
    {
        let registry_guard = registry.lock().unwrap_or_else(|error| error.into_inner());
        assert!(generation_is_retired(&registry_guard, first_generation));
    }
    assert!(leaf_empty(&agents));

    let replacement_generation = admit_first_worker(&registry, SECOND_WORKER);
    let replacement_identity = test_identity(SECOND_WORKER, replacement_generation);
    let replacement_owner =
        register_with_agents(&registry, SECOND_WORKER, replacement_generation, &agents)
            .unwrap_or_else(|| panic!("replacement carrier was not registered"));
    assert!(leaf_holds(&agents, replacement_identity));
    assert!(matches!(
        drain_carrier_kind(&registry),
        AgentDrainCarrier::Live(_)
    ));
    drop(replacement_owner);
}

#[test]
fn stale_registration_cannot_retire_replacement_leaf() {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let agents = open_agents();
    let first_generation = admit_first_worker(&registry, FIRST_WORKER);
    let first_identity = test_identity(FIRST_WORKER, first_generation);
    {
        let mut registry_guard = registry.lock().unwrap_or_else(|error| error.into_inner());
        assert!(registry_guard.cancel_pending_admission(first_generation));
    }
    let replacement_generation = admit_first_worker(&registry, SECOND_WORKER);
    let replacement_identity = test_identity(SECOND_WORKER, replacement_generation);
    let replacement_owner =
        register_with_agents(&registry, SECOND_WORKER, replacement_generation, &agents)
            .unwrap_or_else(|| panic!("replacement carrier was not registered"));

    // A delayed old-registration guard carries the OLD leaf identity — both
    // fences (full identity in the leaf, generation in the registry) refuse it.
    drop(WorkerGenerationRegistration {
        generation: first_generation,
        registry: Arc::downgrade(&registry),
        leaf: Some(LeafBinding {
            identity: first_identity,
            agents: Arc::downgrade(&agents),
        }),
    });

    assert!(leaf_holds(&agents, replacement_identity));
    let registry_guard = registry.lock().unwrap_or_else(|error| error.into_inner());
    assert_eq!(
        registry_guard
            .request_handle()
            .map(|handle| handle.generation()),
        Some(replacement_generation)
    );
    drop(registry_guard);
    drop(replacement_owner);
}

#[test]
fn wire_owner_settles_before_native_leaf_retires() {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let agents = open_agents();
    let generation = admit_first_worker(&registry, FIRST_WORKER);
    let identity = test_identity(FIRST_WORKER, generation);
    let carrier = create_worker_carrier(FIRST_WORKER, generation);
    let observer = carrier.request_handle();
    let owner =
        WorkerRegistry::register_carrier_with_agents(&registry, generation, carrier, &agents)
            .unwrap_or_else(|| panic!("carrier was not registered"));

    // Holding the registry gate freezes the registration guard between the
    // wire-owner drop and the leaf retirement. The deterministic witness is
    // the wire owner's own liveness: `is_active() == false` proves wire
    // settlement completed while the registration guard could not have run —
    // its drop needs this same held lock. The mid-window leaf probe below is
    // supporting evidence only: the in-guard ordering itself is established
    // lexically in `WorkerGenerationRegistration::drop` (the leaf retire
    // sits inside the `registry.lock()` section), and the post-join outcome
    // asserts are the real completion proof. `join()` is unbounded owned
    // cleanup — a different discipline from a bounded receiver wait.
    let registry_guard = registry.lock().unwrap_or_else(|error| error.into_inner());
    let dropping_owner = thread::spawn(move || drop(owner));
    let deadline = Instant::now() + Duration::from_secs(2);
    while observer.is_active() && Instant::now() < deadline {
        thread::yield_now();
    }
    assert!(!observer.is_active(), "wire owner did not begin teardown");
    assert!(registry_guard.request_handle().is_none());
    assert!(leaf_holds(&agents, identity));
    drop(registry_guard);
    dropping_owner
        .join()
        .unwrap_or_else(|_panic| panic!("carrier owner teardown panicked"));
    assert!(leaf_empty(&agents));
    let registry_guard = registry.lock().unwrap_or_else(|error| error.into_inner());
    assert!(generation_is_retired(&registry_guard, generation));
}

#[test]
fn dead_service_still_allows_registry_retirement() {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let generation = admit_first_worker(&registry, FIRST_WORKER);
    let owner = {
        let agents = open_agents();
        let owner = register_with_agents(&registry, FIRST_WORKER, generation, &agents)
            .unwrap_or_else(|| panic!("carrier was not registered"));
        drop(agents);
        owner
    };

    drop(owner);
    let registry_guard = registry.lock().unwrap_or_else(|error| error.into_inner());
    assert!(generation_is_retired(&registry_guard, generation));
    assert!(registry_guard.request_handle().is_none());
}

#[test]
fn poisoned_service_still_allows_registry_retirement() {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let agents = open_agents();
    let generation = admit_first_worker(&registry, FIRST_WORKER);
    let identity = test_identity(FIRST_WORKER, generation);
    let owner = register_with_agents(&registry, FIRST_WORKER, generation, &agents)
        .unwrap_or_else(|| panic!("carrier was not registered"));
    let _ = agents.native_poison_after_mutation_for_test(identity, Instant::now());
    assert!(agents.cell_poisoned_for_test());

    drop(owner);
    let registry_guard = registry.lock().unwrap_or_else(|error| error.into_inner());
    assert!(generation_is_retired(&registry_guard, generation));
    assert!(registry_guard.request_handle().is_none());
}

#[test]
fn disposed_runtime_bookkeeping_never_yields_healthy_read() {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let agents = open_agents();
    agents.shutdown();
    // The runtime gate is absolute: a disposed service answers reads with
    // RuntimeUnavailable regardless of leaf bookkeeping.
    let AgentReadResult::List { result, .. } = agents
        .execute(AgentReadRequest::List {
            request_id: ObapRequestId::parse(&format!("{:032x}", 9u8))
                .unwrap_or_else(|_error| panic!("request id parse failed")),
            include_ended: false,
            limit: AgentSafeInteger::new(50),
            cursor: None,
        })
        .unwrap_or_else(|fault| panic!("read execution faulted: {fault}"))
    else {
        panic!("expected a list result");
    };
    assert_eq!(
        result.outcome,
        AgentListOutcome::Unavailable {
            reason: AgentUnavailableReason::RuntimeUnavailable,
        }
    );

    // Leaf bookkeeping still admits/retires on a disposed runtime — the fold
    // is not gated on disposal — but the fenced read can only yield None.
    let generation = admit_first_worker(&registry, FIRST_WORKER);
    let identity = test_identity(FIRST_WORKER, generation);
    let owner = register_with_agents(&registry, FIRST_WORKER, generation, &agents)
        .unwrap_or_else(|| panic!("disposed service must still admit leaf bookkeeping"));
    assert!(matches!(
        agents.native_fenced_final_snapshot(
            DrainExpectation::Carrier {
                req_carrier: identity,
                barrier: 0,
            },
            Vec::new(),
            Instant::now(),
        ),
        Ok(Ok(None))
    ));
    drop(owner);
    let registry_guard = registry.lock().unwrap_or_else(|error| error.into_inner());
    assert!(generation_is_retired(&registry_guard, generation));
}

#[test]
fn peer_close_teardown_retires_leaf_and_registry() {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let agents = open_agents();
    let generation = admit_first_worker(&registry, FIRST_WORKER);
    let identity = test_identity(FIRST_WORKER, generation);
    let owner = register_with_agents(&registry, FIRST_WORKER, generation, &agents)
        .unwrap_or_else(|| panic!("carrier was not registered"));
    assert!(leaf_holds(&agents, identity));

    let (server, worker) =
        UnixStream::pair().unwrap_or_else(|error| panic!("socket pair failed: {error}"));
    let owner_thread = thread::spawn(move || owner.run(server));
    drop(worker);
    let exit = owner_thread
        .join()
        .unwrap_or_else(|_panic| panic!("wire owner panicked"));
    assert_eq!(exit, WorkerWireExit::PeerClosed);

    assert!(leaf_empty(&agents));
    let registry_guard = registry.lock().unwrap_or_else(|error| error.into_inner());
    assert!(generation_is_retired(&registry_guard, generation));
    assert!(registry_guard.request_handle().is_none());
}
