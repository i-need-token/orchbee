# Desktop

Sandboxed Electron client and scoped local broker for Orchbee's native Terminal Host.

## Current scope

The current Desktop foundation mounts one app-local `ProjectedTerminal` and binds live projection,
bounded history, Host-owned selection, client-side find, copy, and approved-link activation through
the real Host → Electron main → preload → renderer path. Electron main owns the Host socket, record
assembly, process launch, attachment routing, platform clipboard write, and external-navigation
policy.
Preload forwards one allowlisted `MessagePort`; it exposes no generic IPC, filesystem, executable,
terminal-enumeration, creation, termination, or disposal capability. The renderer runs sandboxed
with context isolation and without Node integration.

The application intentionally does not import Workbench or model Project, Workspace, Tab, Panel,
updates, sharing, network clients, or recovered-terminal product presentation. Closing or
reloading the renderer, or restarting Electron Main, detaches only the scoped attachment; it never
terminates or disposes the Host terminal. Only an explicit per-binding lifetime request may close a
terminal. The foundation exposes that Main-process operation, but no window, quit, Panel, or
Workbench action invokes it yet.

Desktop requires the current Host protocol `1.4` exactly. The handshake envelope and accepted
version are both `1.4`; an obsolete Host is rejected without creating or adopting a terminal.

## Terminal document control foundation

Desktop defines one document-scoped lifecycle control contract for the later Workbench
integration. Its renderer-to-Main commands are exactly `terminal-provision-requested` and
`terminal-close-requested`; results are correlated provision/close completion or failure events,
plus presentation-scoped terminal exit. Every command carries only Workbench-owned
`TerminalPresentationId` and operation identity. `TerminalBindingId`, `HostEpoch`, `CreationId`,
`TerminalId`, executable choice, environment, arbitrary channel names, and generic IPC operations
are absent from the document contract. Launch CWD is nullable and bounded per message by the Host
protocol's 16 KiB UTF-8 string limit.

Preload installs an allowlisted transfer bridge for this control port alongside the existing
projected-attachment bridge. It accepts one exact bootstrap shape and one transferred port, then
attests the sandbox and context-isolation flags to the renderer. Structurally malformed commands or
conflicting reuse of one operation ID retire the document port. An exact repeated command is
idempotent; independently completed commands remain correlated; late completions after document
retirement are dropped. Adapter exceptions become a fixed retryable failure code and never expose
exception text. Main composition does not publish this port and Renderer does not consume it in
the current slice, so the existing single-terminal foundation remains the only live product path.

`DesktopTerminalBindingRegistry` is the Main-owned technical identity index for that later path. It
maintains a strict bijection between product `TerminalPresentationId` and Desktop
`TerminalBindingId`, treats an exact repeated association as idempotent, rejects either-side
conflicts without mutation, and removes only an exact expected association. The registry does not
allocate or persist Binding IDs, own product lifecycle, retain Host identity, create a terminal, or
choose recovery policy. Durable bindings remain owned by the version-2 binding store and product
phase remains owned by Workbench. No product terminal, binding, or attachment count threshold is
introduced; allocator, storage, and memory exhaustion remain explicit operation failures rather
than eviction or truncation policy.

Contract tests cover strict boundary rejection and privileged-identity exclusion. Registry traces
cover bijection, idempotence, both conflict directions, exact removal, canonical observation, and a
multi-entry set without eviction. Broker traces cover invalid capability retirement, duplicate and
conflicting operation IDs, reordered completion, disposal, fixed failure mapping, and typed exit
publication. These tests do not claim live Workbench binding, viewport attachment, Main restart
recovery, or end-to-end lifecycle behavior; those remain later integration slices.

## Terminal binding store and recovery model

The actor is the single running Electron Main process. At startup it must recover the exact terminal
previously bound to this Desktop foundation, provision it only when the target binding is absent and
the Host has no unbound terminal, and stop without mutating the Host when durable intent is
ambiguous. The single-instance lock is acquired before any binding-store access. The fixed
foundation binding remains only the current composition consumer; neither it nor the store is a
Project, Workspace, Tab, Panel, or product-session model.

The version-2 store contains a canonical BindingId-ordered collection. Each `TerminalBinding`
contains one opaque binding identifier, one random `CreationId`, a normalized launch profile, and
exactly one state: `planned`, `create-requested`, `retained`, `exited`, `closing`, or `lost`.
`create-requested` records the attempted `HostEpoch`; retained and exited states record the complete
Host identity; exited additionally records the exit result and final revision. BindingId,
CreationId, and retained Host identity are unique, and every identity repeats its owning
CreationId. The store declares no total binding count or encoded-file-size product limit: each
record remains strictly bounded, while allocator, filesystem, and storage exhaustion surface as
operation failures instead of truncating or evicting another binding.

