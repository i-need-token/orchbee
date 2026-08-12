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
- product recovery choices, update timing, UI, rendering, clipboard, external navigation, or
  localized product copy;
- search queries or match ranking; clients search bounded text-detail rows;
- durable PTY migration, Host-process crash recovery, or operating-system crash continuity; or
- a reusable native library API for another workspace.

Desktop may reconnect to a surviving Host process. If the Host or operating system crashes, the
contract does not promise that its PTYs or terminal processes can be recovered.

The owned process tree is the operating-system containment domain established at launch: the
dedicated POSIX session or the Windows kill-on-close Job. Containment must observe that domain as
having no live members before cleanup is committed. A same-user child that deliberately escapes
the POSIX session also escapes this contract; preventing hostile same-user code from detaching is
not part of the accepted trust boundary.

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

| Owner                        | Authoritative facts                                                                                                                                                                                                           |
| ---------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `HostState`                  | Host epoch and process defaults                                                                                                                                                                                               |
| `TerminalRegistry`           | Terminal membership, creation-identity binding index, and inventory generation                                                                                                                                                |
| Local IPC connection         | `awaiting_hello`/`active`/`closing`, negotiated protocol limits, transport lifetime, and the physical connection                                                                                                              |
| `BrokerSession`              | The active connection identifier, diagnostic `ClientId`, and attachment membership                                                                                                                                            |
| `TerminalSession`            | Terminal identity, lifecycle, committed effective configuration and its revision, one engine, and the rebuildable last-synchronized engine observation                                                                        |
| `TerminalEngine`             | PTY/process resources, Ghostty canonical state, content revision, generation, geometry, bell sequence, physical configuration application, input queue, attachment-scoped held-input cleanup ledgers, and failure observation |
| `AttachmentSession`          | Terminal binding, capabilities, demand, delivery flights, bell delivery cursor, history anchor, selection, and export progress                                                                                                |
| Local IPC                    | Endpoint security, accept/read/write/timeout behavior, and record framing                                                                                                                                                     |
| Native protocol adapter      | Lossless conversion between transport records and typed Host commands, replies, and events                                                                                                                                    |
| `@orchbee/terminal-protocol` | Wire schemas, codecs, message kinds, format versions, and protocol safety limits                                                                                                                                              |
| Desktop                      | Product binding, recovery decisions, escalation timing, platform effects, presentation, and updates                                                                                                                           |

In particular, terminal state and terminal runtime must not live in independent authoritative maps.
An attachment is owned by its broker session and refers to one terminal; a terminal must not keep a
second authoritative attachment registry. Terminal-scoped caches derived from active attachment
demand are disposable accelerators, not a second source of attachment truth.

The engine's held-input cleanup ledger is not an attachment registry: it contains no capability or
terminal binding and cannot authorize an operation. Core input-capable attachment admission creates
its empty slot in the same commit, and detach removes it before Core removes the capability. The ledger owns
only the exact key and application-mouse facts accepted for that attachment and the bounded repair
credit reserved for releasing them.

`EngineObservation` is likewise not terminal truth. It is a monotonic snapshot copied onto the Host
owner thread at explicit synchronization boundaries so Core can validate delivery and serialize a
coherent inventory summary without reaching through an untyped runtime map. It may be rebuilt from
the engine and must never advance beyond the engine fact it records. The committed configuration is
different: `TerminalSession` owns its protocol-visible value and revision. A native update prepares
all Core preconditions, applies the value to the engine, and then performs an infallible session
commit; failed physical application leaves the committed configuration unchanged.

The fair background synchronizer may process a bounded subset of retained terminals per poll. A
`ListTerminals` request therefore synchronizes the exact bounded page it will serialize after cursor
validation and before encoding it; one response never mixes an already observed page prefix with an
unsynchronized suffix merely because the global registry is larger than one poll budget.

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
    by_id: Map(TerminalId, TerminalSession),
    creation_index: Map(CreationId, CreationBinding),
    inventory_generation: InventoryGeneration,
};

