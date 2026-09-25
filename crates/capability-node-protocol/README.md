# Capability Node Protocol

Private, experimental Rust semantic contract for authorized terminal operations and bounded
directory observations between Orchbee Server and a Capability Node. The initial Capability Node
is the Worker, but this crate does not bind the messages to a local socket or any other carrier. The
local Server adapter now consumes its codec in an application-private post-OBLS carrier. The local
Server coordinator submits authorized Create/Start and generation-pinned cwd validation through
that carrier. It is not a public API or a compatibility promise.

Simplified Chinese maintenance translation:
[README.zh-CN.md](README.zh-CN.md).
English remains canonical.

## Local Close lifecycle step

[Canonical terminal Tab Close](../../apps/orchbee-server/docs/terminal-tab-close.md) owns the accepted
Server continuation and exact Worker/Host cleanup contract. Its OBCN extension is a narrow
authorized lifecycle step carrying product binding, Worker generation, full Host identity and
operation/step correlation. Each step admits at most one existing Host request; Viewer/controller
authority alone never grants termination. Host target-bearing acknowledgments and the full OBCN
result correlation must be validated before retaining effects or publishing Disposed.

Worker retains exact lifecycle observations and cleans only the disposed target's viewer/controller
and queued feedback/frame entries before returning Disposed. Server retains exact target revocation
and clears its matching reservations; lost acknowledgments preserve known/unknown facts at the owner
that observed them. New Control Acquire is fenced with Attach/Input/Resize/Start. Close keeps the
existing 1s Host exchange, 2s Server frame I/O and separate 20s response wait; the Server's 30s
attempt budget only gates next-step admission and cannot discard an outstanding receipt.

The lifecycle codec and Server carrier correlation implement the following bounded representation.
Worker execution and Server continuation remain pending; this codec does not implement replay,
admission, cleanup, or a whole inventory walk. Protocol, Server, Worker and native Close activate
together. Old private cohorts reject the additive tags; no mixed-cohort fallback is defined.

Kind 9 `TerminalLifecycleRequest` and kind 10 `TerminalLifecycleResult` use frame version 2 and
the 8 KiB payload bound. All integers are unsigned big-endian and IDs are nonzero 16-byte values.
The request body after its kind is, in order: lifecycle capability version (`u16`, current 1),
product Terminal ID (16), Worker ID (16), Worker generation (`u64`, positive), retained binding fence
(`u64`, positive), fresh lease ID (16), lease Unix-millisecond expiry (`u64`, positive), Close
operation ID (16), step request ID (16), attempt generation (`u64`, positive), step ordinal (`u8`,
1 through 32), Host creation/epoch/terminal IDs (16 each), and action (`u8`). The lifecycle
capability is distinct from launch capability version 2. The fence references the exact current
retained Launch/Start binding without replacing it. The fresh lifecycle lease is independent of the
original launch lease. Viewer/controller authority is absent. The authenticated Server issues fresh
step request IDs; codec validation alone does not establish authorization or replay safety.

Action tags 1 through 4 are Terminate, Force, Dispose, and InventoryPage. InventoryPage adds a
cursor option tag (`0` absent, `1` present); a present cursor adds positive inventory generation
and offset (`u64` each). Every Host query requests the fixed maximum of 128 terminals. A result
echoes the complete request body, then an outcome tag: 1 TerminateAccepted, 2 ForceAccepted,
3 Disposed, 4 InventoryPage, 5 Rejected, 6 HostFailure, 7 InventoryFailure, 8 OutcomeUnknown,
or 9 HostUnavailableBeforeHandoff (no additional payload).
Constructors and decoding reject action/outcome contradictions. Host success validates the ACK's
actual terminal ID plus envelope role, kind, epoch and correlation; its retained full request seal
supplies the product, creation, Worker and attempt identities that the Host ACK does not echo.

InventoryPage adds positive inventory generation, optional next cursor, returned row count (`u16`,
0 through 128), and target option (`0` NoMatch, `1` Match). Match adds revision (`u64`) and phase:
1 AwaitingStart, 2 Running, 3 Finishing, 4 ContainingFailure, 5 Exited, 6 Failed. Running, Finishing
and Exited add the exact `terminateRequested` boolean (`0` or `1`); other phases carry none.
Match requires a positive returned count. The count covers all scanned Host summaries, not matches.
Successful page generation matches a supplied request cursor; any next cursor shares that
generation and advances exactly by returned count without overflow. NoMatch is page-local, never
absence. Worker validates full Host pages and fences duplicate or contradictory terminal-ID or
creation-ID matches before projecting the exact target. Server owns cross-page consistency,
duplicate matches, complete exhaustion, absence and continuation. Finishing and ContainingFailure
are not settled outcomes. No raw Host inventory, theme, configuration or unrelated identity enters
this fixed-size projection.

Rejected adds a reason tag 1 through 8: WrongWorker, StaleWorkerGeneration, UnsupportedCapability,
LeaseExpired, BindingUnavailable, BindingFenceMismatch, HostIdentityMismatch, LifecycleConflict.
These are pre-handoff refusals. LifecycleConflict conservatively includes a different Close owner,
an older step and a conflicting step; it never authorizes replay of an ambiguous mutation.
HostFailure reason tags 1 and 2 are HostRefused and CleanupIncomplete; CleanupIncomplete is legal
only for Dispose. Correlated Host typed failures are known observations, not proof of no effect.
InventoryFailure, legal only for InventoryPage, adds tags 1 through 4: StaleCursor, PageRejected,
InvalidPage, IdentityConflict. The first two classify correlated Host failures; the latter two
classify contradictory page observations. OutcomeUnknown adds tags 1 through 3: TransportLost,
Deadline, InvalidResponse. Query unknowns map to inventory-incomplete in the Server workflow;
mutation unknowns retain the affected lifecycle phase. No arbitrary error text is serialized.

