# Desktop

Sandboxed Electron product client and scoped local broker for Orchbee's native Terminal Host.

## Current scope

Desktop composes the shared Workbench with Host-backed projected terminals. Workbench owns Project, Workspace, Tab, Panel, and `TerminalPresentationId` state; Electron Main owns durable technical bindings and Electron-only privileged effects; `apps/terminal-host` owns PTYs, processes, Ghostty VT state, history, and real terminal identity. A Browser client commits clipboard and allowlisted link effects in its own page. None of the shared Workbench document is stored in the Host.

Main starts without creating a fixed terminal, opening an attachment, or starting a Web listener. It restores the durable `TerminalPresentationId` to `TerminalBindingId` registry, owns one process-wide canonical Workbench shared-document coordinator, and transfers one atomic allowlisted capability bundle through the sandboxed preload. Its six ports have one fixed order:

- client-session control delivers one atomic document/terminal-observation bootstrap, accepts complete exact-next shared-document proposals, and publishes one ordered canonical event stream;
- projection control opens and releases scoped Host attachments only for displayed terminal viewports;
- application-lifetime control carries only exit reason, user choice, quiescing acknowledgement, shared-document revision, aggregate progress, typed failures, and the Workbench presentation identities needed to resume an in-progress development Host restart;
- Browser-access control carries only explicit start, mode-switch, open, copy, stop, sanitized Origin state, and bounded failures. It never carries the capability-bearing URL;
- Project Folder acquisition requests one native existing-directory selection and returns only a correlated selected path, cancellation, or bounded failure;
- terminal-launch-profile control projects and mutates one bounded device-local catalog of system-shell or explicit-executable launch choices. It carries no environment, CWD, secret, project payload, or generic filesystem capability.

The client-session and projection payloads are platform-neutral contracts from `@orchbee/workbench-client-protocol`. `@orchbee/desktop-client-protocol` owns the atomic Electron renderer bootstrap, Electron application-lifetime contract, narrow Browser-access command contract, typed Project Folder-acquisition contract, and terminal-launch-profile command contract. Desktop Main and the Web renderer each adapt the generic contracts to their end of a `MessagePort`; neither transport detail enters the shared Workbench client runtime.

The renderer has no Node integration, generic Electron IPC, filesystem read capability, Host inventory, or raw terminal-lifetime authority. It cannot invoke provision, restore, close, or storage CAS effects directly. Preload accepts exactly one bootstrap, verifies all six ports together, preserves their fixed client-session/projection/application-lifetime/Browser-access/Project-Folder-acquisition/terminal-launch-profiles order, and attests context isolation and sandboxing. The Project Folder port accepts no renderer path and exposes no enumeration or mutation primitive. A separate context-isolated file-drop adapter exposes only the current platform and Electron's `File`-scoped backing-path lookup; it cannot accept an arbitrary path string, read file contents, enumerate a directory, or perform an IPC request. The Web renderer admits that adapter only for a user drop, applies its own bounded POSIX quoting and mandatory review policy, and never appends Enter. A partial or malformed bootstrap starts no renderer runtime.

OSC 52 clipboard writes use the existing projection port rather than a generic preload clipboard API. Main remembers only the exact first Host request forwarded to an attachment; a renderer approval or rejection must repeat its attachment, sequence, and bounded text exactly. A matching approval performs one Desktop-owned plain-text write and consumes the pending request, while cancellation, mismatch, duplication, broker backpressure, page retirement, and attachment disposal perform no write. Gateway Browser attachments commit an approved request through their local Clipboard API and send the same exact decision only to consume Main's pending record, so Electron cannot repeat that client-local effect.

Main loads and validates the shared document once before admitting clients. An available document restores Project, Workspace, Tab, Group, durable fallback/custom Tab titles, Panel/Split structure, presentation placement, and lifecycle intent; every page derives a fresh in-memory Client View. Active navigation, disclosure, focus, maximization, active child, natural terminal title, current CWD, and Split weights do not enter the shared file. A missing document projects every recovered binding as a standalone `retain` Tab and gives each joining page the same revision-`0` initialization seed. The first accepted proposal persists that seed before Main performs restore/provision effects. An unavailable load or failed first commit creates no replacement document or terminal effect. Unreferenced technical bindings remain preserved but are not silently adopted into the shared document.