The store parses disk data as untrusted input and accepts only its current version-2 collection
shape. Targeted writes are serialized inside the one owning Main process, written through a
same-directory temporary file, synchronized, atomically renamed, and owner-only on POSIX. Missing,
invalid, unreadable, or unwritable state is distinct; invalid or unreadable data is never
overwritten automatically. Launch environment values are sensitive recovery data and never enter
diagnostics.

Recovery reconciliation has the following observable outcomes: `provisioning`, `recovered`,
`closing`, `orphaned`, `lost`, `conflict`, or `ledger-unavailable`. A per-binding lifecycle
coordinator serializes recovery, explicit close, and `TerminalExited` handling so they cannot race
one store writer. It consumes `closing` during startup and reports either `close-completed` or a
safely pending close rather than provisioning a replacement. The combined lifecycle obeys these
invariants:

- A Host mutation occurs only after `planned` and then same-epoch `create-requested` are durable;
  each targeted commit preserves every other binding at the semantic record level.
- A lost Create response may replay the identical `CreationId` and launch only in the recorded
  `HostEpoch`; Host idempotency validates the result. A changed epoch becomes `lost` and never
  silently creates a replacement.
- Retained and exited bindings recover only an exact `HostEpoch`/`TerminalId`/`CreationId` match.
  Inventory pages are consumed to one generation-scoped `nextCursor: null`.
  Duplicate, repeated-cursor, cross-generation, or identity-conflicting pages fail instead of
  returning a partial inventory. An inventory entry not named by any binding is an orphan and is
  never adopted, terminated, disposed, or hidden by a new terminal.
- A recovered exited terminal is attached read-only. Its retained exit is published to the
  renderer immediately, but the terminal surface exposes completion only after a replacement
  snapshot reaches the recorded final revision.
- An explicit close first persists `closing` with the complete Host identity. Only then does Main
  retire the renderer broker and await Host detach. For a running target it requests graceful
  termination, waits an injected five-second grace period, requests protocol-1.4 force termination
  if the exact terminal has not exited, waits for its exact `TerminalExited`, disposes it, and
  removes only that binding after the Host acknowledgment. A target already starting or naturally
  finishing remains safely pending instead of receiving a phase-invalid signal; its later exact
  exit route completes disposal.
- A terminal already absent from the current Host or owned by an old `HostEpoch` permits only the
  target binding to be removed. A `CreationId` or `TerminalId` contradiction is a conflict and
  never authorizes a signal. Transport loss or an operational failure leaves durable `closing`
  intent for the next Main process; a later exact exit route completes disposal. Unrelated
  terminals and bindings are never terminated, disposed, removed, or used as replacements.
- Every renderer document gets a fresh `AttachmentId`, `MessageChannel`, and broker. Old command
  authority is closed immediately, Host detach is awaited, and only then may the next attachment
  acquire a resize lease. Once explicit close retires a document, reload cannot attach another
  projection to that closing terminal. Stale document generations cannot receive or issue terminal
  work.

The binding store owns durable Desktop intent; the Host owns terminal inventory, process lifetime, VT
state, history, and final revision; each renderer document owns only its current projection. File
storage, randomness, Host requests, and Electron windows are injected capabilities around a
deterministic reconciliation policy. Unit traces cover first provision, lost-response retry,
cross-epoch loss, targeted multi-binding preservation, corrupt storage,
orphan inventory, paged inventory, exited recovery, and ordered attachment replacement. The checked
`TerminalRecoveryBinding` TLA+ model covers the existing recovery safety claims within its declared
two-epoch/two-Main/two-document bounds; binding-count and filesystem exhaustion are deliberately not
proved by that finite model. The independent `DesktopTerminalBindingClose` model covers durable
close intent, projection retirement, grace escalation, exact disposal, targeted removal, and Main
restart for two bindings. Deterministic close traces cover graceful exit, grace-timeout force,
transport loss, resumed closing, confirmed absence, failed persistence, exact late exit, and
unrelated-binding preservation. The native Electron system trace reloads a renderer and restarts
Main while asserting stable terminal identity, retained content, and a fresh attachment.

Main validates and routes packed Host frames and row windows without VT parsing, row
reconstruction, JSON/base64, or per-cell work. Electron structured-clones each binary projection
once from Main to Renderer; the route does not claim zero-copy `ArrayBuffer` ownership transfer.
Host projection backpressure still permits at most one frame in flight for the attachment, and the
renderer issues at most one bounded row read at a time.
Format-1.1 text windows use an implicit mapping for verified narrow ASCII rows, so search does not
transport or materialize a 12-byte mapping record per character. Unicode and non-identity rows keep
their explicit cell mappings.