HostUnavailableBeforeHandoff is legal for every action and reports known zero lifecycle Host
requests. It does not undo the already admitted Server-to-Worker step or its continuation charge.
Worker emits it only before the first possible lifecycle Host write, including failure during
`HostExchange::prepare`; `round_trip` owns the subsequent write boundary. Server maps this known
unavailability to CleanupBlocked(worker-unavailable), never HostFailure or OutcomeUnknown.

Lifecycle requests are at most 182 payload bytes and results at most 221 (190 and 229 framed bytes).
Hand-derived golden bytes, all outcome/reason round trips, strict prefixes and malformed bounds,
action/page contradictions, exact maximum lengths, and Server correlation-mutation tests verify
this representation. They do not prove freshness, Host execution, cache cleanup, elapsed budgets,
replay policy or asynchronous progress; those obligations remain with the owning Close models and
runtime refinement. Rollback replaces the private cohort together and migrates no durable data.

Launch rejection tag 9 and Start rejection tag 10 add `LifecycleActive`. Exact original cached
Launch/Start replay remains readable; a different operation, including a higher fence, cannot
replace a binding owned by Close. This refusal is not a stale-fence or Host-failure observation.
Server maps it to the existing terminal-runtime-unavailable outcome. Lifecycle mutation Unknown
remains sticky across attempts; no new lease or higher attempt authorizes another mutation.
Inventory representation alone creates no Server reconciliation or Resume path for Unknown.

## Terminal cwd validation messages

OBCN kind 11 `TerminalCwdValidationRequest` and kind 12 `TerminalCwdValidationResult` carry one
read-only, generation-pinned directory observation on the existing serialized Server-Worker
carrier. A request contains operation ID (16 bytes), Worker ID (16), positive Worker generation
(`u64`), cwd byte length (`u16`) and one required `AbsoluteCwd`. A result echoes that complete
request and adds one outcome tag: 1 `DirectoryAvailable`, 2 `DirectoryUnavailable`, or 3
`WorkerUnavailable`. All integers retain the existing unsigned big-endian encoding.

The accepted [Workspace terminal directory contract](../../apps/orchbee-server/docs/workspace-terminal-directory.md)
owns the association that consumes this observation.

The codec validates nonzero identities and generation, absolute UTF-8 cwd length and spelling,
exact payload length, and the closed outcome set. It does not access the filesystem, resolve a
symbolic link, create a directory, authorize association, or establish that a Worker is current.
The Worker owns the point-in-time metadata observation; the Server carrier validates the full echo
and owns its existing bounded queue, deadlines and connection-loss classification. The message has
no terminal identity, lease, Host command, launch authority or durable effect. Protocol, Worker and
Server activate kinds 11 and 12 together in the same unpublished local cohort.

## Framing and messages

One canonical frame contains the fixed `OBCN` magic, an unsigned big-endian frame version, an
unsigned big-endian payload length, and at most 8 KiB of payload. The frame codec rejects unknown
magic, version, message or enum tags, malformed or incomplete values, excess bytes, and oversized
payloads. Frame version 1 and terminal-launch capability version 2 are separate version domains.
Bounded streaming helpers read the complete eight-byte header before validating it and allocating
the declared payload, then read exactly that payload; writes tolerate partial progress. I/O failure
is distinct from a syntactically invalid protocol frame. Writing one frame does not flush a
caller-owned buffer; that carrier policy remains with the adapter.

Capability version 2 adds the generation-local Worker binding bound and its typed capacity result
without changing the frame envelope. A Worker admits only its exact supported capability version;
version 1 and version 2 requests therefore reject each other as `UnsupportedCapability`. Both
versions use the same rejection tag for that outcome, while a version 1 decoder rejects the new
capacity tag as unknown. Mixed versions consequently fail closed instead of silently using version
1's unbounded binding behavior. The current Worker authorization kernel is fixed to version 2
rather than accepting a configurable version with version 2 capacity behavior. Construction and
decoding also reject `CapacityExceeded` when its echoed authorization is not version 2.

| Message                    | Direction                 | Closed semantic payload                                                                                                                                                                            |
| -------------------------- | ------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `AuthorizedTerminalLaunch` | Server -> Capability Node | Complete operation authorization, optional bounded node-local current directory, either `Immediate` with complete pixel geometry or `Deferred`, and an optional bounded explicit launch selection. |
| `TerminalLaunchResult`     | Capability Node -> Server | The exact authorization plus `HostAccepted`, `Rejected`, `Failed`, or `OutcomeUnknown`; every post-handoff outcome preserves the observed Host epoch.                                              |
| `AuthorizedTerminalStart`  | Server -> Capability Node | A separately fenced and leased authorization, complete target Host terminal identity, and complete pixel geometry for a previously deferred terminal.                                              |
| `TerminalStartResult`      | Capability Node -> Server | The exact authorization and target Host identity plus `HostAccepted`, `Rejected`, `Failed`, or `OutcomeUnknown`; success is explicitly `Started` or `AlreadyStarted`.                              |

Every authorization binds the product terminal ID, operation ID, protocol-owned Worker ID,
nonzero Worker generation, nonzero per-terminal monotonic fence, stateful lease ID and nonzero
absolute Unix-millisecond expiry, and explicit capability version. All protocol-owned IDs are
nonzero opaque 128-bit values. Results echo the complete authorization so Server can discard stale
generation, fence, operation, lease, or capability observations.

