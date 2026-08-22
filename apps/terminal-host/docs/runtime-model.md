# Terminal Host Runtime Contract and Correctness Model

## Status and authority

This document is the authoritative runtime model for `terminal-host`. The accepted contract is
stable enough to govern implementation and review, while the application remains experimentally
versioned. Acceptance of the model does not by itself claim that the current implementation
conforms to every invariant; conformance requires the evidence defined below.

Authority is divided deliberately:

| Concern                                                                                 | Authority                                                                                  |
| --------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| Host runtime ownership, lifecycle, delivery, retry, and concurrency semantics           | This document                                                                              |
| Operator-facing purpose, launcher usage, and development commands                       | `../README.md`                                                                             |
| Transport records, message schemas, codecs, format versions, and protocol safety limits | [`@orchbee/terminal-protocol`](../../../packages/terminal-protocol/references/protocol.md) |
| Product topology, recovery policy, update policy, rendering, and platform effects       | Desktop                                                                                    |
| Platform process, PTY, and endpoint behavior                                            | The owning native adapter plus its native tests                                            |

An implementation comment, source layout description, or protocol codec must not define a second
version of a fact owned here.

## Purpose

Terminal Host is Orchbee's local authoritative terminal-execution service. For one active trusted
Desktop broker per endpoint, it owns:

- retained terminal identities and their lifecycle;
- each terminal's PTY and process tree;
- the canonical Ghostty terminal state;
- terminal configuration and ordered input admission;
- attachment-scoped projection, history, selection, and export state; and
- the private local endpoint through which Desktop accesses those capabilities.

The Host exists so that terminal truth and terminal resources do not move into Electron, a
renderer, or a product-layout model. A Desktop disconnect or restart destroys connection-scoped
capabilities but does not dispose retained terminals.

## Boundary and non-goals

One endpoint admits at most one active broker session. The endpoint's operating-system ownership
checks define the trust boundary: processes running as the same effective user are in the same
trust domain. The endpoint identifier is not a secret, and the Host does not authenticate one
same-user application against another. This is a single-user local service, not a multi-tenant
isolation boundary.

The Host does not own:

- Project, Workspace, Workbench, Tab, Panel, agent, or product-session models;
- concurrent independent brokers, same-user application isolation, or authorization policy;
- TCP, HTTP, WebSocket, TLS, remote authentication, or sharing gateways;
- product recovery choices, update timing, UI, rendering, platform clipboard contents or effects,
  external navigation, or localized product copy;
- search queries or match ranking; clients search bounded text-detail rows;
- durable PTY migration, Host-process crash recovery, or operating-system crash continuity; or
- a reusable native library API for another workspace.

Desktop may reconnect to a surviving Host process. If the Host or operating system crashes, the
contract does not promise that its PTYs or terminal processes can be recovered.

The owned process tree is the operating-system containment domain established at launch: the
dedicated session on Linux and macOS, the retained direct-child process-reaper subtree on FreeBSD,
or the Windows kill-on-close Job. Containment must observe that domain as having no live members
before cleanup is committed. On Linux and macOS, a same-user child that deliberately escapes the
POSIX session also escapes this contract; preventing hostile same-user code from detaching is not
part of the accepted trust boundary. FreeBSD containment follows the reaper subtree rather than
session membership.

The runtime also derives one non-authoritative close-activity observation for each projected live
terminal.
On POSIX, `active` means another process group owns the PTY foreground; on Windows, it means the
kill-on-close Job contains a process in addition to the retained shell. `idle` means the respective
platform query found no active command, and `unknown` means that query failed. Before any
activity-affecting input in a terminal lifecycle, OSC 133 shell integration may also classify a
cursor outside the prompt/input region as `active`, covering a command executed inside the shell
process. After such input, semantic markers are not causal evidence: only an independently observed
active foreground process may produce `active`, while inactive or failed platform observation is
`unknown`. This value is sampled for projection only: it never changes lifecycle, containment, or
process ownership, and consumers must treat `unknown` conservatively.

A Host with retained terminals remains available without a broker. A Host with neither a broker
nor a retained terminal may exit. A wire `ClientId` is diagnostic metadata, not connection
authority; only the current physical broker session grants attachment capabilities.

The protocol is experimental and negotiated as one exact compatible version. The Host does not
carry a compatibility translation layer. While this contract remains experimental, a breaking
wire change may reset the protocol, but its schema, TypeScript and native codecs, all producers and
consumers, conformance fixtures, and affected models must change atomically. Incompatibility closes
only the attempted broker session.

## Semantic ownership

Every mutable fact has one authoritative owner. Derived indexes and scheduler queues are allowed
only when they can be validated against, rebuilt from, or discarded without changing the owned
fact.

| Owner                        | Authoritative facts                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ---------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `HostState`                  | Host epoch and process defaults                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `TerminalRegistry`           | Terminal membership, creation-identity binding index, inventory generation, and the stable reference to each retained session                                                                                                                                                                                                                                                                                                                                                                                      |
| `RetainedTerminal`           | The allocation and one-to-one binding of exactly one `TerminalSession` to either one deferred `RuntimePreparation` or one started `TerminalEngine`; stable retained state never owns neither or both                                                                                                                                                                                                                                                                                                               |
| Local IPC connection         | `awaiting_hello`/`active`/`closing`, negotiated protocol limits, transport lifetime, and the physical connection                                                                                                                                                                                                                                                                                                                                                                                                   |
| `BrokerSession`              | The active connection identifier, diagnostic `ClientId`, and attachment membership                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `TerminalSession`            | Terminal identity, lifecycle, committed effective configuration and its revision, committed complete runtime theme with operation/revision identity, immutable initial start geometry after start, and the rebuildable last-synchronized engine observation                                                                                                                                                                                                                                                        |
| `TerminalEngine`             | PTY/process resources, Ghostty canonical state, content revision, generation, geometry, current and default dynamic colors, color scheme, bell sequence, current progress, one bounded pending clipboard-write intent, physical configuration/theme application, input queue, attachment-scoped held-input and focus-claim ledgers, terminal focus aggregate, projection-mutex-owned close-activity cache and permanent per-lifecycle input latch, bounded close-activity sample deadline, and failure observation |
| `AttachmentSession`          | Terminal binding, immutable admitted capabilities, current input capability and monotonic capability revision, demand, delivery flights, bell delivery cursor, history anchor, selection, and export progress                                                                                                                                                                                                                                                                                                      |
| Local IPC                    | Endpoint security, accept/read/write/timeout behavior, and record framing                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Native protocol adapter      | Lossless conversion between transport records and typed Host commands, replies, and events                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `@orchbee/terminal-protocol` | Wire schemas, codecs, message kinds, format versions, and protocol safety limits                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Desktop                      | Product binding, recovery decisions, escalation timing, platform effects, presentation, and updates                                                                                                                                                                                                                                                                                                                                                                                                                |

`TerminalEngine` remains one semantic owner, but it is not one implementation file. Its runtime
source is a directory of review units: one private state record, one narrow public handle, and
transaction modules for creation, lifecycle, containment, failure fencing, input admission and
writing, output parsing, terminal control, resize, projection, attachment, and history or
selection interaction. Each transaction module is the only implementation writer for the facts it
owns; a cross-transaction operation names its lock order and invokes the other owner through a
typed operation. Moving the owner into a directory does not create another terminal engine,
another Ghostty state, or another process owner.

The same rule applies to the Host-side owners. `state/` keeps one concrete
`HostCore` while connection, terminal-lifecycle, attachment, and projection
transactions live in separate owner-local files. `runtime_aggregate/` keeps one concrete
`HostRuntimeAggregate` while retained ownership, creation, attachment lifecycle, input admission,
configuration, runtime theme, geometry, terminal control, interaction access, projection,
termination, and maintenance orchestration are separate review units.
`runtime_maintenance/` separates scheduling from immutable runtime-observation conversion.
Their roots preserve the existing namespaces; none of these directories introduces another
registry, broker, retained-terminal map, or cleanup owner.

In particular, terminal state and terminal runtime must not live in independent authoritative maps.
The Host runtime coordinator allocates one `RetainedTerminal` whose adjacent fields are the complete
semantic session and its owned deferred launch preparation or started engine. `TerminalRegistry`
indexes a stable reference to that embedded session; it does not copy the session or maintain a
parallel engine lookup. Runtime access resolves the retained record through that reference, so
registry membership and runtime ownership cannot drift independently. An attachment is owned by its
broker session and refers to one terminal; a terminal must not keep a second authoritative
attachment registry. Terminal-scoped caches derived from active attachment demand are disposable
accelerators, not a second source of attachment truth.

The engine's input cleanup ledger is not an attachment registry: it contains no capability or
terminal binding and cannot authorize an operation. Core input-capable attachment admission creates
its empty slot in the same commit. Detach or input-capability downgrade removes it before Core
removes input authority; an upgrade prepares and commits a fresh empty slot before exposing the
new authority. The ledger owns only the exact key and application-mouse facts and attachment focus
claim accepted for that attachment, the terminal-wide OR aggregate of those claims, and the bounded
repair credit reserved for releasing them. Core projection demand remains the authoritative
visibility fact that gates whether a claim may exist.

`EngineObservation` is likewise not terminal truth. It is a monotonic snapshot copied onto the Host
owner thread at explicit synchronization boundaries so Core can validate delivery and serialize a
coherent inventory summary without reaching through an untyped runtime map. It may be rebuilt from
the engine and must never advance beyond the engine fact it records. The committed configuration is
different: `TerminalSession` owns its protocol-visible value and revision. A native update prepares
all Core preconditions, applies the value to the engine, and then performs an infallible session
commit; failed physical application leaves the committed configuration unchanged. Runtime theme
uses the same ownership direction: `TerminalSession` owns the complete protocol-visible theme
state, while a started engine or deferred `RuntimePreparation` owns its physical application. The
aggregate verifies owner agreement, applies the complete terminal theme, and only then commits the
next operation ID and theme revision infallibly. A rejected or failed effect leaves both owners at
the prior complete state.

The fair background synchronizer may process a bounded subset of retained terminals per poll. A
`ListTerminals` request therefore synchronizes the exact bounded page it will serialize after cursor
validation and before encoding it; one response never mixes an already observed page prefix with an
unsynchronized suffix merely because the global registry is larger than one poll budget.

### Composition observation and invariant oracle

The test-only composition observation is a copied projection of owner-provided facts, not another
semantic owner. Core and Engine expose narrow testing observations from their own state, while the
Service assembles them with connection and retained-runtime membership. Engine facts are copied
under the canonical projection, terminal, then input lock order; the observation owns no mutable
domain state and cannot authorize or commit an operation.

The composition oracle first runs the Core and Engine owner-local invariant oracles, then checks
cross-owner relationships. Retained runtime membership is one-to-one with Core terminal membership;
the active connection agrees with the Core broker; committed configuration and terminal theme
agree exactly; Core's observed revision never leads the Engine, geometry agrees whenever the
revisions are equal, and a settled terminal has an exact revision match. Each attachment retains a
positive capability revision and its immutable admitted read/resize value; current input capability
agrees with Engine input-ledger membership. Every Engine focus claim belongs to an input-capable
attachment whose Core projection demand is active, and the Engine terminal-focus aggregate agrees
with whether that claim set is empty. History-ledger membership and projection cursors likewise
agree with their owners and never lead the Engine generation or revision. Explicit model-only
fixtures may omit a native runtime, but cannot own or imitate runtime facts.

Service tests invoke this oracle after shared successful request and poll boundaries. Stateful,
generated, replayed, and fault-injected traces must also invoke it after every stable action or
injected failure.

## Core data model

The following pseudo-Zig describes semantic shape, not public source names:

```zig
const HostState = struct {
    epoch: HostEpoch,
    defaults: TerminalDefaults,
    terminals: TerminalRegistry,
    broker: ?BrokerSession,
};

const ConnectionState = union(enum) {
    awaiting_hello,
    active: struct {
        connection_id: BrokerSessionId,
        limits: NegotiatedLimits,
    },
    closing,
};

const TerminalRegistry = struct {
    // Each value is a stable reference to RetainedTerminal.session.
    by_id: Map(TerminalId, *TerminalSession),
    creation_index: Map(CreationId, CreationBinding),
    inventory_generation: InventoryGeneration,
};

const RetainedTerminal = struct {
    session: TerminalSession,
    runtime: union(enum) {
        awaiting_start: RuntimePreparation,
        started: TerminalEngine,
    },
};

const TerminalSession = struct {
    id: TerminalId,
    lifecycle: TerminalLifecycle,
    configuration: CommittedTerminalConfiguration,
    theme: TerminalThemeState,
    start_geometry: ?TerminalGeometry,
    observed_engine: ?EngineObservation, // derived, rebuildable after start
};

const BrokerSession = struct {
    id: BrokerSessionId,
    client_id: ClientId,
    attachments: Map(AttachmentId, AttachmentSession),
};

const AttachmentSession = struct {
    terminal_id: TerminalId,
    admitted_capabilities: AttachmentCapabilities,
    capabilities: AttachmentCapabilities,
    capability_revision: CapabilityRevision,
    bell_cursor: DeliverySequence,
    projection: ProjectionState,
    interaction: InteractionState,
};
```

Identifiers for hosts, brokers, terminals, creations, attachments, theme operations, requests,
revisions, and delivery sequences are distinct types. Conversion at a wire boundary is explicit.
Zero or a missing value is not silently reinterpreted as another state.

