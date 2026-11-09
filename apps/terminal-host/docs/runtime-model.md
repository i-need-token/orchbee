# Terminal Host Runtime Contract and Correctness Model

On Windows, absolute `.cmd`/`.bat` selections use the system directory’s `cmd.exe` with `/e:ON /v:OFF /d /c`. The private command-line adapter distinguishes script quoting from ordinary executable argv, protects percent literals and metacharacters, and rejects NUL/CR/LF parameters and encoded lines above 8191 bytes before creating a PTY. Canonical verbatim drive/UNC paths are converted only when component meaning is preserved; device paths, relative script paths and ambiguous verbatim components are refused. The script remains responsible for its own parameter expansion. Neither `ComSpec` nor `PATH` can replace the command processor. The same suspended launch, Job containment, exit observation and cleanup own the script and descendants; no wrapper supervisor is introduced.

A Windows launch's `currentDirectory` reaches the child through the same conversion: a canonical verbatim spelling, such as a plugin root, becomes the ordinary spelling when that names the same directory and fits the `MAX_PATH` current-directory limit, because the child inherits the spelling and `cmd.exe` refuses a verbatim directory as a UNC path. A spelling that cannot be shortened faithfully is passed unchanged. Relative executables resolve against the converted directory.

## Prompt-preserving clear

The required Boolean `preservePrompt` on `ClearTerminal` selects the operation. `preservePrompt: true` preserves the complete soft-wrapped cursor line on the primary screen, erases earlier history and other output, clears selection/images and restores the active viewport. Alternate screens fail with `terminal-clear-alternate-screen` before mutation. Explicit false selects erase/home semantics; ResetTerminal never accepts this field. The same runtime lock owns mutation and one generation/revision advance, preserving modes and metadata. Input-capable attachment authorization is required; no resize lease or synthetic child bytes are used.

## Earlier output before a restored child

`StageTerminalOutput` stores at most 1 MiB of earlier output in one slot on the requesting
connection's Host aggregate, admitted by `engine/output_seed.zig` (UTF-8 text, CR, LF and SGR
only). The next `CreateTerminal` on that connection takes the slot under the same aggregate lock
whether or not its creation id matches, so staged bytes never outlive one creation attempt; a
replayed creation frees them unused. A matching creation applies them to the canonical stream
before the child or its worker exists, then resets style, scrolls the drawn rows into history with
`ESC [ 22 J` and homes the cursor. The console host assumes a blank screen at the origin, so its
first output lands below the history. A later width change reflows the screen and can pull the
newest history rows, including the restart divider, back into it. The system ConHost repaints its
whole screen from the top after a resize and overwrites those rows; the pinned OpenConsole did not
in the Windows integration witness. Nothing reaches the PTY. The slot is freed with the aggregate.

## Scrollback editor capture

The staged Windows scrollback editor workflow starts with `captureScrollbackText`.
The Host copies every retained row of the active screen under one terminal lock,
joining soft wraps and preserving hard line breaks and Unicode. The capture owns
its UTF-8 storage plus the generation/revision observed under that same lock.
Subsequent output, clear, resize or runtime destruction cannot alter those bytes.
Selection, history demand, viewport, modes, metadata and child input remain unchanged.

The capture has a positive caller budget at most 32 MiB. Invalid budgets, allocation
failure and a formatted result beyond the budget fail whole; no tail truncation or
empty-success fallback is permitted. Both primary history and the active alternate
screen are readable, under the existing readable-state/failure fence. Storage is
released after the last consumer.

Host editor creation consumes the capture: an exclusively created document must
remain owned by the editor runtime through confirmed Job containment and thread
joining. This capture boundary alone does not establish public editor support;
upstream Worker authorization and live-Client overlay integration remain.
Document reservation owns an open canonical handle to the folder the Worker passed
and a time-stamped leaf name before creating a file. Materialization uses exclusive
creation and plain UTF-8; a taken name never grants ownership of the existing file and
moves the unwritten document to the next numbered name, which the editor argv follows.
Reserved → writing → ready → released state distinguishes a partial write from
a ready document, and a ready document is never rewritten. Cleanup failure retains
the path, directory handle and ownership for retry; handoff moves the sole owner.
No recursive deletion or transcript-selected path is permitted.

Editor creation must retain this state in an acknowledged deferred Host terminal
before materialization. Start failure retains it there; successful runtime startup
transfers it after all fallible startup steps. Confirmed Job containment and joined
workers precede document release, which precedes terminal Dispose acknowledgement.
Optional `CreateTerminal.captureSource` admits a Windows editor only with entirely
null launch geometry. It names one read-capable attachment and its exact terminal,
positive generation and canonical unsigned content revision (including zero). The
same-lock capture must match both fences; missing authority, unreadable state,
stale fences or capture failure refuse whole before terminal creation and file
effects. The complete source tuple discriminates the retained creation digest;
omission preserves ordinary creation identity and wire bytes. Matching replay
returns the existing editor identity without recapturing or rewriting a document.
The Host appends one literal generated document path to the validated editor argv.
Current-Client/focus and Worker identity admission remain upstream obligations.
Runtime preparation owns the immutable capture, reserved document and preallocated
argv with one literal document path. Runtime options borrow its document owner
until the last fallible process-spawn step succeeds, then move ownership before
opening worker startup gates. Failed validation, thread startup or process spawn
retains the preparation's document. Shutdown releases process identity after
containment and joining, then releases the document; failed deletion retains the
runtime with `cleanup-release-editor-document` and the cleanup-only
`editor-document/release-editor-document` pair.

An awaiting-start terminal may retain this document cleanup in the existing
retained-cleanup state without fabricating an exited child. Once deferred cleanup
starts, Start is denied even after cleanup completes. Maintenance retries the exact
deferred preparation, including fatal teardown and bounded exhaustion, and Dispose
removes it only after release succeeds. Other cleanup failures remain restricted to
settled terminal outcomes. This Host workflow is staged; it does not yet provide
the public CLI or live-Client editor overlay. Focused Windows filesystem
tests cover immutable UTF-8 and empty documents, literal paths containing spaces
and Unicode, exclusive collisions, sole-owner handoff, allocation rollback, and
blocked deletion followed by successful owner retry. The focused creation and
lifecycle tests in `editor_creation_test.zig`, `editor_document_lifecycle_test.zig`
and `editor_deferred_lifecycle_test.zig` additionally cover source authority and
digest fences, unchanged ordinary creation identity, capture retention after
source disposal, startup rollback, containment before release, deferred cleanup
retries and fatal-teardown exhaustion without inventing a child outcome.

Focused runtime tests establish capture consistency,
Unicode/wrap/retained-history behavior, exact byte limits and read purity. Full
repository and deployed Host/TUI acceptance remain final-stage work; TLA is excluded
by the User's standing instruction.

## Status and authority

This document is the authoritative runtime model for `terminal-host`. The accepted contract is
stable enough to govern implementation and review, while the application remains experimentally
versioned. Acceptance of the model does not by itself claim that the current implementation
conforms to every invariant; conformance requires the evidence defined below.

Authority is divided deliberately:

| Concern                                                                                 | Authority                                                                        |
| --------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| Host runtime ownership, lifecycle, delivery, retry, and concurrency semantics           | This document                                                                    |
| Operator-facing purpose, launcher usage, and development commands                       | `../README.md`                                                                   |
| Transport records, message schemas, codecs, format versions, and protocol safety limits | [`@orchbee/terminal-protocol`](../../../schemas/terminal/references/protocol.md) |
| Product topology, recovery policy, update policy, rendering, and platform effects       | Desktop                                                                          |
| Platform process, PTY, and endpoint behavior                                            | The owning native adapter plus its native tests                                  |

An implementation comment, source layout description, or protocol codec must not define a second
version of a fact owned here.

## Windows executable spelling

The Windows process adapter synthesizes `argv[0]` from the selected executable with backslash
separators. This prevents `cmd.exe` from interpreting forward slashes in its own executable path
as switches. Application-name candidate selection and the retained launch fingerprint keep their
original values; the adapter performs no additional lookup or fallback. Every supplied argument
in `argv[1..]` retains its original bytes and existing Windows quoting. No other platform changes.
The spelling conversion uses the spawn arena and finishes before PTY/process acquisition.

The native witness starts real ConPTY commands with backslash, forward-slash and mixed executable
spellings, requires literal slash-bearing output and exit code 37, and bounds the wait for Job
containment. The test owns and settles its Job and reader on success and failure. This establishes
process-adapter behavior, not full CLI/cohort acceptance or UNC/long-directory support.

## Terminal process queries

A read-capable attachment may request a fresh process sample for its exact retained terminal. The engine owns the process resources; brokers never inspect arbitrary PIDs, infer ownership from hooks, or substitute initial launch parameters for current facts. The query does not mutate terminal state, input, focus, or lifecycle. A disconnected attachment, deferred terminal, unavailable platform, exhausted bound, or OS query failure has an explicit failure result; it is never an empty successful sample.

On Windows, the engine enumerates only its retained Job Object (at most 256 processes) within a 250 ms budget, opens each candidate with limited query rights, and verifies membership on the opened handle before observing it. PID plus creation time identifies an observation, and every row carries both. The sample keeps these invariants:

- **No hidden member.** A member that denies limited query rights is an opaque row: no image, command line, or directory, and console `unknown`. Its creation time and parent come from one system process snapshot taken only for such members; the snapshot is read for those PIDs alone and discarded. A second Job enumeration then confirms each opaque PID is still a member, so a recycled PID never inherits a published identity.
- **Churn is not failure.** A member that exits between enumeration and reading is omitted because it is no longer a member; a process that joins later appears in the next sample. Job accounting can still count a member whose exit is under way after the Job's process list has dropped it, so the list, not the count, is the membership and that lag is not over-capacity. Membership change never fails the sample.
- **Anchor liveness.** Because every row, opaque or readable, carries PID and creation time, a consumer's anchor is live exactly when a row matches both.
- **Limited rights first.** Image, creation time, parent, command line (`ProcessCommandLineInformation`), and console come from `PROCESS_QUERY_LIMITED_INFORMATION`. Only the current directory, and the command line where the limited query is unavailable, read process memory; a denied read leaves that field absent. The engine never enables privileges.
- **Console attachment.** Each readable row compares its console host with the live root's: `attached` when equal, `detached` for another console or a GUI-subsystem program (which reports its creator rather than a console host), `none` for a console program started without a console, and `unknown` when unobservable or after root exit. A console program reports its creator until its console connection completes, so a just-started member can briefly read `detached`.

