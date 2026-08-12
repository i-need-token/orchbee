# Desktop

Sandboxed Electron product client and scoped local broker for Orchbee's native Terminal Host.

## Current scope

Desktop composes the shared Workbench with Host-backed projected terminals. Workbench owns Project, Workspace, Tab, Panel, and `TerminalPresentationId` state; Electron Main owns durable technical bindings and Electron-only privileged effects; `apps/terminal-host` owns PTYs, processes, Ghostty VT state, history, and real terminal identity. A Browser client commits clipboard and allowlisted link effects in its own page. None of the shared Workbench document is stored in the Host.

Main starts without creating a fixed terminal, opening an attachment, or starting a Web listener. It restores the durable `TerminalPresentationId` to `TerminalBindingId` registry, owns one process-wide canonical Workbench shared-document coordinator, and transfers one atomic allowlisted capability bundle through the sandboxed preload. Its four ports have one fixed order:

- client-session control delivers one atomic document/terminal-observation bootstrap, accepts complete exact-next shared-document proposals, and publishes one ordered canonical event stream;
- projection control opens and releases scoped Host attachments only for displayed terminal viewports;
- application-lifetime control carries only exit reason, user choice, quiescing acknowledgement, shared-document revision, aggregate progress, and typed failures for the current renderer page;
- Browser-access control carries only explicit start, mode-switch, open, copy, stop, sanitized Origin state, and bounded failures. It never carries the capability-bearing URL.

The client-session and projection payloads are platform-neutral contracts from `@orchbee/workbench-client-protocol`. `@orchbee/desktop-client-protocol` owns the atomic Electron renderer bootstrap, Electron application-lifetime contract, and narrow Browser-access command contract. Desktop Main and the Web renderer each adapt the generic contracts to their end of a `MessagePort`; neither transport detail enters the shared Workbench client runtime.

The renderer has no Node integration, generic Electron IPC, filesystem capability, Host inventory, or raw terminal-lifetime authority. It cannot invoke provision, restore, close, or storage CAS effects directly. Preload accepts exactly one bootstrap, verifies all four ports together, preserves their fixed client-session/projection/application-lifetime/Browser-access order, and attests context isolation and sandboxing. A partial or malformed bundle starts no renderer runtime.

Main loads and validates the shared document once before admitting clients. An available document restores Project, Workspace, Tab, Group, Panel/Split structure, presentation placement, and lifecycle intent; every page derives a fresh in-memory Client View. Active navigation, disclosure, focus, maximization, active child, and Split weights do not enter the shared file. A missing document projects every recovered binding as a standalone `retain` Tab and gives each joining page the same revision-`0` initialization seed. The first accepted proposal persists that seed before Main performs restore/provision effects. An unavailable load or failed first commit creates no replacement document or terminal effect. Unreferenced technical bindings remain preserved but are not silently adopted into the shared document.

The coordinator is transport-neutral and supports independent page sessions without a product count ceiling. Electron uses a direct `MessagePort` adapter. Explicit Browser access starts `apps/server` and `apps/gateway` as on-demand Utility Processes; Desktop owns both lifetimes and allocates exactly one client-session/projection port pair per authenticated page. Server owns static renderer delivery, while Gateway owns exact-Origin admission, capability authentication, framing, and transport backpressure on loopback or an explicitly selected trusted LAN. Gateway never receives an application-lifetime port or a Host connection. `apps/web` owns the platform-neutral Workbench client runtime and the single renderer application used in an ordinary browser, a Gateway Browser client, and Electron. Development Electron is discovered from its User-Agent on the loopback Web origin. Packaged Desktop registers the standard secure `orchbee:` scheme before Electron becomes ready, serves the Web build from `orchbee://app`, and falls back to `index.html` for Browser Router document requests. Desktop owns renderer resource delivery, not another renderer source tree. See [`docs/multi-client-session-coordinator.md`](docs/multi-client-session-coordinator.md) and [`../web/docs/navigation-routing.md`](../web/docs/navigation-routing.md).

