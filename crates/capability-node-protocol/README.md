# Capability Node Protocol

Private, experimental Rust semantic contract for authorized terminal operations and bounded
directory observations between Orchbee Server and a Capability Node. The initial Capability Node
is the Worker, but this crate does not bind the messages to a local socket or any other carrier. The
local Server adapter now consumes its codec in an application-private post-OBLW carrier. The local
Server coordinator submits authorized Create/Start and generation-pinned cwd validation through
that carrier. It is not a public API or a compatibility promise.

## Terminal control result pairing

`TerminalControlAction::admits` owns control action/result pairing. Every action may be rejected;
only Clear admits `ClearOutcomeUnknown`, and only Reset admits `ResetOutcomeUnknown`. Resize requires an exact geometry echo, and Paste accepts
at most twice its content length plus 12 encoded bytes. The action match is exhaustive, so adding
an action requires declaring its admissible outcomes. The carrier separately validates request,
viewer, and controller identities.

## Controller query colors

`HostDefaultColors` carries effective foreground/background RGB plus an optional cursor and sixteen
optional ANSI colors. `HostColors` control action 7 encodes its positive sequence (`u64`), foreground
and background (`u32` each), cursor (`u32`), then sixteen palette slots (`u32` each), all big-endian.
Only optional cursor/slot values use `0xffffffff` for unknown; every present color is at most
`0xffffff`. The current frame version stays 1. The existing controller fence owns authorization,
and the Worker applies known query colors regardless of launch display mode; null outer-terminal
observations retain fallback, never an invented sampled color.

## Job result pairing

`WorktreeJobAction::admits` and `WorktreeMutationJobAction::admits` enumerate Start, Poll and
Cancel response classes. The carrier separately validates operation and job identities,
including the operation-owned job identity minted by Start.

## Windows Worker handoff

Frame version 1 adds fixed-size kinds 26 Request, 27 Result, 28 Hello, 29 Ready and 30 Owned.
Private cohorts reject unknown kinds; there is no mixed-cohort ordinary admission fallback.
A Request body after its kind is operation ID (16 bytes), Worker ID (16), positive generation
(`u64`), nonzero handoff ID (16), target cohort SHA-256 (32) and action (`u8`: 1 Prepare,
2 Commit, 3 Abort). IDs are nonzero and all integers are big-endian. Payload size is 90 bytes.
Hello/Ready/Owned omit the operation ID and action and carry the exact same owner/target seal;
their payload is 73 bytes. The cohort digest represents the current Server's explicit delegation;
codec validation cannot prove executable admission, import authorization or compatibility.

Result echoes the entire Request body, then outcome (`u8`: 1 Prepared, 2 Committed, 3 Aborted,
4 Refused). Success must match its action. Refused adds one reason (`u8`, 1–10): WrongWorker,
Unmanaged, Busy, TicketMismatch, NotPrepared, ReusedTicket, Capacity, TargetUnavailable,
Authentication, AttachmentsBusy. Result payloads are exactly 91 or 92 bytes. Constructors and
decoders reject contradictory successes, unknown tags, zero IDs/generation, prefixes and suffixes.
The ordinary 40 KiB frame budget remains unchanged.

Only the current authenticated Server may prepare a derived private Windows runtime pipe.
The standby authenticates the exact ticket/Worker/generation/cohort and may claim the connection
only after Owned. Committed on the old wire is the transport commit boundary; the public Server
result also requires canonical import and endpoint ownership. Lost receipts never authorize
automatic reconnection or replay. [The Worker model](../../apps/orchbee-worker/docs/windows-server-handoff.md)
owns deadlines, ticket bounds, physical-owner preservation and focused evidence limits.

## Configured background commands

The process-job lane admits configured Windows commands without a plugin identity or plugin log. OBPJ uses frame version 1 for every action and outcome. Request action 5 carries a bounded catalog ID, absolute directory, literal command and unique bounded `ORCHBEE_*` environment entries; result outcome 5 echoes the job ID and whether the same frozen job already exists. Worker ID, generation, operation correlation and closed refusals apply to plugin and configured jobs alike.

The Server resolves the installed configuration before dispatch. The Worker
overwrites Session routing, executes the literal command with Windows `cmd /d /c`,
and owns its cancellation and join independently of the invoking connection.
Plugin and configured jobs share the 32-running-process admission bound.
Configured receipts are bounded to 200 records and are never exposed as plugin
history. Shutdown cancels and joins both kinds before a complete receipt.

## Agent start command

