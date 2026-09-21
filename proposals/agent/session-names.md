# Agent session names sidecar

| Field             | Value                                                                                                                                                                                                                                                                                                                                |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Status            | Design candidate, not implemented and not accepted. The OBAP wire union, names DTOs, and frame codec merged ahead of any Server executor or CLI surface, so this document carries the intended semantics forward for the future implementation slice. Nothing here is current authority until the product and architecture decision owner accepts it and the owning implementation lands |
| Decision owner    | The active requester acting as Orchbee product and architecture decision owner                                                                                                                                                                                                                                                         |
| Semantic owners   | Server `AgentState` names-table ownership, the `AgentService` executor, the OBAP names family, and the CLI names command surface                                                                                                                                                                                                         |
| Current authority | [`apps/orchbee-server/docs/agent-canonical-model.md`](../../apps/orchbee-server/docs/agent-canonical-model.md) (implemented read model and the session-demux names refusal), [`crates/agent-observation-protocol/README.md`](../../crates/agent-observation-protocol/README.md) (staged names codec/DTO status)                              |
| Exit condition    | The names feature is implemented under its own accepted authorization and the owning authority — canonical-model, command-surface, and `schemas/agent-observation/v1` — converges to describe the shipped behavior; this proposal and its translation are then deleted                                                                     |

Simplified Chinese review translation:
[session-names.zh-CN.md](session-names.zh-CN.md). English remains canonical.

## Change authorization (proposed)

| Field         | Proposed authorization                                                                                                                                                                                                                                                                                                                                                      |
| ------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Tier          | Modeled — the sidecar introduces a new stateful table inside the `AgentState` cell, selector-resolution ordering, atomic rename transitions, and a bounded read payload; Consequential surface area is limited to the already-merged wire kinds                                                                                                                                    |
| Problem       | Clients address sessions only by opaque session identities. The merged protocol already emits and consumes a closed names request/result family (resolve, names.list, name.assign, name.release), but no Server state, executor, policy gate, or CLI grammar exists to answer it                                                                                                   |
| Phase outcome | A per-Site, runtime-scoped alias table inside `AgentState` answers all four names operations through the shared `with_state` fold; the CLI resolves positional `auto` tokens and explicit `--session`/`--name` escapes; the published schema covers kinds 7–14 alongside the read family                                                                                             |
| Non-goals     | Persisting names across runtime disposal, remote (non-managed) resolution, changing canonical session documents, a write ledger or replay semantics, session deletion (`delete.request` remains reserved — when it lands it MUST route through the same in-fold release), and any second names authority                                                                                |
| Compatibility | Until this slice lands, names behavior is the current refusal: a well-formed names frame ends the client session with `WrongDocumentKind`. After it lands, every names operation answers through the typed union — no client changes are required for sessions that never name                                                                                                                                           |
| Delivery      | One slice: `AgentState` names maps + in-fold sweep, `AgentService` executor operations, `AgentPolicy.manage` admission, session-demux routing to the executor, CLI command surface, schema kinds 7–14, and owning-authority convergence — evidence at protocol, service, session-demux, and CLI process layers                                                                       |
| Recovery      | Withdraw the names demux routing; the staged codec/DTOs stay inert and the session-demux refusal is the absence behavior — never a fabricated names result                                                                                                                                                          |

## Verified current state

- `crates/agent-observation-protocol` carries the closed `AgentRequest`/`AgentResult`
  union, all names DTOs (`AgentSessionSelector`, `AgentNameBinding`, the four
  request/outcome pairs), `MAXIMUM_NAME_BINDINGS` (512), and OBAP frame kinds 7–14
  with bounded bodies. `schemas/agent-observation/v1` covers only the read family
  (kinds 1–6); the names kinds have no published schema.
- `apps/orchbee-server` `AgentState` holds no names table and `AgentService`
  executes only list/get/explain. `AgentPolicy` has only a `read` gate — no
  `manage` capability exists. The application-session demux answers every
  well-formed names request with `AgentWireError::Contract(WrongDocumentKind)`,
  ending the session without invoking the read executor or writing a result.
- The CLI exposes no names command path; `docs/cli/command-surface.md` has no
  admitted names grammar.
- The design below was carried forward verbatim from the names section that
  briefly lived inside `agent-canonical-model.md`; it is preserved here as the
  candidate the future implementation slice reviews, not as accepted fact.

## Proposed design

The names sidecar is a per-Site, runtime-scoped alias table owned by the
canonical `AgentState` cell — never a canonical-model field, never persisted,
and empty again after every runtime disposal. It exists so clients can address
a session by a short operator-assigned alias instead of the opaque session
identity; the canonical session document itself is unchanged and never carries
a name.

### Carrier and invariants

- `AgentState` holds `names: BTreeMap<AgentName, AgentSessionId>` plus the
  reverse index `sessions_named: BTreeMap<AgentSessionId, AgentName>` — one
  forward/reverse pair under the same `Mutex` the runtime already lives in.
  `names` order is the emitted order (byte-sorted by name).