Desktop requires the single current Host protocol `0.0.5` exactly; pre-release builds do not negotiate or document historical protocol variants. Electron uses `42.8.0`, the final release in the major immediately preceding the current Electron major selected for this repository.

`HostConnection` owns one connection-global outbound window across every broker caller: at most 64 unanswered request records and 4 MiB of encoded request bytes. It retains that admission until the correlated response or connection failure. A full window or a Node writable that has returned `false` rejects work before handoff with retryable `host-transport-backpressure`; no request write resumes before `drain`. If a handed-off request reaches its deadline, Desktop retires the connection and rejects all pending and future requests with non-retryable `host-request-outcome-unknown`, so input is never replayed after an ambiguous result. The broker's 64-command per-attachment window remains an additional capability-local guard.

Before it constructs `HostConnection` on POSIX, Desktop asks the packaged native Host helper to validate the runtime directory and actual socket, connect, and authenticate the server's effective UID from kernel peer credentials. Only an authenticated same-UID connection can suppress Host launch, and every retry or later Desktop reconnect repeats the check. The helper is a bounded stdio proxy for that one connection; its exit cannot own or terminate the retained Host process, and its transport preserves `HostConnection` admission and unknown-outcome behavior unchanged.

Desktop scopes one Terminal Host endpoint to its product `userData` identity, which separates OS users, product profiles, and release-channel data roots. The short POSIX runtime path is a stable product-scoped child of the operating system's trusted per-user runtime location (`/run/user/<effective-uid>` on Linux and Electron's per-user temporary location on macOS), never a deterministic child of shared `/tmp`. The Windows named-pipe identity is derived from the same product scope. Explicit endpoint and runtime overrides exist only for controlled development and tests; POSIX overrides still pass through native no-follow owner, private-mode, and ACL preparation and validation. After every successful handshake, Desktop sends the current terminal default through `SetTerminalDefaults`, so reconnecting to an already-running singleton has the same result as a fresh CLI launch. New terminals use a light profile aligned with the Workbench canvas; retained terminals preserve the profile stored with their existing Host session. Product settings UI and durable preference storage remain a later Desktop-owned slice.

## Renderer document lifecycle

Electron Main owns the renderer-document generation and its client-session, projection, and application-lifetime controls. Electron `did-start-navigation` details are the trusted navigation input; Main classifies them with one deterministic policy before changing that generation:

| Main frame | Same document | Result                               |
| ---------- | ------------- | ------------------------------------ |
| Yes        | No            | Start a renderer document generation |
| Yes        | Yes           | No-op                                |
| No         | No            | No-op                                |
| No         | Yes           | No-op                                |

A renderer document start retires the previous page controls and records the pending generation. `did-finish-load` publishes one fresh atomic bootstrap only for the latest generation; retirement failure keeps that generation closed. Same-document Browser Router updates and subframe navigation preserve the current generation and every live control. Renderer-process loss and window close remain explicit retirement paths, while Host terminals and the canonical shared document survive page retirement. This model does not add projection retries, control-port timeouts, or a new IPC contract.

`startsDesktopRendererDocument` owns the pure decision. `DesktopApplication` owns generation transitions and Electron effects. Unit tests cover all four decision rows and full navigation/reload sequencing; `DesktopRendererDocumentLifecycle` checks ignored-navigation preservation, current-generation control ownership, retirement before replacement, and bounded pending-generation progress through generation `2`. Electron 42.8.0 supplies the navigation classification and event order; operating-system termination before Electron emits an event remains outside this claim.

## Browser access preview

Desktop owns an opt-in Browser-access workflow for the same canonical Workbench and terminal
sessions. Browser access is stopped at process start. Choosing **This Computer** starts the trusted
Web renderer Server and authenticated Gateway on ephemeral `127.0.0.1` ports. Choosing **Local
Network Preview** is a separate explicit action that binds both listeners to `0.0.0.0`, while
publishing only detected RFC 1918 IPv4 origins. The unspecified listener address is never a client
URL. Public interface addresses, IPv6, hostnames, discovery, Internet ingress, and background
service installation are outside this preview.

