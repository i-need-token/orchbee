//! `agent_drain_read` — the production Agent drain read pump.
//!
//! The pump is the `prepare` provider handed to
//! `AgentService::execute_prepared`: policy admission runs before it, it
//! is invoked at most once outside the `AgentState` lock, and every
//! `Err(PreparedUnavailable)` maps to the existing `RuntimeUnavailable`
//! wire arm. It owns no new production surface: no new socket, thread,
//! runtime, setting, protocol, or window value; the production consumer
//! is the `LocalTerminalService` prepared-read provider wired through
//! `AgentService::execute_prepared`.
//!
//! Contract (procedure42 + Root corrections):
//!
//! * Step 0 takes the typed `agent_drain_carrier()` tri-state under a
//!   scoped registry guard. `NoCarrier` is the only true-none path;
//!   `CarrierUnavailable` (pending admission, dead handle, unnameable
//!   identity) is `Unavailable`, never conflated with `NoCarrier`.
//! * A Carrier read MUST complete at least one fresh typed exchange
//!   before serving: mint a token from the carrier's shared
//!   `AgentDrainMint` (checked CAS; exhaustion is `None`, never a
//!   mutation), admit on the request handle (at most one
//!   `PendingExchange` in flight at any instant), wait with
//!   `recv_deadline`, then apply the entire validated page in ONE
//!   `native_apply_drain_page` fold under the full
//!   `{worker_id, generation}` fence. The barrier is pinned once from
//!   the FIRST carrier-matched reply's `queue_tail`, after that reply's
//!   own apply — a live leaf watermark alone can never mint a barrier.
//! * `w_obs` is the last `Applied { w }` the leaf reported; the next
//!   request's `acknowledged` echoes it. Acknowledgement safety is a
//!   validation-point property of the serialized Worker loop
//!   (send-Ok -> `commit_offered` -> next `receive`/`respond`), not an
//!   all-times `leaf W <= offered_hi` claim: the Server may fold a
//!   reply while the Worker is descheduled before its commit, and the
//!   echoed acknowledgement is only ever validated at the NEXT
//!   `respond`, which runs strictly after that commit.
//! * FINAL is the only multi-guard section: the registry guard is
//!   acquired first, then the coordinator guard, and both remain held
//!   lexically through the carrier re-check, the real
//!   `agent_registration_snapshot()` projection, binding mapping, the
//!   `native_fenced_final_snapshot` fold (which takes the AgentState
//!   lock inside the service seam), and the post-fold deadline
//!   recheck — dropped only on success return or error exit. Lock
//!   order is registry -> coordinator -> AgentState; nothing takes
//!   the reverse edge and no queue admit, wait, receive, send, or I/O
//!   runs inside either guard.
//! * The deadline is caller-supplied once and never reset. Because
//!   mutex acquisition is cooperative the pump can overrun; every
//!   acquisition and phase boundary is followed by an explicit
//!   elapsed check, a `Ready` reply is not window proof (a buffered
//!   result can surface at an already-elapsed deadline), and a late
//!   result is rejected without rollback — no hard-duration promise.
//!
//! `retry` is a supplied pacing duration for the `Backpressure` busy
//! result only; it is a private body parameter, not a product setting.

use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use crate::agent::{
    AgentRuntimeSnapshot,
    native_observation::{Binding, CarrierIdentity},
};
use crate::agent_service::{AgentService, DrainExpectation, DrainPageOutcome, PreparedUnavailable};
use crate::site_coordinator::SiteCoordinator;
use crate::worker_obcn_carrier::{
    ExchangeWait, PendingExchange, WorkerOperationResult, WorkerRequestError,
};
use crate::worker_registry::{AgentDrainCarrier, AgentDrainTarget, WorkerRegistry};
use orchbee_capability_node_protocol::AgentDrainRequest;

/// What the step-0 registry read established: only the provenance of the
/// read — the `DrainExpectation` itself cannot be built until FINAL,
/// because a Carrier expectation needs the barrier the first
/// carrier-matched reply has not produced yet.
enum Provenance {
    NoCarrier,
    Carrier {
        target: AgentDrainTarget,
        req: CarrierIdentity,
    },
}

