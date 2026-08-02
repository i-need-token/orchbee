# Desktop

Sandboxed Electron product client and scoped local broker for Orchbee's native Terminal Host.

## Current scope

Desktop composes the shared Workbench with Host-backed projected terminals. Workbench owns Project,
Workspace, Tab, Panel, and `TerminalPresentationId` state; Electron Main owns durable technical
bindings and privileged effects; `apps/terminal-host` owns PTYs, processes, Ghostty VT state,
history, and real terminal identity. None of the product topology is stored in the Host.

Main starts without creating a fixed terminal or opening an attachment. It restores the durable
`TerminalPresentationId` to `TerminalBindingId` registry, owns the private Workbench-topology file
adapter, and transfers three allowlisted document ports through the sandboxed preload:

- lifecycle control accepts only presentation-scoped provision, restore, and close commands and
  publishes correlated results plus terminal exit;
- projection control opens and releases one scoped Host attachment for a displayed terminal
  viewport.
- topology control loads or exact-next compare-and-swaps only canonical
  `@orchbee/workbench/topology` documents.

The renderer has no Node integration, generic Electron IPC, filesystem capability, Host inventory,
or raw terminal-lifetime authority. Preload transfers exactly one port for each recognized control
channel and attests context isolation and sandboxing.

The renderer loads topology before constructing its managed Workbench runtime. An available
document restores its exact Project, Workspace, Tab, Group, Panel, Split, and presentation
placement. A missing document projects recovered bindings as standalone Tabs and commits revision
`0` before any initial lifecycle command. An unavailable load or failed first commit creates no
replacement topology or terminal effect. Unreferenced technical bindings remain preserved but are
not silently adopted into product topology. Update activation UI, sharing, network clients, and the
user-facing quit/retention choice remain outside the current composition.

Desktop requires Host protocol `1.4` exactly. Electron uses `42.8.0`, the final release in the major
immediately preceding the current Electron major selected for this repository.

## Lifecycle and identity ownership

The maintained identity chain is:

```text
Workbench TerminalPresentationId
        │ document lifecycle command
        ▼
Desktop TerminalBindingId ── durable binding store
        │ recovery reconciliation
        ▼
HostEpoch + CreationId + TerminalId ── Terminal Host authority
        │ visible viewport demand
        ▼
AttachmentId ── one disposable projection capability
```

`TerminalPresentationId` is created and retired by Workbench. Main maintains a strict in-memory
bijection to `TerminalBindingId`; it never exposes Binding, Creation, Host, Terminal, or Attachment
identity through lifecycle control. The Host remains the source of truth for the process and VT.

The version-3 binding store contains a canonical BindingId-ordered collection. Every record has one
unique BindingId, PresentationId, CreationId, bounded launch profile, and exactly one durable state:
`planned`, `create-requested`, `retained`, `exited`, `closing`, or `lost`. Retained identities are
unique and must repeat their owning CreationId. The file is parsed as untrusted input, updated
through owner-only atomic replacement, and never rewritten automatically when invalid or
unreadable. Launch environment values never enter diagnostics.

No product maximum is imposed on terminals, bindings, or attachments. Active collections grow from
user topology and visible demand; allocation, filesystem, Host, or memory exhaustion returns an
operation failure instead of evicting or silently truncating another terminal. Host inventory is
read through generation-scoped protocol pages rather than one bounded global list.

The separate Workbench topology store accepts only the canonical version-1 document through one
Main-owned serialized writer. A missing store starts at revision `0`; every later commit must name
the exact current revision and advance it by one. Conflicts, non-next revisions, invalid files, and
read failures preserve current data. Writes use owner-only temporary files, file synchronization,
atomic replacement, and parent-directory synchronization where supported. The renderer receives
only typed load and commit results, never a path, Node error, or general filesystem capability. See
[`docs/workbench-topology-storage.md`](docs/workbench-topology-storage.md).

Provisioning allocates and persists a fresh binding before creating a terminal. Recovery accepts
only an exact HostEpoch/TerminalId/CreationId match and never substitutes a new shell for lost
continuity. A lost create response may replay only the same CreationId and launch in the recorded
Host epoch. Orphan, conflict, corrupt-ledger, and transport states fail closed.

Retain reconciliation is a separate restore-only operation. It succeeds only for a Binding that
was already durably retained or exited, or became restorable after successful provision in the same
Main lifetime. Missing, planned, closing, or lost bindings never allocate a replacement through the
restore path. Restore failure remains durable retain intent.

One operation tail serializes lifecycle work per presentation while unrelated terminals progress
independently. The Renderer coordinator commits the complete topology containing provision or close
intent before forwarding the corresponding effect. It coalesces ordinary topology changes while a
CAS is in flight and stops the generation after conflict or storage failure. Close then persists
technical `closing`, retires only that presentation's active projections, and delegates
graceful/force/dispose behavior to the existing binding close lifecycle. A Panel or Tab remains in
Workbench until every exact terminal close result completes; cleanup commits afterward, while the
previously stored close intent remains replayable if cleanup fails. Renderer reload, a renderer
crash, window close, and current application shutdown release document attachments but preserve
Host terminals.