The complete access URL contains one process-lifetime high-entropy capability in its fragment.
Desktop Main owns that secret and performs copy and system-Browser effects itself. The sandboxed
renderer receives only the selected mode, lifecycle phase, and non-secret HTTP origins; it never
receives the fragment, Gateway port, capability, child-process handle, or arbitrary clipboard and
URL capabilities. Anyone who obtains a copied preview link can observe and control the shared
Workbench and terminals until Browser access stops. HTTP traffic is unencrypted. The UI labels and
warns about that accepted preview boundary; TLS, login, durable credentials, revocation finer than
process lifetime, and production remote-client protocol are non-goals.

The workflow has one authoritative Desktop Main owner and the following serialized transitions:

| Current phase      | Intent or result                 | Next phase                                           |
| ------------------ | -------------------------------- | ---------------------------------------------------- |
| `stopped`          | Start one selected mode          | `server-starting`                                    |
| `server-starting`  | Server owns an ephemeral port    | `gateway-starting`, or `stopping` if intent changed  |
| `gateway-starting` | Gateway owns an ephemeral port   | `ready`, or `stopping` if intent changed             |
| `ready`            | Same-mode start                  | No-op; reuse the capability and listeners            |
| `ready`            | Stop or choose the other mode    | `stopping`                                           |
| Any starting phase | Allocation or validation failure | `stopping`                                           |
| `stopping`         | Both owned resources are retired | `stopped`, then start a queued mode when one remains |
| Any live phase     | Desktop shutdown                 | `stopping`, then terminal `disposed`                 |

At most one Server and one Gateway belong to a Desktop process. A ready Gateway always has its
matching Server. Failed or superseded starts clean partial resources before another start, and
Desktop shutdown revokes the capability even if a renderer page has disappeared. The Server owns
static HTTP delivery only. The Gateway owns exact-Origin admission, capability authentication,
framing, and page sockets. Desktop Main remains the only writer of shared product state and grants a
fresh page-scoped client-session/projection pair only after authentication.

`DesktopBrowserAccessLifecycle` checks resource ownership, single-mode readiness, cleanup, and
eventual capability revocation under fair completion of owned process effects. Deterministic unit
tests cover address policy, same-mode coalescing, mode replacement, partial-start compensation,
copy/open confinement, and shutdown. Real Server/Gateway tests cover both bind modes, exact-Origin
rejection, wrong-capability rejection, and remote-host WebSocket construction. The packaged system
path verifies the four required resources and one Electron plus Browser shared-session trace.

## Lifecycle and identity ownership

The maintained identity chain is:

```text
Workbench TerminalPresentationId
        │ committed shared-document intent
        ▼
Desktop TerminalBindingId ── durable binding store
        │ recovery reconciliation
        ▼
HostEpoch + CreationId + TerminalId ── Terminal Host authority
        │ visible viewport demand
        ▼
AttachmentId ── one disposable projection capability
```

`TerminalPresentationId` is created and retired by Workbench. Main maintains a strict in-memory bijection to `TerminalBindingId`; it never exposes Binding, Creation, Host, Terminal, or Attachment identity through lifecycle control. The Host remains the source of truth for the process and VT.

The version-3 binding store contains a canonical BindingId-ordered collection. Every record has one unique BindingId, PresentationId, CreationId, bounded launch profile, and exactly one durable state: `planned`, `create-requested`, `retained`, `exited`, `closing`, or `lost`. Retained identities are unique and must repeat their owning CreationId. The file is parsed as untrusted input, updated through owner-only atomic replacement, and never rewritten automatically when invalid or unreadable. Launch environment values never enter diagnostics. Desktop preserves an explicit UTF-8 locale; when inherited POSIX locale precedence would not produce UTF-8, Main removes a conflicting `LC_ALL`, repairs an invalid `LANG`, and supplies a UTF-8 `LC_CTYPE` fallback as needed. Darwin uses `en_US.UTF-8` because its shell environment does not provide Linux's `C.UTF-8` locale.

