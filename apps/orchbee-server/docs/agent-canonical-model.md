# Canonical Agent observation model

This document owns the Server-side canonical Agent observation model under `src/agent/`.
It records the accepted scope, the semantic claims the port must hold, and the boundary
mapping between the authoritative TypeScript model and its Rust mirror.

## Status and activation boundary

`apps/orchbee-server` owns a faithful Rust implementation of the deterministic semantics
currently defined by `packages/agent` at base commit
`0d4b685a1b7db9405ca94908385e0534ddd41dc9`. The module is registered from `main.rs` under
`#[cfg(any(unix, windows))]`: the canonical runtime lives inside the **Server binary crate**, never in
the library. Its conformance suites run once in that same binary target; `lib.rs`
neither declares nor exports the Agent module. No public API, new crate, or second
canonical state exists.

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
f64-domain corpus settlement advance, the test-only service
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

| Path                                         | Responsibility                                                                                                                                                                                                                                                                      |
| -------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/agent.rs`                               | Module declarations and the `pub(crate)` production re-exports; corpus/test-only helpers stay gated inside each module                                                                                                                                                              |
| `src/agent/model.rs`                         | Canonical value types, identity validation, UTF-16 ordering, snapshot invariants; raw construction and validation are the production admission path                                                                                                                                 |
| `src/agent/events.rs`                        | The fourteen observation event kinds and the four semantic event kinds; `#[cfg(test)]` `event_id` provenance accessor                                                                                                                                                               |
| `src/agent/reducer.rs`                       | `transition_agent_runtime` per-kind guards, unchanged/rejected/applied, semantic event order; `#[cfg(test)]` transition accessors                                                                                                                                                   |
| `src/agent/changes.rs`                       | Change records, production append with retention 512 and contiguity; production `read_agent_runtime_changes` paging kinds                                                                                                                                                           |
| `src/agent/runtime.rs`                       | `AgentRuntime` — production: empty-snapshot construction, `snapshot`, `dispose`, `is_disposed`, `dispatch`; `#[cfg(test)]`: seed; production change reads                                                                                                                           |
| `src/agent/projection.rs`                    | Semantic status, summary, and explanation projections; `#[cfg(test)]` aggregate/location presentations and wait-condition matching                                                                                                                                                  |
| `src/agent/tests/corpus.rs`                  | `#[cfg(test)]` conformance corpus runner and private fixture decoder                                                                                                                                                                                                                |
| `src/agent/tests/targeted.rs`                | `#[cfg(test)]` named boundary regression tests                                                                                                                                                                                                                                      |
| `src/agent/tests/oracle.rs`                  | `#[cfg(test)]` named regression tests against recorded TypeScript-oracle outcomes                                                                                                                                                                                                   |
| `src/agent/join.rs`                          | Pure port of the Desktop `AgentObservationJoinStateMachine`: generation registration/retirement, hint/event join, TTL cleanup, settlement effects, admission bounds; `#[cfg(test)]` `reset`                                                                                         |
| `src/agent/hook_contract.rs`                 | Strict-envelope/correlation-hint boundary parse and the twelve-adapter event-gate registry (ports `agent-hook-observation-contract.ts` and `resolveAgentHookAdapter.ts`)                                                                                                            |
| `src/agent/hook_ingress.rs`                  | Pure port of `AgentHookObservationIngress`, runtime-free after the bridge split: session bindings, turn/request/child maps, join-command consumption, canonical dispatch in order through the injected `&mut AgentRuntime`; `#[cfg(test)]` corpus/trace/f64-settlement/JSON helpers |
| `src/agent/hook_ingress/settlement_queue.rs` | `SettlementQueue` timing primitives used by the production `Instant` funnel, with a `#[cfg(test)]` clock-advance seam for corpus replay                                                                                                                                             |
| `src/agent/tests/conformance_support.rs`     | Test-only fixture-shape JSON emitters, `DispatchTrace`, and the `FixtureIngress` corpus owner pair                                                                                                                                                                                  |
| `src/agent/hook_ingress/tests.rs`            | Adjacent unit tests for dispatch identity, settlement-clock ordering, and provider state translation                                                                                                                                                                                |
| `src/agent/tests/hook_conformance.rs`        | `#[cfg(test)]` — replay harnesses for the frozen join and hook corpora plus the native joint-capacity checks                                                                                                                                                                        |