The production `RetainedTerminal` has no unowned or empty runtime variant. An immediate creation
constructs its native engine before committing the embedded session. A deferred creation instead
deep-copies and owns its complete prepared launch without constructing a child, PTY, Ghostty state,
or geometry, then commits the embedded `awaiting_start` session, launch-baseline theme state, and
both identity indexes in one allocation-free step. A theme transaction admitted before start
updates that owned preparation and the adjacent session together. `StartTerminal` constructs the
engine from the latest owned preparation at the request's exact geometry, releases the preparation,
and commits `running` plus the same initial geometry. A failed start retains the preparation and the
unchanged `awaiting_start` session for a safe retry. Disposal or Host shutdown removes the stable
session reference before destroying the preparation or engine and the complete retained record.
Deterministic Core tests allocate pure
standalone sessions through a test-build-only storage harness; they do not add a model runtime
variant to `TerminalSession`, production code, or wire-visible state.

### Terminal lifecycle

```zig
const TerminalLifecycle = union(enum) {
    awaiting_start,
    running: struct {
        termination: TerminationIntent,
    },
    finishing: struct {
        first: union(enum) {
            child_exited: ProcessResult,
            output_closed: OutputClose,
        },
        termination: TerminationIntent,
    },
    containing_failure: struct {
        failure: RuntimeFailure,
        containment: FailureContainment,
    },
    settled: TerminalOutcome,
};

const FailureContainment = union(enum) {
    stopping: struct {
        process: ?ProcessObservation,
    },
    blocked: struct {
        process: ?ProcessObservation,
        cleanup_failure: RuntimeCleanupFailure,
        retry_at_ns: MonotonicTime,
    },
};

const ProcessObservation = union(enum) {
    observed: ProcessResult,
    unavailable: ProcessObservationFailure,
};

const RuntimeCleanupFailure = enum {
    contain_process,
    release_output,
    release_process_identity,
};
const ProcessObservationFailure = enum { observe_process };

const TerminalOutcome = union(enum) {
    exited: struct {
        final_revision: Revision,
        result: ProcessResult,
    },
    failed: struct {
        failure: RuntimeFailure,
        final_revision: Revision,
        process: ProcessObservation,
    },
};

const TerminationIntent = enum {
    none,
    graceful,
    force,
};

const EngineFailure = enum {
    process_observation,
    pty_read,
    pty_write,
    resize,
    terminal_reply,
    worker_integrity,
};

const EngineCompletion = struct {
    child_exited: bool,
    exit_result: ?ProcessResult,
    final_revision: Revision,
    output_drained: bool,
    writer_stopped: bool,
    classification: union(enum) {
        pending,
        normal,
        failed: EngineFailure,
    },
};
```

`RuntimeFailure` is one semantic operation-stage variant. Its stable coarse wire kind is derived
from that variant, so an invalid kind/stage combination cannot be retained. It is not an English
product message. At minimum, unrecoverable PTY read, PTY write, terminal-reply admission, resize
fencing, process observation, and worker-integrity failures remain distinguishable.
The primary terminal-failure type excludes all cleanup stages. `contain_process`,
`release_output`, and `release_process_identity` exist only in `RuntimeCleanupFailure` after a
primary failure has already been retained.

`EngineCompletion.classification` is the only completion authority. `normal` and `failed` cannot
coexist, and `failed` retains exactly the first linearly published engine failure. Per-worker
diagnostic flags may not form a second completion model. A normal classification is legal only
with child completion, normal output drain, writer quiescence, and a process result. The Host maps
the engine failure into its semantic `RuntimeFailure` stage exactly once.

Outcome publication uses the semantic outcome kind (`exited` or `failed`) together with terminal
identity and final revision. It does not re-encode lifecycle as an independent failure boolean.
The transition into either settled variant captures that exact final revision in `TerminalOutcome`.
It is outcome data, not a value recomputed from later observations.

The failure carried by `ProcessObservation.unavailable` is always `observe_process`. A blocked
cleanup carries exactly one `RuntimeCleanupFailure`: process-tree containment, PTY output release,
or process-identity release. Dedicated contextual types make primary/cleanup confusion and invalid
kind/stage combinations unrepresentable in the Host model. The protocol adapter emits their
corresponding fixed wire `kind`/`stage` objects.

A retained terminal is created atomically in `awaiting_start` when its launch omits geometry, or in
`running` when an explicit geometry permits immediate startup; partially constructed terminals are
not observable. `awaiting_start` owns identity, effective configuration, and the complete prepared
launch, but no child, PTY, engine observation, geometry, attachment, termination intent, or visible
frame. Only exact start, inventory, creation replay, and direct disposal are admitted. A successful
start makes the supplied complete geometry the initial PTY, Ghostty, Core, and projected-frame
geometry. A spawn failure leaves the same deferred owner unchanged and retryable; an exact start
replay after success is idempotent, while a different geometry is a typed conflict.

`finishing` records exactly the first normal completion fact. `OutputClose` in this state means
normal PTY drain only. The complementary fact produces `settled.exited` only after the wait worker
has classified the completion path and the PTY writer is fully quiesced. An in-progress write
therefore still has an opportunity to become the terminal's typed failure before normal exit can be
committed. Process-observation or reap failure has the same precedence. Normal and failed
classifications are terminal alternatives; later disposal diagnostics cannot rewrite a retained
normal outcome. Owner-requested output abort and natural PTY drain race through one publication
boundary: the reader rechecks the abort request while holding the terminal mutex immediately before
publishing drain. An abort that linearizes first suppresses drain publication; a natural drain that
linearizes first remains a valid normal-completion fact.

Settlement is the terminal-owned mutation barrier. Once `settled.exited` or `settled.failed` is
committed, canonical content, generation, geometry, effective configuration, configuration
revision, complete runtime theme and theme revision, and observed content revision are frozen at
the captured final revision. Input, resize, configuration, and theme updates are rejected.
Projection, history, selection, export, inventory, and
outcome publication may continue only as reads of that frozen terminal state; broker-scoped
attachment and delivery state may still advance independently. The pending outcome becomes
published exactly once and is never recreated by a later read or acknowledgement. Explicit
disposal is retryably rejected until the exact settled outcome handoff commits publication; that
rejection preserves the complete retained-terminal record, outcome token, and attachments.

On POSIX, an owned descendant may remain in the process containment domain after closing every PTY
handle. Root-child completion plus normal PTY drain is still the terminal's normal outcome in that
case; the retained process domain is force-contained during explicit disposal. Keeping the POSIX
session leader identity unreaped until then prevents session-identifier reuse from weakening
containment. Exit observation therefore requires a non-reaping platform observation. On macOS the
wait worker first waits for `EVFILT_PROC/NOTE_EXIT`, then reads the exit status with a bounded
non-reaping `waitid(WNOHANG)` poll; an `EVFILT_PROC` registration `ESRCH` enters that same poll
because XNU may drain the process reference before the child becomes waitable. A transient
`CLD_STOPPED` can never become child completion. Observation failure is a typed runtime failure,
never a fallback that reaps the leader before containment. Final identity release uses
`waitpid(WNOHANG)`; a child that is not yet reapable returns `EAGAIN`, and the identity fence remains
retained for a later containment, observation, and reap attempt.

### Stable POSIX process identity (THCR-006)

A numeric PID is never an effect authority. On Linux and macOS, a session-member scan acquires one
`SessionMemberRef` for the process currently bound to a candidate PID before its authoritative
numeric session-membership observation, and uses only that retained reference for liveness and
signaling. The reference is released after the member operation. macOS may use `getsid` on the
reusable PID only as a negative prefilter before stable acquisition; a possible member is always
rechecked authoritatively after acquisition. If acquisition cannot be prepared or an observation
cannot be completed reliably, the platform operation fails and the lifecycle retains cleanup
ownership. An `ESRCH` from the initial reusable-PID macOS prefilter may skip a vanished candidate;
once that prefilter has matched, loss and every non-`ESRCH` observation error fail conservatively.
There is no raw-PID signal or liveness fallback. An authoritative non-member may be skipped, but a
matched reference that becomes invalid during signaling fails the scan for retry rather than
reporting success. A replacement process that inherits the numeric PID is neither targeted nor
accepted as empty evidence through the invalidated reference; a same-session replacement is
observed only by a fresh scan and stable reference.

- Linux acquires `pidfd_open` before `getsid` and signals through `pidfd_send_signal`; an actual
  signal returning `ESRCH` is an invalidated-reference retry, never silent success. Liveness first
  polls that same pidfd without waiting. No readiness is exact-live; `POLLHUP`, invalid events, and
  poll errors fail conservatively. A readable non-leader is probed with signal 0 through the same
  pidfd as the linearization point: success proves the exact unreaped zombie and is non-live, while
  `ESRCH` means the PID may already name a replacement and fails for a fresh scan. Other probe errors,
  including `EPERM`, fail conservatively. The retained session leader is the only readable special
  case: the Host owns and delays its wait, so its PID cannot be reused and it is safely non-live
  without a signal-0 permission check after a credential transition.
- macOS enumerates all process PIDs rather than filtering by real or effective UID, because session
  membership survives credential transitions. It uses an initial reusable-PID `getsid` only to
  reject definite non-members, then acquires a Mach task-name right and its `TASK_AUDIT_TOKEN`
  before the authoritative `getsid` and exact PID-version rechecks. Once the prefilter has matched,
  acquisition loss, `ESRCH`, a terminated/invalid task right, token change, or exact-ID loss fails
  the whole scan for retry; exec or same-session PID ABA can therefore never become false empty
  evidence. It signals through `proc_signal_with_audittoken`, validates liveness through the task
  right and exact PID-version query, and deallocates the task-name right after the member operation.
- FreeBSD verifies that the Terminal Host is a process reaper before any terminal leader is forked.
  `PROC_REAP_KILL` with `REAPER_KILL_SUBTREE` and `PROC_REAP_GETPIDS` use the retained direct-child
  leader as the subtree identity; the kernel resolves members while holding process identities.
  Containment reaps only zombie descendants in that subtree that have become direct Host children,
  never the retained leader, a non-child zombie, or a member of another terminal subtree.
  Descendant waits and retained leader waits are serialized across terminals. Under that mutex,
  containment performs bounded snapshot, targeted-reap, and fresh-snapshot passes: a fresh live
  member returns non-contained, a fresh targeted zombie forces another pass, and only a fresh
  snapshot with neither may return zero. Churn, truncation, exhaustion, or wait/query error fails
  conservatively while retaining ownership.

The per-terminal identity fence serializes graceful HUP, force STOP/KILL/CONT, containment
observation, and final leader reap. It admits those operations only while the leader identity is
retained. Containment confirmation must precede reap; after reap, the numeric session/subtree key is
never consulted again. Platform preparation, enumeration, stable-reference, signal, liveness, and
reap failures remain typed failures and preserve the retained ownership state for retry. A stable
member signal scan records its first member error but still attempts and releases every later
captured reference. A partial force KILL therefore enters a complete CONT compensation scan; on
FreeBSD, a successful `PROC_REAP_KILL` call with `rk_fpid` set is also a partial failure.

Windows declares the complementary drain fact only after its kill-on-close Job has no active
processes. Closing a pseudoconsole while a descendant remains can signal that descendant and can
block indefinitely on supported Windows versions, so Job emptiness is part of the Windows
adapter's safe-drain condition. This changes when the shared lifecycle may leave `finishing`; it
does not create a second lifecycle or permit an unsafe normal classification.

An unrecoverable runtime failure fences new client and terminal-generated input, stops further PTY
ingestion, retains the last revision known to be coherent, records the first linearly published
failure as its typed primary cause, and starts
force containment exactly once. Inventory and failure state remain observable throughout
containment. Runtime-backed projection, history, and selection are suspended while containment is
unconfirmed. A projection frame captured before the fence still requires the serialized
read-commit guard, so an unconfirmed failure cannot gain a new frame handoff through a stale
readability sample. After containment, existing read attachments may inspect retained coherent state;
resize corruption is the exception because the emulator cannot guarantee that its failed mutation
left a readable screen. When the owned process tree is confirmed stopped and runtime resources are
contained, the terminal becomes `settled.failed`, publishes its typed outcome, and remains
observable until explicit disposal. A
process observation may be typed as unavailable after an observation failure; that is distinct from
pretending that the child exited normally.

The primary failure value is the semantic stage itself; its coarser wire `kind` is derived by the
protocol adapter. Independent kind/stage fields and unowned platform-code placeholders are not part
of the Host model, so contradictory or permanently-empty diagnostics cannot be retained.

Later cleanup or observation failures are retained as containment diagnostics; they never replace
the terminal's primary runtime cause. Concurrent workers race through one atomic first-failure
publication point, so reporting does not depend on a hard-coded precedence among boolean flags.

Engine failure fencing and the first bounded containment attempt do not depend on a connected
broker. A worker performs only that engine-owned safety action; it does not mutate Host lifecycle or
connection state. The Host linearizes the immutable failure observation into `TerminalLifecycle` on
its next transition pass, and completes or retries owner cleanup there. Local IPC continues this
maintenance while waiting for an accepted connection to complete its handshake or a partial record;
transport progress is not a cleanup scheduler gate.

### Input admission and terminal replies