No product maximum is imposed on terminals, bindings, or attachments. Active collections grow from shared product structure and visible demand; allocation, filesystem, Host, or memory exhaustion returns an operation failure instead of evicting or silently truncating another terminal. Host inventory is read through generation-scoped protocol pages rather than one bounded global list. Pre-association `TerminalExited` delivery uses a bounded identity-keyed admission buffer: admitted identities are never evicted, and an identity that has not acquired its binding association is retried before the adapter buffer is released. After the Host envelope is accepted, Desktop Main synchronously records the event in an owner-only durable pending journal before the fire-once Host listener returns. The separate 64-entry identity-keyed in-memory queue is only a scheduler/cache; it is not the crash recovery owner. The pending journal has its own 4096-entry capacity, while the restart-safe publication ledger and durable rejection journal have independent capacities. Journal-full or write failure is recorded as an observable durable rejection when possible, and the admission path fails closed if that rejection cannot be recorded. Unknown identities stay in durable pending ownership without a timer loop; an exact identity observation wakes promotion. Queue-full, shutdown, and observed pending entries may be promoted by the retry owner. A pending entry is removed only after successful durable runtime recording, adapter publication commit, publication-ledger admission, reconciler acceptance, and live publication; compaction failure therefore leaves durable recovery state. Shutdown transfers unresolved in-memory entries to the pending journal before completing the remaining application teardown.

The terminal-exit spill store migrates legacy v1 files with a bounded event array, normalizing each identity to its newest revision and atomically rewriting the file once. Legacy pending records remain v2 pending ownership. Legacy published records are discarded only after the already-loaded durable publication ledger confirms the identity and revision; without that evidence the startup migration fails closed and leaves the v1 file untouched. Oversized or malformed legacy input also fails closed.

`TerminalFailed` is a different result, not a synthetic exit. Desktop keeps an identity-keyed association buffer only while a binding is still acquiring its Host identity, then records the failure on that binding before exposing another projection. The failed terminal remains retained and authoritative in the surviving Host, so Desktop restart recovers the same typed failure from Host inventory; it is not copied into the exit-specific spill store or publication ledger. This does not promise recovery after a Host or operating-system crash.

The separate Workbench shared-document store accepts only the canonical version-1 document through one Main-owned serialized writer. A missing store starts at revision `0`; every later commit must name the exact current revision and advance it by one. Conflicts, non-next revisions, invalid files, and read failures preserve current data. Writes use owner-only temporary files, file synchronization, atomic replacement, and parent-directory synchronization where supported. A client receives only an atomic bootstrap, typed proposal result, and ordered publication; it never receives a path, Node error, store capability, or general filesystem capability. See [`docs/workbench-shared-document-storage.md`](docs/workbench-shared-document-storage.md). The store uses `workbench-shared-document-v1.json`. It has no reader, alias, or migration for the unshipped development topology file; that old path is ignored and not deleted automatically.

Provisioning allocates and persists a fresh binding before creating a terminal. Recovery accepts only an exact HostEpoch/TerminalId/CreationId match and never substitutes a new shell for lost continuity. A lost create response may replay only the same CreationId and launch in the recorded Host epoch. Orphan, conflict, corrupt-ledger, and transport states fail closed.

Retain reconciliation is a separate restore-only operation. It succeeds only for a Binding that was already durably retained or exited, or became restorable after successful provision in the same Main lifetime. Missing, planned, closing, or lost bindings never allocate a replacement through the restore path. Restore failure remains durable retain intent.