Operation identity also has a producer invariant that cannot be established by decoding one frame:
the authenticated authoritative Server allocates every `OperationId` globally fresh within its
active authority, across all Terminal IDs, Workers, and Worker generations, and never assigns one
identity to two different authorizations. A successor Server authority may begin a new producer
scope only after its authority transition makes all earlier authorizations non-current through
generation, lease, and fence validation. This contract does not specify that transition or require
a Capability Node to retain an unbounded cross-terminal tombstone ledger; Worker refinement assumes
well-formed issuance from the authenticated authority and does not claim to contain a violating
issuer.

The optional current directory is an absolute local-platform UTF-8 path of 1 through 4,096 encoded bytes and
cannot contain NUL. Its absence is distinct from an empty value. Geometry contains columns, rows,
cell-pixel width, and cell-pixel height. Columns and rows are in `1..=4096`; cell-pixel width and
height are in `0..=4096` and must be either both positive (pixels known) or both zero (pixels
unknown), never mixed. The grid is at most 262,144 cells, and each total pixel extent fits `u16`. These types are Capability Node refinements, not
imports or re-exports of Terminal Host wire payloads; a future Worker adapter must convert them
explicitly and preserve validation.

The optional launch selection is a closed tail after the launch mode: a `u16` executable length,
the executable bytes, a `u16` argument count, then `u16`-length-prefixed argument strings. The
executable is a nonempty absolute UTF-8 path without NUL; arguments are NUL-free UTF-8 strings
(empty and dash-leading values allowed) with at most 128 entries and at most 2,048 UTF-8 bytes
across the executable and all arguments combined. A payload that ends at the mode block decodes as
the default launch; a present tail is validated with complete consumption, so partial or trailing
bytes reject. The tail carries `argv[1..]` only — `argv[0]` is the Host's synthesis from the
executable path — and absence is distinct from an explicit empty argument list.

## Read-only viewer messages

The Worker now consumes OBCN kind 5 `TerminalViewerRequest` and emits kind 6
`TerminalViewerResult`. Server now submits these messages for its admitted Client Sessions alongside Launch/Start. These additive messages remain private and unshipped.
An older decoder rejects the new tags or magic; no fallback or mixed-cohort compatibility is claimed.

Every request contains a fresh request `OperationId`, exact Worker ID and nonzero generation, and a
Server-issued attachment `OperationId`. Server allocates attachment identities fresh across its
active authority and never reuses them after detach. They name independent transient viewers, not
product Terminal identities or Host creation identities. The authenticated Server alone grants
read access; Worker checks the admitted Worker/generation and exact Host epoch before attaching.
This path grants neither input nor resize, and never claims the Host resize lease.

The closed actions are `Attach` with Host epoch and Host terminal ID, `Poll`, `FrameApplied` with
positive sequence/generation and revision, `NeedSnapshot` with positive sequence and the existing
eight snapshot reasons, `Detach`, and `ReadRows`. Kind 5 lays out request ID (16 bytes), Worker ID (16),
generation (8), attachment ID (16), then action tag (1). Tags 1 through 6 select those actions in
that order. Attach adds two 16-byte identities; FrameApplied adds three 8-byte integers;
NeedSnapshot adds an 8-byte sequence and reason tag 1 through 8: `ActivityStale`, `BaseMismatch`,
`DecoderRejected`, `DimensionsMismatch`, `GenerationMismatch`, `IdentityMismatch`, `SequenceGap`,
and `StoreReset`. `ReadRows` adds big-endian `startRow` (u64), `rowCount` (u16, 1 through 4096),
`revision` (u64), positive `generation` (u64), positive `requestSequence` (u64), and
`maximumBytes` (u32, 112 through 8,388,608 — an OBRW payload bound, not a record bound); its
attachment stays on the request envelope and is not repeated. Tags 1 through 5 are unchanged
byte-for-byte.
All integers are unsigned big-endian. Kind 6 contains request and attachment IDs followed by
`Attached`, `Empty`, `FeedbackAccepted`, `Detached`, `Rejected`, or `RowsRejected` tags 1 through 6.
Rejection tags 1 through 8 are wrong Worker, stale Worker generation, stale Host epoch, capacity
exceeded, attachment unavailable, attachment already exists, feedback mismatch, and Host rejected.
`RowsRejected` adds one reason tag 1 through 9: `StaleRevision`, `StaleGeneration`,
`HistoryEvicted`, `MissingResource`, `InvalidField`, `InvalidAcknowledgement`, `LimitExceeded`,
`ResourceExhausted`, and `InvalidPhase`; it answers only `ReadRows`, and a `ReadRows` request
accepts only a `Record`, `RowsRejected`, or the existing explicit unavailability/rejection
outcomes. A `RowsRejected` on any other action, or a `Record` answering an action that cannot
produce one, is a protocol violation.
Results always echo both request and attachment IDs. A poll never waits for future Host output.

A record result uses the separate `OBNR` envelope: four-byte magic, big-endian version 1 (`u16`),
zero reserved field (`u16`), and big-endian payload length (`u32`), then request ID (16), attachment
ID (16), and one unchanged OBTP record. For `Poll` that record is a Host event; for `ReadRows` it
is the correlated Host response. Its record is 64 through `64 + 36 MiB` bytes, matching the
Host envelope plus maximum OBTF payload. The codec validates the complete 12-byte header and bound
before allocating payload; consumers validate the opaque record's Host envelope, route, role,
identity, and negotiated size. This does not enlarge the 8 KiB OBCN control bound or decode frame
content. Strict in-memory decoding rejects trailing bytes; streaming decoding consumes exactly one
record and preserves the following frame boundary. No acknowledgement, retry, or queue policy is
hidden in these codecs. Viewer carrier failure retires transient state; callers never replay an
ambiguous attachment or feedback exchange automatically.

