# Viewer capacity and retained record budget

`src/host_connection/viewers.rs` owns the Worker-side bounds for Server-issued
terminal viewers: at most `MAXIMUM_VIEWERS` (24) admitted attachments, at most
one retained projection record, four FIFO events and one terminal-settlement record per viewer, and an aggregate
`MAXIMUM_RETAINED_VIEWER_RECORD_BYTES` (128 MiB) budget over retained record
payloads. `src/host_connection/controllers.rs` keeps the separate
`MAXIMUM_CONTROLLERS` (8) grant bound, and `src/host_connection/target_cleanup.rs`
sizes each retired-target viewer tombstone array by the same `MAXIMUM_VIEWERS`
constant so a prepared cleanup still captures every viewer on its terminal.
These are implementation bounds for the authorized Worker capacity slice, not
release performance budgets and not a process RSS limit.

## Budget domain

- Counted: the logical `len()` of each `Vec<u8>` retained in a viewer's `record`
  slot, event FIFO and lifecycle slot — including `TerminalFrame` and `ProjectionBlocked` records. `Vec` capacity
  beyond `len` is not counted.
- Not counted: `flight` route metadata (it holds no payload), the temporary
  incoming `record: Vec<u8>` before the store decision (it remains governed by
  the wire `MAXIMUM_VIEWER_RECORD_BYTES` limit and is deallocated on return),
  records already handed to a session by `poll`, row-window reads, and any
  session-side or protocol-codec copy.
- `retained_record_bytes` is a private `HostViewers` counter; only owner code
  mutates it, always paired with a record, event or lifecycle slot transition. The ceiling lives
  in a private owner field defaulting to the constant so tests may exercise a
  smaller budget only through the real check — never by poking the counter.

## Transitions and exactly-once accounting

- Acquire: the two validated store sites in `receive` (TerminalFrame,
  ProjectionBlocked) admit a record only when
  `retained_record_bytes.checked_add(record.len())` stays within the ceiling;
  the checked total is assigned to the counter before `viewer.record` is set.
- Release: exactly once per stored record — `poll` take, `Viewer::close`
  record drop (queue saturation or aggregate pressure), or `remove` (explicit
  detach, failed-attach rollback, retired-target commit). The decrement runs on
  the same path that takes or drops the slot, so no record is counted or
  released twice.
- Closed viewers keep their admission slot until `remove`; their later Host
  events are still validated, then discarded without retaining.

## Pressure retirement

- Ordering inside `receive`: a malformed envelope, wrong kind/encoding,
  route/epoch disagreement, or an unknown attachment always fails the Host
  connection first. Closed-viewer records are then discarded, and an occupied
  delivery slot retires its viewer — both pre-existing semantics that run
  ahead of flow checks. Only for an active viewer with an empty slot do the
  projection-flow checks (sequence, snapshot requirement, blocked-revision
  advance) run; the aggregate-budget check follows them, so pressure never
  converts a flow-invalid record into a silent retirement — it still fails
  the Host connection with `InvalidData`. The Worker validates the envelope
  and routing prefix and retains the record opaquely; complete OBTF structure
  is the Client/protocol decoder's domain, not this owner's.
- A record that would push the aggregate above the ceiling retires only the
  addressed viewer through the existing `Viewer::close`: its record and flight
  are dropped, `closed` is set, and the ordinary `closed_ids` →
  `cleanup_closed` path detaches it from the Host before the next ordinary
  Server operation. Poll and feedback then return the existing
  `AttachmentUnavailable`; the Client may attach a fresh Server-issued
  identity. No other viewer is evicted, and neither the Host nor the retained
  terminal is harmed.
- Equality retains: `retained + incoming == ceiling` stores; only `>` retires.
  A single record can never self-retire because the wire record bound is far
  below the budget.

## Terminal settlement delivery

Kinds 15 and 19 are validated Host terminal facts, fanned out only to viewers of that terminal.
Each viewer keeps one replaceable lifecycle slot, charged to the aggregate budget and polled before
frame/event delivery without touching the frame flight. Replacement releases the old charge before
admitting the new one; poll, close and remove release it exactly once. Pressure retires only the
affected viewer. The four-event FIFO remains independent, so a burst cannot bury settlement behind
ordinary events or consume an additional unbounded queue.

After Host attach and projection-demand admission, the Worker reads bounded retained inventory
under its existing two-second observation budget, with at most eight pages of 128 entries. A settled
summary seeds the same lifecycle slot using the Host's identity, exit result and final revision.
The read validates page generation, cursor progress, unique terminal/creation identities and Host
epoch. It never changes terminal state. This supplies reentry evidence even when the original exit
notification preceded attachment. A missing or invalid target fails the exchange; it is never
presented as a running shell.

## Retired-target propagation

`RemovedTarget.viewers` is `[Option<OperationId>; MAXIMUM_VIEWERS]`, and
`HostViewers::target_ids` returns that same array type. Raising admission to 24
grows the tombstone identically, so a prepared cleanup captures all 24 viewers
that may be bound to one terminal with no truncation. Owner tests assert the
store's bound directly:
`size_of::<RemovedTarget>() * MAXIMUM_RETAINED_TERMINAL_BINDINGS <= 1 MiB` —
a separately bounded owner allocation, not a process-RSS claim.

## Evidence

Deterministic owner tests — the mechanism is sequential inside one broker, so
the evidence gap is accounting correctness, not interleaving:

- Admission: 24 viewers admitted, the 25th returns typed `CapacityExceeded`;
  the controller grant bound stays at 8.
- Accounting: both record kinds are charged by logical length; equality is
  retained; `poll`, `close`, `remove`, and retired-target commit each release
  exactly once.
- Pressure: only the addressed viewer retires, a sibling record stays
  untouched, the closed viewer discards future events, and poll/feedback
  return `AttachmentUnavailable`.
- Safety: a malformed envelope always returns `InvalidData`, and a
  flow-invalid record for an active empty-slot viewer still fails under
  pressure rather than retiring silently.
- Tombstone: a 24-viewer terminal's prepared cleanup captures all 24 ids.
- Boundary: one test exercises the real 128 MiB ceiling with frames grown
  through a valid optional extension record that the canonical
  `decode_terminal_text_frame` decoder accepts; smaller budgets reach the
  same check only through the owner limit field.

Session-held delivered records and the temporary incoming allocation are
outside this budget by construction and are not claimed bounded here.
