# Canonical Agent observation model

This document owns the Server-side canonical Agent observation model under `src/agent/`.
It records the accepted scope, the semantic claims the port must hold, and the boundary
mapping between the authoritative TypeScript model and its Rust mirror.

## Status and activation boundary

`apps/orchbee-server` owns a faithful Rust implementation of the deterministic semantics
currently defined by `packages/agent` at base commit
`0d4b685a1b7db9405ca94908385e0534ddd41dc9`. The module is registered from `main.rs` under
`#[cfg(any(unix, windows))]`: the canonical runtime lives inside the **Server binary crate**, never in
the library (`lib.rs` exports nothing Agent-related and stays untouched), and no public
API, new crate, or second canonical state exists.

The production surface now covers the managed local observation path end to end:

- `model` value types and `AgentRuntimeSnapshot` — the typed canonical state; the raw
  presence-aware construction/validation layer is the production admission path
  (envelopes parse to typed snapshots; reads mirror them back);
- `AgentRuntime` — empty-snapshot construction, `snapshot`, `dispose`, `is_disposed`,
  and production `dispatch` through the canonical reducer. The binary-private
  `agent_service` owns one `Mutex<AgentState>` read/write cell, clones one consistent
  bounded snapshot per request under the lock, and releases the lock before projection
  encoding or socket writes. `AgentState` holds the single `AgentRuntime` plus the
  hook translator and the native observation leaf in the same cell; every
  runtime-touching translator method takes `&mut AgentRuntime`, so reads, hook
  dispatches, and native applications share exactly one canonical store;
- `join`, `hook_contract`, and `hook_ingress` — the ported
  `AgentObservationJoinStateMachine` and `AgentHookObservationIngress` translator,
  production-active for managed endpoint envelope admission, terminal application
  signals, presentation registration/retirement, and settlement advancement;
- `native_observation` — the private native reducer leaf, production-active through
  Worker carrier admission/retirement and the drain page/final-fence service seams;
- `project_agent_session`, `project_agent_session_summary`, and `explain_agent_session` —
  the status/summary/detail/explanation projections consumed by the OBAP
  `list`/`get`/`explain` operations;
- `local_site_service/agent_observation_endpoint.rs` — the managed private Unix
  endpoint bound before Site readiness;
- `local_terminal_service/agent_drain_read.rs` — the production drain-read pump wired
  as the `execute_prepared` provider.

Test-only surfaces stay `#[cfg(test)]`: the seed dispatch path (`AgentService::seed`),
the corpus/JSON emitters and `FixtureIngress`, the `DispatchTrace` recorder, the
f64-domain corpus settlement advance, the change-read surface
(`read_agent_runtime_changes`, `AgentRuntime::read_changes`), the test-only service
bridge seams (`admit_hook_envelope`, `observe_hook_signal`, `hook_environment_overlay`,
`retire_hook_presentation`, `advance_hook_settlement`), the test-only per-event native
seams (`native_apply_event`, `native_apply_gap`, `native_reconcile_bindings`,
`native_reconcile_and_snapshot`, `native_sweep`, `native_held_count`), the test-only
`environment_overlay_for` overlay seam (the production overlay is generated
Worker-side from the validated site runtime), aggregate/location/wait projections,
and all conformance harnesses.
Production builds contain no
seeding flag, environment variable, wire replay, or second state path.

The initial runtime is genuinely empty: revision `0`, an empty session list, `get`/`explain`
report `not-found`, and `runtime-unavailable` is observed only after the owning listener
stops serving and calls `AgentService::shutdown`, which disposes the canonical runtime
under the same lock — surviving admitted connections then see real unavailability.

Every `AgentState` access runs through one private `with_state` failure boundary: the
mutex guard is created and dropped inside `catch_unwind`, so a panicked fold permanently
poisons the cell and surfaces as `AgentFoldUnavailable` without crossing the caller's
outer guards. The cell is never unpoisoned — no `into_inner` or `clear_poison` exists —
a partial mutation is never observable, and `shutdown` stays an idempotent no-op on a
poisoned cell. Reads map both disposal and poison to the existing `RuntimeUnavailable`
wire arm; the write seams return `Result<_, AgentFoldUnavailable>` so a fold
failure is never confused with a rejected payload or an empty overlay.

## Layout