A default or explicit launch may carry an `AgentStartCommand`: the command the launched shell runs
first, resolved by the Server from the startable Agent catalog. It is a bare command name of at most
64 bytes, not beginning with `-` or `.`, and at most eight arguments of at most 128 bytes; every token
uses only `[A-Za-z0-9._:=-]`, so no shell reads it as syntax. Launch kinds 5 and 6 are kinds 0 and 1
followed by the command in the selection's program encoding. It excludes plugin metadata and is part of the authorized launch's replay identity. The Worker passes it only to a
shell whose startup takes a first command and launches any other program without it; see
[Agent start command](../../apps/orchbee-worker/README.md#agent-start-command).

## Earlier output for a restored launch

A launch may carry earlier output the new terminal shows before its program starts: at most 1 MiB,
never empty. Such a launch travels in its own `OBNL` record instead
of an `OBCN` frame: the four bytes `OBNL` and the bytes `00 01 00 00`, an unsigned big-endian body length, then the
launch's ordinary `OBCN` frame followed by the output bytes. The launch frame keeps the 40 KiB bound
and exact decoding; only this record admits the larger body, and the streaming reader validates its
length before allocating. The output is not part of authorization replay identity: the Worker takes
it from the launch before admission, so the authorization kernel never retains it, and stages it in
the Host for that exact creation. The Host checks its grammar.

## Transient terminal clear and reset

Control action tag 9 is `Reset`, framed and admitted exactly like `Clear` below; the Worker sends the Host's full reset instead of a prompt-preserving clear. Result tag 10 is `ResetAccepted` with `Cleared`'s fields, and tag 11 is `ResetOutcomeUnknown`, valid only for Reset.

Control action tag 6 is `Clear`, using the request viewer plus a newly allocated transient attachment in `controller_id`; it never names or changes a controller reservation. Result tag 7 is `Cleared` with positive u64 generation/revision and a canonical one-byte cleanup Boolean; tag 8 is `ClearOutcomeUnknown` and is valid only for Clear. Rejection tag 12 denotes an alternate screen. Old tags and framing remain unchanged. The carrier validates the complete request/viewer/transient identity and result kind before delivery. [Native clear](../../apps/orchbee/docs/native-cli-query.md#clear-terminal-output) owns the end-to-end state machine; same-cohort deployment is required.

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

Kind 9 `TerminalLifecycleRequest` and kind 10 `TerminalLifecycleResult` use frame version 1 and
the 40 KiB payload bound. All integers are unsigned big-endian and IDs are nonzero 16-byte values.
The request body after its kind is, in order: lifecycle capability version (`u16`, current 1),
product Terminal ID (16), Worker ID (16), Worker generation (`u64`, positive), retained binding fence
(`u64`, positive), fresh lease ID (16), lease Unix-millisecond expiry (`u64`, positive), Close
operation ID (16), step request ID (16), attempt generation (`u64`, positive), step ordinal (`u8`,
1 through 32), Host creation/epoch/terminal IDs (16 each), and action (`u8`). The lifecycle
capability is distinct from launch capability version 1. The fence references the exact current
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
request and adds one outcome tag: 1 `DirectoryAvailable`, 2 `DirectoryMissing`, 3
`WorkerUnavailable`, 4 `NotADirectory`, or 5 `DirectoryInaccessible`. All integers retain the
existing unsigned big-endian encoding; old cohorts reject tags 4 and 5.

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
unsigned big-endian payload length, and at most 40 KiB of payload. The frame codec rejects unknown
magic, version, message or enum tags, malformed or incomplete values, excess bytes, and oversized
payloads. Frame version 1 and terminal-launch capability version 1 are separate version domains.
Bounded streaming helpers read the complete eight-byte header before validating it and allocating
the declared payload, then read exactly that payload; writes tolerate partial progress. I/O failure
is distinct from a syntactically invalid protocol frame. Writing one frame does not flush a
caller-owned buffer; that carrier policy remains with the adapter.

Launch capability version 1 includes the generation-local Worker binding bound and typed
capacity result. The Worker authorization kernel admits only the exact current capability;
other versions return `UnsupportedCapability`. Construction and decoding reject
`CapacityExceeded` when the echoed authorization does not use the current capability.
The matched native cohort follows the repository
[pre-release version rule](../../docs/ARCHITECTURE.md#data-boundaries).

| Message                    | Direction                 | Closed semantic payload                                                                                                                                                                                                                     |
| -------------------------- | ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `AuthorizedTerminalLaunch` | Server -> Capability Node | Complete operation authorization, optional bounded node-local current directory, the Session's launch preference capture, either `Immediate` with complete pixel geometry or `Deferred`, and an optional bounded explicit launch selection. |
| `TerminalLaunchResult`     | Capability Node -> Server | The exact authorization plus `HostAccepted`, `Rejected`, `Failed`, or `OutcomeUnknown`; every post-handoff outcome preserves the observed Host epoch.                                                                                       |
| `AuthorizedTerminalStart`  | Server -> Capability Node | A separately fenced and leased authorization, complete target Host terminal identity, and complete pixel geometry for a previously deferred terminal.                                                                                       |
| `TerminalStartResult`      | Capability Node -> Server | The exact authorization and target Host identity plus `HostAccepted`, `Rejected`, `Failed`, or `OutcomeUnknown`; success is explicitly `Started` or `AlreadyStarted`.                                                                       |

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

Every launch carries one required u8 kind immediately after the launch mode: 0 uses the default program, 1 carries an explicit selection, and 3 carries an explicit plugin selection; kind 2 is [retired](#retired-launch-kind-2). Each kind includes the caller environment, including a zero entry count when empty. Kinds are alternatives in the current layout, not payload versions. Missing kinds, unknown kinds, truncation and trailing bytes are rejected.

An explicit selection contains a `u16` executable length, executable bytes, a `u16` argument count and `u16`-length-prefixed argument strings. The executable is a nonempty absolute UTF-8 path without NUL; arguments are NUL-free UTF-8 strings (empty and dash-leading values allowed), at most 128 entries and at most 30,720 UTF-8 bytes across executable and arguments. The selection carries `argv[1..]`; Host synthesizes `argv[0]` from the executable. Kind 0 remains distinct from kind 1 with an empty argument list.

### Launch preference capture

Every launch carries one u8 preference tag between the optional current directory and the launch
mode. Tag 0 (`Absent`) leaves the System shell, Host colors and Host scrollback to the Capability
Node; it serves adapters without a Session preference owner. Tag 1 (`Unavailable`) states that the
Session could not read or bound its confirmed snapshot, and the Worker refuses that creation before
Host effects. Tag 2 (`Available`) is followed by `u32` scrollback bytes of at most 1,000,000,000,
a u8 color tag, the theme selection and a u8 program tag. Color tag 0 follows the Host's colors;
tags 1 (light scheme) and 2 (dark scheme) add `u32` foreground and background values within 24-bit
RGB. The theme selection is the light then the dark color-theme id, each a u8 length of 1 to 64
followed by UTF-8 bytes; it stays literal intent, and the Capability Node resolves an id it does
not ship to its default theme of that polarity. Program tag 0
accompanies every explicit kind. Kind 0 requires program tag 1, the System shell followed by a u8
login flag of 0 or 1, or tag 2, a custom program encoded like an explicit selection. A custom
program keeps the launch profile's literal path and shares the 30 KiB selection budget. Unknown
tags, out-of-range values and a program tag that disagrees with the launch kind are rejected.

The capture is user intent only. The Capability Node never reads the settings file; it resolves
device facts such as System shell discovery and `PATH` itself, so the same capture suits a Worker
on another device.

## Read-only viewer messages

`TerminalViewerAction::admits` exhaustively pairs each viewer action with its response class.
The carrier separately checks request/attachment correlation and lifecycle target echoes;
record consumers validate the opaque Host payloads.

The Worker now consumes OBCN kind 5 `TerminalViewerRequest` and emits kind 6
`TerminalViewerResult`. Server now submits these messages for its admitted Client Sessions alongside Launch/Start. These additive messages remain private and unshipped.
An older decoder rejects the new tags or magic; no fallback or mixed-cohort compatibility is claimed.

Every request contains a fresh request `OperationId`, exact Worker ID and nonzero generation, and a
Server-issued attachment `OperationId`. Server allocates attachment identities fresh across its
active authority and never reuses them after detach. They name independent transient viewers, not
product Terminal identities or Host creation identities. The authenticated Server alone grants
read access; Worker checks the admitted Worker/generation and exact Host epoch before attaching.
This path grants neither input nor resize, and never claims the Host resize lease.

Kind 33 `TerminalViewerRecordReady` is the one viewer message without a request. Between requests
the Worker may tell the Server that an attachment holds a record its next Poll would return. It
carries only the attachment `OperationId`, moves no record, grants nothing and has no response. A
Worker repeats it for an attachment only after a later Poll, and a Server discards one for an
attachment it no longer routes. Kind 35, described under
[physical title changes](#physical-title-changes), is the only other unsolicited kind; any other
still retires the carrier.

The closed actions are `Attach` with Host epoch and Host terminal ID, `Poll`, `FrameApplied` with
positive sequence/generation and revision, `NeedSnapshot` with positive sequence and the existing
seven snapshot reasons, `Detach`, and `ReadRows`. Kind 5 lays out request ID (16 bytes), Worker ID (16),
generation (8), attachment ID (16), then action tag (1). Tags 1 through 6 select those actions in
that order. Attach adds two 16-byte identities; FrameApplied adds three 8-byte integers;
NeedSnapshot adds an 8-byte sequence and reason tag 2 through 8: `BaseMismatch`,
`DecoderRejected`, `DimensionsMismatch`, `GenerationMismatch`, `IdentityMismatch`, `SequenceGap`,
and `StoreReset`; reason tag 1 is unassigned and rejected. `ReadRows` adds big-endian `startRow` (u64), `rowCount` (u16, 1 through 4096),
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
identity, and negotiated size. This does not enlarge the 40 KiB OBCN control bound or decode frame
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
- Tag 9 `read-viewport-rows` adds `origin` (`u8`: 1 tail, 2 committed, 3 absolute) followed by
  `rowsBefore` (`u32`) for tail, `delta` (`i32`) for committed or `row` (`u64`) for absolute, then
  `rowCount` (`u16`, 1 through 4096), `revision` (`u64`), positive `generation` (`u64`), positive
  `requestSequence` (`u64`), and `maximumBytes` (`u32`, 112 through 8,388,608 — an OBRW payload
  bound, not a record bound). Tag 9 selects text detail; tag 14 selects render detail with the
  identical bounded body. The purpose stays viewport. Tail and committed reads use the
  current-view revision policy, and an absolute read the exact policy, so the Host refuses it as
  stale once the history moved past its revision. Existing text bytes are unchanged; old receivers
  reject tag 14 and origin 3. Upgrade or roll back complete cohorts.
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

The 40 KiB OBCN frame limit, exact-length decoding, and direction rules remain unchanged; `OBNQ`
is a raw record alongside the frame stream, like `OBNR`. Codec tests cover
all actions/outcomes/reasons, raw byte preservation, maximum input and invalid scope bounds. The
Server and Worker guides own single-controller reservation, cleanup and real PTY evidence.

## Send messages

Native send requests use a dedicated `OBNS` record: magic, big-endian u16 encoding version 1, big-endian u16 precondition flags, u32 body length, then request ID (16 bytes), Worker ID (16), Worker generation (u64), Host creation/epoch/terminal IDs (48), Server-assigned input attachment ID (16), u8 record count (1–64), and ordered records. The maximum body is `105 + 64 * 7 + 1,048,576` bytes, checked before allocation. The retired kind-13 request is not decoded as a current request. No target rebinding or controller/viewer/resize authority is granted.

Record tags 1, 4, 5 and 6 carry text, opaque bytes, paste content and semantic stroke, respectively; tags 2 and 3 are invalid. Exact fields and bounds belong to the [native send contract](../../schemas/client-server/README.md#current-native-contract). `SendKeyStroke` independently validates modifier bits 0..15, named keys 1..13, Insert, F1..F25 and non-control Unicode scalars. The aggregate decoded payload limit is 1 MiB. Server maps independent OBCS values; Worker forwards every semantic key through the same Host encoder.

Flag bit 0 is `SendPrecondition::IdlePrompt`; every other bit is reserved zero and refused, so a decoder that predates a flag never ignores it. An idle-prompt request admits exactly one prompt line (`SendRecords::prompt_line`): a paste of 1 byte to 256 KiB of UTF-8 with no C0 control or DEL, an optional unmodified Right, and an unmodified Enter. Encoding and decoding both refuse any other shape. The Worker sends that line to the Host as one `PromptLineInput` instead of one exchange per record, and only when its Host negotiated `prompt-gated-input`.

Kind 14 results retain request identity, input outcome (unavailable, rejected, accepted or unknown), confirmed prefix progress and independent cleanup state. Progress encodes accepted-record count and first stopped record as u8, then accepted encoded bytes as u64. A stopped request is not atomic; unknown or partially accepted input is never replayed automatically. Rejection codes 1–9 keep their meaning; code 10 is `PromptUnconfirmed`, followed by one prompt-state byte in the Host's check order (1 `not-shell`, 2 `busy`, 3 `full-screen`, 4 `unmarked`, 5 `not-at-prompt`, 6 `input-pending`, 7 `changed`, 8 `settling`), and code 11 is `HostPromptGateUnsupported`, the Worker's refusal when its Host did not negotiate the gate. Both type nothing, so their progress must be zero. Existing typed constructors and codecs validate outcome/progress/cleanup consistency. Dedicated request records coexist with the bounded OBCN result frames.

The sequence's single temporary attachment, exact incarnation fence, one total deadline and cleanup remain owned by the Server/Worker send transaction. Golden mixed-stroke frames and malformed/truncated input tests cover both protocol boundaries. Client, Server, Worker and Host must deploy or roll back together for this unpublished extension; no persisted migration is introduced.

## Worktree job messages

OBCN kind 15 `WorktreeJobRequest` and kind 16 `WorktreeJobResult` carry one bounded
`git worktree list` discovery job for the `worktree list` Server flow. A request contains
operation ID (16 bytes), Worker ID (16), Worker generation (u64), then an action tag: 1
`Start` with a u8 optional trust policy (0 absent, 1 false, 2 true), a u16 length and 1 through 4,096 directory bytes (an absolute, nonempty,
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

The 40 KiB OBCN frame limit is unchanged — 7,936 is a conservative policy bound that keeps
every arm below the payload ceiling with declared headroom. Codec tests cover every action
and outcome arm, strict goldens, the field-budget boundary, unknown tags, stage codes, and
trailing bytes; the Server and Worker guides own admission, fencing, and job evidence.

## Agent integration job messages

OBCN kind 31 `AgentIntegrationJobRequest` and kind 32 `AgentIntegrationJobResult` carry the
node's one Agent integration job. A request contains operation ID (16 bytes), Worker ID (16),
Worker generation (u64), then an action tag: 1 `Start` with a u16 length and 1 through 8,192
bytes of one encoded [Agent integration request](../agent-integration/README.md#contract), 2
`Poll` with a job ID (16), or 3 `Cancel` with a job ID (16). The job ID is the Start operation's
own operation ID.

A result echoes the operation ID (16), the action tag, and the job ID (16), then one
action-scoped outcome byte. Start outcomes: 1 Accepted, 2 Busy (another integration job still
owns the node writer; no effect), 3 InvalidRequest (the document failed the contract before any
effect), 4 SpawnThreadFailed, 5 Dead, or 12 RejectedFencing. Poll outcomes: 1 Pending; 2
Completed with a u16 length and 1 through 32,768 bytes of one encoded Agent integration report;
3 Abandoned (the job ended without a report the contract can carry, so its effects are unknown);
10 Unknown; or 12 RejectedFencing. Cancel outcomes: 1 Registered or 2 AlreadyTerminal (the final
report stays observable through Poll), 10 Unknown, or 12 RejectedFencing.

This crate validates only the byte bounds; it never parses the contract documents. Codec tests
cover every action and outcome arm, both document bounds, action-to-outcome admission, and
trailing bytes.

## Agent probe job messages

OBCN kind 37 `AgentProbeJobRequest` and kind 38 `AgentProbeJobResult` carry the node's one Agent
probe job, which answers whether each named command is found on the search path a newly started
shell would use. A request contains operation ID (16 bytes), Worker ID (16), Worker generation
(u64), then an action tag: 1 `Start` with a u8 count of 1 through 64 command names, each a u8
length and 1 through 64 bytes of visible ASCII without `"` or `'`; 2 `Poll` with a job ID (16);
or 3 `Cancel` with a job ID (16). The job ID is the Start operation's own operation ID. A name is
one bare command word, never a path, an argument or a command line.

A result echoes the operation ID (16), the action tag, and the job ID (16), then one
action-scoped outcome byte. Start outcomes: 1 Accepted, 2 Busy (another probe still runs; no
effect), 3 Unsupported (this Worker's platform has no probe), 4 SpawnThreadFailed, 5 Dead, or 12
RejectedFencing. Poll outcomes: 1 Pending; 2 Completed with a u8 count of 1 through 64 result
bytes, one per requested command in request order: 1 Found, 2 Missing, or 3 Unavailable (the
query ended without an answer); 3 Abandoned (the job ended without results); 10 Unknown; or 12
RejectedFencing. Cancel outcomes: 1 Registered or 2 AlreadyTerminal (the results stay observable
through Poll), 10 Unknown, or 12 RejectedFencing.

Codec tests cover every action and outcome arm, the count and name bounds, the refused name
bytes, action-to-outcome admission, and trailing bytes.

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

Every launch kind carries a bounded literal `LaunchEnvironment`: a `u16` entry count followed by `u16`-length-prefixed UTF-8 name/value pairs. Kind 0 carries it directly; kinds 1, 3 and 4 carry it after their selection fields. Empty and nonempty environments use this same layout.

An environment has at most 32 entries and 1,024 combined UTF-8 name/value bytes. Names match
`[A-Za-z_][A-Za-z0-9_]*`, are at most 128 bytes, and are unique ignoring ASCII case. NUL values,
`ORCHBEE_*`, `TERM`, `COLORTERM` and `TERM_PROGRAM` are rejected; empty values are preserved.
Validation precedes admission; Debug output omits values. The combined maximum cwd, preference
capture, selection or default program, argument count and environment still fit the 40 KiB frame.
There are no Project, Checkout, Worktree, Agent or Launch Profile identity fields. Cohort validation requires matching deployed binaries;
this extension does not promise mixed-version operation.

## Host query messages

Kind 34 asks the Worker for the process entries of the Terminal Hosts connected to it. Its body
after the kind is the request ID (16 bytes), Worker ID (16) and positive generation (`u64`), 40
bytes in all. The result travels in its own `OBNH` record: the bytes `OBNH`, the bytes
`00 01 00 00`, an unsigned big-endian body length, then the whole echoed request, a `u16` Host count
of at most 256 and each entry. An entry is the PID (nonzero `u32`), state (`u8`: 0 unknown, 1 ready,
2 draining, 3 stopping), terminal count (`u32`), product version and environment (each a length byte
and at most 64 bytes, in the handshake's grammar), the source commit and the cohort identity (each a
presence byte and 20 or 32 bytes), then a connection count of at most 64 and each connection's kind
(a length byte and a name) with its lowest and highest nonzero `u16` generation. Every entry is a
Terminal Host's, states no capabilities and lists its connections in strictly ascending `kind`
order. The record admits 256 maximal entries, beyond the 40 KiB frame, and the streaming reader
refuses an out-of-bounds length before allocating. Values are the `orchbee-connection-handshake`
process entries, this crate's only dependency. The Server carrier checks the whole echoed request.
The pair follows this protocol's exact-cohort rule: frame version 1 stays, and older cohorts reject
the new tag.

## Carrier and ownership boundaries

This crate owns only the closed semantic values and canonical `OBCN` frame codec. It does not open,
authenticate, admit, retain, retry, reconnect, multiplex, or apply backpressure to a connection.
Its streaming helpers borrow a caller-owned `Read` or `Write` only for one frame and retain no
socket, buffer, connection, or phase state.
Unix-domain sockets, same-user peer credentials, executable cohort, the `OBLW` admission handshake,
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

Worker authorization/capacity implementation tests cover below-limit admission, the full threshold, rejection of a new identity, occupied-slot priority, result clearing, cached replay, higher-fence replacement only after the global slot is empty, rejection preservation and classification priority. Production retains exactly 1,024 bindings per Worker generation, with no release, TTL, LRU or eviction path; explicit retirement remains required before production. This sampled implementation evidence replaces the separate bounded capacity model and is not an exhaustive proof over all 1,024 entries.

## Verification

Run from the repository root:

```sh
cargo test -p orchbee-capability-node-protocol --all-targets --all-features --locked
cargo clippy -p orchbee-capability-node-protocol --all-targets --all-features --locked -- -D warnings
cargo test --locked -p orchbee-worker --all-targets --all-features
pnpm test:tla:inventory
pnpm lint:projects
```

## Managed Worker shutdown

The Windows managed Session lifecycle adds kinds 19/20 under frame version 1 and the
existing 40 KiB bound. The request contains operation ID and Worker ID (16 nonzero
bytes each), positive Worker generation (u64), a cooperative budget (u32,
1..60000 ms, one whole Server stop attempt), all integers big-endian, and a terminal disposition byte: 1 Dispose
contains and disposes every terminal, 2 Keep leaves the Host and every terminal running
for a Server `exit`. Only the exact admitted managed Worker may
act; unmanaged or mismatched identity is a pre-effect refusal. A shutdown may spend its whole
budget before it answers, so the Server carrier waits for its reply for the budget plus five
seconds, and never less than the ordinary 20-second reply window. A reply echoes the complete
request.

Outcome tags are 1 NoHost, 2 Stopped (Host epoch, u32 disposed count), 3 Refused
(1 wrong Worker, 2 unmanaged), 4 Incomplete (optional Host epoch, u32 disposed
count, stage and failure), and 5 Released (Host epoch). NoHost means no owned Host was
started and background jobs settled; Stopped requires the owned Host's exit after
containment/disposal. Released answers only Keep: background jobs settled and the
Worker closed its Host connection without inventory, containment, disposal or a Host
exit wait, so the Host keeps every terminal; the Worker then ends.
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

Viewer action tag 12 is `ReadProcessInfo`, with no bytes after the tag; it uses the request's existing attachment ID. Worker requires that exact retained viewer and sends one Host `ReadProcessInfo` (OBTP kind 79) with a fresh correlation; it sets `includeLaunchKind` exactly when that Host's handshake listed `prompt-gated-input`, so the sample then carries the Host's `launchKind` and an older Host never sees the field. The response is bounded to 64 KiB plus the envelope and returned unchanged as `Record`, including a well-formed correlated Host `TypedFailure`. An observed result must echo the exact attachment and physical terminal. Wrong Worker/generation, missing viewer, wrong Host epoch or invalid result fails through existing rejection/carrier outcomes. No process enumeration runs in Worker; Host remains the owner. A process read neither acknowledges a frame nor creates input authority. Existing action encodings remain unchanged; the development cohort must be updated together.

## Worktree mutation job contract

This private contract defines the boundary for the Worker Git owner
and Server Worktree continuation. Worker dispatch implements this boundary on Windows, Linux and macOS; the Server continuation runs on Windows. The representation alone does not establish end-to-end CLI correctness. Server owns Workspace association; Worker owns Git
checkout observations and effects.

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

OBCN version 1 kind 21 encodes operation ID (16), Worker ID (16), generation (u64),
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
is a Checkout observation: 7982 payload bytes. Both remain below the 40,960-byte
frame payload cap. Public conformance tests cover hand-derived golden frames, all
variants, maximum lengths, strict truncations and malformed fields. They do not prove
Git behavior, filesystem identity, ownership, asynchronous progress or shutdown.
Deploy the private Server/Worker cohort together; older cohorts reject additive tags.
Rollback replaces the cohort and migrates no durable protocol data.

## Source-based Worktree discovery

Kind 15 uses one `Start` action tag 1. An absent `trust_repository` selects registered-directory listing; a present Boolean selects source discovery and its command-local trust policy. Source paths are bounded absolute UTF-8 directories. Start result tag 6 means unsupported execution. Poll result tag 13 is `SourceCompleted`: common-directory, source-checkout, listing and stderr bytes share the 7936-byte variable-data bound, with four explicit lengths. Worker owns execution and cleanup; the read consumes no mutation job receipt. `tests/worktree_source.rs` covers canonical bytes, truncated/trailing input, optional trust and combined bounds.

Registered-directory list results retain native Unix path bytes when they are not UTF-8; this is a current producer, not an old-peer representation. Source-query results validate one UTF-8 path representation and reject byte-path entries.

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

## Explicit controller takeover

Control request action tag 5 is Takeover, followed by the expected previous controller ID
(nonzero 16 bytes). The outer controller ID names the fresh replacement. Only Acquired, Rejected
or a transport failure may answer it; the Server maps uncertain delivery without replay.
Worker validates the exact terminal and incumbent, acknowledges old Host detach, then acquires
the replacement within one serialized operation. An already-released incumbent is allowed;
another live incumbent is refused. See the [transfer model](../../apps/orchbee-server/docs/terminal-controller-takeover.md).
Existing tags remain unchanged; mixed private cohorts are unsupported.

Launch selections additionally cap worst-case JSON string-content bytes at 30 KiB: ASCII controls count as six, quotes and backslashes as two, and other UTF-8 bytes as one. This preserves the previous 2 KiB input domain while bounding the larger resume commands. Matching cohorts are required for the enlarged limits.

## Retired launch kind 2

Authorized launch kind 2 carried a per-launch default-shell override. Shared launch profiles now
select the default program, so decoders refuse kind 2 as invalid payload.

## Plugin job records

Plugin requests/results use distinct `OBPJ` records: magic, encoding version 1 (`u16`), reserved zero (`u16`), bounded big-endian `u32` body length, maximum 2 MiB. Ordinary OBCN frames stay unchanged. Bodies carry direction, operation ID, Worker ID/generation and checked start/list/get/cancel or started/logs/cancellation/refused variants. Strings and byte buffers have `u32` lengths, argv counts `u16`, environment/page counts `u8`; no trailing bytes are accepted. Frozen intent is capped at 160 KiB, argv at 128 entries/128 KiB and each retained output stream at 64 KiB. Pages carry at most four descending logs and a consistent next sequence. Frame validation does not grant execution authority. Server/Worker matched cohorts activate together; [the owning native plugin guide](../../apps/orchbee/docs/windows-plugins.md) defines lifecycle and semantics.

## Plugin terminal display environment

An explicit terminal launch may carry a separate `PluginTerminalEnvironment` (`pluginEnvironment` in OBCS). It admits only plugin ID/root/entrypoint/config/state/context, native binary path and optional Workspace/Tab/Pane display IDs: ten unique named entries, seven required, at most 24 KiB combined UTF-8. These are display data, not authority or plugin registration. The ordinary caller environment retains its existing limits and reserved-name refusal. Runtime base/scope and Worker/terminal identities remain owned by the Worker adapter. A plugin environment requires an explicit executable. OBCN launch kind 3 transports the explicit selection, ordinary environment and plugin environment under the 40 KiB frame limit. Server checks the complete encoded plugin launch before reserving a physical operation. Worker removes inherited known plugin display fields before applying this launch's values. Workbench plugin resolution, Pane association and Client placement remain separate integration responsibilities. Focused boundary and launch-fact tests cover this carrier; this is not a complete plugin Panel receipt.

## Windows physical title observations

`TerminalTitlesRequest` uses OBCN request kind 23 and carries operation identity,
Worker identity/generation and at most four complete Host identities. The
observation-only Windows adapter drains already queued Host metadata; it cannot
start a Host, attach a viewer or claim input/resize. Replies echo operation and
ordered physical identities, distinguishing unavailable from observed null clear.

`TerminalTitlesResult` uses a separate OBNT version 1 frame: magic/version/zero
reserved bytes, big-endian u32 payload length, 16-byte operation ID, u8 row count,
then each 48-byte Host identity and u8 observation tag. Tag 0 is unavailable; tag 1
adds positive u64 metadata revision and u8 nullable-title tag. A present title adds
u32 UTF-8 byte length and complete bytes. Rows cap at four, each title at 64 KiB;
the whole payload caps at 17 + 4 * (62 + 65536) bytes before allocation. Decoders
reject bad tags, UTF-8, duplicates, versions, reserved bytes, zero revision,
truncation and tails. Ordinary OBCN limits stay unchanged. Matched private cohorts
ship/roll back together; no persisted data or Host protocol changes.

## Physical title changes

A Worker announces changes to its physical title projection and the Server reads them by cursor,
so a background terminal's title reaches the Server without a viewer. The contract is
platform-neutral. Titles stay untrusted terminal text: the Server projects them with
`orchbee-workbench-protocol`'s `project_terminal_title` before any Client sees them, and paces,
retains and discards what it reads.

The Worker numbers each change to its projection with the next value of one increasing `u64`
change sequence, which starts at 0 for each Worker generation and never repeats within it. A change
is a new title or revision for a terminal, or a terminal the Worker stops observing.

Kind 35 `TerminalTitlesChanged` is the Worker's unsolicited note that its projection holds changes
after the cursor of the last change read it answered. After the kind it carries the newest change
sequence `through` (positive big-endian `u64`), 9 payload bytes in all. It moves no title, grants
nothing and has no response. The Worker writes it only between requests, never inside another
record, and writes no second note before it answers a later change read with a page or a reset. A
Server whose cursor already reached `through` drops the note; a lost note costs only latency.

Kind 36 `TerminalTitleChangesRequest` carries the request ID (16 bytes), Worker ID (16), positive
generation (`u64`) and cursor `after` (`u64`), 49 payload bytes in all. Cursor 0 reads every current
title afresh; any other cursor is the `next` of the last page the reader applied from that Worker
generation. The result has three outcomes:

| Outcome           | Meaning                                                                                                                                                                                                                               |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Page`            | Each terminal changed after the cursor, once, in order of its latest change, with its observation as of `next`: observed with revision and nullable title (null is a title the program cleared), or unavailable (no longer observed). |
| `Reset`           | The Worker cannot resume the cursor: it retains less history, or it replaced its Host connection after that cursor. The reader discards every title it holds from that Worker generation and reads again from 0.                      |
| `RejectedFencing` | The request named another Worker or generation; nothing was read.                                                                                                                                                                     |

A page holds at most 256 rows and 1 MiB of titles in all, each title at most 64 KiB, so a page is
never empty while changes remain. A fresh read lists no unavailable row and is never reset. `next`
never passes the Worker's newest change, never falls behind the cursor and passes it whenever the
page has rows; it may pass it without rows when every change after the cursor was superseded or
removed. `complete` says whether `next` reached the newest change; an incomplete page has rows, and
the reader continues with another read at once.

The result travels in its own `OBNC` record: the bytes `OBNC`, the bytes `00 01 00 00`, an unsigned
big-endian `u32` body length, the whole echoed request (48 bytes) and the outcome tag: 1 page, 2
reset, 3 rejected fencing. A page adds `next` (`u64`), `complete` (strict 0/1), a `u16` row count and
the rows. A row is the OBNT row above: the 48-byte Host identity and observation tag 0 unavailable,
or 1 observed with positive `u64` revision and title tag 0 null or 1 followed by a `u32` UTF-8 byte
length and the bytes. The body length lies between 49 and 49 + 11 + 256 × 62 + 1 MiB bytes, refused
before allocation. Decoders reject unknown tags, noncanonical Booleans, invalid UTF-8, zero
revisions, through values and generations, duplicate terminals, over-bound titles, a page breaking
the cursor rules above, a reset of a fresh read, truncation and tails; the Server checks that the
echoed request equals its own. Frame version 1 stays, older cohorts reject the new kinds and
magic, and the matched Server/Worker cohort ships and rolls back together with no persisted data.

## Workspace Git status observations

The existing Worktree job request/result kinds 15/16 add `StartStatus` action tag 5
and `StatusCompleted` poll tag 14. Directory validation and identity/generation
fences are unchanged; status/stderr use separate u16-prefixed byte fields with the
existing 7936-byte summed budget. Status observes branch/upstream facts and remains distinct from Start discovery.
The Worker owner on Windows, Linux and macOS reuses the discovery slot, TTL, cancellation,
settlement and retained receipt. No mutation/Host
startup/input authority is added. Runtime ownership is in the
[Workspace sidebar model](../../apps/orchbee/docs/machines.md#workspace-sidebar-model).

## Viewer selection and export controls

Private viewer action tags 16–20 extend the already admitted attachment; the action body never
repeats or changes its identity. Tag 16 is `SelectionGesture`: positive big-endian u64 generation,
then gesture tag 1 Start, 2 Update, 3 End, or 4 Cancel. Start carries click count, viewport point,
signed pointer position, and canonical 0/1 select-output. Update carries cell width, columns,
left padding, screen height, viewport point, pointer position, and canonical 0/1 rectangle. End
carries only its viewport point. Points are `u16` values below 4096, click count is `1..=3`, pointer
axes are `-65535..=65535`, cell width and columns are `1..=4096`, and screen height is nonzero.

Tag 17 `SelectionExport` carries positive big-endian u64 export sequence and closed format tag 1
Plain. Tag 18 `SelectionExportChunkApplied` carries that sequence plus big-endian u32 chunk
sequence. Tag 19 `SelectionExportCancel` carries the export sequence and reason tag 1
ClipboardFailed, 2 Disposed, 3 Replaced, or 4 User. Tag 20 `SelectionCommand` carries positive
big-endian u64 generation, big-endian u64 revision, then command tag 1 Clear, 2 SelectAll, 3
SetRange, or 4 SelectOutput. SetRange carries its start point, end point, and canonical 0/1
rectangle; SelectOutput carries one point. A point is a `u16` column below 4096 and a big-endian
u64 absolute row. Unknown tags, zero positive values, noncanonical Booleans, out-of-range fields,
truncation, and tails fail before Worker Host access. All viewer tags 1–15 retain their bytes.

Worker maps the five controls to one exact attachment- and correlation-scoped OBTP exchange.
SelectionChanged, SelectionExportStarted, and the cancelled SelectionExportFinished return as the
unchanged opaque `Record`; a matching chunk-applied echo returns `FeedbackAccepted`. A correlated
typed Host failure to any of the five, such as a gesture for a superseded generation or the
cancellation of an export that already finished, also returns as the unchanged opaque `Record`, so
Server relays it and the Client reads its code; any mismatched response is a carrier failure.
Poll-delivered raw export chunks remain opaque bytes in the viewer FIFO. This carrier does not
assemble text or write a clipboard. Public fixed-byte and malformed-input conformance tests cover
every gesture branch, format/reason tag, old-tag preservation, and exact response conversion. The
matched private cohort deploys or rolls back together with no persistent migration.

## Read-only viewer lifecycle observation

Private viewer action tag 15 `ReadLifecycle` pins Host creation ID, epoch and terminal ID (three 16-byte identities). Result tag 7 `LifecycleObserved` echoes that identity, a big-endian u64 revision and a closed 0/1 settled byte. Worker validates its admitted viewer target and walks Host inventory page by page under one bounded budget, answering `Rejected` when the walk cannot finish; only exact Exited/Failed summaries mean settled. Absence, paging/identity conflict and transport failure do not mean exit. The read never enters the exclusive lifecycle kernel or claims Close ownership. Server checks Worker generation, exact target and temporary-viewer cleanup before publishing settlement. Older tags keep their bytes; old cohorts reject these additive tags. Upgrade and roll back the matched cohort together.

## Viewer-scoped PNG requests

`TerminalGraphicsDecodeRequest` is a read-only operation on an already admitted attachment. OBNG v1 uses a 12-byte header and 64-byte identity/dimension prefix, followed by at most 16 MiB of PNG. The length bound is checked before reading or allocating the body. A result reuses the exact request/attachment-correlated viewer carrier (OBNR), containing either the bounded Host reply or a typed viewer rejection. The operation does not authorize a Host start, PTY input, lifecycle change or another connection’s attachment.

## Read-only Close inspection

Lifecycle capability version 1 uses one current layout. Action 6 `InspectInventoryPage` has the
same cursor and page representation as action 4, but is read-only and never claims, advances or
replaces a retained Worker Close owner/ordinal/mutation receipt. It still validates Worker
generation, binding fence, lease and complete Host identity before handoff. A preflight can
therefore be followed by a fresh confirmed Close without a phantom lifecycle conflict. Only the
current capability may carry action 6. Action tag 5 and HostFailure reason 3 are unassigned, and
decoders reject them.

An inspection page reports phase and revision only; it carries no idle classification. Whether a
conditional Close asks first is the Server's own decision from its read of the terminal. Independent
byte goldens, Worker receipt tests and the Server conditional-close witnesses cover this
representation. Upgrade and roll back the matched native cohort together; no mixed-version safety
bypass is supported.