- A name is ASCII `[a-z][a-z0-9_-]{0,31}`; the `AgentName` typed constructor is
  the only admission point, so no path can store a malformed alias.
- At most one name per session and at most one session per name; the forward
  and reverse maps agree at every fold boundary.
- Bindings only ever point at live sessions: every `with_state` fold ends with
  a reconcile sweep that drops bindings whose session is absent or no longer
  `live`. Hook `session-ended`, native presentation retirement,
  settlement/tick expiry folds, and any future removal path all converge on
  the same in-fold cleanup, so a binding can never outlive the fold that ended
  its session — and can never be observed after it.
- The table is bounded by `MAX_AGENT_SESSIONS` (512). The bound applies to NEW
  bindings only: renaming an already-named session frees and takes a slot
  atomically inside the same fold, so a rename at full capacity succeeds.
- A disposed or poisoned cell answers `runtime-unavailable`, never a
  fabricated empty table or a stale last-good binding.

### Selector resolution

Every targeted operation carries an `AgentSessionSelector` — `session` (a
typed `AgentSessionId`), `name` (a typed `AgentName`), or `auto` (the CLI
positional token). Resolution runs inside the same `with_state` fold that
reads or mutates:

- `session` — exact session-identity existence, any lifecycle. Never falls
  back to names.
- `name` — the complete names map only.
- `auto` — exact session-identity existence first, then the names map. Session
  identities can equal valid names, so existence decides, not the charset: a
  name that equals a live session's id resolves to the id; `--name` is the
  only escape onto the names table.

`resolve` reports `resolved{sessionId, resolvedBy}` where `resolvedBy`
records which rule hit (`session-id` or `name`); a miss is `not-found`.
`assign` and `release` resolve their `target` the same way — inside the
mutating fold — so a session that ended between request and fold yields
`session-ended`, never a ghost binding.

### Finite transitions

| Wire request kind                     | Policy          | Fold semantics | Outcomes |
| ------------------------------------- | --------------- | -------------- | -------- |
| `agent.session.resolve.request`       | `policy.read`   | Selector resolution only; zero mutation | `resolved{sessionId,resolvedBy}` / `rejected{not-found\|read-denied}` / `unavailable` |
| `agent.session.names.list.request`    | `policy.read`   | Enumerate the whole table after the in-fold sweep | `listed{bindings:[{name,sessionId,lifecycle}]}` / `rejected{read-denied}` / `unavailable` |
| `agent.session.name.assign.request`   | `policy.manage` | Resolve target; liveness and collision checks; commit the binding (or atomic swap when renaming) | `assigned{sessionId,name,previousName}` / `unchanged{sessionId,name}` / `rejected{name-in-use\|session-not-found\|session-ended\|name-not-found\|capacity-exceeded\|manage-denied}` / `unavailable` |
| `agent.session.name.release.request`  | `policy.manage` | Resolve target; drop the binding if present | `released{sessionId,name}` / `unchanged{sessionId}` / `rejected{name-not-found\|session-not-found\|manage-denied}` / `unavailable` |

- `assigned` carries `previousName` when the session already held an alias —
  the old binding is released and the new one committed in the same fold, so
  a concurrent `names.list` can never observe both or neither.
- `unchanged` is state idempotence (`assign` of the pair already bound,
  `release` of a session that has no name), not a requestId replay promise —
  there is no write ledger and a lost reply is `outcome-unknown` to the
  caller, never retried by the transport.
- `name-in-use` keeps the incumbent binding; the rejection carries the
  incumbent `conflictSessionId` so the caller can see the true owner.
- `names.list` is a bounded-read payload (≤512 typed bindings), not a small
  outcome — it is bounded and validated like `listed` sessions, and
  `is_small_outcome` admits only `rejected`/`unavailable` and the one-shot
  manage answers (`resolved`, `assigned`, `unchanged`, `released`).

### Bounded falsification notes

- A `name` that collides with a live session id: positional `auto` resolution
  picks the session; `--name` is the only path to the alias. A binding is
  released on its session's end, so the freed name can later bind a different
  session.
- `assign` on an ended or absent session is `session-ended`/`session-not-found`
  inside the fold — no pre-fold read is ever trusted as liveness proof.
- `release` on a `--name` that is not bound is `name-not-found`; on a session
  with no alias it is `unchanged`; on an absent session `session-not-found`.
- A `rename` committed while another client holds a `wait` poll never
  retargets the wait: the waiter pinned the resolved full session id.
- `dispose` clears the table with the runtime; every names operation then
  answers `unavailable{runtime-unavailable}`.
- When a `delete.request` ever lands, session removal MUST route through the
  same in-fold release — recorded here so its future owner cannot bypass the
  sweep.

## Open evidence gaps for the implementation slice

- The names small-outcome bodies are produced-side bounded at 1,024 bytes; the
  review-flagged question of pre-allocating against that bound on the read side
  stays open for the implementation slice (review item C8).
- No names codec round-trip corpus, Server executor evidence, selector-rule
  coverage, or CLI process evidence exists yet; the staged DTOs are untested
  beyond the protocol crate's type-level validation and must not be presented
  as usable until the slice's own evidence lands.