Five additive action tags 7 through 11 serve the history viewport and remain private and
unshipped; tags 1 through 6 are unchanged byte-for-byte. All carry the attachment only on the
request envelope. All integers are big-endian; `i32` is the only signed field. Unknown tags,
unknown origin or demand values, zero positive integers, out-of-range bounds, and trailing bytes
all reject.

- Tag 7 `set-projection-demand` adds `demand` (`u8`): 1 active, 2 paused.
- Tag 8 `set-history-demand` adds `demand` (`u8`): 1 following-bottom, 2 viewing-history.
- Tag 9 `read-viewport-rows` adds `origin` (`u8`: 1 tail, 2 committed) followed by `rowsBefore`
  (`u32`) for tail or `delta` (`i32`) for committed, then `rowCount` (`u16`, 1 through 4096),
  `revision` (`u64`), positive `generation` (`u64`), positive `requestSequence` (`u64`), and
  `maximumBytes` (`u32`, 112 through 8,388,608 — an OBRW payload bound, not a record bound).
  Purpose, detail, and revision policy are fixed viewport/text/current-view by the caller's
  adapter, not by this payload.
- Tag 10 `rows-applied` adds positive `requestSequence` (`u64`), positive `generation` (`u64`),
  and `revision` (`u64`) — the same field order `FrameApplied` uses.
- Tag 11 `history-observed` adds positive `sequence` (`u64`), positive `generation` (`u64`), and
  `revision` (`u64`).

The four control acknowledgement actions answer with `FeedbackAccepted`, or `Rejected` carrying
the existing Host-rejected reason tag 8 when the Host returns a typed failure. `read-viewport-rows`
answers with a `Record` holding the correlated kind-46 row-window response, or `RowsRejected` with
the existing nine-reason vocabulary. Any other outcome pairing stays a protocol violation.

## Controller messages

OBCN kind 7 `TerminalControlRequest` and kind 8 `TerminalControlResult` extend the same serialized
Server-Worker stream. Server is the actual producer and Worker maps the granted controller to a
separate non-reading Host attachment. Server allocates fresh request and controller identities;
Viewer and control identities never alias, and all requests pin the admitted Worker generation.

A request contains request ID (16 bytes), Worker ID (16), Worker generation (u64), viewer ID (16),
controller ID (16), then action: 1 Acquire, 2 Input, 3 Resize, or 4 Release. Input adds a positive u64
sequence, u16 length, and 1 through 4,096 raw bytes. Resize adds a positive u64 sequence and the four
u16 geometry fields in the existing columns/rows/cell-width/cell-height order. Other actions add no
field. Integers use big-endian encoding; all IDs are nonzero. Input and resize share a strictly
increasing per-grant sequence; admission consumes it even on a definite Host rejection.

`Paste` requests use the dedicated `OBNQ` record instead of a kind-7 frame: four-byte magic,
big-endian version 1 (`u16`), zero reserved field (`u16`), and big-endian body length (`u32`), then
request ID (16), Worker ID (16), Worker generation (u64), viewer ID (16), controller ID (16),
positive u64 sequence (8), and 1 through 262,144 opaque content bytes. A complete `OBNQ` record is
92 through 262,236 bytes. The codec validates the complete 12-byte header and bound before
allocating payload, consumes exactly one record, and preserves the following frame boundary.

A result contains request, viewer, and controller IDs (16 bytes each), then outcome: 1 Acquired,
2 Released, 3 InputAccepted, 4 Resized with the same complete geometry, 5 Rejected with a reason
byte: 1 WrongWorker, 2 StaleWorkerGeneration, 3 ViewerUnavailable, 4 ControllerBusy, 5 NotController,
6 SequenceMismatch, 7 HostRejected, 8 HostInputInvalid, 9 HostInputLimitExceeded,
10 HostInputBackpressure, 11 HostInputUnavailable — or 6 Pasted with the positive u32
`accepted_bytes` count the Host reported. Exact IDs and action/outcome compatibility are required;
Resize must echo geometry, and the Server carrier also rejects a `Pasted` result whose
`accepted_bytes` exceeds `min(2 × content + 12, negotiated input limit)`. Release is internal cleanup and is idempotent if that viewer has no controller;
a different current grant rejects. An ambiguous Host exchange closes the Worker carrier instead
of fabricating acceptance, and Server classifies complete delivery without a result as unknown.

The 8 KiB OBCN frame limit, exact-length decoding, and direction rules remain unchanged; `OBNQ`
is a raw record alongside the frame stream, like `OBNR`. Codec tests cover
all actions/outcomes/reasons, raw byte preservation, maximum input and invalid scope bounds. The
Server and Worker guides own single-controller reservation, cleanup and real PTY evidence.

## Send messages

OBCN kind 13 `TerminalSendRequest` and kind 14 `TerminalSendResult` carry one Server-authorized
non-interactive input submission. Server allocates a fresh request identity and the input-only
attachment identity; Worker never mints either, never consults its viewer or controller maps, and
never re-resolves the target by a logical terminal ID — the request already pins the exact
authorized incarnation.

A request contains request ID (16 bytes), Worker ID (16), Worker generation (u64), the exact
Host terminal identity (48: creation ID + Host epoch + Host terminal ID), then the
Server-assigned attachment ID (16), then a payload tag: 1 text or 2 key. Text adds a u16
length and 1 through 4,096 UTF-8 bytes plus an `enter` flag byte (0 or 1); the carriage return is
declared inside the same payload, so one request always encodes exactly one Host input record and
`text` + `enter` together must fit the 4,096-byte bound. Key adds a closed u8 code: 1 Enter,
2 Escape, 3 Tab, 4 Backspace, 5 Delete, 6 Up, 7 Down, 8 Left, 9 Right, 10 Home, 11 End, 12 PageUp,
13 PageDown, 14 CtrlC, 15 CtrlD. All integers are big-endian and all IDs nonzero. The generation,
epoch, and terminal identity are fences: the Worker rejects a request pinned to a different
generation or epoch instead of rebinding, and no restart-rebind behavior exists in this contract.