/// The drain read pump body. Request-local state only; the one shared
/// cell is the carrier's mint `Arc`. Every `Err` is `PreparedUnavailable`
/// — the executor maps it to the existing `RuntimeUnavailable` wire arm.
pub(crate) fn agent_drain_prepare(
    workers: &Mutex<WorkerRegistry>,
    coordinator: &Mutex<SiteCoordinator>,
    agents: &AgentService,
    deadline: Instant,
    retry: Duration,
) -> Result<AgentRuntimeSnapshot, PreparedUnavailable> {
    agent_drain_finish(
        workers,
        coordinator,
        agents,
        deadline,
        retry,
        |_, snapshot| Ok(snapshot),
    )
}

pub(super) fn agent_drain_finish<R>(
    workers: &Mutex<WorkerRegistry>,
    coordinator: &Mutex<SiteCoordinator>,
    agents: &AgentService,
    deadline: Instant,
    retry: Duration,
    finish: impl FnOnce(&SiteCoordinator, AgentRuntimeSnapshot) -> Result<R, PreparedUnavailable>,
) -> Result<R, PreparedUnavailable> {
    // STEP 0 — eligible preparation. The registry guard is scoped to the
    // typed accessor and released before any mint/admit/wait work.
    let provenance = {
        let registry = workers.lock().map_err(|_| PreparedUnavailable)?;
        match registry.agent_drain_carrier() {
            AgentDrainCarrier::NoCarrier => Provenance::NoCarrier,
            AgentDrainCarrier::CarrierUnavailable => {
                return Err(PreparedUnavailable);
            }
            AgentDrainCarrier::Live(target) => Provenance::Carrier {
                req: CarrierIdentity {
                    worker_id: target.worker_id,
                    generation: target.worker_generation,
                },
                target,
            },
        }
    };

    // CAPTURE — Carrier provenance only. At most one exchange is in
    // flight at any instant; `barrier` is pinned exactly once from the
    // first carrier-matched reply's `queue_tail`, after its own apply.
    let mut barrier: Option<u64> = None;
    if let Provenance::Carrier { ref target, req } = provenance {
        if target.mint.floor().is_none() {
            return Err(PreparedUnavailable);
        }
        let mut w_obs: u64 = 0;
        let mut attempt: Option<PendingExchange> = None;
        loop {
            if Instant::now() >= deadline {
                return Err(PreparedUnavailable);
            }
            if attempt.is_none() {
                let token = target.mint.mint().ok_or(PreparedUnavailable)?;
                let request = AgentDrainRequest {
                    worker_id: target.worker_id,
                    worker_generation: target.worker_generation,
                    token,
                    acknowledged: w_obs,
                };
                match target.request.admit_agent_drain(request) {
                    Ok(exchange) => attempt = Some(exchange),
                    Err(WorkerRequestError::Backpressure) => {
                        // Cooperative pacing only — saturating
                        // subtraction, never a raw deadline - now: a past
                        // deadline yields zero sleep and the loop-top
                        // guard exits on the next pass.
                        thread::sleep(
                            retry.min(deadline.saturating_duration_since(Instant::now())),
                        );
                    }
                    Err(_) => return Err(PreparedUnavailable),
                }
                continue;
            }
            match attempt
                .as_ref()
                .expect("attempt is populated above")
                .recv_deadline(deadline)
            {
                ExchangeWait::Elapsed => return Err(PreparedUnavailable),
                ExchangeWait::Lost(_) => return Err(PreparedUnavailable),
                ExchangeWait::Ready(result) => match *result {
                    Err(_) => return Err(PreparedUnavailable),
                    Ok(WorkerOperationResult::AgentDrain(reply)) => {
                        attempt = None;
                        // A buffered Ready can surface at an already-elapsed
                        // deadline — Ready is not window proof; check elapsed
                        // explicitly before consuming the reply.
                        if Instant::now() >= deadline {
                            return Err(PreparedUnavailable);
                        }
                        match agents.native_apply_drain_page(req, &reply, Instant::now()) {
                            Err(_) | Ok(Err(_)) => return Err(PreparedUnavailable),
                            Ok(Ok(DrainPageOutcome::CarrierMismatch)) => {
                                return Err(PreparedUnavailable);
                            }
                            Ok(Ok(DrainPageOutcome::Applied { w })) => w_obs = w,
                        }
                        // The barrier pins once, from the FIRST carrier-matched
                        // reply's `queue_tail` — after that reply's own apply.
                        if barrier.is_none() {
                            barrier = Some(reply.queue_tail());
                        }
                        if w_obs >= barrier.expect("pinned above") {
                            break;
                        }
                    }
                    Ok(_) => return Err(PreparedUnavailable),
                },
            }
        }
    }

    // FINAL — the only multi-guard section. `registry_guard` is acquired
    // first, then `coordinator_guard`, and BOTH stay locked until the
    // explicit drops at the bottom of this section: they span the carrier
    // re-check, the real `agent_registration_snapshot()` projection,
    // binding mapping, the `native_fenced_final_snapshot` fold (the
    // AgentState lock is taken inside the service seam), and the
    // post-fold deadline recheck — they release only on the success
    // return or on an early error exit. The lock order is registry ->
    // coordinator -> AgentState; nothing takes the reverse edge, and no
    // queue admit, wait, receive, send, or I/O runs inside either guard.
    let registry_guard = workers.lock().map_err(|_| PreparedUnavailable)?;
    if Instant::now() >= deadline {
        return Err(PreparedUnavailable);
    }
    let expected = match (&provenance, registry_guard.agent_drain_carrier()) {
        (Provenance::Carrier { req, .. }, AgentDrainCarrier::Live(current))
            if current.worker_id == req.worker_id
                && current.worker_generation == req.generation =>
        {
            DrainExpectation::Carrier {
                req_carrier: *req,
                barrier: barrier.unwrap_or_else(|| unreachable!("capture implies pin")),
            }
        }
        (Provenance::NoCarrier, AgentDrainCarrier::NoCarrier) => DrainExpectation::NoCarrier,
        // Carrier read: a rotated, pending, or dead carrier at the
        // re-check is Unavailable. NoCarrier read: a carrier that
        // appeared (or cannot be classified) before finalization is
        // Unavailable — the present registry authority cannot be
        // replaced by leaf absence.
        _ => return Err(PreparedUnavailable),
    };
    let coordinator_guard = coordinator.lock().map_err(|_| PreparedUnavailable)?;
    if Instant::now() >= deadline {
        return Err(PreparedUnavailable);
    }
    let facts = coordinator_guard
        .agent_registration_snapshot()
        .map_err(|_| PreparedUnavailable)?;
    let bindings: Vec<Binding> = facts
        .iter()
        .map(|fact| Binding {
            presentation: fact.presentation_id().clone(),
            carrier: CarrierIdentity {
                worker_id: fact.worker_id(),
                generation: fact.worker_generation(),
            },
            host: fact.identity(),
        })
        .collect();
    // Post-mapping recheck before the fold, and again after it — a late
    // result is rejected without rollback.
    if Instant::now() >= deadline {
        return Err(PreparedUnavailable);
    }
    let snapshot = match agents.native_fenced_final_snapshot(expected, bindings, Instant::now()) {
        Err(_) | Ok(Err(_)) | Ok(Ok(None)) => return Err(PreparedUnavailable),
        Ok(Ok(Some(snapshot))) => snapshot,
    };
    if Instant::now() >= deadline {
        return Err(PreparedUnavailable);
    }
    let result = finish(&coordinator_guard, snapshot);
    // The final callback may commit a mutation: preserve its correlated receipt even if time passed.
    drop(coordinator_guard);
    drop(registry_guard);
    result
}

#[cfg(test)]
mod tests;