The coordinator is transport-neutral and supports independent page sessions without a product count ceiling. Electron uses a direct `MessagePort` adapter. Explicit Browser access starts `apps/server` and `apps/gateway` as on-demand Utility Processes; Desktop owns both lifetimes and allocates exactly one client-session/projection port pair per authenticated page. Server owns static renderer delivery, while Gateway owns exact-Origin admission, capability authentication, framing, and transport backpressure on loopback or an explicitly selected trusted LAN. Gateway never receives an application-lifetime port or a Host connection. `apps/web` owns the platform-neutral Workbench client runtime and the single renderer application used in an ordinary browser, a Gateway Browser client, and Electron. Development Electron is discovered from its User-Agent on the loopback Web origin. Packaged Desktop registers the standard secure `orchbee:` scheme before Electron becomes ready, serves the Web build from `orchbee://app`, and falls back to `index.html` for Browser Router document requests. Desktop owns renderer resource delivery, not another renderer source tree. See [`docs/multi-client-session-coordinator.md`](docs/multi-client-session-coordinator.md) and [`../web/docs/navigation-routing.md`](../web/docs/navigation-routing.md).

Desktop requires the single current Host protocol `0.0.13` exactly; pre-release builds do not negotiate or document historical protocol variants. Electron uses `42.8.0`, the final release in the major immediately preceding the current Electron major selected for this repository.

`HostConnection` owns one connection-global outbound window across every broker caller: at most 64 unanswered request records and 4 MiB of encoded request bytes. It retains that admission until the correlated response or connection failure. A full window or a Node writable that has returned `false` rejects work before handoff with retryable `host-transport-backpressure`; no request write resumes before `drain`. If a handed-off request reaches its deadline, Desktop retires the connection and rejects all pending and future requests with non-retryable `host-request-outcome-unknown`, so input is never replayed after an ambiguous result. The broker's 64-command per-attachment window remains an additional capability-local guard.

Inbound Host records retain their wire order across JavaScript Promise settlement. `HostConnection`
claims each correlated response as it is decoded, settles that response, and holds only following records
behind one event-loop barrier so the response continuation runs before a later Host event. An event that
precedes the response remains first, consecutive event/frame delivery remains synchronous, and a
transport end follows every already-decoded record without reopening request admission.

Before it constructs `HostConnection` on POSIX, Desktop asks the packaged native Host helper to validate the runtime directory and actual socket, connect, and authenticate the server's effective UID from kernel peer credentials. Only an authenticated same-UID connection can suppress Host launch, and every retry or later Desktop reconnect repeats the check. The helper is a bounded stdio proxy for that one connection; its exit cannot own or terminate the retained Host process, and its transport preserves `HostConnection` admission and unknown-outcome behavior unchanged.

Desktop scopes one Terminal Host endpoint to its product `userData` identity, which separates OS users, product profiles, and release-channel data roots. The short POSIX runtime path is a stable product-scoped child of the operating system's trusted per-user runtime location (`/run/user/<effective-uid>` on Linux and Electron's per-user temporary location on macOS), never a deterministic child of shared `/tmp`. The Windows named-pipe identity is derived from the same product scope. Explicit endpoint and runtime overrides exist only for controlled development and tests; POSIX overrides still pass through native no-follow owner, private-mode, and ACL preparation and validation. After every successful handshake, Desktop sends the current terminal default through `SetTerminalDefaults`, so reconnecting to an already-running singleton has the same result as a fresh CLI launch. New terminals use a light profile aligned with the Workbench canvas; retained terminals preserve the profile stored with their existing Host session. Device-local product settings are owned by the shared Web renderer and never become Host defaults.

Desktop Main also owns the single local Electron window lifecycle. It restores a strict device-local
normal-bounds and maximized record, reprojects saved geometry into the current display work areas,
and exposes maximize/restore and full-screen through Main-owned semantic Actions used by the
application menu and its accelerators. Full-screen remains transient, and window state never enters
the shared Workbench document or terminal recovery. See
[`docs/window-lifecycle.md`](docs/window-lifecycle.md).

On macOS, the native application menu also exposes a session-only **Secure Keyboard Entry**
checkbox. One Main-owned semantic Action controls Electron's privileged app capability; the
sandboxed renderer receives no Secure Input IPC or state. The default is off. Window blur,
application disposal, and quit disable the capability and clear the checked state; returning focus
does not silently re-enable it. This slice deliberately does not infer password prompts, persist the
choice, or automatically reacquire Secure Keyboard Entry after deactivation.

## Renderer document lifecycle

Electron Main owns the renderer-document generation and its client-session, projection, and application-lifetime controls. Electron `did-start-navigation` details are the trusted navigation input; Main classifies them with one deterministic policy before changing that generation:

| Main frame | Same document | Result                               |
| ---------- | ------------- | ------------------------------------ |
| Yes        | No            | Start a renderer document generation |
| Yes        | Yes           | No-op                                |
| No         | No            | No-op                                |
| No         | Yes           | No-op                                |