A result contains the request ID (16), then the input axis — 1 Unavailable with a reason byte
(1 HostUnavailable, 2 HostProtocolFault), 2 Rejected with a reason byte (1 WrongWorker,
2 StaleWorkerGeneration, 3 StaleHostEpoch, 4 TerminalUnavailable, 5 HostRejected,
6 HostInputInvalid, 7 HostInputLimitExceeded, 8 HostInputBackpressure, 9 HostInputUnavailable),
3 Accepted with the u32 `accepted_bytes` count the Host encoded into its input queue (zero is
legal when a mode ignores a semantic key; it is never the request byte length or a key-name
length), or 4 Unknown — then the orthogonal cleanup axis byte: 1 NotNeeded, 2 Confirmed, or
3 Unconfirmed. Two pairs cannot arise from the Attach → input → Detach sequence and are rejected
at construction and at both codec boundaries: Accepted + NotNeeded (an accepted input implies a
real attachment) and Unknown + Confirmed (a confirmed release implies a completed exchange).
Unknown and Unconfirmed mean the input may have landed or the attachment may persist; neither may
be retried automatically. A definite Host rejection still releases the temporary attachment,
while a desynchronizing exchange is answered by closing the Worker-Host carrier rather than
further requests — the degraded cleanup status records that honestly instead of claiming release.

The 8 KiB OBCN frame limit, exact-length decoding, direction rules, and nonzero-identity rules are
unchanged. This message grants no controller, viewer, resize, or lease authority and consumes no
slot in either map; the authorized attachment is temporary to one sequential send. Codec tests
cover every payload/outcome/reason combination, strict goldens, unknown tags, trailing bytes, and
boundary lengths; the Server and Worker guides own admission and the real Host evidence.

## Worktree job messages

OBCN kind 15 `WorktreeJobRequest` and kind 16 `WorktreeJobResult` carry one bounded
`git worktree list` discovery job for the `worktree list` Server flow. A request contains
operation ID (16 bytes), Worker ID (16), Worker generation (u64), then an action tag: 1
`Start` with a u16 length and 1 through 4,096 directory bytes (an absolute, nonempty,
NUL-free local-platform path — validated at construction and decode), 2 `Poll` with a job ID (16),
or 3 `Cancel` with a job ID (16). The job ID is the Start operation's own operation ID:
a Server mints one identity per job and reuses it on Poll and Cancel.

A result echoes the operation ID (16), the action tag, and the job ID (16) — the action echo
keeps every outcome code unambiguous — then one action-scoped outcome byte. Start outcomes:
1 Accepted, 2 Busy, 3 InvalidDirectory, 4 SpawnThreadFailed (the owner thread, never a
child), 5 Dead (a permanent tombstone slot), or 12 RejectedFencing. Poll outcomes: 1
Pending; 2 Completed with three u16-length-prefixed byte fields — probe `common_dir`,
`listing`, and `stderr` — whose summed length must fit the 7,936-byte field budget at
construction and decode; 3 GitFailed with stage (1 probe / 2 list), a status-presence byte,
an optional i32 exit status, and one length-prefixed `stderr` field under the same budget;
4 SpawnUnavailable{stage}; 5 SpawnFailed{stage}; 6 TooLarge with an optional stage (0 when
the wire serialization budget itself overflowed); 7 Timeout; 8 Cancelled; 9 OwnerIo with an
optional stage; 10 Unknown; 11 PhysicalUnknown; or 12 RejectedFencing. Cancel outcomes are
summary-only: 1 Registered or 2 AlreadyTerminal — the terminal outcome stays re-readable
through Poll until the bounded receipt expires — plus 10 Unknown, 11 PhysicalUnknown, and 12
RejectedFencing. A `RejectedFencing` result is a typed frame, not a disconnect: a request
pinned to another Worker or generation is refused without retiring the carrier.

The 8 KiB OBCN frame limit is unchanged — 7,936 is a conservative policy bound that keeps
every arm below the payload ceiling with declared headroom. Codec tests cover every action
and outcome arm, strict goldens, the field-budget boundary, unknown tags, stage codes, and
trailing bytes; the Server and Worker guides own admission, fencing, and job evidence.

## Agent drain messages

OBCN kind 17 `AgentDrainRequest` and kind 18 `AgentDrainReply` carry the Server-initiated drain of
the Worker's retained terminal application signals. The request body after its kind is, in order:
the protocol-owned Worker ID (`16` bytes), the nonzero Worker generation (`u64`), the nonzero
request token (`u64`), and the acknowledged sequence watermark (`u64`) — 41 payload bytes, 49
framed. The Worker echoes the exact token; it never mints or remints one. An acknowledgement above
the Worker's offered watermark is a fatal protocol error; a stale or zero acknowledgement is a
legal no-op and never removes an unconfirmed sequence.

The reply carries the echoed token and acknowledgement, the queue tail watermark, a bounded entry
count, and a contiguous non-overlapping page of `Event` and `Gap` entries. Each event entry is the
complete CNP-side `HostTerminalIdentity` plus a UTF-8 payload of at most 4,096 bytes; each gap
entry covers a closed sequence range evicted from the retained window. The decoder rejects
more than 480 entries before any allocation, unknown tags, nonpositive sequences or gap bounds,
reversed gap ranges, oversized payloads, non-contiguous or overlapping pages, entries beyond the
declared queue tail, trailing bytes, and integer overflow. The first entry is not required to
follow the echoed acknowledgement, because stale acknowledgements are legal.

