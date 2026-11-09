# Worker carrier scheduling model

Status: accepted Server-owned capacity slice. This reference governs the
`worker_obcn_carrier` submission, queueing, dispatch, drain and failure contract for the
serialized OBCN wire. The Worker-side viewer/controller capacities and frame-byte budget are
owned by the `orchbee-worker` application and are not settled here. Bounds below are dispatch-step
counts under continuously enabled conditions, not wall-time guarantees.

## Roles and states

One Worker generation owns one carrier. A single wire-owner thread serializes every exchange;
at most one request is in flight (the single-writer invariant). Producers are Client Connection
handler threads; each session has at most one outstanding request, so the producer count is
bounded by the active-session bound (32).

The carrier queue is one `Mutex`+`Condvar` cell holding:

- `high`: bounded FIFO, capacity 8.
- `low`: bounded FIFO, capacity 8.
- `high_waiters`: FIFO of blocked cleanup producers holding their deposited payloads.
- `consecutive_high`: dispatch burst counter.
- `open_handles`: live request-handle count, mirroring the previous sender liveness.
- `closed`: tombstone set only by owner teardown.

A request lives in exactly one place at a time: its producer thread (before admission),
`high_waiters`, `high`, `low`, the owner's in-flight slot, or settled (one completion send, or a
fire-forget drop for completion-less cleanup detaches, or a consumed-and-discarded reply for an
abandoned drain exchange). There is no hidden or unbounded queue behind the lanes.

## Caller lane map

The lane follows producer semantics, not the wire variant alone:

| Submit path                                                  | Variants                                                             | Lane | Admission                                          |
| ------------------------------------------------------------ | -------------------------------------------------------------------- | ---- | -------------------------------------------------- |
| `request()`, `submit_lifecycle()`, `validate_terminal_cwd()` | CwdValidation, Lifecycle, Control (non-cleanup), Launch, Start, Send | high | try-admit; full or waiter-present → `Backpressure` |
| `request()`                                                  | Viewer (Attach/Poll/ReadRows/feedback/explicit Detach)               | low  | try-admit; full → `Backpressure`                   |
| `cleanup_control()`                                          | Control Release                                                      | high | FIFO wait; priority over try-admit                 |
| `enqueue_detach()`                                           | Viewer Detach                                                        | high | FIFO wait; priority over try-admit                 |
| `admit_agent_drain()`                                        | AgentDrain                                                           | low  | try-admit; full → `Backpressure`                   |
| `admit_title_changes()`                                      | TitleChanges                                                         | idle | try-admit; one slot, full → `Backpressure`         |

The identical Viewer Detach variant is high when produced by session-drop cleanup and low when
produced by ordinary observation. Only the two existing cleanup producers block; every ordinary
producer try-admits and receives the existing typed `Backpressure` busy result. The
`admit_agent_drain` seam is production: the Server drain-read pump try-admits a typed
AgentDrain request
through the same worker/generation fence and receives a `PendingExchange` whose bounded
`recv_deadline` wait keeps the attempt alive when it elapses, so the pump
waits with its own deadline instead of `submit`'s blocking receive. The Server's title read pump
admits its one outstanding change read the same way, with discardable completion.

## Admission

- Try-admit fails `Closed` on a closed queue; fails `Full` when the lane is at capacity —
  and, for the high lane only, while `high_waiters` is non-empty (waiter priority; the low
  lane consults only its own capacity). Otherwise it appends and notifies the owner. `Full`
  maps to `Backpressure` while the carrier is active, else `Unavailable`; `Closed` maps to
  `Unavailable`.
- Block-admit (high only) deposits the payload into `high_waiters` when the lane cannot accept
  it, then waits until the entry is transferred into `high` (admitted) or the queue closes.
  Waiters are bounded by blocked cleanup producers — at most one outstanding request per
  Client Connection, so ≤ 32 resident waiter payloads under the current session bound.

## Dispatch

The wire owner picks under the same lock:

```
if consecutive_high < 4 and high non-empty → pop high; consecutive_high += 1
elif low non-empty                          → pop low;  consecutive_high = 0
else (low empty, high non-empty)            → pop high; consecutive_high += 1
else (both empty)                           → take idle
```

The idle lane holds at most one title change read and is taken only when both lanes and the
high waiters are empty, so a title read never goes ahead of input, a viewer poll or an Agent
drain, and leaves their bounds unchanged. Once taken it occupies the wire for one bounded
exchange: the Worker answers it from memory, at most 256 rows and 1 MiB of titles.

Popping `high` frees its lane slot immediately (in-flight is counted separately) and transfers
the oldest `high_waiters` entry into the freed tail slot atomically, waking that producer.
With both lanes saturated this bounds consecutive high-priority dispatches to 4 before one low
dispatch; the reference model verifies per-request dispatch-step waits of ≤ 10 (high position
8: ⌈8/4⌉·5) and ≤ 40 (low position 8: 8·5), and waiter progress bounded by the producer count.
With no dispatchable work the owner waits on the Worker stream and a private wake together, for
at most its 250-millisecond idle interval. Every queue change signals the wake, and the wake's
pending flag is cleared only after its bytes are read, so a signal during that read is seen by the
owner's next queue inspection. A carrier without a wake bounds this wait at 2 milliseconds.

The only unsolicited kinds are `TerminalViewerRecordReady` and `TerminalTitlesChanged`, read while
idle and while a response is awaited. The owner routes each viewer hint, and each title note to the
Session title registry for its own generation, and keeps waiting; a wire without a registry drops
the note, which costs only latency. A hint never completes a request and changes no dispatch
position, so the bounds above are unchanged; any other unsolicited bytes still retire the carrier.

## Drain and tombstone