A renderer document start retires the previous page controls and records the pending generation. `did-finish-load` publishes one fresh atomic bootstrap only for the latest generation; retirement failure keeps that generation closed. Main permits top-level navigation only within the configured renderer origin so development reload and packaged Browser Router document requests can complete; cross-origin navigation and new windows remain denied. Same-document Browser Router updates and subframe navigation preserve the current generation and every live control. Renderer-process loss and window close remain explicit retirement paths, while Host terminals and the canonical shared document survive page retirement. This model does not add projection retries, control-port timeouts, or a new IPC contract.

`startsDesktopRendererDocument` owns the pure decision. `DesktopApplication` owns generation transitions and Electron effects. Unit tests cover all four decision rows and full navigation/reload sequencing; the `renderer-document` configuration of `DesktopLifecycle` checks ignored-navigation preservation, current-generation control ownership, retirement before replacement, and bounded pending-generation progress through generation `2`. Electron 42.8.0 supplies the navigation classification and event order; operating-system termination before Electron emits an event remains outside this claim.

## Native Project Folder acquisition

Desktop is the only current composition that injects the Workbench existing-Folder acquisition
capability. One correlated renderer request asks Electron Main to open
`dialog.showOpenDialog(..., { properties: ['openDirectory'] })` for the owning window. Main accepts
at most one active request for that renderer, requires exactly one selected path, normalizes it to an
absolute path, and checks that it still names a readable, traversable directory. Validation follows
a symbolic link while preserving the user's selected path spelling. The renderer cannot submit a
path for validation or request directory contents.

A selected result enters the existing Workbench Project-create or Project-directory-add Action.
Cancellation and bounded dialog, type, existence, permission, or validation failures change no
Project state. Workbench rechecks the latest Add target and exact duplicate paths before mutation.
Renderer-document retirement, reload, unmount, port closure, window close, and application disposal
revoke delivery, so a late native completion cannot mutate the replacement page. This slice creates,
deletes, or edits no filesystem entry; `New Folder…` remains disabled. The maintained behavior,
evidence, limits, and rollback contract live in
[`docs/project-folder-acquisition.md`](docs/project-folder-acquisition.md).

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

The `browser-access` configuration of `DesktopLifecycle` checks resource ownership, single-mode readiness, cleanup, and
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

The version-3 binding store contains a canonical BindingId-ordered collection. Every record has one unique BindingId, PresentationId, CreationId, bounded launch profile, and exactly one durable state: `planned`, `create-requested`, `retained`, `exited`, `closing`, or `lost`. Retained identities are unique and must repeat their owning CreationId. The file is parsed as untrusted input, updated through owner-only atomic replacement, and never rewritten automatically when invalid or unreadable. Launch environment values never enter diagnostics. Every default launch declares `TERM=xterm-256color`, `COLORTERM=truecolor`, and `TERM_PROGRAM=Orchbee`, and removes inherited `NO_COLOR`, `FORCE_COLOR`, and `NODE_DISABLE_COLORS` policy before the Host creates the PTY process. New POSIX terminals also receive `ORCHBEE_SHELL_INTEGRATION_DIR` from a code-resolved application resource path: development resolves it relative to the built Main entry, and packaged applications resolve it beneath `process.resourcesPath`. Ambient environment cannot replace that path. Desktop ships its own Zsh, Bash, and Fish scripts as package resources but never sources them or modifies a startup file; Settings exposes the exact manual commands. Desktop preserves an explicit UTF-8 locale; when inherited POSIX locale precedence would not produce UTF-8, Main removes a conflicting `LC_ALL`, repairs an invalid `LANG`, and supplies a UTF-8 `LC_CTYPE` fallback as needed. Darwin uses `en_US.UTF-8` because its shell environment does not provide Linux's `C.UTF-8` locale.

## Local terminal launch profiles

Electron Main owns `terminal-launch-profiles-v1.json`, a strict bounded catalog of named device-local
launch profiles. A missing file selects the built-in System shell profile with login mode disabled.
An invalid or unreadable file projects that same safe fallback together with a recoverable issue and
is never rewritten automatically. Explicit create, edit, default-selection, delete, and reset
operations are serialized and use owner-only atomic replacement. The previous catalog remains
authoritative until replacement commits; a failed write preserves the previous default and catalog.

A profile selects either the trusted system shell with a POSIX login-shell decision or one absolute
custom executable with a literal ordered argument vector. It overrides only `executable` and
`arguments` in the trusted base `TerminalLaunch`; Desktop continues to own environment, terminal
presentation profile, configuration, geometry, and CWD policy. Windows reports login-shell support
as unavailable. Main resolves and bounds every catalog profile against the concrete Host
`create-terminal` encoding before accepting storage. An accepted custom executable that later fails
to launch reports the existing
terminal lifecycle failure and never falls back to the system shell.