A request that sets `includeLaunchKind` also receives the launch kind the Host recorded when it started the root (see [prompt-gated input](#prompt-gated-input)); the kind comes from the launch record, never from the sampled command line.

Handles belong to the query, pin each process until the sample is published, and are closed on every path. The retained root process handle supplies the launch PID even after root exit. No persistent process cache is introduced.

Each process exposes PID, nullable parent PID, creation time, and nullable image path, command line, and current directory. UTF-16 strings are bounded to 4096 code units each and decoded strictly. Missing access, unsupported remote layout, invalid text, or a changing remote descriptor yields an absent optional field. Remote process parameters are untrusted diagnostic data, never shell syntax or authority. Native and WOW64 parameter layouts are isolated in the Windows adapter; unavailable layouts fail closed. The total encoded reply obeys the existing 64 KiB control bound; oversize results fail without truncating membership. A monotonic 250 ms sampling budget is checked between process observations; it does not promise to preempt a stuck kernel call.

The response is a sequential observation, not an atomic process-tree snapshot, foreground group, or input-delivery guarantee. Equal membership before and after cannot detect a process born and exited entirely between samples. Windows has no POSIX foreground process group; higher-level Agent detection must state its inference and reject missing or ambiguous evidence. Other platform adapters return unsupported; this work does not add macOS/Linux process inspection.

The principal operation is `sampleProcesses(allocator) -> owned sample | unavailable | changed | limit | unsupported`. The Host resolves read authority before entering it and releases the sample after response encoding. The actual consumers are `panel process-info` and subsequent Agent launch/input readiness guards through the existing Server/Worker viewer route. Verification covers empty and multi-process Jobs, escaped/unrelated PIDs, PID identity, nullable fields, remote descriptor bounds, query failures, changed membership, over-capacity, allocation cleanup, denied attachments, and an actual Windows shell/child lifecycle. No query proves that an Agent has rendered an interactive prompt.

## Purpose

Terminal Host is Orchbee's local authoritative terminal-execution service. For one active trusted
local broker per endpoint, it owns:

- retained terminal identities and their lifecycle;
- each terminal's PTY and process tree;
- the canonical Ghostty terminal state;
- terminal configuration and ordered input admission;
- attachment-scoped projection, history, selection, and export state; and
- the private local endpoint through which the active broker accesses those capabilities.

The Host exists so that terminal truth and terminal resources do not move into Electron, a
renderer, or a product-layout model. A broker disconnect or restart destroys connection-scoped
capabilities but does not dispose retained terminals.

## Boundary and non-goals

One endpoint admits at most one active broker session. Every connection to it opens with one
`OBHS` frame of the [connection handshake](../../../schemas/connection/README.md) before any OBTP
byte: only an admitted `worker-host` offer continues as the broker session, and a control request
gets one reply and closes. While the broker session runs, at most four further control connections
are served concurrently; they carry no attachment, terminal, or broker authority. An accepted
connection has 2 s to send its frame, on the dispatch thread and on every control lane alike; how
long the Host had been waiting for a connection never shortens that deadline. The endpoint's
operating-system ownership checks define the trust boundary: processes running as the same
effective user are in the same trust domain. The endpoint identifier is not a secret, and the Host
does not authenticate one same-user application against another. This is a single-user local
service, not a multi-tenant isolation boundary.

The Host does not own:

- Project, Workspace, Workbench, Tab, Panel, agent, or product-session models;
- concurrent independent brokers, same-user application isolation, or authorization policy;
- TCP, HTTP, WebSocket, TLS, remote authentication, or remote sharing services;
- product recovery choices, update timing, UI, rendering, platform clipboard contents or effects,
  external navigation, or localized product copy;
- search queries or match ranking; clients search bounded text-detail rows;
- durable PTY migration, Host-process crash recovery, or operating-system crash continuity; or
- a reusable native library API for another workspace.

Desktop may reconnect to a surviving Host process. If the Host or operating system crashes, the
contract does not promise that its PTYs or terminal processes can be recovered.

The current Desktop adapter remains one broker implementation. The unshipped Rust Worker is an
additive implementation of the same single-broker role: only after Server admission, it performs
same-user authentication, the `worker-host` offer, and the exact-version OBTP Hello, retains the
accepted Host epoch and negotiated limits, and couples Server and Host connection liveness. This
connection-only slice sends no terminal command, creates no PTY, and adds neither launch authority
nor a concurrent broker session.

The owned process tree is the operating-system containment domain established at launch: the
dedicated session on Linux and macOS, the retained direct-child process-reaper subtree on FreeBSD,
or the Windows kill-on-close Job. Containment must observe that domain as having no live members
before cleanup is committed. On Linux and macOS, a same-user child that deliberately escapes the
POSIX session also escapes this contract; preventing hostile same-user code from detaching is not
part of the accepted trust boundary. FreeBSD containment follows the reaper subtree rather than
session membership.

On Windows, the Job keeps every descendant and kill-on-close while the Terminal runs, so process
samples, Agent identity and the foreground query still observe the whole tree. The containment domain is
the Job members that hold the Terminal console: the root, every member whose console reads
`attached`, and every member whose console is `unknown`, opaque members included. Members that read
`detached` (another console or a GUI-subsystem program) or `none` are outside it, as is a GUI child
started by a console holder. The Terminal console is the live root's console host, learned once
the root has connected and pinned by a process handle, so its PID cannot name another console after
the root exits or the pseudoconsole closes; until it is learned, every member with a console reads
`unknown`. Force termination ends the console holders round by round, waiting at most 250 ms for
each round's exits, and completes at the first round that finds none. A holder that cannot be read
or ended individually, a failed round, or holders that still appear after four rounds fall back to
terminating the whole Job, after which containment again counts every member. Before the last Job
handle closes, the Host clears kill-on-close only when that fallback was never taken and a final
reading finds no holder; the remaining members then outlive the Terminal outside Orchbee ownership.
Any failure keeps kill-on-close, so closing the handle ends every member. A Host that exits without
that release, for example by crashing, still ends every member, GUI programs included.

The runtime's foreground query answers whether something other than the retained shell holds the
terminal. On POSIX, it holds the terminal when another process group owns the PTY foreground; on
Windows, when a kill-on-close Job member other than the retained shell holds the Terminal console,
so a GUI program started from the shell alone does not count. A failed console reading keeps the
Job-wide answer, and a failed platform query is unknown. Only prompt-time mouse recovery reads it,
and an active or unknown answer leaves application mouse state alone. It never changes lifecycle,
containment, or process ownership.

A Host with retained terminals remains available without a broker. A Host with neither a broker
nor a retained terminal may exit. The `drain` and `exit` control requests end it earlier, as
[Host lifecycle](#host-lifecycle) describes. A wire `ClientId` is diagnostic metadata, not connection
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

| Owner                        | Authoritative facts                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| ---------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `HostState`                  | Host epoch and process defaults                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| `TerminalRegistry`           | Terminal membership, creation-identity binding index, inventory generation, and the stable reference to each retained session                                                                                                                                                                                                                                                                                                                                                                                                        |
| `RetainedTerminal`           | The allocation and one-to-one binding of exactly one `TerminalSession` to either one deferred `RuntimePreparation` or one started `TerminalEngine`; stable retained state never owns neither or both                                                                                                                                                                                                                                                                                                                                 |
| Local IPC connection         | `awaiting_hello`/`active`/`closing`, negotiated protocol limits, transport lifetime, and the physical connection                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `BrokerSession`              | The active connection identifier, diagnostic `ClientId`, and attachment membership                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `TerminalSession`            | Terminal identity, lifecycle, committed effective configuration and its revision, committed complete runtime theme with operation/revision identity, immutable initial start geometry after start, and the rebuildable last-synchronized engine observation                                                                                                                                                                                                                                                                          |
| `TerminalEngine`             | PTY/process resources, Ghostty canonical state, content revision, generation, geometry, the terminal-scoped viewport restore target, current and default dynamic colors, color scheme, bell sequence, current progress, the current OSC 7501 program status root record, one bounded pending clipboard-write intent, one bounded FIFO of pending application signals, physical configuration/theme application, input queue, attachment-scoped held-input and focus-claim ledgers, terminal focus aggregate, and failure observation |
| `AttachmentSession`          | Terminal binding, immutable admitted capabilities, current input capability and monotonic capability revision, demand, delivery flights, bell delivery cursor, history anchor, selection, and export progress                                                                                                                                                                                                                                                                                                                        |
| Local IPC                    | Endpoint security, accept/read/write/timeout behavior, and record framing                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| Native protocol adapter      | Lossless conversion between transport records and typed Host commands, replies, and events                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `@orchbee/terminal-protocol` | Wire schemas, codecs, message kinds, format versions, and protocol safety limits                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| Desktop                      | Product binding, recovery decisions, escalation timing, platform effects, presentation, and updates                                                                                                                                                                                                                                                                                                                                                                                                                                  |

On Windows, Local IPC owns one overlapped listen, one persistent manual-reset
event, one byte of prefetch storage, and at most one overlapped readiness read
per pipe instance. The listen stays armed across accept timeouts, so a client
that connects between two accept waits is accepted by the next one, and
accepting reads nothing. A pipe server can impersonate its client only after
reading from the pipe, so the connection's first byte arrives through the
readiness wait, which keeps it only if the client runs as the Host's user; the
Host writes nothing to a connection before that check, and a mismatch closes
only that connection. Each control lane waits on its own pipe instance until the
dispatch thread stops the lanes, so an idle lane never wakes. From `idle`, a
wait on a checked connection first probes the pipe: queued bytes stay available
for a bulk framed read, while an empty pipe arms exactly one one-byte read and
enters `pending`. The 20-millisecond maintenance timeout leaves that same operation
pending; completion retains its byte and enters `prefetched`, and consuming the
byte returns to `idle`. Disconnect cancels and drains a pending operation before
clearing its state or reusing the pipe, and destruction closes the event only
after that drain, so bytes and completions cannot cross connections.

`platform/output_wake.zig` keeps a frame from waiting out that timeout. Each committed PTY reader
batch signals one process-wide wake, and the projection scheduler arms it for its next due time.
On Windows these are an auto-reset event and a high-resolution waitable timer that the readiness wait
includes; on POSIX they are a pipe in the service's `poll` set and a shorter `poll` timeout. A wake is
only a hint: the service loop owns every delivery decision, and the 20-millisecond timeout stays the
bound in a process without a wake.

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

Outcome publication has no such window. Every event poll inspects every retained terminal for a
pending outcome, so the next poll publishes the outcome of a terminal that a page read or
terminal-scoped command has already settled, whatever the registry size, and the publication
barrier on its disposal lasts only until that poll. One poll hands off at most 64 outcome notices;
a full batch resumes after its last notice, so the outcomes it left out lead the next poll and none
starves.

### Idle scrollback compression

Pinned Ghostty owns the page representation, opaque compression-activity token, and incremental
compression cursor. `TerminalEngine` owns the last observed Ghostty token, a terminal-mutex-owned
interaction serial, a 250-millisecond monotonic idle deadline, and the latest pending or dormant
scheduling outcome. Every successfully prepared non-empty client input and every committed PTY
reader batch or visible Kitty animation tick advances the interaction serial under the terminal
mutex. Service maintenance owns the disposable fair scan cursor and one process-wide 20-millisecond
opportunity cadence.

Each opportunity selects at most one retained started runtime in round-robin order. It attempts the
terminal mutex without waiting. Lock contention stutters without postponing an existing deadline.
An observed Ghostty-token change starts a fresh 250-millisecond idle interval. An observed
interaction-serial change restarts that interval only while compression is waiting or pending;
complete and unsupported work absorb the observation without waking. A due unchanged token and
serial permit exactly one Ghostty `compress(.incremental)` call. Pending work may continue only on a
later fair opportunity, complete work stays dormant until compression-relevant Ghostty activity,
and delayed polling never catches up in a burst. Unsupported targets become dormant after Ghostty
reports `unsupported`.

Ghostty currently compresses complete cold primary-screen scrollback pages that do not intersect the
editable viewport. Compression retains each virtual mapping and encoded bytes while decommitting the
raw physical backing on supported 64-bit Darwin and Linux targets; test builds simulate that
reclamation, and other production targets perform no compression. This changes storage
representation only: terminal revision, generation, geometry, logical rows, projection, history,
selection, and delivery identities are unchanged. It is broker-independent, so retained terminals
remain eligible, and its page-backing savings are not a whole-process RSS guarantee.

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
    environment_overlay_digest: EnvironmentOverlayDigest,
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

The durable launch fingerprint and the volatile environment-overlay digest are deliberately
separate. The overlay is target-normalized and order-independent, and only its digest is retained
with the exact creation generation; its values exist only while preparing and owning the runtime
process environment and are never inventory or diagnostic data. A non-empty create replay must
match the retained digest. An empty replay is a wildcard only for an already retained creation, so
Desktop can validate durable recovery identity without reconstructing a retired capability; an
absent creation with an empty overlay simply creates a runtime with no overlay.

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
because XNU may drain the process reference before the child becomes waitable. While waiting on a
successful registration, the worker also performs a non-reaping observation at a bounded interval
so a child cannot leave it waiting forever when XNU loses `NOTE_EXIT` across a process-reference
drain race. An audit timeout closes that kqueue before the observation, allowing its process
reference to drain; a child that is still running is registered again. A transient
`CLD_STOPPED` can never become child completion. Observation failure is a typed runtime failure,
never a fallback that reaps the leader before containment. Final identity release uses
`waitpid(WNOHANG)`. Without a prior exact exit observation, `EAGAIN` returns immediately and the
identity fence remains retained. After an exact non-reaping exit observation, the process adapter
retries only `EAGAIN` within a fixed bound while holding that fence, bridging the narrow macOS
interval in which `waitid` has reported exit but `waitpid` cannot reap yet; exhaustion still retains
the fence for a later containment, observation, and reap attempt.

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
captured reference. A permanent error takes precedence over an invalidated-reference retry from
the same scan, so a later scan cannot hide a permission, resource or unknown failure. On Linux,
the native signal adapter preserves the explicit `EAGAIN` invalidation result and makes at most
eight fresh whole-session scans under the original identity fence. Each failed scan releases all
captured references and its iterator before another scan binds members and checks membership
again. The eight-pass limit bounds retries through transient exit or identity churn; it adds no
sleep, deadline, thread or ownership state. Only a complete successful scan returns success;
invalidation exhaustion remains a failure and neither an old reference nor its reused numeric PID
can authorize an effect. Other native errors, macOS/FreeBSD behavior and containment queries keep
their existing failure semantics. A partial force KILL whose retries fail still enters a complete CONT compensation scan; on
FreeBSD, a successful `PROC_REAP_KILL` call with `rk_fpid` set is also a partial failure.

The THCR-006 Host termination barrier captures the pending runtime before dispatch, then completes
that runtime's shutdown before resuming the admitted graceful or force request. Normal completion
alone is not an identity-release barrier: a descendant without PTY handles may still require owned
containment, as covered by THCR-028. The barrier checks actual identity release and verifies that
the resumed request issues no additional signals, commits the normal exit, and replays idempotently.

Windows declares the complementary drain fact only after no member of its kill-on-close Job holds
the Terminal console. Closing a pseudoconsole while an attached descendant remains can signal that
descendant and can block indefinitely on supported Windows versions, so the absence of console
holders is part of the Windows adapter's safe-drain condition; members without the console are not
attached and do not delay it. This changes when the shared lifecycle may leave `finishing`; it
does not create a second lifecycle or permit an unsafe normal classification. The adapter observes
Job accounting at a quiet monotonic cadence until the active count reaches zero or two consecutive
readings find no console holder, because a console program reports its creator until its console
connection completes. While holders remain, it re-reads console ownership when accounting changes
and at least once per second. Once containment or force termination has observed no holder, a
holder or a failed reading here is a race and falls back to terminating the whole Job instead of
waiting. Force termination can miss a console program its holder started just before ending: the
program still reports that creator, reads as detached, and becomes a holder only when it connects.
Natural process exit does not signal the Job handle. A signaled handle after force termination
cannot bypass that predicate or turn its retry into a busy loop.

The Windows Process owner keeps one successful root-termination submission receipt, initially
absent. A short per-Process mutex serializes termination syscalls and receipt publication; it is
not held during blocking waits, joins or PTY closure, and it acquires no other lifecycle lock;
console-holder rounds and their bounded exit waits run without it.
Containment may hold its existing outer mutex while invoking force termination; the lock order is
containment then termination, never the reverse. Successful graceful
root termination or successful force termination of the console holders or the Job publishes the
receipt before returning.
A later graceful request is then an idempotent no-op even while Windows is still completing exit.
Force requests always still target the console holders, or the whole Job after the fallback,
including after a graceful receipt, because descendants may remain. A failed syscall never creates a
receipt. Without one, a failed graceful root termination is
ignored only when a zero-time observation of the retained process handle proves exit, or when the
native status proves the root's exit already under way (`STATUS_PROCESS_IS_TERMINATING`, which
Win32 reports as `ACCESS_DENIED`), as after a concurrent force whose console-holder round has not
yet seen the root signal. A failed
force termination falls back to Job termination, whose failure always remains an error, even if the
root has exited. Exit status 259 is not a
liveness test. The receipt proves accepted termination submission only: actual child exit, Job
emptiness, PTY EOF and containment retain their existing independent observations.

Windows termination permission refusal is the typed `AccessDenied` failure, not an
unexpected-error diagnostic; it publishes no submission receipt and keeps the retained
process/Job owner available for the existing explicit retry. A Windows PTY write returning
`BROKEN_PIPE` or `NO_DATA` is the existing typed `BrokenPipe` failure. The writer applies
its ordinary PTY failure/containment path; classification neither invents accepted input
nor changes the independent child-exit and Job-empty observations.

The Windows retained-descendant termination witness uses a test-only child with explicit
begin/ready kernel events and no natural exit timer. Job membership is observed before releasing
the begin event, proving that a process count alone is not a readiness acknowledgment. Only after
the child signals ready does the test terminate the root, require a retained descendant, then force
termination and require complete containment. The test's Job owns cleanup on every path; event handles
are released after the owned processes stop. This fixture is a test-build dependency, never installed.

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

ANSI linefeed mode `20` applies at the Host-owned PTY-write preparation boundary. While it is
enabled, every literal carriage return in client input or a terminal-generated reply is expanded
exactly once to carriage-return/line-feed before the queue reservation is charged. The expanded
length, rather than the source length, must satisfy the negotiated semantic-input limit and the
appropriate bounded queue budget. Preparation observes the canonical mode while holding
`terminal_mutex`; a later mode change does not rewrite an already prepared FIFO entry.

Semantic key and application-mouse input commit their attachment-owned held-state mutation under
the input mutex at the same linearization point that publishes their client FIFO entry. Preparation,
cancellation, rejection, and a late commit after detach mutate neither ledger nor queue. Key facts
retain the exact accepted text and modifiers; mouse facts retain press order and the latest accepted
sample. A wheel report has an empty pressed-button set and is never retained. A semantic key tap is
an ordered press and release, so it has no steady-state ownership while still remaining repairable
if attachment loss lands between the pair.

Semantic mouse preparation also captures one positive terminal-owned mouse-input epoch while
holding `terminal_mutex`. Every accepted mouse event-mode or mouse format-mode action, full reset,
and confirmed local-shell prompt recovery advances that epoch under `terminal -> input`. Final
commit takes `terminal -> input` for mouse input and rejects a
different epoch as `input-mode-changed`, including a zero-byte encoding. Advancing the epoch never
rewrites an accepted ordered input entry. Epoch exhaustion is an unrecoverable worker-integrity
failure rather than a wraparound that could admit an ABA match.

Only a buttonless semantic mouse-motion entry is classified as transient hover. Its accepted bytes
retain their client-queue position until the writer selects them. At the write-start linearization
point, an unstarted hover whose epoch is no longer current is removed, freed, and fully uncharged;
the writer then selects again. Writer start orders this check under `terminal -> input`, so a Ghostty
mode mutation and its epoch advance cannot expose an intermediate start window. Key, text, paste,
focus, mouse press/release, drag, wheel, attachment repair, raw client input, and terminal replies
are never retired by this rule. Once any byte of an entry is authorized to start, every partial
write of that entry completes under its original bytes even if the epoch changes, so the Host never
truncates a control sequence. Surviving client entries keep their relative FIFO order.

OSC 133 new-prompt markers (`A` / `fresh_line_new_prompt` and `P` / `prompt_start`) provide a narrow
recovery seam for a child that disappears without disabling mouse tracking, such as a disconnected
SSH client. The Host samples the retained platform process synchronously without waiting. Only a
successful observation that no foreground command remains may clear all canonical mouse event
modes, restore the X10 mouse format, and advance the epoch. An active foreground command preserves
the modes so a prompt emitted inside Zellij, SSH, or another nested shell cannot reset its owner;
an observation failure also preserves them. Parser order remains authoritative: a later mouse-mode
action in the same reader batch wins.
Without OSC 133 integration, prompt recovery is unavailable and ordinary child-emitted mode reset
remains the only automatic recovery path.

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
active projection demand and input capability. An input-capable attachment without read capability
holds demand for this purpose alone: it delivers no projection and does not count toward the
terminal's active read demand, so a broker that splits presentation and input across two attachments
keeps their demand equal. The Engine OR-aggregates all eligible claims for the
terminal: the first claim emits one semantic `FocusIn`, later claims emit no PTY bytes, non-final
releases emit no PTY bytes, and the final release emits one semantic `FocusOut`. Replaying the
attachment's current claim is inert. Paused, disabled, detached, or stale-revision input cannot
create a claim. Activating demand or upgrading input capability creates no claim and replays no
focus; a fresh exact-revision focus input is required. Enabling DEC mode `1004` additionally admits
one immediate report of the current aggregate through the terminal-reply budget. The mode update,
aggregate observation, and reply reservation are ordered under `terminal -> input`; prepared focus
input records the reporting mode it observed and a conflicting final commit is rejected as
`input-mode-changed`, so an enable action cannot overtake an unreported focus transition.

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

Automatic paste (kind 39 `PasteAutoInput`) carries no mode expectation: preparation reads the live
bracketed-paste mode once under the canonical terminal mutex, encodes with the shared Ghostty paste
encoder — the same unsafe-byte strip set, optional twelve-byte bracketed wrapper, and non-bracketed
LF-to-CR conversion — then applies the runtime linefeed expansion on the encoded bytes. Because the
encoded bytes are snapshotted at preparation, a mode flip between prepare and commit can never
change them; commit runs later under the input-admission lock, re-authorizes the attachment, and
publishes the prepared bytes exactly once. There is no `input-mode-changed` outcome on this path.
The acknowledged `accepted_bytes` counts encoded bytes admitted to the client-input queue, at most
`2 × content + 12`; it is not a PTY write completion.

The client-byte and client-entry limits cannot consume the bounded terminal-reply byte and entry
budgets. Exhausting a client budget is ordinary retryable backpressure and never becomes a terminal
failure. Exhausting the independent reply budget means the child generated replies faster than the
bounded writer could carry them; that is the actual `terminal_reply` integrity failure, which uses
the normal fencing and containment path. A prepared client input reserves its FIFO position and
budget before reply preparation; cancelling it releases both and admits no bytes.

Ghostty's terminal handler remains the only terminal-state mutator. The Host-owned stream adapter
delegates every ordinary parser action and effect to that handler and intercepts the bounded
`DCS $ q` and `DCS + q` query families because the pinned allocation-free
`ghostty_vt.TerminalStream` intentionally ignores DCS actions. Ghostty's DCS handler classifies the
payload. The Host encodes DECRPSS from the same canonical SGR, cursor, and margin state, and answers
XTGETTCAP from a reviewed fixed subset of the advertised `TERM=xterm-256color` identity: terminal
name, color counts, direct-color width, and the baseline navigation and F1-F12 key strings. One
request produces one ordered response and every name and value is uppercase hexadecimal. An invalid
or unsupported first name returns `DCS 0 + r ST`; a later invalid name ends the list and preserves
the preceding semicolon-separated `DCS 1 + r ... ST` prefix, matching xterm's multi-name behavior.
The adapter does not enable XTSETTCAP, tmux control mode, or a second VT owner.
Its APC handler disables Glyph Protocol recognition: support, query, registration, and clear
requests produce neither a reply nor glossary mutation until an end-to-end projected renderer and
resource contract are accepted. A retained source-independent corpus fixes the exact ordered bytes
for DSR, primary/secondary/tertiary DA, XTWINOPS, color-scheme, XTVERSION, ENQ, DECRQSS,
XTGETTCAP, disabled Glyph requests, malformed boundaries, and parser chunking.

### Pinned Ghostty termio/effect inventory

This inventory is maintained against pinned Ghostty commit
`9c3ec931d64561a8407dde7ac984ce156ae91539`. A Ghostty pin change must re-audit
`src/termio/stream_handler.zig` and the embedded `TerminalStream.Handler` effect interface before
the Host claims compatibility. “Host effect” means the consequence is owned and bounded in the
native runtime; “renderer/application effect” means canonical state crosses the protocol before a
product owner acts; the other dispositions are intentionally not advertised as capabilities.

| Upstream effect family                                                                                                    | Disposition                                    | Orchbee contract                                                                                                                                                                                                                                                                              |
| ------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Ordinary VT mutations; mode and Kitty-keyboard queries; DA, DSR, XTWINOPS, color-scheme/color, XTVERSION, and ENQ replies | Host effect                                    | The pinned terminal handler remains the sole state mutator. Profile, canonical state, and complete grid-and-cell-pixel geometry feed the bounded terminal-reply queue; reply admission failure uses typed `terminal_reply` containment.                                                       |
| BEL plus OSC title and current-directory metadata                                                                         | Host effect                                    | BEL advances one terminal sequence. Title and raw current-directory metadata remain canonical terminal facts and are revisioned for projection; product presentation does not become a second owner.                                                                                          |
| OSC 133 semantic prompts and OSC 9;4 progress                                                                             | Host effect                                    | Semantic prompt state drives bounded completion feedback; progress is one bounded current projection sample and is not generic notification authority.                                                                                                                                        |
| OSC 7501 program status                                                                                                   | Host effect beside Ghostty                     | The query is answered in stream order; the root record's state and receipt time are one bounded current projection fact, not Agent or lifecycle authority.                                                                                                                                    |
| Modes `20`, `40`, `1004`, `2026`, and `2048`, plus DEC 80/132-column mode `3`                                             | Host effect                                    | Input expansion, immediate aggregate focus reporting, synchronized-projection release, exact size reports, DECCOLM, and viewport restoration are serialized with canonical state, PTY effects, and reply admission as specified by `TH-022`.                                                  |
| DECRQSS and the reviewed `xterm-256color` XTGETTCAP subset                                                                | Host effect                                    | The Host-owned DCS adapter supplies bounded, ordered, source-independent replies. It never installs child-supplied capabilities.                                                                                                                                                              |
| OSC 8 hyperlinks and canonical color/palette changes                                                                      | Renderer/application effect                    | Hyperlink and color facts are projected from canonical state. Link activation and browser painting remain scoped application effects; child text does not grant local-file authority.                                                                                                         |
| Bounded Kitty graphics                                                                                                    | Renderer/application effect (bounded)          | Ghostty owns direct/file/temporary/shared-memory media, animation, virtual/relative placement, crop, query/delete, and signed-z state. Host resolves those relationships and projects only owned decoded pixels and absolute placements; source capabilities never cross the native boundary. |
| OSC 52 selection-kind `c` plain-text write                                                                                | Host effect plus application effect            | The Host publishes at most one bounded write intent; one selected readable attachment applies it. Query/read, other selection kinds, binary data, and unbounded content are deliberate product and security boundaries.                                                                       |
| Private APC prefix `orchbee.agent.v1;`                                                                                    | Host transport plus application interpretation | The Host transactionally publishes a bounded FIFO of decoded UTF-8 application signals. It does not authenticate the emitter, infer Agent/provider semantics, or request a notification; the application validates any narrower payload meaning.                                              |
| OSC 52 read, public notification channels, and OSC 22 pointer shape                                                       | Deliberate product boundary                    | OSC 9, OSC 777, OSC 99, OSC 1337, and related channels require permission, rate-limit, focus, precedence, and multi-client effect-owner decisions. They are neither silently promoted from canonical state nor advertised as current Host capabilities.                                       |
| Glyph Protocol, XTSETTCAP, and tmux control mode                                                                          | Unsupported capability                         | Glyph recognition is disabled before parsing and cannot reply or retain glossary resources. XTSETTCAP and tmux control mode are not enabled; ordinary tmux operation as a terminal application remains supported.                                                                             |
| Title-stack push/pop and Ghostty-native shell, renderer, window, notification, updater, or remote helpers                 | Unsupported or outside this boundary           | The pinned Ghostty handler itself leaves title push/pop unimplemented. Native Ghostty application integrations are not terminal-emulation capabilities and require their own Orchbee product contracts if adopted.                                                                            |

### Bounded real-application maintenance matrix

The latest local maintenance run replaces, rather than accumulates, the observation column. On
2026-08-23, macOS 14.8.7 used the advertised `TERM=xterm-256color` identity and an allocated PTY.
`PASS` means the program reached and left its expected terminal surface without startup,
terminfo, or geometry failure; it is discovery evidence, not a portable CI gate or a claim of
browser-pixel parity. A `TERM`, XTGETTCAP, parser, mode-effect, or Ghostty-pin change must rerun the
applicable rows alongside the Host's source-independent corpus and real-PTY integration tests.

| Surface       | Bounded probe                                                                                          | 2026-08-23 observation                                                       | Evidence boundary                                                                    |
| ------------- | ------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| Terminfo      | `infocmp -1 xterm-256color`; `tput colors`; `tput pairs`                                               | `PASS`: system entry resolved; `256` colors and `32767` pairs                | Confirms the advertised local identity, not every optional escape sequence           |
| VT diagnostic | `vttest -V`; open its main menu in a PTY and select Exit                                               | `PASS`: vttest 2.7 (20251205) entered and exited                             | Startup smoke only; the retained compatibility corpus owns deterministic query bytes |
| Shells        | Start clean interactive Zsh and Bash sessions; print `$TERM`; exit                                     | `PASS`: Zsh 5.9 and Bash 3.2.57 both retained `xterm-256color`               | Does not claim automatic shell-integration injection                                 |
| Multiplexer   | Start a detached 90x30 tmux session, capture the pane geometry and child `$TERM`, then stop the server | `PASS`: tmux 3.7b reported `90x30` and supplied `tmux-256color` to its child | Normal terminal application path only; tmux control mode remains unsupported         |
| Editors       | Open Vim and Neovim in a PTY, query `term`, and exit                                                   | `PASS`: Vim 9.1 and Neovim 0.12.4 both observed `xterm-256color`             | Startup/full-screen capability smoke; editing behavior remains application-owned     |
| Pager         | Open `/etc/services` in `less` and quit                                                                | `PASS`: less 581.2 entered and exited                                        | Bounded alternate-screen/input smoke                                                 |
| Modern TUI    | Start btop in a PTY and quit                                                                           | `PASS`: btop 1.4.7 entered and exited                                        | Bounded modern-TUI smoke; graphics-protocol support is not inferred                  |

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

DEC synchronized-output mode `2026` suppresses live projection capture, not canonical parsing,
revision advancement, history mutation, or terminal replies. Enabling or re-enabling the mode arms
a one-second deadline in the Engine's monotonic clock domain. An explicit reset, RIS or application
reset, a successful geometry change, or deadline expiry clears both the canonical mode and the
deadline; the next eligible scheduler pass may publish only the newest coherent revision. If a
monotonic sample or deadline addition is unavailable, the Host clears the mode immediately rather
than permitting an unbounded presentation freeze. No attachment owns a separate synchronized
region, and pausing or attaching a client does not extend the child-owned deadline.

History row delivery and plain-text selection export use the same principle: each flight has one
positive, strictly increasing sequence; only its exact acknowledgement can commit it. The control
boundary rejects HTML and raw VT export values before runtime access. Stale or duplicate
acknowledgements cannot advance a newer flight. An exact row acknowledgement replay and a stale
row acknowledgement at or below the attachment's issued high-water mark after a demand, screen, or
generation transition are idempotent no-ops; neither can clear a newer pending window or promote an
anchor. A tuple that misidentifies the current pending window, or a sequence beyond the issued
high-water mark, is rejected. Pausing projection or destroying the attachment abandons its flight
and invalidates its delta base without changing canonical terminal state.

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

Live and render-history projection use Ghostty's dynamic-color override provenance. An
otherwise-default foreground or background becomes explicit RGB only while that channel has a
program override; equality with the configured RGB never removes that provenance. Untouched or
reset channels retain the existing default sentinel. DECSCNM projects both already-swapped
effective defaults as RGB while active. Explicit SGR, resolved palette, erased-cell background,
and underline colors keep their existing priority, and cell inverse remains a style flag.
The live encoder captures the encoded default pair under the canonical terminal lock and reuses
it in both encoding passes. Any pair change, including an equal-RGB override set or reset,
invalidates every captured row for the next delta. Release or capture failure discards this
derived pair with the other capture resources. Render-history reads derive the same colors at
their existing capture boundary without moving the retained anchor. Header RGB and color-query
responses remain canonical terminal facts; no Client host-color query or wire field is added.

The same corpus carries an OSC 8 target across its soft wrap and crosses OSC 133 prompt, input, and
output boundaries in its final row. Host encoding publishes the Ghostty-owned hyperlink, cell
semantic content, and row prompt class without parsing or inferring them in an attachment. The
terminal package consumes those fields for bounded link and semantic workflows; application policy
remains the only authority that may perform an external open effect.

Projected soft-wrap topology has the same single owner. Every frame, merged delta, and row window
joins two rows exactly where the canonical page row above wraps, as Ghostty selection and reflow
do. Some Ghostty edits leave the lower half of such a pair disagreeing: IL or DL inside a wrapped
line drops a continuation below a wrap, ED 1 keeps one below a cleared row, and EL 2 on a row that
starts with a wrapped wide character keeps the spacer head above it. Host encoders therefore derive
each row's continuation from the row above instead of copying Ghostty's continuation flag, and keep
a trailing spacer head only on a wrapped row whose next row starts with the wide character; any
other spacer head is projected as a narrow blank. Ghostty also rewrites a clean neighbouring row
without marking it dirty: EL 0, EL 2, ECH, and DCH clear the next row's continuation, overwriting or
splitting a wrapped wide character rewrites the previous row's spacer head, and OSC 133 changes a
row's prompt class. Each live capture therefore compares every row's page metadata and final cell
with the incremental render state and rebuilds each row that differs. A row whose projected
topology changes advances its row revision, so a delta carries both halves of every changed pair
and a merged client grid matches a fresh snapshot of the same revision.

If a client nevertheless rejects a frame body, `NeedSnapshot(S, decoder-rejected)` abandons exact
flight `S` and also discards the terminal's incremental projection, so snapshot `S+1` is captured
afresh from canonical Ghostty state instead of re-encoding the capture that the client could not
accept. The other snapshot reasons describe client delivery state and keep the resident projection.

Captures of one terminal share a 16-millisecond sustained cadence with a burst allowance of two
intervals. A terminal quiet for that long may capture three times at once before the cadence applies,
so a program that answers one key in a few separate writes shows the last of them immediately, while
continuous output settles to 60 captures a second. Only a capture spends this budget: another reader
of the current revision costs one encode, so a sampling viewer cannot delay the next echo.

A capture also waits up to 2 milliseconds for a paint the program has only begun. ConPTY hides the
cursor, writes each paint in several pieces and shows the cursor again, so a capture after the first
piece shows only a missing cursor and pushes the text to a later frame. Only a reader batch that takes
the cursor from visible to hidden opens this wait, and showing the cursor ends it at once, so a program
that keeps its cursor hidden never pays for it.

The read-commit guard takes `projection_mutex` then `terminal_mutex`, revalidates generation and
failure-fence epoch, and holds both locks through allocation-free outgoing and Core-flight commit,
so a frame encoded before either changed is rejected without consuming its sequence.

### Kitty graphics projection model

Kitty graphics stay inside the same terminal-state and projection transaction. The problem being
closed is the former projection gap that discarded file-backed media, temporary-file media, shared
memory, Unicode-placeholder and relative placements, negative-z composition, animation, and
historical render images after pinned Ghostty had accepted them. Inputs are the untrusted PTY byte
stream, active-screen and scrollback geometry, a Host monotonic timestamp, an optional requested
history interval, and the declared resource limits. The reference scope is the complete Kitty
graphics feature set implemented by Ghostty revision
`9c3ec931d64561a8407dde7ac984ce156ae91539`, not unspecified future Ghostty behavior.

Ghostty image storage is the sole canonical owner of decoded images, animation frames, placement
relationships, and protocol replies. Host owns the Wuffs decoder installation, its temporary
directory lifetime, the monotonic animation deadline, and each immutable resolved capture. It
enables direct, regular-file, temporary-file, and, where available, POSIX shared-memory loading;
raw and PNG data, zlib compression, chunking, transmit/display/query/delete, frame composition,
animation control, virtual placements, and relative placements remain native Ghostty transitions.
The loader's Ghostty-matching 400 MiB acquisition bound and the Host's per-screen 32 MiB
decoded-storage bound remain finite and independent. Host never serializes a source path, open file, temporary-file handle,
shared-memory name or mapping, PNG container, or compressed stream. Only Host-owned decoded pixel
bytes and fully resolved placement values can cross the native boundary.

Capture executes `advance -> resolve -> encode -> commit`. While holding the terminal-state lock,
Host first advances any due animation, then resolves pinned, virtual, and relative placements to
cropped pixel rectangles and absolute cell/pixel geometry, and finally sorts images and placements
into deterministic wire order. OBKG preserves signed z values and divides composition into exactly
three ordered layers: `z < -1073741824` is below cell backgrounds,
`-1073741824 <= z < 0` is above cell backgrounds but below text, and `z >= 0` is above text. Both
Canvas2D and WebGPU consumers paint those layers at the corresponding renderer seams; neither
backend reparses Kitty commands or resolves Ghostty pins.

Live OBTF graphics extension kind `2` carries either a complete image-resource snapshot plus the
current resolved placements or a placement-only replacement naming the exact previously committed
image revision. A terminal snapshot and every changed image generation require the complete form.
A delta is placement-only only when its acknowledged base is at or after the capture revision that
installed the identical visible image set; otherwise the whole terminal frame is promoted to a
snapshot. A render-detail OBRW carries an independent complete graphics snapshot for placements
intersecting its selected rows, rebased to that window and captured at request time; text-detail
OBRW carries no graphics. History is a render snapshot, not restoration of earlier VT or animation
state. Image retransmission, visibility change, delete, reset, screen change, history eviction, and
reconnect cannot retain or resurrect stale client resources.

The projection scheduler calls Ghostty `animationTick` under the terminal-state lock using one
monotonic clock domain and retains the returned next-delay deadline per terminal. It compares the
Ghostty graphics generation before and after the tick. A tick that changes one or more displayed
frames advances the terminal revision exactly once and invalidates eligible readers; a tick with no
display change advances no revision. A future deadline schedules a wake without a PTY byte. A
committed PTY batch wakes this scheduler only when the active screen or Kitty resource generation
changes, while no running animation schedules no wake. Counter exhaustion or an unrepresentable deadline follows
the existing typed runtime-failure or saturating bounded-time path and cannot create an ABA-valid
frame or an unbounded busy loop.

| Transition                                                           | Canonical effect                                                                                                      | Projection and failure rule                                                                                           |
| -------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| PTY transmit, query, display, frame, animation, or delete action     | Ghostty validates and atomically mutates its bounded graphics storage or emits its protocol reply                     | Rejection preserves the prior canonical image/placement state; no client payload is partially committed               |
| Direct, file, temporary-file, or shared-memory load                  | Ghostty acquires bounded input and stores owned decoded pixels; temporary-file and shared-memory cleanup stays native | Paths, handles, mappings, encoded containers, and compressed bytes never enter OBKG or OBRW                           |
| Due animation tick                                                   | Ghostty selects the displayed frame and returns the next delay                                                        | A graphics-generation change advances one terminal revision; no change is a projection stutter                        |
| Scroll, reflow, resize, screen switch, or placement mutation         | Ghostty keeps pins and placement relationships canonical                                                              | Host resolves the current absolute crop and layer during capture; no unresolved pin crosses the boundary              |
| Live capture and serialized handoff                                  | Host stages deterministic resources and placements against one generation/revision                                    | Exact commit publishes all bytes and base metadata together; allocation, limit, or stale-guard failure publishes none |
| Render-detail row-window capture                                     | Host selects current placements intersecting the requested historical rows and rebases them                           | OBRW commits rows and its independent graphics snapshot atomically; text detail excludes graphics                     |
| Exact acknowledgement, reset, delete, eviction, detach, or reconnect | Delivery ownership advances or invalidates the attachment-scoped base                                                 | A stale acknowledgement cannot advance resources; invalidation forces a complete successor snapshot                   |

macOS and supported libc-backed Linux targets enable every medium. Windows enables direct, regular
file, and temporary file media but reports POSIX shared memory unsupported, matching the pinned
upstream implementation; this platform exclusion is explicit rather than emulated with a path or
copy side channel. Completeness here means the bounded pinned-revision Kitty graphics feature set,
not unlimited image dimensions or memory, every Kitty keyboard keysym, OSC 66, OSC 99, or
Ghostty's application/window integrations.

Conformance requires stateful placement/animation property tests, a source-independent corpus differentially checked
against the exact Ghostty pin, strict Zig/TypeScript OBKG and render-detail OBRW agreement, renderer
tests that observe all three paint seams in Canvas2D and WebGPU, and native execution or cross-build
evidence for each claimed platform. The aggregate 36 MiB frame, 33 MiB graphics-extension, 32 MiB
decoded-resource, 256-visible-image, 1024-visible-placement, and 44 MiB row-window limits remain
independent admission checks; exceeding any one rejects the current capture without weakening the
canonical terminal state or the last committed client base.

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

Ghostty unknown APC input is admitted only when its content begins with the exact private prefix
`orchbee.agent.v1;` and the remaining unpadded base64url decodes to valid, NUL-free UTF-8 of at most
4 KiB. Truncated, empty, malformed, or oversized input produces no signal. The reader transaction
stages the decoded signal with its PTY read batch; rollback publishes nothing. Commit assigns a
positive terminal-owned sequence and appends the signal to a 64-entry FIFO; a full FIFO
conservatively drops the new signal without disturbing already admitted order. Egress encodes the
exact queue-head terminal identity, sequence, and decoded body as `TerminalApplicationSignal`;
encoding or batch fullness preserves that head, and exact append consumes only it. The record's
literal channel `orchbee.agent.v1` is an internal historical Host-to-Desktop discriminator, not the
child wire protocol. OSC 9, OSC 777, and other public notification input produces no application
signal. This path is an untrusted application transport: it authenticates no child, derives no
Agent/provider state, and owns no product or platform notification policy.

Ghostty progress reports update one terminal-owned current sample transactionally with their PTY
read batch. The state is `remove`, `set`, `error`, `indeterminate`, or `pause`; a present percentage
is bounded to `0..100`. Projection carries the sample in the optional OBTF progress extension, so
a new attachment receives current active state, intermediate values may coalesce, and `remove`
prevents an old task from reappearing. Progress never changes lifecycle, process ownership,
scheduling priority, notification policy, or terminal content revision.

OSC 7501 program status reaches the Host beside Ghostty, whose parser discards the unknown OSC. A
per-terminal scanner with the same VT500 string boundaries finds each complete
`ESC ] 7501 ; ... BEL|ST` sequence, and the reader feeds Ghostty slices that end at each one, so the
query reply and record changes take effect in stream order with Ghostty's own replies and OSC 133
effects. A sequence longer than 4096 bytes, containing a control or non-ASCII byte, or ended by
ESC, CAN, or SUB is dropped. The query `?` enqueues the reply `ESC ] 7501 ; ?` with the query's
terminator through the ordinary terminal-reply path. A report follows the specification's pair
grammar: malformed pairs and unknown keys are skipped, a repeated key keeps its last value, and an
exceeded limit, undecodable base64, or control text discards the report. Only the root record's
state is retained, with a real-time clock read taken after the report arrived; a report with an
`id` addresses a child record, which the Host does not keep, and display fields are validated and
dropped. The record is staged and committed with its reader batch. `state=clear`, RIS, and Host
reset remove it; OSC 133 `A`, `N`, and a primary `P` remove `idle`, `working`, and `blocked`
(Orchbee takes the specification's option to drop `idle`, and counts `P` because its PowerShell
integration marks prompts that way); `done` and `error` survive prompts. Screen switches do not
affect it. Projection carries the record in optional OBTF kind 7. It is the program's own claim and
never lifecycle, process, or Agent identity authority.

Ghostty OSC 133 `C` and `D` semantic-prompt actions update one terminal-owned command interval
transactionally with their PTY read batch. The Host retains only the latest completion sequence,
duration, wall-clock completion time, and normalized exit code. It never retains or projects command
text, arguments, environment, or output. A failed reader batch publishes neither a start nor a
completion. The optional OBTF command-completion extension lets a newly attached client see the
latest fact; the Host does not decide freshness, focus, notification permission, rate limiting, or
whether a client should produce an effect.

Every `ReadRows` request declares its revision policy. `exact` preserves captured-revision identity
for absolute search, buffer access, and other deterministic reads. `current-view` is admitted only
for a `viewport` request whose origin is `tail` or `committed`; the request revision is an observation
cursor, not an admission fence. While holding the terminal mutex, the Host resolves that origin
against the then-current screen or attachment-owned tracked anchor and encodes the current revision
and row count. Both policies still require the exact current projection generation. Continuous PTY
output can therefore advance revision without starving interactive viewport reads, while resize,
screen replacement, and anchor eviction remain hard barriers.

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

When DEC in-band size-report mode `2048` is enabled, the same reader transaction immediately queues
`CSI 48 ; rows ; columns ; height-pixels ; width-pixels t` from the committed canonical geometry —
real cell counts with zero pixel fields while the committed geometry is pixel-unknown.
Every later successful attachment or terminal-driven geometry commit first clears synchronized
output and then queues another report from that exact committed geometry; no-op and failed resize
attempts emit none. Report admission uses the independent terminal-reply budget. Failure to admit a
required report is a typed `terminal_reply` integrity failure rather than a successful resize with
an unreported geometry.

Each attachment also owns one pinned Ghostty selection gesture. Pointer actions carry one captured
generation and viewport-relative numeric endpoints, but no revision identity. Under the terminal
mutex, each action resolves its endpoint at processing time from the attachment's committed tracked
anchor while viewing history, or from the current active viewport otherwise. A revision advance can
therefore neither reject nor reinterpret the action outside that current view; generation mismatch,
screen invalidation, and anchor eviction cancel it. A start replaces the prior gesture only after its
click behavior and selection can be installed, an update applies Ghostty's within-cell, rectangle,
word-between, line, or semantic-output drag rule from bounded surface geometry, and an ordinary end
releases pointer activity while retaining only the click-sequence anchor. Cancellation is point-free
and idempotently resets the gesture and releases its tracked pin while preserving the latest
selection snapshot; it never substitutes for an ordinary end. Selection commands,
detach, generation or screen invalidation, and state teardown perform the same reset. Invalid
action-specific gesture fields are rejected by the protocol adapter before the runtime mutation
boundary.

Selection commands preserve Ghostty's canonical content semantics. Select-all trims surrounding
blank cells, excludes unused viewport rows, and leaves an empty screen unselected instead of
manufacturing a geometry-wide selection. A command that names a row holds only for the exact revision
and generation that row was read from. Clear and select-all name none, so ordinary output cannot make
them stale and only a generation mismatch rejects them.

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

| Operation class                     | Required semantics                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ----------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Read-only inventory or state query  | May be repeated; each reply describes one coherent observation                                                                                                                                                                                                                                                                                                                                                                                                    |
| Create                              | While its terminal remains retained in one Host epoch, `CreationId` plus durable launch fingerprint is idempotent. A non-empty environment-overlay replay must also match its retained digest; an empty overlay is a wildcard only for validating that retained creation. Exact replay returns the same terminal, while a launch or non-empty overlay conflict is typed. Explicit disposal releases the identity instead of retaining an unbounded Host tombstone |
| Deferred start                      | The first valid geometry starts exactly one child. An exact replay returns `already-started` with the committed geometry; reuse with different geometry is a non-retryable `start-conflict`. A typed spawn failure leaves `awaiting_start` owned and retryable.                                                                                                                                                                                                   |
| Absolute desired-state update       | Reapplying the same effective value has no additional semantic effect                                                                                                                                                                                                                                                                                                                                                                                             |
| Attach                              | An attachment identifier is session-scoped; an exact replay resolves to the same binding and conflicting reuse is rejected                                                                                                                                                                                                                                                                                                                                        |
| Detach, pause, and acknowledgements | Exact duplicates are no-ops; stale or cross-attachment values never affect current state                                                                                                                                                                                                                                                                                                                                                                          |
| Graceful and force termination      | Idempotent intents; escalation timing belongs to Desktop                                                                                                                                                                                                                                                                                                                                                                                                          |
| Input and relative interaction      | Ordered and non-replayable after an unknown result; automatic retry could duplicate user action                                                                                                                                                                                                                                                                                                                                                                   |
| Dispose                             | Removes only a contained settled terminal whose exact outcome is published; pending publication or blocked containment returns a retryable typed failure; after an unknown result Desktop reconciles through inventory                                                                                                                                                                                                                                            |

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
changes active terminal colors or clears an OSC override, content revision advances once while
generation remains unchanged. Equal-RGB overrides are still cleared by a newly admitted operation;
exact operation replay returns before physical application. If the complete defaults and active
values already match and no override remains, content revision need not advance even though the
independent Host theme revision commits the new operation. All attachments project the
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

A complete terminal geometry carries positive column and row counts plus one paired cell-pixel
dimension. Both pixel fields are either measured and positive together or zero together; the
paired-zero form is the single admitted representation of unknown pixel metrics. It is not an
estimate, never substitutes `8x16` or `1x1`, and is preserved verbatim across launch resolution,
engine state, the OS PTY winsize, projection-buffer headers, and encoded frames. A mixed pair —
one axis measured, the other zero — is malformed and rejected at every admission boundary. Cell
dimensions stay positive in every state; a pixel-unknown terminal is a normal runnable state,
never an error, a wait, or a fabricated grid. Every committed geometry fully replaces the pixel
pair: a known-to-unknown commit explicitly clears engine pixel metrics to zero instead of keeping
the stale measurement, and a later known commit installs the new one. Pixel-dependent surfaces
degrade honestly: XTWINOPS `14t`/`16t` pixel-size queries receive no reply while pixels are
unknown, `18t` still answers real cell counts, mode-`2048` reports carry real cells with zero
pixel fields, and semantic pixel-position mouse input has no encodable form under unknown pixels
and produces no PTY bytes. Raw cell-byte input, keyboard, and paste are unaffected.

Geometry has two admitted initiators and one native commit owner. The terminal-scoped viewport
restore target starts as the complete launch geometry. Every successful request from the current
resize-lease owner replaces that target; a geometry-changing request does so only after the PTY and
Ghostty commit, while a request already equal to canonical geometry may still replace a stale
target without a platform effect, generation change, or geometry revision. Lease transfer alone
never synthesizes a measurement or changes the target.

PTY output may instead request Ghostty's enabled 80/132-column mode. The Host stream adapter
recognizes the parsed mode action rather than reparsing bytes, serializes it with attachment resize,
applies the new OS PTY grid and pixel extent, then invokes pinned Ghostty's DECCOLM operation while
the reader batch still owns its commit. Cell pixel metrics remain unchanged and DECCOLM never
overwrites the viewport target. Every explicit DEC mode `40` set, reset, or saved-mode restore runs
the same transaction toward the saved complete viewport grid and cell-pixel geometry. An
already-equal restoration has no platform effect or geometry revision. A geometry-changing restore
releases synchronized-output suppression and, when mode `2048` is active, admits an exact report
for the committed geometry. Multiple column-mode and mode-40 actions in one batch apply in parser
order, and only the batch's final coherent geometry becomes observable.

After an attachment resize commits revision `r`, Host synchronization accepts an engine observation
at `r'` only when `r' >= r`. An observation at `r` must contain the requested complete geometry. PTY
output may commit between the resize and that observation; when it advances to `r' > r`, the newer
complete geometry and revision are canonical and are copied into Core without rewriting them or
aborting the Host. The resize acknowledgement continues to identify the accepted request commit at
`r`, while subsequent projection carries the newer observation.

For each running terminal, the broker grants at most one mutable resize lease. Attachment admission
retains the immutable `requestResizeLease` fact separately from current ownership, so an exact
observer attach remains idempotent after a later transfer. A resize-capable attachment may
explicitly claim ownership. Claiming current ownership is an event-free idempotent response; a
transfer reserves its response and both ownership-event records before mutation, atomically moves
the lease and successful-claim rank, publishes the previous owner's attachment-scoped
`ResizeLeaseChanged(false)` fact, publishes the claimant's `ResizeLeaseChanged(true)` fact, and only
then publishes the complete capability response. Rejection or reservation failure preserves the
prior owner and rank.

Detaching the current owner atomically selects the eligible observer with the newest successful
claim rank; observers never granted ownership tie-break by earliest broker admission sequence. The
Host reserves the ownership event and detach response before mutation, publishes the promoted
owner's event, and only then publishes the detach acknowledgement. If no eligible observer remains,
the terminal has no attachment resize owner until a later attach or explicit claim requests the
lease. Settled terminals reject claims and do not promote a new resize owner.

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

Without a control request, a normal self-exit is legal only when no broker and no retained
terminal remain; `drain` and `exit` end the Host as [Host lifecycle](#host-lifecycle) describes. During fatal
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
- A pending application signal follows the same prepare, append, and commit discipline without
  attachment selection. New admitted private-APC input joins the bounded FIFO behind predecessors;
  failed encoding or append preserves the queue head, and only the exact appended
  `TerminalApplicationSignal` consumes that head.
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

| Current state      | Input                                                | Next state       | Required effect                                           |
| ------------------ | ---------------------------------------------------- | ---------------- | --------------------------------------------------------- |
| no broker          | accepted local transport with an admitted offer      | `awaiting_hello` | Allocate a fresh physical session identity                |
| no broker          | refused offer, control request, or no frame in 2 s   | no broker        | Send at most one reply and close only this transport      |
| `awaiting_hello`   | valid compatible hello                               | `active`         | Commit negotiated limits and broker metadata              |
| `awaiting_hello`   | timeout, malformed or incompatible hello, disconnect | no broker        | Close only this transport; terminals are unchanged        |
| `active`           | disconnect or transport failure                      | no broker        | Destroy all session attachments; terminals are unchanged  |
| any broker session | another offer                                        | unchanged        | Reject it with `capacity` on a control connection         |
| any broker session | control request                                      | unchanged        | Answer it on a control connection from published facts    |
| any broker session | `drain` or `exit` request                            | unchanged        | Post it to the dispatch thread; answer once it is applied |
| `active`           | the Host lifecycle ends the Host                     | no broker        | Linger at most 1 s for the Worker to close, then close    |

### Host lifecycle

The `drain` and `exit` control requests move the Host through three process states, which `query`
and an offer's admission read. States only move forward, so a repeated, late, or reordered request
never undoes another.

| Current state          | Input                        | Next state | Required effect                                                    |
| ---------------------- | ---------------------------- | ---------- | ------------------------------------------------------------------ |
| `ready`                | `drain`                      | `draining` | Close terminal admission, then answer `drain-result` (`draining`)  |
| `ready`, `draining`    | `exit`                       | `stopping` | Answer `exit-result`, then end the Host with every terminal        |
| `draining`, `stopping` | `drain`                      | unchanged  | Answer `drain-result` with the current state                       |
| `stopping`             | `exit`                       | unchanged  | Answer `exit-result`                                               |
| `draining`             | no retained terminal remains | Host ends  | Close the broker session, then return from the serve loop normally |

`ready` admits new terminals. `draining` fails a `CreateTerminal` that would add a retained terminal
with the non-retryable `host-draining`; a replay of a retained creation, start, input, attachment,
and disposal continue. An offer to a `draining` or `stopping` Host is rejected with `stopping`,
after the `capacity` check, and control requests are still answered. "Last retained terminal" means
an empty registry: a settled terminal the broker has not disposed keeps a draining Host running.

Only the dispatch thread changes the state. Without a broker session it applies a request on the
control connection itself. During one, a control lane posts the request and answers once the facts
the dispatch thread publishes show it applied, waiting at most 10 s and otherwise closing without a
reply; the dispatch thread applies posted requests between two Worker requests, never inside one,
and again while it joins the lanes. A `drain` answer therefore means no later `CreateTerminal` adds
a terminal.

When the state ends the Host, the dispatch thread stops reading Worker requests, waits at most one
second for the broker to read what was already written and close, and closes the session; without
a broker it returns at once. The process then runs its ordinary teardown, which ends every retained
terminal and confirms its containment. After `exit`, those terminals end without `TerminalExited`
events reaching the broker.

The lifecycle has one owner and three forward-only states, and a lane only waits for a published
state, so it keeps no TLA+ model; `TH-025` maps to implementation tests.

### Terminal session

| Current state              | Input                                                        | Next state                                  | Required effect                                                                                                                                                         |
| -------------------------- | ------------------------------------------------------------ | ------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| absent                     | valid create with explicit geometry                          | `running`                                   | Prepare capacity, allocate one retained record, spawn exactly one runtime at that geometry, then atomically admit its embedded session and both indexes before replying |
| absent                     | valid create with deferred geometry                          | `awaiting_start`                            | Prepare capacity, allocate one retained record, retain the complete prepared launch, and atomically admit its embedded session and both indexes without spawning        |
| any retained state         | exact launch replay with an empty overlay                    | unchanged                                   | Return the retained identity without reconstructing or extending the original overlay capability                                                                        |
| any retained state         | exact launch replay with a matching non-empty overlay        | unchanged                                   | Return the retained identity after matching the target-normalized overlay digest                                                                                        |
| any retained state         | launch conflict or non-empty overlay-digest conflict         | unchanged                                   | Return a typed creation conflict without mutating identity, runtime, or process environment                                                                             |
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
capability revision `1`. Protocol generation 1 permits only the input bit to change live; read
and resize remain equal to the admitted value. Every successful attach or update reply returns the
complete current capabilities and revision.

| Current state  | Input                                                                   | Next state                    | Required effect                                                                                                     |
| -------------- | ----------------------------------------------------------------------- | ----------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| absent         | authorized attach with at least one capability                          | projection `paused`           | Create exactly one broker-scoped capability binding; a resize lease requires resize capability                      |
| any live state | exact current input-capability revision and value                       | same state                    | Return the complete current binding as an idempotent replay                                                         |
| any live state | exact next revision disabling input                                     | same projection state         | Repair and remove only this attachment's input ledger, then revoke Core input authority                             |
| any live state | exact next revision enabling input                                      | same projection state         | Prepare a fresh empty input ledger, then expose Core input authority; do not replay prior input                     |
| any live state | old, gapped, conflicting, or unchanged next-revision input update       | unchanged                     | Reject with a typed non-retryable capability-revision failure                                                       |
| any live state | current resize owner claims its resize-capable attachment               | same projection state         | Return the complete current capability binding without an ownership event                                           |
| any live state | another resize-capable attachment claims ownership                      | same projection state         | Atomically revoke then grant ownership, advance successful-claim rank, and publish both events before the response  |
| `paused`       | activate demand                                                         | `ready.snapshot(base = null)` | Lazily acquire terminal render resources as needed                                                                  |
| `ready`        | provisional frame wins the serialized read-commit guard                 | `in_flight`                   | Commit its reserved outgoing record, exact frame, base, and sequence together                                       |
| `in_flight`    | exact applied acknowledgement                                           | `ready`                       | Promote only the acknowledged base                                                                                  |
| `ready`        | capture cannot be represented within its semantic bound                 | `blocked`                     | Record the exact captured or attempted revision, which cannot exceed the current observation                        |
| `blocked`      | Core already observes a revision newer than the failure                 | `ready.snapshot`              | Lazily invalidate the failed capture and retry without requiring another terminal mutation                          |
| active state   | pause                                                                   | `paused`                      | Repair this attachment's held input and focus claim, then abandon the flight and invalidate its delta base          |
| any live state | owner detach selects this newest successfully claimed eligible observer | same projection state         | Grant current resize ownership and publish its scoped event before the detach acknowledgement                       |
| any state      | detach or broker loss                                                   | absent                        | Queue attachment-owned key/mouse/final-focus repair, then release pins, flights, selections, exports, and authority |

## Correctness invariants

Stable claim identifiers are used by tests, models, and review findings.

| Claim    | Invariant                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `TH-001` | Each retained `TerminalId` names one registry membership whose stable session reference belongs to exactly one `RetainedTerminal`; that record contains exactly one `TerminalSession` and, in stable state, exactly one deferred launch preparation or started engine. No engine exists outside a create/start transaction or retained record.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `TH-002` | Each mutable semantic fact has one authoritative owner; derived state cannot outlive or contradict that owner.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `TH-003` | At most one broker session exists per endpoint, only an admitted `worker-host` offer opens it, and every attachment operation is authorized by that physical session plus its `AttachmentId`. While it runs, at most four control connections are served concurrently from facts the session's dispatch thread published, and post `drain` or `exit` for that thread to apply (`TH-025`); they never reach `HostService` or attachment authority, and an offer on one is rejected with `capacity` until every control lane has stopped. A running terminal has at most one current resize lease; explicit transfer revokes the old owner before granting the new owner, and owner detach promotes the eligible observer with the newest successful claim, using earliest admission only to tie-break never-granted observers, without changing immutable attach-replay identity. Accepted held input belongs to exactly that attachment; detach repairs and removes only its ledger before revoking authority.                                                                                                 |
| `TH-004` | Broker loss repairs and destroys only broker-scoped attachment state and capabilities; it never cross-releases another attachment or disposes a healthy retained terminal. An unrecoverable repair integrity failure uses the explicit runtime-failure containment path.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `TH-005` | Creation is atomic and idempotent by `HostEpoch + CreationId + durable launch fingerprint` while the terminal is retained; explicit disposal releases that identity. A separate target-normalized, order-independent environment-overlay digest belongs to that retained creation generation. A non-empty replay must match it, while an empty replay is a wildcard only for an already retained creation and never reconstructs a capability. Deferred creation exposes no runtime geometry or process. Start commits exactly the supplied initial geometry once, exact replay is idempotent, conflicting replay is rejected, and failed spawn retains a startable owner without an orphan process.                                                                                                                                                                                                                                                                                                                                                                                                           |
| `TH-006` | PTY output, input encoding, projection, history, selection, and resize synchronization use one canonical Ghostty state and coherent revision/generation pair; a failed projection names its exact captured or attempted revision. A post-resize observation never regresses the accepted revision, requires exact geometry at the same revision, and adopts a newer canonical observation. Every frame, merged delta, and row window joins rows exactly where the canonical row above wraps.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `TH-007` | Accepted client input occupies exactly one client-FIFO position and atomically commits any held-input cleanup ownership; rejected input occupies neither. A buttonless semantic hover may retire only while wholly unstarted and stale against the terminal-owned mouse-input epoch; every other accepted client entry remains ordered, a started entry completes without control-sequence truncation, and surviving client entries preserve relative FIFO order. Held input precharges bounded detach-repair capacity. Terminal replies have independent bounded admission and priority within a bounded reply burst after which the writer yields to a waiting accepted client entry; an unknown client-input result is never automatically replayed.                                                                                                                                                                                                                                                                                                                                                        |
| `TH-008` | Normal exit is published only after child completion, normal output closure, and writer quiescence, irrespective of observation order. Settlement captures one final revision; terminal-owned content, generation, geometry, configuration, and revisions cannot advance afterward.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| `TH-009` | An unrecoverable runtime failure is explicit, fences unsafe I/O, and cannot be reported as normal drain or normal exit. Unconfirmed containment admits no new frame handoff. Cleanup remains Host-owned and retryable without a broker; a settled outcome cannot be rewritten by a later cleanup symptom or removed before its exact publication commits, and disposal never silently orphans an uncontained process tree. Every POSIX member signal and liveness decision binds to one stable kernel identity before the authoritative membership observation. An invalidated matched reference cannot report signal success or empty containment and requires a fresh scan. Linux distinguishes an exact unreaped zombie from a reaped/reusable pidfd using same-reference signal-0, except that the retained leader is protected from reuse by its Host-owned delayed wait. A recycled numeric PID can never target or count its replacement through a stale reference; a same-session replacement requires a fresh stable reference.                                                                       |
| `TH-010` | An outgoing delivery transition commits if and only if its exact record is in the handed-off batch; batch fullness returns a committed prefix without dropping it. Projection record and flight commit are allocation-free while the engine read-commit guard holds the canonical `projection -> terminal` locks and validates generation and failure-fence epoch. A generation or fence change rejects an older provisional frame without advancing its sequence. A handed-off record is immutable for its scoped sequence and is never reconstructed with different bytes. `NeedSnapshot(S, decoder-rejected)` also discards the incremental projection before that snapshot.                                                                                                                                                                                                                                                                                                                                                                                                                                |
| `TH-011` | At most one frame, row request, or export chunk of each kind is in flight per attachment; only the exact scoped acknowledgement advances it. A published frame sequence identifies one immutable record for that Host epoch and attachment.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `TH-012` | Bounds and backpressure reject or defer the current operation without corrupting retained registries, canonical terminal state, delivery bases, or sequence numbers. A full outgoing batch preserves the exact unsent projection for retry without advancing delivery.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `TH-013` | Every successfully acquired resource, including a POSIX stable member reference, has one release owner and is released at most once; a teardown pass attempts every retained runtime even when one attempt fails, and never reports failed cleanup as clean shutdown. FreeBSD descendant-zombie reaping is confined to the retained leader's reaper subtree and cannot consume that leader's wait result or another terminal subtree.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| `TH-014` | All control JSON crosses one native protocol adapter, all binary data crosses its owning codec, and every registered role/encoding/success-kind/dispatch-operation association comes from the protocol registry; malformed, unknown, incompatible, mismatched, or unhandled records cannot reach domain transitions.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `TH-015` | Each Ghostty bell advances one terminal-owned sequence exactly once. A read-capable attachment receives each later sequence at most once, advances only with exact outgoing-batch append, and never replays a sequence that predates its admission or survives its destruction.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| `TH-016` | An input-authorized application clear or reset mutates only the Host-owned Ghostty state, advances generation and revision exactly once, invalidates dependent projection and interaction state coherently across attachments, never enters PTY input/reply queues, and is never automatically replayed after an unknown result.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `TH-017` | OSC 52 can publish only one bounded plain-text write intent for selection kind `c`; it never reads platform clipboard data. Reader-batch rollback publishes nothing, outgoing-batch rollback preserves the same request for retry, one successful handoff consumes it across all attachments, and reconnect does not replay it. The exact private APC prefix may transactionally append one decoded, bounded UTF-8 payload to a 64-entry FIFO in terminal sequence order; public notification channels, unrelated APC prefixes, malformed payloads, and reader rollback publish none, outgoing rollback preserves the exact queue head, and append consumes only that head. Neither that signal, terminal progress, nor the OSC 7501 program status record becomes Agent, provider, lifecycle, or notification authority; the Server alone decides how a program status record informs Agent state.                                                                                                                                                                                                            |
| `TH-018` | One accepted runtime-theme operation atomically replaces color scheme, default foreground/background, nullable cursor color, and all 256 palette entries at one independent Host theme revision. Replay, stale, conflict, validation, overflow, lifecycle, and runtime-coherence failures preserve the complete prior Core and physical state. Recovery publishes the complete current state; all attachments observe one coherent content revision, and OSC/reset/RIS resolve against the latest committed runtime theme. A color-scheme change first queues one report (`ESC [ ? 997 ; 1 n` for dark, `ESC [ ? 997 ; 2 n` for light) to each program that enabled mode `2031`, before the theme installs; palette-only changes and unsubscribed programs queue none, a stopped writer skips it, and an unqueued color-scheme notice is dropped while the theme still applies.                                                                                                                                                                                                                                |
| `TH-019` | Each attachment has one positive monotonic capability revision initialized to `1`. Input preparation and final queue commit require its exact current revision and enabled input bit. An exact capability-update replay is inert; stale, gapped, conflicting, and unchanged next revisions cannot mutate authority. Downgrade repairs and removes only the target attachment's held-input ledger before revocation, preserves already accepted FIFO entries without duplication, and upgrade creates a fresh empty ledger. Immutable admitted capabilities continue to define attach replay.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `TH-020` | `TerminalEngine` OR-aggregates focus claims from attached, input-capable attachments whose Core projection demand is active. Only aggregate zero-to-nonzero and nonzero-to-zero transitions emit terminal `FocusIn` and `FocusOut`. Detach, broker loss, demand pause, and capability downgrade clear only the target claim after its key and mouse repair; a final focus repair is precharged and ordered last. Stale, disabled, paused, and detached input cannot create a claim, while activation and capability upgrade never replay one.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `TH-021` | The Host advertises only terminal capabilities with an owned, bounded, observable implementation. Glyph Protocol is disabled and cannot reply or mutate its glossary. XTGETTCAP answers one fixed `xterm-256color` subset with one bounded ordered response and exact hexadecimal encoding; an invalid first name returns a negative response, while a later invalid name terminates the successful prefix. It never installs capability data or introduces another VT owner.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `TH-022` | External effects of modes `20`, `40`, `1004`, `2026`, and `2048`, plus DEC column mode `3`, refine the same canonical mode transition. Mode `20` charges CR-to-CRLF expansion before PTY-write admission. Mode `1004` orders its immediate aggregate report against prepared focus commits. Mode `2026` suppresses only projections and is bounded by explicit reset, full reset, successful geometry change, or a one-second monotonic deadline. Mode `3` serializes PTY and Ghostty 80/132-column geometry without replacing the terminal-scoped viewport target. Every explicit mode-`40` set, reset, or saved restore restores the complete latest successful resize-owner geometry; an already-equal target adds no geometry revision. Mode `2048` reports exact geometry on enable and every successful geometry commit. Resize or required-reply failure enters its typed containment path without rolling back an already committed coherent geometry; no effect creates attachment-local terminal state.                                                                                              |
| `TH-023` | Pinned Ghostty is the only Kitty graphics state owner. Every accepted media, frame, animation, placement, query, and delete transition stays within finite loading, decoded-storage, cardinality, wire, and history bounds. A live or render-history capture names one exact terminal revision and graphics generation; every placement references an included owned image, has an in-bounds crop, contains no unresolved pin/path/handle, and belongs to exactly one signed-z paint layer. A due animation tick that changes displayed pixels advances the terminal revision exactly once, while a stutter advances none. Live OBKG and render-detail OBRW commit resources and placements atomically; text-detail OBRW contains no graphics. Reset, deletion, eviction, screen change, detach, reconnect, stale acknowledgement, or failed admission cannot retain, partially publish, or resurrect an invalid client resource base.                                                                                                                                                                         |
| `TH-024` | Cold primary-screen scrollback compression observes Ghostty's opaque activity token and a terminal-owned interaction serial only under a non-blocking terminal-lock attempt. Every successfully prepared non-empty client input and every committed PTY reader batch or visible Kitty animation tick advances that serial under the same lock. A changed Ghostty token, or a changed interaction serial while work is waiting or pending, receives at least 250 milliseconds of monotonic idle time before one bounded incremental step; interaction alone never wakes complete or unsupported work. Host maintenance offers at most one fair runtime opportunity per 20 milliseconds and never catches up in bursts; lock contention stutters without waiting or postponing an existing deadline. Pending work continues only on later fair turns; complete or unsupported work is dormant until compression-relevant Ghostty activity. Compression changes only page representation: revision, generation, geometry, logical rows, projection, history, selection, and delivery identities remain unchanged. |
| `TH-025` | The Host lifecycle only moves forward from `ready` through `draining` to `stopping`, and only the dispatch thread moves it. Once a `drain` answer is sent, no `CreateTerminal` adds a retained terminal and every new offer is rejected with `stopping`; a draining Host ends once it retains no terminal, and a stopping Host ends with every terminal. A control lane answers `drain` or `exit` only after the published state shows it applied.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `TH-026` | A prompt line occupies one client-FIFO position only when the root interactive shell is confirmed at a fresh, empty, quiet prompt at both preparation and publication; no other console program, alternate screen, input after the prompt's input start, moved anchor, or later prompt mark may admit it. A refusal names the first failing rule and occupies no position. Preparation never waits; only a prompt younger than the quiet period that passes every other rule is refused as retryable `settling`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |

## Scenario decisions

| Scenario                                          | Contract decision                                                                                                                                  |
| ------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| Desktop restarts while terminals run              | Host retains terminals; the new broker handshakes, inventories, and creates new attachments.                                                       |
| A same-user process knows the endpoint            | It is inside the current trust boundary; application-level isolation is not promised.                                                              |
| A second broker connects while one is active      | Its offer is rejected with `capacity`, and it dials again after the active session ends.                                                           |
| A control request arrives during a Worker session | A control connection answers it from the facts the session last published; it gains no broker authority.                                           |
| An operator drains the Host                       | Refuse new terminals with `host-draining` and new offers with `stopping`; keep retained terminals; exit once none remains.                         |
| An operator sends `exit`                          | Answer, close the broker session, and end every retained terminal and the Host through ordinary teardown.                                          |
| Protocol negotiation fails                        | Close that broker transport; retained terminals remain.                                                                                            |
| A response is lost after input admission          | Outcome is unknown; Desktop must not automatically replay the input.                                                                               |
| Mouse tracking changes after hover admission      | Retire only an unstarted stale buttonless hover; complete a started sequence and preserve every ordered input entry.                               |
| OSC 133 reports a local prompt after child loss   | Clear orphaned mouse tracking only after a successful no-active-command observation; nested or unknown process state preserves it.                 |
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

Only delivery and process identity retain TLA protocol cores. Other workflows use the concrete tests below and their adjacent owner suites. They cover named/generated traces, not exhaustive interleavings from the retired models. All behavior obligations remain; changing evidence methods does not turn an unexecuted check into a pass.

| Evidence                                                                                | Required coverage                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| --------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [Delivery TLA+ model](../formal/tla/TerminalHostDelivery.tla)                           | `TH-003`, `TH-010`, `TH-011`, post-authorization by the current physical connection and `AttachmentId`, `brokerActive` as a bounded delivery scope rather than a wire epoch, batch prefixes, acknowledgement races, pause/detach, reconnect, and transport failure; the coordinator rejects prior-connection requests before this model boundary, control connections never reach it, and checked bounds live in the adjacent `.cfg`                                                                                   |
| [Process-identity TLA+ model](../formal/tla/TerminalHostProcessIdentity.tla)            | `TH-008`, `TH-009`, `TH-013`, a finite two-scenario proof family: the PID-race scenario acquires a stable member reference across numeric PID reuse and cannot confirm whole-session containment from one candidate, while the two-member scan retains the first error, continues through every candidate, releases every acquired reference, and confirms containment only after a complete error-free count finds no live member; later SID rebinding remains fenced, and checked bounds live in the adjacent `.cfg` |
| Registry and creation — `src/core/__tests__/registry_broker_test.zig`                   | `TH-001`, `TH-004`, `TH-005`: identity uniqueness, launch/overlay replay and conflicts, atomic creation failure and settlement-gated disposal                                                                                                                                                                                                                                                                                                                                                                          |
| Lifecycle and shutdown — `src/service/__tests__/runtime_aggregate_lifecycle_test.zig`   | `TH-005`, `TH-008`, `TH-009`, `TH-013`: start/settle/cleanup order, failure retention and attempting every runtime before shutdown completes                                                                                                                                                                                                                                                                                                                                                                           |
| Request outcomes — `src/service/__tests__/interaction_replay_test.zig`                  | `TH-005`, `TH-007`, `TH-010`, `TH-016`: prepare/commit/handoff, retryable rejection, unknown outcome and non-replayable clear/reset                                                                                                                                                                                                                                                                                                                                                                                    |
| Runtime theme — `src/engine/__tests__/runtime_theme_test.zig`                           | `TH-018`: atomic defaults/palette/cursor update, exact replay, conflict and no-mutation failure                                                                                                                                                                                                                                                                                                                                                                                                                        |
| Inventory pagination — `src/service/__tests__/connection_request_test.zig`              | `TH-002`, `TH-012`: generation-bound cursors, complete membership, payload packing and stale-cursor rejection                                                                                                                                                                                                                                                                                                                                                                                                          |
| Frame and activity publication — `src/service/__tests__/interaction_delivery_test.zig`  | `TH-006`, `TH-009`, `TH-010`, `TH-011`, `TH-012`, `TH-016`: exact acknowledgement, provisional commit, invalidation, failure containment and preserved newer flight                                                                                                                                                                                                                                                                                                                                                    |
| Input and mouse ownership — `src/engine/__tests__/input_ownership_test.zig`             | `TH-003`, `TH-004`, `TH-007`, `TH-019`, `TH-020`: attachment-only repair, FIFO, capacity/priority, stale mouse epochs and writer cleanup                                                                                                                                                                                                                                                                                                                                                                               |
| Mode effects — `src/engine/__tests__/runtime_mode_effects_test.zig`                     | `TH-006`, `TH-007`, `TH-020`, `TH-022`: synchronized output, viewport/canonical geometry, reset/resize and failure containment                                                                                                                                                                                                                                                                                                                                                                                         |
| Graphics and history — `src/service/__tests__/graphics_decode_test.zig`                 | `TH-002`, `TH-006`, `TH-010`, `TH-011`, `TH-012`, `TH-016`, `TH-023`: generations, bounded capture/decoding, invalidation and exact row/export/search identity; history and renderer suites refine the corresponding seams                                                                                                                                                                                                                                                                                             |
| Compression scheduling — `src/service/__tests__/runtime_aggregate_maintenance_test.zig` | `TH-024`: nonblocking lock contention, idle/cadence boundaries, incremental continuation, fairness and unchanged canonical revision                                                                                                                                                                                                                                                                                                                                                                                    |
| Composition invariant oracle                                                            | `TH-001` through `TH-013` and `TH-016` through `TH-024` across owner boundaries: retained-runtime membership, broker identity, lifecycle and containment, configuration, terminal-theme, geometry, capability revision, input-ledger/focus eligibility agreement, graphics-generation/base agreement, compression representation-only agreement, and bounded projection observations after every stable Service step                                                                                                   |
| Stateful property tests                                                                 | `TH-001` through `TH-024` as applicable to each owner: registry/index coherence, lifecycle, terminal-control and runtime-theme command sequences, attachment capability-revision and focus-claim sequences, graphics placement/animation sequences, compression scheduling, projection sequences, history/export sequences, and invariant checks after every action                                                                                                                                                    |
| Deterministic fault-injection tests                                                     | Allocation, encoding, immediate-create and deferred-start spawn, queue admission, runtime-theme preflight, resize, PTY I/O, process observation, fail-conservative membership errors, scripted PID recycle immediately before graceful HUP, force STOP/KILL/CONT, and liveness, reply handoff, partial batch, and multi-runtime shutdown failures at every commit seam                                                                                                                                                 |
| Native integration tests                                                                | Stable POSIX member identity and session/reaper-subtree process behavior, Windows Job/ConPTY behavior, endpoint ownership, peer identity, descriptor/handle cleanup, worker joining, and a real executable/Local IPC/PTY trace on every natively claimed platform                                                                                                                                                                                                                                                      |
| Cross-language protocol conformance                                                     | Every kind's role, encoding, and successful response; every native command, reply, event, identifier type, and declared union or error variant; every transport and binary format; both sides of every negotiated or hard bound; and each declared malformed-record rejection class                                                                                                                                                                                                                                    |
| Focused performance evidence                                                            | Bounded memory, no-demand behavior, fairness, large input backpressure, projection cadence, and history/export bounds without weakening semantic invariants                                                                                                                                                                                                                                                                                                                                                            |

Every TLA+ module header and stateful property-test suite must identify the transition vocabulary it
refines and the stable claim IDs it covers. Trace action names must use the transition vocabulary in
this document. Production changes that alter a state, owner, commit boundary, retry class, or
failure outcome must update this model and its evidence in the same change.

### Native evidence index

The following entries are the required executable refinements, not a claim that an unexecuted test
currently passes. A handoff may report conformance only after the package verification commands run
these entries successfully.

The following stable compatibility witnesses are aliases for existing Host invariants rather than
a parallel specification. The ordinary Host `test` gate executes the named native suites and
cross-language corpus; the separately selected `test:tla` gate executes the linked models. These
witnesses control Host adapters, failure injection, and deterministic schedules, but make no claim
about application UI or operating-system effects beyond the native integrations named below.

| Stable witness                                                            | Host invariant and executable refinement                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| ------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `terminal-parity/PR-10-replies`                                           | `TH-006`, `TH-007`, and `TH-021`; [`../src/engine/__tests__/terminal_compatibility_test.zig`](../src/engine/__tests__/terminal_compatibility_test.zig), [`../src/engine/__tests__/terminal_stream_test.zig`](../src/engine/__tests__/terminal_stream_test.zig), [`../src/engine/__tests__/runtime_input_test.zig`](../src/engine/__tests__/runtime_input_test.zig), protocol conformance, and real-PTY reply routing execute the bounded pinned-Ghostty query corpus and ordered admission. |
| `terminal-parity/PR-12-deccolm`                                           | `TH-006` and `TH-022`; [`../src/engine/__tests__/runtime_mode_effects_test.zig`](../src/engine/__tests__/runtime_mode_effects_test.zig), projection/Core synchronization tests, and controlled implementation tests execute mode-3 geometry, mode-40 viewport restoration, parser order, serialization, and containment.                                                                                                                                                                    |
| `terminal-parity/PR-13-clear-reset`                                       | `TH-016`; [`../src/engine/__tests__/runtime_state_test.zig`](../src/engine/__tests__/runtime_state_test.zig), [`../src/service/__tests__/runtime_aggregate_lifecycle_test.zig`](../src/service/__tests__/runtime_aggregate_lifecycle_test.zig), protocol conformance, and request-outcome/frame-delivery implementation tests exercise authorization, one canonical mutation, complete snapshot invalidation, and unknown-result non-replay.                                                |
| `terminal-parity/IN-15-detach-repair`, `terminal-parity/IN-16-input-fifo` | `TH-003`, `TH-004`, and `TH-007`; [`../src/engine/__tests__/input_ownership_test.zig`](../src/engine/__tests__/input_ownership_test.zig), [`../src/engine/__tests__/input_queue_test.zig`](../src/engine/__tests__/input_queue_test.zig), [`../src/engine/__tests__/runtime_input_test.zig`](../src/engine/__tests__/runtime_input_test.zig), Service input tests, and controlled implementation tests execute attachment-only repair and the bounded terminal-global FIFO.                 |

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
the native evidence below. The real-PTY `terminal-parity/SU-13-cursor-defaults`,
`terminal-parity/SU-16-startup-geometry`, and `terminal-parity/HS-07-selection-export` refinements
retired with the Desktop Electron suite; renderer geometry remains owner-local evidence. The controlled headed Canvas
`terminal-parity/SU-09-theme`, `terminal-parity/PR-11-bell`,
`terminal-parity/IN-10-read-only`, and `terminal-parity/IN-14-shared-focus` witnesses refine the
Host theme revision, bell sequence, capability revision, and aggregate-focus contracts through the
application boundary while capturing rather than claiming native theme, sound, notification, or
multi-window focus effects. Generated TypeScript/Zig conformance remains the authority that HTML
and raw VT formats are rejected before runtime access.

| Claims             | Required native evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `TH-001`, `TH-002` | `src/core/__tests__/registry_broker_test.zig`, `lifecycle_observation_test.zig`, and `projection_state_test.zig`: registry/index coherence and all three stateful transition suites; `src/service/__tests__/runtime_aggregate_lifecycle_test.zig`: immediate/deferred ownership, failed creation/start, and replay identity                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `TH-003`, `TH-004` | `src/engine/__tests__/input_ownership_test.zig` and `runtime_input_test.zig`: held-input ownership, late prepared-input rejection, precharged repair, and containment on repair failure; `src/core/__tests__/registry_broker_test.zig`: one-broker attachment ownership, explicit resize transfer, idempotence, denial, rank overflow, and recency promotion; `src/service/__tests__/connection_request_test.zig`: revoke-before-grant event order, replay, broker replacement, disconnect, and detach acknowledgement; `src/transport/__tests__/local_endpoint_test.zig`: failed-handoff preservation, and a Windows pipe that keeps listening across accept waits and gives a late first frame its 2 s deadline; `src/transport/__tests__/candidate_test.zig`: refused foreign offers, a control request before the Worker, and `capacity` and `query` answers on control connections during a live Worker session; `src/handshake/__tests__/handshake_test.zig`: every `schemas/connection` fixture and admission case                                                                                                                                                                                                                                                                      |
| `TH-005`           | `src/model/__tests__/launch_test.zig`: all-numeric or all-null launch geometry, durable launch digest identity, target-normalized overlay validation, and order-independent overlay digest; `src/service/__tests__/runtime_aggregate_lifecycle_test.zig`: deferred ownership, failed-start retry, exact initial geometry, empty-overlay retained replay, matching and conflicting non-empty overlay replay, creation replay, and immediate creation failure; `src/service/__tests__/terminal_integration_test.zig`: real PTY disconnect/replay and isolated immediate spawn failure; `src/engine/__tests__/runtime_launch_test.zig`, `runtime_startup_test.zig`, and `runtime_state_test.zig`: canonical launch geometry, overlay application, one-shot startup, and partial-start cleanup                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `TH-006`           | `src/platform/__tests__/process_test.zig`, `src/engine/__tests__/runtime_state_test.zig`, and `runtime_input_test.zig`: active-command classification, canonical parsing, terminal replies, OSC colors, projection metadata, and terminal-driven resize ordering; `src/engine/__tests__/history_rows_test.zig`, `history_selection_test.zig`, and `history_export_test.zig`: rows, reflow, selection, and export; `src/core/__tests__/lifecycle_observation_test.zig` and `projection_state_test.zig`: observation and delivery rules; `src/service/__tests__/projection_test.zig`: immediate input invalidation, fresh inactive recovery before and after physical delivery, active-to-idle transitions, byte-bearing and zero-byte input, current OSC 133 prompt/command reclassification, platform-active precedence, and query failure; `runtime_aggregate_lifecycle_test.zig`: serialized projection races and full-geometry synchronization; `runtime_aggregate_attachment_test.zig`: rejected/focus input exclusion, accepted raw/non-focus invalidation, deterministic runtime-revision advancement between resize commit and Host synchronization, and monotonic adoption; engine stream/projection/selection unit tests and the paired pinned-Ghostty surface corpus remain required |
| `TH-007`           | `src/engine/__tests__/input_queue_test.zig`, `input_ownership_test.zig`, and `runtime_input_test.zig`: exact byte and entry capacity, bounded admission, priority, FIFO, settlement, and detach repair; `src/service/__tests__/runtime_aggregate_attachment_test.zig`: preparation rollback; `src/service/__tests__/input_request_test.zig`: raw/semantic failure equivalence and queue-admission-only acknowledgement                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `TH-008`, `TH-009` | `src/model/__tests__/lifecycle_test.zig`; `src/core/__tests__/lifecycle_observation_test.zig` and `registry_broker_test.zig`: lifecycle, immutable outcomes, and publication-gated removal; `src/platform/__tests__/process_test.zig`: retained leader/SID fencing, macOS stop-versus-exit and already-waitable pre-registration exit observation, fail-conservative membership errors, and deterministic member-PID recycle immediately before graceful HUP, force STOP/KILL/CONT, and liveness counting through the production stable-member algorithm; engine completion/containment tests plus `runtime_state_test.zig` and `runtime_failure_test.zig`: state freeze and broker-independent containment; `src/service/__tests__/event_delivery_test.zig`, `lifecycle_request_test.zig`, `runtime_aggregate_lifecycle_test.zig`, and `runtime_aggregate_cleanup_test.zig`: handoff, dispose, synchronization, exact cleanup phases, and fair retry; `src/transport/__tests__/transport_maintenance_test.zig`: brokerless maintenance                                                                                                                                                                                                                                                        |
| `TH-010`, `TH-011` | `src/service/__tests__/outgoing_test.zig`, `projection_test.zig`, `interaction_transaction_test.zig`, and `event_delivery_test.zig`: prefix, reservation, rollback, serialized commit, scheduler fairness and generation- and fence-guarded provisional rejection; `src/engine/__tests__/selection_export_test.zig`, `history_rows_test.zig`, and `history_export_test.zig`: row/export identity, rollback, and ABA rejection; `src/service/__tests__/input_request_test.zig`: exact `decoder-rejected` flight abandonment and a freshly captured S+1 snapshot                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `TH-012`           | Protocol limit tests; `src/engine/__tests__/input_queue_test.zig`, `runtime_input_test.zig`, `runtime_state_test.zig`, `projection_test.zig`, and `history_rows_test.zig`; `src/service/__tests__/projection_test.zig`: batch-full retry of the exact unsent projection; `src/service/__tests__/connection_request_test.zig`: payload-shortened inventory pages, exact emitted-count cursor advancement, and reply reservation failure; `src/__tests__/protocol_conformance_test.zig`: complete TypeScript/native limit and snapshot-reason equality                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `TH-013`           | Engine startup/containment unit tests plus `runtime_state_test.zig` and `runtime_failure_test.zig`: ownership publication, partial startup, and contained destruction; platform process tests: failed exec, POSIX stable-reference release, deterministic PID-recycle containment, and Windows containment; `src/service/__tests__/runtime_aggregate_cleanup_test.zig`: all-runtime shutdown and retained cleanup; `src/transport/__tests__/local_endpoint_test.zig`: endpoint and connection cleanup                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| `TH-014`           | Control decode/wire tests, protocol registry tests, and `src/__tests__/protocol_conformance_test.zig`; `src/service/__tests__/lifecycle_request_test.zig`: malformed, stale-epoch, unknown-optional, and role/encoding rejection before dispatch                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `TH-015`           | `src/engine/__tests__/runtime_state_test.zig`: Ghostty BEL effect counting; `src/service/__tests__/interaction_delivery_test.zig`: per-attachment baseline, ordered delivery, batch-full retry, and destruction behavior; Desktop and Web projected-terminal tests: exact scoped forwarding and client deduplication                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `TH-016`           | `src/engine/__tests__/runtime_state_test.zig`: application clear/reset semantics and counter-exhaustion rejection; `src/service/__tests__/runtime_aggregate_lifecycle_test.zig`: input-capability authorization and exact Core observation synchronization; `src/__tests__/protocol_conformance_test.zig`: both control operations and exact accepted replies                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `TH-017`           | `src/engine/__tests__/runtime_state_test.zig`: OSC 52 read/selection/base64/UTF-8/control rejection, exact 16 KiB boundary, first-pending ownership, retry identity, exact private-APC admission, public OSC/unrelated-prefix rejection, same-batch signal FIFO order and bound, and progress set/remove projection; `src/engine/__tests__/program_status_test.zig`: OSC 7501 chunk-independent scanning, terminators, bounds, pair grammar, field limits, root-only retention, stream-ordered query replies, report time, and prompt, RIS, Host-reset, and clear removal; `src/service/__tests__/event_delivery_test.zig`: real reader-batch private-APC application signal through control-event commit and consumption; `src/service/__tests__/interaction_delivery_test.zig`: clipboard outgoing-full rollback, one-recipient routing, consumption, and reconnect non-replay; protocol and Desktop tests cover the bounded signal event, strict v1 launch/correlation interpretation, clipboard event, and exact automatic application decision                                                                                                                                                                                                                                            |
| `TH-018`           | `src/model/__tests__/theme_test.zig` and `src/core/__tests__/theme_revision_test.zig`: complete state validity, compare-and-set, exact replay, stale/conflict, overflow, and no-mutation rejection; `src/engine/__tests__/runtime_theme_test.zig`: atomic physical replacement, mode 2031 scheme reports, an unqueued notice dropped while the theme applies, palette-only silence, OSC override/reset, RIS/application-reset priority, failure preservation, and two-attachment revision agreement; `src/service/__tests__/runtime_aggregate_theme_test.zig` and `connection_request_test.zig`: deferred/running cross-owner commit, reply preallocation, recovery inventory, replay/stale/conflict/failure results; `src/__tests__/protocol_conformance_test.zig`: exact TypeScript/native theme request, result, state, and registry agreement                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| `TH-019`           | `src/core/__tests__/registry_broker_test.zig`: revision initialization, exact update/replay, stale/gap/conflict rejection, immutable attach replay, and invariants; `src/service/__tests__/input_request_test.zig`: two-attachment held ownership, target-only downgrade repair, prepared-input final reauthorization, queued-input preservation, disabled/stale wire rejection, fresh upgrade ledger, and current-revision acceptance; composition state-machine traces and protocol conformance cover revision visibility and exact cross-language bytes                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `TH-020`           | `src/engine/__tests__/runtime_input_test.zig`: two-attachment aggregate focus transitions, duplicate claims, retained-ledger visibility reset, ordered key/mouse/focus repair, precharged final release, stale prepared-input rejection, and repair-failure containment; `src/service/__tests__/input_request_test.zig` plus composition traces: stale/paused rejection, demand-loss repair, detach and downgrade ordering, activation/upgrade without replay, and invariant agreement; `src/service/__tests__/terminal_integration_test.zig`: deterministic real-PTY two-attachment focus trace; headed `terminal-parity/IN-14-shared-focus`: two production projected surfaces and the application command trace                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `TH-021`           | `src/engine/__tests__/terminal_compatibility_test.zig`: disabled Glyph support/registration without reply or glossary mutation; bounded, chunked, multi-name, unsupported, malformed, and ordered XTGETTCAP/DECRQSS behavior against the fixed compatibility corpus                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| `TH-022`           | `src/engine/__tests__/runtime_input_test.zig`, `runtime_state_test.zig`, `runtime_mode_effects_test.zig`, and `projection_test.zig`: mode-20 expansion and post-expansion limits, mode-1004 immediate aggregate reporting and stale preparation, mode-2026 split-batch suppression/reset/resize/timeout, launch and same-canonical viewport replacement, mode-3 DECCOLM followed by mode-40 set/reset/saved restoration, parser-order convergence, serialized PTY failure, mode-2048 exact reports, reply-budget containment, and complete grid/cell-pixel geometry; `src/service/__tests__/runtime_aggregate_lifecycle_test.zig`: Core adopts a newer terminal-driven viewport restoration without overwriting it                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `TH-023`           | `src/engine/__tests__/projection_test.zig` and `history_rows_test.zig`: pinned-Ghostty direct/file/temporary/shared-memory, raw/PNG/zlib/chunk, virtual/relative/crop/layer, animation-generation, live/history capture, deletion/reset/eviction, bounds, and source-independent differential cases; `src/service/__tests__/projection_test.zig`: monotonic due wake, one revision per changed tick, stutter, in-flight invalidation, retry, and two-attachment agreement; protocol conformance plus `src/protocol/__tests__/graphics_extension_test.zig`: exact signed-z OBKG and render-detail OBRW bytes; terminal store and both renderer backends: atomic cache replacement and three paint seams; controlled implementation tests: transition/refinement checks                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| `TH-024`           | `src/engine/__tests__/runtime_state_test.zig`: non-blocking lock contention, exact idle deadline, one-page-per-step bound, pending-to-complete convergence, cold-page backing savings, same-line output and prepared client-input idle restart, dormant interaction absorption, and unchanged revision/generation; `src/service/__tests__/runtime_aggregate_maintenance_test.zig`: global cadence, round-robin fairness, and no catch-up burst; controlled implementation tests: scheduling and representation-only refinement checks                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| `TH-025`           | `src/service/__tests__/host_lifecycle_test.zig`: every forward-only transition, terminal admission and end conditions, the `host-draining` refusal while a replayed creation is still answered, and the end after the last disposal; `src/transport/__tests__/candidate_test.zig`: `drain` and `exit` on the dispatch thread before the Worker, and on a control connection during a live Worker session, ending the session and the serve loop                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| `TH-026`           | `src/engine/__tests__/prompt_gate_test.zig`: every refusal reason, `settling` as the last rule without a wait, publication-time refusal, and no reservation on refusal; `prompt_gate_witness_test.zig`: real cmd and managed PowerShell on system ConHost and the pinned bundle; `src/service/__tests__/input_request_test.zig`: the `prompt-unconfirmed` reply and its `reason` member.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |

The deterministic commit-seam set is: creation preparation/spawn/admission; deferred-start spawn;
control-reply
reservation; raw and semantic queue reservation/publication with final capability reauthorization;
attachment input-ledger admission, focus aggregation, visibility repair, capability
downgrade/upgrade, and detach-repair materialization/publication/failure containment; application terminal-control
validation and canonical mutation; resize effect and unsafe rollback;
mode-effect reply admission, focus-mode revalidation, and synchronized-output deadline release;
graphics media acquisition/cleanup, animation advance, placement resolution, live/history provisional
capture, and atomic resource-placement commit; projection capture/encoding, provisional payload
adoption, and serialized read commit; row, history, selection, and export provisional mutation; batch
prefix/full behavior; lifecycle outcome
handoff; Local IPC partial write; process observation and containment; multi-runtime shutdown;
mouse-input epoch revalidation and unstarted-hover retirement; and runtime-theme physical
application before semantic commit.
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
| `CS-026` | `ModeEffectReplyAdmission`             | contain and retain           | single owner               |
| `CS-027` | `FocusModeRevalidation`                | reject without commit        | deterministic interleaving |
| `CS-028` | `SynchronizedOutputDeadlineRelease`    | preserve for retry           | deterministic interleaving |
| `CS-029` | `MouseInputEpochRetirement`            | reject without commit        | deterministic interleaving |

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

The opaque runtime handle composes a private `process_handle.zig` adapter for process sampling, completion and termination. This adapter only converts the handle to private State and delegates to the existing process/lifecycle owners; it introduces no state, ordering change, or new process authority. This keeps process-facing growth out of the general runtime facade. A private `schedule_handle.zig` adapter owns the projection scheduler's deadline and capture readiness.

## Interactive PowerShell reader markers

On Windows, interactive PowerShell launches receive the Local Runtime's shared, process-local
`-NoExit -Command` bootstrap after normal profile execution. Worker-prepared argv is interactive
only when the command operand exactly equals the shared common-initialization and reader assets,
preceded solely by admitted interactive flags including `-NoExit`. The Host retains that argv
without injecting it again; plain interactive argv receives the same script. Modified scripts,
extra operands and arbitrary `-Command`/`-File` launches remain programs. The common initialization
preserves UTF-8 and invokes the original profile prompt before synchronizing process cwd. The
reader extension requires standard PSReadLine module/export ScriptBlock identity and closed
SHA-256 definition fingerprints for the two-argument 2.0.0 and status-aware three-argument 2.4.5
wrappers, normalizing only CRLF and outer whitespace. Unknown or custom definitions remain
untouched and emit no markers. Its invoked child scope keeps skipped or repeated installation
from returning out of the surrounding initializer. The wrapper captures `$?` as its first
statement, retains StrictMode/debugger behavior, then emits P/B at the already-rendered prompt's
input boundary. P marks prompt semantics without A's fresh-line action, so reader entry preserves
the cursor and prompt text. Windows startup, RIS and ResetTerminal default to retaining prompt
cells during resize because PSReadLine/ConPTY does not redraw them on every geometry change.
An explicit OSC 133 `A;redraw=...` still overrides that default until reset. Deterministic stream
and application-reset checks cover preservation, explicit redraw and default restoration.
The legacy reader delegates to
the original ScriptBlock; the recognized modern adapter calls native ReadLine with saved status
and the original module execution context. Both emit C in finally and preserve input/errors.
It preserves prompt output (including mise), PSReadLine key hooks, the user's history handler and all profile files.
Commands, scripts and unknown shells receive no bootstrap. Missing or restricted reader
support emits no markers. The reader
hook boundary also appears in [VS Code's PowerShell integration](https://github.com/microsoft/vscode/blob/main/src/vs/workbench/contrib/terminal/common/scripts/shellIntegration.ps1);
Orchbee adopts read-phase markers and the bounded correlation described below; it does not report command text or environment, or remap keys.

The recurring `powershell-prompt.test.mjs` check covers reader output/exceptions, custom-module
refusal, initializer continuation and preserved hooks.
The bundled-ConPTY cursor suite also checks real Windows PowerShell and available PowerShell 7
startup, resize and draft input without moving the insertion position off the prompt line.
A real PSReadLine prediction-feedback witness checks failed/successful command status at the
native ReadLine boundary.

## Prompt-gated input

`PromptLineInput` types one shell command line only while the Host can confirm that the
terminal's root shell waits at a fresh, empty prompt; the
[protocol reference](../../../schemas/terminal/references/protocol.md) owns its payload, its seven
rules, and its refusal reasons. The terminal mutex owns one prompt-gate tracker. Reader effects
record each OSC 133 mark in stream order before Ghostty applies it: a primary prompt start (`A`,
`N`, or `P` with no kind or `k=i`) starts a candidate, and its input start (`B` or `I`) anchors it.
The anchor is a tracked Ghostty pin at the cursor, its pending-wrap state, the screen key and
generation, a digest of the cells from the pin to the end of the logical line, the monotonic mark
time, and the client input facts read under the input mutex. Trailing cells without text stay out
of the digest: a resize adds or removes them, and the pinned bundle and system ConHost leave
resized cells empty rather than repainting them, so an idle prompt stays confirmable after a
resize while its line fits both widths. Every written character counts, a space included.
Narrowing below the prompt line re-wraps it; OpenConsole and Ghostty may then disagree on the
cursor row, and the gate refuses while the cursor does not match the anchor. A command start or
end, a newer prompt start, a continuation or secondary prompt, or an input start without a
candidate leaves the prompt; a right prompt before the input start does not. Every transition
releases the previous pin and advances a serial.

Client input facts are a published-input sequence and the count of client queue entries reserved
or not yet written. The sequence advances when any client entry other than a focus report is
published; Host terminal replies are a separate source. Preparation checks, under the terminal
mutex, the launch record, a synchronous process sample (a failed sample refuses), the screen,
the marks, the input facts, the anchor, and last the quiet period
(`protocol.Limits.prompt_line_quiet_milliseconds`, 100 ms), in that order. The Host never sleeps:
its dispatch thread applies requests one at a time, so a wait would stall input and resize for
every terminal it serves. A prompt younger than the quiet period that passes every other rule is
refused at once as retryable `settling`, and the sender waits and retries off the Host; an anchor
without a mark time refuses as changed. An admitted line reserves one client FIFO
position holding the paste, an optional Right press and release, and an Enter press and release,
encoded under the same terminal lock. Publication takes the terminal and input mutexes and repeats
every rule except the process sample, with the reservation counted as the only pending entry. It
also requires the serial seen at preparation and no reader commit in progress, so a mark parsed
after preparation refuses. A refusal reserves nothing, and an accepted line is itself input after
the anchor, so a second line waits for a new prompt. The line mutates no input-ownership ledger.

On Windows, an interactive cmd launch receives prompt marks through its own environment. The
adapter wraps the `PROMPT` of the environment it builds for the child, launch values included, or
cmd's default `$P$G` when it is unset or empty, between `$E]133;P$E\` and `$E]133;B$E\`. A value
already wrapped by an outer Orchbee cmd is kept, and no registry value, AutoRun entry, or file
changes. `P` has no fresh-line action, so the prompt text and cursor stay where cmd draws them.
cmd has no preexec and sends no `C`: a command it runs keeps the old anchor, and the typed command
already advanced the input sequence. After `echo off`, or after the user changes `PROMPT` in the
session, prompts carry no marks, so every later line is refused. A nested shell inherits the
wrapped value but runs as a child the process sample sees. On current Windows `cmd /Q` still draws
its prompt and admits lines.

The adapter records the launch kind beside the interactive-shell classification:
`managed-powershell` for PowerShell receiving the managed startup, `cmd` for an interactive cmd
receiving prompt marks, and `other` otherwise and on every POSIX launch. `ReadProcessInfo` reports
it only when the request asks with `includeLaunchKind`. The cmd switches `/D`, `/Q`, `/A`, `/U`,
`/V:ON`, `/V:OFF`, `/E:ON`, and `/E:OFF` keep cmd interactive, matching the Client's shell-root
classification; any other argument makes it a program.

The deterministic decision table in `src/engine/__tests__/prompt_gate_test.zig` feeds marks, text
and input to a gated runtime and asserts each rule's refusal, `settling` without a wait and only
after every other rule, and publication-time refusal.
`src/engine/__tests__/prompt_gate_witness_test.zig` runs the same real-shell witness on system
ConHost in the engine suite and on the pinned bundle in the bundled-ConPTY suite: cmd,
managed Windows PowerShell and, when installed, PowerShell 7 admit only an idle prompt and refuse
during `set /p`, `pause`, `Read-Host`, `Start-Sleep`, a half-typed line, and after `echo off`;
cmd created at 80x24 still admits a line after widening to 109x39 and narrowing to 90x30. A
host whose first prompt shows no marks must refuse every line; the pinned bundle must show them.
The witness submits each line as its sender does, waiting out one `settling` refusal off the Host.
PowerShell starts with `-NoProfile`, every line the test types carries the Orchbee run marker, and
the first one turns PSReadLine history saving off.

## Correlated shell observation and row origin

The runtime's terminal mutex owns one `command_observation` tracker. Its oldest-row tracked Ghostty pin fences numeric row coordinates: eviction, screen replacement or origin movement increments a checked positive output epoch. Allocation failure blocks capture; epoch exhaustion never wraps. Projection generation separately fences geometry/reflow. There is no retained command history.

Private APC `orchbee.command.v1;ready`, `start;<32-lowercase-hex-id>` and `finish;<id>;<i32-status>` are bounded cooperative Shell facts. Start records generation, epoch, cursor coordinates and a real-time clock read taken after the marker's bytes arrived; only a matching unfinished invocation accepts finish. Semantic C clears ready. Reader batches stage and commit these facts with their terminal content. OBTF optional kind 5 publishes them and kind 6 the start time; old consumers can skip both. These markers grant no input, focus, close or process-lifecycle authority.

The managed PowerShell reader strips only `<#orchbee-run:<id>#> ` at the beginning of the returned command and keeps that line out of PSReadLine history; the prompt also removes its session-history entry. The prompt captures `$?` before integration work, emits D and a matching finish with 0/1 status before rendering the original prompt, then clears the ID. The original command runs in its normal scope. Readiness advertises integration capability and current reader entry; it does not reserve the input line against other writers and is not required by [prompt-gated input](#prompt-gated-input), which relies on the reader's OSC 133 marks because the system ConHost drops private APC.