## Read service and transport integration

The first-read is served by the binary-private `agent_service`, integrated into the
existing admitted local-client path — no new socket, listener, thread, or unsolicited
frame:

| Path                                                           | Responsibility                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| -------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/agent_service.rs`                                         | `AgentPolicy`, `AgentService` (one `Mutex<AgentState>` cell + boot-scoped cursor authority), the shared prepared-read executor `execute_prepared` for list/get/explain, `open_observing` construction-time socket binding, `shutdown`, `AgentReadFault` loud-failure type, `PreparedUnavailable` provider-decline marker, the production `Instant` clock seams (`admit_envelope`, `tick`, `next_deadline`), the production native carrier seams (`native_admit_carrier`, `native_retire_carrier`) and drain/fence seams (`native_apply_drain_page`, `native_fenced_final_snapshot`), `#[cfg(test)]` `seed`, test-only bridge seams (`admit_hook_envelope`, `observe_hook_signal`, `hook_environment_overlay`, `retire_hook_presentation`, `advance_hook_settlement`), and test-only native seams (`native_apply_event`, `native_apply_gap`, `native_reconcile_bindings`, `native_reconcile_and_snapshot`, `native_sweep`, `native_held_count`, `native_poison_after_mutation_for_test`) |
| `src/agent_service/map.rs`                                     | Explicit typed canonical-to-wire mapper; every unrepresentable canonical value fails loudly rather than being filtered or fabricated                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `src/agent_service/cursor.rs`                                  | Private boot-scoped HMAC list/change-cursor authority: mint, canonical base64url shape, MAC/shape/semantic verification order                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `src/agent_service/tests.rs` + `cursor/tests.rs`               | Policy, ordering, pagination, cursor invalid/stale/boot-scope, ended filtering, disposal, and mapping-fidelity coverage                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `local_client_service.rs`                                      | One `Arc<AgentService>` beside the workbench service; the `AgentRuntimeOwner` guard disposes the runtime when the listener loop ends                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `local_client_service/application_session.rs`                  | The third `OBAP` demultiplex arm inside the same sequential record loop — read, execute, write exactly one correlated result                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| `local_client_service/tests/agent_harness.rs` (`#[cfg(test)]`) | Reusable seeded-service transport harness: real event-dispatch seeds, `admitted` socket-pair sessions, and `listen` real-listener fixtures                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| `local_client_service/tests/agent.rs`                          | Real listener-socket and admitted-pair OBAP evidence: seeded reads, empty state, owner-drop unavailability, malformed-record termination                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| `local_client_service/application_session/tests/agent.rs`      | OBAP demux coverage: routing, interleaved OBCS ordering, malformed records, executor faults, result write failures                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| `local_terminal_service/agent_drain_read.rs`                   | The production Agent drain read pump — the `execute_prepared` provider invoked once per read: typed carrier tri-state (`NoCarrier` is the only true-none read), one in-flight exchange under the private four-second window minted once before preparation locking, 25 ms backpressure-only admission retry, one whole-page fold, the barrier pinned once from the first carrier-matched reply, and the fenced final snapshot taken while the registry -> coordinator guards stay held in that order                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `local_terminal_service/agent_drain_read/tests.rs`             | Real-wire composition evidence: the canonical hint + rich observation through the real registry/`UnixStream`/coordinator projection, `NoCarrier`/pending/`Unavailable` distinction, carrier-rotation and page fences, deadline refusal, poison/disposal, and retained-snapshot `NoCarrier` reads                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `local_site_service/agent_observation_endpoint.rs`             | The managed observation endpoint — private Unix-socket transport for the hook ingress, bound inside the Site runtime directory before readiness: one bound path per site, one nonblocking listener plus a stop pipe on a single poll thread, bounded accept draining, the per-connection read/admission/ACK machine under one shared 500 ms deadline, and identity-gated unlink on drop                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `local_site_service/agent_observation_endpoint/tests.rs`       | Real-socket endpoint evidence: immediate vs settled application, unmatched/zero-correlation refusals, deadline sweep, connection cap and interleaved connections, poisoned/disposed cells, scripted writer seams, identity mismatch, live-connection drop reaping, and the bounded accept batch                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |

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
`agent.rs` re-exports have real consumers in the binary's production or test code
and need no unused-import suppression. No module-wide suppression, no fake use,
no const tables, no variant narrowing.

