//! The binary-private Agent observation and runtime alias service. It owns the one
//! canonical `AgentState` cell behind a leaf lock — the single
//! `AgentRuntime` plus the hook translator, which borrows the
//! runtime per call instead of owning a second one — mints and verifies
//! boot-scoped opaque list cursors, and executes the OBAP
//! list/get/explain first-read. Read projection never mutates
//! canonical state: every eligible request receives the snapshot cloned by its
//! authorized preparation fold, releases every guard, then projects and maps.
//! Runtime disposal is the single unavailability authority — shutdown
//! disposes the runtime under the same lock so surviving connections
//! observe `runtime-unavailable` rather than fabricated state. A fold that
//! panics costs only its own call: the cell is rebuilt from the facts earlier
//! folds committed before anything reads it again.

mod changes;
mod cursor;
mod launch;
mod map;
mod names;
#[cfg(any(windows, target_os = "macos", test))]
mod resume;
pub(crate) use names::ObservedCommand;
pub(crate) use names::TerminalNameFact;
#[cfg(any(windows, target_os = "macos", test))]
pub(crate) use resume::AgentResumeCandidate;
#[cfg(test)]
mod tests;

use std::fmt::{self, Display, Formatter};
use std::io;
use std::sync::{Mutex, MutexGuard};

use orchbee_agent_observation_protocol::{
    AgentExplainOutcome, AgentExplainOutcomeEnvelope, AgentGetOutcome, AgentGetOutcomeEnvelope,
    AgentListOutcome, AgentListOutcomeEnvelope, AgentListRejectionReason, AgentReadRequest,
    AgentReadResult, AgentSessionRejectionReason, AgentUnavailableReason,
};

use crate::agent::{AgentLifecycle, AgentRuntime, AgentRuntimeSnapshot, HookGeneration};

use std::num::NonZeroU64;
use std::time::{Duration, Instant};

use orchbee_capability_node_protocol::{AgentDrainEntry, AgentDrainReply};

use crate::agent::AgentModelError;
use crate::agent::native_observation::{
    Binding, CarrierIdentity, NativeEvent, NativeObservation, NativeObservationEffect,
};
use crate::fault_records::{self, RecordInterval};

/// One validated domain fact lifted from a typed drain page. Private on
/// purpose: the only construction route is `drain_facts`, so an
/// arbitrary or mixed-carrier fact vector is unrepresentable.
enum DrainFact {
    Event(NativeEvent),
    Gap { lo: NonZeroU64, hi: u64 },
}

/// The page-apply outcome — a whole typed page under one
/// full-identity fence inside a single `with_state` section.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DrainPageOutcome {
    /// The carrier matched; `w` is the live watermark read after the
    /// page — an empty or fully-deduped page still returns it.
    Applied { w: u64 },
    /// The leaf carrier was absent, retired, or replaced — zero entries
    /// applied, zero mutation.
    CarrierMismatch,
}

/// The final-phase expectation for one drain read: the carrier the
/// read fenced on plus its pinned barrier, or the read's step-0
/// true-none.
#[derive(Clone, Copy, Debug)]
pub(crate) enum DrainExpectation {
    Carrier {
        req_carrier: CarrierIdentity,
        barrier: u64,
    },
    NoCarrier,
}

/// Fenced-final refusal — every arm is a typed cause, never a silent
/// serve.
#[derive(Debug)]
pub(crate) enum FinalFence {
    /// The leaf carrier differed from the requested full identity.
    CarrierMismatch,
    /// A `NoCarrier` read found a carrier installed by final time.
    CarrierAppeared,
    /// The live watermark sat below the pinned barrier — defensive so
    /// the gate is total; unreachable while identity matches.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "fence diagnostics travel to callers that fold every cause to PreparedUnavailable"
        )
    )]
    BarrierUnmet { w: u64, barrier: u64 },
    /// The whole-facts snapshot was rejected atomically — leaf bindings
    /// and the translator are untouched.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "fence diagnostics travel to callers that fold every cause to PreparedUnavailable"
        )
    )]
    Rejected(AgentModelError),
}

/// Lifts every domain fact for one drain page BEFORE the `AgentState`
/// fold: each `NativeEvent` is constructed with the read's requested
/// carrier, and gap bounds come straight off the wire-validated
/// entries. The typed reply already guarantees contiguous bounded
/// coverage, per-entry payload bounds, and ordered gaps; this adapter
/// supplies only the domain carrier — a construction failure is a
/// typed refusal returned before any fold, so nothing mutates.
fn drain_facts(
    req_carrier: CarrierIdentity,
    reply: &AgentDrainReply,
) -> Result<Vec<DrainFact>, AgentModelError> {
    reply
        .entries()
        .iter()
        .map(|entry| match entry {
            AgentDrainEntry::Event {
                sequence,
                identity,
                payload,
            } => NativeEvent::new(req_carrier, *sequence, *identity, payload.clone())
                .map(DrainFact::Event),
            AgentDrainEntry::Gap { low, high } => Ok(DrainFact::Gap {
                lo: *low,
                hi: *high,
            }),
        })
        .collect()
}

/// The sequence range one drain page covers, from its first entry's start to its last entry's
/// end; `None` for an empty page.
fn page_range(reply: &AgentDrainReply) -> Option<(u64, u64)> {
    let entries = reply.entries();
    Some((entries.first()?.start(), entries.last()?.end()))
}