Client input and terminal-generated protocol replies share one PTY writer but not one admission
budget. Accepted client inputs retain FIFO order relative to other client inputs. Terminal replies
retain FIFO order relative to other terminal replies and receive priority within a bounded reply
burst: the writer delivers at most `max_terminal_reply_burst_slices` terminal-reply slices before
yielding to a waiting accepted client entry. Each slice is at most
`max_pty_write_slice_bytes`, so the burst bounds the reply work a client entry may wait behind.
Replies therefore still sit ahead of a large client paste when the child needs a reply to make
progress, but sustained reply work cannot starve accepted client input indefinitely. Once the burst
is exhausted the writer yields to a waiting client entry and the burst restarts after that client
slice; replies continue to make progress whenever no client entry is waiting. `InputAccepted`
remains queue-admission-only and never promises delivery order relative to terminal replies.

Semantic key and application-mouse input commit their attachment-owned held-state mutation under
the input mutex at the same linearization point that publishes their client FIFO entry. Preparation,
cancellation, rejection, and a late commit after detach mutate neither ledger nor queue. Key facts
retain the exact accepted text and modifiers; mouse facts retain press order and the latest accepted
sample. A wheel report has an empty pressed-button set and is never retained. A semantic key tap is
an ordered press and release, so it has no steady-state ownership while still remaining repairable
if attachment loss lands between the pair.

Raw and semantic input carry the attachment's positive capability revision in the binary authority
header. Core validates the exact current revision and input bit before Engine preparation, and the
prepared input validates them again immediately before its queue commit. A capability update is a
per-attachment monotonic compare-and-set: the current revision and value is an exact replay, the
exact next revision must change the input bit, and old, conflicting, or gapped revisions fail
without mutation. A downgrade first converts only that attachment's held ownership into ordered
repair and removes its ledger, then commits the new Core revision with input disabled. Already
accepted queue entries keep their one existing FIFO position; the transition neither duplicates
nor re-enqueues them. An upgrade reserves a fresh empty ledger before committing the new Core
revision and cannot restore prior held ownership or replay prior input. The original admitted
capability value remains the immutable `AttachTerminal` replay identity, so a live update cannot
turn an exact attach retry into a conflict or let that retry overwrite current authority.

An exact-revision focus input mutates the attachment's focus claim only while that attachment has
active projection demand and input capability. The Engine OR-aggregates all eligible claims for the
terminal: the first claim emits one semantic `FocusIn`, later claims emit no PTY bytes, non-final
releases emit no PTY bytes, and the final release emits one semantic `FocusOut`. Replaying the
attachment's current claim is inert. Paused, disabled, detached, or stale-revision input cannot
create a claim. Activating demand or upgrading input capability creates no claim and replays no
focus; a fresh exact-revision focus input is required.

Every newly held key or mouse button precharges its worst-case release bytes, and the first terminal
focus claim precharges the worst-case final `FocusOut` bytes. Held state and a non-empty focus
aggregate each reserve one future repair entry against the bounded client-input budget. Ordinary
client input cannot consume that credit. Projection-demand pause repairs and clears only that
attachment's held state and focus claim while retaining its empty input ledger; detach, broker loss,
or input-capability downgrade performs the same repair and then removes the ledger. Repair encodes
accepted key releases, then mouse releases, then `FocusOut` only when removal of the target claim
makes the aggregate empty. Only after repair commits does Core publish paused demand or remove input
authority. A failure to materialize or commit repair while the runtime is still live is an integrity
failure that fences the runtime and enters its containment path; natural exit or an already-fenced
runtime needs no PTY repair. Another attachment's ledger is never released, removed, or allowed to
produce an intermediate aggregate transition.

Paste preparation carries the bracketed-paste mode observed by the reviewing projected client. The
engine holds the canonical terminal mutex while comparing that expectation with Ghostty mode and
encoding the paste. A mismatch is an `input-mode-changed` rejection and reserves no FIFO position;
it is non-retryable because automatic replay would bypass a fresh inspection decision.

The client-byte and client-entry limits cannot consume the bounded terminal-reply byte and entry
budgets. Exhausting a client budget is ordinary retryable backpressure and never becomes a terminal
failure. Exhausting the independent reply budget means the child generated replies faster than the
bounded writer could carry them; that is the actual `terminal_reply` integrity failure, which uses
the normal fencing and containment path. A prepared client input reserves its FIFO position and
budget before reply preparation; cancelling it releases both and admits no bytes.

Ghostty's terminal handler remains the only terminal-state mutator. The Host-owned stream adapter
delegates every ordinary parser action and effect to that handler and intercepts only `DCS $ q`,
because the pinned allocation-free `ghostty_vt.TerminalStream` intentionally ignores DCS actions.
Ghostty's DCS handler classifies the bounded DECRQSS payload, and the Host encodes DECRPSS from the
same canonical SGR, cursor, and margin state before admitting it to the terminal-reply queue. It
does not enable XTGETTCAP, tmux control mode, or a second VT owner. A retained legacy corpus fixes
the exact ordered bytes for DSR, primary/secondary/tertiary DA, XTWINOPS, color-scheme, XTVERSION,
ENQ, and supported and unsupported DECRQSS queries.

`TerminationIntent` moves monotonically from `none` to `graceful` to `force`; force subsumes
graceful. An accepted intent performs its platform effect at most once. A partial or failed
termination effect enters typed failure containment instead of recording a successful intent.
Failure cleanup may be retried independently when the platform reports that process containment,
PTY output release, or process-identity release did not complete. A transition into failure never
discards a child-exit observation already retained by `finishing`.

If owner cleanup cannot complete, the terminal remains in `containing_failure.blocked`. A force
request may retry cleanup. Disposal is rejected with the typed failing cleanup phase; it must not
silently orphan a process tree or release memory still reachable by a worker. The Host retains the
terminal until process containment, worker quiescence, and identity release all succeed. The Host
retries independently of broker connectivity on a bounded schedule and limits attempts per poll;
`retry_at_ns` is internal monotonic scheduling state and is not part of the wire summary.

### Fatal-teardown retained cleanup (THDD-001)

A fatal teardown is the Host's last-pass shutdown after `serveUntilIdle` fails with an error other
than a recoverable class such as `PeerIdentityMismatch` (for example listener descriptor exhaustion).
The teardown attempts to stop and join every retained runtime. When one retained POSIX process tree
cannot be confirmed contained — session observation or signaling fails throughout the bounded
containment window, or live members never produce two consecutive empty observations before the
eight-attempt limit — the single failed pass must NOT abandon the tree. THDD-001 makes the ownership
explicit:

- **Retained cleanup ownership**: an unsettled failed terminal continues to use
  `containing_failure.blocked`. A terminal whose normal or failed outcome is already settled instead
  retains that immutable outcome and records the typed cleanup failure in an independent
  terminal-owned `retained_cleanup.blocked` state. Both forms carry a bounded retry deadline derived
  from the same Host-lifetime monotonic cleanup clock used by the broker-independent scheduler. The
  aggregate, runtime, and POSIX session identity remain fully retained; containment was not
  confirmed, so the session leader identity is never released (the session-identity fence from
  THCR-006 holds).
- **Bounded backoff cadence**: the Host enters a retained-cleanup window that reuses the existing
  bounded cleanup-retry scheduler. Each pass retries only cleanups whose `retry_at_ns` deadline is
  due, so retries fire on the same one-second bounded backoff cadence rather than spinning. The
  window is bounded to a fixed number of passes.
- **Endpoint and lifetime-lock behavior**: during the window the local endpoint stays bound and the
  lifetime lock stays held (the server is deinitialized only after the window resolves), so no second
  Host can start for the same endpoint while an unconfirmed tree is still owned. The Host accepts no
  new broker sessions while the lifecycle is shutting down.
- **Observability**: the retained-cleanup state is surfaced by an observable status (`active`,
  `attempts`, `exhausted`) and by completion/exhaustion logs, so an operator can see that cleanup is
  being retried rather than silently dropped.
- **Permanent-failure policy**: after the bounded budget is exhausted with a tree still unconfirmed,
  the Host does not silently orphan or release it. The blocked cleanup becomes a stable explicit
  exhausted state, the observable status records `exhausted`, the failure is surfaced loudly, and
  teardown exits with the shutdown failure instead of pretending a clean release. Exhaustion is a
  cleanup result only; it never rewrites an already-settled normal or failed terminal outcome.

### Attachment delivery

Projection sequencing and delivery form one state owner. The next sequence and the highest
abandoned sequence are not attachment metadata that may advance independently of the tagged
delivery state:

```zig
const ProjectionState = struct {
    next_sequence: DeliverySequence,
    last_abandoned_sequence: DeliverySequence,
    delivery: ProjectionDelivery,
};

const ProjectionDelivery = union(enum) {
    paused,
    ready: union(enum) {
        snapshot: ?FrameBase,
        delta: FrameBase,
    },
    in_flight: struct {
        base_before: ?FrameBase,
        frame: FrameFlight,
        dirty_after_capture: bool,
    },
    blocked: struct {
        base: ?FrameBase,
        observed_revision: Revision,
        reason: ProjectionBlock,
    },
};
```

`FrameBase` includes the exact acknowledged generation, revision, and sequence. The nested ready
union makes a delta without a base unrepresentable; a snapshot may retain its old base only for
duplicate-acknowledgement recognition and does not use that base for delta capture. A blocked
delivery records the exact revision captured or attempted under the engine's terminal-state mutex,
even when the scheduler has already observed a newer revision by the time encoding fails. Core
rejects a failed revision newer than its current terminal observation. A block remains stable only
while that observation is not newer than the failed revision; lazy attachment refresh therefore
makes an already-observed newer revision snapshot-ready without another PTY mutation or demand
toggle. In particular, the absence of an acknowledged base does not itself unblock an initial
snapshot failure. `base_before` lets a negative acknowledgement
invalidate the current flight without inventing an acknowledged base. Frame commit advances
`next_sequence` and enters `in_flight` as one transaction; rollback restores both. Pausing an
in-flight delivery records its sequence as abandoned before entering `paused`, so a late
acknowledgement is an idempotent no-op rather than authority over a newer flight.

Before batch handoff, a failed reservation or encoding may roll back and reuse an unpublished
sequence because no client could have observed it. Committing the exact encoded record into the
handed-off batch publishes `(HostEpoch, AttachmentId, sequence)` as an immutable delivery identity:
the Host never re-encodes different bytes or different commit metadata for that scoped sequence.
If transport fails after handoff, the broker session and its attachments are destroyed instead of
reusing or reconstructing the published sequence. This invariant lets the capability-scoped local
client recognize a duplicate from its fixed header without hashing or decoding the payload again.

An encoded projection frame remains provisional after capture. Egress first reserves record and
byte capacity and adopts the one encoded allocation, then the engine acquires a read-commit guard
on the same terminal-state mutex used by completion and failure publication. The guard revalidates
resize fencing, completion classification, the captured generation, and the one-shot failure-fence
epoch, and remains held through the allocation-free outgoing-record and Core-flight commit. A
generation change or failure publication invalidates every earlier provisional frame. Cancelling
such an adoption releases its capacity and allocation without advancing the flight, sequence,
projection cadence, scheduler position, or visible outgoing batch. If the guard wins, exactly one
frame commits before a generation-changing mutation or failure publisher can acquire the mutex.
Confirmed containment may again admit a newly prepared retained-state frame, but cannot revive a
pre-fence provisional frame; resize failure never admits retained reads.

History row delivery and plain-text selection export use the same principle: each flight has one
positive, strictly increasing sequence; only its exact acknowledgement can commit it. The control
boundary rejects HTML and raw VT export values before runtime access. Stale or duplicate
acknowledgements cannot advance a newer flight. Pausing projection or destroying the attachment
abandons its flight and invalidates its delta base without changing canonical terminal state.

The maintained Ghostty surface fixtures refine `TH-006` across the native/browser boundary. Their
bytes are produced only by enabling mode `2027`, feeding fixed UTF-8 streams through one pinned
Ghostty VT, and using one real Host projection encoder. Host verification regenerates both OBTF
snapshots and compares them byte for byte with the protocol-owned fixtures; the terminal package
consumes the primary snapshot through its decoder, packed store, and both renderers, then commits
the paired primary/alternate snapshots in sequence. This preserves one authority for grapheme
boundaries, cell widths, spacer topology, wrap placement, and terminal metadata without introducing
a browser VT. The primary corpus also carries one OSC 8 range across a soft wrap and closes it
before an explicit hard line; projected search, link, and accessibility tests consume those exact
row flags and cells instead of rebuilding equivalent synthetic rows. Its final rows set dynamic
default colors and exercise explicit RGB, inverse, faint, invisible, blink, overline,
strikethrough, every Ghostty underline kind, independent underline color, bold, and italic. The
same captured style records are decoded and rendered by both browser backends, so Host conformance
does not depend on a separately hand-built TypeScript style fixture. The second snapshot enters
the alternate screen and changes title, current directory, cursor, interaction modes, generation,
revision, and rows as one projection commit; the runtime reader test separately proves the 1049
generation transition.

The same corpus carries an OSC 8 target across its soft wrap and crosses OSC 133 prompt, input, and
output boundaries in its final row. Host encoding publishes the Ghostty-owned hyperlink, cell
semantic content, and row prompt class without parsing or inferring them in an attachment. The
terminal package consumes those fields for bounded link and semantic workflows; application policy
remains the only authority that may perform an external open effect.

