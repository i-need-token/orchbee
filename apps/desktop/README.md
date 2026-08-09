# Desktop

Sandboxed Electron product client and scoped local broker for Orchbee's native Terminal Host.

## Current scope

Desktop composes the shared Workbench with Host-backed projected terminals. Workbench owns Project, Workspace, Tab, Panel, and `TerminalPresentationId` state; Electron Main owns durable technical bindings and Electron-only privileged effects; `apps/terminal-host` owns PTYs, processes, Ghostty VT state, history, and real terminal identity. A Browser client commits clipboard and allowlisted link effects in its own page. None of the shared Workbench document is stored in the Host.

Main starts without creating a fixed terminal or opening an attachment. It restores the durable `TerminalPresentationId` to `TerminalBindingId` registry, owns one process-wide canonical Workbench shared-document coordinator, and transfers one atomic allowlisted capability bundle through the sandboxed preload. Its three ports have one fixed order:

- client-session control delivers one atomic document/terminal-observation bootstrap, accepts complete exact-next shared-document proposals, and publishes one ordered canonical event stream;
- projection control opens and releases scoped Host attachments only for displayed terminal viewports;
- application-lifetime control carries only exit reason, user choice, quiescing acknowledgement, shared-document revision, aggregate progress, and typed failures for the current renderer page.

The client-session and projection payloads are platform-neutral contracts from `@orchbee/workbench-client-protocol`. `@orchbee/desktop-client-protocol` owns only the atomic Electron renderer bootstrap and Electron application-lifetime contract. Desktop Main and the Web renderer each adapt the generic contracts to their end of a `MessagePort`; neither transport detail enters the shared Workbench client runtime.

The renderer has no Node integration, generic Electron IPC, filesystem capability, Host inventory, or raw terminal-lifetime authority. It cannot invoke provision, restore, close, or storage CAS effects directly. Preload accepts exactly one bootstrap, verifies all three ports together, preserves their fixed client-session/projection/application-lifetime order, and attests context isolation and sandboxing. A partial or malformed bundle starts no renderer runtime.

Main loads and validates the shared document once before admitting clients. An available document restores Project, Workspace, Tab, Group, Panel/Split structure, presentation placement, and lifecycle intent; every page derives a fresh in-memory Client View. Active navigation, disclosure, focus, maximization, active child, and Split weights do not enter the shared file. A missing document projects every recovered binding as a standalone `retain` Tab and gives each joining page the same revision-`0` initialization seed. The first accepted proposal persists that seed before Main performs restore/provision effects. An unavailable load or failed first commit creates no replacement document or terminal effect. Unreferenced technical bindings remain preserved but are not silently adopted into the shared document.

The coordinator is transport-neutral and supports independent page sessions without a product count ceiling. Electron uses a direct `MessagePort` adapter. Local Browser development starts `apps/gateway` as an on-demand Utility Process; Desktop owns its lifetime and allocates exactly one client-session/projection port pair per authenticated page, while the Utility owns the loopback listener, authentication, framing, and transport backpressure. The Gateway never receives an application-lifetime port or a Host connection. `apps/web` owns the platform-neutral Workbench client runtime and the single renderer application used in an ordinary browser, a Gateway Browser client, and Electron. Development Electron is discovered from its User-Agent on the loopback Web origin. Packaged Desktop registers the standard secure `orchbee:` scheme before Electron becomes ready, serves the Web build from `orchbee://app`, and falls back to `index.html` for Browser Router document requests. Desktop owns renderer resource delivery, not another renderer source tree. See [`docs/multi-client-session-coordinator.md`](docs/multi-client-session-coordinator.md) and [`../web/docs/navigation-routing.md`](../web/docs/navigation-routing.md).

Desktop requires the single current Host protocol `0.0.2` exactly; pre-release builds do not negotiate or document historical protocol variants. Electron uses `42.8.0`, the final release in the major immediately preceding the current Electron major selected for this repository.

Desktop scopes one Terminal Host endpoint to its product `userData` identity, which separates OS users, product profiles, and release-channel data roots. The short POSIX runtime path and Windows named-pipe identity are derived from that scope; explicit endpoint and runtime overrides exist only for controlled development and tests. After every successful handshake, Desktop sends the current terminal default through `SetTerminalDefaults`, so reconnecting to an already-running singleton has the same result as a fresh CLI launch. Product settings UI and durable preference storage remain a later Desktop-owned slice.

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

Existing TLA+ models continue to check terminal recovery, binding close, lifecycle-adapter serialization, projection demand, and Workbench removal safety within their declared finite bounds. The Desktop shared-document-store model additionally checks exact-next CAS, conflict preservation, serialization, unavailable storage, generation disposal, and progress through revision `2`. The Desktop multi-client model checks two sessions, missing/available bootstrap, equivalent and divergent races, effect publication ordering, attachment/session ownership, disconnect, quiescing handoff, unavailable storage, and bounded publication progress. The Desktop write-ahead model checks matching durable intent before provision, restore, close, or stop dispatch; failed-commit preservation; restore without replacement creation; renderer disposal and reload replay; and live-generation progress for one abstract presentation. The Desktop application-lifetime model checks prompt and update paths, stop-before-close ordering, unreferenced binding drain, cancellation before effects, clean-stop exit, and the explicit forced exit escape. Workbench's terminal-stop model checks layout retention, stopped-without-binding, retry identity, and fresh PresentationId generation. The repository imposes no new throughput acceptance gate for this integration slice; the optional benchmark remains a diagnostic tool.

## Development

Build the selected development Host, then run the canonical Web renderer and Desktop in separate terminals:

```sh
pnpm --filter @orchbee/terminal-host build
pnpm web
pnpm desktop
```

`pnpm desktop` defaults to `http://127.0.0.1:3000/`; an explicit loopback Web server can be selected with `ORCHBEE_DESKTOP_RENDERER_URL`. Electron's User-Agent selects the Desktop adapter without changing the development URL. Desktop builds and starts the local Browser Gateway by default and prints one `ORCHBEE_BROWSER_CLIENT_URL=<url>` line. Open that complete process-lifetime URL in each Browser page that should join the same canonical Workbench. The capability stays in the fragment and must not be persisted or copied into diagnostics. Set `ORCHBEE_DESKTOP_GATEWAY=0` to disable the Utility Process.

Build an unsigned current-platform application bundle with:

```sh
pnpm desktop:package
```

The command builds the Terminal Host, Web renderer, and Electron Main/preload bundles before packaging. The Web build is a read-only resource served from `orchbee://app`; the native Host remains executable outside `app.asar`. The output is generated beneath `apps/desktop/out/` and is not a signed installer or release artifact.

`ORCHBEE_TERMINAL_HOST_BINARY` selects an absolute development/test Host binary. `ORCHBEE_TERMINAL_SCROLLBACK_BYTES` selects the temporary development startup/default scrollback budget in decimal bytes from `0` through `1000000000`; the built-in default is `10000000`. `ORCHBEE_GATEWAY_ENTRY_PATH` selects the built development Gateway entry when the default sibling path is not used. `ORCHBEE_DESKTOP_RUNTIME_DIRECTORY`, `ORCHBEE_DESKTOP_ENDPOINT_ID`, and `ORCHBEE_DESKTOP_RECOVERY_DIRECTORY` isolate controlled development and system-test state. They are not a versioned Host installation or discovery mechanism. The default POSIX runtime uses a short private path derived from the product `userData` scope under `/tmp` so its Unix socket remains within platform limits.

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