/// Canonical 32-lowercase-hex rendering of a 16-byte identity — the
/// join/projection string form for `HostCreationId` (join's
/// `generation`) and other CNP identities at the translator seam.
fn canonical_hex32(bytes: [u8; 16]) -> String {
    let mut out = String::with_capacity(32);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

use cursor::{AgentCursorAuthority, AgentCursorRejection};

/// Admission-time read capability granted to this listener's local
/// clients, mirroring `WorkbenchPolicy`.
pub(crate) struct AgentPolicy {
    /// Whether the admitted client may execute Agent read operations.
    pub read: bool,
    /// Whether the admitted client may assign or release session aliases.
    pub manage: bool,
}

/// A request that cannot be answered because canonical state could not be
/// represented on the wire, or because cursor authority could not act.
/// Loudly ends the record loop — the malformed record is never silently
/// skipped and unavailable state is never fabricated.
#[derive(Debug)]
pub(crate) struct AgentReadFault(pub(crate) &'static str);

impl Display for AgentReadFault {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for AgentReadFault {}

/// The one canonical read/write state inside the service's leaf mutex: the
/// single `AgentRuntime` plus the hook-observation translator state.
/// The translator never owns a runtime — it receives `&mut runtime` per
/// call — so reads, dispatches, and (later) held-hint replay all share
/// this state. `AgentState` never acquires registry or coordinator locks;
/// callers pass already-fenced facts in.
pub(crate) struct AgentState {
    runtime: AgentRuntime,
    ingress: crate::agent::AgentHookObservationIngress,
    /// The native observation leaf — held hints, the one current
    /// carrier's scalar W, and committed bindings. The managed socket /
    /// Worker drain / registry admission path owns every producer;
    /// callers pass already-fenced facts in.
    native: NativeObservation,
    names: names::Names,
    /// One monotonic origin owned by the service; leaf timestamps and
    /// the JSM `now_ms` translator clock both derive from it so a benign
    /// earlier caller instant saturates to zero instead of panicking.
    clock_origin: Instant,
}

impl AgentState {
    fn launch_now_ms(&self, now: Instant) -> u64 {
        u64::try_from(now.saturating_duration_since(self.clock_origin).as_millis())
            .unwrap_or(u64::MAX)
    }

    fn reconcile_launch_names(&mut self, now: Instant) {
        let milliseconds = self.launch_now_ms(now);
        self.names.reconcile_launches(
            self.native.bindings(),
            milliseconds,
            self.runtime.is_disposed(),
        );
    }

    /// Opens the state with a genuine empty canonical runtime. The
    /// translator receives the observation socket path at construction —
    /// `None` when the owning session has no managed endpoint, and also when
    /// the bound endpoint's path has no UTF-8 overlay representation while
    /// the endpoint itself remains bound — so the path is immutable before
    /// the cell is ever shared.
    fn open(socket_path: Option<String>, clock_origin: Instant) -> Self {
        Self {
            runtime: AgentRuntime::new(),
            ingress: crate::agent::AgentHookObservationIngress::new(
                socket_path,
                crate::agent::JoinOptions::default(),
            ),
            native: NativeObservation::new(),
            names: names::Names::default(),
            clock_origin,
        }
    }

    /// The state a recovering cell starts from: only `committed`'s facts, with the same socket
    /// path and clock origin. Sessions, the journal, join state, held hints, bindings and names
    /// start empty.
    fn rebuilt(committed: &Committed, socket_path: Option<String>, clock_origin: Instant) -> Self {
        Self {
            runtime: AgentRuntime::rebuilt(committed.revision, committed.disposed),
            ingress: crate::agent::AgentHookObservationIngress::rebuilt(
                socket_path,
                crate::agent::JoinOptions::default(),
                committed.identifiers,
            ),
            native: NativeObservation::rebuilt(committed.carrier),
            names: names::Names::default(),
            clock_origin,
        }
    }

    /// Applies one owned leaf effect batch inside the same cell guard —
    /// the atomicity boundary: join fences and hint replays can never
    /// outlive the fold that produced them. Raw payloads are moved, not
    /// cloned. `now` is the caller's monotonic instant converted to the
    /// translator's f64 milliseconds since the service-owned origin;
    /// earlier-than-origin saturates to 0.0 (never a fold fault).
    fn apply_native_effects(&mut self, effects: Vec<NativeObservationEffect>, now: Instant) {
        let AgentState {
            runtime,
            ingress,
            clock_origin,
            ..
        } = self;
        let now_ms = now
            .checked_duration_since(*clock_origin)
            .map_or(0.0, |elapsed| elapsed.as_secs_f64() * 1000.0);
        // The single native Instant funnel head: RouteHint replays can
        // emit ScheduleSettlement inside this fold, so the scheduler
        // base must track the freshest entry instant — never the stale
        // base of an earlier admit. The f64 corpus seams stay
        // unreached by this call.
        ingress.sync_settlement_clock(now_ms);
        for effect in effects {
            match effect {
                NativeObservationEffect::RouteHint {
                    raw,
                    host,
                    presentation,
                } => {
                    ingress.observe_terminal_application_signal(
                        runtime,
                        &raw,
                        canonical_hex32(host.creation_id().as_bytes()),
                        presentation.as_str().to_owned(),
                        now_ms,
                    );
                }
                NativeObservationEffect::RegisterPresentation {
                    presentation,
                    creation_id,
                } => {
                    ingress.register_presentation_binding(
                        runtime,
                        presentation.as_str().to_owned(),
                        canonical_hex32(creation_id.as_bytes()),
                    );
                }
                NativeObservationEffect::RetirePresentation { presentation } => {
                    ingress.retire_presentation(runtime, presentation.as_str());
                }
            }
        }
    }
}

/// The facts a rebuilt cell keeps: copied out after every fold that returns and adjusted before a
/// fold runs, never read from a state a panic left behind.
#[derive(Clone, Copy)]
struct Committed {
    /// A rebuild continues one past it, so no cursor or baseline from before stays current.
    revision: u64,
    /// Disposal stays the single unavailability authority across a rebuild.
    disposed: bool,
    /// The registry's carrier and its watermark: the Worker trims the prefix it was acknowledged
    /// for, so a rebuilt watermark at zero would read every later page as a hole.
    carrier: Option<(CarrierIdentity, u64)>,
    /// The translator's minted identifiers, so no `desktop:agent:*:N` identity repeats.
    identifiers: u64,
}

impl Committed {
    fn of(state: &AgentState) -> Self {
        Self {
            revision: state.runtime.snapshot().revision,
            disposed: state.runtime.is_disposed(),
            carrier: state.native.current_carrier(),
            identifiers: state.ingress.allocated_identifiers(),
        }
    }

    fn prepare(&mut self, on_fault: OnFault) {
        match on_fault {
            OnFault::Keep => {}
            OnFault::Dispose => self.disposed = true,
            OnFault::Forget(carrier) => {
                if self.carrier.is_some_and(|(current, _w)| current == carrier) {
                    self.carrier = None;
                }
            }
            // The leaf's gap rule: a range starting within `w + 1` and reaching past `w` moves the
            // watermark to its end.
            OnFault::Skip { carrier, low, high } => {
                if let Some((current, w)) = &mut self.carrier
                    && *current == carrier
                    && high > *w
                    && low <= *w + 1
                {
                    *w = high;
                }
            }
        }
    }
}

/// What a fold's own fault leaves of the committed facts a rebuild starts from. It is applied
/// before the fold runs; a fold that returns replaces the facts with its own.
#[derive(Clone, Copy)]
enum OnFault {
    Keep,
    /// A shutdown disposes the rebuilt runtime, so a fault never brings back a live runtime after
    /// its owner stopped serving.
    Dispose,
    /// A retirement forgets its carrier, so a fault never brings back a carrier the registry has
    /// released and every later Worker could not be admitted.
    Forget(CarrierIdentity),
    /// A drain page counts as lost, as a Worker gap over the same range would, so a page whose fold
    /// faults is never offered to the rebuilt state again.
    Skip {
        carrier: CarrierIdentity,
        low: u64,
        high: u64,
    },
}

/// The service's one locked cell: the Agent state, the facts a rebuild keeps, what a rebuild needs
/// beyond them, and the cell's own record pacing, which outlives every rebuild.
struct AgentCell {
    state: AgentState,
    committed: Committed,
    socket_path: Option<String>,
    clock_origin: Instant,
    refusals: RecordInterval,
    recoveries: RecordInterval,
}

impl AgentCell {
    fn open(socket_path: Option<String>) -> Self {
        let clock_origin = Instant::now();
        let state = AgentState::open(socket_path.clone(), clock_origin);
        Self {
            committed: Committed::of(&state),
            state,
            socket_path,
            clock_origin,
            refusals: RecordInterval::default(),
            recoveries: RecordInterval::default(),
        }
    }

    /// Replaces the state with one rebuilt from the committed facts and records the recovery.
    fn rebuild(&mut self) {
        self.state =
            AgentState::rebuilt(&self.committed, self.socket_path.clone(), self.clock_origin);
        let carrier = self.committed.carrier.is_some();
        self.committed = Committed::of(&self.state);
        fault_records::agent_state_recovered(
            &mut self.recoveries,
            self.committed.revision,
            carrier,
        );
    }
}

/// A fold that panicked. Only its own call is unavailable: the cell is rebuilt
/// before any later call reads it. Reads map this to the existing
/// `RuntimeUnavailable` wire arm and write seams surface `Err(..)` so a fault
/// is never confused with a rejected payload.
#[derive(Debug)]
pub(crate) struct AgentFoldUnavailable;

/// Snapshot preparation declined or failed for one read: the required
/// provider could not hand back a canonical snapshot. The shared
/// executor maps it to the requesting operation's existing
/// `RuntimeUnavailable` arm — never an `AgentReadFault`, never a
/// fabricated or last-good state, never a suppressed panic result.
#[derive(Debug)]
pub(crate) struct PreparedUnavailable;

/// Owns the canonical Agent runtime and the private list-cursor authority.
pub(crate) struct AgentService {
    changes: orchbee_server::SessionChangeSource,
    cursors: AgentCursorAuthority,
    policy: AgentPolicy,
    cell: Mutex<AgentCell>,
}

impl AgentService {
    /// Creates the service with a genuine empty canonical runtime and no
    /// observation socket — the narrow wrapper for owners that have no
    /// managed endpoint.
    pub(crate) fn open(policy: AgentPolicy) -> io::Result<Self> {
        Self::open_observing(policy, None)
    }

    /// The production constructor: the observation socket path is bound
    /// into the translator at construction, before the returned service
    /// can be shared — there is no post-construction path mutation.
    pub(crate) fn open_observing(
        policy: AgentPolicy,
        socket_path: Option<String>,
    ) -> io::Result<Self> {
        Ok(Self {
            changes: orchbee_server::SessionChangeSource::default(),
            cursors: AgentCursorAuthority::open()?,
            policy,
            cell: Mutex::new(AgentCell::open(socket_path)),
        })
    }

    /// The fold boundary for every access that keeps the committed facts.
    fn with_state<R>(
        &self,
        f: impl FnOnce(&mut AgentState) -> R,
    ) -> Result<R, AgentFoldUnavailable> {
        self.fold(OnFault::Keep, f)
    }

    /// The single boundary every `AgentState` access runs through. `f` performs CPU work on the
    /// state only: no outer guards, no socket/ACK/timer I/O. A fold that returns copies out the
    /// committed facts and writes the cell's paced `agent-change-refused` record for any change
    /// the runtime refused as invalid.
    ///
    /// A panic anywhere in the fold is caught here, inside the caller's outer guards, and costs only
    /// this call: it returns `AgentFoldUnavailable`, and the cell is rebuilt from the committed
    /// facts — adjusted by `on_fault` before `f` ran — before anything reads it again: right after
    /// the catch, or by whichever caller first finds the lock poisoned, so a second fault recovers
    /// the same way. `AssertUnwindSafe` is sound because the state a panic leaves is never read
    /// again; only the committed facts, written between folds, outlive it.
    fn fold<R>(
        &self,
        on_fault: OnFault,
        f: impl FnOnce(&mut AgentState) -> R,
    ) -> Result<R, AgentFoldUnavailable> {
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let (mut guard, rebuilt) = self.lock_cell();
            let cell = &mut *guard;
            cell.committed.prepare(on_fault);
            let state = &mut cell.state;
            let previous = (
                state.runtime.snapshot().revision,
                state.runtime.is_disposed(),
            );
            state.reconcile_launch_names(Instant::now());
            let result = f(state);
            state.reconcile_launch_names(Instant::now());
            let AgentState { runtime, names, .. } = &mut *state;
            names.reconcile(runtime);
            if let Some((transition, refusals)) = state.runtime.take_refused() {
                fault_records::agent_change_refused(&mut cell.refusals, transition, refusals);
            }
            cell.committed = Committed::of(state);
            let changed = rebuilt || previous != (cell.committed.revision, cell.committed.disposed);
            (result, changed)
        }));
        match outcome {
            Ok((result, changed)) => {
                if changed {
                    self.changes.notify();
                }
                Ok(result)
            }
            Err(_) => {
                // Unwinding dropped the guard and poisoned the lock: rebuild now so change
                // watchers learn of it at once. A rebuild that panics leaves the lock poisoned
                // for the next caller to rebuild.
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    drop(self.lock_cell());
                }));
                self.changes.notify();
                Err(AgentFoldUnavailable)
            }
        }
    }

    /// Locks the cell. A lock that a panicked fold left poisoned is rebuilt from the committed
    /// facts first, so no caller ever reads the state that panic left; `true` reports the rebuild.
    fn lock_cell(&self) -> (MutexGuard<'_, AgentCell>, bool) {
        match self.cell.lock() {
            Ok(cell) => (cell, false),
            Err(poisoned) => {
                let mut cell = poisoned.into_inner();
                cell.rebuild();
                self.cell.clear_poison();
                (cell, true)
            }
        }
    }

    pub(crate) fn watch_changes(
        &self,
        signal: &std::sync::Arc<orchbee_server::SessionChangeSignal>,
    ) {
        self.changes.watch(signal);
    }

    /// Disposes the canonical runtime under the same lock the read path
    /// holds. Runtime disposal is the single unavailability authority —
    /// surviving connections observe `runtime-unavailable` afterwards, and a
    /// rebuild keeps it, also one after a fault in this fold. Idempotent.
    pub(crate) fn shutdown(&self) {
        let _ = self.fold(OnFault::Dispose, |state| state.runtime.dispose());
    }

    /// The one request executor every read entry shares. Order is
    /// load-bearing: the immutable policy gate runs BEFORE the provider,
    /// so a denied request never invokes preparation at all; an eligible
    /// request invokes it exactly once, outside any `AgentState` lock,
    /// and a provider panic is caught at this boundary and mapped to the
    /// correlated `RuntimeUnavailable` arm — never propagated into the
    /// session loop, never an `AgentReadFault`. `AssertUnwindSafe` is
    /// sound because the consumed provider holds no cell guard while it
    /// runs — any state it touches passes through the fold boundary, which
    /// rebuilds the cell after a fault — so a panic can only forfeit this
    /// request's evidence, not corrupt shared state.
    pub(crate) fn execute_prepared(
        &self,
        request: AgentReadRequest,
        prepare: impl FnOnce() -> Result<AgentRuntimeSnapshot, PreparedUnavailable>,
    ) -> Result<AgentReadResult, AgentReadFault> {
        if !self.policy.read {
            return Ok(match request {
                AgentReadRequest::List { request_id, .. } => AgentReadResult::List {
                    request_id,
                    result: AgentListOutcomeEnvelope {
                        outcome: AgentListOutcome::Rejected {
                            reason: AgentListRejectionReason::ReadDenied,
                        },
                    },
                },
                AgentReadRequest::Get { request_id, .. } => AgentReadResult::Get {
                    request_id,
                    result: AgentGetOutcomeEnvelope {
                        outcome: AgentGetOutcome::Rejected {
                            reason: AgentSessionRejectionReason::ReadDenied,
                        },
                    },
                },
                AgentReadRequest::Explain { request_id, .. } => AgentReadResult::Explain {
                    request_id,
                    result: AgentExplainOutcomeEnvelope {
                        outcome: AgentExplainOutcome::Rejected {
                            reason: AgentSessionRejectionReason::ReadDenied,
                        },
                    },
                },
            });
        }
        let prepared = std::panic::catch_unwind(std::panic::AssertUnwindSafe(prepare))
            .unwrap_or(Err(PreparedUnavailable));
        match request {
            AgentReadRequest::List {
                request_id,
                include_ended,
                limit,
                cursor,
            } => {
                let outcome = self.list(
                    prepared,
                    include_ended,
                    limit.value(),
                    cursor.as_ref().map(|cursor| cursor.as_str()),
                )?;
                Ok(AgentReadResult::List {
                    request_id,
                    result: AgentListOutcomeEnvelope { outcome },
                })
            }
            AgentReadRequest::Get {
                request_id,
                session_id,
            } => {
                let outcome = self.get(prepared, session_id.as_str())?;
                Ok(AgentReadResult::Get {
                    request_id,
                    result: AgentGetOutcomeEnvelope { outcome },
                })
            }
            AgentReadRequest::Explain {
                request_id,
                session_id,
            } => {
                let outcome = self.explain(prepared, session_id.as_str())?;
                Ok(AgentReadResult::Explain {
                    request_id,
                    result: AgentExplainOutcomeEnvelope { outcome },
                })
            }
        }
    }

    /// Test entry to the prepared-read executor: the supplied provider is invoked at
    /// most once and only after policy admission. There is no
    /// seam-absent fallback — a caller without a provider has no read.
    #[cfg(test)]
    pub(crate) fn execute_prepared_for_test(
        &self,
        request: AgentReadRequest,
        prepare: impl FnOnce() -> Result<AgentRuntimeSnapshot, PreparedUnavailable>,
    ) -> Result<AgentReadResult, AgentReadFault> {
        self.execute_prepared(request, prepare)
    }

    /// Clones one consistent canonical snapshot under the lock;
    /// `Ok(None)` only when the owning runtime is genuinely disposed and
    /// `Err` when this fold faulted. Reads map both to the existing
    /// `RuntimeUnavailable` wire arm.
    #[cfg(test)]
    fn snapshot(&self) -> Result<Option<AgentRuntimeSnapshot>, AgentFoldUnavailable> {
        self.with_state(|state| {
            if state.runtime.is_disposed() {
                None
            } else {
                Some(state.runtime.snapshot().clone())
            }
        })
    }

    fn list(
        &self,
        prepared: Result<AgentRuntimeSnapshot, PreparedUnavailable>,
        include_ended: bool,
        limit: u64,
        cursor: Option<&str>,
    ) -> Result<AgentListOutcome, AgentReadFault> {
        let Ok(snapshot) = prepared else {
            return Ok(AgentListOutcome::Unavailable {
                reason: AgentUnavailableReason::RuntimeUnavailable,
            });
        };
        // Canonical storage order is already UTF-16 session-id order.
        let filtered = snapshot
            .sessions
            .iter()
            .filter(|session| include_ended || session.lifecycle == AgentLifecycle::Live)
            .collect::<Vec<_>>();
        let position = match cursor {
            None => 0,
            Some(cursor) => match self.cursors.verify(
                cursor,
                include_ended,
                snapshot.revision,
                filtered.len() as u64,
            ) {
                Ok(position) => position as usize,
                Err(AgentCursorRejection::Invalid) => {
                    return Ok(AgentListOutcome::Rejected {
                        reason: AgentListRejectionReason::CursorInvalid,
                    });
                }
                Err(AgentCursorRejection::Stale) => {
                    return Ok(AgentListOutcome::Rejected {
                        reason: AgentListRejectionReason::CursorStale,
                    });
                }
            },
        };
        let remaining = filtered.len() - position;
        let take = (limit as usize).min(remaining);
        let end = position + take;
        let mut items = Vec::with_capacity(take);
        for session in &filtered[position..end] {
            items.push(map::session_summary(session)?);
        }
        let next_cursor = (end < filtered.len()).then(|| {
            self.cursors
                .mint(include_ended, snapshot.revision, end as u64)
        });
        Ok(AgentListOutcome::Listed {
            items,
            next_cursor,
            revision: map::revision(snapshot.revision)?,
        })
    }

    fn get(
        &self,
        prepared: Result<AgentRuntimeSnapshot, PreparedUnavailable>,
        session_id: &str,
    ) -> Result<AgentGetOutcome, AgentReadFault> {
        let Ok(snapshot) = prepared else {
            return Ok(AgentGetOutcome::Unavailable {
                reason: AgentUnavailableReason::RuntimeUnavailable,
            });
        };
        let Some(session) = snapshot
            .sessions
            .iter()
            .find(|session| session.session_id.as_str() == session_id)
        else {
            return Ok(AgentGetOutcome::Rejected {
                reason: AgentSessionRejectionReason::NotFound,
            });
        };
        Ok(AgentGetOutcome::Found {
            revision: map::revision(snapshot.revision)?,
            session: Box::new(map::session_detail(session)?),
        })
    }

    fn explain(
        &self,
        prepared: Result<AgentRuntimeSnapshot, PreparedUnavailable>,
        session_id: &str,
    ) -> Result<AgentExplainOutcome, AgentReadFault> {
        let Ok(snapshot) = prepared else {
            return Ok(AgentExplainOutcome::Unavailable {
                reason: AgentUnavailableReason::RuntimeUnavailable,
            });
        };
        let Some(session) = snapshot
            .sessions
            .iter()
            .find(|session| session.session_id.as_str() == session_id)
        else {
            return Ok(AgentExplainOutcome::Rejected {
                reason: AgentSessionRejectionReason::NotFound,
            });
        };
        Ok(AgentExplainOutcome::Found {
            explanation: map::session_explanation(session)?,
            revision: map::revision(snapshot.revision)?,
            session: Box::new(map::session_detail(session)?),
        })
    }

    /// Test-only narrow wrapper over the production constructor: the
    /// translator is configured with a fixture socket path so
    /// `environment_overlay_for` both registers the generation and emits
    /// the overlay, exactly like the corpus driver.
    #[cfg(test)]
    pub(crate) fn open_hook_fixture(policy: AgentPolicy, socket_path: &str) -> io::Result<Self> {
        Self::open_observing(policy, Some(socket_path.to_owned()))
    }

    /// Test-only provider-ingress substitute: drives one real canonical
    /// observation event through the runtime under the lock. There is no
    /// production seed flag, environment variable, or wire operation.
    #[cfg(test)]
    pub(crate) fn seed(
        &self,
        event: &crate::agent::AgentObservationEvent,
    ) -> Result<crate::agent::AgentTransition, AgentFoldUnavailable> {
        self.with_state(|state| state.runtime.dispatch(event))
    }

    /// Test-only envelope admission seam driving the translator through
    /// the shared cell; the production dispatch path is the managed
    /// endpoint fold, not this bridge. Only conformance/service tests
    /// drive it. Every
    /// dispatch lands in the same `state.runtime` the read path clones,
    /// inside the one leaf-mutex section — no second runtime exists.
    /// `Ok(false)` is the translator's genuine rejected-payload answer;
    /// `Err` is fold unavailability — never conflated.
    #[cfg(test)]
    pub(crate) fn admit_hook_envelope(
        &self,
        body: &[u8],
        now: f64,
    ) -> Result<bool, AgentFoldUnavailable> {
        self.with_state(|state| {
            let AgentState {
                runtime, ingress, ..
            } = &mut *state;
            ingress.admit_envelope(runtime, body, now).is_some()
        })
    }

    /// Test-only bridge seam for the application-signal hint path;
    /// production signals enter via `observe_terminal_application_signal`
    /// inside the drain fold.
    #[cfg(test)]
    pub(crate) fn observe_hook_signal(
        &self,
        payload: &str,
        creation_id: String,
        presentation_id: String,
        now: f64,
    ) -> Result<(), AgentFoldUnavailable> {
        self.with_state(|state| {
            let AgentState {
                runtime, ingress, ..
            } = &mut *state;
            ingress.observe_terminal_application_signal(
                runtime,
                payload,
                creation_id,
                presentation_id,
                now,
            );
        })
    }

    /// Test-only bridge seam for overlay registration; the production
    /// socket overlay is generated worker-side, not by this seam.
    /// `Ok(vec![])` means no socket bound; `Err` is fold
    /// unavailability — an empty overlay is never a false-healthy signal.
    #[cfg(test)]
    pub(crate) fn hook_environment_overlay(
        &self,
        presentation_id: String,
        creation_id: String,
    ) -> Result<Vec<(String, String)>, AgentFoldUnavailable> {
        self.with_state(|state| {
            let AgentState {
                runtime, ingress, ..
            } = &mut *state;
            ingress.environment_overlay_for(runtime, presentation_id, creation_id)
        })
    }

    /// Test-only bridge seam for presentation retirement; production
    /// retirement enters via the native carrier lifecycle folds.
    #[cfg(test)]
    pub(crate) fn retire_hook_presentation(
        &self,
        presentation_id: &str,
    ) -> Result<(), AgentFoldUnavailable> {
        self.with_state(|state| {
            let AgentState {
                runtime, ingress, ..
            } = &mut *state;
            ingress.retire_presentation(runtime, presentation_id);
        })
    }

    /// Test-only settlement timer pass; production settlement syncs at
    /// fold entry inside the endpoint fold, the corpus scheduler stays a
    /// test fixture.
    #[cfg(test)]
    pub(crate) fn advance_hook_settlement(
        &self,
        milliseconds: f64,
        now: f64,
    ) -> Result<(), AgentFoldUnavailable> {
        self.with_state(|state| {
            let AgentState {
                runtime, ingress, ..
            } = &mut *state;
            ingress.advance_settlement(runtime, milliseconds, now);
        })
    }

    /// Converts the caller's monotonic instant into translator-time
    /// milliseconds since the service-owned `clock_origin`; an earlier
    /// instant saturates to 0.0 rather than faulting the fold.
    fn instant_to_now_ms(state: &AgentState, now: Instant) -> f64 {
        now.checked_duration_since(state.clock_origin)
            .map_or(0.0, |elapsed| elapsed.as_secs_f64() * 1000.0)
    }

    /// The managed observation endpoint seam — one Instant-domain fold:
    /// the disposal check runs FIRST
    /// (an admitted-but-undispatchable envelope must never be ACK'd;
    /// dispatch on a disposed runtime returns `Rejected{RuntimeDisposed}`
    /// while JSM admission would still succeed), then the settlement
    /// scheduler syncs to the call instant at fold entry — structural
    /// coverage for every scheduling producer inside the fold — then the
    /// translator admission decides the ACK: `Ok(Some)` names the accepted
    /// envelope's generation, and `Ok(None)` covers both a rejection and a
    /// clean-disposed runtime; only a fold fault surfaces `Err`.
    pub(crate) fn admit_envelope(
        &self,
        body: &[u8],
        now: Instant,
    ) -> Result<Option<HookGeneration>, AgentFoldUnavailable> {
        self.with_state(|state| {
            if state.runtime.is_disposed() {
                return None;
            }
            let now_ms = Self::instant_to_now_ms(state, now);
            let AgentState {
                runtime, ingress, ..
            } = &mut *state;
            ingress.sync_settlement_clock(now_ms);
            ingress.admit_envelope(runtime, body, now_ms)
        })
    }

    /// The endpoint-owned clock drives settle/expiry — sync the scheduler
    /// to the call instant, then fire every due timer through
    /// `join.settle`.
    pub(crate) fn tick(&self, now: Instant) -> Result<(), AgentFoldUnavailable> {
        self.with_state(|state| {
            state.reconcile_launch_names(now);
            let now_ms = Self::instant_to_now_ms(state, now);
            let AgentState {
                runtime, ingress, ..
            } = &mut *state;
            ingress.tick(runtime, now_ms);
        })
    }

    /// The earliest pending settlement
    /// mapped back into the service's own `Instant` domain — the
    /// endpoint's poll-deadline source. `None` while no timer producer
    /// has scheduled anything; `Err` only on fold unavailability.
    pub(crate) fn next_deadline(&self) -> Result<Option<Instant>, AgentFoldUnavailable> {
        self.with_state(|state| {
            let observation = state.ingress.next_settlement_at().and_then(|fire_at| {
                state
                    .clock_origin
                    .checked_add(Duration::from_secs_f64(fire_at / 1000.0))
            });
            let launch = state
                .names
                .next_launch_deadline_ms()
                .and_then(|milliseconds| {
                    state
                        .clock_origin
                        .checked_add(Duration::from_millis(milliseconds))
                });
            observation.into_iter().chain(launch).min()
        })
    }

    /// Test-only fault injector: dispatches one real canonical event
    /// through the fold boundary and then panics, so the mutation made
    /// before the fault must never be observable.
    #[cfg(test)]
    pub(crate) fn fault_after_dispatch_for_test(
        &self,
        event: &crate::agent::AgentObservationEvent,
    ) -> Result<(), AgentFoldUnavailable> {
        self.with_state(|state| {
            state.runtime.dispatch(event);
            panic!("injected fold fault");
        })
    }

    /// The native leaf's registry admission fact (
    /// Worker-connection accept consumer). Inner `Err` is the leaf's
    /// contract refusal (a carrier is already active); outer `Err` is
    /// fold unavailability — the two axes stay distinct.
    pub(crate) fn native_admit_carrier(
        &self,
        carrier: CarrierIdentity,
        now: Instant,
    ) -> Result<Result<(), AgentModelError>, AgentFoldUnavailable> {
        self.with_state(|state| state.native.admit_carrier(carrier, now))
    }

    /// The native leaf's registry retirement fact (
    /// Worker-connection close consumer). Owned `RetirePresentation`
    /// effects are applied to the join inside the same fold, so a join
    /// fence can never remain live after its carrier is gone — nor after
    /// a fault in this fold, whose rebuild forgets the carrier.
    pub(crate) fn native_retire_carrier(
        &self,
        carrier: CarrierIdentity,
        now: Instant,
    ) -> Result<(), AgentFoldUnavailable> {
        self.fold(OnFault::Forget(carrier), |state| {
            let effects = state.native.retire_carrier(&carrier, now);
            state.apply_native_effects(effects, now);
        })
    }

    /// Test-only fault injector for the retirement fold: the leaf retires
    /// `carrier` and its effects apply, then the fold panics.
    #[cfg(test)]
    pub(crate) fn native_retire_with_fault_for_test(
        &self,
        carrier: CarrierIdentity,
        now: Instant,
    ) -> Result<(), AgentFoldUnavailable> {
        self.fold(OnFault::Forget(carrier), |state| {
            let effects = state.native.retire_carrier(&carrier, now);
            state.apply_native_effects(effects, now);
            panic!("injected fold fault");
        })
    }

    /// Test-only whole-snapshot seam; production reconciliation runs
    /// inside `native_fenced_final_snapshot`. Inner `Err`
    /// rejects an ambiguous or oversized snapshot atomically — the leaf
    /// bindings and the translator are left untouched; outer `Err` is
    /// fold unavailability.
    #[cfg(test)]
    pub(crate) fn native_reconcile_bindings(
        &self,
        snapshot: Vec<Binding>,
        now: Instant,
    ) -> Result<Result<(), AgentModelError>, AgentFoldUnavailable> {
        self.with_state(|state| {
            let effects = state.native.reconcile_complete_bindings(&snapshot, now)?;
            state.apply_native_effects(effects, now);
            Ok(())
        })
    }

    /// Test-only read-preparation fold — the same shape the production
    /// `LocalTerminalService` prepared-read provider exercises while holding its outer
    /// registry/coordinator guards for the whole duration. ONE
    /// `with_state` section covers all of it: runtime-disposal check,
    /// whole-facts reconcile, effect consumption, and the canonical
    /// snapshot clone. Never composed as `native_reconcile_bindings()`
    /// + `snapshot()` — two folds would reopen the mutation-interleave
    ///   window where a stale reconcile could regress bindings and still
    ///   be returned as fresh. `Err` is this fold's fault;
    ///   `Ok(Err(..))` is the leaf's atomic whole-snapshot rejection
    ///   (bindings and translator untouched); `Ok(Ok(None))` is a
    ///   disposed runtime; `Ok(Ok(Some))` is the post-reconcile truth.
    #[cfg(test)]
    pub(crate) fn native_reconcile_and_snapshot(
        &self,
        snapshot: Vec<Binding>,
        now: Instant,
    ) -> Result<Result<Option<AgentRuntimeSnapshot>, AgentModelError>, AgentFoldUnavailable> {
        self.with_state(|state| {
            if state.runtime.is_disposed() {
                return Ok(None);
            }
            let effects = state.native.reconcile_complete_bindings(&snapshot, now)?;
            state.apply_native_effects(effects, now);
            Ok(Some(state.runtime.snapshot().clone()))
        })
    }

    /// Test-only explicit expiry sweep; every production entry point
    /// already sweeps at its head inside the fold.
    #[cfg(test)]
    pub(crate) fn native_sweep(&self, now: Instant) -> Result<(), AgentFoldUnavailable> {
        self.with_state(|state| state.native.sweep_expired(now))
    }

    /// Test-only observability of the leaf's held count.
    #[cfg(test)]
    pub(crate) fn native_held_count(&self) -> Result<usize, AgentFoldUnavailable> {
        self.with_state(|state| state.native.held_count())
    }

    /// The drain page fold — applies one ENTIRE validated reply page
    /// under a full-identity fence inside a single `with_state` section.
    /// The fence is checked before ANY entry applies; every returned
    /// effect is consumed inside the same guard; the live `W` is then
    /// read from the carrier tuple — an empty or fully-deduped page
    /// still returns it. `CarrierMismatch` applies nothing: retirement
    /// or replacement needs this same guard and can only run before or
    /// after the fold, never inside it. The inner `Err` refuses a page
    /// whose domain facts cannot be built — before any fold, with zero
    /// mutation; the outer `Err` is this fold's fault, after which the
    /// rebuilt leaf treats the page as a gap rather than read it again.
    pub(crate) fn native_apply_drain_page(
        &self,
        req_carrier: CarrierIdentity,
        reply: &AgentDrainReply,
        now: Instant,
    ) -> Result<Result<DrainPageOutcome, AgentModelError>, AgentFoldUnavailable> {
        let facts = match drain_facts(req_carrier, reply) {
            Ok(facts) => facts,
            Err(error) => return Ok(Err(error)),
        };
        let on_fault = page_range(reply).map_or(OnFault::Keep, |(low, high)| OnFault::Skip {
            carrier: req_carrier,
            low,
            high,
        });
        self.fold(on_fault, |state| {
            let Some((current, _w)) = state.native.current_carrier() else {
                return Ok(DrainPageOutcome::CarrierMismatch);
            };
            if current != req_carrier {
                return Ok(DrainPageOutcome::CarrierMismatch);
            }
            let mut effects = Vec::new();
            for fact in facts {
                let transition = match fact {
                    DrainFact::Event(event) => state.native.apply_event(event, now),
                    DrainFact::Gap { lo, hi } => {
                        state.native.apply_gap(&req_carrier, lo.get(), hi, now)
                    }
                };
                effects.extend(transition.effects);
            }
            state.apply_native_effects(effects, now);
            Ok(match state.native.current_carrier() {
                Some((current, w)) if current == req_carrier => DrainPageOutcome::Applied { w },
                // Unreachable inside one guard — nothing in this fold can
                // retire or replace the carrier; kept as a defensive
                // fence value rather than an unwrap.
                _ => DrainPageOutcome::CarrierMismatch,
            })
        })
    }

    /// The fenced final snapshot — ONE `with_state` fold covering, in
    /// this cause order: the live `(identity, W)` fence against the
    /// pinned barrier (or leaf-absence for a `NoCarrier` expectation),
    /// the runtime-disposal check, the atomic whole-facts reconcile,
    /// effect consumption, and the canonical clone. The fold re-reads
    /// the live carrier tuple, so a stale local watermark can never
    /// justify a different carrier's snapshot.
    pub(crate) fn native_fenced_final_snapshot(
        &self,
        expected: DrainExpectation,
        snapshot: Vec<Binding>,
        now: Instant,
    ) -> Result<Result<Option<AgentRuntimeSnapshot>, FinalFence>, AgentFoldUnavailable> {
        self.with_state(|state| {
            match expected {
                DrainExpectation::Carrier {
                    req_carrier,
                    barrier,
                } => match state.native.current_carrier() {
                    Some((current, w)) if current == req_carrier => {
                        if w < barrier {
                            return Err(FinalFence::BarrierUnmet { w, barrier });
                        }
                    }
                    _ => return Err(FinalFence::CarrierMismatch),
                },
                DrainExpectation::NoCarrier => {
                    if state.native.current_carrier().is_some() {
                        return Err(FinalFence::CarrierAppeared);
                    }
                }
            }
            if state.runtime.is_disposed() {
                return Ok(None);
            }
            let effects = state
                .native
                .reconcile_complete_bindings(&snapshot, now)
                .map_err(FinalFence::Rejected)?;
            state.apply_native_effects(effects, now);
            Ok(Some(state.runtime.snapshot().clone()))
        })
    }
}