One operation tail serializes lifecycle work per presentation while unrelated terminals progress independently. The Main coordinator persists and publishes the complete shared document containing provision, close, or stop intent before deriving the corresponding effect. Its FIFO proposal queue uses first-writer-wins for divergent concurrent complete documents and converges equivalent races. Close then persists technical `closing`, retires only that presentation's active projections, and delegates graceful/force/dispose behavior to the existing binding close lifecycle. A Panel or Tab remains in Workbench until every exact terminal close result completes; cleanup commits afterward, while the previously stored close intent remains replayable if cleanup fails. Renderer reload, a renderer crash, a normal macOS window close, and uncontrolled Main loss release that page's projection attachments but preserve Host terminals. A controlled application quit follows the policy below.

## Application lifetime

A normal macOS window close releases only the renderer page. Command+Q, a menu quit, or closing the last window on Windows and Linux enters one coalesced user-quit workflow while the renderer is still available. When terminal sessions exist, the application-wide dialog offers Keep Sessions and Quit as the default, Stop Sessions and Quit as the destructive choice, or Cancel before any stop effect. With no sessions, Desktop flushes the shared document and quits without prompting. The future updater enters the same preserve preparation as `update-restart` without prompting; updater installation, signing, and release distribution remain outside this package.

Before preserve or stop preparation, Main acknowledges that the application-wide coordinator has entered quiescing. This acknowledgement orders the lifetime and client-session ports: other pages cannot race a new shared change into the preparation window. A replacement renderer may resume only the same quiescing mode after the former exit owner disconnects.

Stop seals terminal creation for the exit-owner page, projects every visible and hidden Workbench presentation to durable `stop`, compare-and-swaps the complete shared document, and only then releases close commands. Successful close results change presentations to `stopped` while retaining Project, Workspace, Tab, Panel, split, title, and launch CWD. Hydration never starts a stopped presentation. Its visible Start New Session action replaces the stopped PresentationId with a fresh identity and provisions only that selected presentation.

After referenced presentations stop, Main loads the exact committed shared-document revision and closes every binding that it owns but the document does not reference. It then requires the binding registry to be empty and scans Host inventory. Unknown Host terminals are reported as unresolved; they are never silently killed. A partial failure keeps Desktop in an irreversible recovery dialog with Retry when supported and an explicit Quit Anyway warning. Quit Anyway may leave processes running; the durable `stop` intent remains replay authority for the next start. OS shutdown, logout, and forced termination are not claimed by this workflow.

## Demand-driven projection

Workbench mounts the application-owned terminal surface only inside displayed panel viewports. The surface requests projection only while its lifecycle is `running` or `exited`; unmounting, switching Tabs, hiding another terminal context, or entering `closing` releases the request. Background Projects, Workspaces, Tabs, inactive child terminals, and otherwise invisible panels therefore hold no renderer, GPU surface, frame stream, history cache, search Worker, or Host attachment.

Before the renderer document paints, Electron fills the native window with the default background
from the same launch profile sent to Terminal Host. That value is only a startup bridge: committed
Host frame metadata owns the live terminal background in both Electron and Browser clients, so
Desktop Main never maintains a second runtime terminal palette.

Each successful request receives a fresh AttachmentId, capability bootstrap, and private `MessagePort`. For one running presentation, serialized opens grant read/input/resize to the first successful attachment and read/input without resize to later observers. Existing observers are not promoted when the owner detaches; the next new attachment may acquire resize. Exited and failed terminals attach read-only with their retained final revision and typed outcome. Multiple visible split panels and page clients may hold independent attachments, with no global attachment threshold.

`HostConnection` reads and validates the fixed terminal-frame header once, then routes the binary record directly by AttachmentId. Main performs no VT parsing, row reconstruction, JSON/base64 conversion, or per-cell pass. At the Electron capability boundary, the broker copies only the routed payload into an exact-size `ArrayBuffer` before structured clone, so envelope headers and coalesced neighboring records never cross the renderer port. OBPB format `0.0.1` local history render buffers then use the structural-view boundary; Gateway delivery remains externally validated. History, selection, copy, link, and accessibility behavior remains attachment-scoped. Normal bottom-following use creates no history cache or search Worker; these resources are activated only by their existing interaction contracts.