Owner exit or destruction marks the carrier inactive, settles in-flight from its
complete-write marker (`Unavailable` before, `PostDeliveryOutcomeUnknown(CarrierStopped)`
after), then closes the queue and settles every queued and waiter request exactly once with
`Unavailable`; the in-flight request is never re-settled. `open_handles` reaching zero makes
`take_next` report `Closed` — but only after `dispatch` has offered every remaining queued
payload to the wire, so completion-less cleanup detaches still reach the worker at final
teardown — the owner then exits `Shutdown` and the same drain runs, the previous
all-senders-dropped semantics.
Completion ownership is one of `Required`, `DiscardableDrain` or `Detached`, fixed by the
admitted request kind at admission construction: `Detached` cleanup requests settle by drop;
losing the reply receiver of an in-flight `Required` request still retires the carrier
conservatively; an abandoned `DiscardableDrain` exchange consumes and discards the valid reply
so healthy carrier traffic survives — malformed frames, wrong kinds or directions, echo
mismatches and transport failures stay fatal exactly as before. The `NotDelivered` versus
`PostDeliveryOutcomeUnknown` distinction across the complete-write boundary is unchanged.

## Drain target

`WorkerRegistry::agent_drain_carrier()` answers the drain-scoped carrier
question `request_handle()` cannot: `NoCarrier` only when `active == None`;
`CarrierUnavailable` for a pending admission, a present-but-inactive carrier record, or a
carrier whose identity cannot be named on the drain wire (CNP rejects an all-zero
`WorkerId`); `Live` carrying the cloned request handle, the full CNP `WorkerId` +
`WorkerGeneration` identity pair, and the shared per-carrier `AgentDrainMint`.

The mint is born at successful carrier registration and dies with the record. `mint()`
issues the next drain token through a checked compare-and-swap — at `u64::MAX` it returns
`None` and leaves `last_issued` untouched: exhaustion is a value answer, never a carrier
retirement and never token reuse. `floor()` peeks the next would-be token without issuing
it. A cloned target may outlive retirement only alongside a dead identity — the mint is
not a liveness proof; the handle's admission fence is. The production consumer is
the `LocalTerminalService` prepared-read provider (`agent_drain_read`).

## Native leaf

`WorkerRegistry::register_carrier_with_agents()` runs the same registration
path as `register_carrier()` — identical checks, identical publication — plus the canonical
`AgentService` native leaf attached to the outcome. While the registry guard is held, the
exact CNP `WorkerId` + `WorkerGeneration` identity is derived from the admitted worker and
the registry-minted generation (the same pair `agent_drain_carrier()` re-derives), then
`AgentService::native_admit_carrier()` runs inside the same guard. A contract refusal or a
poisoned cell fails the registration closed: the carrier record is never installed, so a
readable Live carrier always implies its leaf was admitted.

The registration carries the leaf as `{identity, Weak<AgentService>}`. On teardown the wire
owner still settles first; the registration guard then retires the leaf inside the registry
guard — before `retire_carrier` frees the slot — so a replacement can never be exposed
between leaf retirement and slot release. A dead service (weak upgrade fails) or a poisoned
cell (typed `Err`) cannot block registry retirement, and a stale registration's old identity
cannot retire a replacement's leaf: both the leaf's full-identity check and the registry's
generation check refuse it. No mutex is held across socket I/O, and the unmanaged
`register_carrier()` keeps its behavior untouched — it is the same
`register_carrier_impl` without a service argument. The managed listener path wires the
seam in production: `local_worker_service` calls `register_carrier_with_agents` when the
shared `AgentService` exists and plain `register_carrier` otherwise; lifecycle authority
stays at the session composition root.

## Lock discipline

Producers hold no registry, coordinator or service lock while block-admitting or awaiting a
result; the wire owner never waits on producer progress. Queue lock hold times are bounded by
bounded-deque operations; completion sends happen outside it on the drain path.

## Ownership accounting

Resident payloads: `high` ≤ 8, `low` ≤ 8, `high_waiters` ≤ 32 (producer-bound), in-flight 1 —
each a `PendingRequest` (encoded frame bytes + typed request + one completion endpoint under
the `Required`/`DiscardableDrain`/`Detached` ownership policy). Per-request result receivers
live on their producer threads. No whole-process memory bound is claimed here; this enumerates
the queue-owned and producer-held copies.

## Windows handoff admission fence

`WorkerHandoffRequest` uses the high lane and an exact Worker/generation/target echo.
Prepare changes the queue from Normal to one fenced ticket under the queue lock. All queued
high/low requests and blocked cleanup payloads settle as unwritten Unavailable outside the lock.
The already dispatched exchange completes before Prepare reaches the wire. Normal admission and
concurrent Shutdown cannot enter this fence. At most one matching handoff exchange is queued or
dispatched. This closes transport admission, not the higher coordinator's canonical-state writers.

Prepared keeps the fence. Successful Abort, known NotPrepared, failed Prepare with no other
Prepared owner, or Commit's TargetUnavailable restores Normal. Busy and TicketMismatch cannot
prove absence and retain the fence. Transport loss after a complete write remains Unknown; it
never opens admission or authorizes replay. Committed closes the old queue, retires its active
carrier and settles any remaining payloads before another write. The importer admits its new
carrier only after exact Owned on its standby wire; the Worker generation remains unchanged.

The [Worker model](../../orchbee-worker/docs/windows-server-handoff.md) owns standby admission,
the 15-second lease, physical resources and expiry. A known expiry is reconciled by matching
Abort/Commit -> NotPrepared; this queue cannot renew or infer the Worker lease. Native pipe and
deterministic scheduling witnesses cover in-flight ordering, blocked cleanup release, exact echoes,
rollback admission and committed retirement. Canonical import and the public live-handoff result
require Server integration beyond this queue boundary.