These types are Capability Node refinements: the event identity reuses the crate's own
`HostTerminalIdentity`, not the Terminal Host wire identity. Wire kinds 15 and 16 remain reserved
for Worktree functionality and are not consumed here. Server-side drain production, registry, and
held-listener behavior remain outside this crate's scope.

## Authorization outcomes

Pre-admission rejection distinguishes wrong Worker, stale generation, expired lease, stale fence,
conflicting same-fence replay, unsupported capability, and retained-binding capacity. A
`CapacityExceeded` result is valid only for a previously unseen `TerminalId` after one Worker
generation has retained 1,024 bindings; it inserts no tombstone and changes no binding. Host-side
rejection and failure retain the observed Host epoch. `HostUnavailableBeforeHandoff` is a definite
pre-effect failure;
`OutcomeUnknown` is the conservative result when a dispatched request has no observable result,
because the Server cannot infer the Worker's internal handoff stage from carrier write completion.
Launch success returns complete Host creation, epoch, and terminal identity plus either
awaiting-start or started geometry. Start results always retain their complete target Host identity;
accepted results additionally return the observed identity, geometry, and started/already-started
transition.

Launch and Start use distinct rejection and failure types: Launch cannot encode
`HostIdentityChanged` because it has no target Host terminal. A Start `HostAccepted` observation
must exactly equal the frame's target identity. Its `HostRejected` and `HostReportedFailure` epochs
must equal the target epoch; a different observed epoch is represented only by
`HostIdentityChanged`, whose epoch must actually differ. Validated result construction and decoding
reject these identity and epoch contradictions, so canonical encoding cannot emit one through those
constructors. The Worker refinement additionally requires a successful Start observation to retain
the exact authorized geometry before it constructs a result.

The launch payload accepts a bounded literal `LaunchEnvironment` in addition to the optional
launch selection. Empty environments keep the legacy bytes unchanged. A nonempty environment
replaces the selection tail with `u16(0)`, extension version `u8(1)`, selection-present `u8(0|1)`,
the optional existing selection, then a `u16` entry count and `u16`-length-prefixed UTF-8 name/value
pairs. Zero is an invalid legacy executable length, so older decoders reject this extension.
Unknown versions, truncated fields, an empty extension and trailing bytes are rejected.

An environment has at most 32 entries and 1,024 combined UTF-8 name/value bytes. Names match
`[A-Za-z_][A-Za-z0-9_]*`, are at most 128 bytes, and are unique ignoring ASCII case. NUL values,
`ORCHBEE_*`, `TERM`, `COLORTERM` and `TERM_PROGRAM` are rejected; empty values are preserved.
Validation precedes admission; Debug output omits values. The combined maximum cwd, selection,
argument count and environment still fit the 8 KiB frame. There are no Project, Checkout,
Worktree, Agent or Launch Profile fields. Cohort validation requires matching deployed binaries;
this extension does not promise mixed-version operation.

## Carrier and ownership boundaries

This crate owns only the closed semantic values and canonical `OBCN` frame codec. It does not open,
authenticate, admit, retain, retry, reconnect, multiplex, or apply backpressure to a connection.
Its streaming helpers borrow a caller-owned `Read` or `Write` only for one frame and retain no
socket, buffer, connection, or phase state.
Unix-domain sockets, same-user peer credentials, executable cohort, the `OBLS` admission handshake,
remote transport, encryption, and connection state machines belong to carrier adapters and their
own protocols. The current local Server adapter owns a capacity-one submitted slot behind at most
one wire request, serializes complete request/result exchanges, validates exact correlation and
echoed authorization, and distinguishes failures before and after a complete request write. It
does not map OBCS, synthesize semantic outcomes, retry, or reconcile. Server owns local terminal authorization and
reconciliation; a Capability Node adapter owns explicit mapping to its Worker
admission identity and Terminal Host protocol. This crate depends on neither protocol.

For the current adapter, an attached generation remains registered until its sole wire owner has
settled or conservatively abandoned the current exchange and drained the submitted slot. Only then
does a lifetime guard remove that exact generation, so no replacement overlaps an old write or
exact-result publication. Normal return, completion-receiver loss, thread-spawn failure, and panic
unwinding follow the same ordering. Abandonment after a complete write remains a typed carrier-level
post-delivery unknown; the adapter does not manufacture an OBCN result.

## Checked authorization model

The registered
[`TerminalLaunchAuthorization`](../../apps/orchbee-server/formal/tla/TerminalLaunchAuthorization.tla)
model is the Server-owned behavioral authority for this experimental contract. It
explores current and stale Worker generations, lease expiry before admission, between admission
and Host handoff,
and after handoff, lower/exact/conflicting fences, definite pre-handoff failure, post-handoff
unknown outcomes, Host epoch replacement—including an admitted Start whose target epoch changes
before handoff—deferred launch followed by separate authorized start, stale result suppression,
every connection loss, and retained-terminal lifetime. That raced identity change is cached as the
exact `HostIdentityChanged` result without a Host effect. The model's invariants require at most one
Host effect per operation and prevent old generation, fence, operation, or Host epoch results from
overwriting current authority. Because this protocol has no progress acknowledgement, a lost Worker
link after dispatch produces a conservative unknown observation at every unobserved Worker stage;
only a returned Worker result can report definite pre-handoff failure.

The model permits one outstanding request and one pending-result slot. Worker loss before
admission, after admission, and after Host handoff is checked separately and always occupies that
empty slot with an unknown result. A replacement Worker generation is not activated until that
pending loss observation is settled, so reconnection cannot make the current unknown result appear
stale. Start acceptance separately explores `Started` and `AlreadyStarted`. Once an admitted
operation has a terminal result, the model caches its exact outcome, Host epoch, and transition. An
exact replay returns that cache without a new Host effect—including after lease expiry—so a
rejection, failure, or unknown result can never be manufactured into success.