| Path                               | Responsibility                                                                                                                                                                                                                                                                      |
| ---------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/agent/mod.rs`                 | Module declarations and the `pub(crate)` production re-exports; corpus/test-only helpers stay gated inside each module                                                                                                                                                              |
| `src/agent/model.rs`               | Canonical value types, identity validation, UTF-16 ordering, snapshot invariants; raw construction and validation are the production admission path                                                                                                                                 |
| `src/agent/events.rs`              | The fourteen observation event kinds and the four semantic event kinds; `#[cfg(test)]` `event_id` provenance accessor                                                                                                                                                               |
| `src/agent/reducer.rs`             | `transition_agent_runtime` per-kind guards, unchanged/rejected/applied, semantic event order; `#[cfg(test)]` transition accessors                                                                                                                                                   |
| `src/agent/changes.rs`             | Change records, production append with retention 512 and contiguity; `#[cfg(test)]` `read_agent_runtime_changes` paging kinds                                                                                                                                                       |
| `src/agent/runtime.rs`             | `AgentRuntime` — production: empty-snapshot construction, `snapshot`, `dispose`, `is_disposed`, `dispatch`; `#[cfg(test)]`: seed and change reads                                                                                                                                   |
| `src/agent/projection.rs`          | Semantic status, summary, and explanation projections; `#[cfg(test)]` aggregate/location presentations and wait-condition matching                                                                                                                                                  |
| `src/agent/corpus.rs`              | `#[cfg(test)]` conformance corpus runner and private fixture decoder                                                                                                                                                                                                                |
| `src/agent/targeted.rs`            | `#[cfg(test)]` named boundary regression tests                                                                                                                                                                                                                                      |
| `src/agent/oracle.rs`              | `#[cfg(test)]` named regression tests against recorded TypeScript-oracle outcomes                                                                                                                                                                                                   |
| `src/agent/join.rs`                | Pure port of the Desktop `AgentObservationJoinStateMachine`: generation registration/retirement, hint/event join, TTL cleanup, settlement effects, admission bounds; `#[cfg(test)]` `reset`                                                                                         |
| `src/agent/hook_contract.rs`       | Strict-envelope/correlation-hint boundary parse and the ten-adapter event-gate registry (ports `agent-hook-observation-contract.ts` and `resolveAgentHookAdapter.ts`)                                                                                                               |
| `src/agent/hook_ingress.rs`        | Pure port of `AgentHookObservationIngress`, runtime-free after the bridge split: session bindings, turn/request/child maps, join-command consumption, canonical dispatch in order through the injected `&mut AgentRuntime`; `#[cfg(test)]` corpus/trace/f64-settlement/JSON helpers |
| `src/agent/conformance_support.rs` | `SettlementQueue` timing primitives shared with the production `Instant` funnel; `#[cfg(test)]` fixture-shape JSON emitters, `DispatchTrace`, corpus clock advance, and `FixtureIngress`                                                                                            |
| `src/agent/hook_conformance.rs`    | `#[cfg(test)]` — replay harnesses for the frozen join and hook corpora plus the native joint-capacity checks                                                                                                                                                                        |

## Read service and transport integration

The first-read is served by the binary-private `agent_service`, integrated into the
existing admitted local-client path — no new socket, listener, thread, or unsolicited
frame:

| Path                                                      | Responsibility                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| --------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/agent_service.rs`                                    | `AgentPolicy`, `AgentService` (one `Mutex<AgentState>` cell + boot-scoped cursor authority), the shared prepared-read executor `execute_prepared` for list/get/explain, `open_observing` construction-time socket binding, `shutdown`, `AgentReadFault` loud-failure type, `PreparedUnavailable` provider-decline marker, the production `Instant` clock seams (`admit_envelope`, `tick`, `next_deadline`), the production native carrier seams (`native_admit_carrier`, `native_retire_carrier`) and drain/fence seams (`native_apply_drain_page`, `native_fenced_final_snapshot`), `#[cfg(test)]` `seed`, test-only bridge seams (`admit_hook_envelope`, `observe_hook_signal`, `hook_environment_overlay`, `retire_hook_presentation`, `advance_hook_settlement`), and test-only native seams (`native_apply_event`, `native_apply_gap`, `native_reconcile_bindings`, `native_reconcile_and_snapshot`, `native_sweep`, `native_held_count`, `native_poison_after_mutation_for_test`) |
| `src/agent_service/map.rs`                                | Explicit typed canonical-to-wire mapper; every unrepresentable canonical value fails loudly rather than being filtered or fabricated                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `src/agent_service/cursor.rs`                             | Private boot-scoped HMAC list-cursor authority: mint, canonical base64url shape, MAC/shape/semantic verification order                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `src/agent_service/tests.rs` + `cursor/tests.rs`          | Policy, ordering, pagination, cursor invalid/stale/boot-scope, ended filtering, disposal, and mapping-fidelity coverage                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `local_client_service.rs`                                 | One `Arc<AgentService>` beside the workbench service; the `AgentRuntimeOwner` guard disposes the runtime when the listener loop ends                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `local_client_service/application_session.rs`             | The third `OBAP` demultiplex arm inside the same sequential record loop — read, execute, write exactly one correlated result                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| `local_client_service/agent_harness.rs` (`#[cfg(test)]`)  | Reusable seeded-service transport harness: real event-dispatch seeds, `admitted` socket-pair sessions, and `listen` real-listener fixtures                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| `local_client_service/tests/agent.rs`                     | Real listener-socket and admitted-pair OBAP evidence: seeded reads, empty state, owner-drop unavailability, malformed-record termination                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| `local_client_service/application_session/tests/agent.rs` | OBAP demux coverage: routing, interleaved OBCS ordering, malformed records, executor faults, result write failures                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| `local_terminal_service/agent_drain_read.rs`              | The production Agent drain read pump — the `execute_prepared` provider invoked once per read: typed carrier tri-state (`NoCarrier` is the only true-none read), one in-flight exchange under the private four-second window minted once before preparation locking, 25 ms backpressure-only admission retry, one whole-page fold, the barrier pinned once from the first carrier-matched reply, and the fenced final snapshot taken while the registry -> coordinator guards stay held in that order                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `local_terminal_service/agent_drain_read/tests.rs`        | Real-wire composition evidence: the canonical hint + rich observation through the real registry/`UnixStream`/coordinator projection, `NoCarrier`/pending/`Unavailable` distinction, carrier-rotation and page fences, deadline refusal, poison/disposal, and retained-snapshot `NoCarrier` reads                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `local_site_service/agent_observation_endpoint.rs`        | The managed observation endpoint — private Unix-socket transport for the hook ingress, bound inside the Site runtime directory before readiness: one bound path per site, one nonblocking listener plus a stop pipe on a single poll thread, bounded accept draining, the per-connection read/admission/ACK machine under one shared 500 ms deadline, and identity-gated unlink on drop                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `local_site_service/agent_observation_endpoint/tests.rs`  | Real-socket endpoint evidence: immediate vs settled application, unmatched/zero-correlation refusals, deadline sweep, connection cap and interleaved connections, poisoned/disposed cells, scripted writer seams, identity mismatch, live-connection drop reaping, and the bounded accept batch                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |

Read projection never mutates canonical state: every request runs through one shared
executor that applies the immutable policy gate FIRST, then invokes the request's
required snapshot provider exactly once — outside any `AgentState` lock and under a
panic catch — releases every guard, then projects and maps. `execute_prepared` is the
production entry: `local_terminal_service/agent_drain_read.rs` is the caller-owned
provider, and it holds its outer registry -> coordinator guards through the provider's
own final fold. The legacy `execute` test seam keeps the raw snapshot clone for
fixtures — preserved verbatim and not a freshness proof.
A declined or panicked provider maps to the
requesting operation's correlated `runtime-unavailable` arm — never an
`AgentReadFault`, never a fabricated or last-good snapshot. `list` preserves canonical
UTF-16 storage order, honors bounded `limit` and `include_ended`, and mints/verifies
opaque boot-scoped cursors (`cursor-invalid`/`cursor-stale`); `get`/`explain` answer
`not-found` for absent sessions; every operation reports `read-denied` under a denied
policy and `runtime-unavailable` after disposal.

Boundary note: canonical carried payloads whose only readers are the gated surfaces
or absent remote consumers stay verbatim — removing or narrowing them would break
the accepted TypeScript-parity model. Per the docs/RUST.md lint policy they carry
item-scoped `#[cfg_attr(not(test), expect(dead_code, reason = "..."))]` exceptions —
each names the canonical invariant and the absent production consumer, and `expect`
self-falsifies the moment a real reader lands: corpus-only event constructors
(`LimitedActivityObserved`, `ObservationBecameStale`, `ObservationReconnected`,
`RawPresence::Other`), event `event_id` fields consumed only by the gated trace/read
paths, `AgentSemanticEvent` payloads, `AgentTransition` payloads,
`NativeTransition`'s `class`/`ack_w` (read only by the test-only per-event service
seams), the fold outcomes' diagnostic fields (`FinalFence::BarrierUnmet`/`Rejected`),
the change-feed projection fields (`AgentRuntimeChange`/`AgentRuntimeChangeSession`),
the ingress `socket_path` kept for the test-only overlay seam,
`RegistrationFact::binding`, and the drain exchange's `Lost` payload. The
`agent/mod.rs` re-export block for bin-only consumers uses a narrow
`#[cfg_attr(test, allow(unused_imports))]` instead — `expect` would be unfulfilled in
the bin-test target where `agent_service::map` does consume them. No module-wide
suppression, no fake use, no const tables, no variant narrowing.