## Session names sidecar

The OBAP names family is implemented by `agent_service/names.rs` in the same
`AgentState` cell. The canonical indexes hold name → session and session → name;
the terminal extension below shares their namespace. Names are runtime-scoped,
absent from canonical session documents, and not yet persisted.
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
and `agent rename` consume the names family; get/explain/wait and terminal control commands resolve names on the same admitted session and pin the resulting session ID. CLI composition and deadlines belong to the [native CLI contract](../../orchbee/docs/native-cli-query.md).
Deterministic service tests cover collision, atomic rename, exact-ID precedence,
full capacity, lifecycle cleanup, policy separation, stale preparation, disposal,
poison, and concurrent claims. Wire and Windows process tests cover bounds,
schema-conforming replies, malformed operations, and ambiguous mutation outcomes.

## Terminal-backed names

Windows also admits a terminal with an actual reported, process-detected or retained provider
identity when it has no canonical Agent session. The same Names owner stores its alias against
the exact Presentation, creation generation, Host epoch and Host terminal identity. Closed
terminal outcome variants distinguish this target from a canonical session; no synthetic session
ID is introduced. The 512-name capacity and collision checks include canonical names, terminal
names and pending launch reservations. Atomic rename at capacity remains valid.

An unmanaged terminal name retains its provider label and optional external session reference.
A first reference upgrades ownership; a different provider or different known reference clears
it. A proven release/exit, terminal removal or physical-generation replacement also clears it;
missing observations do not prove exit. Report transitions notify the Names leaf synchronously
under the coordinator commit guard, including intermediate changes between queries. A single
weak handle connects that composition to the existing AgentState, without a second registry,
event log or process observer. Workbench owns the pure report-owner projection.

Names requests retain the existing registry → coordinator → AgentState lock order through final
resolution/mutation, following one drain preparation with policy checked first. The final fold
does no I/O; a committed mutation returns its correlated result even if the deadline then passes.
Disposed/poisoned owners remain unavailable. Existing canonical ID precedence, detached-session
names and request ambiguity rules remain. Whole matched cohorts carry the extended protocol;
rollback replaces the cohort. Managed-name persistence follows the fresh-binding restart contract
below; a terminal alias alone never establishes managed startup.

Evidence must cover mixed-name/reservation collisions, full capacity, reference upgrade and
replacement, provider A → B → A between reads, proven versus uncertain exit, physical replacement,
policy/disposal, strict wire bounds and actual Windows rename/resolve/list/clear plus named
terminal access. The protocol schema and CLI envelope versions must agree with emitted targets.

## Managed name restart

The private Windows checkpoint version 4 retains only completed managed name claims, keyed by
Presentation and containing name plus provider ID. Every claim must have a valid official resume
reference for that same normalized provider. Claims share the 512-name bound and have unique names
and presentations. Pending launches, canonical session IDs, old physical identities and reservation
IDs are not restart authority. Versions 1–3 remain readable; older binaries refuse version 4 rather
than silently discarding it. Matched-cohort rollback must retain compatible restart data or use an
explicit conversion/prior valid checkpoint; it must not overwrite a retained version 4 on failure.

Clean-stop capture holds registry → coordinator → AgentState fences over exact current membership.
Startup selects only enabled, deduplicated provider resume plans. After fresh Host creation and
reference seeding, Names restores the selected claims onto the new physical bindings before Site
publication. Disabled resume, duplicate suppression and missing physical creation do not restore
names. Restored management expresses ownership, not provider readiness or a fabricated canonical
observation. Same-provider reference changes retain managed names; provider changes, confirmed
release/exit and physical replacement/removal invalidate them. Rename retains management.