Electron attachments commit selection export through Main's clipboard and open allowlisted links through Electron. Gateway attachments receive bounded selection bytes, commit them through the Browser Clipboard API, and open `http`, `https`, or `mailto` targets through a local Browser gesture. The Browser never receives a generic Electron effect port. Attachment transport backpressure retires only that attachment; the visible surface requests a fresh projection and full snapshot.

Known Host `TerminalExited` events enter the Main-owned retry/reconciliation path before they are resolved through the authoritative runtime registry, persisted to the exact binding, and published to Workbench by PresentationId. An identity that remains unresolvable is retained as a durable, observable reconciliation record and is promoted only when its exact identity is observed (or by the retryable known-entry paths); it is not spun in an unbounded unknown-identity timer loop. A recovered exited binding is restored into Workbench, attached read-only, and retains its final-revision presentation behavior. The reconciler reservation, adapter commit, publication-ledger record, and visible event are separate steps: a bootstrap can see the event only after all durable and presentation ownership steps succeed. Durability is confirmed before the first exit publication is consumed.

The Workbench `terminal-presentation-event-published` message is the single Main-owned lifecycle publication. A typed Host failure becomes a non-retryable `terminal-failed` presentation outcome; the reconciler retains it for later client bootstrap, and Workbench keeps the terminal projectable and read-only until explicit close. `ProjectedTerminalBroker` may also forward an identity-checked, attachment-local raw `terminal-exited` or `terminal-failed` notification so a live projection can synchronize with Host state; that renderer transport has its own monotonic revision filter, is outside the coordinator sequence, and is not a second Workbench publication. The two channels therefore do not provide a cross-channel exactly-once claim.

## Correctness evidence

Focused traces cover:

- strict client-session and projection boundary validation, malformed-message retirement, exact proposal correlation, and page attachment disposal;
- atomic bootstrap, FIFO complete-document proposals, first-writer-wins races, stale rollback with Client View preservation, invalid-file preservation, and one ordered publication stream;
- first-commit and per-command write-ahead ordering, Main-owned restore-only recovery, failed-store effect suppression, and post-effect managed container cleanup;
- fresh provisioning, multi-binding store preservation, recovery, exited recovery, orphan and conflict handling, durable close, and presentation-registry reconstruction;
- unrelated terminal concurrency, exact close targeting, Tab-close result ordering, and stale result rejection;
- two recovered Workbench tabs where both lifecycle records reconcile but only the active visible Tab has an attachment, followed by exact release/open behavior on Tab switch;
- concurrent viewers with exactly one resize owner, observer non-promotion, and next-open ownership;
- fixed-header frame routing to only the matching AttachmentId, with exact-size structured-clone payload isolation;
- sandboxed end-to-end input, output, history, search, selection, copy, links, exit, renderer reload, and Electron Main restart;
- one Electron renderer and two ordinary Browser pages sharing canonical Workbench and terminal state while retaining fresh page sessions, Client Views, and attachment identities, including Browser copy, link, disconnect, rejection, and reconnect behavior;
- user-quit preserve/cancel/stop, hidden-terminal stop, failed-CAS suppression, partial-stop retry, unreferenced-binding drain, unknown-Host-orphan refusal, update-restart preserve, and fresh identity after explicitly starting a stopped session.

