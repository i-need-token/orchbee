# Rust Orchbee Worker

Experimental private Rust Worker application for admitted local Session operations on Windows,
Linux, and macOS, with explicit-endpoint POSIX support. The emitted `orchbee-worker` executable belongs to
the verified four-artifact cohort and participates in the root Cargo gate. Server owns activation;
Worker owns its admitted Host connection and authorized job lifetimes.

Simplified Chinese maintenance translation:
[README.zh-CN.md](README.zh-CN.md).
English remains canonical.

## Windows scrollback editor selection

The private editor-command query resolves a launch on the admitted Worker without
starting a Host, creating a document or launching an editor. It selects nonblank
`VISUAL`, then nonblank `EDITOR`, then the system directory's `notepad.exe`.
Native `CommandLineToArgvW` preserves literal argv. Invalid selected values refuse
whole rather than trying the next preference. Non-UTF-8 and NUL are refused;
the existing 128-argument and 30 KiB escaped launch-selection limits apply.

Absolute programs must exist. Bare names search only absolute inherited `PATH`
directories, using the literal name and an `.exe` candidate when no extension is
present. Relative program paths, current-directory lookup, `PATHEXT`, `ComSpec`
and shell reparsing never select an executable. The default uses `GetSystemDirectory`,
independent of inherited `SystemRoot` or `PATH`. No PowerShell arguments are added.

The private reply echoes the entire generation-fenced request and grants no source
read or launch authority. Current Client/focus admission, exact authorized capture,
independent overlay and public CLI remain upstream obligations; only Host appends
its one generated document path and owns the deferred terminal through cleanup.

An authorized deferred launch can carry an existing read-viewer source. Worker
checks its exact open attachment target, current Host epoch and non-disposal before
handoff; Host independently checks actual read authority and immutable capture
generation/revision. No source is created or borrowed across Worker incarnations.
The Host creation adapter emits `CreateScrollbackEditor`; Worker creates no file,
appends no path and preserves literal editor argv without PowerShell augmentation.
Lazy mode refuses such a launch before Host startup. Failed preparation preserves
the existing pre-handoff receipt; post-handoff loss stays unknown without replay.

## Windows child CLI routing

Managed Windows launches overlay `ORCHBEE_RUNTIME_BASE` and `ORCHBEE_RUNTIME_SCOPE` from the
admitted Session runtime directory, alongside the existing terminal and Worker identities.
The overlay wins over inherited and launch-supplied values. Both deferred and immediate
launches use the same route; unavailable or invalid path text rejects preparation instead
of silently routing child CLI commands to another Session. Explicit-endpoint launches without
a managed runtime retain their existing behavior.

## Process boundary

On macOS and Linux, the exact private development invocation is:

```text
orchbee-worker \
  --local-endpoint <absolute-server-socket> \
  --terminal-host-runtime-directory <absolute-directory> \
  --terminal-host-endpoint-id <32-lowercase-hex>
```

Worker first connects to the supplied Server Unix-domain socket, requires the same effective
operating-system user, submits its bounded cohort handshake, and verifies typed admission. It does
not contact Terminal Host before Server accepts that Worker connection.

After Server admission, Worker resolves only `<runtime-directory>/<endpoint-id>.s`, connects to the
existing Terminal Host, independently requires the same effective user, and performs the existing
exact-version Terminal Protocol `Hello`/`HelloAccepted` exchange. The request identifies one fixed
diagnostic Worker broker client, advertises the protocol hard limits and no optional feature, and
is accepted only through the registered request/response roles, exact correlation, exact protocol
version, matching negotiated limits, an empty feature intersection, and a nonzero Host epoch.

Only after both peers accept does Worker write `ready\n` to stdout. Its main thread then runs one
serialized OBCN Launch/Start, lifecycle, viewer, controller, and cwd-validation request loop over
the two owned sockets. It has no connection thread or operation queue; each viewer owns one bounded
delivery slot. Server loss closes the Host broker socket. Host loss stops admission and allows only
a bounded delivery attempt for an already classified result before closing the Server attachment.
Either peer loss exits silently with status `1`; retained terminals remain owned by Host. Invalid
invocation, connection, authentication, handshake rejection, framing, output, or transport failure
has the same silent status-`1` result. A valid operation-level rejection returns its typed OBCN result
and leaves the loop available for the next request.

Worker consumes authorization only from the admitted same-user Server and uses the OBLW generation
for its authorization kernel. It sends Create/Start only after local admission and a fresh handoff
check. Explicit-endpoint mode does not start a Host; managed Linux mode uses the lazy workflow below.
Connection retry, restart, inventory adoption, PTY/VT operations, Agent behavior, service installation,
and recovery remain excluded.

## Managed Windows explicit shutdown