The default profile is resolved once when a new terminal first provisions, before its durable
binding is written. Retry, renderer reload, application restart, and restore reuse the complete
launch captured by that binding, so editing or changing the current default cannot mutate an
existing terminal. Only the local Electron renderer receives the management port; authenticated
Gateway pages and ordinary Browser clients cannot inspect or change the catalog. See
[`docs/terminal-launch-profiles.md`](docs/terminal-launch-profiles.md).

Ordinary remote shells remain owned by the system SSH client the user invokes inside that PTY.
Desktop does not wrap `ssh`, retain remote credentials or host records, add OpenSSH environment
forwarding rules, install terminfo, or write a remote cache. A requested SSH PTY can carry the
standard `TERM=xterm-256color`; `COLORTERM`, `TERM_PROGRAM`, and any other extra variables cross the
connection only when the user's OpenSSH client configuration and the server both permit them. The
shared Settings surface provides one constant read-only remote diagnostic command that reports the
observed variables and distinguishes an unset TERM, a missing `infocmp` program, and a missing
terminfo entry. That command performs no network, installation, filesystem, or terminal-input
effect until the user explicitly chooses to run it in a shell.

No product maximum is imposed on terminals, bindings, or attachments. Active collections grow from shared product structure and visible demand; allocation, filesystem, Host, or memory exhaustion returns an operation failure instead of evicting or silently truncating another terminal. Host inventory is read through generation-scoped protocol pages rather than one bounded global list. Pre-association `TerminalExited` delivery uses a bounded identity-keyed admission buffer: admitted identities are never evicted, and an identity that has not acquired its binding association is retried before the adapter buffer is released. After the Host envelope is accepted, Desktop Main synchronously records the event in an owner-only durable pending journal before the fire-once Host listener returns. The separate 64-entry identity-keyed in-memory queue is only a scheduler/cache; it is not the crash recovery owner. The pending journal has its own 4096-entry capacity, while the restart-safe publication ledger and durable rejection journal have independent capacities. Journal-full or write failure is recorded as an observable durable rejection when possible, and the admission path fails closed if that rejection cannot be recorded. Unknown identities stay in durable pending ownership without a timer loop; an exact identity observation wakes promotion. Queue-full, shutdown, and observed pending entries may be promoted by the retry owner. A pending entry is removed only after successful durable runtime recording, adapter publication commit, publication-ledger admission, reconciler acceptance, and live publication; compaction failure therefore leaves durable recovery state. Shutdown transfers unresolved in-memory entries to the pending journal before completing the remaining application teardown.

The terminal-exit spill store migrates legacy v1 files with a bounded event array, normalizing each identity to its newest revision and atomically rewriting the file once. Legacy pending records remain v2 pending ownership. Legacy published records are discarded only after the already-loaded durable publication ledger confirms the identity and revision; without that evidence the startup migration fails closed and leaves the v1 file untouched. Oversized or malformed legacy input also fails closed.

`TerminalFailed` is a different result, not a synthetic exit. Desktop keeps an identity-keyed association buffer only while a binding is still acquiring its Host identity, then records the failure on that binding before exposing another projection. The failed terminal remains retained and authoritative in the surviving Host, so Desktop restart recovers the same typed failure from Host inventory; it is not copied into the exit-specific spill store or publication ledger. This does not promise recovery after a Host or operating-system crash.

The separate Workbench shared-document store accepts only the canonical version-1 document through one Main-owned serialized writer. A missing store starts at revision `0`; every later commit must name the exact current revision and advance it by one. Conflicts, non-next revisions, invalid files, and read failures preserve current data. Writes use owner-only temporary files, file synchronization, atomic replacement, and parent-directory synchronization where supported. A client receives only an atomic bootstrap, typed proposal result, and ordered publication; it never receives a path, Node error, store capability, or general filesystem capability. See [`docs/workbench-shared-document-storage.md`](docs/workbench-shared-document-storage.md). The store uses `workbench-shared-document-v1.json`. It has no reader, alias, or migration for the unshipped development topology file; that old path is ignored and not deleted automatically.

Provisioning allocates and persists a fresh binding before creating a terminal. Recovery accepts only an exact HostEpoch/TerminalId/CreationId match and never substitutes a new shell for lost continuity. A lost create response may replay only the same CreationId and launch in the recorded Host epoch. Orphan, conflict, corrupt-ledger, and transport states fail closed.