Each live OBTF frame carries the runtime's cached `idle` or `active` close-activity observation in
its typed optional extension; omission means `unknown`. This three-value wire shape does not expose
the Host's internal freshness token. A client may use fresh `idle` to omit a destructive-close
confirmation, but `active`, `unknown`, or locally stale evidence always requires confirmation. The
value cannot authorize process control or infer containment. Historical OBPB row windows do not
carry this live fact, so applying a history window cannot refresh or clear the latest live-frame
activity decision.

The close-activity cache, activity epoch and exhaustion bit, and permanent per-lifecycle input latch
are owned by `projection_mutex`. A successful raw or non-focus semantic input, including one whose
encoding is zero bytes, commits its input admission and activity invalidation as one `projection ->
input` transaction: the latch becomes true, the cache becomes `unknown`, and the activity epoch
advances. Rejected input and focus-only input do not set the latch. After the engine commit, the Host
owner invalidates every read presentation for that terminal: a ready reader requires a snapshot, an
in-flight reader records a forced snapshot after its exact flight resolves, and a paused reader
remains paused.

The input latch never clears within that terminal identity and lifecycle. Writer completion,
partial or multiple writes, a PTY reader batch, buffered output, input echo, OSC 133 markers,
semantic prompt/cursor state, a foreground-process active-to-inactive transition, and elapsed time
are not an epoch-tagged acknowledgement from the shell and cannot restore idle. While latched, only
a successful platform observation of an active foreground process may publish conservative
`active`; an inactive or failed platform observation publishes `unknown` regardless of semantic
prompt state. Publishing `active` does not clear the latch, so every later inactive observation is
again `unknown`. Only construction of a new terminal identity/lifecycle starts with the pristine,
unlatched state in which a successful inactive sample may publish `idle`. Activity epoch exhaustion
is fail-conservative: the token saturates, its exhausted bit prevents an ABA match, and the cache
remains permanently `unknown`.

Sampling takes `projection -> terminal`; activity-affecting input takes `projection -> input`;
reader commits take `terminal`; and the writer accounts completion under `input` alone. No path
acquires `projection` from `input`, so cache invalidation preserves the runtime lock order without
cross-owner delivery evidence.

The Host projection scheduler samples each eligible terminal at most once per monotonic projection
cadence, before checking whether the scanned attachment is ready. Sampling therefore continues
with no terminal-revision delta and while that attachment has a frame in flight. An independently
observed foreground-process change, including a silent process-group change with no PTY bytes,
invalidates every reader and produces a same-revision forced snapshot. The activity sample deadline
advances when the query is attempted, so a full outgoing batch or retry cannot repeat the query in
the same cadence; ordinary provisional-frame and Core-flight rollback still preserves the exact
sequence and retry state.

Every encoded frame captures the activity epoch and exhaustion bit in Host-local metadata. The
read-commit guard takes `projection_mutex` then `terminal_mutex`, revalidates that token together
with generation and failure-fence epoch, and holds both locks through allocation-free outgoing and
Core-flight commit. Input or sampling invalidation between encode and commit therefore rejects the
old frame without consuming its sequence, including the maximum-counter ABA case. A new
same-revision snapshot encodes the current conservative value.

There is no ordering barrier from an input reply to a frame traveling in the opposite direction.
After a projected client publishes activity-affecting input locally, it keeps close activity
`unknown`. If the first later frame still carries idle, the client replaces `FrameApplied(S)` with
`NeedSnapshot(S, activity-stale)`. The ordered client-to-Host stream places that request after the
input; Host accepts only the exact in-flight sequence, abandons it, and forces snapshot `S+1`.
If Host accepted the earlier input, `S+1` must pass the activity-token guard after Host
invalidation and the permanent lifecycle latch permits only `active` or `unknown`. If Host rejected
or otherwise did not accept the input, the causal replacement may legitimately carry fresh
`idle`. The barrier proves ordering, not Host acceptance or shell consumption, and keeps a
pre-input idle frame unsafe even when it arrives after the local invalidation.

Kitty graphics stay inside the same terminal-state and projection transaction. Native Terminal
Host configures the pinned Ghostty VT for direct in-memory image media only and limits each primary
or alternate screen to 4 MiB of decoded image resources. Because Ghostty VT library builds leave
PNG decoding to their embedder, Host installs its pinned Wuffs decoder once before terminal
creation and rejects either a PNG container or declared RGBA dimensions above that same 4 MiB
budget before decoding. A capture selects only viewport-visible,
non-virtual placements at `z >= 0`; it copies the referenced, already decoded raw pixels while the
terminal mutex is held and never exposes a file path, temporary-file handle, shared memory, PNG
container, or compression stream. Visible images and placements are sorted before encoding so one
canonical state has deterministic bytes.

OBTF graphics extension kind `2` carries either a complete image-resource snapshot plus current
placements or a placement-only replacement naming the exact previously committed image revision.
A terminal snapshot always carries the complete form. A delta is placement-only only when its
acknowledged base is at or after the capture revision that installed the visible image set;
otherwise the whole terminal frame is promoted to a snapshot. Image retransmission,
visibility-set change, delete, reset, screen change, and reconnect therefore cannot retain or
resurrect stale client resources. Scroll-only placement movement may reuse confirmed pixels. The
aggregate 8 MiB frame, 5 MiB graphics-extension, 4 MiB decoded-resource, 256-visible-image, and
1024-visible-placement limits are independent checks.

This is deliberately not complete Kitty parity. Unicode virtual placements, negative-z
composition below text/background, glyph graphics, animations, file media, temporary-file media,
and shared memory are not projected. Historical OBPB row windows carry no image resources;
leaving the live bottom viewport clears the client overlay, and returning presents the newest live
state rather than replaying a history image.

The launch profile seeds Ghostty's default foreground, background, cursor color, complete palette,
cursor shape, and cursor blinking mode. Missing cursor-default fields normalize to the product
default of a block, non-blinking cursor; current profiles retain those values explicitly. DECSCUSR
and equivalent terminal escape sequences may override the active shape and blinking mode. Ghostty
RIS and an application `reset` restore the retained profile defaults, while projection remains the
only cursor input to a renderer. OSC 4, 10, 11, and 12 changes mutate the canonical color state:
projection resolves palette cells to RGB and carries the current dynamic defaults, while queries
produce terminal-reply bytes from the same state. The maintained surface corpus covers mutation and
projection, and the runtime test covers exact pinned-Ghostty color-query replies. A separate
real-PTY trace covers Host terminal-reply routing and accounting without depending on a fixed-length
color-query read. No attachment or renderer owns a second terminal palette or cursor default.

A Ghostty bell effect advances one terminal-owned positive monotonic `bell_sequence` exactly once
while the canonical terminal-state mutex is held. A newly admitted read-capable attachment starts
its `bell_cursor` at that current sequence, so attach, reconnect, and retained-terminal recovery do
not replay earlier audible intent. While the attachment exists, each later sequence is eligible for
one attachment-scoped `TerminalBell` event. The scheduler may hand off at most the next sequence in
one poll and advances the cursor only after the exact event record is appended to the successful
outgoing batch. Batch fullness or allocation failure leaves that sequence eligible for retry;
detachment or broker loss destroys the cursor and drops undelivered bell intent. Bell delivery has
no client acknowledgement and never mutates projection, history, or canonical screen content.

Ghostty OSC 52 is admitted only for selection kind `c`, standard-base64 plain text, valid UTF-8,
and at most 16 KiB after decoding. Clipboard reads (`?`), primary selection, invalid base64, invalid
UTF-8, NUL, disallowed C0 controls, DEL, and oversized text produce no event and no terminal reply.
The reader transaction stages the decoded allocation and publishes it only with the complete PTY
read batch. Each terminal retains at most one pending request; later OSC 52 writes are dropped until
that request is handed off or the runtime is destroyed. The interaction scheduler prepares it for
one currently scanned read-capable attachment, rolls preparation back if encoding or outgoing-batch
admission fails, and removes it only after the exact `TerminalClipboardWriteRequest` record is in
the handed-off batch. Runtime-wide in-flight ownership prevents a second attachment in the same or
a later poll from receiving the request. A new attachment or reconnect never receives an already
handed-off request. The Host never reads a platform clipboard, performs the write, stores a trust or
commit policy, or receives the application outcome; those remain application effects.

Ghostty progress reports update one terminal-owned current sample transactionally with their PTY
read batch. The state is `remove`, `set`, `error`, `indeterminate`, or `pause`; a present percentage
is bounded to `0..100`. Projection carries the sample in the optional OBTF progress extension, so
a new attachment receives current active state, intermediate values may coalesce, and `remove`
prevents an old task from reappearing. Progress never changes lifecycle, process ownership,
scheduling priority, notification policy, or terminal content revision.

Ghostty OSC 133 `C` and `D` semantic-prompt actions update one terminal-owned command interval
transactionally with their PTY read batch. The Host retains only the latest completion sequence,
duration, wall-clock completion time, and normalized exit code. It never retains or projects command
text, arguments, environment, or output. A failed reader batch publishes neither a start nor a
completion. The optional OBTF command-completion extension lets a newly attached client see the
latest fact; the Host does not decide freshness, focus, notification permission, rate limiting, or
whether a client should produce an effect.

Every `HistoryChanged` event allocates a positive attachment-local monotonic event sequence
transactionally with its handoff; the sequence is never reused or reset by a demand change, so a
re-entered viewing-history session without a terminal mutation still publishes a distinct identity.
A `HistoryObserved` carries the acknowledged event's own sequence. The Host clears exactly the
currently in-flight event whose sequence and revision/generation match; an acknowledgement older
than the current flight — already observed, abandoned by a demand change or screen/generation
invalidation, or superseded by a later event — is an idempotent no-op and never clears a newer
flight.

A successful cell-grid resize advances content revision and projection generation. It abandons
generation-pinned pending row captures and cancels already encoded selection-export bytes. A
committed history anchor and active selection remain attached to Ghostty's tracked pins when those
pins survive reflow; the Host republishes the selection's reflowed endpoints. Screen replacement,
retained-row eviction, or a garbage tracked pin instead releases the selection and reports
`evicted`, so reflow and content loss are not conflated.

Each attachment also owns one pinned Ghostty selection gesture. A start replaces the prior gesture
only after its click behavior and selection can be installed, an update applies Ghostty's
within-cell, rectangle, word-between, line, or semantic-output drag rule from bounded surface
geometry, and an ordinary end releases pointer activity while retaining only the click-sequence
anchor. Cancellation is point-free and idempotently resets the gesture and releases its tracked pin
while preserving the latest selection snapshot; it never substitutes for an ordinary end. Selection
commands, detach, generation or screen invalidation, and state teardown perform the same reset.
Invalid action-specific gesture fields are rejected by the protocol adapter before the runtime
mutation boundary.

Selection commands preserve Ghostty's canonical content semantics. Select-all trims surrounding
blank cells, excludes unused viewport rows, and leaves an empty screen unselected instead of
manufacturing a geometry-wide selection.

## Command and outcome contract

The Host boundary validates each envelope before dispatch. Control-payload parsing and every
control reply or event encoding go through the native protocol adapter; representation validation
and command-specific semantic validation both finish before a domain transition is invoked. Core,
runtime, projection, and interaction owners never parse raw JSON or manually assemble control
JSON.

The protocol-owned message contract determines whether a registered kind is legal as a request,
response, or event, which payload encoding that role requires, and which success kind answers a
request. Host dispatch and egress consult the native mirror before domain work or handoff; the
cross-language conformance corpus compares that mirror with the TypeScript registry. Business
callers do not supply a second expected-kind or encoding association.

Each operation declares its retry class:

| Operation class                     | Required semantics                                                                                                                                                                                                                                                                        |
| ----------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Read-only inventory or state query  | May be repeated; each reply describes one coherent observation                                                                                                                                                                                                                            |
| Create                              | While its terminal remains retained in one Host epoch, `CreationId` plus launch fingerprint is idempotent: an exact replay returns the same terminal and a different launch is a typed conflict. Explicit disposal releases the identity instead of retaining an unbounded Host tombstone |
| Deferred start                      | The first valid geometry starts exactly one child. An exact replay returns `already-started` with the committed geometry; reuse with different geometry is a non-retryable `start-conflict`. A typed spawn failure leaves `awaiting_start` owned and retryable.                           |
| Absolute desired-state update       | Reapplying the same effective value has no additional semantic effect                                                                                                                                                                                                                     |
| Attach                              | An attachment identifier is session-scoped; an exact replay resolves to the same binding and conflicting reuse is rejected                                                                                                                                                                |
| Detach, pause, and acknowledgements | Exact duplicates are no-ops; stale or cross-attachment values never affect current state                                                                                                                                                                                                  |
| Graceful and force termination      | Idempotent intents; escalation timing belongs to Desktop                                                                                                                                                                                                                                  |
| Input and relative interaction      | Ordered and non-replayable after an unknown result; automatic retry could duplicate user action                                                                                                                                                                                           |
| Dispose                             | Removes only a contained settled terminal whose exact outcome is published; pending publication or blocked containment returns a retryable typed failure; after an unknown result Desktop reconciles through inventory                                                                    |

For every request, one of these externally meaningful outcomes applies:

- **rejected**: no semantic commit occurred; a typed error says whether retry is meaningful;
- **applied**: one semantic commit occurred and the reply was delivered;
- **replayed**: an idempotency identity matched an earlier commit and no duplicate effect occurred;
- **outcome unknown**: the transport failed after the commit boundary and before the broker could
  confirm the reply.