The only fairness condition settles a pending result while Server remains connected. It does not
require any healthy connection to fail and does not promise automatic reconnection, authorization,
or terminal recovery. The model is bounded to two operation IDs, two distinct lease identities, and
two Worker generations, fences, and Host epochs. Its deterministic fence-to-operation-and-lease
mapping is a state-space symmetry reduction, not a runtime derivation rule; a checked no-revival
property prevents an expired accepted lease from becoming valid again without replacing that
binding. `FreshOperation` is the one-retained-terminal projection of the global Server producer
invariant and remains in force across modeled Worker reconnection because current authority and Host
effects are retained. The model does not enumerate the cross-terminal and cross-Worker namespace or
a successor Server-authority transition. Future runtime adapters require separate refinement
evidence.

The registered `TerminalLaunchAuthorization` model uses capability version 2 as its supported
authority and version 1 as its unsupported-capability witness.

The Worker-owned registered
[`WorkerTerminalBindingCapacity`](../../apps/orchbee-worker/formal/tla/WorkerTerminalBindingCapacity.tla)
refinement composes the cross-terminal bound with the Worker's abstract global operation/result
slot while the single-terminal Server authority model retains the detailed handoff phases. Its TLC
configuration uses capacity two and three terminal identities to exhaustively cover below-limit
admission, the full threshold, rejection of one new identity, occupied-slot priority, result
delivery clearing, exact cached replay and higher-fence replacement at capacity only after the slot
is empty, rejection preservation, and classification priority. The production constant is exactly
1,024 retained bindings per Worker generation. There is no release, TTL, LRU, or eviction path;
explicit retirement remains required before production. This bounded model is not a proof over
1,024 concrete entries or of the Rust implementation, and its authority ships only in the same
unshipped delivery cohort as the corresponding Rust enforcement.

## Verification

Run from the repository root:

```sh
cargo test -p orchbee-capability-node-protocol --all-targets --all-features --locked
cargo clippy -p orchbee-capability-node-protocol --all-targets --all-features --locked -- -D warnings
node scripts/tla/run-models.mjs --models terminal-launch-authorization,worker-terminal-binding-capacity
pnpm test:tla:inventory
pnpm lint:documentation
pnpm lint:projects
```

## Managed Worker shutdown

The Windows managed Site lifecycle adds kinds 19/20 under frame version 2 and the
existing 8 KiB bound. The request contains operation ID and Worker ID (16 nonzero
bytes each), positive Worker generation (u64), and a cooperative budget (u32,
1..15000 ms), all integers big-endian. Only the exact admitted managed Worker may
act; unmanaged or mismatched identity is a pre-effect refusal. The existing 20-second
carrier reply window is unchanged. A reply echoes the complete request.

Outcome tags are 1 NoHost, 2 Stopped (Host epoch, u32 disposed count), 3 Refused
(1 wrong Worker, 2 unmanaged), and 4 Incomplete (optional Host epoch, u32 disposed
count, stage and failure). NoHost means no owned Host was started and background
jobs settled; Stopped requires the owned Host's exit after containment/disposal.
Neither claims that the Worker has already exited; its Server must wait on the
retained creation handle. Counts describe only this attempt. Stages 1..5 are
background jobs, inventory, force, dispose and Host exit. Failures 1..5 are deadline,
transport, invalid protocol, Host refusal and process wait. Optional epoch uses
tag 0/1 followed by the nonzero ID when present. Invalid tags, zero values, budget
overflow and trailing bytes are rejected. No terminal permission is inferred from
codec validity and no request is automatically replayed after an unknown handoff.
Normal connection loss is not shutdown. The typed exchange supplies evidence to
the Server lifecycle; it never publishes the local activation completion record.

## Native Windows path refinement