const TerminalSession = struct {
    id: TerminalId,
    lifecycle: TerminalLifecycle,
    configuration: CommittedTerminalConfiguration,
    engine: TerminalEngine,
    observed_engine: EngineObservation, // derived, rebuildable
};

const BrokerSession = struct {
    id: BrokerSessionId,
    client_id: ClientId,
    attachments: Map(AttachmentId, AttachmentSession),
};

const AttachmentSession = struct {
    terminal_id: TerminalId,
    capabilities: AttachmentCapabilities,
    bell_cursor: DeliverySequence,
    projection: ProjectionState,
    interaction: InteractionState,
};
```

Identifiers for hosts, brokers, terminals, creations, attachments, requests, revisions, and
delivery sequences are distinct types. Conversion at a wire boundary is explicit. Zero or a
missing value is not silently reinterpreted as another state.

The production `TerminalEngine` has no empty variant: registry admission commits a complete native
engine with both identity indexes, and disposal or Host shutdown explicitly transfers that engine
out before destroying it. Deterministic Core tests may use a test-build-only model engine; that
variant is not part of a production build or a wire-visible state.

### Terminal lifecycle

```zig
const TerminalLifecycle = union(enum) {
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

A retained terminal is created atomically in `running`; partially constructed terminals are not
observable. `finishing` records exactly the first normal completion fact. `OutputClose` in this
state means normal PTY drain only. The complementary fact produces `settled.exited` only after the
wait worker has classified the completion path and the PTY writer is fully quiesced. An in-progress
write therefore still has an opportunity to become the terminal's typed failure before normal exit
can be committed. Process-observation or reap failure has the same precedence. Normal and failed
classifications are terminal alternatives; later disposal diagnostics cannot rewrite a retained
normal outcome.

Settlement is the terminal-owned mutation barrier. Once `settled.exited` or `settled.failed` is
committed, canonical content, generation, geometry, effective configuration, configuration
revision, and observed content revision are frozen at the captured final revision. Input, resize,
and configuration updates are rejected. Projection, history, selection, export, inventory, and
outcome publication may continue only as reads of that frozen terminal state; broker-scoped
attachment and delivery state may still advance independently. The pending outcome becomes
published exactly once and is never recreated by a later read or acknowledgement. Explicit
disposal is retryably rejected until the exact settled outcome handoff commits publication; that
rejection preserves the complete terminal session, engine, outcome token, and attachments.

On POSIX, an owned descendant may remain in the process containment domain after closing every PTY
handle. Root-child completion plus normal PTY drain is still the terminal's normal outcome in that
case; the retained process domain is force-contained during explicit disposal. Keeping the POSIX
session leader identity unreaped until then prevents session-identifier reuse from weakening
containment. Exit observation therefore requires a non-reaping platform observation; failure of
that observation is a typed runtime failure, never a fallback that reaps the leader before
containment.

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

Every newly held key or mouse button precharges its worst-case release bytes and one
future repair entry against the bounded client-input budget. Ordinary client input cannot consume
that credit. Detach or broker loss first marks only the affected ledger as detaching, rejects older
prepared mutations that have not committed, encodes its accepted key releases followed by its mouse
releases under the current Ghostty modes, and converts the precharged capacity into one FIFO repair
entry. Only after that commit removes the ledger does Core remove capability authority. A failure
to materialize or commit repair while the runtime is still live is an integrity failure that fences
the runtime and enters its containment path; natural exit or an already-fenced runtime needs no PTY
repair. Another attachment's ledger is never read, released, or removed by this transition.

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

- **Retained cleanup ownership**: the failed terminal transitions to `containing_failure.blocked`
  with the typed `contain_process` cleanup failure and a bounded retry deadline derived from the same
  Host-lifetime monotonic cleanup clock used by the broker-independent scheduler. The aggregate, the
  runtime, and the POSIX session identity are fully retained; containment was not confirmed, so the
  session leader identity is never released (the session-identity fence from THCR-006 holds).
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
  retained failed state (never-due retry), the observable status records `exhausted`, the failure is
  surfaced loudly, and teardown exits with the shutdown failure instead of pretending a clean
  release.

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
resize fencing, completion classification, the failure fence, and containment confirmation, and
remains held through the allocation-free outgoing-record and Core-flight commit. If failure
publication wins while containment is unconfirmed, cancelling the provisional adoption releases
its capacity and allocation without advancing the flight, sequence, projection cadence, scheduler
position, or visible outgoing batch. If the guard wins, exactly one frame commits before a failure
publisher can acquire the mutex. Confirmed containment may again admit retained read-only state;
resize failure never does.

History row delivery and selection export use the same principle: each flight has one positive,
strictly increasing sequence; only its exact acknowledgement can commit it. Stale or duplicate
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

The launch profile seeds Ghostty's default foreground, background, cursor color, and complete
palette. OSC 4, 10, 11, and 12 changes mutate that canonical state: projection resolves palette
cells to RGB and carries the current dynamic defaults, while queries produce terminal-reply bytes
from the same state. The maintained surface corpus covers mutation and projection, and the runtime
test covers exact pinned-Ghostty color-query replies. A separate real-PTY trace covers Host
terminal-reply routing and accounting without depending on a fixed-length color-query read. No
attachment or renderer owns a second terminal palette.

A Ghostty bell effect advances one terminal-owned positive monotonic `bell_sequence` exactly once
while the canonical terminal-state mutex is held. A newly admitted read-capable attachment starts
its `bell_cursor` at that current sequence, so attach, reconnect, and retained-terminal recovery do
not replay earlier audible intent. While the attachment exists, each later sequence is eligible for
one attachment-scoped `TerminalBell` event. The scheduler may hand off at most the next sequence in
one poll and advances the cursor only after the exact event record is appended to the successful
outgoing batch. Batch fullness or allocation failure leaves that sequence eligible for retry;
detachment or broker loss destroys the cursor and drops undelivered bell intent. Bell delivery has
no client acknowledgement and never mutates projection, history, or canonical screen content.

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
cursor, tab stops, palette-related terminal state, and interaction ownership to their reset values;
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

Creation reserves registry and reply capacity before spawning. If spawn succeeds but admission
cannot complete, the process tree is force-contained before the operation returns. Engine workers
may mutate only engine-owned queue or canonical VT state under the engine's synchronization. They
publish immutable lifecycle observations and do not mutate registry, lifecycle, broker, or
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

| Current state            | Input                                                  | Next state                         | Required effect                                                                                      |
| ------------------------ | ------------------------------------------------------ | ---------------------------------- | ---------------------------------------------------------------------------------------------------- |
| absent                   | valid create                                           | `running`                          | Prepare capacity, spawn exactly one runtime, then atomically admit both indexes and reply            |
| `running`                | child exits or output closes normally                  | `finishing(first)`                 | Retain exactly the first completion fact and reject new client input                                 |
| `finishing`              | complementary completion fact after writer quiescence  | `settled.exited`                   | Commit one immutable normal outcome                                                                  |
| `running` or `finishing` | unrecoverable runtime failure                          | `containing_failure.stopping`      | Fence I/O, record failure, and initiate force containment once; do not publish a settled outcome yet |
| `containing_failure`     | process tree and runtime resources confirmed contained | `settled.failed`                   | Retain the typed failure and observed or explicitly unavailable process result                       |
| `containing_failure`     | owner cleanup phase fails                              | `containing_failure.blocked`       | Retain the exact cleanup failure and schedule a broker-independent retry                             |
| `running` or `finishing` | graceful or force request                              | same lifecycle with updated intent | Perform each intent at most once; never dispose implicitly                                           |
| `settled`                | input, resize, configuration, or revision advancement  | unchanged                          | Reject the mutation; the captured final revision and terminal-owned visible state remain immutable   |
| `settled`                | dispose while outcome publication is pending           | unchanged                          | Reject retryably before runtime shutdown or any removal side effect                                  |
| `settled`                | dispose after outcome publication                      | absent                             | Release engine and indexes atomically after resource cleanup                                         |
| any retained state       | broker disconnect                                      | unchanged                          | Remove attachment capabilities only                                                                  |

### Attachment session

| Current state | Input                                                   | Next state                    | Required effect                                                                                                    |
| ------------- | ------------------------------------------------------- | ----------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| absent        | authorized attach with at least one capability          | projection `paused`           | Create exactly one broker-scoped capability binding; a resize lease requires resize capability                     |
| `paused`      | activate demand                                         | `ready.snapshot(base = null)` | Lazily acquire terminal render resources as needed                                                                 |
| `ready`       | provisional frame wins the serialized read-commit guard | `in_flight`                   | Commit its reserved outgoing record, exact frame, base, and sequence together                                      |
| `in_flight`   | exact applied acknowledgement                           | `ready`                       | Promote only the acknowledged base                                                                                 |
| `ready`       | capture cannot be represented within its semantic bound | `blocked`                     | Record the exact captured or attempted revision, which cannot exceed the current observation                       |
| `blocked`     | Core already observes a revision newer than the failure | `ready.snapshot`              | Lazily invalidate the failed capture and retry without requiring another terminal mutation                         |
| active state  | pause                                                   | `paused`                      | Abandon the flight and invalidate its delta base                                                                   |
| any state     | detach or broker loss                                   | absent                        | Queue attachment-owned key/mouse repair, then release pins, flights, selections, exports, and capability authority |

## Correctness invariants

Stable claim identifiers are used by tests, models, and review findings.

| Claim    | Invariant                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `TH-001` | Each retained `TerminalId` names exactly one `TerminalSession` and exactly one engine; no engine exists outside a creation transaction or retained session.                                                                                                                                                                                                                                                                                     |
| `TH-002` | Each mutable semantic fact has one authoritative owner; derived state cannot outlive or contradict that owner.                                                                                                                                                                                                                                                                                                                                  |
| `TH-003` | At most one broker session exists per endpoint, and every attachment operation is authorized by that physical session plus its `AttachmentId`. Accepted held input belongs to exactly that attachment; detach repairs and removes only its ledger before revoking authority.                                                                                                                                                                    |
| `TH-004` | Broker loss repairs and destroys only broker-scoped attachment state and capabilities; it never cross-releases another attachment or disposes a healthy retained terminal. An unrecoverable repair integrity failure uses the explicit runtime-failure containment path.                                                                                                                                                                        |
| `TH-005` | Creation is atomic and idempotent by `HostEpoch + CreationId + launch fingerprint` while the terminal is retained; explicit disposal releases that identity. Failures admit neither a terminal nor an orphan process.                                                                                                                                                                                                                           |
| `TH-006` | PTY output, input encoding, projection, history, and selection use one canonical Ghostty state and coherent revision/generation pair; a failed projection names its exact captured or attempted revision.                                                                                                                                                                                                                                       |
| `TH-007` | Accepted client input occupies exactly one client-FIFO position and atomically commits any held-input cleanup ownership; rejected input occupies neither. Held input precharges bounded detach-repair capacity. Terminal replies have independent bounded admission and priority within a bounded reply burst after which the writer yields to a waiting accepted client entry; an unknown client-input result is never automatically replayed. |
| `TH-008` | Normal exit is published only after child completion, normal output closure, and writer quiescence, irrespective of observation order. Settlement captures one final revision; terminal-owned content, generation, geometry, configuration, and revisions cannot advance afterward.                                                                                                                                                             |
| `TH-009` | An unrecoverable runtime failure is explicit, fences unsafe I/O, and cannot be reported as normal drain or normal exit. Unconfirmed containment admits no new frame handoff. Cleanup remains Host-owned and retryable without a broker; a settled outcome cannot be rewritten by a later cleanup symptom or removed before its exact publication commits, and disposal never silently orphans an uncontained process tree.                      |
| `TH-010` | An outgoing delivery transition commits if and only if its exact record is in the handed-off batch; batch fullness returns a committed prefix without dropping it. Projection record and flight commit are allocation-free while holding the engine read-commit guard. A handed-off frame record is immutable for its scoped sequence and is never reconstructed with different bytes.                                                          |
| `TH-011` | At most one frame, row request, or export chunk of each kind is in flight per attachment; only the exact scoped acknowledgement advances it. A published frame sequence identifies one immutable record for that Host epoch and attachment.                                                                                                                                                                                                     |
| `TH-012` | Bounds and backpressure reject or defer the current operation without corrupting retained registries, canonical terminal state, delivery bases, or sequence numbers.                                                                                                                                                                                                                                                                            |
| `TH-013` | Every successfully acquired resource has one release owner and is released at most once; a teardown pass attempts every retained runtime even when one attempt fails, and never reports failed cleanup as clean shutdown.                                                                                                                                                                                                                       |
| `TH-014` | All control JSON crosses one native protocol adapter, all binary data crosses its owning codec, and every registered role/encoding/success-kind/dispatch-operation association comes from the protocol registry; malformed, unknown, incompatible, mismatched, or unhandled records cannot reach domain transitions.                                                                                                                            |
| `TH-015` | Each Ghostty bell advances one terminal-owned sequence exactly once. A read-capable attachment receives each later sequence at most once, advances only with exact outgoing-batch append, and never replays a sequence that predates its admission or survives its destruction.                                                                                                                                                                 |
| `TH-016` | An input-authorized application clear or reset mutates only the Host-owned Ghostty state, advances generation and revision exactly once, invalidates dependent projection and interaction state coherently across attachments, never enters PTY input/reply queues, and is never automatically replayed after an unknown result.                                                                                                                |

## Scenario decisions

| Scenario                                          | Contract decision                                                                                                                              |
| ------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| Desktop restarts while terminals run              | Host retains terminals; the new broker handshakes, inventories, and creates new attachments.                                                   |
| A same-user process knows the endpoint            | It is inside the current trust boundary; application-level isolation is not promised.                                                          |
| A second broker connects while one is active      | It cannot become active concurrently.                                                                                                          |
| Protocol negotiation fails                        | Close that broker transport; retained terminals remain.                                                                                        |
| A response is lost after input admission          | Outcome is unknown; Desktop must not automatically replay the input.                                                                           |
| A scheduler batch fills                           | Return the prepared prefix normally and defer the next record.                                                                                 |
| A client observes a repeated frame sequence       | The already committed immutable frame is acknowledged idempotently from fixed-header metadata; it is not decoded or committed again.           |
| PTY read, write, reply, or resize integrity fails | Record a typed failure, fence I/O, confirm process-tree containment, then publish and retain the failed terminal for observation and disposal. |
| One runtime fails during Host shutdown            | Continue shutdown for all other runtimes, then report the aggregate failure.                                                                   |
| Host or operating system crashes                  | PTY continuity and recovery are not promised. Platform cleanup mechanisms still apply where the OS provides them.                              |

## Required correctness evidence

The implementation is not considered conformant until claims map to executable evidence. File
coverage alone is insufficient.

| Evidence                                                                       | Required coverage                                                                                                                                                                                                                                                                                                                                                                          |
| ------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| [Host registry TLA+ model](../formal/tla/TerminalHostRegistry.tla)             | `TH-001`, `TH-004`, `TH-005`, two-terminal identity uniqueness, atomic creation failure, broker-independent retention, settlement-gated disposal, and identity reuse after disposal; checked bounds live in its adjacent `.cfg`                                                                                                                                                            |
| [Terminal lifecycle TLA+ model](../formal/tla/TerminalHostLifecycle.tla)       | `TH-008`, `TH-009`, writer quiescence, both normal-completion orders, failure containment, cleanup retry, exact settled revision, mutation freeze, and settlement-gated disposal; checked bounds live in its adjacent `.cfg`                                                                                                                                                               |
| [Host shutdown TLA+ model](../formal/tla/TerminalHostShutdown.tla)             | `TH-013`, two-runtime teardown, failed-attempt retention, no short-circuit after one failure, and completion only after every retained runtime has been attempted; checked bounds live in its adjacent `.cfg`                                                                                                                                                                              |
| [Delivery TLA+ model](../formal/tla/TerminalHostDelivery.tla)                  | `TH-003`, `TH-010`, `TH-011`, batch prefixes, acknowledgement races, pause/detach, reconnect, and transport failure; checked bounds live in its adjacent `.cfg`                                                                                                                                                                                                                            |
| [Request-outcome TLA+ model](../formal/tla/TerminalHostRequestOutcome.tla)     | `TH-005`, `TH-007`, `TH-010`, `TH-016`, prepare/commit/handoff, retryable rejection, delivered reply, unknown result, idempotent replay, and forbidden non-replayable retry, including application clear/reset                                                                                                                                                                             |
| [Inventory TLA+ model](../formal/tla/TerminalInventoryPagination.tla)          | `TH-002`, `TH-012`, generation-scoped pagination, exact membership, duplicate prevention, and stale-cursor rejection                                                                                                                                                                                                                                                                       |
| [Frame-delivery TLA+ model](../formal/tla/TerminalFrameBackpressure.tla)       | `TH-006`, `TH-009`, `TH-010`, `TH-011`, `TH-012`, `TH-016`, two-attachment projection mutation/capture separation, generation-changing clear/reset, exact failed revisions, provisional frame commit, failure publication and containment confirmation, no unconfirmed-failure handoff, one flight per attachment, acknowledgement bases, pause/abandonment, and blocked snapshot recovery |
| [Input-backpressure TLA+ model](../formal/tla/TerminalInputBackpressure.tla)   | `TH-003`, `TH-004`, `TH-007`, `TH-012`, two-attachment held-input ownership, precharged detach repair without cross-release, independent client/reply admission budgets, client FIFO ownership, post-exit client closure, writer shutdown cleanup, terminal-reply priority within a bounded reply burst, and bounded client progress; checked bounds live in its adjacent `.cfg`           |
| [Process-identity TLA+ model](../formal/tla/TerminalHostProcessIdentity.tla)   | `TH-008`, `TH-009`, `TH-013`, completion sampling, containment confirmation, reap, SID rebinding, and request dispatch; every emitted process signal targets the retained launch identity and a rebound session never receives a signal; checked bounds live in its adjacent `.cfg`                                                                                                        |
| [History-interaction TLA+ model](../formal/tla/TerminalHistoryInteraction.tla) | `TH-002`, `TH-006`, `TH-010`, `TH-011`, `TH-012`, `TH-016`, row flights and anchors, generation-changing clear/reset invalidation, history observation, selection eviction, acknowledged export chunks, stale search pages, and detach cleanup                                                                                                                                             |
| Stateful property tests                                                        | `TH-001` through `TH-016` as applicable to each owner: registry/index coherence, lifecycle and terminal-control command sequences, attachment capability sequences, projection sequences, history/export sequences, and invariant checks after every action                                                                                                                                |
| Deterministic fault-injection tests                                            | Allocation, encoding, spawn, queue admission, resize, PTY I/O, process observation, reply handoff, partial batch, and multi-runtime shutdown failures at every commit seam                                                                                                                                                                                                                 |
| Native integration tests                                                       | POSIX session/process behavior, Windows Job/ConPTY behavior, endpoint ownership, peer identity, descriptor/handle cleanup, and worker joining                                                                                                                                                                                                                                              |
| Cross-language protocol conformance                                            | Every kind's role, encoding, and successful response; every native command, reply, event, identifier type, and declared union or error variant; every transport and binary format; both sides of every negotiated or hard bound; and each declared malformed-record rejection class                                                                                                        |
| Focused performance evidence                                                   | Bounded memory, no-demand behavior, fairness, large input backpressure, projection cadence, and history/export bounds without weakening semantic invariants                                                                                                                                                                                                                                |

Every TLA+ module header and stateful property-test suite must identify the transition vocabulary it
refines and the stable claim IDs it covers. Trace action names must use the transition vocabulary in
this document. Production changes that alter a state, owner, commit boundary, retry class, or
failure outcome must update this model and its evidence in the same change.

### Native evidence index

The following entries are the required executable refinements, not a claim that an unexecuted test
currently passes. A handoff may report conformance only after the package verification commands run
these entries successfully.

| Claims             | Required native evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `TH-001`, `TH-002` | `src/core/__tests__/state_test.zig`: `multi-terminal creation is idempotent without a product count cap`, `attachment routing and terminal disposal are exact across registries`, and all three stateful transition property suites; `src/service/__tests__/runtime_aggregate_test.zig`: `failed runtime creation leaves no retained terminal commit`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| `TH-003`, `TH-004` | `src/engine/__tests__/runtime_test.zig`: `attachment detach repairs only its accepted key and mouse ownership`, late prepared-input rejection, precharged repair under full client backpressure, and repair-allocation failure containment; `src/service/__tests__/service_test.zig`: `a second broker is rejected until the active broker disconnects` and `disconnect removes every broker attachment and preserves retained terminals`; `src/core/__tests__/state_test.zig`: `one broker owns every attachment and disconnect preserves terminals`; `src/transport/__tests__/root_test.zig`: failed-handoff preservation                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `TH-005`           | `src/service/__tests__/runtime_aggregate_test.zig`: creation replay and failed creation; `src/service/__tests__/service_test.zig`: real PTY disconnect/replay and isolated spawn failure; `src/engine/__tests__/runtime_test.zig`: startup-thread and process-spawn cleanup                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `TH-006`           | `src/engine/__tests__/runtime_test.zig`: canonical PTY parsing, semantic input, exact legacy terminal-query/reply PTY corpus, OSC color mutation/query replies, exact attempted projection revision, coherent primary/alternate metadata generation, and terminal-driven DECCOLM ordering, failure fencing, and attachment-resize serialization; `src/engine/__tests__/terminal_stream_test.zig`: scoped, chunk-safe DECRQSS restoration with non-target DCS exclusion; `src/core/__tests__/state_test.zig`: explicit failed-revision validation, rollback, lazy newer-observation refresh, and finishing/settled geometry rules; `src/service/__tests__/projection_test.zig`: both post-capture and stale-pre-capture revision races plus exact terminal-driven snapshot geometry; `src/service/__tests__/runtime_aggregate_test.zig`: projection-independent and configuration-adjacent full-geometry synchronization; `src/engine/__tests__/projection_test.zig`: bounded snapshot/delta capture; `src/engine/surface_fixture.zig` plus `scripts/run-surface-fixture-conformance.mjs`: byte-identical paired pinned-Ghostty surface corpus; `src/engine/__tests__/history_test.zig`: render/text rows, reflow preservation, screen/eviction invalidation, selection, and export |
| `TH-007`           | `src/engine/__tests__/runtime_test.zig`: source-scoped bounded queues, terminal-reply priority within a bounded reply burst, exact burst yield, reservation cancellation, client FIFO, zero-output, client-byte settlement, attachment-owned mutation at queue commit, stale prepared-input rejection, and precharged detach repair; `src/service/__tests__/runtime_aggregate_test.zig`: input preparation rollback; `src/service/__tests__/service_test.zig`: raw/semantic input failure equivalence and `InputAccepted` queue-admission-only semantics                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `TH-008`, `TH-009` | `src/model/__tests__/lifecycle_test.zig`: matching publication-gated disposal for both settled outcome kinds; `src/core/__tests__/state_test.zig`: both finishing orders, exact settled-revision checks, terminal outcome immutability, failure containment, and publication-gated removal; `src/engine/__tests__/runtime_test.zig`: atomic completion classification, full terminal-owned state freeze, and broker-independent worker containment; `src/service/__tests__/projection_test.zig`: both serialized frame-commit/failure-publication orders; `src/service/__tests__/service_test.zig`: one-shot settled outcome handoff, bounded-batch retention, native dispose retry, and rejected post-exit configuration; `src/service/__tests__/runtime_aggregate_test.zig`: completion synchronization, failed-runtime publication gating, exact cleanup phases, and fair blocked-cleanup retry selection; `src/transport/__tests__/root_test.zig`: brokerless maintenance across accept timeouts                                                                                                                                                                                                                                                                               |
| `TH-010`, `TH-011` | `src/service/__tests__/outgoing_test.zig`: prefix, reservation, provisional adoption cancellation, rollback, and allocation failure; `src/service/__tests__/projection_test.zig`: full-prefix delivery, exact failure rollback, scheduler preservation, and serialized read commit; `src/service/__tests__/interaction_test.zig`: full-prefix and allocator rollback; `src/service/__tests__/service_test.zig`: outcome handoff and scheduler fairness; `src/engine/__tests__/history_test.zig`: row/export sequence and ABA rejection                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `TH-012`           | All limit tests in `src/protocol/__tests__/root_test.zig`, `src/engine/__tests__/runtime_test.zig`, `src/engine/__tests__/projection_test.zig`, and `src/engine/__tests__/history_test.zig`; `src/service/__tests__/service_test.zig`: reply reservation failure; `src/__tests__/protocol_conformance_test.zig`: complete TypeScript/native limit equality                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `TH-013`           | `src/engine/__tests__/runtime_test.zig`: partial startup ownership and contained destruction; `src/platform/__tests__/process_test.zig`: failed exec, POSIX session containment, and Windows Job assignment; `src/service/__tests__/runtime_aggregate_test.zig`: `runtime destruction starts only after every shutdown succeeds`; `src/transport/__tests__/root_test.zig`: endpoint and connection cleanup                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `TH-014`           | `src/control/__tests__/decode_test.zig`, `src/control/__tests__/wire_test.zig`, `src/protocol/__tests__/root_test.zig` (including exhaustive registry-owned dispatch operations), and `src/__tests__/protocol_conformance_test.zig`; `src/service/__tests__/service_test.zig`: malformed, stale-epoch, unknown-optional, and role/encoding rejection before dispatch                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `TH-015`           | `src/engine/__tests__/runtime_test.zig`: Ghostty BEL effect counting; `src/service/__tests__/interaction_test.zig`: per-attachment baseline, ordered delivery, batch-full retry, and destruction behavior; Desktop and Web projected-terminal tests: exact scoped forwarding and client deduplication                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| `TH-016`           | `src/engine/__tests__/runtime_test.zig`: `application clear erases and homes the active screen without resetting metadata or modes`, `application reset returns alternate state to a fresh primary terminal`, and `application terminal controls reject counter exhaustion before mutation`; `src/service/__tests__/runtime_aggregate_test.zig`: input-capability authorization and exact Core observation synchronization; `src/__tests__/protocol_conformance_test.zig`: both control operations and their exact accepted reply                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |

The deterministic commit-seam set is: creation preparation/spawn/admission; control-reply
reservation; raw and semantic queue reservation/publication; attachment input-ledger commit and
detach-repair materialization/publication/failure containment; application terminal-control
validation and canonical mutation; resize effect and unsafe rollback;
projection capture/encoding, provisional payload adoption, and serialized read commit; row,
history, selection, and export provisional mutation; batch prefix/full behavior; lifecycle outcome
handoff; Local IPC partial write; process observation and containment; and multi-runtime shutdown.
A new fallible seam must be added to this set and receive a
named failure test before the implementation may rely on `catch unreachable` after that seam.

## Review rule

Review follows semantic boundaries rather than file order. For each transition, reviewers verify:

1. the authoritative owner and legal source state;
2. validation and reserved capacity before effects;
3. the external effect and its compensation or containment path;
4. one infallible domain commit;
5. reply/event handoff and unknown-result behavior;
6. resource cleanup for every failure edge; and
7. evidence for every affected claim ID.

A smaller file is useful only when it gives one of these responsibilities a clearer owner. Moving
code without removing duplicated authority, illegal state combinations, or cross-owner commits is
not a boundary improvement.