Transport loss cannot roll back an already performed PTY or process effect. Desktop may blindly
retry only operations whose retry class makes that safe. It must reconcile or ask the user for
non-replayable operations such as input.

A failed disposal keeps the complete retained terminal. Pending outcome publication returns
`outcome-publication-pending`; blocked runtime cleanup returns one retryable typed code derived from
the exact owner phase: `cleanup-contain-process`, `cleanup-release-output`, or
`cleanup-release-process-identity`. Neither barrier is flattened into a permanent phase failure or
a generic termination failure.

## Commit discipline

Every fallible operation has an explicit `prepare -> effect -> commit` boundary:

1. **Prepare** validates authority and state, reserves required memory and capacity, and encodes any
   response data that must exist before commit. It does not mutate authoritative state.
2. **Effect** performs the smallest necessary PTY, process, or platform operation. When an effect
   can partially succeed, its compensation or failure-containment transition is defined first.
3. **Commit** is infallible with respect to domain allocation and changes authoritative state once.
   No later domain failure may leave half of a cross-owner transition visible.
4. **Transport handoff** may still fail. That produces an unknown result for terminal-scoped
   effects and closes the broker session; it does not pretend that the domain commit rolled back.

A runtime-theme request names one `TerminalThemeOperationId`, one expected theme revision, and the
complete terminal color state: color scheme, default foreground/background, nullable cursor color,
and exactly 256 palette entries. The Host reserves the complete result record before applying the
theme. Under serialized admission it checks exact replay before compare-and-set: the current
operation with identical theme returns `already-applied`, reuse of that operation identity with a
different theme is a non-retryable conflict, and a different operation based on an old revision
returns `stale` together with the complete current theme state. None of those paths mutates Core,
the deferred preparation, or the engine.

An admitted operation applies every terminal theme field to the one deferred preparation or
started engine before Core advances the operation identity and theme revision once. Deferred start
therefore constructs its later engine from the latest committed theme. A started engine replaces
both dynamic-color defaults and their current active values atomically under the canonical Ghostty
mutation lock, clears earlier OSC 4/10/11/12 overrides, and marks palette projection dirty. If that
changes active terminal colors, content revision advances once while generation remains unchanged;
if the complete defaults and active values already match, content revision need not advance even
though the independent Host theme revision commits the new operation. All attachments project the
same resulting content revision and color state.

Every successful semantic theme commit invalidates presentation delivery for each read-capable
attachment. A ready attachment therefore receives a fresh complete snapshot carrying the new
`themeRevision` even when the terminal content revision did not change. A frame already handed to
transport remains immutable and finishes its exact acknowledgement path; the attachment forces the
revision-tagged snapshot afterward. A blocked delivery is reopened as a snapshot, while a paused
attachment obtains the latest complete snapshot through its ordinary resume transition. Theme
revision is a presentation barrier and remains independent of content revision, projection
generation, attachment sequence, and history anchors.

After replacement, later OSC 4/10/11/12 actions may override the new runtime theme normally. Their
reset forms, terminal RIS, and Host-owned application reset restore the latest runtime theme rather
than the launch-time colors or an earlier OSC value. Cursor shape and blinking remain separate
retained profile defaults. Any validation, lifecycle, revision-overflow, runtime-coherence, or
physical preflight failure occurs before the semantic theme commit and preserves the complete prior
state. Recovery inventory always includes the current operation ID, theme revision, and complete
terminal theme, including the revision-zero launch baseline with a null operation ID.

A platform resize error is retryable only after the engine has restored the last committed PTY
geometry. If that compensation cannot be confirmed, the engine is fenced and enters typed resize
failure containment; Host state never assumes that a failed platform call was side-effect free.

Geometry has two admitted initiators and one native commit owner. An attachment holding the resize
lease may request a complete grid-and-cell-pixel geometry; that path applies the PTY effect before
committing Ghostty state, generation, and revision. PTY output may instead request Ghostty's
enabled 80/132-column mode. The Host stream adapter recognizes the parsed mode action rather than
reparsing bytes, serializes it with attachment resize, applies the new OS PTY grid and pixel extent,
then invokes pinned Ghostty's DECCOLM operation while the reader batch still owns its commit. Cell
pixel metrics remain unchanged on this terminal-originated path. Multiple column-mode actions in
one batch apply in parser order, and only the batch's final coherent geometry becomes observable.
After an attachment resize commits revision `r`, Host synchronization accepts an engine observation
at `r'` only when `r' >= r`. An observation at `r` must contain the requested complete geometry. PTY
output may commit between the resize and that observation; when it advances to `r' > r`, the newer
complete geometry and revision are canonical and are copied into Core without rewriting them or
aborting the Host. The resize acknowledgement continues to identify the accepted request commit at
`r`, while subsequent projection carries the newer observation.

For each running terminal, the broker grants at most one mutable resize lease. Attachment admission
retains the immutable `requestResizeLease` fact separately from current ownership, so an exact
observer attach remains idempotent after later promotion. Detaching the current owner atomically
selects the resize-capable observer with the earliest broker admission sequence. The Host reserves
both outgoing records before mutation, publishes one attachment-scoped `ResizeLeaseChanged` event,
and only then publishes the detach acknowledgement. If no eligible observer remains, the terminal
has no attachment resize owner until a later attach requests the lease. Settled terminals do not
promote a new resize owner.

The reader commit may release its terminal-state mutex only around that platform effect. A guarded
reader transaction prevents shutdown or an unrelated failure fence from treating the temporary
unlock as a completed batch, while `resize_effect_in_flight` prevents an attachment resize from
overtaking it. A failed platform effect skips the Ghostty mode mutation and the remainder of the
batch, then enters typed resize failure containment because the child cannot receive an ordinary
request rejection. A Ghostty resize failure after a successful platform effect first attempts to
restore the previous PTY geometry and still fences the engine because canonical screen memory may
be unsafe. On success, Core copies columns, rows, cell pixels, and revision from one engine
observation before projection or lifecycle settlement; a generation-changing snapshot is the
client-visible resize notification. Receiving that snapshot does not itself manufacture a reverse
attachment resize request.

Application-requested `clear` and `reset` are semantic Host operations, not PTY input. Both require
an input-capable attachment to a running terminal and execute while holding the same canonical
Ghostty mutation boundary as PTY output. They never write escape bytes to the child, enter the
client-input or terminal-reply queues, or feed synthetic bytes through a parser that may already
hold a partial child sequence. `clear` invokes Ghostty's complete active-screen erase and homes the
cursor while preserving the active screen, terminal modes, title, current directory, geometry, and
cell pixels. `reset` invokes Ghostty's full reset: it returns to the primary screen, discards the
alternate screen, clears screen and retained history, and restores modes, title, current directory,
cursor shape and blinking to their retained profile defaults, dynamic colors to the latest runtime
theme, tab stops, palette-related terminal state, and interaction ownership to their reset values;
geometry and cell pixels remain unchanged.

Each accepted operation advances terminal generation and revision exactly once, refreshes the
history model, invalidates projection bases and generation-pinned row or export work, and releases
selection or history anchors that no longer name retained Ghostty state. The next projected frame
is therefore a full canonical snapshot shared by every attachment. The initiating client also
repairs its browser-owned transient state before `reset`: it cancels IME composition, releases
pressed key and mouse state through ordinary semantic input, clears its selection through the
ordinary selection command, clears search and hover presentation, and resumes following the
bottom. The Host operation itself never drops input that was already accepted.

Authorization, running-lifecycle validation, and generation/revision overflow checks complete
before Ghostty mutation. Once mutation starts, its domain commit is infallible. A lost response
leaves the outcome unknown and must not be automatically replayed: repeated clear or reset may
converge to the same pixels but still advances revision and delivery identity. Projection
`store-reset` remains client recovery for an invalid encoded-frame base and is never interpreted as
either terminal operation.

Creation reserves registry and reply capacity before committing either retained form. Immediate
creation spawns before admission; if spawn succeeds but admission cannot complete, the process tree
is force-contained before the operation returns. Deferred creation admits the prepared launch and
identity without a process effect. Start reserves its accepted reply and required `TerminalStarted`
event before spawning, then commits the exact request geometry only after the complete engine exists.
If spawn fails, those records are rolled back and the retained preparation remains startable. Engine
workers may mutate only engine-owned queue or canonical VT state under the engine's synchronization.
They publish immutable lifecycle observations and do not mutate registry, lifecycle, broker, or
attachment state. The Host linearizes commands, runtime observations, timers, and acknowledgements
at one transition boundary.

A normal self-exit is legal only when no broker and no retained terminal remain. During fatal
process teardown, a failure while stopping one terminal is recorded but does not prevent the Host
from attempting to stop and join every other terminal. Errors are aggregated after every retained
runtime receives its attempt. A failed teardown pass is not described as clean resource release. If
the failed pass leaves an unconfirmed process tree, the Host does not exit through that boundary:
it retains the cleanup ownership and retries on the bounded backoff schedule for a bounded window
before reaching the permanent-failure state, so an unconfirmed tree is never silently orphaned
(see "Fatal-teardown retained cleanup (THDD-001)").

## Egress transaction

Outgoing records are built as a transaction over connection-scoped delivery state.

- Request dispatch synchronizes runtime observations before the command but emits only records
  caused by that request. Lifecycle publication, projection, and interaction scheduling run in a
  later poll, so an asynchronous encoding or scheduler failure cannot discard an already committed
  command reply.
- A record is first encoded and checked against record and byte limits. Large binary producers
  either write directly into a response reservation or return one owned allocation with transport
  header headroom. Egress adopts that allocation without copying. Immediate adoption transfers
  ownership when the record is appended; provisional adoption transfers it with a capacity
  reservation whose cancellation releases the exact allocation. Rejection or batch fullness
  leaves release with the producer.
- Its flight or acknowledgement state becomes committed only if that exact record is included in
  the batch handed to Local IPC.
- An ephemeral bell cursor follows the same append-before-commit rule even though it has no client
  acknowledgement: one appended event advances one attachment cursor by exactly one sequence.
- A pending clipboard-write intent has one runtime-wide preparation owner rather than one cursor per
  attachment. Failed append rolls it back; one successful append consumes it, so multiple readable
  attachments and reconnect cannot multiply a privileged application effect.
- If the next record does not fit, the already prepared prefix is returned normally and the next
  item remains eligible for a later batch. A full batch is scheduler backpressure, not a connection
  error.
- If an individual record can never fit, the owning capability enters a typed blocked state without
  consuming a sequence or corrupting a base.
- Before handoff, allocation or encoding failure rolls back all provisional delivery transitions
  and releases exactly one owner-held payload allocation. No large payload is reconstructed merely
  to cross the engine-to-egress ownership boundary.
- Projection adoption remains provisional until a terminal read-commit guard is held. Guard
  rejection cancels the reservation; guard success makes outgoing-record and Core-flight commit
  allocation-free and serial with failure publication.
- After handoff, any short or failed write closes the broker session. All of its attachments and
  delivery flights are destroyed. Retained terminals and their canonical state survive.

No caller may commit a frame, row flight, export chunk, or event and then discard the batch that was
supposed to carry it.

### Semantic trace vocabulary

Executable traces, reference reducers, shrink output, replay inputs, and formal counterexamples use
these exact stable action names:

- `Connect`, `Disconnect`, `Create`, `Attach`, `Detach`, and `Dispose` change composition
  membership or authority;
- `SetProjectionDemand`, `AcceptInput`, `Resize`, `UpdateConfiguration`,
  `ApplyTerminalControl`, and `RequestTermination` exercise terminal-scoped commands;
- `ObserveChildExit`, `ObservePtyDrain`, `ObserveRuntimeFailure`,
  `ConfirmFailureContainment`, and `PublishOutcome` linearize asynchronous lifecycle facts;
- `Poll` advances scheduled observation and delivery, `BeginShutdown` changes Service lifetime, and
  `Reconcile` is the required observation boundary after an outcome-unknown operation.

Every recorded step carries exactly one of `applied`, `replayed`, `rejected`, or `unknown`.
`rejected` leaves the reference state unchanged. `unknown` makes exact refinement intentionally
unavailable until a recorded `Reconcile` replaces uncertainty with a coherent composition
observation. The executable reference state contains only action-determined abstract facts; owner
queues, runtime resources, locks, and monotonic implementation counters remain outside it and are
checked by their owner-local oracles.

One externally atomic call may linearize several named actions. The reducer applies that complete
ordered action batch before comparing its single stable post-call observation. For example, a
successful `Poll` may also contain `PublishOutcome`; comparing between those inseparable commits
would invent a product state that never existed.

## State transitions

### Broker session

| Current state      | Input                                                | Next state       | Required effect                                          |
| ------------------ | ---------------------------------------------------- | ---------------- | -------------------------------------------------------- |
| no broker          | accepted local transport                             | `awaiting_hello` | Allocate a fresh physical session identity               |
| `awaiting_hello`   | valid compatible hello                               | `active`         | Commit negotiated limits and broker metadata             |
| `awaiting_hello`   | timeout, malformed or incompatible hello, disconnect | no broker        | Close only this transport; terminals are unchanged       |
| `active`           | disconnect or transport failure                      | no broker        | Destroy all session attachments; terminals are unchanged |
| any broker session | another connection attempt                           | unchanged        | Never admit a second broker session concurrently         |

### Terminal session