Normal bottom-following use creates neither a history cache nor a search Worker. Entering history
pauses live projection work, commits one contiguous packed window with bounded overscan, and falls
back to the exact viewport if overscan exceeds the negotiated 2 MiB row-window limit. Find lazily
streams text-detail rows through a source-neutral reader into one disposable Worker; the query
never enters Main or Host. The Host remains the selection authority. Main assembles at most one
bounded selection export, writes it to the platform clipboard, and reports only completion metadata
to the renderer. Main opens only `http`, `https`, and `mailto` URL targets; file and unknown targets
remain unavailable.

One app-owned Actions runtime binds find, navigation, select-all, copy, and link activation. The
private canonical find surface owns its query history and localized UI in `en-US` and `zh-CN`.
`Command+A`/`Command+C` are desktop selection actions on macOS; non-Apple clients keep `Ctrl+A` and
`Ctrl+C` available to the PTY and use `Ctrl+Shift+C` for copy.

The controlled verification surface uses `Menlo` at 15 points with a generic monospace fallback. This
is a controlled benchmark default, not the future product font-preference model.

## Development

Build the explicitly selected development Host first, then start Desktop:

```sh
pnpm --filter @orchbee/terminal-host build
pnpm desktop
```

`ORCHBEE_TERMINAL_HOST_BINARY` may select another absolute development/test Host binary.
`ORCHBEE_DESKTOP_RUNTIME_DIRECTORY` and `ORCHBEE_DESKTOP_ENDPOINT_ID` exist only for controlled
foundation development and system tests. They are not the versioned installation or discovery design.
`ORCHBEE_DESKTOP_RECOVERY_DIRECTORY` isolates the sensitive terminal binding store in controlled tests;
normal development and product composition use the Electron user-data directory.
The default POSIX development runtime uses a short, per-home private directory under `/tmp` so its
Unix-domain socket stays inside platform path limits; the Host still enforces owner-only access.

Run focused checks from the repository root:

```sh
pnpm --filter @orchbee/desktop typecheck
pnpm --filter @orchbee/desktop test
pnpm --filter @orchbee/desktop build
pnpm --filter @orchbee/desktop test:system
```

Run the controlled throughput harness only after building Desktop and the Host. It requires at
least five Electron runs and accepts separately measured, geometry-matched Ghostty and Host-only
runs for comparison:

```sh
pnpm --filter @orchbee/desktop benchmark -- \
  --file /absolute/path/to/large-output.txt \
  --runs 5 \
  --measure-ghostty \
  --slice5-median 28.351 \
  --host-only-runs 18.4,18.5,18.3,18.6,18.4
```

On macOS, `--measure-ghostty` launches five isolated Ghostty surfaces with default config files
disabled and pins their grid, Menlo 15-point font, and 10,000,000-byte scrollback to the geometry
reported by Desktop. After the first Desktop run establishes geometry, Desktop and Ghostty order
alternates within paired samples so sustained-load or thermal drift is not assigned to only one
implementation. `--ghostty-app` may select another absolute `.app` path. Alternatively,
`--ghostty-runs` accepts an already controlled five-run reference; do not use both options.
`--slice5-median` supplies the same-machine Slice 5 baseline and activates its no-more-than-5%
regression gate. The harness executes `apps/terminal-host/zig-out/bin/terminal-host`, so rerun the
Host `build` command after native source changes; the focused Host `benchmark` target does not
replace that service binary.

The timer starts when Enter is sent and stops only after the final revision has produced renderer
frame telemetry. Each run reports Host protocol bytes/frame count, Main routing, renderer
commit/render counts and backend health, plus idle Electron-process and Host CPU/RSS observations.
After command completion it measures one exact `113 × 38` history draw, a retained-content literal
search, and select-all/copy through the platform clipboard. Renderer event timestamps exclude
automation round trips; `PerformanceObserver` records long tasks, Host RSS is sampled every 20 ms
during copy, and Electron reports Main's peak working set. The active gates are history p95 at most
50 ms, search p95 at most 750 ms, copy broker p95 at most 1,000 ms, no renderer task of at least
50 ms, and one lazily created Worker that is terminated after search.
The Electron CPU sample is primed and then measured after a one-second idle window; it is not the
first `getAppMetrics()` call, whose interval CPU values are zero by definition.
The summary reports median, median absolute deviation, incremental Host-to-Electron cost, and the
accepted `1.15 × Ghostty` and `1.05 × Slice 5` targets. Comparative and active targets apply only to
fixtures of at least 500,000,000 bytes on macOS; smaller smoke fixtures still report measurements
but cannot pass or fail those targets. Idle evidence also requires zero row-window, selection-export,
clipboard, and search-Worker work before the feature interactions. Omit reference run lists when
establishing raw local baselines; the harness then makes no comparative pass claim.

The system test requires the Host build above. Raw benchmark inputs, traces, and screenshots stay
untracked.