## Session names sidecar

The OBAP names family is implemented by `agent_service/names.rs` in the same
`AgentState` cell. Two private ordered maps hold name → session and session → name;
names are runtime-scoped, absent from canonical session documents, and never persisted.
Names use `[a-z][a-z0-9_-]{0,31}`. There is at most one binding per name and per
session, bounded at 512. New assignments enforce the bound; an atomic rename at
capacity still succeeds. Collision refuses with the incumbent session identity.

Every `with_state` fold reconciles the maps before releasing the leaf mutex, removing
bindings to absent or ended sessions. Disposal clears both maps. Poison and disposal
answer `runtime-unavailable`, never an empty success. Exact session selectors accept
retained identities of any lifecycle; assignment additionally requires live state.
Name selectors use only the alias table. Auto selectors test exact session existence
first, including ended history, then names; explicit name selection escapes collisions.
Release of an existing unnamed session is unchanged; a missing explicit name is refused.

The read gate owns resolve/list; the independent manage gate owns assign/release.
The gate precedes observation preparation. The caller's existing drain provider runs
once outside the cell; after it succeeds, one fold rechecks the current runtime and
resolves/mutates atomically. The prepared clone is freshness evidence, never a stale
liveness authority. No socket I/O or outer lock is acquired inside the leaf fold.
The sequential application-session demux routes both OBAP families and writes exactly
one correlated result. A lost mutation reply is outcome-unknown, without a replay ledger.

`schemas/agent-observation/v1` covers kinds 1–14. Native `agent names`, `agent resolve`,
and `agent rename` consume the names family; existing get/explain/wait still accept
session IDs. Their direct name-target follow-up remains in
[`proposals/agent/session-names.md`](../../../proposals/agent/session-names.md).
Deterministic service tests cover collision, atomic rename, exact-ID precedence,
full capacity, lifecycle cleanup, policy separation, stale preparation, disposal,
poison, and concurrent claims. Wire and Windows process tests cover bounds,
schema-conforming replies, malformed operations, and ambiguous mutation outcomes.

## Authoritative semantics (claims)

The claims below mirror `packages/agent` source; the TypeScript source remains the
authority. Each claim is exercised by the durable corpus and/or the named boundary tests.

- Applied transitions advance `revision` by one; `unchanged` and `rejected` results retain
  the input snapshot and emit no semantic events. Applied transitions are the only journal
  appends.
- Non-`session-observed` events are rejected `invalid-event` before lookup when
  `generation`/`sequence` is not a nonnegative safe integer; then `session-not-found`;
  then `generation-mismatch` (unless `observation-reconnected` with a strictly greater
  generation); then `unchanged` when `sequence` is not greater than the stored sequence.
- `session-observed` folds numeric/source/fidelity/capability checks into
  `source-capability-exceeded` **before** the identity check; an identical re-observe
  (provider, execution location, attachment set) is `unchanged`, a conflicting one is
  `identity-conflict`. Duplicate capability tokens are deduplicated by the reducer before
  validation, so a duplicated-but-valid capability list applies; an unknown token fails
  snapshot validation as `invalid-event`.
- Turn lifecycle (`turn-started`, `turn-activity-changed`, `turn-finished`,
  `session-idle-observed`), attention requests, and `session-ended` are gated by declared
  capabilities and `live` lifecycle; `turn-not-found`/`request-not-found`/`request-conflict`
  follow the authoritative guard order. `session-idle-observed` clears turn-scoped pending
  requests for the interrupted turn only.
- `observation-became-stale` on `disconnected` sessions is `unchanged`; every other applied
  event restores `freshness: fresh` except `presentation-detached`, which preserves the
  stored freshness while another navigable attachment remains and forces `disconnected`
  when the last navigable attachment is detached. `presentation-visibility-changed` has
  no lifecycle guard and still applies on ended sessions.
