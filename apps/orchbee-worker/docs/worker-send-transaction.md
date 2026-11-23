# WorkerSendTransaction bounded model

`formal/tla/WorkerSendTransaction.tla` checks the Server-authorized send lane implemented by `src/terminal_send_operation.rs` and dispatched from `src/terminal_operations.rs`. Its concurrency gap is independently ordered Host responses, carrier loss and Server loss. Single-command authorization and lifecycle receipts use implementation tests; Send does not enter their ledger and takes no kernel argument before `admit_launch`/`admit_start`.

## Scope and trusted assumptions

- One already-admitted `TerminalSendRequest` over one shared Host carrier. The
  request pins a fixed 48-byte `HostTerminalIdentity` plus a Server-minted
  `attachment_id`; the model keeps a single `Target` and treats identity
  checking as exact. The pre-admission fence (wrong worker, stale generation,
  stale epoch, disposed terminal, an idle-prompt request whose Host did not
  negotiate `prompt-gated-input`, a screen-gated request whose Host did not
  negotiate `screen-gated-send`, negotiated-limit excess) is a declared
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
- Post-attach local abort (`?` propagation in `apply`/`step_command` after a
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
- Self-built encode failures: the attach answer is `Unavailable` with no progress, and an input
  record that cannot be built is a determinate stop after the confirmed prefix, with the attachment
  detached. Both reuse the shapes of the existing `LocalFault` branches, which the model covers as
  `AttachLocalFault` and `InputLocalFault`. A built outcome the protocol refuses is answered
  `OutcomeUnknown` with cleanup unconfirmed and logged as `send-outcome-unbuildable`; that fallback is
  defensive and outside the model, since no current constructor produces an invalid outcome.
- `exchange_fresh` per step → `SendAttach`/`SendInput`/`SendDetach` plus the
  response family: `*Exact`, `*Refused` (TypedFailure), `*Desync` (an intact
  answer that contradicts the stage: another message, an envelope outside its
  contract, an echo mismatch or a bound violation), `*Malformed` (a record
  whose framing is lost), `*Lost`
  (EOF/write failure/step deadline), `*LocalFault` (pre-write, or no room left
  in the request), and `ServerLost`/`CarrierLost` peer events. `ServerLost*`
  also stands for a Server request observed during the send. The Server writes
  none while a reply is owed, even after its window, so only a broken Server or
  a real reply that follows a frame the Server already took as this send's
  answer leads there; the Worker treats it as Server loss and ends the session,
  so the next send never starts behind this one and no input is resent. An
  intact answer
  never retires the carrier: `AttachDesync` refuses the send and `InputDesync`
  settles it unknown, and both detach the attachment (`detach-pending`), as
  `InputLocalFault` does. At detach, `DetachRefused` confirms cleanup like
  `DetachExact`: the Host's Detach is idempotent, so it refuses only an
  attachment it no longer holds (a trusted Host assumption). `DetachMismatch`,
  another intact answer, leaves the attachment `deferred` to a later cleanup on
  the kept carrier. Only `*Malformed` and `*Lost` retire the carrier, as
  `DetachUnreachable` does at detach. A detach-stage LocalFault is a pre-send
  failure (`DetachLocalFault`, `detachSent` stays 0) deferred the same way — it
  must not be written as an emitted detach.
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
`ConfirmedRequiresDetachAck`, `ConfirmedMeansReleased`, `AttachRefusalNeedsNoDetach`,
`UntrustedStreamNoDetach`, `IntactAnswerKeepsCarrier`, `UnconfirmedCleanupIsHandedOn`,
`TuiLossOnlyViaCarrierDeath`, `SendAttachmentMatchesAdmission`,
`NoResultOnEarlyServerLoss`, `ResultProducedOnce`, `*AtMostOnce` per stage.
Temporal: `EventualSettlement`, `TeardownEventuallyReleasesAttachments`,
`DetachOnlyAfterSettledStage`, `UnknownNeverResends`,
`AcceptedNeverDowngraded`, `AcceptedBytesStable`. Progress is weak fairness on
`Next` — every continuously enabled resolution (reply, expiry, loss, or local
settle) may fire; no peer is assumed to answer successfully.

`PairingMatrix` applies only once settled (`result-ready`/`done`): cleanup is
`none` while the detach axis is unresolved. Only an answer whose framing is lost, a lost answer,
carrier loss, Server loss and the post-attach abort retire the carrier; every
settlement reached through an intact answer or an unsent record keeps it, and
unconfirmed cleanup at `done` implies a dead carrier or a `deferred`
attachment.

## Bounds and evidence consumers

One request, one terminal pair, one TUI attachment, byte domain 0..2
(`acceptedBytes` is a singleton set once the input is acked, `{}` before),
per-stage send counters ≤1, event drains ≤3. Refinement consumers are the
scripted-Host suite (`tests/process/terminal_operations/send.rs`, 12 cases)
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

Outcome: exit 0, 4872 generated / 3400 distinct states, depth 15, every
action covered (no zero-coverage line). The batch evidence directories
`a3/evidence-13/` and `a3/evidence-14/` predate the revision in which an
intact answer keeps the carrier.

The authoritative run uses one TLC worker and the registry's configuration
deadline, as `scripts/tla/models.json` and `scripts/tla/run-models.mjs` set them.

## Negative controls

Mutant modules are produced out-of-tree, each one guard or action edit, each
run separately with the configuration above. The current revision's controls:

- `refused-while-held` — `DetachRefused` is enabled while the Host still holds
  the attachment: violates `ConfirmedMeansReleased` (exit 12).
- `desync-retires-carrier` — `InputDesync` retires the carrier: violates
  `IntactAnswerKeepsCarrier` (exit 12).
- `mismatch-not-deferred` — `DetachMismatch` drops the deferral: violates
  `UnconfirmedCleanupIsHandedOn` (exit 12).
- `desync-resends-input` — `SendInput` is enabled again after an unknown
  input: `inputSent = 2` is caught by `InputAtMostOnce`, not `TypeOK`
  (exit 12). This is the load-bearing replay check — a counter bit would make
  the same guard mutation stutter invisibly.

The earlier batches' controls (`m1`–`m4` under `a3/evidence-13/` and
`a3/evidence-14/`) ran against the previous revision; its
`m1-detach-after-desync` is now the specified behavior.

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
- `eventDrains <= 3` is an arbitrary bound; a real step, bounded at ten seconds, may drain
  more interleaved Host events. The bound only limits stutter depth.