Retain reconciliation is a separate restore-only operation. It succeeds only for a Binding that was already durably retained or exited, or became restorable after successful provision in the same Main lifetime. Missing, planned, closing, or lost bindings never allocate a replacement through the restore path. Restore failure remains durable retain intent.

One operation tail serializes lifecycle work per presentation while unrelated terminals progress independently. The Main coordinator persists and publishes the complete shared document containing provision, close, or stop intent before deriving the corresponding effect. Its FIFO proposal queue uses first-writer-wins for divergent concurrent complete documents and converges equivalent races. Close then persists technical `closing`, retires only that presentation's active projections, and delegates graceful/force/dispose behavior to the existing binding close lifecycle. A Panel or Tab remains in Workbench until every exact terminal close result completes; cleanup commits afterward, while the previously stored close intent remains replayable if cleanup fails. Renderer reload, a renderer crash, a normal macOS window close, and uncontrolled Main loss release that page's projection attachments but preserve Host terminals. A controlled application quit follows the policy below.

## Application lifetime

A normal macOS window close releases only the renderer page. Command+Q, a menu quit, or closing the last window on Windows and Linux enters one coalesced user-quit workflow while the renderer is still available. When one or more presentations require protection under Workbench's Panel-close predicate, the application-wide dialog offers Keep Sessions and Quit as the default, Stop Sessions and Quit as the destructive choice, or Cancel before any stop effect. When every running presentation is observed idle, or no presentation remains, Desktop preserves the sessions, flushes the shared document, and quits without prompting. The future updater enters the same preserve preparation as `update-restart` without prompting; updater installation, signing, and release distribution remain outside this package.

Before preserve or stop preparation, Main acknowledges that the application-wide coordinator has entered quiescing. This acknowledgement orders the lifetime and client-session ports: other pages cannot race a new shared change into the preparation window. A replacement renderer may resume only the same quiescing mode after the former exit owner disconnects.

Development Host restart enters its own non-interactive quiescing mode. Before any stop effect, the renderer captures only currently restartable presentations and Main retains that exact target set across renderer-document replacement. A replacement renderer resumes from the Main-owned targets even when the shared document already records them as stopping or stopped. The workflow follows the same durable stop and Host-drain barriers, replaces each captured stopped presentation with a fresh provision identity without dispatching that provision to the retiring Host, and flushes the staged document. Main writes the generation-bound ready plan only after the exact staged revision is loaded and the old Host owns no binding or unknown terminal. Ordinary update restart remains the preserve path and never uses this destructive development mode.

Stop seals terminal creation for the exit-owner page, projects every visible and hidden Workbench presentation to durable `stop`, compare-and-swaps the complete shared document, and only then releases close commands. Successful close results change presentations to `stopped` while retaining Project, Workspace, Tab, Panel, split, title, and launch CWD. Hydration never starts a stopped presentation. Its visible Start New Session action replaces the stopped PresentationId with a fresh identity and provisions only that selected presentation.

After referenced presentations stop, Main loads the exact committed shared-document revision and closes every binding that it owns but the document does not reference. It then requires the binding registry to be empty and scans Host inventory. Unknown Host terminals are reported as unresolved; they are never silently killed. A partial failure keeps Desktop in an irreversible recovery dialog with Retry when supported and an explicit Quit Anyway warning. Quit Anyway may leave processes running; the durable `stop` intent remains replay authority for the next start. OS shutdown, logout, and forced termination are not claimed by this workflow.

## Demand-driven projection

Workbench mounts the application-owned terminal surface only inside displayed panel viewports. The surface requests projection only while its lifecycle is `running`, `exited`, or `failed`; unmounting, switching Tabs, hiding another terminal context, or entering `closing` releases the request. Background Projects, Workspaces, Tabs, inactive child terminals, and otherwise invisible panels therefore hold no renderer, GPU surface, frame stream, history cache, search Worker, or Host attachment.

The default visual launch carries null geometry. Provisioning durably binds the presentation and
creates the Host identity in `awaiting-start`, but does not spawn a child or PTY. A visible shared
Web surface waits for its configured fonts and a positive mounted content box, then computes one
complete grid and physical cell geometry from that exact presentation configuration, padding, and
document DPR. Electron and Gateway projection-open commands carry that geometry unchanged. Main
serializes `StartTerminal` before `AttachTerminal`, validates the accepted identity and geometry,
and marks the binding started only after that exact Host transition. The child, PTY, Host Core, first
Host frame, and the renderer's pinned first presentation therefore start from the same geometry.
There is no timeout or 80x24 visual fallback. Releasing an unmounted or still-pending presentation
aborts measurement; closing a Host terminal that remains `awaiting-start` disposes it directly
without graceful or force termination. Immediate headless and test compositions continue to supply
complete geometry at creation.

