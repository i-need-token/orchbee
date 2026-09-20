# Worker carrier scheduling model

Status: accepted Server-owned capacity slice. This reference governs the
`worker_obcn_carrier` submission, queueing, dispatch, drain and failure contract for the
serialized OBCN wire. The Worker-side viewer/controller capacities and frame-byte budget are
owned by the `orchbee-worker` application and are not settled here. Bounds below are dispatch-step
counts under continuously enabled conditions, not wall-time guarantees.

## Roles and states

One Worker generation owns one carrier. A single wire-owner thread serializes every exchange;
at most one request is in flight (the single-writer invariant). Producers are Client Session
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
fire-forget drop for completion-less cleanup detaches). There is no hidden or unbounded queue
behind the lanes.

## Caller lane map

The lane follows producer semantics, not the wire variant alone:

| Submit path                                                  | Variants                                                             | Lane | Admission                                          |
| ------------------------------------------------------------ | -------------------------------------------------------------------- | ---- | -------------------------------------------------- |
| `request()`, `submit_lifecycle()`, `validate_terminal_cwd()` | CwdValidation, Lifecycle, Control (non-cleanup), Launch, Start, Send | high | try-admit; full or waiter-present → `Backpressure` |
| `request()`                                                  | Viewer (Attach/Poll/ReadRows/feedback/explicit Detach)               | low  | try-admit; full → `Backpressure`                   |
| `cleanup_control()`                                          | Control Release                                                      | high | FIFO wait; priority over try-admit                 |
| `enqueue_detach()`                                           | Viewer Detach                                                        | high | FIFO wait; priority over try-admit                 |

The identical Viewer Detach variant is high when produced by session-drop cleanup and low when
produced by ordinary observation. Only the two existing cleanup producers block; every ordinary
producer try-admits and receives the existing typed `Backpressure` busy result.

## Admission

- Try-admit fails `Closed` on a closed queue; fails `Full` when the lane is at capacity —
  and, for the high lane only, while `high_waiters` is non-empty (waiter priority; the low
  lane consults only its own capacity). Otherwise it appends and notifies the owner. `Full`
  maps to `Backpressure` while the carrier is active, else `Unavailable`; `Closed` maps to
  `Unavailable`.
- Block-admit (high only) deposits the payload into `high_waiters` when the lane cannot accept
  it, then waits until the entry is transferred into `high` (admitted) or the queue closes.
  Waiters are bounded by blocked cleanup producers — at most one outstanding request per
  Client Session, so ≤ 32 resident waiter payloads under the current session bound.

## Dispatch

The wire owner picks under the same lock:

```
if consecutive_high < 4 and high non-empty → pop high; consecutive_high += 1
elif low non-empty                          → pop low;  consecutive_high = 0
else (low empty, high non-empty)            → pop high; consecutive_high += 1
```

Popping `high` frees its lane slot immediately (in-flight is counted separately) and transfers
the oldest `high_waiters` entry into the freed tail slot atomically, waking that producer.
With both lanes saturated this bounds consecutive high-priority dispatches to 4 before one low
dispatch; the reference model verifies per-request dispatch-step waits of ≤ 10 (high position
8: ⌈8/4⌉·5) and ≤ 40 (low position 8: 8·5), and waiter progress bounded by the producer count.
With no dispatchable work the owner waits on the condition variable up to the 25-millisecond
idle poll, then probes the stream as before.

## Drain and tombstone

Owner exit or destruction marks the carrier inactive, settles in-flight from its
complete-write marker (`Unavailable` before, `PostDeliveryOutcomeUnknown(CarrierStopped)`
after), then closes the queue and settles every queued and waiter request exactly once with
`Unavailable`; the in-flight request is never re-settled. `open_handles` reaching zero makes
`take_next` report `Closed` — but only after `dispatch` has offered every remaining queued
payload to the wire, so completion-less cleanup detaches still reach the worker at final
teardown — the owner then exits `Shutdown` and the same drain runs, the previous
all-senders-dropped semantics.
Completion-less (`None`) cleanup requests settle by drop; losing the reply receiver of an
in-flight request still retires the carrier conservatively. The `NotDelivered` versus
`PostDeliveryOutcomeUnknown` distinction across the complete-write boundary is unchanged.

## Lock discipline

Producers hold no registry, coordinator or service lock while block-admitting or awaiting a
result; the wire owner never waits on producer progress. Queue lock hold times are bounded by
bounded-deque operations; completion sends happen outside it on the drain path.

## Ownership accounting

Resident payloads: `high` ≤ 8, `low` ≤ 8, `high_waiters` ≤ 32 (producer-bound), in-flight 1 —
each a `PendingRequest` (encoded frame bytes + typed request + one `sync_channel(1)`
completion pair). Per-request result receivers live on their producer threads. No whole-process
memory bound is claimed here; this enumerates the queue-owned and producer-held copies.