| Current state              | Input                                                        | Next state                                  | Required effect                                                                                                                                                         |
| -------------------------- | ------------------------------------------------------------ | ------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| absent                     | valid create with explicit geometry                          | `running`                                   | Prepare capacity, allocate one retained record, spawn exactly one runtime at that geometry, then atomically admit its embedded session and both indexes before replying |
| absent                     | valid create with deferred geometry                          | `awaiting_start`                            | Prepare capacity, allocate one retained record, retain the complete prepared launch, and atomically admit its embedded session and both indexes without spawning        |
| `awaiting_start`           | start with valid geometry; spawn succeeds                    | `running`                                   | Spawn the only runtime and atomically commit the same initial geometry to lifecycle and engine observation before replying                                              |
| `awaiting_start`           | start spawn fails                                            | unchanged                                   | Return a typed retryable failure; retain identity and prepared launch and expose no geometry, child, PTY, attachment, or frame                                          |
| `running` or later         | exact repeated start geometry                                | unchanged                                   | Return `already-started` without another process effect; conflicting geometry returns non-retryable `start-conflict`                                                    |
| `awaiting_start`           | valid runtime-theme compare-and-set                          | unchanged lifecycle                         | Replace the complete owned preparation theme, then commit its operation ID and next theme revision to the adjacent session                                              |
| `running` or `finishing`   | valid runtime-theme compare-and-set                          | unchanged lifecycle                         | Apply the complete engine defaults/current colors under one Ghostty mutation boundary, then infallibly commit the same complete semantic theme state                    |
| any retained state         | exact replay of the current theme operation and data         | unchanged                                   | Return `already-applied`; the replay check precedes lifecycle and stale checks and performs no physical or semantic mutation                                            |
| theme-mutable state        | stale theme revision                                         | unchanged                                   | Return `stale` plus the complete current theme state; conflicting current operation identity or any failed preflight also preserves both owners                         |
| `running`                  | child exits or output closes normally                        | `finishing(first)`                          | Retain exactly the first completion fact and reject new client input                                                                                                    |
| `finishing`                | complementary completion fact after writer quiescence        | `settled.exited`                            | Commit one immutable normal outcome                                                                                                                                     |
| `running` or `finishing`   | unrecoverable runtime failure                                | `containing_failure.stopping`               | Fence I/O, record failure, and initiate force containment once; do not publish a settled outcome yet                                                                    |
| `containing_failure`       | process tree and runtime resources confirmed contained       | `settled.failed`                            | Retain the typed failure and observed or explicitly unavailable process result                                                                                          |
| `containing_failure`       | owner cleanup phase fails                                    | `containing_failure.blocked`                | Retain the exact cleanup failure and schedule a broker-independent retry                                                                                                |
| `settled`                  | fatal teardown cannot confirm retained cleanup               | same outcome + `retained_cleanup.blocked`   | Retain the cleanup independently and schedule a broker-independent retry; never rewrite the outcome                                                                     |
| settled + retained cleanup | cleanup retry succeeds                                       | same outcome + `retained_cleanup.complete`  | Confirm cleanup while preserving the settled outcome and final revision                                                                                                 |
| settled + retained cleanup | bounded retry budget is exhausted                            | same outcome + `retained_cleanup.exhausted` | Keep ownership and the exact cleanup failure; reject disposal and never report clean release                                                                            |
| `running` or `finishing`   | graceful or force request                                    | same lifecycle with updated intent          | Perform each intent at most once; never dispose implicitly                                                                                                              |
| `settled`                  | input, resize, configuration, theme, or revision advancement | unchanged                                   | Reject the mutation; the captured final revision and terminal-owned visible state remain immutable                                                                      |
| `settled`                  | dispose while outcome publication is pending                 | unchanged                                   | Reject retryably before runtime shutdown or any removal side effect                                                                                                     |
| `settled`                  | dispose after outcome publication                            | absent                                      | Complete resource cleanup, remove the session reference and indexes, then destroy the retained record                                                                   |
| `awaiting_start`           | dispose                                                      | absent                                      | Remove the session reference and indexes, release the prepared launch, and destroy the retained record without termination                                              |
| any retained state         | broker disconnect                                            | unchanged                                   | Remove attachment capabilities only                                                                                                                                     |

### Attachment session

Attachment admission passes one canonical `Capabilities` value through Core
and Engine preparation. The Engine derives live-runtime admission from
`input || resize`, retained-readable admission from `read`, input-ledger
capacity from `input` only, and history state from `read` only. Resize has no
attachment-local Engine allocation. This final Engine check runs while holding
the terminal mutex, so a completion or failure that races after Core validation
cannot admit stale input or resize authority.

Core retains that admitted value as immutable attach-replay identity and initializes current
capability revision `1`. Protocol `0.0.17` permits only the input bit to change live; read and resize
remain equal to the admitted value. Every successful attach or update reply returns the complete
current capabilities and revision.

| Current state  | Input                                                                           | Next state                    | Required effect                                                                                                     |
| -------------- | ------------------------------------------------------------------------------- | ----------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| absent         | authorized attach with at least one capability                                  | projection `paused`           | Create exactly one broker-scoped capability binding; a resize lease requires resize capability                      |
| any live state | exact current input-capability revision and value                               | same state                    | Return the complete current binding as an idempotent replay                                                         |
| any live state | exact next revision disabling input                                             | same projection state         | Repair and remove only this attachment's input ledger, then revoke Core input authority                             |
| any live state | exact next revision enabling input                                              | same projection state         | Prepare a fresh empty input ledger, then expose Core input authority; do not replay prior input                     |
| any live state | old, gapped, conflicting, or unchanged next-revision input update               | unchanged                     | Reject with a typed non-retryable capability-revision failure                                                       |
| `paused`       | activate demand                                                                 | `ready.snapshot(base = null)` | Lazily acquire terminal render resources as needed                                                                  |
| `ready`        | provisional frame wins the serialized read-commit guard                         | `in_flight`                   | Commit its reserved outgoing record, exact frame, base, and sequence together                                       |
| `in_flight`    | exact applied acknowledgement                                                   | `ready`                       | Promote only the acknowledged base                                                                                  |
| `ready`        | capture cannot be represented within its semantic bound                         | `blocked`                     | Record the exact captured or attempted revision, which cannot exceed the current observation                        |
| `blocked`      | Core already observes a revision newer than the failure                         | `ready.snapshot`              | Lazily invalidate the failed capture and retry without requiring another terminal mutation                          |
| active state   | pause                                                                           | `paused`                      | Repair this attachment's held input and focus claim, then abandon the flight and invalidate its delta base          |
| any live state | another attachment's owner detach selects this earliest resize-capable observer | same projection state         | Grant current resize ownership and publish its scoped event before the detach acknowledgement                       |
| any state      | detach or broker loss                                                           | absent                        | Queue attachment-owned key/mouse/final-focus repair, then release pins, flights, selections, exports, and authority |

## Correctness invariants

Stable claim identifiers are used by tests, models, and review findings.

| Claim    | Invariant                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `TH-001` | Each retained `TerminalId` names one registry membership whose stable session reference belongs to exactly one `RetainedTerminal`; that record contains exactly one `TerminalSession` and, in stable state, exactly one deferred launch preparation or started engine. No engine exists outside a create/start transaction or retained record.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `TH-002` | Each mutable semantic fact has one authoritative owner; derived state cannot outlive or contradict that owner.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `TH-003` | At most one broker session exists per endpoint, and every attachment operation is authorized by that physical session plus its `AttachmentId`. A running terminal has at most one current resize lease; owner detach promotes the earliest admitted eligible observer without changing its immutable attach-replay identity. Accepted held input belongs to exactly that attachment; detach repairs and removes only its ledger before revoking authority.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `TH-004` | Broker loss repairs and destroys only broker-scoped attachment state and capabilities; it never cross-releases another attachment or disposes a healthy retained terminal. An unrecoverable repair integrity failure uses the explicit runtime-failure containment path.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `TH-005` | Creation is atomic and idempotent by `HostEpoch + CreationId + launch fingerprint` while the terminal is retained; explicit disposal releases that identity. Deferred creation exposes no runtime geometry or process. Start commits exactly the supplied initial geometry once, exact replay is idempotent, conflicting replay is rejected, and failed spawn retains a startable owner without an orphan process.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `TH-006` | PTY output, input encoding, projection, history, selection, and resize synchronization use one canonical Ghostty state and coherent revision/generation pair; a failed projection names its exact captured or attempted revision. A post-resize observation never regresses the accepted revision, requires exact geometry at the same revision, and adopts a newer canonical observation. `projection_mutex` exclusively owns cached close activity, its activity epoch/exhaustion bit, and a permanent per-lifecycle input latch. Successful raw or non-focus semantic input, including a zero-byte encoding, sets that latch and invalidates idle; rejected and focus-only input do not. While latched, only an independently observed active foreground process may publish active. Writer completion, PTY output or echo, semantic markers, inactive process observation, and time cannot restore idle; inactive or query failure is unknown until a new terminal lifecycle. Close activity never becomes lifecycle or containment authority. |
| `TH-007` | Accepted client input occupies exactly one client-FIFO position and atomically commits any held-input cleanup ownership; rejected input occupies neither. Held input precharges bounded detach-repair capacity. Terminal replies have independent bounded admission and priority within a bounded reply burst after which the writer yields to a waiting accepted client entry; an unknown client-input result is never automatically replayed.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `TH-008` | Normal exit is published only after child completion, normal output closure, and writer quiescence, irrespective of observation order. Settlement captures one final revision; terminal-owned content, generation, geometry, configuration, and revisions cannot advance afterward.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| `TH-009` | An unrecoverable runtime failure is explicit, fences unsafe I/O, and cannot be reported as normal drain or normal exit. Unconfirmed containment admits no new frame handoff. Cleanup remains Host-owned and retryable without a broker; a settled outcome cannot be rewritten by a later cleanup symptom or removed before its exact publication commits, and disposal never silently orphans an uncontained process tree. Every POSIX member signal and liveness decision binds to one stable kernel identity before the authoritative membership observation. An invalidated matched reference cannot report signal success or empty containment and requires a fresh scan. Linux distinguishes an exact unreaped zombie from a reaped/reusable pidfd using same-reference signal-0, except that the retained leader is protected from reuse by its Host-owned delayed wait. A recycled numeric PID can never target or count its replacement through a stale reference; a same-session replacement requires a fresh stable reference.           |
| `TH-010` | An outgoing delivery transition commits if and only if its exact record is in the handed-off batch; batch fullness returns a committed prefix without dropping it. Projection record and flight commit are allocation-free while the engine read-commit guard holds the canonical `projection -> terminal` locks and validates generation, failure-fence epoch, and the captured close-activity epoch plus exhaustion bit. Input or sampling invalidation rejects an older provisional frame without advancing its sequence. `NeedSnapshot(S, activity-stale)` abandons only exact flight `S` and forces a guarded snapshot at a higher sequence. A handed-off record is immutable for its scoped sequence and is never reconstructed with different bytes.                                                                                                                                                                                                                                                                                        |
| `TH-011` | At most one frame, row request, or export chunk of each kind is in flight per attachment; only the exact scoped acknowledgement advances it. A published frame sequence identifies one immutable record for that Host epoch and attachment.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `TH-012` | Bounds and backpressure reject or defer the current operation without corrupting retained registries, canonical terminal state, delivery bases, or sequence numbers. Close activity is queried at most once per eligible terminal and monotonic cadence, including same-revision and in-flight states; a full outgoing batch preserves the forced-snapshot retry without repeating that cadence's query. Activity epoch overflow saturates to permanent unknown and cannot create an ABA-valid idle frame.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `TH-013` | Every successfully acquired resource, including a POSIX stable member reference, has one release owner and is released at most once; a teardown pass attempts every retained runtime even when one attempt fails, and never reports failed cleanup as clean shutdown. FreeBSD descendant-zombie reaping is confined to the retained leader's reaper subtree and cannot consume that leader's wait result or another terminal subtree.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| `TH-014` | All control JSON crosses one native protocol adapter, all binary data crosses its owning codec, and every registered role/encoding/success-kind/dispatch-operation association comes from the protocol registry; malformed, unknown, incompatible, mismatched, or unhandled records cannot reach domain transitions.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `TH-015` | Each Ghostty bell advances one terminal-owned sequence exactly once. A read-capable attachment receives each later sequence at most once, advances only with exact outgoing-batch append, and never replays a sequence that predates its admission or survives its destruction.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `TH-016` | An input-authorized application clear or reset mutates only the Host-owned Ghostty state, advances generation and revision exactly once, invalidates dependent projection and interaction state coherently across attachments, never enters PTY input/reply queues, and is never automatically replayed after an unknown result.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `TH-017` | OSC 52 can publish only one bounded plain-text write intent for selection kind `c`; it never reads platform clipboard data. Reader-batch rollback publishes nothing, outgoing-batch rollback preserves the same request for retry, one successful handoff consumes it across all attachments, and reconnect does not replay it. Terminal progress is a bounded current projection sample and never becomes lifecycle or notification authority.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `TH-018` | One accepted runtime-theme operation atomically replaces color scheme, default foreground/background, nullable cursor color, and all 256 palette entries at one independent Host theme revision. Replay, stale, conflict, validation, overflow, lifecycle, and runtime-coherence failures preserve the complete prior Core and physical state. Recovery publishes the complete current state; all attachments observe one coherent content revision, and OSC/reset/RIS resolve against the latest committed runtime theme.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `TH-019` | Each attachment has one positive monotonic capability revision initialized to `1`. Input preparation and final queue commit require its exact current revision and enabled input bit. An exact capability-update replay is inert; stale, gapped, conflicting, and unchanged next revisions cannot mutate authority. Downgrade repairs and removes only the target attachment's held-input ledger before revocation, preserves already accepted FIFO entries without duplication, and upgrade creates a fresh empty ledger. Immutable admitted capabilities continue to define attach replay.                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `TH-020` | `TerminalEngine` OR-aggregates focus claims from attached, input-capable attachments whose Core projection demand is active. Only aggregate zero-to-nonzero and nonzero-to-zero transitions emit terminal `FocusIn` and `FocusOut`. Detach, broker loss, demand pause, and capability downgrade clear only the target claim after its key and mouse repair; a final focus repair is precharged and ordered last. Stale, disabled, paused, and detached input cannot create a claim, while activation and capability upgrade never replay one.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |

## Scenario decisions

| Scenario                                          | Contract decision                                                                                                                                  |
| ------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| Desktop restarts while terminals run              | Host retains terminals; the new broker handshakes, inventories, and creates new attachments.                                                       |
| A same-user process knows the endpoint            | It is inside the current trust boundary; application-level isolation is not promised.                                                              |
| A second broker connects while one is active      | It cannot become active concurrently.                                                                                                              |
| Protocol negotiation fails                        | Close that broker transport; retained terminals remain.                                                                                            |
| A response is lost after input admission          | Outcome is unknown; Desktop must not automatically replay the input.                                                                               |
| A theme response is stale, lost, or replayed      | Return or recover the complete authoritative theme state; exact replay is idempotent, while an unknown outcome must be reconciled before rebasing. |
| Deferred terminal start fails to spawn            | Keep the retained identity and prepared launch in `awaiting_start`; publish no geometry or start event, and permit an explicit retry or disposal.  |
| A scheduler batch fills                           | Return the prepared prefix normally and defer the next record.                                                                                     |
| A client observes a repeated frame sequence       | The already committed immutable frame is acknowledged idempotently from fixed-header metadata; it is not decoded or committed again.               |
| PTY read, write, reply, or resize integrity fails | Record a typed failure, fence I/O, confirm process-tree containment, then publish and retain the failed terminal for observation and disposal.     |
| One runtime fails during Host shutdown            | Continue shutdown for all other runtimes, then report the aggregate failure.                                                                       |
| Host or operating system crashes                  | PTY continuity and recovery are not promised. Platform cleanup mechanisms still apply where the OS provides them.                                  |

## Required correctness evidence

The implementation is not considered conformant until claims map to executable evidence. File
coverage alone is insufficient.

| Evidence                                                                                     | Required coverage                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| -------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [Host registry TLA+ model](../formal/tla/TerminalHostRegistry.tla)                           | `TH-001`, `TH-004`, `TH-005`, two-terminal identity uniqueness, atomic creation failure, broker-independent retention, settlement-gated disposal, and identity reuse after disposal; checked bounds live in its adjacent `.cfg`                                                                                                                                                                                                                                                                                                                                                                       |
| [Terminal lifecycle TLA+ model](../formal/tla/TerminalHostLifecycle.tla)                     | `TH-005`, `TH-008`, `TH-009`, geometry-gated start, failed-start stutter, writer quiescence, both normal-completion orders, failure containment, cleanup retry, exact settled revision, mutation freeze, and disposal from awaiting-start or settlement; checked bounds live in its adjacent `.cfg`                                                                                                                                                                                                                                                                                                   |
| [Host shutdown TLA+ model](../formal/tla/TerminalHostShutdown.tla)                           | `TH-013`, two-runtime teardown, failed-attempt retention, no short-circuit after one failure, and completion only after every retained runtime has been attempted; checked bounds live in its adjacent `.cfg`                                                                                                                                                                                                                                                                                                                                                                                         |
| [Delivery TLA+ model](../formal/tla/TerminalHostDelivery.tla)                                | `TH-003`, `TH-010`, `TH-011`, batch prefixes, acknowledgement races, pause/detach, reconnect, and transport failure; checked bounds live in its adjacent `.cfg`                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| [Request-outcome TLA+ model](../formal/tla/TerminalHostRequestOutcome.tla)                   | `TH-005`, `TH-007`, `TH-010`, `TH-016`, prepare/commit/handoff, retryable rejection, delivered reply, unknown result, idempotent replay, and forbidden non-replayable retry, including application clear/reset                                                                                                                                                                                                                                                                                                                                                                                        |
| [Runtime-theme TLA+ model](../../../packages/workbench/formal/tla/TerminalThemeRevision.tla) | `TH-018`, two-operation Host compare-and-set, atomic defaults/palette/cursor publication, exact replay, conflict, stale and failed no-mutation paths, unknown-outcome reconciliation, and the later client presentation barrier                                                                                                                                                                                                                                                                                                                                                                       |
| [Inventory TLA+ model](../formal/tla/TerminalInventoryPagination.tla)                        | `TH-002`, `TH-012`, generation-scoped pagination, exact membership, duplicate prevention, and stale-cursor rejection                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| [Frame-delivery TLA+ model](../formal/tla/TerminalFrameBackpressure.tla)                     | `TH-006`, `TH-009`, `TH-010`, `TH-011`, `TH-012`, `TH-016`, two-attachment projection mutation/capture separation, generation-changing clear/reset, exact failed revisions, provisional frame commit, failure publication and containment confirmation, no unconfirmed-failure handoff, one flight per attachment, acknowledgement bases, pause/abandonment, and blocked snapshot recovery                                                                                                                                                                                                            |
| [Close-activity freshness TLA+ model](../formal/tla/TerminalCloseActivityFreshness.tla)      | `TH-006`, `TH-010`, `TH-012`, permanent Host-side per-lifecycle invalidation for byte-bearing and zero-byte input, non-causal writer/output/semantic evidence, platform-only conservative active observation after input, silent same-revision process change, bounded sampling cadence, provisional epoch/exhaustion guards, batch-full rollback, current-token non-idle publication after latch, and forced successor snapshots for invalidated flights; consumer-local presentation and close decisions remain outside this Host model and are verified by the terminal client and Workbench tests |
| [Input-backpressure TLA+ model](../formal/tla/TerminalInputBackpressure.tla)                 | `TH-003`, `TH-004`, `TH-007`, `TH-012`, `TH-019`, `TH-020`, two-attachment held-input ownership and focus aggregation, visibility loss/gain without replay, monotonic input-capability downgrade/upgrade/replay/rejection, stale-input rejection, queued-input preservation, precharged repair without cross-release, independent client/reply admission budgets, client FIFO ownership, writer shutdown cleanup, reply priority, and bounded client progress                                                                                                                                         |
| [Process-identity TLA+ model](../formal/tla/TerminalHostProcessIdentity.tla)                 | `TH-008`, `TH-009`, `TH-013`, retained leader/SID fencing, stable member-reference acquisition before authoritative membership observation, identity-independent owned/foreign membership for a replacement PID, invalidated signal and liveness retries followed by a fresh scan, containment confirmation, reap, and later SID rebinding; no foreign replacement is signaled or counted, while an owned replacement requires a fresh stable reference; checked bounds live in its adjacent `.cfg`                                                                                                   |
| [History-interaction TLA+ model](../formal/tla/TerminalHistoryInteraction.tla)               | `TH-002`, `TH-006`, `TH-010`, `TH-011`, `TH-012`, `TH-016`, row flights and anchors, generation-changing clear/reset invalidation, history observation, selection eviction, exact export/chunk acknowledgement identity, stale search pages, and detach cleanup                                                                                                                                                                                                                                                                                                                                       |
| Composition invariant oracle                                                                 | `TH-001` through `TH-013` and `TH-016` through `TH-020` across owner boundaries: retained-runtime membership, broker identity, lifecycle and containment, configuration, terminal-theme, geometry, capability revision, input-ledger/focus eligibility agreement, and bounded projection observations after every stable Service step                                                                                                                                                                                                                                                                 |
| Stateful property tests                                                                      | `TH-001` through `TH-020` as applicable to each owner: registry/index coherence, lifecycle, terminal-control and runtime-theme command sequences, attachment capability-revision and focus-claim sequences, projection sequences, history/export sequences, and invariant checks after every action                                                                                                                                                                                                                                                                                                   |
| Deterministic fault-injection tests                                                          | Allocation, encoding, immediate-create and deferred-start spawn, queue admission, runtime-theme preflight, resize, PTY I/O, process observation, fail-conservative membership errors, scripted PID recycle immediately before graceful HUP, force STOP/KILL/CONT, and liveness, reply handoff, partial batch, and multi-runtime shutdown failures at every commit seam                                                                                                                                                                                                                                |
| Native integration tests                                                                     | Stable POSIX member identity and session/reaper-subtree process behavior, Windows Job/ConPTY behavior, endpoint ownership, peer identity, descriptor/handle cleanup, worker joining, and a real executable/Local IPC/PTY trace on every natively claimed platform                                                                                                                                                                                                                                                                                                                                     |
| Cross-language protocol conformance                                                          | Every kind's role, encoding, and successful response; every native command, reply, event, identifier type, and declared union or error variant; every transport and binary format; both sides of every negotiated or hard bound; and each declared malformed-record rejection class                                                                                                                                                                                                                                                                                                                   |
| Focused performance evidence                                                                 | Bounded memory, no-demand behavior, fairness, large input backpressure, projection cadence, and history/export bounds without weakening semantic invariants                                                                                                                                                                                                                                                                                                                                                                                                                                           |

Every TLA+ module header and stateful property-test suite must identify the transition vocabulary it
refines and the stable claim IDs it covers. Trace action names must use the transition vocabulary in
this document. Production changes that alter a state, owner, commit boundary, retry class, or
failure outcome must update this model and its evidence in the same change.

### Native evidence index

The following entries are the required executable refinements, not a claim that an unexecuted test
currently passes. A handoff may report conformance only after the package verification commands run
these entries successfully.

`src/service/__tests__/composition_observation_test.zig` exercises native-runtime and explicit
model-only composition, while the shared checked request and poll harness applies the same oracle
throughout Service integration suites. The Core test harness delegates to the same owner-local
oracle used by composition checks, so the two evidence layers cannot silently define different Core
invariants.

`src/service/__tests__/composition_refinement_test.zig` executes the stable semantic vocabulary
against a native runtime, including applied, replayed, rejected, and outcome-unknown/reconcile
paths. The adjacent `composition_reference*.zig` files keep state shape, transition reduction,
observation comparison, and orchestration as separate review units inside the test directory.

`src/service/__tests__/composition_state_machine_test.zig` generates deterministic multi-owner
HostService command sequences, replays every candidate from a fresh Service, and checks refinement
after each stable action or atomic action batch. A failure is delta-debugged by range deletion and
command simplification, then emitted as an exact value for
`ORCHBEE_TERMINAL_HOST_PROPERTY_REPLAY`; an individual generated stream can also be reproduced with
`ORCHBEE_TERMINAL_HOST_PROPERTY_SEED`.

`src/__tests__/host_black_box_test.zig` closes the remaining in-process assumption. The build passes
it the actual `orchbee-terminal-host` artifact; the runner creates a private endpoint, launches the
Host as a child process, and uses only real Local IPC records. Its exact observation sequence is:
process start, endpoint connection and Hello acceptance, terminal creation/start and attachment,
physical disconnect, reconnect in the same Host epoch, idempotent creation replay and reattachment
with the same identities, PTY input acceptance, a successful child exit whose final revision proves
PTY output was drained into canonical state, explicit disposal, and idle Host-process exit. A
watchdog bounds Host shutdown, while record and endpoint deadlines bound every external wait.

The ordinary package `test` target runs this trace. Repository CI therefore supplies native
execution evidence on Linux, macOS, and Windows; `test:cross` compiles the same runner for FreeBSD
but does not promote that compile result to PTY, IPC, or ABI runtime evidence.

Named application-system refinements close the affected cross-owner boundaries without replacing
the native evidence below. `terminal-parity/SU-13-cursor-defaults` observes the Host-owned explicit
block/non-blinking profile, a DECSCUSR override, and RIS restoration in real-PTY projected metadata;
renderer geometry remains owner-local evidence. The POSIX-only
`terminal-parity/SU-16-startup-geometry` records the child's first `stty size` before its shell
starts and requires it to equal both the independently measured mounted grid and the sole first
Host/client frame; it does not claim Windows ConPTY or a later resize. The controlled headed Canvas
`terminal-parity/SU-09-theme`, `terminal-parity/PR-11-bell`,
`terminal-parity/IN-10-read-only`, and `terminal-parity/IN-14-shared-focus` witnesses refine the
Host theme revision, bell sequence, capability revision, and aggregate-focus contracts through the
application boundary while capturing rather than claiming native theme, sound, notification, or
multi-window focus effects. `terminal-parity/HS-07-selection-export` adds the real PTY-to-Electron
plain clipboard consequence; generated TypeScript/Zig conformance remains the authority that HTML
and raw VT formats are rejected before runtime access.