Before the renderer document paints, Electron fills the native window with the default background
from the same launch profile sent to Terminal Host. That value is only a startup bridge: committed
Host frame metadata owns the live terminal background in both Electron and Browser clients, so
Desktop Main never maintains a second runtime terminal palette.

Desktop hides the native title bar while retaining the operating system's window controls. The
Desktop renderer marks only the sidebar's top chrome and unoccupied horizontal Tab-track background
as native window-drag regions; Tabs, groups, the sidebar toggle, and the new-Tab control remain
interactive non-drag regions. On macOS, Main leaves the Window Controls Overlay disabled, positions
the native traffic lights at `(8, 12)`, and the renderer centers the sidebar toggle in the same
`40px` horizontal Tab row after the reserved native-control space. The toggle position is itself a
non-drag region, its reserved leading lane is never marked draggable, and visible sidebar drag
geometry begins after that lane so the complete pointer target remains interactive.

Desktop reads the terminal machine's hostname once at the trusted Main composition boundary,
validates it as runtime configuration, and includes it in every Electron or Gateway client-session
bootstrap. The shared Web renderer combines that hostname with committed Host title/CWD metadata
and applies the Workbench-owned Warp-scoped policy. Renderer input, Browser location, and transport
peer names cannot replace this hostname. Natural title and current CWD remain per-page transient
observations; only fallback and explicit custom Tab titles enter the shared-document coordinator.

Each successful request receives a fresh AttachmentId, capability bootstrap, and private `MessagePort`. For one running presentation, every writable attachment receives read/input/resize capability, while serialized opens request the resize lease only when no active lease owner exists. Later attachments remain observers until the owner detaches. The Host then promotes the earliest admitted live resize-capable observer, publishes its attachment-scoped ownership change before acknowledging the detach, and Desktop forwards the change without reopening that surface. Open, release, and retirement operations are serialized per presentation, so a concurrent release/open cannot bypass an existing observer or create two owners. Main returns a correlated release acknowledgement only after detach completes; a cleanup failure retains the same request for an exact later retry and cannot invalidate another projection request. Exited and failed terminals attach read-only with their retained final revision and typed outcome. Multiple visible split panels and page clients may hold independent attachments, with no global attachment threshold.

`HostConnection` reads and validates the fixed terminal-frame header once, then routes the binary record directly by AttachmentId. Main performs no VT parsing, row reconstruction, Kitty image loading, JSON/base64 conversion, or per-cell pass. At the Electron capability boundary, the broker copies only the routed payload into an exact-size `ArrayBuffer` before structured clone, so envelope headers and coalesced neighboring records never cross the renderer port. The renderer strictly decodes bounded OBKG raw pixels inside OBTF and never receives a path, URL, temporary-file handle, or shared-memory capability. OBPB format `0.0.1` local history render buffers then use the structural-view boundary; Gateway delivery remains externally validated. History, selection, copy, link, and accessibility behavior remains attachment-scoped. Normal bottom-following use creates no history cache or search Worker; these resources are activated only by their existing interaction contracts.

A correlated Host `TypedFailure` remains a typed `HostRequestError`; `HostConnection` retains the
response correlation ID, and `ProjectedTerminalBroker` adds the exact renderer-command operation
without replacing the Host code, message, or retryability. A failed row read also retains its
renderer request sequence so only that history flight can settle. Client-owned copy and link effects
use their request sequence instead. Electron and Gateway forward the same bounded failure contract,
and neither broker retries user effects from that diagnostic identity.

Electron attachments commit selection export through Main's clipboard and open allowlisted links through Electron. Gateway attachments receive bounded selection bytes, commit them through the Browser Clipboard API, and open `http`, `https`, or `mailto` targets through a local Browser gesture. The Browser never receives a generic Electron effect port. Attachment transport backpressure retires only that attachment; the visible surface requests a fresh projection and full snapshot.

An unexpected shared Host-connection close publishes one retryable `host-connection-unavailable`
failure to every still-live attachment before requesting its cleanup. Each visible surface freezes only
its last committed projection and enters the same attachment-local recovery path; it does not wait for
another user command to discover that the Host transport has already ended.

