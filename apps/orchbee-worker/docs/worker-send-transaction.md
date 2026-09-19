# WorkerSendTransaction bounded model

`formal/tla/WorkerSendTransaction.tla` is the bounded correctness model for the
Server-authorized terminal-send lane implemented by
`src/terminal_send_operation.rs` and dispatched from `src/terminal_operations.rs`.
It exists because the lane's evidence gap is a concurrency one — independently
ordered Host responses, carrier loss, and Server loss — not because any existing
model covers it: `LocalHostBroker` abstracts a single Host command,
`WorkerTerminalLifecycleStep` one lifecycle exchange, and the lifecycle receipt
ledger is never entered (the Send branch takes no kernel argument and continues
before `admit_launch`/`admit_start`).

## Scope and trusted assumptions

- One already-admitted `TerminalSendRequest` over one shared Host carrier. The
  request pins a fixed 48-byte `HostTerminalIdentity` plus a Server-minted
  `attachment_id`; the model keeps a single `Target` and treats identity
  checking as exact. The pre-admission fence (wrong worker, stale generation,
  stale epoch, disposed terminal, negotiated-limit excess) is a declared
  exclusion: it is a deterministic local decision that emits zero Host bytes
  and contains no interleaving, so its evidence stays with the scripted fence
  test (`send_fence_rejections_never_reach_the_host`). The model starts at the
  admitted request.
- The three correlations are pairwise distinct by construction: the model
  assumes the fresh-ID source (`worker_identity::fresh_identity`, 16 bytes of
  `/dev/urandom`) never collides inside one request. This is a trusted oracle
  assumption disclosed here, not a collision-freedom proof; it applies to the
  three correlation draws only — the attachment id is Server-minted.
- The existing TUI controller attachment on the same terminal is modeled as
  `tui` inside `hostAttachments`; it shares the carrier, so carrier death
  legitimately releases it (fault sharing is real, not claimed as isolation).
- Post-attach local abort (`?` propagation in `apply`/`input_command` after a
  committed attach) is modeled as `PostAttachAbort` even though Server-side
  validation makes it unreachable in production — it is a deterministic local
  path, modeled rather than silently assumed away.
- Byte counts are abstracted to `{0, 1, 2}`: 1 is the exact text echo, 0 is the
  legal ignored-key count, 2 is over the negotiated maximum. `payload` is
  `text` (echo must equal 1) or `key` (0 and 1 accepted), chosen at Init.
  `acceptedBytes` is a singleton drawn from `ValidAcks(payload)` once
  `inputAcked` — never `{2}` — and immutable thereafter
  (`AcceptedBytesMatchesPayload`, `AcceptedBytesStable`).
- Stage counters (`attachSent`/`inputSent`/`detachSent`) and `deliveries` are
  real occurrence counts (`@ + 1` at emit), not occurred bits. `TypeOK`
  deliberately admits `0..2` so a second emission is representable and is
  caught by the named `*AtMostOnce`/`ResultProducedOnce` properties — never by
  the type domain, and no guard may erase the possibility of a second send.

## Source mapping

- Fence checks → excluded by the scope boundary above; `Rejected` +
  `NotNeeded` is reachable in-model only through `AttachRefused`.
- `exchange_fresh` per step → `SendAttach`/`SendInput`/`SendDetach` plus the
  response family: `*Exact`, `*Refused` (TypedFailure), `*Desync` (payload-level
  echo or bound violation), `*Malformed` (envelope contract violation),
  `*Lost` (EOF/write failure/step deadline), `*LocalFault` (pre-write), and
  `ServerLost`/`CarrierLost` peer events. `StepAnswer` mapping: Record→exact,
  TypedFailure→refused, Desync and ProtocolFault both close the carrier but are
  recorded as distinct actions; at detach, everything non-`Released` is
  `Unreachable` — refused vs unreachable are kept distinct because refused is
  typed while unreachable folds desync/malformed/lost failures. A detach-stage
  LocalFault is a pre-send failure (`DetachLocalFault`, `detachSent` stays 0)
  folded into the same unreachable settlement — it must not be written as an
  emitted detach.