Existing TLA+ models continue to check terminal recovery, binding close, lifecycle-adapter serialization, projection demand, and Workbench removal safety within their declared finite bounds. The Desktop renderer-document model checks ignored same-document and subframe navigation, generation-scoped control replacement, and bounded load progress. The Desktop shared-document-store model additionally checks exact-next CAS, conflict preservation, serialization, unavailable storage, generation disposal, and progress through revision `2`. The Desktop multi-client model checks two sessions, missing/available bootstrap, equivalent and divergent races, effect publication ordering, attachment/session ownership, disconnect, quiescing handoff, unavailable storage, and bounded publication progress. The Desktop write-ahead model checks matching durable intent before provision, restore, close, or stop dispatch; failed-commit preservation; restore without replacement creation; renderer disposal and reload replay; and live-generation progress for one abstract presentation. The Desktop application-lifetime model checks prompt and update paths, stop-before-close ordering, unreferenced binding drain, cancellation before effects, clean-stop exit, and the explicit forced exit escape. Workbench's terminal-stop model checks layout retention, stopped-without-binding, retry identity, and fresh PresentationId generation. The repository imposes no new throughput acceptance gate for this integration slice; the optional benchmark remains a diagnostic tool.

## Development

Build the selected development Host, then start the canonical Web development server and Desktop together:

```sh
pnpm --filter @orchbee/terminal-host build
pnpm desktop
```

`pnpm desktop` builds the Browser-access Server, Gateway, and static Web resources, then runs the Web development server and Electron together. Electron defaults to `http://127.0.0.1:3000/`; an explicit loopback Web server can be selected with `ORCHBEE_DESKTOP_RENDERER_URL`. Open **Settings → Browser access** to start loopback access or explicitly enable trusted-LAN preview. Desktop never prints or returns the capability-bearing URL to renderer state; its Main process performs the Copy Link and Open Browser effects. Set `ORCHBEE_DESKTOP_GATEWAY=0` to remove Browser-access resources from a development launch.

Build an unsigned current-platform application bundle with:

```sh
pnpm desktop:package
```

The command builds the Terminal Host, Web renderer, Browser Server, Gateway, and Electron Main/preload bundles before packaging. The Web build is both a read-only `orchbee://app` resource and the on-demand Browser renderer root; the native Host, Server, and Gateway entries remain available outside `app.asar`. The output is generated beneath `apps/desktop/out/` and is not a signed installer or release artifact.

`ORCHBEE_TERMINAL_HOST_BINARY` selects an absolute development/test Host binary. `ORCHBEE_TERMINAL_SCROLLBACK_BYTES` selects the temporary development startup/default scrollback budget in decimal bytes from `0` through `1000000000`; the built-in default is `10000000`. `ORCHBEE_GATEWAY_ENTRY_PATH`, `ORCHBEE_SERVER_ENTRY_PATH`, and `ORCHBEE_WEB_RENDERER_ROOT` select explicit development Browser-access resources when the default sibling builds are not used. `ORCHBEE_DESKTOP_RUNTIME_DIRECTORY`, `ORCHBEE_DESKTOP_ENDPOINT_ID`, and `ORCHBEE_DESKTOP_RECOVERY_DIRECTORY` isolate controlled development and system-test state. They are not a versioned Host installation or discovery mechanism. The default POSIX runtime uses the short stable product-scoped child of the trusted per-user runtime location described above; an explicit runtime directory is still prepared and validated by the native endpoint-security boundary.

Run package checks from the repository root:

```sh
pnpm --filter @orchbee/desktop typecheck
pnpm --filter @orchbee/desktop test
pnpm --filter @orchbee/gateway-protocol test
pnpm --filter @orchbee/gateway test
pnpm --filter @orchbee/web build
pnpm --filter @orchbee/desktop build
pnpm --filter @orchbee/desktop test:system
```

The optional throughput harness requires an absolute large-output fixture and at least five runs:

```sh
pnpm --filter @orchbee/desktop benchmark -- \
  --file /absolute/path/to/large-output.txt \
  --runs 5
```

Keep `pnpm web` running while using the benchmark, or set `ORCHBEE_DESKTOP_RENDERER_URL` to another loopback Web development server.

It can also collect controlled Ghostty and Host-only references. Results are diagnostic evidence; they are not a merge or slice-completion gate in the current development phase.