- `CarrierLost` at a between-exchange phase folds "death discovered at the
  next write" into a lossless transition; the dominant real trace (zero-byte
  discovery inside `prepare`) is covered, while the write-halfway-failed
  sub-case is abstracted away.
- A `deferred` attachment is detached by a later request's cleanup, or ahead
  of a later Acquire or Takeover, outside this one-request model. The Worker's backlog of eight deferred Detaches,
  which refuses new sends while full, is not counted here.
- A step with no room left in the request or past the send's input budget is
  modeled as `*LocalFault`; the implementation reports it as
  `InputBudgetExhausted` rather than `HostUnavailable`, a reason the model
  abstracts.

## Inventory recommendation (for A6)

- Model name: `worker-send-transaction`
- Module: `apps/orchbee-worker/formal/tla/WorkerSendTransaction.tla`
- Config: `apps/orchbee-worker/formal/tla/WorkerSendTransaction.cfg`
- Owner record: `apps/orchbee-worker/docs/worker-send-transaction.md`
- Source mapping: `src/terminal_send_operation.rs`, `src/terminal_operations.rs`
  dispatch arm, `src/host_connection/exchange.rs` (`exchange_fresh`,
  `PeerFailure`), Host carrier teardown.
- `requiredCoverage`: every action — `SendAttach`, `SendInput`, `SendDetach`,
  `HostApplyAttach`, `HostApplyInput`, `HostApplyDetach`, `HostTeardown`,
  `AttachExact`, `AttachRefused`, `AttachDesync`, `AttachMalformed`,
  `AttachLost`, `AttachLocalFault`, `InputExact`, `InputRefused`,
  `InputDesync`, `InputMalformed`, `InputLost`, `InputLocalFault`,
  `DetachExact`, `DetachRefused`, `DetachMismatch`, `DetachUnreachable`,
  `DetachLocalFault`,
  `ServerLostBeforeInputFact`, `ServerLostAfterInputFact`,
  `ServerLostAtResult`, `CarrierLost`, `PostAttachAbort`, `DrainHostEvent`,
  `DeliverResult`, `AbandonResult`.