The existing launch family owns an exact terminal-management status read. Physical Agent Get
consumes this status; canonical launch status remains independent. Read policy precedes preparation,
and registry/coordinator guards remain held through the Names status fold so physical replacement
cannot pass between final membership validation and status sampling. Clearing a restored name also
clears its matching owned persisted resume seed, while preserving later independent report
references and authority. The coordinator stages the bounded report update before the Names
mutation, then commits that prepared update only for the matching successful name release, while
its guard is still held. Both owners perform CPU work only; no transport runs under commit guards.
This prevents a visible half-release or accidental deletion of a newer provider session.

Evidence must establish strict old/new checkpoint admission, bounded and unique claims, pending
exclusion, provider/reference agreement, new physical identities, disabled/deduplicated restoration,
same-provider reference replacement, staged clear semantics and actual Windows managed restart
with subsequent name/query/input/clear behavior. Codec or storage-only checks do not prove this
runtime chain. Terminal semantic wait/prompt/focus and complete provider readiness remain separate
obligations.

## Managed launch reservations

Agent startup uses the existing `AgentState` cell and names owner. A private launch
reservation is a temporary claim on one name and one native presentation binding;
it is not a canonical session and does not manufacture an idle observation. The
Windows CLI is the first consumer. OBAP carries a separate closed launch family
(reserve, inspect, complete, cancel) over the existing admitted Client Session.
Read policy gates inspection; manage policy gates all three mutations, before the
existing observation preparation outside the leaf lock.

Inputs are typed names, provider IDs, presentation IDs, the sampled Host epoch and
Terminal identity, a nonzero request identity, and a 1–300000 ms relative TTL.
Reserve compares the current native binding against both sampled Host identities,
pins its carrier and creation generation, and records matching sessions' observation
generation/sequence baselines. A candidate must be a live root session, exactly attached to that
pinned generation, provider-matching, and newer than its baseline (or absent from
it). Zero candidates means pending; multiple candidates are an explicit ambiguity.
This is local observation evidence, not causal authentication of a provider process.

Names and reservations share the 512-entry capacity. Each name, reserved presentation
and committed session alias is unique. Normal name assignment cannot claim a reserved
name or rename a session on its reserved presentation; release and resolution report
pending-launch explicitly where applicable. `agent names` exposes committed bindings
and pending reservations separately. Expired claims cannot block a subsequent request.
Reservations are runtime-scoped, never durable session restoration data.

| Event                                    | Guard and transition                                                                                                                                               | Observable result                                        |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------- |
| Reserve                                  | Runtime and binding live; name/presentation free; capacity available; record baseline and expiry                                                                   | Reserved identity and pinned binding; no terminal input  |
| Inspect                                  | Reservation still live; recompute candidates against current canonical state                                                                                       | Pending with zero/one candidate, or explicit ambiguity   |
| Complete                                 | Same binding; unique requested candidate; at least 3000 ms elapsed; fresh idle/completed state without attention or pending requests; candidate has no other alias | Assign name and remove reservation in one fold           |
| Cancel                                   | Matching live reservation                                                                                                                                          | Release only that reservation; never terminate a process |
| Expire or binding replacement/retirement | Deadline reached or exact native binding no longer exists                                                                                                          | Release name/presentation claim                          |
| Dispose                                  | Runtime disposed                                                                                                                                                   | Clear aliases and reservations; answer unavailable       |

The service samples monotonic time at its fold boundary; the names owner clamps
injected times monotonically and receives only integer milliseconds. Reconciliation
runs before decisions and after folds. The existing endpoint deadline/tick owner also
includes reservation expiry, so abandoned clients need no per-launch thread. The
3000 ms settlement interval starts at reservation acceptance; the CLI additionally
waits after its actual input submission. No timeout refresh or automatic replay exists.

Reserve/complete/cancel are explicit commit points inside the leaf lock; serialization
and publication occur afterward. A lost mutation reply is outcome-unknown. The caller
retains its reservation identity and input receipt, queries names/inspection when
possible, and relies on bounded expiry otherwise. A relative server TTL can end later
than the CLI deadline by transport delay; an unconfirmed cancellation must never be
reported as a confirmed release. Expiry releases a name, not a potentially running Agent.
The CLI owns sampled shell/process preflight and one terminal submission; no socket,
process query, outer lock, or terminal write runs inside the names fold.