- `session-ended` emits `turn-outcome{interrupted}` then `session-ended` when the turn is
  active; a heuristic-only presence (`heuristic` source + `heuristic` fidelity + zero
  capabilities) retires with zero semantic events and forced `disconnected` freshness.
- `observation-reconnected` requires a strictly greater generation to enter reconnect
  handling; a sequence not greater than the stored one is then `unchanged`. Its adopted
  claims are validated against declared capabilities and the runtime invariants
  (`capability-not-declared` then `invalid-event`).
- Bounds: 512 sessions, 128 pending requests, 32 presentation attachments, 10 capabilities,
  change page limit 1..=100, retention 512 changes, revisions capped at `MAX_SAFE_INTEGER`
  (`capacity-exceeded` at the cap).
- Ordering is by UTF-16 code unit, not Unicode scalar or byte order — including
  `presentationId` sort keys, which may hold well-formed non-ASCII text.
- Semantic status precedence: `needs-attention` (pending requests) → last outcome
  (`failed`/`interrupted`/`canceled`/`succeeded`) → `working` (active turn) → `idle` →
  `unknown` (with `activityHint`) → `ended`. Aggregate and location projections sort,
  prefer live sessions over ended, and rank by status priority, freshness, activity-hint
  presence, and descending generation on ties.
- Wait matching: `fresh`/`stale`/`disconnected` conditions match freshness directly;
  status conditions additionally require `fresh` freshness.
- Journal reads return `changes`/`current`/`reset-required`/`revision-ahead` with
  contiguous pages, `observedThroughRevision` equal to the last page revision, and
  `hasMore` iff the cursor has not reached the head. A deep-stale cursor yields
  `reset-required` with `observedThroughRevision` jumped to the head; `afterRevision`
  beyond the runtime revision yields `revision-ahead`. Invalid read input is a typed
  `Err` at the private boundary, not a transition result.
- `dispose` is idempotent; afterwards every dispatch is `runtime-disposed` without
  touching state or journal.

## TypeScript-to-Rust boundary mapping

- Event fields mirror the schema-less TypeScript union: raw strings and `f64` numbers
  enter the reducer, so guards stay reachable in the same order. Stored snapshot fields
  are validated typed values (`u64` for safe-integer numerics, enums, identity newtypes).
- `presentationId` keeps the 1..=256 bound counted in UTF-16 code units. Rust `String`
  cannot hold lone surrogates, so the well-formedness check is structurally discharged;
  the unit count is still enforced.
- Session/runtime construction sites map to the TypeScript parse points: the
  `session-observed` and `observation-reconnected` candidate checks yield `invalid-event`,
  while violations that would only surface inside the TypeScript `applied()` schema parse
  panic, mirroring the thrown error (e.g. `presentation-attached` carrying an oversize
  `presentationId`). This is parity of reachable behavior, not new public policy.
- Journal input validation returns `Result<_, AgentChangeReadError>`; the TypeScript
  `ZodError` throw on malformed private input maps to that typed error.
- Strict-object presence is preserved at the private raw boundary: `RawPresence`
  distinguishes `Absent`, `Null`, `Value`, and malformed `Other`, so required-nullable
  keys (active-turn `activity`, attention-request `turnId`, `activityHint`,
  `lastOutcome`, `parentSessionId`) keep the TypeScript distinction between a missing
  key and an explicit `null`, and `unknown`/`idle` turns reject any residue key —
  including keys whose value is `null`. Fields that are required and non-nullable stay
  bare in the typed domain; representing their absence is the private fixture decoder's
  responsibility, and any future public ingress is a separate layer with its own
  malformed-input policy.
- Determinism: no ambient time, randomness, IO, or global state exists anywhere in the
  module; freshness transitions are explicit event inputs.

## Pure ingress port (join + hook translator)

Behind the same binary-private boundary and production-active for the managed path.
`join.rs` ports the Desktop
`AgentObservationJoinStateMachine` semantics exactly: `pendingHints` keyed by correlation id,
`adapterId\0eventId` event keys, lazy expiry at the head of `receiveHint`/`admitEvent`/`settle`
(registration, retirement, and the test-only `reset` take no clock), full-correlation `apply-observation`,
settle-time partial application, immediate `attach-presentations` for late hints on applied
events, silent matched-attachment removal on generation retirement, hidden-wins/else-`unknown`
visibility merge, and drop-on-unknown-generation hints. `hook_ingress.rs` ports the
`AgentHookObservationIngress` translator:
namespaced session bindings, adapter-gated 18-event translation, execution/session/turn/request/
event id allocation order, turn-key fencing and adoption, request and child-session maps (128
each, sessions 128), ended-session binding retention, and in-order canonical dispatch through
the injected `&mut AgentRuntime` — the translator owns no runtime, no second reducer, and no
fake always-applied sink.