A Host-admitted OSC 52 plain-text write is different from user-initiated selection copy. Main routes
the bounded attachment event without interpreting its text. The shared renderer shows the localized
exact-text review and calls the already scoped platform clipboard effect only after explicit approval.
Cancel, Escape, attachment retirement, renderer replacement, or an unavailable effect writes
nothing. Neither Main nor the renderer persists request text or a trust decision; Host delivery to
one attachment means another Desktop or Gateway page cannot receive the same request after handoff.
Bell remains a local visual surface cue, and OBTF progress remains quiet presentation metadata;
neither requests window focus or a system notification. Semantic command completion is separate:
after the user enables the default-off device-local policy, the renderer may present a localized
terminal status and use its page-local notification capability. PTY output cannot request
notification permission, and the completion payload contains no command text.

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
- deferred visual creation with no child before mounted geometry, exact start-before-attach ordering,
  duplicate and conflicting start behavior, start failure without attachment, pending direct
  disposal, and matching first renderer/Host geometry;
- concurrent viewers with exactly one resize owner, earliest-observer promotion, dynamic renderer ownership, and serialized release/open ordering;
- fixed-header frame routing to only the matching AttachmentId, with exact-size structured-clone payload isolation;
- sandboxed end-to-end input, output, history, search, selection, copy, links, exit, renderer reload, and Electron Main restart;
- development and packaged Shell-resource resolution, resource presence and privacy checks, real Zsh
  OSC 133 output, and one macOS Electron path from manual Settings policy through a long-command
  completion reminder without replacing the projected attachment;
- one Electron renderer and two ordinary Browser pages sharing canonical Workbench and terminal state while retaining fresh page sessions, Client Views, and attachment identities, including Browser copy, link, disconnect, rejection, and reconnect behavior;
- user-quit preserve/cancel/stop, automatic preserve for idle running terminals, exact reuse of Panel-close protection, hidden-terminal stop, failed-CAS suppression, partial-stop retry, unreferenced-binding drain, unknown-Host-orphan refusal, update-restart preserve, development Host stop/stage/ready ordering, and fresh identity after explicitly starting a stopped session.

The Desktop terminal-lifecycle adapter model checks that a fresh deferred binding waits for mounted
geometry, start failure returns to that wait without committing geometry, start success commits the
exact observed geometry, and a queued close follows either provision or start before retirement.
Existing TLA+ models continue to check independently bounded terminal binding recovery and exit-event recovery, lifecycle-adapter serialization, projection demand, and Workbench removal safety. The recovery checks separately exercise Host epoch/create/document-generation behavior and durable exit admission/publication across Main release and reacquisition; every declared recovery action must have a reachable TLC witness. `DesktopLifecycle` keeps application lifetime, Browser access, development runtime, Project Folder acquisition, renderer document, terminal binding close, and window state as seven scenario-isolated configurations in one registered artifact; each configuration preserves its transition system, invariants, progress properties, and required action witnesses without multiplying the seven state spaces. Its Project Folder-acquisition configuration checks one live request, selected-only mutation, cancellation and failure without mutation, stale or duplicate Workbench targets, renderer retirement, and late native completion. Its window-state configuration checks ordered normal-bounds captures, serialized persistence, transient full-screen exclusion, flush completion, and disposal across writable and unavailable storage branches. Its renderer-document configuration checks ignored same-document and subframe navigation, generation-scoped control replacement, and bounded load progress. Its application-lifetime configuration checks prompt, preserve, stop, update, and development Host restart paths; stop-before-close ordering; durable restart staging and plan readiness; unreferenced binding drain; cancellation before effects; clean exit; and the explicit forced exit escape. Its development-runtime configuration checks failed-build preservation, coalesced process generations, ready-plan ordering, and close without relaunch. Its terminal-binding-close configuration retains exact-target mutation and unrelated-binding preservation. The Desktop resize-lease model checks one serialized open/release operation, one owner, earliest-observer promotion, owner-only canonical geometry mutation, and eventual operation completion. The Desktop shared-document-store model additionally checks exact-next CAS, conflict preservation, serialization, unavailable storage, generation disposal, and progress through revision `2`. The Desktop multi-client model checks two sessions, missing/available bootstrap, equivalent and divergent races, effect publication ordering, attachment/session ownership, disconnect, quiescing handoff, unavailable storage, and bounded publication progress. The Desktop write-ahead model checks matching durable intent before provision, restore, close, or stop dispatch; failed-commit preservation; restore without replacement creation; renderer disposal and reload replay; and live-generation progress for one abstract presentation. Workbench's terminal-stop model checks layout retention, stopped-without-binding, retry identity, and fresh PresentationId generation. The repository imposes no new throughput acceptance gate for this integration slice; the optional benchmark remains a diagnostic tool.

## Development

`pnpm desktop` owns one development runtime containing the Web development server, Electron, and a
supervised Terminal Host. The runtime follows this update policy:

| Successful change                                    | Development action                                                                                                           | Preserved state                                                       |
| ---------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| Web renderer or source-built frontend package        | Reload the renderer document through the Web development server                                                              | Canonical Workbench document, Desktop bindings, Host terminals        |
| Electron Main, preload, or a source-built dependency | Flush the renderer document, restart Electron, and reconnect to the same Host                                                | Canonical Workbench document and Host terminals                       |
| Terminal Host Zig source                             | Stage every running presentation as a fresh durable provision, stop the old sessions, replace the Host, and restart Electron | Project, Workspace, Tab, Panel, split, title, and launch CWD topology |

A failed Electron or Host compilation leaves the last running processes untouched. Host replacement
starts only after the new binary is complete and the old renderer has durably staged fresh terminal
provisions; an unavailable staging path aborts replacement. The restarted presentations receive
fresh Workbench presentation, Desktop binding, Host, creation, terminal, and attachment identities.
Their processes, shell state, transient metadata, selection, and scrollback are intentionally lost.
This policy is development-only: packaged update restart continues to preserve retained Host
terminals.

The Desktop development runtime owns one serialized update workflow. A newer successful build that
arrives during replacement is coalesced and applied afterward; every changed Host binary is admitted
even when consecutive observations see the same source digest. Captured binaries are bounded to the
active, rollback, and pending generations, and closing the Rsbuild server prevents all relaunch.
The `development-runtime` configuration of `DesktopLifecycle` checks two bounded Host and Electron generations, failed
builds, coalesced successful builds, durable restart preparation, process replacement, and fair
convergence. Deterministic runtime tests own command order, adapter failures, aborted preparation,
update coalescing, workspace cleanup, and captured-binary immutability. Forced operating-system
termination, compiler correctness, and PTY migration across a Host process remain outside these
claims.

The development runtime removes inherited `PAGER` and `GIT_PAGER` overrides only from the supervised
Terminal Host environment. Interactive shells therefore select their configured or default pager,
while Electron, build processes, packaged launches, and the invoking developer environment remain
unchanged.

Start the canonical Web development server and Desktop together:

```sh
pnpm desktop
```

`pnpm desktop` builds the Browser-access Server, Gateway, and static Web resources, then runs the Web development server and the Desktop-owned native/Electron development lifecycle. Electron defaults to `http://127.0.0.1:3000/`; an explicit loopback Web server can be selected with `ORCHBEE_DESKTOP_RENDERER_URL`. Open **Settings → Browser access** to start loopback access or explicitly enable trusted-LAN preview. Desktop never prints or returns the capability-bearing URL to renderer state; its Main process performs the Copy Link and Open Browser effects. Set `ORCHBEE_DESKTOP_GATEWAY=0` to remove Browser-access resources from a development launch.

Build an unsigned current-platform application bundle with:

```sh
pnpm desktop:package
```

The command builds the Terminal Host, Web renderer, Browser Server, Gateway, and Electron Main/preload bundles before packaging. The Web build is both a read-only `orchbee://app` resource and the on-demand Browser renderer root; the native Host, Server, and Gateway entries remain available outside `app.asar`. The output is generated beneath `apps/desktop/out/` and is not a signed installer or release artifact.

`ORCHBEE_TERMINAL_HOST_BINARY` selects an absolute development/test Host binary. `ORCHBEE_TERMINAL_SCROLLBACK_BYTES` selects the temporary development startup/default scrollback budget in decimal bytes from `0` through `1000000000`; the built-in default is `10000000`. `ORCHBEE_GATEWAY_ENTRY_PATH`, `ORCHBEE_SERVER_ENTRY_PATH`, and `ORCHBEE_WEB_RENDERER_ROOT` select explicit development Browser-access resources when the default sibling builds are not used. `ORCHBEE_DESKTOP_RUNTIME_DIRECTORY`, `ORCHBEE_DESKTOP_ENDPOINT_ID`, and `ORCHBEE_DESKTOP_RECOVERY_DIRECTORY` isolate controlled development and system-test state. They are not a versioned Host installation or discovery mechanism. The default POSIX runtime uses the short stable product-scoped child of the trusted per-user runtime location described above; an explicit runtime directory is still prepared and validated by the native endpoint-security boundary.

`pnpm desktop` internally sets `ORCHBEE_TERMINAL_HOST_SUPERVISED=1` so Main retries only the Host process owned by the development runtime, and supplies an owner-only `ORCHBEE_DESKTOP_DEVELOPMENT_RESTART_PLAN` path for the generation readiness handshake. These two variables are private development composition controls, not supported product configuration or a production update mechanism.

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