- Author's control bound: 1 worker / 512 MiB / 120 s (observed: about 1 s,
  3400 distinct states); the authoritative run uses the runner budget above.
- Known exclusions: pre-admission fence (deterministic, zero Host bytes;
  scripted test `send_fence_rejections_never_reach_the_host` owns it), fresh-ID
  collision freedom (trusted oracle, disclosed), real byte counts and wire
  encodings (pinned by the scripted-Host suite), process-level TUI isolation
  (only the shared-carrier boundary is modeled).

This model is registered in `scripts/tla/models.json` as
`worker-send-transaction`.

## Idle-prompt refinement

An idle-prompt request (`SendPrecondition::IdlePrompt`) carries exactly one prompt line: a paste, an
optional Right, and Enter. The Worker sends the whole line as one Host `PromptLineInput`, so the
model's single abstract input action covers it unchanged: one attach, one input exchange, one detach.
A Host answer whose `worker-host` handshake lacked `prompt-gated-input` is refused in the fence as
`HostPromptGateUnsupported` with not-needed cleanup; the line is never split into ordinary records.
The Host's `prompt-unconfirmed` refusal is `InputRefused`: it types nothing, so the result is
`PromptUnconfirmed(state)` with zero progress, and the attachment is still detached. The Worker
returns `settling` as is instead of waiting the quiet period: the Server carrier serves one request
at a time and also carries interactive typing, so a wait would stall it. The caller that chose the
precondition owns the single retry. Evidence is the in-crate scripted Host
(`host_connection::tests::prompt_line_send`): the one-exchange line, the settling refusal, and the
fence refusal with no Host bytes.

## Screen-gated refinement

A screen-gated request carries one unmodified Enter or the prompt line shape above. A hold carries
the Enter shape and queues zero bytes. The Worker sends the whole request
as one Host `ScreenGatedKeyInput` between the transient attach and detach. The prompt form also
requires `screen-gated-prompt` from the Host. A Host whose `worker-host`
handshake lacked `screen-gated-send` is refused in the fence as `HostScreenGateUnsupported` with
not-needed cleanup; `screen-gated-input` alone, which the Hosts of 0.1.0-beta.22 answer, is refused
the same way, and the Worker's offer does not list it. The Host's `screen-*` refusals are `InputRefused`, mapped to
`ScreenRefused(state)`: they type nothing, so the result has zero progress and the attachment is
still detached. Evidence is the in-crate scripted Host (`host_connection::tests::screen_gated_send`):
the one-exchange key and prompt, the six refusals, the hold, and the fence refusals with no Host bytes.

## Semantic stroke refinement

The ordered send implementation maps each validated stroke to one semantic Host key press. Key identity, text, consumed Shift and Super affect encoding, not Attach/Input/Detach ownership or ordering. Host chooses bytes under its terminal lock using current modes; an encoded zero-byte key remains valid. These deterministic mappings refine the model's abstract input action, with parser/codec/Worker/Host boundary regressions as evidence. The abstract model does not prove keyboard encodings or multi-record byte semantics.