- Host apply vs worker-observed reply are independent variables:
  `hostAttachments`/`hostInputApplied` advance via `HostApply*` actions that
  may interleave with any response loss — an applied-but-unacknowledged input
  is reachable and is what makes `OutcomeUnknown` honest.
- `host.stream().shutdown(Both)` is local only; `HostTeardown` is the separate
  Host-side action that eventually releases orphaned attachments (send's own
  and TUI's) under weak fairness.
- `cleanup_closed` and interleaved Host events drained by `prepare`/
  `round_trip` are represented by `DrainHostEvent`, a bounded self-loop that
  is legal, never an error, and cannot settle the step.
- `server_transport::send` → `DeliverResult` (link alive) or `AbandonResult`
  (link already lost) — the result record is produced either way once
  `result-ready` is reached.

## Invariants and progress

`PairingMatrix`, `AcceptedFactSurvivesCleanup`, `AcceptedBytesMatchesPayload`,
`ConfirmedRequiresDetachAck`, `AttachRefusalNeedsNoDetach`,
`UntrustedStreamNoDetach`, `UnconfirmedCleanupClosesCarrier`,
`TuiLossOnlyViaCarrierDeath`, `SendAttachmentMatchesAdmission`,
`NoResultOnEarlyServerLoss`, `ResultProducedOnce`, `*AtMostOnce` per stage.
Temporal: `EventualSettlement`, `TeardownEventuallyReleasesAttachments`,
`DetachOnlyAfterCommittedInputFact`, `UnknownNeverResends`,
`AcceptedNeverDowngraded`, `AcceptedBytesStable`. Progress is weak fairness on
`Next` — every continuously enabled resolution (reply, expiry, loss, or local
settle) may fire; no peer is assumed to answer successfully.

`PairingMatrix` applies only once settled (`result-ready`/`done`): cleanup is
`none` while the detach axis is unresolved. The healthy carrier survives only
two settlements — `AttachRefused` and `DetachExact`; every other terminal path
retires it, and unconfirmed cleanup at `done` implies a dead carrier.

## Bounds and evidence consumers

One request, one terminal pair, one TUI attachment, byte domain 0..2
(`acceptedBytes` is a singleton set once the input is acked, `{}` before),
per-stage send counters ≤1, event drains ≤3. Refinement consumers are the
scripted-Host suite (`tests/process/terminal_operations/send.rs`, 15 cases)
and the public send tests; this model does not replace them — they pin wire
bytes it abstracts.

## Verification

Author's pinned positive control (under
`/home/liushuai/orchbee-batches/heavy.lock`, pinned `tla2tools-1.7.4.jar`,
1 worker, 512 MiB, 120 s ceiling):

```sh
java -Xmx512m -XX:+UseParallelGC -jar tla2tools-1.7.4.jar \
  -fp 0 -lncheck final -coverage 1 -workers 1 \
  -metadir <dir> -config WorkerSendTransaction.cfg WorkerSendTransaction.tla
```

Outcome: exit 0, 2688 generated / 1834 distinct states, depth 15, every
action covered (no zero-coverage line). Logs and the child exit code live in
the batch evidence directory `a3/evidence-14/positive/` (earlier
`a3/evidence-13/` runs predate the counter fix and are retained as-is).

The authoritative run uses the existing runner resource budget — currently a
2048 MiB heap, one worker, and a 120 s ceiling, as recorded in
`scripts/tla/models.json` (`workers` 1, `timeoutMilliseconds` 120000) and
capped by `maximumHeapMiB` in `scripts/tla/model-checks.mjs`.

## Negative controls

Mutant modules are produced out-of-tree per batch, each one guard or action
edit, each run separately under the same lock with full counterexamples and
exit codes preserved. Current batch: `a3/evidence-14/mutants/` on the frozen
module digest.

- `m1-detach-after-desync` — `InputDesync` still emits the detach record:
  violates `UntrustedStreamNoDetach` (exit 12).