Deterministic state witnesses cover competing claims, shared capacity, stale and
ambiguous candidates, blocked/working/stale state, exact binding replacement, expiry,
cancellation, disposal and alias interference. Wire witnesses cover closed families,
operation correlation and bounds; Windows process and ConPTY witnesses cover literal
arguments, one submission and retained failure receipts. This workflow uses implementation tests by the active user request. No paid-provider task, arbitrary provider UI
readiness, process-only session discovery, or durable name restoration is established
by these reservation claims.

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
inside the port. The `SettlementQueue` timing primitives in `hook_ingress/settlement_queue.rs` are shared
with the production `Instant` funnel (admission, `tick`, and the native-effects funnel head all
sync the scheduler through the service's single `clock_origin`). The fixture JSON emitters,
the `DispatchTrace` per-step dispatch recorder, and the `FixtureIngress` owner pair live in
`tests/conformance_support.rs`; they and the f64-domain corpus clock-advance seam remain
`#[cfg(test)]`. The translator core
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

Additional named boundary cases are asserted in `src/agent/tests/targeted.rs` against outcomes
recorded by executing the same TypeScript sources: guard precedence, post-end visibility,
capacity and `MAX_SAFE_INTEGER` edges, UTF-16 length/ordering (including the executed
exact-256/257-code-unit boundary), journal input errors, projection ties, stale-session
partial and final-navigable detaches, and the presence distinctions above (residue on
`unknown`/`idle` turns, absent vs explicit `null` on required-nullable keys, absent
`parentSessionId`/`turnId`/`activityHint`/`lastOutcome`/`turn` claims). A second named
set in `src/agent/tests/oracle.rs` asserts further recorded outcomes: the aggregate
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
endpoint before publishing Site readiness, and fails `server start` honestly when the
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
extension, waits/watches, canonical-model mutations beyond the
observation folds, canonical session deletion,
TUI
consumers, wire framing/codec ownership (the `orchbee-agent-observation-protocol`
crate), cursor secrets and their rotation policy (`agent_service`), CLI grammar,
rendering, and localization, TypeScript behavior changes, and listener/wait
concurrency. The client-facing read budget is unchanged; the private pump window is an
internal server constant, not a product setting or a client guarantee. New semantics,
schema, dependency, or ownership questions return to Root rather than being decided
here.

The names family additionally accepts `{kind:"presentation", presentationId}`. Resolution, assignment and release inspect the current canonical snapshot inside one Agent fold. Only live sessions with a matching navigable attachment qualify, including hidden attachments; attachment generations are owned by the canonical model. Zero matches refuses as not-found (session-not-found for mutation), multiple sessions refuse as ambiguous-target, and one resolves with resolvedBy presentation. A duplicate attachment never counts a session twice. Refusals leave name bindings unchanged and stay within the 1,024-byte result bound. Workbench Panel IDs and primary-presentation selection remain CLI composition, outside the Agent cell; no socket read occurs under its lock. Same-cohort delivery gates the additive wire vocabulary; rollback replaces the cohort and requires no persistent-state migration.

## Native bounded change reads

The native `agent.session.changes` family exposes the existing canonical journal to Client attention, waits and incremental observation. The journal retains 512 applied transitions; no second event store or Agent writer is introduced. Each record carries its revision, one of the fourteen canonical transition kinds, and the journal's exact session projection (lifecycle, observation, parent, pending count, provider, session ID and semantic status). Attachments and execution location require a coherent session read, not invented history fields.

Read admission precedes exactly one caller-owned drain preparation outside the Agent cell. Denial never runs that provider. Failure or panic yields correlated runtime-unavailable. After successful preparation, read the journal and head revision together under the Agent cell; later canonical events may be included. Disposal or poison refuses the read. No I/O occurs under the Agent lock.

| Input/state                                              | Result and obligation                                                                                         |
| -------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| Omitted cursor                                           | Current-head checkpoint; this does not claim delivery of earlier changes.                                     |
| Authentic cursor at head                                 | Current with the same observed revision and a reusable cursor.                                                |
| Authentic retained cursor                                | 1–100 contiguous changes, cursor at the last delivered revision, hasMore exactly when below the sampled head. |
| Authentic cursor older than retention                    | Reset-required with a new head checkpoint; consumer must re-read its baseline and expose lost continuity.     |
| Invalid, foreign-boot, wrong-operation or ahead cursor   | Cursor-invalid; consumer must explicitly establish a new baseline, never silently treat it as current.        |
| Read denied, provider failure, disposed/poisoned runtime | Correlated rejection/unavailability; no delivered progress.                                                   |

Changes tokens reuse the cursor payload layout with operation 2, zero flags/target/position, the boot instance and observed revision. Their HMAC domain is `orchbee.agent.changes.cursor.v1\0`, distinct from list tokens. Shape/canonical encoding and constant-time MAC verification precede semantic inspection. The existing boot authority owns both domains and dies with the service; tokens do not survive restart. Omitted limit defaults to 50; accepted limits are 1–100. No filtering is applied before pagination. The wire owner bounds each page at 1 MiB before allocation and validates positive contiguous revisions, tail/head/hasMore agreement, closed transition kinds and session bounds.

A consumer establishes a checkpoint before its coherent session baseline and reconciles later changes. A reset or cursor-invalid result invalidates continuity and any acknowledgement based on it. Client-local seen/unseen never changes canonical status. This extension uses a matching development cohort; replacing that cohort rolls it back without a persistent-state migration. Native Windows transport is the initial consumer. Protocol, cursor-domain/restart/retention, canonical corpus and prepared-read failure tests cover the boundary; physical presentation and final CLI acceptance remain consumer obligations.

## Managed startup status

Read-only `agent.launch.status` inspects an exact Session after read-policy admission and one preparation outside the Agent cell. It samples the canonical revision and the names owner's startup state together: unmanaged; pending with the existing exact reservation and current blocked flag; or active with the accepted Complete operation's reservation identity, provider and physical binding. Status never commits, renews, replays or launches.

Only accepted launch Complete creates active state. Manual naming cannot establish startup readiness; renaming preserves an existing managed identity. The active record is bounded by the committed name capacity and is removed on explicit name clear, canonical end/removal/disposal, provider mismatch or exact native binding replacement/retirement. Pending keeps its original TTL. Active means startup was completed, independently of later working/blocked/freshness state, OS focus or input ownership. The existing CLI blocked-start cancellation remains a separate workflow obligation. Completed ownership
can also be restored as a physical name through the managed-name restart contract; it never restores
a canonical session or repeats the original readiness assertion.

The frozen corpora retain their original source fingerprints. The additive Kilo registry boundary is checked separately by `kilo_hook_boundary_matches_opencode_with_distinct_provider` and the Desktop adapter registry tests; this does not relabel the historical corpus as regenerated.

Cursor adds a direct native and Desktop boundary witness for session-identified (and legacy session-started) identity only; historical corpus fingerprints remain unchanged. It grants sessionLifecycle without turn, activity or permission claims.

Qoder admits only session-identified with sessionLifecycle. Direct native registry and exact ingress tests extend the identity-only witnesses without changing historical corpora.

Antigravity CLI shares the identity-only boundary witness and maps adapter antigravity-cli to canonical provider antigravity. Its PreInvocation does not establish turn state or readiness.

Mastracode grants session/turn lifecycle, activity, approvals and subagents through the closed observation registry. Direct joined-ingress evidence covers identity, one keyed run, attention request/resolution and completion. Historical corpus fingerprints remain unchanged.

Droid joins the identity-only registry and native ingress witnesses, preserving unknown or existing turn state on SessionStart.

Kimi admits only session-identified and session-ended. Repeated identity preserves canonical turn state; Stop and other unsupported lifecycle events remain rejected.

Letta admits only session-identified and preserves unknown or existing turn state. Default conversation identities include the Agent ID; content and guessed completion never enter canonical state.

Devin admits only session-identified across its six hooks. Identity refreshes preserve unknown or existing turn state and never resolve permission requests or infer completion.

Hermes admits only session-identified. Session startup/reset and CLI resume observations preserve canonical turn state; they never manufacture idle or completion.