Absolute wire paths accept a POSIX leading slash or an ASCII drive letter followed by `:` and `/` or `\`, optionally prefixed by `\\?\` for a Windows verbatim drive path. This lexical rule is independent of the decoder platform; bytes are preserved without expansion or normalization. Backslash-rooted, drive-relative, UNC, and device paths are not admitted by this Windows increment. Existing byte bounds and NUL/Unicode validation remain unchanged. The execution owner still validates the path against its local filesystem before effects.

## Terminal process reads (unpublished)

Viewer action tag 12 is `ReadProcessInfo`, with no bytes after the tag; it uses the request's existing attachment ID. Worker requires that exact retained viewer and sends one Host `ReadProcessInfo` (OBTP kind 79) with a fresh correlation. The response is bounded to 64 KiB plus the envelope and returned unchanged as `Record`, including a well-formed correlated Host `TypedFailure`. An observed result must echo the exact attachment and physical terminal. Wrong Worker/generation, missing viewer, wrong Host epoch or invalid result fails through existing rejection/carrier outcomes. No process enumeration runs in Worker; Host remains the owner. A process read neither acknowledges a frame nor creates input authority. Existing action encodings remain unchanged; the development cohort must be updated together.

## Worktree mutation job contract

This private contract defines the boundary for the Windows Worker Git owner
and Server Worktree continuation. Windows Worker dispatch and Server continuation implement this boundary. The representation alone does not establish end-to-end CLI correctness. Server owns Workspace association; Worker owns Git
checkout observations and effects. No new dependency or Unix execution is admitted.

The request carries one nonzero operation ID, Worker ID and positive generation.
Start admits a checked intent; Poll and Cancel address the original Start ID.
Create carries source directory, checkout path, branch and base. Open carries source
and exactly one path or branch selector, including the main checkout. InspectSource resolves the source checkout, including a source subdirectory. InspectRemove observes one registered linked
checkout and its dirty state. Remove carries source, checkout, expected common Git
directory and force. Every intent also carries explicit repository trust. These are
semantic operands, never an executable, shell expression or arbitrary argv.

Paths use the existing absolute, NUL-free UTF-8 `AbsoluteCwd` limit (4096 bytes).
Reference operands are nonempty, at most 1024 bytes, contain no ASCII control or DEL,
and never begin with `-`; Git remains the reference-syntax authority. Variable fields
sum to at most 7936 bytes per intent or checkout observation. Diagnostics retain at
most 2048 raw bytes; they are data, never product copy or shell input.

| Owner state / event            | Guard and observable next state                                     | Effect and recovery                                                  |
| ------------------------------ | ------------------------------------------------------------------- | -------------------------------------------------------------------- |
| Idle / Start                   | Current Worker generation, valid intent and free capacity: Accepted | Retain intent and Job ID before one owner thread can run Git         |
| Retained ID / Start            | Identical intent: Existing; different intent: Conflict              | Neither repeats Git                                                  |
| Running / another Start        | Busy                                                                | No second physical job                                               |
| Receipt capacity / fresh Start | Full after 64 lifetime admissions                                   | No expiry permits mutation replay; restart changes Worker generation |
| Running / Poll                 | Pending until owner settlement                                      | No Git command is triggered by observation                           |
| Running / Cancel               | Registered                                                          | Request cancellation; never imply rollback or discard a known effect |
| Settled / Poll or Cancel       | Retained outcome or AlreadyTerminal                                 | Repeat observation does not consume the receipt                      |
| Unproven physical cleanup      | PhysicalUnknown and retired admission                               | No later root status upgrades uncertainty into completion            |
| Wrong Worker or generation     | RejectedFencing                                                     | No owner state or process is touched                                 |

The runtime owner must implement these transitions; codec tests establish only their
representation. A successful checkout observation contains canonical repository root,
common Git directory, checkout path, optional branch and dirty state. Remove success
contains the removed checkout path. Failure contains stage, closed reason, whether a
mutation may have occurred, and bounded diagnostics. Stage tags 1–6 are Probe, Select,
CheckBranch, DirtyCheck, Mutate and Verify. Effect tags 0/1 are None/MayHaveChanged.
An interrupted or failed Git mutation is never represented as proven rollback.

OBCN version 2 kind 21 encodes operation ID (16), Worker ID (16), generation (u64),
and action (u8: 1 Start, 2 Poll, 3 Cancel). Poll/Cancel add Job ID (16). Start adds
trust (strict boolean), intent tag (1 Create, 2 Open, 3 InspectRemove, 4 Remove, 5 InspectSource),
source and its intent operands in the order above. Open selector tags 1/2 are path/branch.
Remove force follows its expected common directory. Strings and byte fields use a
u16 byte length; integers are big-endian. All UTF-8 and checked constructors run on
decode. No unknown tag, trailing bytes, invalid identity, boolean or option is accepted.

Kind 22 encodes operation ID (16), Job ID (16), action tag and outcome tag. Start
tags 1–8 are Accepted, Existing, Busy, Conflict, Full, SpawnFailed, PhysicalUnknown,
Unsupported; 255 is RejectedFencing. Poll tags 1–4 are Pending, Completed, Unknown,
PhysicalUnknown; 255 is RejectedFencing. Completed adds 1 Checkout, 2 Removed or
3 Failed. Checkout fields follow the observation order above, with strict 0/1 branch
option and dirty boolean. Failed adds stage, effect, failure tag and diagnostics.
Failure tags 1–14 are GitFailed (optional i32 exit code), SpawnUnavailable, SpawnFailed,
InvalidRepository, CheckoutNotFound, MainCheckout, Dirty, InvalidReference, TooLarge,
Timeout, Cancelled, Io, RepositoryChanged and AmbiguousCheckout. Cancel tags 1–4 are Registered,
AlreadyTerminal, Unknown and PhysicalUnknown; 255 is RejectedFencing.

The largest request is a maximum-field Create: 7988 payload bytes. The largest result
is a Checkout observation: 7982 payload bytes. Both remain below the existing 8192-byte
frame payload cap. Public conformance tests cover hand-derived golden frames, all
variants, maximum lengths, strict truncations and malformed fields. They do not prove
Git behavior, filesystem identity, ownership, asynchronous progress or shutdown.
Deploy the private Server/Worker cohort together; older cohorts reject additive tags.
Rollback replaces the cohort and migrates no durable protocol data.

## Source-based Worktree discovery

Kind 15 additionally admits `StartSource` action tag 4: a bounded absolute UTF-8 directory plus strict 0/1 command-local trust. Legacy Start tag 1 is unchanged. Start result tag 6 means unsupported execution. Poll result tag 13 is `SourceCompleted`: common-directory, source-checkout, listing and stderr bytes share the existing 7936-byte variable-data bound, with four explicit lengths. The Worker owns execution and cleanup; the new read consumes no mutation job receipt. Golden bytes, wrong-direction/trailing/truncated input, strict trust and combined bounds are covered in `tests/worktree_source.rs`.

## Styled buffer reads

`ReadRows` carries a typed text/render detail. Text preserves viewer action tag 6 and its entire
existing body byte-for-byte. Render uses tag 13 followed by the same start row, row count, revision,
generation, request sequence and maximum-byte fields. Bounds, result compatibility and correlation
stay unchanged; unsupported tags are rejected. Worker maps render to the existing Host render
row-access request, and Server checks the returned detail. Viewport reads remain text.

This private unshipped extension retains the OBCN frame version and 8 MiB row payload limit. It
adds neither effects nor retry/fallback policy. Matching development cohorts deploy or roll back
together without persistent migration. Exact old text and new render bytes, malformed bodies and
unknown tags are checked independently of the runtime forwarding and response-route tests.