| Claims             | Required native evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `TH-001`, `TH-002` | `src/core/__tests__/registry_broker_test.zig`, `lifecycle_observation_test.zig`, and `projection_state_test.zig`: registry/index coherence and all three stateful transition suites; `src/service/__tests__/runtime_aggregate_lifecycle_test.zig`: immediate/deferred ownership, failed creation/start, and replay identity                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `TH-003`, `TH-004` | `src/engine/__tests__/input_ownership_test.zig` and `runtime_input_test.zig`: held-input ownership, late prepared-input rejection, precharged repair, and containment on repair failure; `src/core/__tests__/registry_broker_test.zig`: one-broker attachment ownership and resize-owner promotion; `src/service/__tests__/connection_request_test.zig`: broker replacement, disconnect, and detach acknowledgement; `src/transport/__tests__/local_endpoint_test.zig`: failed-handoff preservation                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `TH-005`           | `src/model/__tests__/launch_test.zig`: all-numeric or all-null launch geometry and digest identity; `src/service/__tests__/runtime_aggregate_lifecycle_test.zig`: deferred ownership, failed-start retry, exact initial geometry, duplicate start, conflicting start, creation replay, and immediate creation failure; `src/service/__tests__/terminal_integration_test.zig`: real PTY disconnect/replay and isolated immediate spawn failure; `src/engine/__tests__/runtime_launch_test.zig`, `runtime_startup_test.zig`, and `runtime_state_test.zig`: canonical launch geometry, one-shot startup, and partial-start cleanup                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `TH-006`           | `src/platform/__tests__/process_test.zig`, `src/engine/__tests__/runtime_state_test.zig`, and `runtime_input_test.zig`: active-command classification, canonical parsing, terminal replies, OSC colors, projection metadata, and terminal-driven resize ordering; `src/engine/__tests__/history_rows_test.zig`, `history_selection_test.zig`, and `history_export_test.zig`: rows, reflow, selection, and export; `src/core/__tests__/lifecycle_observation_test.zig` and `projection_state_test.zig`: observation and delivery rules; `src/service/__tests__/projection_test.zig`: pre-write buffered output and post-write echo rejection, pre-existing active then idle, two written inputs with activity attributable to only one, zero-byte input, OSC 133 semantic-marker rejection, active publication without latch clearing, and query failure; `runtime_aggregate_lifecycle_test.zig`: serialized projection races and full-geometry synchronization; `runtime_aggregate_attachment_test.zig`: rejected/focus input exclusion, accepted raw/non-focus invalidation, deterministic runtime-revision advancement between resize commit and Host synchronization, and monotonic adoption; engine stream/projection/selection unit tests and the paired pinned-Ghostty surface corpus remain required |
| `TH-007`           | `src/engine/__tests__/input_queue_test.zig`, `input_ownership_test.zig`, and `runtime_input_test.zig`: bounded admission, priority, FIFO, settlement, and detach repair; `src/service/__tests__/runtime_aggregate_attachment_test.zig`: preparation rollback; `src/service/__tests__/input_request_test.zig`: raw/semantic failure equivalence and queue-admission-only acknowledgement                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `TH-008`, `TH-009` | `src/model/__tests__/lifecycle_test.zig`; `src/core/__tests__/lifecycle_observation_test.zig` and `registry_broker_test.zig`: lifecycle, immutable outcomes, and publication-gated removal; `src/platform/__tests__/process_test.zig`: retained leader/SID fencing, macOS stop-versus-exit observation, fail-conservative membership errors, and deterministic member-PID recycle immediately before graceful HUP, force STOP/KILL/CONT, and liveness counting through the production stable-member algorithm; engine completion/containment tests plus `runtime_state_test.zig` and `runtime_failure_test.zig`: state freeze and broker-independent containment; `src/service/__tests__/event_delivery_test.zig`, `lifecycle_request_test.zig`, `runtime_aggregate_lifecycle_test.zig`, and `runtime_aggregate_cleanup_test.zig`: handoff, dispose, synchronization, exact cleanup phases, and fair retry; `src/transport/__tests__/transport_maintenance_test.zig`: brokerless maintenance                                                                                                                                                                                                                                                                                                                |
| `TH-010`, `TH-011` | `src/service/__tests__/outgoing_test.zig`, `projection_test.zig`, `interaction_transaction_test.zig`, and `event_delivery_test.zig`: prefix, reservation, rollback, serialized commit, scheduler fairness, activity-epoch and exhausted-bit provisional rejection, same-revision forced snapshots, exact `activity-stale` flight abandonment, and S+1 recovery; `src/engine/__tests__/selection_export_test.zig`, `history_rows_test.zig`, and `history_export_test.zig`: row/export identity, rollback, and ABA rejection                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `TH-012`           | Protocol limit tests; `src/engine/__tests__/input_queue_test.zig`, `runtime_input_test.zig`, `runtime_state_test.zig`, `projection_test.zig`, and `history_rows_test.zig`; `src/service/__tests__/projection_test.zig`: at-most-once-per-terminal cadence with no revision delta, in-flight and batch-full retry, query failure, and activity epoch exhaustion; `src/service/__tests__/connection_request_test.zig`: reply reservation failure; `src/__tests__/protocol_conformance_test.zig`: complete TypeScript/native limit and `activity-stale` reason equality                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `TH-013`           | Engine startup/containment unit tests plus `runtime_state_test.zig` and `runtime_failure_test.zig`: ownership publication, partial startup, and contained destruction; platform process tests: failed exec, POSIX stable-reference release, deterministic PID-recycle containment, and Windows containment; `src/service/__tests__/runtime_aggregate_cleanup_test.zig`: all-runtime shutdown and retained cleanup; `src/transport/__tests__/local_endpoint_test.zig`: endpoint and connection cleanup                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `TH-014`           | Control decode/wire tests, protocol registry tests, and `src/__tests__/protocol_conformance_test.zig`; `src/service/__tests__/lifecycle_request_test.zig`: malformed, stale-epoch, unknown-optional, and role/encoding rejection before dispatch                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| `TH-015`           | `src/engine/__tests__/runtime_state_test.zig`: Ghostty BEL effect counting; `src/service/__tests__/interaction_delivery_test.zig`: per-attachment baseline, ordered delivery, batch-full retry, and destruction behavior; Desktop and Web projected-terminal tests: exact scoped forwarding and client deduplication                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `TH-016`           | `src/engine/__tests__/runtime_state_test.zig`: application clear/reset semantics and counter-exhaustion rejection; `src/service/__tests__/runtime_aggregate_lifecycle_test.zig`: input-capability authorization and exact Core observation synchronization; `src/__tests__/protocol_conformance_test.zig`: both control operations and exact accepted replies                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `TH-017`           | `src/engine/__tests__/runtime_state_test.zig`: OSC 52 read/selection/base64/UTF-8/control rejection, exact 16 KiB boundary, first-pending ownership, retry identity, and progress set/remove projection; `src/service/__tests__/interaction_delivery_test.zig`: outgoing-full rollback, one-recipient routing across two readable attachments, consumption, and reconnect non-replay; protocol, Desktop broker, projected-terminal, and Web renderer tests cover the bounded event and exact automatic application decision                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `TH-018`           | `src/model/__tests__/theme_test.zig` and `src/core/__tests__/theme_revision_test.zig`: complete state validity, compare-and-set, exact replay, stale/conflict, overflow, and no-mutation rejection; `src/engine/__tests__/runtime_theme_test.zig`: atomic physical replacement, OSC override/reset, RIS/application-reset priority, failure preservation, and two-attachment revision agreement; `src/service/__tests__/runtime_aggregate_theme_test.zig` and `connection_request_test.zig`: deferred/running cross-owner commit, reply preallocation, recovery inventory, replay/stale/conflict/failure results; `src/__tests__/protocol_conformance_test.zig`: exact TypeScript/native theme request, result, state, and registry agreement                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `TH-019`           | `src/core/__tests__/registry_broker_test.zig`: revision initialization, exact update/replay, stale/gap/conflict rejection, immutable attach replay, and invariants; `src/service/__tests__/input_request_test.zig`: two-attachment held ownership, target-only downgrade repair, prepared-input final reauthorization, queued-input preservation, disabled/stale wire rejection, fresh upgrade ledger, and current-revision acceptance; composition state-machine traces and protocol conformance cover revision visibility and exact cross-language bytes                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `TH-020`           | `src/engine/__tests__/runtime_input_test.zig`: two-attachment aggregate focus transitions, duplicate claims, retained-ledger visibility reset, ordered key/mouse/focus repair, precharged final release, stale prepared-input rejection, and repair-failure containment; `src/service/__tests__/input_request_test.zig` plus composition traces: stale/paused rejection, demand-loss repair, detach and downgrade ordering, activation/upgrade without replay, and invariant agreement; `src/service/__tests__/terminal_integration_test.zig`: deterministic real-PTY two-attachment focus trace; headed `terminal-parity/IN-14-shared-focus`: two production projected surfaces and the application command trace                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |

The deterministic commit-seam set is: creation preparation/spawn/admission; deferred-start spawn;
control-reply
reservation; raw and semantic queue reservation/publication with final capability reauthorization;
attachment input-ledger admission, focus aggregation, visibility repair, capability
downgrade/upgrade, and detach-repair materialization/publication/failure containment; application terminal-control
validation and canonical mutation; resize effect and unsafe rollback;
projection capture/encoding, provisional payload adoption, and serialized read commit; row,
history, selection, and export provisional mutation; batch prefix/full behavior; lifecycle outcome
handoff; Local IPC partial write; process observation and containment; multi-runtime shutdown; and
runtime-theme physical application before semantic commit.
A new fallible seam must be added to this set and receive a
named failure test before the implementation may rely on `catch unreachable` after that seam.

### Commit-seam fault matrix

Each stable seam has one required failure disposition and one scheduling class. `rollback` restores
the pre-transition observation, `reject_without_commit` reserves or validates before mutation,
`preserve_for_retry` retains an owned value for a later handoff, `publish_prefix` exposes only the
fully committed prefix, and `contain_and_retain` fences unsafe work while retaining lifecycle
ownership. Allocation-index schedules fail a chosen allocation, deterministic-interleaving
schedules admit an explicit named event order, native-boundary schedules use a failing platform or
transport adapter, and single-owner schedules exercise an ordered local transaction.

| ID       | Stable seam                            | Required failure disposition | Scheduling class           |
| -------- | -------------------------------------- | ---------------------------- | -------------------------- |
| `CS-001` | `CreationPreparation`                  | rollback                     | allocation index           |
| `CS-002` | `CreationSpawn`                        | contain and retain           | native boundary            |
| `CS-003` | `CreationAdmission`                    | rollback                     | allocation index           |
| `CS-004` | `ControlReplyReservation`              | reject without commit        | allocation index           |
| `CS-005` | `RawInputReservation`                  | reject without commit        | single owner               |
| `CS-006` | `SemanticInputReservation`             | rollback                     | allocation index           |
| `CS-007` | `InputPublication`                     | rollback                     | deterministic interleaving |
| `CS-008` | `AttachmentRuntimePreparation`         | rollback                     | single owner               |
| `CS-009` | `AttachmentCoreCommit`                 | rollback                     | allocation index           |
| `CS-010` | `AttachmentInputRepairMaterialization` | contain and retain           | allocation index           |
| `CS-011` | `AttachmentInputRepairPublication`     | preserve for retry           | deterministic interleaving |
| `CS-012` | `TerminalControlMutation`              | reject without commit        | single owner               |
| `CS-013` | `ResizeEffect`                         | contain and retain           | native boundary            |
| `CS-014` | `ResizeRollback`                       | contain and retain           | deterministic interleaving |
| `CS-015` | `ProjectionCaptureEncoding`            | preserve for retry           | allocation index           |
| `CS-016` | `ProjectionPayloadAdoption`            | rollback                     | allocation index           |
| `CS-017` | `ProjectionReadCommit`                 | preserve for retry           | deterministic interleaving |
| `CS-018` | `InteractionProvisionalMutation`       | rollback                     | allocation index           |
| `CS-019` | `BatchPrefixFull`                      | publish prefix               | single owner               |
| `CS-020` | `LifecycleOutcomeHandoff`              | preserve for retry           | single owner               |
| `CS-021` | `LocalIpcPartialWrite`                 | preserve for retry           | native boundary            |
| `CS-022` | `ProcessObservationContainment`        | contain and retain           | native boundary            |
| `CS-023` | `MultiRuntimeShutdown`                 | contain and retain           | deterministic interleaving |
| `CS-024` | `DeferredStartSpawn`                   | contain and retain           | native boundary            |
| `CS-025` | `RuntimeThemeCommit`                   | reject without commit        | single owner               |

`src/service/__tests__/commit_seam_matrix_test.zig` is the executable catalog. It requires a
one-to-one enum/table entry for every row above and locates the exact named test that supplies its
failure evidence. The shared scripted scheduler advances only through a declared event sequence;
concurrency evidence must use gates or that scheduler and never timing sleeps as ordering proof.

## Review rule

Review follows semantic boundaries rather than file order. For each transition, reviewers verify:

1. the authoritative owner and legal source state;
2. validation and reserved capacity before effects;
3. the external effect and its compensation or containment path;
4. one infallible domain commit;
5. reply/event handoff and unknown-result behavior;
6. resource cleanup for every failure edge; and
7. evidence for every affected claim ID.

File size is a mandatory review signal even when one semantic owner must remain intact. When an
owner no longer fits in one independently reviewable file, promote it to a directory and partition
its state declaration, public facade, and transaction implementations into named review units.
Keep one authoritative owner and make cross-unit commits explicit; merely moving code while
duplicating authority, permitting illegal state combinations, or hiding cross-owner commits is not
a boundary improvement.