The managed Windows Worker retains its exact Host child handle until completion. A generation-fenced
shutdown request permanently closes ordinary operation admission; unmanaged and wrong-generation
requests refuse before effects. Shutdown first stops every [background job owner](#background-job-owners-and-exit), then uses the
existing Host inventory, force and dispose contracts under one request deadline (at most 15 seconds).
Each inventory request selects the first remaining terminal; force is sent once per selected active
terminal in an attempt, and disposal requires a disposable observed phase and an exact acknowledgement.
Prepared target cleanup retires local viewer/controller ownership only after that acknowledgement.
An empty complete inventory permits broker disconnection, followed by a wait on the retained Host
handle. Pipe disappearance alone cannot establish shutdown. The Server must separately confirm
Worker exit before publishing restart authority.

Incomplete attempts retain their failure stage, reason and count of known disposal acknowledgements
from that attempt. The Worker then accepts only explicit shutdown requests. A subsequent attempt may
continue a healthy connection or wait for an already-disconnected, empty Host; protocol or transport
uncertainty retires the Host connection and never authorizes mutation replay, reconnection or adoption.
Unknown background job cleanup prevents success. A no-Host or confirmed Host-exit result ends the
Worker after its bounded result-write attempt; ordinary Server loss retains its existing semantics.
This Windows lifecycle does not add a Linux or macOS stop path.

## Managed Linux lazy Host startup

The private `--runtime-directory <absolute-runtime-directory>` entry verifies the shared runtime and
four-artifact cohort, then registers with Server without creating a Host directory or process.
The [shared modeling record](../../crates/local-runtime/README.md) owns process/failure boundaries.

The first Launch passes the existing authorization kernel and resolves launch facts before Host
startup. Typed invalid-lease/generation rejection creates no Host. Generation-pinned cwd validation
and worktree-job requests may complete before that Launch without starting a Host — the discovery
slot survives the handoff so an admitted job stays owned across it; any other non-Launch traffic
retires the connection. The admitted dispatch and same kernel survive Host startup, and authorization is
checked again before handoff. The Host directory is `host` under the runtime, with fixed endpoint ID
`00000000000000000000000000000001`. The absolute endpoint address under that directory is checked
against the platform socket-address ABI before the directory or the Host process is created; an
address that cannot fit produces the same pre-handoff failure without either side effect. Any pre-existing endpoint immediately requires recovery; only the
newly spawned Host may be awaited for its final 0600 socket.
Host startup, its one dial, and strict OBTP handshake share five seconds without renewal or retry.
Preparation/startup failure returns a pre-handoff failure when deliverable and retires Worker;
uncertain handed-off outcomes are never replayed. Later operations use the same serialized loop
and replay cache.

Every Launch on this managed path also propagates the local Agent observation channel through the
existing Terminal Protocol `environmentOverlay`. For each admitted Launch the launch adapter joins
the cross-owner file name `agent-observation-v1.sock` onto the verified `--runtime-directory` runtime
directory and admits the candidate only when `lstat` proves an existing socket owned by this
Worker's effective user. An admitted socket sets
`ORCHBEE_AGENT_OBSERVATION_SOCKET` to its absolute path inside the existing 16-entry and 4 KiB
overlay bounds; a missing, non-socket, foreign-owned, or non-UTF-8 candidate emits no variable and
never fails the launch. The gate re-evaluates on every Launch instead of caching admission. The
explicit-endpoint path carries no Session runtime directory and has no observation overlay — remote and
non-managed launches gain no observation channel, and no CNP field or Server-private import is
introduced for it.

The child reaper only waits for exit; it never kills Host. Host retains terminals after the
Server/Worker connection closes. Restart adoption and replacement Host are unsupported.
Explicit-endpoint mode retains its existing two-peer readiness semantics.

## Worktree discovery job

The serialized request loop admits OBCN worktree-job requests (start/poll/cancel) for the
`worktree list` Server flow both before the first Launch and after Host admission. One process-level slot runs one discovery job at a time — a
`git rev-parse --path-format=absolute --git-common-dir` probe followed by `git worktree list
--porcelain -z` inside the Server-supplied directory — under a fixed five-second TTL counted
from admission and one shared 64 KiB stdout budget across both stages; the stage that crosses
the combined budget owns the `too-large` attribution. Every request is fenced by the exact
`{workerId, generation, jobId}` tuple and answers a typed result frame; `rejected-fencing`,
`busy`, and `dead` are honest outcomes, never fabricated completions. The job reports raw
bounded bytes only — porcelain parsing and malformed-listing rejection are the Server's
authority, not the Worker's. The owner reaps its child autonomously — a dead or evicted
job retains only a bounded receipt until the next start replaces it; cancellation keeps the
terminal outcome observable through `poll`.

## Agent integration job

`agent_integration_jobs::AgentIntegrationJobs` answers OBCN kind 31 in both the pre-Host and Host
request loops and is the node's only Agent integration writer. Every action is fenced to this Worker
ID and generation before owner access. Start decodes the contract request, then runs one writer
thread; while it runs, another Start answers `Busy`, and a replayed Start with the same document
answers `Accepted`. The writer uses this Worker's own home directory and `PATH`, under the request
budget or the contract's 40-second ceiling. Cancel sets the cooperative termination the writer
observes between bounded steps. Poll returns the encoded report; a job that panics or whose report
exceeds the contract bound is `Abandoned`. Eight settled jobs stay pollable, and admission evicts
the oldest settled one first.

## Background job owners and exit

Each Worker generation owns four kinds of background jobs. The request loop borrows them;
pre-Host to Host handoff and Windows Server handoff keep the same owners, so ownership never
moves to another thread or connection.

| Job                            | Owner                                           | Platforms | Physical cleanup bound after cancellation |
| ------------------------------ | ----------------------------------------------- | --------- | ----------------------------------------- |
| Worktree discovery             | `worktree_discovery_job::WorktreeDiscoverySlot` | all       | five-second Job/child settlement          |
| Worktree mutation              | `worktree_mutation_job::WorktreeMutationSlot`   | Windows   | five-second Job settlement                |
| Plugin and configured commands | `plugin_jobs::PluginJobs`                       | Windows   | five-second Job settlement                |
| Agent integration              | `agent_integration_jobs::AgentIntegrationJobs`  | all       | 0.6-second provider command teardown      |

Every owner separates the stop request from task completion. `request_stop` closes new
admission where the owner has admission, signals every running task and never waits.
`stop_before(deadline)` requests stop, then joins tasks only after they finish: `Ok` means
every task has ended, `TimedOut` means stop is requested but completion is unproven, and
any other error means cleanup is unknown and admission is retired. Drop requests stop and
joins only already-finished tasks; it never waits for an unfinished one.

`terminal_operations::background_jobs` applies one contract to every exit path. It
requests stop from all owners before waiting, observes each owner against one shared
deadline even after another fails, and reports the first failure. Explicit shutdown uses
the request deadline (at most 15 seconds for the whole shutdown). Every other loop exit,
including Server disconnection, protocol failure and Host start failure, uses one
six-second window, which exceeds each owner's five-second cleanup bound. Unproven cleanup
takes precedence over the loop's own error. A task still running when the window closes
is detached, and the Worker reports the cleanup failure instead of claiming success.

## Live viewer relay refinement

The Worker owns one Host broker; Server alone issues viewer attachments. Each generation-scoped viewer has read capability, an independent bounded delivery slot and at most one frame awaiting exact Client feedback. Worker validates the envelope and fixed routing metadata, retaining the opaque Host record without decoding rows, VT or history. Attach/poll/feedback/detach serialize with Launch/Start on the admitted Server connection. Poll transfers ready records without awaiting terminal output; downstream delivery cannot retain the Worker operation slot. The Server relay core abstracts lane isolation; concrete carrier tests check this integration.

Beside the frame record slot, each viewer keeps a bounded four-deep FIFO of complete opaque viewer
events counted in the same retained-byte budget. It carries kind-53 `HistoryChanged` plus the
selection and external-effect records named below; Worker decodes only the attachment and event
family identity needed to validate exact routing and ordering. Bell and clipboard-write keep
separate strictly increasing sequence cursors, so the same sequence value may occur once in each;
their accepted records share the FIFO's arrival order. Poll serves one of two sources — the
frame/projection-blocked record slot or the event FIFO — under an explicit next-preference that flips
after every successful delivery and prefers the record slot initially, so an unsettled frame flight
never blocks a queued event and a continuously ready source overtakes the other at most once. Event
delivery never touches `flight.delivered`, `last_sequence`, or frame acknowledgement state, and
`HistoryObserved` travels the existing feedback lane without settling a frame flight. FIFO or
aggregate-byte saturation closes only that viewer; close, remove, cleanup, Host loss, and target
cleanup release frame and event bytes exactly once. A valid viewer event addressed to an
already-retired viewer is consumed like a late frame record instead of retiring the broker.

The OBCN viewer carrier and Worker attachment transitions now serve Server's admitted Client
Session relay, with actual Host process traces. Server owns OBCS viewer messages and Session cleanup;
bare Linux entry now consumes this path.
Twenty-four live viewers, at most one retained Host record per viewer, and a 128 MiB aggregate
retained-record budget are implementation bounds, not release performance budgets; the owning
model is the [viewer capacity and record budget reference](docs/viewer-capacity-and-record-budget.md).
A missing or contradictory feedback never acknowledges a Host frame.
`NeedSnapshot` preserves its reason and sequence; `ProjectionBlocked` remains an attachment event.
A full delivery slot closes only that viewer and releases its queued bytes, and a valid record
that would cross the aggregate budget retires only its addressed viewer the same way. Its Host detach runs
before the next ordinary Server operation; lifecycle steps bypass this cleanup. Late events for that closing viewer are drained without retaining
another frame. No blocked event or frame is silently replaced. The broker's `host_connection`
module owns event routing, viewer slots, controller grants, and the shared deadline exchange; `terminal_viewer_operation`
maps admitted read-only commands and validates their exact Host responses. No viewer-state borrow
survives Host I/O. Attach performs two sequential bounded exchanges (capabilities, then active
projection demand); cleanup may perform up to thirty-two bounded viewer/control detach exchanges before a request.
Server now budgets twenty seconds for those exchanges and OBNR record delivery. Lifecycle and
metadata notifications are validated and drained, while application-signal effects use the Agent
drain. Bell, clipboard-write, selection-changed, selection-export-chunk, and
selection-export-finished records retain their original bytes in the exact viewer's bounded event
FIFO and leave through Poll; Worker never performs their external effects. Their Client behavior is
owned by [native terminal selection and effects](../orchbee/docs/native-terminal-selection-effects.md).
Detach and broker loss release transient attachments and preserve retained terminals. Reattachment
requires a fresh Server-issued attachment identity; no input or resize authority is granted here.

Evidence maps the existing authority/relay model to deterministic routing, exact-feedback, stale
scope, and capacity tests, plus a real Worker/Host trace with two viewers, a stalled viewer, snapshot
feedback, detach, reattach, and retained terminal lifetime. Wire tests check bounded framing and
opaque byte preservation. The current-context challenge pass is not independent review evidence.

## Controller grant and effects

`terminal_control_operation` consumes Server-issued OBCN acquisition, input, resize, paste, and
internal release requests; paste arrives as the dedicated `OBNQ` record on the same stream.
`host_connection::controllers` owns bounded pure grant state: one controller per
Host terminal, the exact viewer and controller identities, pending or accepted Host capability
revision, and the highest consumed sequence. No grant-state borrow spans Host I/O. Both viewer and
controller adapters use the correlated, deadline-bounded exchange owned by `host_connection`.

Acquisition requires a live read-only viewer and creates a separate Host attachment with read
disabled, input and resize enabled, and the resize lease requested. It preserves that viewer's
projection baseline. Only an exact capability acknowledgement commits the grant. Input preserves
1 through 4,096 raw bytes and uses the accepted capability revision; the Host must acknowledge the
entire byte count. Resize must acknowledge the complete geometry. Paste preserves 1 through
262,144 opaque bytes as kind-39 `PasteAutoInput`; the Host must acknowledge a positive
`accepted_bytes` no greater than `min(2 × content + 12, negotiated input limit)`, and its typed
failures map to the four `HostInput*` rejections. These actions reject a wrong
viewer/grant or non-increasing sequence before Host I/O. Known Host rejection consumes the sequence;
ambiguous or contradictory effects close the broker without replay.

Viewer detach or closure first releases its controller, then detaches the viewer. Internal release
is idempotent only when that viewer has no controller; it never clears a different current grant.
Unexpected resize-lease revocation of a live grant retires the broker. Exact retired-target events
after known Dispose are validated and drained. Neither release nor loss promotes another
viewer or terminates retained execution. Controller grants stay capped at eight regardless of
viewer count; cleanup and normal operations share the single Host writer.

Deterministic grant tests cover exclusivity, stale scope, pending acquisition, sequence exhaustion,
and explicit re-acquisition. Fault process tests cover silent and contradictory Host input
acknowledgements with no replay. Server's real two-Client process witness verifies shell input,
actual PTY geometry, non-controller rejection, detach, and a fresh grant for the other Client.
These runtime witnesses refine the existing authority/relay models; they do not establish public
TUI behavior, independent review, or measured release performance.

### Semantic paste admission modeling record

The model folds Server's per-controller sequence and Worker's per-grant sequence into one
`grantSequence`, keeps a static grant set, and folds child exit, writer stop, failure fencing, and
client input closure into `terminalAlive`. Preparation snapshots the bracketed-paste mode under
the terminal mutex; commit publishes under the separate input-admission lock after Core
re-authorization — the model never claims one mutex spans both. It asserts no fairness:
`ReservationReleasedOnce` is transition safety, `SettledRequestHasOutcome` states only
`settled => outcome # "none"`, and exactly-once sequence consumption follows the unidirectional
phase graph. Byte budgets, identity widths, and the four typed rejection codes remain
codec/schema/process-test evidence, not model claims.

## Default interactive launch resolution

The Worker never opens the shared preference store or `config.toml`. The Session reads its frozen
store once before it admits each creation and sends the confirmed scrollback, terminal default colors
and, for a default launch, the selected profile's command in the
[launch preference capture](../../crates/capability-node-protocol/README.md#launch-preference-capture).
A saved change therefore reaches only later creations; running terminals, cached launch facts and
deferred Start keep their capture. The Worker resolves device facts itself: explicit programs
bypass default-shell discovery, a custom profile uses its literal argv, and the System shell uses
the resolver below with optional POSIX login. The Worker runs that resolver once per generation,
before it registers with the Server, and reports the resolved shell's file name in its
[`WorkerHello`](../../crates/local-worker-protocol/README.md#framing-and-messages) so the Server can
name the generation's default launches. Every default launch of the generation reuses the result:
a shell installed while the Session runs becomes the System shell only in the next Session, and
removing the resolved shell turns later System-shell launches into the Host's typed launch failure
until then. An `Unavailable` capture refuses only that creation before Host effects. An `Absent`
capture keeps the System shell, Host colors and Host scrollback.

The shared `UserTerminalColors` resolver supplies Host-default Light/Dark from explicit application
appearance, while System keeps the existing Host baseline. Explicit terminal Light/Dark/Custom win.
The launch adapter installs only those foreground/background defaults and an explicit Light/Dark
scheme, preserving ANSI palette entries and cursor settings. Host OSC 10/11 replies therefore use
the captured launch colors. Existing terminals and per-Client Settings previews never trigger a
Worker theme mutation; see [native themes](../orchbee/docs/native-themes.md).

When an admitted OBCN Launch carries an explicit launch selection, the request loop adopts its
executable and argument vector, with only the Windows interactive PowerShell refinement below:
no `PATH` search, no shell resolution, and no
silent fallback to the default shell — a spawn failure is the Host's typed launch failure. The
selection shares the same default environment and cwd/profile composition below.

The shared runtime's `default_interactive_launch` module supplies the running request loop's
launch-fact composition only when no explicit selection is present. Its transport-neutral
`DefaultInteractiveLaunch` output owns only one
resolved executable, an exactly empty literal argument vector, and three fixed environment entries.
The loop combines these with Worker-local cwd, Terminal Protocol profile, geometry, and independent
creation identity. The three entries map to the Terminal Protocol launch environment. The
invocation-scoped `CreateTerminal.environmentOverlay` stays empty on the explicit-endpoint path;
on the managed local-runtime path it carries the single Agent observation variable described below.

Per-launch `cwd` arrives already selected by the Server: explicit override, accepted Workspace
directory, then Worker home if absent. Host owns the actual directory check and spawn failure.
The loop appends the validated OBCN environment to the three default launch entries, for either
an explicit program or the default shell. Host inherits its environment and applies these literal
entries; Worker process environment is never changed. The caller cannot override Orchbee or
terminal identity variables. This is launch data, separate from the observation-only
`CreateTerminal.environmentOverlay` described above.

Resolution considers an inherited `SHELL`, then the effective operating-system account's shell,
then `/bin/sh`. A candidate is valid only when its path is nonempty, absolute, valid UTF-8, free of
U+0000, no longer than 4,096 bytes, resolves to a regular file, and is executable by the process's
effective user at resolution time. An invalid or unusable candidate falls through without exposing
its value. If even `/bin/sh` is unusable, resolution returns the typed `NoUsableShell` failure.

The native adapter reads only `SHELL`, the effective-user account record, and executable metadata
and access state. It neither canonicalizes a selected path nor starts a process. The result always
sets exactly `TERM=xterm-256color`, `COLORTERM=truecolor`, and `TERM_PROGRAM=ghostty`; it never
copies caller-transient or product variables, reads a cwd or launch profile, emits diagnostics,
opens a Server/OBCN/Host transport, or constructs a Host command. Pure table tests own candidate
admission, all selection combinations, output facts, and typed failure; native tests cover the
executable probe and the current-machine resolution boundary. A file can change after this
observation, so the eventual Host execution boundary still owns typed launch failure.

## Worker-local authorized Host-operation kernel

The application-private `terminal_authorization` module is the running request loop's
carrier-neutral deterministic kernel for the private Host launch adapter. It consumes the semantic
[`orchbee-capability-node-protocol`](../../crates/capability-node-protocol/README.md) Launch and
Start values. The loop constructs one kernel per admitted Server attachment.

The composition root constructs this intrinsically version 2 kernel with only the current
local-Session `WorkerId` and the nonzero Worker generation accepted by OBLW.
Its carrier must authenticate the active authoritative Server and supply only well-formed issuance:
within that Server authority, every `OperationId` is globally fresh across Terminals, Workers, and
Worker generations. The kernel deliberately keeps no unbounded cross-terminal collision ledger and
does not claim to contain a Server that violates that producer invariant.
A successor Server authority may begin a new producer scope only after its generation, lease, and
fence transition makes every earlier authorization non-current.

One kernel represents one accepted Worker generation and retains at most 1,024 distinct terminal
bindings for that generation. It has no release, TTL, LRU, or eviction transition. Capacity rejects
only a previously unseen `TerminalId`; exact cached replay and a valid higher-fence replacement of
an existing terminal remain eligible when the map is full. This behavior is terminal-launch
capability version 1; a mismatched request fails closed as
`UnsupportedCapability` before capacity is considered. The current kernel is intrinsically a
version 2 implementation; its constructor cannot be configured to admit a legacy or future
capability while retaining version 2 capacity semantics.

Every admission or pre-handoff decision also receives an explicit Unix-millisecond time, and Start
admission receives the currently observed Host epoch. A returned dispatch is permission to prepare
the matching operation, not evidence that a Host effect occurred. The adapter must call
`begin_host_handoff` with a fresh observed Host epoch immediately before the first possible Host
effect, then report one typed Host completion, definite pre-handoff failure, or post-handoff unknown
outcome through the same private dispatch token. The kernel retains the maximum time it has
observed, so a caller-visible wall-clock rollback cannot revive any expired lease.

### Authorization-kernel modeling record

| Field        | Current contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | The OBCN carrier may deliver a Server-authorized Launch or Start more than once, late, for the wrong Worker generation, or after authority has advanced. Worker must decide locally whether one Host operation may begin and must preserve the exact terminal result without acquiring Server placement authority or performing the Host effect itself.                                                                                                                                                                                                                                                                                                                                                             |
| Inputs       | Canonical OBCN `AuthorizedTerminalLaunch` or `AuthorizedTerminalStart` from an authenticated authoritative Server that obeys the globally fresh `OperationId` producer invariant, with its authorization capability checked against the kernel's intrinsic version 2 policy; the caller-supplied current local-Session Worker ID, nonzero OBLW generation, and current Unix-millisecond time; the observed Host epoch at Start admission and again at the handoff boundary; and a typed Host completion. A missing result or occupied slot remains explicit state rather than inferred success.                                                                                                                     |
| Data         | `AuthorizedHostOperationKernel` owns at most 1,024 complete highest-fence bindings per Worker generation, keyed by product `TerminalId` and including operation kind, full authorization, Launch or Start payload, and an optional exact cached result. It separately owns one process-local slot in `empty`, `awaiting-handoff`, `handed-off`, or `result-ready` state and a maximum-observed Unix-millisecond high-water mark. A private `AuthorizedHostDispatch` retains the complete admitted operation and therefore its generation and authority binding.                                                                                                                                                     |
| Outputs      | Admission returns `Dispatch`, `ResultReady`, or `SlotOccupied`; handoff returns `HandedOff` or `ResultReady`; `take_result` returns one typed OBCN Launch or Start result, including `CapacityExceeded` for a valid unseen terminal after the retained-binding limit is reached; invalid completion observations return a typed internal error without changing the slot. The kernel opens no carrier, performs no Host call, emits no product event, and writes no durable state.                                                                                                                                                                                                                                  |
| Invariants   | Wrong Worker, stale generation, unsupported capability, expired new lease, lower fence, conflicting same-fence binding, and a valid unseen terminal beyond capacity never dispatch. An exact binding dispatches at most once and replays only its cached exact result, even after expiry or at capacity. Once the kernel observes a lease as expired, a lower later time cannot revive it. A valid higher fence replaces the prior binding and clears its cache even at capacity. Capacity rejection never inserts a tombstone or changes any binding. At most one operation or undelivered result occupies the global slot. Only `handed-off` can become a Host outcome.                                           |
| Decisions    | Classification order is occupied slot, Worker identity, Worker generation, exact cached replay, capability, lease expiry, per-terminal fence, unseen-terminal capacity, then Start target epoch. Consequently unsupported or expired new requests retain their capability or lease classification at capacity; existing lower and same-fence requests retain their fence classifications; and only a valid unseen terminal receives `CapacityExceeded`. At Start admission and again at handoff, a target epoch mismatch becomes `HostIdentityChanged`; capacity precedes that check for an unseen terminal. Handoff expiry prevents an effect, and accepted Start identity, epoch, and geometry must remain exact. |
| Model        | One deterministic state owner combines a generation-local, 1,024-entry per-terminal monotonic binding map with one globally bounded operation/result slot. `admit_launch` and `admit_start` are the principal entry points; `begin_host_handoff` is the named external-effect boundary; typed observation methods close the accepted binding and queue its result. Capacity tests cover cross-terminal limits and an unseen identity while the global slot is empty, operation-pending or result-ready.                                                                                                                                                                                                             |
| Transitions  | The transition table below defines admission, handoff, completion, exact replay, result delivery, conflict, and stale-token behavior. Rejection before a binding is accepted queues an uncached result; every terminal outcome after a binding is accepted is cached before it is exposed.                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Dependencies | The Rust standard library, canonical local-Session Worker identity, and OBCN semantic types and constructors only. Time, socket liveness, decoded framing, current Host epoch, and Host observations are supplied as values by the operation adapters; the kernel monotonizes supplied time but never reads a clock. It does not read an environment, filesystem, socket, process, profile, executable, shell, or working directory, and it does not parse or encode Host framing.                                                                                                                                                                                                                                  |
| Ownership    | Server continues to own placement, authorization issuance, and reconciliation. OBCN owns the carrier-independent request/result types and wire codec. Worker owns only local validation, monotonic replay memory, the single dispatch gate, and classification around the Host handoff boundary. Terminal Host remains the sole owner of terminal and process effects.                                                                                                                                                                                                                                                                                                                                              |
| Verification | Table-driven unit tests exercise both operation kinds across wrong identity, generation, legacy and future capability mismatch, expiry and clock rollback, overlapping rejection priorities, the 1,023/1,024/1,025 capacity boundary, no-tombstone rejection, exact replay and higher-fence replacement at capacity, lower and conflicting fences, exact replay of every accepted/rejected/failed/unknown outcome, expiry after admission, stale and wrong completions, slot backpressure, initial and raced Start identity changes, mismatched Start completion geometry, both Launch modes, and both Start success transitions. OBCN tests own value/codec validity.                                              |
| Correctness  | Authorization and capacity tests check exact binding/fence admission, a single global slot, replay, rejection preservation and cached outcome/epoch/transition. Process tests connect those decisions to the Host. The kernel has no separate TLA workflow proof.                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Limits       | The running private Worker request loop invokes this kernel and its Host adapter. Bindings and cached results are process-local, are not crash-persistent, and remain until the kernel is dropped or a higher fence replaces them. The 1,024 limit has no release, TTL, LRU, or eviction path; an explicit retirement contract remains required before production. The kernel cannot detect a Server's cross-terminal `OperationId` reuse without an unbounded ledger.                                                                                                                                                                                                                                              |

| Current slot       | Input or observation                               | Guard                                                                          | Next slot          | Binding/result effect                                                                                      |
| ------------------ | -------------------------------------------------- | ------------------------------------------------------------------------------ | ------------------ | ---------------------------------------------------------------------------------------------------------- |
| `empty`            | New or higher-fence Launch/Start                   | Worker, generation, capability, lease, and fence are current                   | `awaiting-handoff` | Store the complete binding with an empty cache and return its private dispatch.                            |
| `empty`            | Invalid new, stale, or conflicting request         | A named rejection applies                                                      | `result-ready`     | Queue the exact-echo typed rejection without accepting or replacing a binding.                             |
| `empty`            | Valid unseen Launch/Start at 1,024 bindings        | Worker, generation, capability, and lease are current                          | `result-ready`     | Queue exact-echo `CapacityExceeded`; do not insert a tombstone or change any binding.                      |
| `empty`            | Exact accepted binding                             | Its exact terminal result is cached                                            | `result-ready`     | Queue the cached result semantically unchanged; do not dispatch or re-evaluate expiry.                     |
| `awaiting-handoff` | `begin_host_handoff`                               | Lease remains live and any Start target epoch equals the observed one          | `handed-off`       | Preserve the complete dispatch and observed Host epoch; only now may the adapter perform its first effect. |
| `awaiting-handoff` | Expiry, Start epoch replacement, or definite loss  | No Host effect has begun                                                       | `result-ready`     | Cache and queue lease/identity rejection or `HostUnavailableBeforeHandoff`.                                |
| `handed-off`       | Accepted, rejected, failed, or unknown Host result | Dispatch and Host observation are consistent; accepted Start geometry is exact | `result-ready`     | Cache and queue the exact typed result; Launch state derives from its admitted mode.                       |
| `result-ready`     | `take_result`                                      | One result is present                                                          | `empty`            | Deliver the queued result once while retaining any accepted binding cache for replay.                      |
| Any occupied slot  | Another admission                                  | An operation or undelivered result already owns the slot                       | Unchanged          | Return `SlotOccupied`; do not compare or mutate terminal authority.                                        |
| Any phase          | Wrong, stale, or out-of-phase dispatch observation | Dispatch or phase does not match                                               | Unchanged          | Return a typed internal error and preserve the current operation/result.                                   |

| Decision partition                       | Launch evidence                                                                     | Start evidence                                                             | Required result                                                                                       |
| ---------------------------------------- | ----------------------------------------------------------------------------------- | -------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| Worker, generation, capability, or lease | Table case for every rejection                                                      | Same table cases                                                           | Exact request echo, no dispatch.                                                                      |
| Overlapping invalid conditions           | Lower plus unsupported, lower plus expired, same-fence changed capability or expiry | Shared classification path                                                 | Capability precedes expiry, expiry precedes fence, and only an exact cached binding bypasses expiry.  |
| Per-terminal fence                       | Lower, same-operation, lease, and payload conflicts; higher replacement             | Lower, cross-kind, target, and geometry conflicts; higher replacement      | Stale or conflicting rejection; valid higher fence clears the old cache.                              |
| Retained-binding capacity                | 1,023 admits, 1,024 is full, 1,025th identity rejects; replay/rebind while full     | Same boundary plus identity-mismatched unseen Start                        | Only an unseen identity receives `CapacityExceeded`; no tombstone, binding mutation, or eviction.     |
| Exact replay after expiry                | Accepted in both launch modes, rejected, failed, and unknown                        | `Started`, `AlreadyStarted`, rejected, failed, and unknown                 | The original complete cached result; no new Host effect.                                              |
| Handoff boundary                         | Expiry, definite failure, and Host epoch-bound completion                           | Admission mismatch, expiry, target epoch replacement, and definite failure | No accepted Host observation before handoff; post-handoff loss is unknown.                            |
| Completion identity and ordering         | Wrong token, stale token, wrong phase, and wrong epoch                              | Wrong operation kind, target identity, epoch, or accepted geometry         | Typed error with unchanged state; a later valid completion or unknown observation can still close it. |
| Global result slot                       | Awaiting, handed-off, and result-ready backpressure                                 | Shared slot behavior                                                       | Exactly one outstanding operation or undelivered result across both kinds.                            |
| Time rollback                            | Expired unaccepted request and admission-to-handoff expiry                          | Expired unaccepted request                                                 | Maximum-observed time prevents an expired lease from becoming dispatchable again.                     |

## Worker-local Terminal Host launch adapter

The application-private `terminal_host_launch_adapter` module is consumed by the running single-operation
Host exchange. It is a pure value mapper between one admitted `AuthorizedHostDispatch`,
existing Terminal Protocol request/response values, and the Host observations already accepted by
`terminal_authorization`; it neither opens a Host socket nor changes kernel state.

### Launch-adapter modeling record

| Field        | Current contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | An admitted OBCN Launch or Start must cross the existing Terminal Protocol without giving the carrier authority to invent a process, directory, environment, identity, geometry, or outcome.                                                                                                                                                                                                                                                                                                                                |
| Inputs       | One private admitted dispatch; for Launch only, an injected `ResolvedTerminalLaunchFacts` containing a stable Host `CreationId`, previously resolved executable, literal arguments, durable launch environment, omitted-cwd default, terminal profile, and optional terminal configuration; or one decoded Host success/`TypedFailure` plus the response envelope's exact Host epoch.                                                                                                                                       |
| Data         | The mapper retains no state. `ResolvedTerminalLaunchFacts::new` is the narrow composition seam for the node-local default-interactive-launch resolver and separately supplied creation-identity/cwd/profile/configuration facts; operating-system, account, environment, and filesystem lookup remain outside this module.                                                                                                                                                                                                  |
| Outputs      | Launch becomes `CreateTerminal`; Start becomes `StartTerminal`; Host success or `TypedFailure` becomes the existing `LaunchHostCompletion` or `StartHostCompletion`. The caller remains responsible for framing, I/O, correlation, handoff, and applying an observation to the kernel.                                                                                                                                                                                                                                      |
| Invariants   | Launch copies the independently resolved stable Host `CreationId`; it is not derived from product `TerminalId`. Start sends the OBCN target's Host `TerminalId` and preserves its full identity without imposing a product/Host byte-equality policy. Explicit cwd is copied exactly; omission selects the injected Worker-user-home default. Immediate Launch and Start preserve all four geometry dimensions; deferred Launch carries no geometry. Caller overlay is always empty. Other launch facts are copied exactly. |
| Decisions    | The mapper reuses the Terminal Protocol encoders as pure preflight validation, so public protocol values cannot bypass its identity, text, count, palette, geometry, or control-payload bounds. Terminal Protocol `TypedFailure.retryable` controls retry guidance, not failure kind; every generic `TypedFailure` therefore becomes `HostReportedFailure`. `HostRejected` requires a future explicit stable rejection contract. Response kind, identity, epoch, and geometry contradictions remain typed adapter errors.   |
| Model        | `Launch(dispatch, facts) -> CreateTerminal`; `Start(dispatch) -> StartTerminal`; `(Launch dispatch, expected CreationId, response epoch, exact response) -> Launch completion`; `(Start dispatch, response epoch, exact response) -> Start completion`. The operation exchange owns the transport handoff distinction. Each function is deterministic and side-effect free.                                                                                                                                                 |
| Dependencies | Application-private OBCN authorization types and the existing Rust Terminal Protocol semantic values/encoders only. The module has no socket, envelope codec, carrier, clock, process, filesystem, account-database, environment-reader, shell resolver, or kernel-orchestration dependency.                                                                                                                                                                                                                                |
| Ownership    | The default-interactive-launch resolver owns node-local executable and durable default environment selection. The composition root owns stable Host creation identity plus omitted-cwd, profile, and optional configuration facts. This adapter owns only deterministic translation and cross-contract consistency checks. Terminal Host still owns PTY/process effects; the carrier owns exact envelope/correlation transport; the authorization kernel owns replay and outcome state.                                     |
| Verification | Unit tests cover deliberately different product `TerminalId` and Host `CreationId` values, immediate and deferred Launch, explicit and omitted cwd, stable Host creation identity, empty overlay and accepted default environment, Start, both transitions, both `retryable` values mapping to reported failure for each operation, wrong response kinds, identity/epoch/geometry contradictions, and exact argument/environment plus malformed-value bounds.                                                               |
| Limits       | The mapper performs no default resolution itself and does not own Host framing, socket I/O, activation, retry, reconciliation, or a Terminal Protocol change.                                                                                                                                                                                                                                                                                                                                                               |

| Authorized input | Terminal Protocol request | Successful Host response                           | Kernel completion                                                                 |
| ---------------- | ------------------------- | -------------------------------------------------- | --------------------------------------------------------------------------------- |
| Immediate Launch | `CreateTerminal`          | `TerminalIdentity`                                 | Accepted identity; kernel derives `Started` from the authorized complete geometry |
| Deferred Launch  | `CreateTerminal`          | `TerminalIdentity`                                 | Accepted identity; kernel derives `AwaitingStart`                                 |
| Start            | `StartTerminal`           | `TerminalStartAccepted` with exact target/geometry | Accepted identity, geometry, and `started` or `already-started` transition        |

## Local Host broker modeling record

The checked model governs the complete broker lifecycle. The single-operation exchange and
integrated request loop below refine its Host effect and bounded result-delivery boundaries.
The executable now invokes both after its two handshakes; restart recovery remains unimplemented.

The accepted `HostBrokerConnection` epoch is immutable. Host loss before handoff fails closed; replacing Host requires teardown and a fresh Worker/Host handshake. Deterministic tests check epoch preservation and handshake-only installation, including the kernel’s defensive `HostIdentityChanged` result. There is no in-place hot replacement.

| Field        | Modeled broker contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | After Server-first dual admission, the Worker carrier must turn one dispatch already admitted by the authorization kernel into at most one Terminal Host effect and return its exact OBCN result. It must preserve kernel `ResultReady` decisions made during `begin_host_handoff`, validate an actual Host response rather than a diagnostic label, and never invent a Host-rejection classification that the wire does not carry.                                                                                                                                                                                                                                                                                                                                                             |
| Inputs       | Accepted Server and Host links and their immutable handshake Host epoch; one OBCN Launch or Start `Dispatch` already admitted through the kernel's identity, generation, intrinsic capability, lease, fence, 1,024-binding capacity, and global-slot gates; the Start target Host epoch; the retained connection epoch supplied to `begin_host_handoff`; one nonzero command correlation; the actual Host response role, kind, correlation, and epoch; timeout or link loss; and one bounded OBCN result-delivery opportunity. A retained Host terminal may also be observed after Worker loss.                                                                                                                                                                                                 |
| Data         | One process-local broker phase; one operation phase projected onto the kernel's single global `empty`/`awaiting-handoff`/`handed-off`/`result-ready` slot; the Launch/Start kind; handoff-attempt and effect-boundary counts; Start target and retained handoff Host epochs; mapped Host command and correlation; retained response role, kind, correlation, and epoch; whether Host applied the command; the exact completion source and cached-result fact; one bounded delivery count; an unknown retry barrier; and retained-terminal disposition. No adapter queue or durable state is introduced.                                                                                                                                                                                         |
| Outputs      | Launch maps only to Host `CreateTerminal`; Start maps only to Host `StartTerminal`. Lease expiry during `begin_host_handoff` returns cached OBCN `Rejected(LeaseExpired)` without crossing the effect boundary. Definite transport failure or Host loss before the call remains cached `Failed(HostUnavailableBeforeHandoff)`. Exact success becomes `HostAccepted`; an exact generic Terminal Protocol `TypedFailure` becomes `Failed(HostReportedFailure)`. Unknown and retained-state outcomes follow the prior bounded lifecycle.                                                                                                                                                                                                                                                           |
| Invariants   | Server admission precedes Host contact. Readiness requires both accepted links and a nonzero Host epoch. The active connection's Host epoch cannot change. At most one kernel-global operation/result slot and one Host command exist. A successful `begin_host_handoff` precedes the first possible Host effect; its modeled `ResultReady` branch retains an attempted call but zero crossed boundaries, commands, or effects. A known Host completion requires actual response role `Response`, the request-specific expected kind, exact command correlation, and exact retained handoff Host epoch. An exact generic `TypedFailure` can produce only `HostReportedFailure`; a mismatched one fails closed as cached unknown, and neither can produce `HostRejected`. Unknown never retries. |
| Decisions    | Kernel results before dispatch, including `CapacityExceeded`, `SlotOccupied`, and a defensive Start `HostIdentityChanged`, remain distinct and never enter this adapter model. During the modeled handoff, lease expiry is an exact cached rejection rather than a transport failure. Host loss fails closed, and a different Host epoch can be installed only by a fresh connection lifecycle. An authoritative Host-rejection discriminator does not exist in the current Terminal Protocol, so identical exact generic `TypedFailure` input maps conservatively to OBCN `Failed(HostReportedFailure)`. OBCN `HostRejected` is reserved for a future explicitly classified owner contract. Post-handoff loss, timeout, or invalid response is `OutcomeUnknown`.                               |
| Model        | One concrete adapter preserves Server-first admission and the accepted Host epoch, consumes an authorized dispatch, and maps awaiting-handoff and handed-off phases onto the single kernel operation slot. The [Host contract](../terminal-host/docs/runtime-model.md) owns the effect/unknown boundary.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| Transitions  | The table below covers admission, the lease-expiry `begin_host_handoff -> ResultReady` branch, definite pre-call transport failure, successful handoff, command emission and possible apply, concrete response validation, conservative generic failure, independently ordered Server/Host loss, bounded exact-result delivery, the unknown retry barrier, and retained-terminal recovery.                                                                                                                                                                                                                                                                                                                                                                                                      |
| Dependencies | Authorization kernel and generation-local capacity, OBCN values/codec, local-Session carrier, Terminal Protocol bindings, Host outcomes, authenticated sockets and bounded timers. Framing, deadlines, queue storage and PTY behavior remain with their concrete owners.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| Ownership    | Server owns placement, issuance, generation and reconciliation. Worker kernel owns capacity, admission/replay, cached results and the single operation/result slot. Adapter consumes only `Dispatch` and adds no parallel slot. OBCN owns result variants, Terminal Protocol owns correlation/response identity, and Host owns commit/effect/reply order.                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| Verification | Kernel and socket tests cover admission, lease expiry, pre-call failure, handoff, exact/mismatched response, generic failure, ordered link loss, delivery, no-retry and immutable epoch. Real Worker/Host process witnesses below connect these traces to integration.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Correctness  | Keep slot ownership exact, distinguish attempted handoff from possible effect, preserve cached results through delivery or abandonment, reject mismatched fields, classify exact generic failure as `HostReportedFailure` and mismatches as unknown, and never replay unknown effects. These obligations are checked through implementation traces.                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| Limits       | Controlled and generated traces are finite implementation evidence. They do not prove all interleavings, operating-system scheduling, arbitrary byte streams or unbounded capacity. Binding capacity and Host behavior require their own tests.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |

| Current state                   | Observation or action                                                   | Next state               | Exact result and lifecycle rule                                                                                                 |
| ------------------------------- | ----------------------------------------------------------------------- | ------------------------ | ------------------------------------------------------------------------------------------------------------------------------- |
| Before Server admission         | Host connect attempt                                                    | Forbidden                | Host contact has no enabled transition.                                                                                         |
| Ready, kernel slot empty        | Consume one kernel `Dispatch`                                           | `admitted`               | Project the global slot to `awaiting-handoff`; capacity, replay, rejection, and occupied-slot results do not dispatch.          |
| `admitted`                      | `begin_host_handoff` observes expired lease                             | `result-ready`           | Cache and queue exact `Rejected(LeaseExpired)`; record one attempted call and no crossed effect boundary or Host command.       |
| `admitted`                      | Definite transport failure or Host loss before the handoff call         | `result-ready`           | Cache and queue `Failed(HostUnavailableBeforeHandoff)` distinctly from the lease rejection.                                     |
| `admitted`                      | Successful `begin_host_handoff` with the retained immutable Host epoch  | `handed-off`             | Cross the named external-effect boundary exactly once and project the kernel slot to `handed-off`.                              |
| `handed-off`                    | Emit mapped command with one operation correlation                      | `awaiting-host`          | Send at most one Host command; Host may apply it before a response is observable.                                               |
| `awaiting-host`                 | `Response` + expected success kind + exact correlation + handoff epoch  | `result-ready`           | Queue cached OBCN `HostAccepted`.                                                                                               |
| `awaiting-host`                 | `Response` + generic `TypedFailure` + exact correlation + handoff epoch | `result-ready`           | Queue cached `Failed(HostReportedFailure)`; never derive `HostRejected` from the same exact record.                             |
| `awaiting-host`                 | Wrong role, kind, correlation, or Host epoch                            | `draining-result`        | Retain the concrete mismatched observation, fail closed as cached outcome unknown, close Host, and block retry.                 |
| `handed-off` or `awaiting-host` | Host loss or timeout                                                    | `draining-result`        | Queue cached outcome unknown, close Host, block retry, and retain Server only for bounded result delivery.                      |
| Any live operation/result       | Server loss                                                             | `stopped`                | Close both links immediately; an unobserved in-flight dispatch is unknown and any undelivered exact result is abandoned intact. |
| `result-ready`                  | Result write succeeds or its bounded opportunity expires                | `completed` or `stopped` | Empty the global slot; deliver the cached result exactly once, or abandon it without changing its classification.               |
| `stopped`, retained terminal    | Recovery classification                                                 | `recovery-required`      | Preserve the unmatched disposition for explicit reconciliation; no Worker-adoption transition exists.                           |

For runtime refinement, keep capacity, `SlotOccupied`, pre-admission rejection, cached replay, and
the one global slot in `AuthorizedHostOperationKernel`; consume only its post-capacity `Dispatch`
and never introduce an adapter-local operation queue. Call
`begin_host_handoff` once and inspect its return before any Host send: forward its exact
`ResultReady` lease rejection unchanged, or retain the returned handoff epoch for one command. Allocate
one nonzero correlation for that dispatch. A known completion requires actual `Response` role, the
request-specific success kind or generic `TypedFailure`, exact correlation, and exact handoff Host
epoch. Map generic `TypedFailure` only to `HostReportedFailure` until an authoritative classified
Host-rejection contract exists. Treat any post-boundary uncertainty as outcome unknown, cache it in
the kernel, and never resend. Keep delivery single-slot and bounded, and quarantine inventory after
Worker death for explicit recovery rather than startup adoption.

### Single-operation Host exchange refinement

The accepted local-terminal milestone delegates the `terminal_host_operation` adapter to the
Worker owner. Its consumer is the running Worker OBCN request loop: after kernel admission,
that loop supplies resolved launch facts and a fresh nonzero Host correlation, executes one
prepared command, then delivers the kernel's exact result before admitting another request.
Server request consumption, default cwd/profile/creation-identity composition, and result delivery
belong to that loop, not to this adapter.

`PreparedHostOperation` binds one kernel dispatch to a validated Create or Start payload and its
correlation. Preparation is effect-free and cannot change the kernel. `execute` uses the admitted
Host connection, the already admitted Server stream, an injected lease clock, and a caller-supplied
positive I/O budget. A single monotonic deadline bounds the complete Host write and response read,
including fragmented records; it does not choose a release timeout. Kernel state is the only
operation/result slot. No queue, result cache, retry, identity allocator, shell resolver, terminal
inventory, or VT state is added. Valid lifecycle and metadata events are drained as described below,
including before handoff and between a command write and its exact response. They cannot complete
an operation and consume the same Host deadline.

| Boundary         | Runtime refinement and evidence obligation                                                                                                                                                                                                                                                                                                  |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Before handoff   | Check both peers without consuming Server bytes. Observed Server loss or unexpected Server data closes both links and abandons the pending operation. Host loss or invalid unsolicited Host data closes Host and records the kernel's exact pre-handoff failure (or expired-lease rejection). Preparation and invalid budgets send nothing. |
| Handoff          | Call `begin_host_handoff` once with the retained immutable handshake epoch and freshly sampled lease clock. Its `ResultReady` branch sends nothing and preserves the exact cached lease rejection. Only `HandedOff` permits the first write.                                                                                                |
| Response         | Require exact protocol, mandatory control encoding, Response role, command correlation, and handoff epoch before decoding the operation-specific success or generic `TypedFailure`. The existing mapper validates payload identity and geometry. Generic failure maps only to `HostReportedFailure`.                                        |
| Uncertainty      | Any write/read/timeout/invalid-response failure after handoff records the kernel's exact unknown result and shuts down Host. Observed Server loss closes both links; no command is resent.                                                                                                                                                  |
| Result ownership | Success and every classified failure remain in the kernel slot. The caller must deliver or abandon that result and honor `HostClosed`/`ServerClosed`; this adapter never drains the slot or claims OBCN delivery.                                                                                                                           |

The module depends only on `host_connection`, `terminal_authorization`,
`terminal_host_launch_adapter`, and their protocol values. Its private `transport` child owns
nonblocking byte I/O and polls the Server alongside every Host wait; it imports no product policy.
These directions are one-way. The running request loop invokes this module.

Deterministic socket witnesses cover handoff, exact completion, mismatch, ordered loss and unknown/no-retry behavior. A real built Zig Host trace covers deferred Create/Start, stable identity and retained inventory after broker loss. Delivery belongs to the request loop and recovery has separate evidence. Mapping uses the existing checked mapper; finite protocol partitions and controlled socket timelines cover this adapter. Native process evidence applies only to the platform actually run, not the full Client path.

| Claim                                     | Runtime witness (`terminal_host_operation::tests`)                                                                                                                                                    |
| ----------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Handoff rejection sends nothing           | `handoff_rejections_and_invalid_preparation_never_send_a_command`                                                                                                                                     |
| Exact completion and one result slot      | `exact_launch_success_remains_in_the_kernel_slot_and_replays_without_an_effect`, `start_responses_preserve_transitions_and_reject_contradictory_payloads`                                             |
| Generic Host failure stays failed         | `exact_generic_failure_is_failed_even_when_its_diagnostics_say_rejected_and_retryable`                                                                                                                |
| Invalid response is unknown without retry | `malformed_response_partitions_close_host_cache_unknown_and_never_resend`                                                                                                                             |
| Ordered loss closes the right links       | `observed_host_loss_before_handoff_is_a_definite_failure`, `server_loss_before_handoff_abandons_without_sending_or_inventing_a_result`, `server_loss_interrupts_a_waiting_host_and_closes_both_links` |
| Deadline and retained execution           | `a_silent_host_is_bounded_by_the_operation_deadline`, `real_zig_host_creates_starts_and_retains_the_same_terminal_after_broker_loss`                                                                  |

## Integrated authorized request loop

`terminal_operations` composes the accepted local-terminal milestone inside the private Worker.
Its inputs are the admitted Server stream, admitted Host broker, local Worker identity, and OBLW
generation. It owns their serialized operation lifetime; `terminal_authorization` remains the only
launch binding/cache/operation authority. Viewer/control requests use the Host broker
attachment state and their respective operation adapters. Its private `server_transport` child adapts canonical OBCN
`Read`/`Write` codecs to nonblocking sockets and peer observation. Its private `launch_facts` child
resolves native launch inputs. Neither child gains Server placement authority or Host process state.

The refinement is `idle -> bounded request read -> kernel admission -> optional preparation and
Host exchange -> bounded exact result delivery -> idle`. A cached replay or rejection skips
preparation and Host I/O. No next request is admitted until result delivery completes. Malformed,
wrong-direction, oversized, partial-timeout, and observed pipelined requests close the attachment;
the codec bounds OBCN payload allocation to 8 KiB. Idle waits have no timeout; once the first record
is observable, all its fragments share a two-second monotonic budget. Result writes also share one
two-second budget. Both waits observe Host loss. Host loss during result delivery preserves that
bounded delivery attempt, then ends the Worker; Server loss abandons delivery and closes Host.

Each actual Host exchange has a one-second total write/read budget, leaving time for result delivery
inside the Server carrier's twenty-second result wait. This is an experimental local bound,
not a measured release performance claim. Local account/filesystem resolution is synchronous;
those OS calls are not covered by the socket deadline. A slow or failed resolution may therefore
make Server's outcome unknown. A preparation failure closes the attachment before any Host command,
without inventing a Host-reported failure. A missing or unrepresentable wall clock expires leases;
the kernel still prevents time rollback from reviving previously expired authority.

For an admitted Launch, explicit absolute cwd is preserved literally and requires no account-home
lookup. Omission reads the effective user's account home, ignoring inherited `HOME` and Worker
startup cwd. The existing shell resolver supplies executable, empty arguments, and the three
fixed launch environment entries. The production preference snapshot supplies configuration.
The baseline Terminal Protocol profile, before the confirmed color defaults above, uses a light scheme,
`0xfcfcfc` background, `0x141414` foreground, nonblinking block cursor with `0x57bef8` cursor color,
the 16 ANSI colors followed by the 216-color cube and 24 grays, conformance level 62 with feature 22,
and `Orchbee` version response. This profile consumes the shared runtime resolver without importing
Desktop source or adding an appearance preference.

Host also sends unsolicited `TerminalStarted`, `TerminalExited`, `TerminalFailed`, and
`TerminalMetadataChanged` events. The private `host_connection::events` adapter validates their
Event role, absent correlation, exact protocol/epoch, payload codec, and matching identity epoch.
It drains these notifications without a lifecycle/metadata projection. The broker routes viewer
frames and blocked events through their independent slots, including events interleaved with a
Launch/Start response. Events never acknowledge an operation or adopt retained terminals.
Response-like, unknown, malformed, or wrong-epoch events fail closed. An event has a one-second
read budget capped by an active OBCN deadline. Allocation is bounded by the OBTP header plus the
36 MiB frame limit; the envelope still enforces the 64 KiB control limit. Valid discarded lifecycle
events remain state-preserving steps in the symbolic operation model.

Creation and command correlation identities are independent 128-bit reads from the POSIX OS
random source. A zero value or read failure fails preparation closed. Product Terminal identity is
never used as a Host creation identity. Resolved facts live only in the one prepared operation;
replay uses the kernel's exact cached result and never resolves another launch or creates another
Host terminal. Start resolves neither shell nor cwd and preserves the exact authorized Host target
and complete geometry. Worker does not adopt retained Host inventory after reconnect.

Process witnesses in `tests/process/terminal_operations.rs` cover real Worker and Zig Host deferred
Create, Start, immediate Create, exact replay, and both retained terminals after Server/Worker loss;
a fake Host checks omitted/explicit cwd, launch environment, profile, and independent identities.
Further partitions cover authorization rejection with no command, wrong-direction/oversized/partial/
pipelined OBCN, a silent Host yielding one exact unknown result followed by disconnection, and exact result
delivery after Host closes immediately after its response.
These are concrete admission, delivery, loss and no-retry witnesses on the actual carrier. They do not
prove Server Client Session viewer routing, activation, restart reconciliation, or another OS platform.
The viewer process trace separately proves two read attachments, a stalled viewer, exact feedback,
snapshot recovery, detach/reattach, and terminal retention through the real Worker and Host.

## Agent drain queue and request loop

One `AgentDrain` queue (`src/agent_drain.rs`) is owned by each admitted Worker-to-Server
connection and created before any Host startup. A shared `Rc<RefCell<AgentDrain>>` handle is a
required `HostBrokerConnection` constructor argument on both connection paths — the eager path
builds it between Server admission and Host connect, and the lazy path builds it at the top of the
pre-Host loop and passes the same handle through `start_host` into the admitted loop. There is no
optional owner, no empty fallback, and no per-viewer or per-request allocation; the queue's
lifetime is exactly the admitted Server connection epoch. `Rc<RefCell>` is sound because the
Worker event loop is single-threaded, and the borrow is always released before Server I/O:
`server_transport::send` may consume Host events during POLLOUT waits, and those events append to
the same queue.

`HostBrokerConnection::receive_event` is the single producer. After envelope and Host-epoch
validation it decodes every kind-58 application signal, converts the Terminal Host identity
through the existing launch adapter, and appends to the shared drain before borrowing viewers —
a terminal with no attached viewer still enters the queue. Signals for a foreign Host epoch stay
fatal, and an `ApplicationSignal` reaching the viewer path is now unexpected rather than silently
dropped. Bell and Clipboard routing is unchanged.

The queue retains at most 1,024 entries and 4 MiB of encoded bytes. Overflow evicts the oldest
sequence, covering it with a merged leading `Gap` that consumes its own slot and bytes. Replies
page by both an entry count of 480 and 8,165 encoded entry bytes; a successful send commits the
watermark captured at request time, for both `Ok(false)` and `Ok(true)` write results. An
acknowledgement above the offered watermark is fatal, a stale or zero acknowledgement is a
no-op that leaves unconfirmed sequences offered, and the echoed token is exactly the request's.

The pre-Host loop answers drain requests with the same shared dispatch as the admitted loop, so
a request before any Host returns a truthful empty page without starting a Host, and the same
owner, token, and acknowledgement state survives the first launch.

## Terminal cwd validation

The integrated request loop handles `TerminalCwdValidationRequest` before terminal authorization or
viewer/controller cleanup. The lazy Linux loop handles it before resolving launch facts or starting
a Terminal Host. Worker first compares the request's exact logical Worker ID and generation with its
admitted OBLW identity. A mismatch returns `WorkerUnavailable` without observing the path. A match
uses one synchronous `std::fs::metadata` lookup, follows symbolic links, preserves the supplied path
spelling, and returns `DirectoryAvailable` only for an existing directory. Missing paths, regular
files and every metadata error return `DirectoryUnavailable`; arbitrary OS error details do not
cross OBCN.

Validation is a point-in-time read. It creates no directory, terminal, Host, attachment, controller,
lease, launch binding or replay cache entry and sends no Terminal Protocol record. Its response uses
the existing bounded OBCN read/write path. The Server still owns association admission, current
Worker revalidation and canonical state. Filesystem metadata calls are synchronous and are not made
interruptible by carrier deadlines; the current runtime claim is limited to the supported local
Linux filesystem cohort. Unit tests exercise a real directory, spelling-preserving symlink lookup,
regular and missing paths, and Worker identity/generation rejection without a Host dependency.

## Ownership

The [`orchbee-local-worker-protocol`](../../crates/local-worker-protocol/README.md) crate owns the
Server-Worker framing. The
[`orchbee-local-peer-credentials`](../../crates/local-peer-credentials/README.md) crate owns the
kernel same-user proof used for both local connections. The
[`orchbee-terminal-protocol`](../../schemas/terminal/references/protocol.md) package owns
the Host wire; its independent Rust binding supplies the consumed control, envelope, and routing codecs.
The capability-node protocol crate owns OBCN semantic values and framing without importing either
local-Session or Terminal Host types. Worker owns admission order, the Host broker connection, coupled
liveness, integrated deterministic validation/replay/dispatch, bounded default interactive launch
resolution, Host operation mapping, transient viewer relay state, and grant-scoped control effects. Server remains the only
placement, authorization-issuance, and reconciliation owner. Terminal Host remains the sole PTY,
process-tree, VT, history, and attachment authority.

The Server owns admission and Worker-generation authority; the Worker preserves Server-first admission before Host handoff. Capability version 2 grants authority and version 1 is an unsupported-request witness. Deterministic authorization and capacity tests cover handoff, cached results, rejection priority, the generation-local binding bound and the single global operation/result slot. The startup and send-transaction cores own their separately declared protocol claims; they do not prove the whole authorization workflow or the production capacity.

## Close lifecycle-step model

The accepted [Close contract](../orchbee-server/docs/terminal-tab-close.md) is checked through the Worker implementation and deterministic/process witnesses mapped below. These cover authorized lifecycle requests, inventory pages and exact target-cache settlement. Server orchestration and reservation ownership remain separate; there is no separate Worker workflow TLA proof.

The `identity` configuration explores Terminate, Force, Dispose and one inventory page, one current
product/Worker/generation/Host epoch/creation/terminal tuple and a distinct mismatch for each field.
The `target-cleanup` configuration fixes Dispose and explores empty, target, unrelated and full
(two-slot) viewer caches, target/unrelated controller reservations, distinct old/new controller IDs,
queued target frame/feedback, old leases and an already admitted Acquire. These finite bounds do
not establish the actual eight-slot bound. Initial state assumes one authenticated admitted broker
with immutable epoch/generation; no replacement or adoption transition exists. Configurations use
no symmetry, state constraint or fairness assumption and claim no eventual cleanup success.

| Owned claim                                               | Model actions and properties                                                                                                                                                                                                     | Required implementation refinement                                                                                                                                                                                                     |
| --------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Exact fresh authority and one Host request per step/page  | `RejectAuthorization`, `RejectFreshHandoff`, `SubmitHostRequest`; `HandoffRequiresFreshExactAuthorization`, `OneHostRequestPerStep`                                                                                              | Reject every wrong product/Worker/Host field, grant and stale handoff before effects; count actual Host requests for every step kind.                                                                                                  |
| Known acknowledgment and unknown delivery remain distinct | `ReceiveExactAck`, `ReceiveTypedFailure`, `ReceiveInvalidAck`, timeout/loss actions; `KnownAckRequiresFullCorrelation`, `KnownDisposeNeverRegresses`, `UnknownNeverDispatchesAnotherLifecycleRequest`                            | Check actual success ACK terminal ID plus envelope role/kind/epoch/correlation; typed failure is conservative and needs no invented terminal echo. Check full retained identity and operation/attempt/step separately on OBCN results. |
| Dispose success waits for exact Worker and Server cleanup | `CommitWorkerTargetCleanup`, `ObserveExactWorkerResult`, `CommitServerTargetCleanup`; `WorkerDisposedResultRequiresTargetCleanup`, `TargetFrameAndFeedbackFreed`, `ServerCompletionRequiresExactReservationRevocation`           | Prepare bounded exact cleanup before Dispose; retain known ACK before nonallocating target removal; exercise all eight slots, queued frames/feedback, unrelated survival, lost Worker result and Server cleanup failure.               |
| Closing fences fresh grants and old cleanup remains exact | `RejectAcquireAfterClose`, `DropOldLeaseExactMatch`, `DropOldLeaseMismatchedReservation`, `DropOldViewer`; `LateAcquireCannotRestoreClosingTarget`, `OldLeaseCannotDeleteDifferentReservation`, `OldLeaseAfterRevocationIsLocal` | Revalidate Acquire after its acknowledgment; old lease Drop after revocation makes no stale Host call and cannot remove a newer controller ID. Unknown Close still permits legitimate exact lease cleanup.                             |
| Attempt expiry preserves admitted receipts                | `ExpireAttempt`, `ReceiveLateExactAck`; `ExpiryCannotDiscardOutstandingOrKnownReceipt`                                                                                                                                           | Preserve an outstanding result and exact late ACK beyond the 30s attempt admission deadline; settle before Resume or next-step admission.                                                                                              |

`prepared` includes the admitted carrier step and cleanup seal; `sent` marks the first possible Host effect. Result-ready and completed/abandoned boundaries retain the exact receipt without evicting launch bindings. A generic typed failure validates its correlated envelope and is known failure even if an effect preceded it. An inventory-page success is an opaque exact page observation, never absence or disposal permission. Host containment and whole-walk validation remain with Host and Server; lifecycle implementation tests check these decisions.

Independent expiry actions abstract the existing **1s Worker HostExchange**, **2s Server frame
I/O**, **20s Server response wait** and **30s Close attempt admission** boundaries. They do not
simulate numeric timing or guarantee return/join time. Exact Worker Dispose knowledge survives
loss even when Server can report only Unknown. Server-local cleanup can stay pending indefinitely;
there is no automatic retry after an ambiguous lifecycle effect. The post-ACK nonallocation claim
needs code-local bounded-removal evidence; this model proves ordering only, not allocator behavior.

Update this model and its registered action coverage when the lifecycle adapter, carrier result,
viewer/controller cleanup or Server reservation fence changes. Remove it and its registration
when a maintained replacement owns these claims and their executable refinement mapping.

### Retained lifecycle receipts

The accepted Worker refinement keeps one optional Close cell inside each existing retained terminal
binding. It adds no binding eviction or second global operation/result slot. The cell owns one sticky
Close operation/target, the highest attempt/step, the last exact step receipt, the last exact mutating
receipt, and sticky mutating-Unknown/KnownDisposed facts.
[The lifecycle kernel](src/terminal_authorization/lifecycle.rs)
implements this receipt state inside the existing binding and global slot.

After checking current Worker/generation/target association and lifecycle capability, exact last-step
replay returns its cached result; otherwise exact last-mutation replay remains readable after queries.
Reusing either retained request ID with changed fields is `LifecycleConflict`. Other old/conflicting
steps are refused without effects; there is no unbounded receipt ledger. New dispatch needs a fresh
lifecycle lease and fresh handoff validation. Cached exact replay does not require renewing that lease.
The current attempt requires a strictly greater step ordinal; a later attempt begins at ordinal one.
The wire domains are ordinal **1..32** and nonzero u64 attempt generation; neither wraps, and retained
exact replay remains available at exhaustion.

Inventory observation cannot erase a mutating Unknown or KnownDisposed. Once mutation is Unknown,
all later mutation is refused, including a higher attempt. The current Server parks this state
nonresumable and does not grant a new query/reconciliation budget. Known blocked states may Resume;
this Worker observation allowance creates no new Server recovery route. After KnownDisposed, queries
may report failure without erasing disposal; only the exact retained Dispose receipt can replay
Disposed, and only after the one-step cleanup boundary. New termination or Dispose is refused.

Original exact cached Launch/Start replay precedes the `LifecycleActive` fence. A different operation
or higher-fence replacement on the same product binding cannot reset the Close owner or start new
execution. The existing global slot is a structural single-slot premise; it does not independently
prove concurrency correctness.

| Sequence claim                        | Required finite observation                                                                                                                                  | Required deterministic refinement                                                                                               |
| ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------- |
| Retained ID conflicts cannot dispatch | Candidates keep an old request ID while changing attempt/ordinal; actual handoff counts include an error value of two and independently require at most one. | Same ID with higher ordering/action conflict; exact old mutation replay after queries; older receipt refusal after replacement. |
| Queries preserve lifetime facts       | Observe actual handoff history after mutation Unknown and retained KnownDisposed after query Unknown.                                                        | Query after Unknown then higher-attempt mutation refusal; Dispose followed by query failure and exact replay.                   |
| Exhaustion does not erase replay      | Complete a maximum-attempt/maximum-ordinal receipt, then reach its exact replay.                                                                             | Ordinal 31/32/exhaustion and attempt MAX-1/MAX/exhaustion without wrap or cached-result loss.                                   |

The additional receipt model fixes one existing association, two attempts, two ordinals, two fresh-ID
variants and a conflicting Close owner. Derived fresh IDs permit multiple queries; explicit retained-ID
adversaries make identity independent of ordering where conflicts matter. It excludes transport,
cache products, fairness and successful recovery; the one-step model owns handoff/cleanup refinement.
The runtime adapter must branch to lifecycle before ordinary `cleanup_closed`, which can issue extra
Host requests. Dispose uses prepared fixed target IDs and existing nonallocating removals. Current
Host Dispose removes attachments without emitting a false resize-lease event; this slice preserves
existing event rules and adds no speculative pre-ACK revocation policy.

### Worker runtime refinement

[The lifecycle adapter](src/terminal_lifecycle_operation.rs) consumes one admitted request and uses
the existing one-second `HostExchange`. Its dispatch branch precedes `cleanup_closed`, so unrelated
closed viewers cannot add Host requests to a lifecycle step. Preflight Host loss is the known
`HostUnavailableBeforeHandoff`; loss after handoff is Unknown. Exact ACK knowledge is retained before
result delivery, including Host or Server loss after that observation. Inventory requests ask for
at most 128 terminals in one page, also subject to the Host control-record byte bound. Every decoded
row, identity collision, cursor generation and offset is checked before projecting the target.
The current lifecycle capability preserves Host-reported `idle`, `active`, and `unknown` close
activity; absent activity remains `unknown`, without a historical-version fallback.

[Target cleanup](src/host_connection/target_cleanup.rs) captures up to eight exact viewer IDs and
one exact controller ID before Host I/O. It reserves one generation-local retired-target cell,
bounded by the existing 1,024 bindings. After the exact Dispose ACK, committing this prepared cell
and removing existing map entries performs no allocation or further Host exchange. Dropping viewer
entries releases queued records and feedback flights. Exact retired IDs absorb late validated
events and make old viewer Detach local; new attachment to the disposed target is rejected.
Prepared cleanup without an ACK neither removes owners nor establishes disposal.

| Runtime boundary                                              | Executable evidence                                                                                                                                                                                                                                   |
| ------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Receipt ordering, identity, expiry and exhaustion             | [Kernel tests](src/terminal_authorization/lifecycle/tests.rs) cover old mutation replay after multiple queries, changed retained IDs, all later mutation after Unknown, original Launch/Start replay and the `LifecycleActive` fence.                 |
| One request, ACK correlation, page validation and result loss | [Adapter tests](src/terminal_lifecycle_operation/tests.rs) exercise the actual dispatch loop with an unrelated closed viewer, all mutation ACK kinds, one fixed-limit page, every target phase, identity conflicts and pre/post-handoff loss.         |
| Exact bounded nonallocating cleanup                           | [Cleanup tests](src/host_connection/target_cleanup/tests.rs) exercise zero/one/seven/eight viewers, the ninth refusal, queued record/flight removal, seven surviving controllers, distinct later IDs and the 1,024-cell boundary.                     |
| Real Worker and Host                                          | [Process witness](tests/process/terminal_operations/lifecycle.rs) holds eight target viewer entries, forces and observes settlement, disposes the target, replays after inventory, detaches old IDs locally and attaches eight fresh sibling viewers. |

These Worker witnesses do not prove Server's eight live Client reservations, delayed Acquire
revalidation, old Client lease Drop or the final Closed publication gate. Those remain Server-owned
refinement obligations; no Worker receipt permits automatic recovery from mutating Unknown.

## Verification

Run from the repository root. The real Host process witness consumes the normal Terminal Host build
artifact, so build it before a focused Cargo test; the root merge gate already builds before tests.

```sh
pnpm --filter @orchbee/terminal-host build
cargo test -p orchbee-worker --all-targets --all-features --locked
cargo clippy -p orchbee-worker --all-targets --all-features --locked -- -D warnings
cargo test -p orchbee-capability-node-protocol --all-targets --all-features --locked
cargo clippy -p orchbee-capability-node-protocol --all-targets --all-features --locked -- -D warnings
cargo test -p orchbee-local-peer-credentials --all-targets --all-features --locked
node scripts/tla/run-models.mjs --models local-session-startup,worker-send-transaction
pnpm verify
```

## Windows native execution

### Git process ownership

Windows Git discovery owns one suspended-assigned Job per command. Its private
adapter polls the inner root, then terminates remaining Job members and polls for
both root exit and the exact all-processes-exited completion event. Settlement has
one five-second cooperative cleanup budget on the retained discovery thread.
Only successful settlement permits a terminal receipt, the next command or slot
reuse. Failure retires the slot as `PhysicalUnknown`; it cannot be retried as proof.

The states are running, settling, settled or physically unknown. Settling retains
the Job and thread owner. OS calls may overrun cooperative deadlines; `poll` stays
pending and `stop_before` returns bounded failure without claiming cleanup. Drop
requests termination without blocking and proves no exit. Abrupt Worker death and
OS termination failure are outside successful-cleanup claims. Neither root exit
nor an empty Job member list proves all descendant process handles have signaled.

The existing `process-wrap` 10.0.1 `std`, `job-object`, `creation-flags` features and
`windows` 0.62.2 Threading bindings are shared with the native Client. The reviewed
[dependency correction](../../third-party/process-wrap/README.md) fixes completion
notification interpretation without adding authored unsafe or new dependency versions.
The Client's [dependency review](../orchbee/docs/windows-agent-integrations.md)
still governs the existing bindings. Native tests use already pinned `winsafe`
0.0.29 `kernel` as a Windows-only development dependency: retain a live descendant
handle before releasing the root or cancelling, then require it already signaled at
the terminal result. Existing discovery tests cover binary output, cancellation,
limits, failure cleanup and real Git execution.

The managed native Windows path uses the same Worker authorization, receipts, replay fences, and Host protocol. Private named pipes replace Unix sockets. The exact sibling `orchbee-terminal-host.exe` starts lazily after authorized Launch and uses ConPTY. Default launch uses the configured shell, or the first compatible PowerShell 7 on absolute PATH followed by system Windows PowerShell, with `USERPROFILE` as the initial directory. Worktree discovery runs Windows Git through owned child-output pipes with bounded output, cancellation, deadlines, and kill/wait cleanup. Native tests cover real Host retention after broker loss and real Git discovery. See the [Client quick start](../orchbee/README.md#native-windows-quick-start) for staging and limits.

## Windows child caller context

Every admitted Windows Launch adds `ORCHBEE_TERMINAL_ID` and `ORCHBEE_WORKER_ID`
to the Terminal Protocol environment overlay. Worker derives their full lowercase
hex values from the product launch authorization; Host applies them after inherited
and per-launch environment values. They are lookup hints for the native CLI, never
credentials. The existing overlay limits remain unchanged. This also applies to
explicit-endpoint Windows launches; the observation-channel policy is independent.
No Panel or Workspace position is stored because a Panel may later move. The native
`panel current` query resolves the location from one Server snapshot. Existing child
processes are not modified, and no Mac/Linux context injection is introduced.

Windows additionally mints one nonzero 128-bit Worker ID from the existing OS random
source before Server connection. `WorkerHello`, retained `ServerConnection`, and the
operation kernel share that exact identity. Separate connections do not reuse the
old fixed local diagnostic ID: otherwise different Sessions' terminal counters could
alias a stale caller. RNG failure or zero fails before connecting. Identity collision
resistance relies on the OS random source. This changes no OBLW/OBCN fields and does
not enable reconnect, inventory adoption, or migration of old Worker bindings.
Mac/Linux retain their existing identity behavior. Two independent Windows Session
process traces cover reused Terminal IDs and refusal of the old Worker context.

## Windows Worktree mutation owner

The [private mutation contract](../../crates/capability-node-protocol/README.md#worktree-mutation-job-contract)
owns the checked intents and wire vocabulary. `worktree_mutation_job` owns admission,
retained intent identity, one physical thread and completion receipts; the Windows Git
adapter owns literal argv, bounded output and exact process settlement. The Server
continuation remains responsible for Workspace association and end-to-end CLI recovery.

Start validates Worker identity/generation before touching the slot. Equal retained
ID/intent observes Existing; changed intent conflicts. One running job excludes another,
and 64 lifetime admissions bound retained receipts without expiring mutation identities.
A channel releases the owner only after its intent and join handle are retained. Poll
joins only an already-finished thread. A panic or unproven process cleanup retires new
admission and stays PhysicalUnknown. Cancellation is a request, never rollback. Session
shutdown cancels and joins this owner before reporting that background work has stopped;
pre-Host to Host handoff preserves the same slot. Normal transport exit settles it with the
other [background job owners](#background-job-owners-and-exit) in one six-second window. Drop only requests cancellation.

Git execution has a 30-second work budget, 64 KiB stdout bound and 2 KiB diagnostic
retention. These are cooperative admission budgets, not filesystem-call preemption.
A tracked native-spawn boundary distinguishes known no-child failure from post-creation
wrapper failure; the latter retires admission in discovery and mutation owners.
Every read is nonblocking and quantum-bounded; cancellation or a limit settles
the existing Job using the separate five-second cleanup budget. A mutation handoff
changes the failure observation to MayHaveChanged before it can affect the filesystem.
Only the existing Windows absolute drive-path contract is admitted; UNC/device and
drive-relative paths are rejected. Inherited `GIT_*` variables cannot redirect repository, index or config. Explicit trust
applies only to this command's source repository; global config is never changed.

Create checks an existing local branch before choosing checkout or `-b` with base; base
is ignored for an existing branch. Open selects one registered checkout
by path or branch. InspectRemove observes registration and dirty state. Remove rechecks
common-directory identity, excludes the main checkout and invokes Git's dirty/force
policy without deleting the branch or using a recursive filesystem fallback. Checkout
and common-directory facts are canonical observations, not a transaction against another
Git process or filesystem writer. A post-mutation observation failure preserves possible
effects instead of claiming rollback.

Native tests own disposable Git fixtures until the final child, pipe and repository
consumer has ended. Transition tests cover duplicate/conflicting Start, capacity,
cancellation, failed settlement and generation fencing. Real Git tests cover branch
creation/reuse, open selectors, dirty refusal, force removal and branch preservation.
These do not prove external-writer exclusion, abrupt Worker-death recovery or non-Windows
execution. No CLI parity is inferred from this Worker boundary alone.

InspectSource resolves the checkout containing the source directory; Open permits the main checkout. Create re-observes its branch after Git completes, so a successful hook changing HEAD yields RepositoryChanged with possible effects, not a false successful association. The Server now consumes these observations through its retained continuation; ownership and cleanup remain in this Worker.

## Windows source-based Worktree reads

`Start` with a present `trust_repository` reuses the discovery owner and physical slot for source discovery. Admission validates drive syntax only. Its owned task canonicalizes the directory and prepares trust, rechecks cancellation/deadline before spawning, and observes common Git directory and checkout root, then lists porcelain entries under one five-second job budget. Shared `git_repository_windows` removes inherited `GIT_*` repository/config injection and applies optional trust to the exact canonical repository for this command only; mutations use the same command preparation. An absent trust policy selects registered-directory listing. Real Git tests cover main/linked subdirectories, untouched dirty files/configuration, exact trust scope and owner shutdown. See the [source-query contract](../orchbee/docs/windows-worktrees.md#source-based-discovery).

## Windows interactive PowerShell directory model

The launch-fact owner recognizes only `powershell[.exe]` and `pwsh[.exe]` on Windows,
with no arguments or at most three exact case-insensitive `-NoLogo`, `-NoProfile`,
`-NoExit` switches. It preserves those switches and appends fixed in-process
initialization through `-NoExit -Command`. Unknown flags, scripts, command strings,
other executables and other platforms retain verbatim launch arguments.

The shared Local Runtime startup concatenates its common initialization and interactive
reader assets. Host builds consume those same bytes and recognize the exact prepared argv as an
interactive shell without injecting it twice. The `-Command` switch alone never grants this
classification: a modified script, extra command operand or noninteractive flag remains a program.
The reader admits only standard PSReadLine module/export identity and known definition fingerprints;
custom or unsupported definitions remain untouched and unknown for guarded Close. A/B/C markers
preserve the preceding command's status, reader results, history and key hooks. Skipped or repeated
installation cannot abort surrounding initialization. Encoded command helpers retain only
the common UTF-8/directory initialization, with no reader hook.

Initialization sets this child's native-output decoding and outgoing pipeline encoding to
UTF-8 without a BOM. Native CLI JSON therefore survives PowerShell pipelines on both 5.1
and 7. It changes no external shell or stored profile. The wrapper captures the profile-defined prompt, invokes it first so status-aware prompts
observe the previous command's success, then synchronizes a FileSystem provider location
to `[Environment]::CurrentDirectory`. Non-filesystem providers and failed assignments leave
the process directory unchanged. It installs once per process, never writes a user profile,
never interpolates launch data, and emits the original prompt result. Root process sampling
can then observe directory changes for native CLI creation without acquiring terminal input.

This is launch preparation before the existing Host handoff, not a second shell/process or
an acknowledgment that a terminal launched. Existing terminals receive no retrofit. Unit
admission cases and actual Windows PowerShell 5.1/7 prompt and pipeline tests cover native
UTF-8 decoding, custom prompts, failed
commands, Unicode directories, non-filesystem providers and idempotence. Real ConPTY creation
checks the root process cwd after Set-Location. Arbitrary custom startup commands remain outside
this integration; no current-directory accuracy claim is made for their private shell state.

## Explicit controller takeover

Takeover validates the expected committed incumbent and target before any Host effect. The single
operation loop releases that exact controller, waits for DetachTerminalAccepted, then attaches the
new input/resize controller. Late old effects fail token admission; late old cleanup cannot release
the replacement. Host refusal after detach leaves the old grant revoked. Missing detach proof
ends the operation with a broker error and never attempts new Attach. Windows-capable local-pipe
tests exercise real protocol exchanges, not a packaged Worker/Host process acceptance. The
[Server transfer model](../orchbee-server/docs/terminal-controller-takeover.md) owns the joint contract.

## Windows plugin process ownership

The admitted request loop also serves bounded plugin-job start/list/get/cancel records before and after Host creation. `plugin_jobs` retains up to 32 command owners and 200 logs; the shared `orchbee-local-runtime` Windows command owner supplies the same Job settlement used by Git. CLI disconnect leaves accepted work running. Explicit Session shutdown signals, settles and joins all plugin owners before reporting success; every other exit uses the shared [background job window](#background-job-owners-and-exit), and Drop never waits. [Native plugin behavior](../orchbee/docs/windows-plugins.md) owns invocation, paging, output limits and remaining integration.

## Plugin terminal launch environment

An explicit terminal launch may carry a separate `PluginTerminalEnvironment` (`pluginEnvironment` in OBCS). It admits only plugin ID/root/entrypoint/config/state/context, native binary path and optional Workspace/Tab/Pane display IDs: ten unique named entries, seven required, at most 24 KiB combined UTF-8. These are display data, not authority or plugin registration. The ordinary caller environment retains its existing limits and reserved-name refusal. Runtime base/scope and Worker/terminal identities remain owned by the Worker adapter. A plugin environment requires an explicit executable. OBCN launch kind 3 transports the explicit selection, ordinary environment and plugin environment under the 40 KiB frame limit. Server checks the complete encoded plugin launch before reserving a physical operation. Worker removes inherited known plugin display fields before applying this launch's values. Workbench plugin resolution, Pane association and Client placement remain separate integration responsibilities. Focused boundary and launch-fact tests cover this carrier; this is not a complete plugin Pane receipt.

## Windows physical title projection

The admitted Host connection retains exact `TerminalMetadataChanged` title
observations before the viewer consumer. Its `Titles` projection owns at most
1024 identities and 64 MiB of title text, rejects decreasing revisions and changed
replays, and drops observations at confirmed physical disposal or connection end.
The original title is presentation data, independent of Agent reports and input.

A read-only title batch uses the existing low-priority carrier. The lazy Windows
loop answers unavailable without starting a Host. The admitted loop drains queued
events, capped at 32 per read so a busy stream cannot require global quiescence,
and returns complete exact identities/revisions/nullable titles; it creates
no attachment or new thread. Invalid/corrupt Host events retire the connection,
with no stale title promotion. See the [title wire](../../crates/capability-node-protocol/README.md#windows-physical-title-observations)
and [Client model](../orchbee/docs/machines.md#live-terminal-title-observation-model)
for framing, Session fences, rendering and evidence limits.

## Workspace Git status reads

`StartStatus` uses the same owned discovery slot and five-second budget as source
queries. Literal `git status --porcelain=v2 --branch --untracked-files=no
--ignore-submodules=all -- :(top,exclude)**` emits branch headers only. Command-local
`core.fsmonitor=false`, `core.untrackedCache=false`, optional locks off, isolated
Git environment and no trust expansion prevent incidental hooks/cache/configuration
writes; no fetch occurs. Terminal receipts require child/pipes settlement and
retain exact status/stderr bounds. Real Windows Git tests cover ahead/behind,
no upstream, detached/unborn/linked/nonrepository states and unchanged data/config.
See the [status wire](../../crates/capability-node-protocol/README.md#windows-workspace-git-status-observations)
and [Client model](../orchbee/docs/machines.md#workspace-sidebar-model).

## Windows Server transport handoff

An explicit current-Server request can prepare, commit or abort an authenticated standby
connection. The same Worker loop retains its Host, authorization kernel, terminal identities,
Agent Drain and job owners; ordinary EOF never reconnects. This internal transport is available
before and after Host startup. [The owning model](docs/windows-server-handoff.md) defines exact
tickets, deadlines, rollback and evidence limits. Public Server state/endpoint handoff remains
a separate integration requirement; this transport receipt is not final CLI success.