The only intentional semantic delta is the native admission bound `pending + recent <= 4096`
(accepted bounded-resource policy): replay of an existing key charges no slot, an applied expiry
transfers pending occupancy to recent without changing the total, unapplied and recent expiries
release slots, and a new admission at the joint cap is rejected without canonical mutation. Byte
limits are not heap bounds — string and container overhead remains.

The core emits `schedule-settlement`/`cancel-settlement` effects only; no timer or socket IO lives
inside the port. The `SettlementQueue` timing primitives in `conformance_support.rs` are shared
with the production `Instant` funnel (admission, `tick`, and the native-effects funnel head all
sync the scheduler through the service's single `clock_origin`); conformance-only concerns —
the fixture JSON emitters, the `DispatchTrace` per-step dispatch recorder, the f64-domain corpus
clock advance, and the `FixtureIngress` owner pair — stay `#[cfg(test)]`. The translator core
calls `dispatch` on the injected `runtime` verbatim and records into the trace — the core
carries no fixture-JSON dependency. Conformance harnesses drive an
independent scheduler clock: a `settleWait` step advances scheduler time to fire due settlement
effects while the injected `now` stays at the
step's recorded `t`, matching the corpus's two-clock recording — the recorded `f64` seams cannot
reach the production `Instant` sync, and `Instant` never crosses into the translator.

Fixtures `schemas/agent/v1/agent-observation-join-conformance-v1.json` and
`schemas/agent/v1/agent-hook-observation-conformance-v1.json` are distilled replay payloads plus
expected values with the source SHA-256 fingerprints of the Desktop files they were generated
from; they drop generator paths, run history, and host specifics. Provenance and the refresh
contract are owned here, not by `schemas/agent/README.md` (which covers only the canonical
corpus): regeneration re-runs the recorded external harnesses against the real Desktop sources
and re-verifies the fingerprints before replacement.

## Conformance evidence and provenance

`schemas/agent/v1/agent-canonical-conformance-v1.json` is the durable executed corpus:
29 vectors / 665 recorded transitions produced by running the authoritative TypeScript
sources, plus per-vector journal reads, projections, and named quirks. It is parity
**evidence**, not public schema authority, and remains a private fixture format. The
fixture's `provenance.sourceFingerprints` records the SHA-256 of every
`packages/agent/src` file it was generated from; regeneration is owned by
`packages/agent` (`conformance:generate`, `--check` for drift detection) and imports the
owning TypeScript build — never a vendored source copy. See
[`schemas/agent/README.md`](../../../schemas/agent/README.md) for the regeneration
contract.

Additional named boundary cases are asserted in `src/agent/targeted.rs` against outcomes
recorded by executing the same TypeScript sources: guard precedence, post-end visibility,
capacity and `MAX_SAFE_INTEGER` edges, UTF-16 length/ordering (including the executed
exact-256/257-code-unit boundary), journal input errors, projection ties, stale-session
partial and final-navigable detaches, and the presence distinctions above (residue on
`unknown`/`idle` turns, absent vs explicit `null` on required-nullable keys, absent
`parentSessionId`/`turnId`/`activityHint`/`lastOutcome`/`turn` claims). A second named
set in `src/agent/oracle.rs` asserts further recorded outcomes: the aggregate
attention-subset freshness fold, provider dedup/ordering, equal-status freshness and
activity-hint ties, UTF-16 `presentationId` ordering across the BMP/astral boundary,
per-location navigable/freshness/status preference with the non-winning-generation
group exclusion, invalid seeded snapshots, the seeded-head empty-journal
`reset-required`, and double-dispose idempotence. All conformance harnesses compile only
inside `#[cfg(test)]` and now run as Server binary-crate tests.

## Native observation leaf

`agent/native_observation.rs` is the private pure reducer for the native observation
wire, production-active through the managed drain path. It owns held hints (at most 1024 records, raw payloads of at most 4096
bytes each), the one current carrier — a CNP `WorkerId` + `WorkerGeneration` pair copied from
the authoritative `TerminalOperationAuthorization` — and its scalar contiguous watermark `W`,
plus the current committed bindings `presentation -> (carrier, host)`. The fold is pure: it
classifies typed facts and returns owned effect data (`route-hint`, `register-presentation`,
`retire-presentation`); it never touches `AgentRuntime`, the translator, IO, the registry, or
the coordinator. `AgentState` holds the leaf in the same cell and applies each returned effect
batch inside the same `with_state` guard, so join fences and hint replays can never outlive the
fold that produced them, and the whole cell shares the existing permanent poison boundary.

Sequence semantics follow the selected scalar-W contract: `seq <= W` dedups and ACKs the current
W, `seq == W+1` classifies then commits W atomically (invalid payloads and cap rejections still
consume the sequence), `seq > W+1` is a hole ACKing the current W, and a gap fact clips its
prefix to `W+1` then must cover contiguously — a fully covered gap is a dedup-equivalent noop
ACK. Stale or absent carriers mint no ACK; there is no tombstone and no drain epoch beyond the
Worker connection generation itself. Held records expire ten seconds after first
classification; every entry point sweeps first, including `reconcile_complete_bindings`, which
also validates the whole snapshot (bound, unique presentations, one-to-one identities) before
any mutation so an ambiguous snapshot is rejected without partial state. Retiring a carrier
returns one `retire-presentation` effect per binding of that exact carrier so the join fences
drop inside the same fold.

The translator gains one seam for this slice: `register_presentation_binding`, because
`environment_overlay_for` returns early when no socket path is bound and would otherwise skip
registration entirely; the existing overlay behavior is unchanged. The carrier lifecycle
seams `native_admit_carrier` and `native_retire_carrier` are production: the Worker
registry admits the leaf inside the registration guard before the active carrier record
becomes visible, and retires it inside the same guard before carrier release, carrying
the exact admitted `WorkerId` + `WorkerGeneration` identity. The per-event seams
(`native_apply_event`, `native_apply_gap`, `native_reconcile_bindings`, `native_sweep`)
remain `#[cfg(test)]` test-only: the production path applies facts only through the whole-page
folds below, and timer ownership stays inside the managed endpoint's poll thread.

The production read-preparation shape is `AgentService::execute_prepared` — the shared
executor driven by a required caller-owned snapshot provider, with no seam-absent
fallback. The `local_terminal_service/agent_drain_read.rs` pump is that provider:
`LocalTerminalService::prepare_agent_drain_read` mints one private four-second window
once after policy admission and before the first preparation lock, retries only
`WorkerRequestError::Backpressure` admission at a private 25 ms interval, and keeps the
registry -> coordinator guards held through the final fold. An elapsed `recv_deadline`
is terminal for the pump read and maps to `PreparedUnavailable`; empty or no-progress
pages are never retried, and no fabricated last-good snapshot exists.

The drain page/final-fence service seams plus
one read-only leaf accessor, `NativeObservation::current_carrier`, are production:
`current_carrier` exposes the
existing `(CarrierIdentity, W)` tuple — no new leaf state, token, epoch, or second
watermark. A private adapter lifts every domain fact from the typed `AgentDrainReply`
BEFORE the fold: each `NativeEvent` is constructed with the read's own requested carrier,
never a caller-supplied one, so an arbitrary or mixed-carrier fact vector is
unrepresentable; typed-reply construction (contiguous bounded coverage, per-entry payload
and gap bounds) stays the only validation, and a construction failure refuses before any
fold — token echo, floor, and ACK correlation remain the future caller's precondition, not
leaf state. `native_apply_drain_page` checks the full `CarrierIdentity` inside ONE
`with_state` guard before ANY entry applies, applies every fact and consumes its effects
inside the same guard, then reads the live `W` — an empty or fully-deduped page still
returns it, while an absent, retired, or replaced carrier yields `CarrierMismatch` with
zero mutation; retirement or replacement can only run before or after the fold, never
inside it. `native_fenced_final_snapshot` keeps the same single-fold discipline: the live
`(identity, W)` fence against the pinned barrier — or leaf-absence for a `NoCarrier`
expectation — then the disposal check, the atomic whole-facts reconcile, effect
consumption, and the canonical clone, in that cause order, so a stale local watermark can
never justify a different carrier's snapshot. `NoCarrier` is the read's true-none only
when the registry's active carrier is actually absent; a failed live-carrier proof maps to
the existing `RuntimeUnavailable`, never to a silent `NoCarrier` serve. The
`local_terminal_service/agent_drain_read.rs` pump owns the window, pacing, and transport
wiring in production; the carrier lifecycle runs through the managed Worker's real
registry admission.

## Managed observation endpoint

`local_site_service/agent_observation_endpoint.rs` is the
production private Unix transport that feeds the managed hook-observation ingress.
`local_site_service::managed` binds
one socket per site (`agent-observation-v3.sock`, mode `0600`) inside the site runtime
directory, records the bound path's filesystem identity at bind time, and runs a single
poll thread multiplexing the listener, a stop pipe, and up to `MAX_ACTIVE_CONNECTIONS`
accepted nonblocking streams; a stream that fails `set_nonblocking` is dropped, and
accept draining is bounded per loop iteration by the same private budget so a
continuously queued backlog cannot starve the stop pipe, settlement ticks, or live
connections.

The transport owns no canonical semantics: a strict-envelope body folds into the one
shared `AgentService` cell through the `admit_envelope` seam, and the 30-byte
`ACCEPTED_ENVELOPE_ACK` means join-admission bookkeeping accepted the envelope — never
canonical application, so an admitted-but-uncorrelated envelope is honestly consumed
without sessions while a zero-correlation envelope is refused at parse. Reading,
admission, and the ACK write share one absolute 500 ms deadline per accepted stream;
oversized bodies and post-admission deadline expiry close the stream without an ACK, and
disposal is checked inside the fold so admits close honestly rather than fabricating an
acknowledgment.

Clock ownership stays one-way at the cell boundary. The endpoint passes `Instant` values
to the service seams, which convert through the single `clock_origin` into translator
milliseconds and sync the settlement scheduler at every fold entry that can schedule —
admission, `tick`, and the native-effects funnel head — so a timer scheduled by an
external caller (the native event-before-hint fold) reaches the same monotonic
scheduler. `Instant` never crosses into the translator, and the corpus's two-clock
contract is preserved because the recorded `f64` seams cannot reach the sync. Poll
liveness is a private 25 ms cap: the poll timeout is the minimum of connection
deadlines, the next settlement deadline, and the cap, so a timer scheduled with no
connections present settles within one cooperative wake — no manual tick and no second
connection required. The cap bounds staleness; it is not a latency guarantee.

Failure ownership is split deliberately: per-connection and per-fold errors destroy only
that connection (`next_deadline` errors contribute no deadline, `tick` errors are
skipped, admission errors close the current stream), while non-`WouldBlock` listener
accept failures and permanent poll errors terminate the endpoint thread and `EINTR`
retries. `BoundEndpoint` owns the listener, stop pipe, path, and recorded identity until
`start` moves the listener into the poll thread and disarms the identity after a
successful spawn; `AgentObservationEndpoint::drop` writes the stop byte, unconditionally
joins the thread, then unlinks the path only when its current metadata still matches the
identity recorded at bind — a replaced or foreign path is never removed.

Production activation is live for the accepted managed local Site path:
`local_site_service::managed` creates or obtains the runtime directory, binds this
endpoint before publishing Site readiness, and fails `site start` honestly when the
bind or endpoint initialization fails — there is no silent Agent-unavailable mode.
The managed path constructs the canonical `AgentService` once through
`open_observing(policy, socket_path)` — the socket path is bound into the translator
at construction before the `Arc` is shared — and hands the same `Arc` to the endpoint,
to Worker admission (`register_carrier_with_agents` admits the native leaf inside the
registry guard), and to client reads. The Worker derives the socket path from its
existing `--local-site <runtime-dir>` argument, validates socket type and same-user
ownership, and injects `ORCHBEE_AGENT_OBSERVATION_SOCKET_V3` through the existing
environment overlay; no new wire field, public setting, or Server-code dependency in
Worker exists, and remote (non-managed) paths keep their existing behavior. Replaced
or foreign socket paths are never unlinked on cleanup — drop stops and joins the poll
thread before the identity-gated unlink.

## Explicitly out of scope

Persistence/durable state,
remote (non-managed) Agent observation and any remote Agent
extension, waits/watches, `changes`, canonical-model mutations beyond the
observation folds, the names sidecar and session deletion (both
unimplemented — the candidate design and the `delete.request` routing
obligation live in
[`proposals/agent/session-names.md`](../../../proposals/agent/session-names.md)),
TUI
consumers, wire framing/codec ownership (the `orchbee-agent-observation-protocol`
crate), cursor secrets and their rotation policy (`agent_service`), CLI grammar,
rendering, and localization, TypeScript behavior changes, and listener/wait
concurrency. The client-facing read budget is unchanged; the private pump window is an
internal server constant, not a product setting or a client guarantee. New semantics,
schema, dependency, or ownership questions return to Root rather than being decided
here.