- `m2-erase-accepted-fact` — `DetachUnreachable` erases the committed input
  fact: violates `AcceptedFactSurvivesCleanup` (exit 12).
- `m3-duplicate-input-guard-only` — guard-only relaxation of `SendInput`
  (re-entry while one input is in flight; baseline `@ + 1` counting
  untouched): `inputSent = 2` is caught by `InputAtMostOnce`, not `TypeOK`
  (exit 12). This is the load-bearing replay check — a counter bit would make
  the same guard mutation stutter invisibly.
- (earlier batch `a3/evidence-13/` also carried `m4-tui-takeover`, violating
  `TuiLossOnlyViaCarrierDeath`; its counterexample predates the counter fix
  and is retained as-is.)

Named reachability witnesses (probe invariants negating the target state, run
out-of-tree under `a3/evidence-14/named-traces/` on the frozen module digest;
each TLC violation is the witness trace):

- `ProbeKeyAckZero` — `payload="key" /\ outcome="accepted" /\
acceptedBytes={0}`: a semantic key legally acknowledged with zero bytes.
- `ProbeAppliedResponseLost` — `hostInputApplied /\ outcome="unknown"`: the
  Host applied the input but the answer never reached the Worker.

## Known abstractions (annotated, not fixed)

- `ServerLostAtResult` leaves `carrierAlive`/`hostAttachments` frozen as
  alive at `done`; a real worker exit would drop the carrier and release
  `tui`. The terminal state is optimistic here but supports no false
  property.
- `eventDrains <= 3` is an arbitrary bound; a real one-second step may drain
  more interleaved Host events. The bound only limits stutter depth.
- `CarrierLost` at a between-exchange phase folds "death discovered at the
  next write" into a lossless transition; the dominant real trace (zero-byte
  discovery inside `prepare`) is covered, while the write-halfway-failed
  sub-case is abstracted away.

## Inventory recommendation (for A6)

- Model name: `worker-send-transaction`
- Module: `apps/orchbee-worker/formal/tla/WorkerSendTransaction.tla`
- Config: `apps/orchbee-worker/formal/tla/WorkerSendTransaction.cfg`
- Owner record: `apps/orchbee-worker/docs/worker-send-transaction.md` (+ zh-CN)
- Source mapping: `src/terminal_send_operation.rs`, `src/terminal_operations.rs`
  dispatch arm, `src/host_connection/exchange.rs` (`exchange_fresh`,
  `PeerFailure`), Host carrier teardown.
- `requiredCoverage`: every action — `SendAttach`, `SendInput`, `SendDetach`,
  `HostApplyAttach`, `HostApplyInput`, `HostApplyDetach`, `HostTeardown`,
  `AttachExact`, `AttachRefused`, `AttachDesync`, `AttachMalformed`,
  `AttachLost`, `AttachLocalFault`, `InputExact`, `InputRefused`,
  `InputDesync`, `InputMalformed`, `InputLost`, `InputLocalFault`,
  `DetachExact`, `DetachRefused`, `DetachUnreachable`, `DetachLocalFault`,
  `ServerLostBeforeInputFact`, `ServerLostAfterInputFact`,
  `ServerLostAtResult`, `CarrierLost`, `PostAttachAbort`, `DrainHostEvent`,
  `DeliverResult`, `AbandonResult`.
- Author's control bound: 1 worker / 512 MiB / 120 s (observed: <1 s, 1834
  distinct states); the authoritative run uses the runner budget above.
- Known exclusions: pre-admission fence (deterministic, zero Host bytes;
  scripted test `send_fence_rejections_never_reach_the_host` owns it), fresh-ID
  collision freedom (trusted oracle, disclosed), real byte counts and wire
  encodings (pinned by the scripted-Host suite), process-level TUI isolation
  (only the shared-carrier boundary is modeled).

This model is registered in `scripts/tla/models.json` as
`worker-send-transaction` (owner `orchbee-worker`).