## Demand-driven projection

Workbench mounts the application-owned terminal surface only inside displayed panel viewports.
The surface requests projection only while its lifecycle is `running` or `exited`; unmounting,
switching Tabs, hiding another terminal context, or entering `closing` releases the request.
Background Projects, Workspaces, Tabs, inactive child terminals, and otherwise invisible panels
therefore hold no renderer, GPU surface, frame stream, history cache, search Worker, or Host
attachment.

Each successful request receives a fresh AttachmentId, capability bootstrap, and private
`MessagePort`. Running terminals request read/input/resize authority; exited terminals attach
read-only with their retained final revision. Multiple visible split panels may hold independent
attachments, with no global attachment threshold.

`HostConnection` reads and validates the fixed terminal-frame header once, then routes the binary
record directly by AttachmentId. Main performs no VT parsing, row reconstruction, JSON/base64
conversion, or per-cell pass. A broker structured-clones the frame once to its exact renderer port.
History, selection, copy, link, and accessibility behavior remains attachment-scoped. Normal
bottom-following use creates no history cache or search Worker; these resources are activated only
by their existing interaction contracts.

Host `TerminalExited` events are resolved through the authoritative runtime registry, persisted to
the exact binding, published to Workbench by PresentationId, and also forwarded to any live
projection. A recovered exited binding is restored into Workbench, attached read-only, and
retains its final-revision presentation behavior.

## Correctness evidence

Focused traces cover:

- strict lifecycle and projection boundary validation, malformed-message retirement, exact
  correlation, cancellation, and renderer-document disposal;
- strict topology load/CAS boundary validation, shared serialization across renderer generations,
  invalid-file preservation, atomic replacement, and suppressed late responses;
- first-commit and per-command write-ahead ordering, coalesced topology commits, restore-only
  recovery, failed-CAS effect suppression, and post-effect managed container cleanup;
- fresh provisioning, multi-binding store preservation, recovery, exited recovery, orphan and
  conflict handling, durable close, and presentation-registry reconstruction;
- unrelated terminal concurrency, exact close targeting, Tab-close result ordering, and stale
  result rejection;
- two recovered Workbench tabs where both lifecycle records reconcile but only the active visible
  Tab has an attachment, followed by exact release/open behavior on Tab switch;
- fixed-header frame routing to only the matching AttachmentId;
- sandboxed end-to-end input, output, history, search, selection, copy, links, exit, renderer reload,
  and Electron Main restart.

Existing TLA+ models continue to check terminal recovery, binding close, lifecycle-adapter
serialization, projection demand, and Workbench removal safety within their declared finite bounds.
The Desktop topology-store model additionally checks exact-next CAS, conflict preservation,
serialization, unavailable storage, generation disposal, and progress for two clients through
revision `2`.
The Desktop write-ahead model checks matching durable intent before provision, restore, or close
dispatch; failed-commit preservation; restore without replacement creation; renderer disposal and
reload replay; and live-generation progress for one abstract presentation.
The repository imposes no new throughput acceptance gate for this integration slice; the optional
benchmark remains a diagnostic tool.

## Development

Build the selected development Host, then start Desktop:

```sh
pnpm --filter @orchbee/terminal-host build
pnpm desktop
```

`ORCHBEE_TERMINAL_HOST_BINARY` selects an absolute development/test Host binary.
`ORCHBEE_DESKTOP_RUNTIME_DIRECTORY`, `ORCHBEE_DESKTOP_ENDPOINT_ID`, and
`ORCHBEE_DESKTOP_RECOVERY_DIRECTORY` isolate controlled development and system-test state. They are
not a versioned Host installation or discovery mechanism. The default POSIX runtime uses a short
private per-home path under `/tmp` so its Unix socket remains within platform limits.

Run package checks from the repository root:

```sh
pnpm --filter @orchbee/desktop typecheck
pnpm --filter @orchbee/desktop test
pnpm --filter @orchbee/desktop test:tla
pnpm --filter @orchbee/desktop build
pnpm --filter @orchbee/desktop test:system
```

The change-triggered `test:tla` command requires Java 17 or newer for its pinned TLA+ runner. The
ignored app-local `.tools/` directory may cache the verified runner JAR.

The optional throughput harness requires an absolute large-output fixture and at least five runs:

```sh
pnpm --filter @orchbee/desktop benchmark -- \
  --file /absolute/path/to/large-output.txt \
  --runs 5
```

It can also collect controlled Ghostty and Host-only references. Results are diagnostic evidence;
they are not a merge or slice-completion gate in the current development phase.
