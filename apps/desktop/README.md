# Desktop

Sandboxed Electron product client and scoped local broker for Orchbee's native Terminal Host.

## Current scope

Desktop composes the shared Workbench with Host-backed projected terminals. Workbench owns Project, Workspace, Tab, Panel, and `TerminalPresentationId` state; Electron Main owns durable technical bindings and Electron-only privileged effects; `apps/terminal-host` owns PTYs, processes, Ghostty VT state, history, and real terminal identity. A Browser client commits clipboard and allowlisted link effects in its own page. None of the shared Workbench document is stored in the Host.

Main restores the durable `TerminalPresentationId` to `TerminalBindingId` registry, owns one canonical Workbench shared-document coordinator, and transfers one atomic allowlisted capability bundle through the sandboxed preload. Startup creates no fixed terminal or attachment. Its eleven ports have one fixed order:

- client-session control delivers one atomic document/terminal-observation bootstrap, accepts complete exact-next shared-document proposals, and publishes one ordered canonical event stream;
- projection control opens and releases scoped Host attachments only for displayed terminal viewports;
- Agent-observation control accepts one correlated full-snapshot request and publishes complete, revisioned provider-neutral snapshots plus exact semantic events after their state update, without carrying provider payloads, prompts, transcripts, commands, tools, correlation capabilities, or a decision channel;
- Agent-integration-management control projects a bounded ten-entry read-only local inventory and accepts only correlated refresh, enable, and disable commands for Orchbee's exact managed entries. It carries no executable or resource path, command output, generic process/filesystem authority, or remote-host mutation authority;
- application-lifetime control carries only exit reason, user choice, quiescing acknowledgement, shared-document revision, aggregate progress, typed failures, and the Workbench presentation identities needed to resume an in-progress development Host restart;
- Project Folder acquisition selects one native existing directory or explicitly creates one absent child under a selected parent; correlated results distinguish no effect, created, unavailable and unknown outcomes;
- Project Worktree control discovers repositories only from the source Workspace resource Directories and creates one managed Worktree from repository, base, branch, and Workspace-option values; renderer commands never carry a target path;
- terminal-launch-profile control projects and mutates one bounded device-local catalog of system-shell or explicit-executable launch choices. It carries no environment, CWD, secret, project payload, or generic filesystem capability;
- terminal-default-settings control projects and mutates only the bounded device-local default scrollback byte count. It carries no running-terminal, Host, or filesystem authority;
- clipboard-read control accepts only correlated plain-text read requests and returns one terminal-input-bounded text value or typed failure. It exposes no write, alternate format, image, history, or generic Electron capability.

- user-preferences control reads and updates the shared device preference record through the matched native bridge; it carries source-bound patches and confirmed snapshots, never paths or generic I/O.

Client-session and projection payloads come from `@orchbee/workbench-client-protocol`; canonical Agent snapshots and events come from `@orchbee/agent`. `@orchbee/desktop-client-protocol` owns the eleven-port bootstrap and its Agent observation, integration management, application lifetime, local Project Folder and Worktree, terminal launch profile, terminal default, and clipboard contracts. Main and Web adapt these contracts to `MessagePort` without introducing transport details into the shared Workbench runtime.

Projection target selection remains Main-owned. A request carries only `TerminalPresentationId` and viewport geometry. Main resolves canonical containment and requires local Directory placement; a legacy Environment placement is rejected before opening a runtime. Main rechecks canonical authority after open and disposes a provisional attachment before publishing if its target is no longer current.

The renderer has no Node integration, generic Electron IPC, filesystem read capability, Host inventory, or raw terminal-lifetime authority. It cannot invoke raw provision, restore, close, or storage CAS effects directly. Preload accepts exactly one bootstrap, verifies all eleven ports together, preserves their fixed client-session/projection/Agent-observation/Agent-integration-management/application-lifetime/Project-Folder-acquisition/Project-Worktree/terminal-launch-profiles/terminal-default-settings/clipboard-read/user-preferences order, and attests context isolation and sandboxing. The Project Folder port accepts no renderer path and exposes no enumeration or mutation primitive. The Project Worktree port accepts no source or target path: discovery derives repositories from canonical Workspace resource scopes, while creation accepts only returned opaque identities and Git values. A separate context-isolated file-drop adapter exposes only the current platform and Electron's `File`-scoped backing-path lookup; it cannot accept an arbitrary path string, read file contents, enumerate a directory, or perform an IPC request. One dropped disk `File` may carry a file or directory backing path without granting child enumeration. The Web renderer admits that adapter only for a user drop, applies bounded Ghostty-compatible POSIX live-buffer escaping, publishes the prepared text directly, and never appends Enter. A partial or malformed bootstrap starts no renderer runtime.

Main owns one volatile provider-neutral Agent runtime from application construction through shutdown. Replacing or crashing a renderer retires only its broker; each replacement renderer obtains a fresh full snapshot from the same runtime. Desktop restart deliberately begins with an empty snapshot, and no Agent observation is persisted. An exact private APC `orchbee.agent.v3` launch envelope can create only a capability-free, heuristic presence session with unknown status; it becomes stale after 30 seconds, detaches after 120 seconds, emits no semantic event, and has no notification authority. It also triggers a lazy read-only inspection of only that closed integration ID. Rich observation for Claude Code, Codex, Grok, OpenCode, Pi, OMP, Gemini CLI, GitHub Copilot CLI, Cline, and Qwen Code uses the explicitly installed bundled [Agent observation adapters](resources/agent-observation-adapters/README.md). Each Agent-bound adapter validates its upstream contract, discards prompts, tool input and output, transcript paths, working directories, error text, and unknown fields, and sends only the shared normalized v3 allowlist to a private Unix-domain socket. The application-owned closed registry, rather than adapter input, grants adapter provenance, accepted events, and maximum capabilities. Visibility travels in the socket envelope. Only after one admitted local or reverse-forwarded socket returns the exact v3 ACK does the adapter independently write a private APC correlation half directly to every resolved outer TTY for direct, tmux, Zellij, and nested-mux sessions. Desktop admits rich semantics only after both halves join exactly for a live presentation generation; APC alone cannot manufacture idle, attention, outcome, action, or notification authority. Public notification channels including OSC 9, OSC 777, OSC 99, and OSC 1337 are never used for observation. The [Agent observation ingress contract](docs/agent-observation-ingress.md) owns the join state model, timer and generation invariants, reference scope, and correctness evidence.

Desktop supplies only `ORCHBEE_AGENT_OBSERVATION_SOCKET_V3` through the non-persistent `CreateTerminal.environmentOverlay`, never durable `TerminalLaunch`. The explicitly sourced Bash, Zsh, or Fish integration augments eligible interactive `ssh` calls with `StreamLocalBindMask=0177`, `StreamLocalBindUnlink=yes`, and a random `/tmp/orchbee-agent-v3-*.sock` reverse Unix-socket forward to that local socket. Existing aliases/functions, noninteractive and forwarding-only SSH modes, explicit arguments, and OpenSSH configuration remain authoritative. No HTTP listener, TCP port, Bearer credential, inherited terminal secret, remote file installation, or durable remote cache is used. Hook, socket, mux resolver, APC, and SSH-forward failure never blocks the Agent, SSH, or terminal. Adapter identifiers remain ingress provenance; product status, navigation, and attention behavior consume only the canonical Agent model and do not branch on an adapter. The owning [Agent package guide](../../packages/agent/README.md) and [proposal](../../proposals/agent/agent-runtime-observation.md) define the implemented boundary and still-reserved product decisions.

Desktop separately owns explicit local [Agent integration management](docs/agent-integrations.md). Startup performs no provider inspection and installs nothing. Explicit Refresh inspects all providers; a recognized direct launch inspects only that provider. If its integration is not enabled, a quiet bottom-right reminder can open Agent Integrations Settings but cannot mutate configuration. Only the user's explicit Settings action publishes the versioned durable Capsule and updates Orchbee's exact guarded provider entry; Main re-inspects before reflecting success. Persisted hooks guard missing executables and payloads, provider-local modules catch missing Capsule imports, self-contained extensions do not depend on an application-bundle path, and recognized legacy paths become explicit `update-required` migrations. Conflicts remain unchanged, remote hosts remain manual-only, and disable preserves unrelated Agent configuration.

Desktop context-menu and custom-shortcut Paste use the Main-owned clipboard-read port. An idle platform-default paste chord preserves native editable-element delivery, allowing the shared terminal adapter to distinguish plain text from an image-only clipboard and emit the Agent-compatible `Control+V` pair without reading or transferring image bytes. An ordinary terminal selection remains Host/Canvas state and does not occupy a document-global DOM Range, so the hidden textarea stays the sole native input and IME owner. A bounded terminal-root fallback admits paste only from package-owned presentation descendants; application-owned controls such as the Find input retain native editing. When a manual selection Copy or another Paste Action is still committing, the default chord uses the Action path; Paste waits for that exact Copy operation to settle before it requests current plain text, then reuses the shared terminal paste sanitation, review, return-to-bottom, and selection-clear path. A failed Copy does not synthesize text or retry: Paste reads whatever plain text the platform clipboard contains after settlement. Browser Preview pages receive no Desktop clipboard-read port and retain their existing local Browser policy.

OSC 52 clipboard writes use the existing projection port rather than a generic preload clipboard API. Main remembers only the exact first Host request forwarded to an attachment; the renderer's automatic commit decision must repeat its attachment, sequence, and bounded text exactly. A matching positive decision performs one Desktop-owned plain-text write and consumes the pending request, while mismatch, duplication, broker backpressure, page retirement, and attachment disposal perform no write.

Main loads and validates the shared document once before admitting clients. An available document restores Project, Workspace, Tab, Group, durable fallback/custom Tab titles, Panel/Split structure, presentation placement, and lifecycle intent; every page derives a fresh in-memory Client View. The Desktop Web composition may seed only the focused Panel and displayed child from one exact surviving `TerminalPresentationId` stored for that Electron page session. Active navigation, disclosure, focus, maximization, active child, natural terminal title, current CWD, and Split weights do not enter the shared file. A missing document projects every recovered binding as a standalone `retain` Tab and gives each joining page the same revision-`0` initialization seed. The first accepted proposal persists that seed before Main performs restore/provision effects. An unavailable load or failed first commit creates no replacement document or terminal effect. Unreferenced technical bindings remain preserved but are not silently adopted into the shared document.

The coordinator remains transport-neutral and Electron uses a direct `MessagePort` adapter. `apps/web` owns the shared Workbench renderer and an independent in-memory Browser Preview. Development Electron uses its User-Agent for adapter discovery on the loopback Web origin; packaged Desktop serves the Web build through the standard secure `orchbee:` origin. Renderer-document navigation and session teardown do not end retained Host terminals.

Desktop requires the single current Host protocol `0.0.25` exactly; pre-release builds do not negotiate or document historical protocol variants. Electron uses `42.8.0`, the final release in the major immediately preceding the current Electron major selected for this repository.

`HostConnection` owns one connection-global outbound window across every broker caller: at most 64 unanswered request records and 4 MiB of encoded request bytes. It retains that admission until the correlated response or connection failure. A full window or a Node writable that has returned `false` rejects work before handoff with retryable `host-transport-backpressure`; no request write resumes before `drain`. If a handed-off request reaches its deadline, Desktop retires the connection and rejects all pending and future requests with non-retryable `host-request-outcome-unknown`, so input is never replayed after an ambiguous result.

Each projected-terminal broker keeps at most 64 commands handed to the Host, with an input-specific subwindow of 16 so one terminal's wheel or key burst cannot ordinarily consume the connection-global window. It absorbs a renderer burst in one arrival-ordered pending window bounded to 1,024 validated non-resize commands and 4 MiB of their serialized command data. A completing request immediately hands off the oldest dispatchable pending command without allowing a later command to overtake a blocked input head; resize retains its separate latest-value coalescing slot. Commands admitted within those bounds neither report renderer backpressure nor retire the attachment. A genuinely full pending window still fails closed, drops work that was never handed to the Host, and retires only that attachment.

Inbound Host records retain their wire order across JavaScript Promise settlement. `HostConnection`
claims each correlated response as it is decoded, settles that response, and holds only following records
behind one event-loop barrier so the response continuation runs before a later Host event. An event that
precedes the response remains first, consecutive event/frame delivery remains synchronous, and a
transport end follows every already-decoded record without reopening request admission.

Before it constructs `HostConnection` on POSIX, Desktop's in-process native endpoint adapter validates the runtime directory and actual socket, connects, and authenticates the server's effective UID from kernel peer credentials. Only an authenticated same-UID connection can suppress Host launch, and every retry or later Desktop reconnect repeats the check. The adapter transfers the authenticated descriptor directly into a Node socket and starts no helper process, so `serve` is the only Terminal Host product process. Its ownership, failure, evidence, and rollback contract lives in [`docs/posix-host-endpoint.md`](docs/posix-host-endpoint.md).

Desktop scopes one Terminal Host endpoint to its product `userData` identity, which separates OS users, product profiles, and release-channel data roots. The short POSIX runtime path is a stable product-scoped child of the operating system's trusted per-user runtime location (`/run/user/<effective-uid>` on Linux and Electron's per-user temporary location on macOS), never a deterministic child of shared `/tmp`. The Windows named-pipe identity is derived from the same product scope. Explicit endpoint and runtime overrides exist only for controlled development and tests; POSIX overrides still pass through native no-follow owner, private-mode, and ACL preparation and validation. After every successful handshake, Desktop sends the current terminal default through `SetTerminalDefaults`, so reconnecting to an already-running singleton has the same result as a fresh CLI launch. New terminals use a light profile aligned with the Workbench canvas; retained terminals preserve the profile stored with their existing Host session. Common preferences share Native Site's configuration-scoped store; host typography and pixel presentation remain renderer-local. Scrollback updates the Host default for future terminals.

Runtime configuration resolves every resource and storage path with the declared platform path rules, including the CLI launcher. Platform-simulated checks use matching path fixtures; tests that access the real filesystem use the executing platform. Windows drive and UNC resource paths remain Windows paths regardless of the test host.

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

A renderer document start retires the previous page controls and records the pending generation. `did-finish-load` publishes one fresh atomic bootstrap only for the latest generation; retirement failure keeps that generation closed. Main permits top-level navigation only within the configured renderer origin so development reload and packaged Browser Router document requests can complete; cross-origin navigation and new windows remain denied. Same-document Browser Router updates and subframe navigation preserve the current generation and every live control. Renderer-process loss and window close remain explicit retirement paths, while Host terminals and the canonical shared document survive page retirement. On same-window reload, the replacement Web composition independently restores its bounded session focus hint after validating that the exact presentation still belongs to the bootstrapped document; Main receives no Client View or browser-storage capability. This model does not add projection retries, control-port timeouts, or a new IPC contract.

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
revoke delivery, so a late native completion cannot mutate the replacement page. Existing selection is read-only. `New Folder…` uses a native-selected parent token and a single name,
then atomically creates one absent directory only after explicit Create. It retains committed
Folders after later failures, observes the actual Project Action and exact canonical publication,
and enters only the captured Primary Workspace through the existing terminal lifecycle. The maintained behavior,
evidence, limits, and rollback contract live in
[`docs/project-folder-acquisition.md`](docs/project-folder-acquisition.md).

## Managed Project Worktree creation

Desktop is the only current composition that injects managed Worktree creation. Main discovers Git
repositories from the source Workspace's current available resource Directories, deduplicates them by canonical
Git common-directory identity, and returns editable base and new-branch defaults. The renderer shows
the fixed managed location read-only and can request an optional Workspace, but it cannot choose or
submit a filesystem path.

Packaged Desktop uses `~/.orchbee`; each development checkout uses
`~/.orchbee-dev-<scope-digest>`, where the stable digest derives from that checkout's canonical
source path. Managed Worktrees live under `<data-root>/worktrees/<opaque-id>`. This delivery has no
application-level or Project-level location setting. Main records recovery intent before Git
mutation, commits the Workbench Directory and optional Workspace atomically, and retains every known created Worktree on product failure, including source closure or regrouping, and any ambiguous result instead of recursively deleting user data. See
[`docs/data-root.md`](docs/data-root.md) and
[`docs/project-worktree-creation.md`](docs/project-worktree-creation.md).

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

The version-3 binding store contains a canonical BindingId-ordered collection. Every record has one unique BindingId, PresentationId, CreationId, bounded launch profile, and exactly one durable state: `planned`, `create-requested`, `retained`, `exited`, `closing`, or `lost`. Retained identities are unique and must repeat their owning CreationId. The file is parsed as untrusted input, updated through owner-only atomic replacement, and never rewritten automatically when invalid or unreadable. Launch environment values never enter diagnostics. Every default launch declares `TERM=xterm-256color`, `COLORTERM=truecolor`, and the Ghostty compatibility identity `TERM_PROGRAM=ghostty`, and preserves the Host’s inherited `NO_COLOR`, `FORCE_COLOR`, and `NODE_DISABLE_COLORS` policy. Capability declarations do not override a user’s color choice; an explicit Launch Profile environment may still override inherited values. Controlled UI fixtures remove automation-injected color policy before starting both Desktop and its fresh Host, rather than changing production launch semantics. `TERM` remains the portable baseline because Desktop does not install `xterm-ghostty` terminfo; the compatibility identity lets applications select the Kitty graphics and keyboard protocols that Orchbee implements through its pinned Ghostty VT without claiming a Ghostty application version. New POSIX terminals also receive `ORCHBEE_SHELL_INTEGRATION_DIR` from a code-resolved application resource path: development resolves it relative to the built Main entry, and packaged applications resolve it beneath `process.resourcesPath`. Ambient environment cannot replace that path. Desktop ships its own Zsh, Bash, and Fish scripts and one-shot loaders as package resources. After profile selection, supported new local Zsh, non-login Bash, and Fish launches load the shared integration automatically without Main reading or modifying a startup file; unsupported launch forms remain unchanged and Settings exposes the exact manual fallback commands. Running, restored, Windows, and remote terminals are outside this refinement. See [`docs/kitty-automatic-shell-integration-alignment.md`](docs/kitty-automatic-shell-integration-alignment.md).

The launch-locale resolver is a deterministic policy over the target platform, Desktop's inherited `LC_ALL`, `LANG`, and `LC_CTYPE`, the Darwin system locale, and Darwin's installed locale inventory. It emits only the environment delta captured by a new terminal launch. On Darwin that delta explicitly sets or removes all three locale variables, so the PTY locale cannot depend on the ambient environment of a separately supervised Host. Retained launch records and running PTYs never change retroactively.

| Input condition                                    | Launch environment decision                                                                                                                                                               |
| -------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Windows                                            | Emit no locale delta.                                                                                                                                                                     |
| POSIX with an effective UTF-8 locale               | Preserve the effective locale; Darwin projects all three locale variables across the Host boundary and supplies `LANG` when it was absent.                                                |
| Darwin with absent `LANG`                          | Resolve `[NSLocale currentLocale]` through Electron and emit `<language>_<country>.UTF-8` only when `/usr/bin/locale -a` reports that exact locale; otherwise fall back to `en_US.UTF-8`. |
| POSIX with an ineffective higher-precedence locale | Remove conflicting `LC_ALL`, repair ineffective `LANG`, and replace an explicitly ineffective `LC_CTYPE`; Linux uses `C.UTF-8` as its fallback.                                           |

Every emitted locale name is UTF-8, an explicit valid Desktop locale remains authoritative, and a conflicting higher-precedence Host value never survives in the child. Electron Main reads the bounded locale inventory once through the absolute macOS system utility before resolving launch configuration; an unavailable, failed, or oversized inventory admits no derived candidate and therefore selects the fallback. The Darwin missing-`LANG` decision adopts only `src/os/locale.zig` from the pinned Ghostty revision `9c3ec931d64561a8407dde7ac984ce156ae91539`; Desktop replaces Ghostty's process-global `setlocale` validation with the read-only inventory and replaces its same-process environment assumption with an explicit Desktop-to-Host locale projection. It does not adopt Ghostty's `LANGUAGE` translation preference.

## Local terminal defaults

Desktop and TUI share confirmed device preferences. The old terminal-default file is a one-time migration input. Shared persistence commits before Main applies scrollback to Host; application failures are visible and retry application without replaying the write. New terminals capture the latest Host default; running and restored terminals keep their existing configuration. See [Desktop settings](docs/terminal-settings.md).

## Local terminal launch profiles

The shared device catalog defaults to System shell with login disabled on every platform.
POSIX users can enable login; Windows cannot. Custom executables use absolute paths and literal
arguments. The shared store enforces catalog bounds, while concrete Host launch admission happens
before creation. The old userData catalog is migration input only.

Each new terminal captures its selected profile, resolves System shell through the matched helper
when required, then freezes its concrete durable launch. Custom profiles work without System shell.
Concurrent provision shares pending discovery; closing or disposal prevents later creation.
Retries and restored bindings retain the captured launch. Invalid storage and failed writes remain
visible without overwriting the source or falling back to another profile.
See [launch profiles](docs/terminal-launch-profiles.md).

Ordinary remote shells remain owned by the system SSH client the user invokes inside that PTY. The
terminal-launch-profile path does not wrap `ssh`, retain remote credentials or host records, add
OpenSSH environment-forwarding rules, install terminfo, or write a remote cache. A requested SSH
PTY can carry the standard `TERM=xterm-256color`; `COLORTERM`, `TERM_PROGRAM`, and any other extra
variables cross the connection only when the user's OpenSSH client configuration and the server
both permit them. The shared Settings surface provides one constant read-only remote diagnostic
command that reports the observed variables and distinguishes an unset TERM, a missing `infocmp`
program, and a missing terminfo entry. That command performs no network, installation, filesystem,
or terminal-input effect until the user explicitly chooses to run it in a shell.

No product maximum is imposed on terminals, bindings, or attachments. Active collections grow from shared product structure and visible demand; allocation, filesystem, Host, or memory exhaustion returns an operation failure instead of evicting or silently truncating another terminal. Host inventory is read through generation-scoped protocol pages rather than one bounded global list. Pre-association `TerminalExited` delivery uses a bounded identity-keyed admission buffer: admitted identities are never evicted, and an identity that has not acquired its binding association is retried before the adapter buffer is released. After the Host envelope is accepted, Desktop Main synchronously records the event in an owner-only durable pending journal before the fire-once Host listener returns. The separate 64-entry identity-keyed in-memory queue is only a scheduler/cache; it is not the crash recovery owner. The pending journal has its own 4096-entry capacity, while the restart-safe publication ledger and durable rejection journal have independent capacities. Journal-full or write failure is recorded as an observable durable rejection when possible, and the admission path fails closed if that rejection cannot be recorded. Unknown identities stay in durable pending ownership without a timer loop; an exact identity observation wakes promotion. Queue-full, shutdown, and observed pending entries may be promoted by the retry owner. A pending entry is removed only after successful durable runtime recording, adapter publication commit, publication-ledger admission, reconciler acceptance, and live publication; compaction failure therefore leaves durable recovery state. Shutdown transfers unresolved in-memory entries to the pending journal before completing the remaining application teardown.

The terminal-exit spill store migrates legacy v1 files with a bounded event array, normalizing each identity to its newest revision and atomically rewriting the file once. Legacy pending records remain v2 pending ownership. Legacy published records are discarded only after the already-loaded durable publication ledger confirms the identity and revision; without that evidence the startup migration fails closed and leaves the v1 file untouched. Oversized or malformed legacy input also fails closed.

`TerminalFailed` is a different result, not a synthetic exit. Desktop keeps an identity-keyed association buffer only while a binding is still acquiring its Host identity, then records the failure on that binding before exposing another projection. The failed terminal remains retained and authoritative in the surviving Host, so Desktop restart recovers the same typed failure from Host inventory; it is not copied into the exit-specific spill store or publication ledger. This does not promise recovery after a Host or operating-system crash.

The separate Workbench shared-document store accepts only the canonical version-3 document through one Main-owned serialized writer. A missing store starts at revision `0`; every later commit must name the exact current revision and advance it by one. Conflicts, non-next revisions, invalid files, and read failures preserve current data. Writes use owner-only temporary files, file synchronization, atomic replacement, and parent-directory synchronization where supported; a failure that may have crossed rename is reported as indeterminate. A client receives only an atomic bootstrap, typed proposal result, and ordered publication; it never receives a path, Node error, store capability, or general filesystem capability. See [`docs/workbench-shared-document-storage.md`](docs/workbench-shared-document-storage.md). The current store uses `workbench-shared-document-v2.json`. When that path is missing, one exact historical `workbench-shared-document-v1.json` may migrate every Directory to local placement while leaving the v1 bytes untouched. A present file at that path is the sole authority. Its valid v2 payload migrates strictly to v3 in memory; the next canonical commit writes v3 while preserving scoped resources and identities. No reader, alias, or migration is retained for the unshipped development topology schema.

Provisioning allocates and persists a fresh binding before creating a terminal. Recovery accepts only an exact HostEpoch/TerminalId/CreationId match and never substitutes a new shell for lost continuity. A lost create response may replay only the same CreationId and launch in the recorded Host epoch. Orphan, conflict, corrupt-ledger, and transport states fail closed.

Retain reconciliation is a separate restore-only operation. It succeeds only for a Binding that was already durably retained or exited, or became restorable after successful provision in the same Main lifetime. Missing, planned, closing, or lost bindings never allocate a replacement through the restore path. Restore failure remains durable retain intent.

One operation tail serializes lifecycle work per presentation while unrelated terminals progress independently. The Main coordinator persists and publishes the complete shared document containing provision, close, or stop intent before deriving the corresponding effect. Its FIFO proposal queue uses first-writer-wins for divergent concurrent complete documents and converges equivalent races. Close then persists technical `closing`, retires only that presentation's active projections, and delegates graceful/force/dispose behavior to the existing binding close lifecycle. A Panel or Tab remains in Workbench until every exact terminal close result completes; cleanup commits afterward, while the previously stored close intent remains replayable if cleanup fails. Renderer reload, a renderer crash, a normal macOS window close, and uncontrolled Main loss release that page's projection attachments but preserve Host terminals. A controlled application quit follows the policy below.

## Application lifetime

A fatal failure while constructing the Desktop application sets process exit status `1` before Electron quits. Ordinary single-instance rejection and normal window or managed-lifecycle exits retain a successful process status.

Shutdown and renderer-document retirement continue their independently guarded cleanup steps after a failure and report the first original thrown value, including `undefined` or `null`. A failed retirement cannot publish a replacement renderer bootstrap; a later retry must complete the retained cleanup first. [Application composition tests](src/main/composition/__tests__/DesktopApplication.unit.test.ts) cover failure identity, independent cleanup, and retry without repeating already completed terminal-runtime disposal.

A normal macOS window close releases only the renderer page. Command+Q, a menu quit, or closing the last window on Windows and Linux enters one coalesced user-quit workflow while the renderer is still available. When one or more presentations require protection under Workbench's Panel-close predicate, the application-wide dialog offers Keep Sessions and Quit as the default, Stop Sessions and Quit as the destructive choice, or Cancel before any stop effect. When every running presentation is observed idle, or no presentation remains, Desktop preserves the sessions, flushes the shared document, and quits without prompting. The future updater enters the same preserve preparation as `update-restart` without prompting; updater installation, signing, and release distribution remain outside this package.

Before preserve or stop preparation, Main acknowledges that the application-wide coordinator has entered quiescing. This acknowledgement orders the lifetime and client-session ports: other pages cannot race a new shared change into the preparation window. A replacement renderer may resume only the same quiescing mode after the former exit owner disconnects.

Development Host restart enters its own non-interactive quiescing mode. Before any stop effect, the renderer captures only currently restartable presentations and Main retains that exact target set across renderer-document replacement. A replacement renderer resumes from the Main-owned targets even when the shared document already records them as stopping or stopped. The workflow follows the same durable stop and Host-drain barriers, replaces each captured stopped presentation with a fresh provision identity without dispatching that provision to the retiring Host, and flushes the staged document. Main writes the generation-bound ready plan only after the exact staged revision is loaded and the old Host owns no binding or unknown terminal. Ordinary update restart remains the preserve path and never uses this destructive development mode.

Stop seals terminal creation for the exit-owner page, projects every visible and hidden Workbench presentation to durable `stop`, compare-and-swaps the complete shared document, and only then releases close commands. Successful close results change presentations to `stopped` while retaining Project, Workspace, Tab, Panel, split, title, and launch CWD. Hydration never starts a stopped presentation. Its visible Start New Session action replaces the stopped PresentationId with a fresh identity and provisions only that selected presentation.

When a durable stop reaches a final Host revision, Main opens one temporary read-only attachment after the terminal outcome and before Host Dispose. It archives the newest 5,000 final render rows or 10 MiB for that Presentation, whichever bound is reached first. Archive format `2` stores the final viewport projection with its complete OBKG resource snapshot and retains the independent graphics snapshot in every render-detail OBRW `0.0.3` chunk; its decoder accepts only format `2` and the exact current nested formats, without silent fallback. The Desktop-local store uses content-validated atomic files with private permissions and a 250 MiB global least-recently-used limit. A valid archive feeds the existing terminal renderer on the local Electron stopped surface, preserving graphics, scroll, search, selection, and copy while permanently rejecting input and resize. Missing, corrupt, timed-out, unwritable, or evicted archives fall back to the empty stopped state and never block Host disposal or application exit. Archives never enter the shared document, renderer bootstrap, Browser storage, or cloud. Ordinary close and Start New Session immediately remove the archive association; their best-effort store deletion and shared-document orphan reconciliation may leave an unreachable physical file for a later reconcile. Archived rows and images are never restored into or concatenated with the fresh VT.

After referenced presentations stop, Main loads the exact committed shared-document revision and closes every binding that it owns but the document does not reference. It then requires the binding registry to be empty and scans Host inventory. Unknown Host terminals are reported as unresolved; they are never silently killed. A partial failure keeps Desktop in an irreversible recovery dialog with Retry when supported and an explicit Quit Anyway warning. Quit Anyway may leave processes running; the durable `stop` intent remains replay authority for the next start. OS shutdown, logout, and forced termination are not claimed by this workflow.

## Demand-driven projection

Workbench mounts the application-owned terminal surface only inside displayed panel viewports. A live surface requests projection only while its lifecycle is `running`, `exited`, or `failed`; unmounting, switching Tabs, hiding another terminal context, or entering `closing` releases the request and Host attachment. An already painted surface entering managed `closing` keeps that renderer only as a frozen, non-interactive frame without transport, input, resize, frame-stream, metadata, or search ownership. It opens no replacement attachment and is disposed after a surviving Tab crosses its display barrier, or immediately when no Tab survives. Background Projects, Workspaces, Tabs, inactive child terminals, and otherwise invisible panels therefore hold no live renderer, GPU surface, frame stream, history cache, search Worker, or Host attachment; at most one transient outgoing Tab projection remains display-only during a handoff.

The default visual launch carries null geometry. Provisioning durably binds the presentation and
creates the Host identity in `awaiting-start`, but does not spawn a child or PTY. A visible shared
Web surface waits for its configured fonts and a positive mounted content box, then computes one
complete grid and physical cell geometry from that exact presentation configuration, padding, and
document DPR. Electron projection-open commands carry that geometry unchanged. Main
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

Every projection bootstrap includes the complete latest Host theme state retained by the lifecycle
adapter. The projection broker forwards an attachment-scoped `apply-terminal-theme` command as one
Host compare-and-set and publishes only the identity-checked complete result. It records that result
back into lifecycle recovery state before the renderer can use a later bootstrap. Reconciliation
reads Host inventory, requires the exact creation/Host/terminal identity, refreshes the same complete
state, and returns a dedicated reconciled event. Known unchanged failures and unknown transport
outcomes use the theme-specific result channel; an unknown outcome must reconcile before retry and
is never flattened into a generic projection failure. Desktop Main does not render or independently
choose terminal colors.

Desktop hides the native title bar while retaining the operating system's window controls. On
macOS, Workbench exclusively owns `Command+W` as Close Terminal; Main keeps the native Close Window
menu item clickable but does not register its accelerator, so a repeated or temporarily unavailable
terminal-close event cannot fall through and close the native window. Main also leaves the Window
Controls Overlay disabled, positions the native traffic lights at `(8, 10)`, and the renderer centers
the sidebar toggle in the same `36px` horizontal Tab row after the reserved native-control space.

### Native window drag-region projection

- **Problem and scope:** A Desktop user must be able to move a frameless native window from every
  unoccupied point in its `36px` top chrome without losing Tabs, groups, the sidebar toggle, the
  new-Tab control, or operating-system window controls. This projection covers the sidebar top row
  and a horizontal Tab bar. The vertical Tab Strip, content below the top chrome, Panel and sidebar
  interactions, resize separators, Tab cross-window drag, and the native move lifecycle are non-goals.
- **Inputs, data, and ownership:** Trusted Workbench layout supplies sidebar visibility, Tab Strip
  orientation, the horizontal chrome boundary, and the rendered control geometry; `windowControls`
  supplies the operating-system control side. There is no durable drag-region state and no missing
  input is inferred. Workbench owns the shell geometry, the Desktop renderer exclusively projects
  CSS app regions, and Electron and the operating system own native-control precedence plus pointer
  activation, movement, release, and cancellation.
- **Model and output:** The renderer applies this hit-test policy after every layout projection:

  | Horizontal-chrome target                                                                | CSS app region | Observable result                                |
  | --------------------------------------------------------------------------------------- | -------------- | ------------------------------------------------ |
  | Unoccupied sidebar top-row background                                                   | `drag`         | Starts native window movement                    |
  | Unoccupied horizontal Tab-bar background, including reserved leading and trailing space | `drag`         | Starts native window movement                    |
  | Sidebar-toggle position, Tab, group, or button geometry                                 | `no-drag`      | Retains the control's ordinary pointer behavior  |
  | Vertical Tab Strip or content outside the horizontal chrome                             | Unset          | Retains its existing non-window-drag interaction |

- **Invariants and transitions:** Native window controls and rendered Workbench controls always
  take precedence over a background drag region; no Desktop drag projection extends below the
  `36px` boundary. Showing or hiding the sidebar and switching Tab Strip orientation recompute the
  projection without starting, committing, or cancelling a native drag. Unsupported or absent
  horizontal chrome is an intentional no-op.
- **Dependencies and verification:** Electron's CSS app-region contract is the only platform
  capability. The controlled Desktop system trace checks the complete sidebar-row geometry, the
  horizontal Tab-bar default, every interactive exclusion, sidebar visibility transitions, and
  live control activation. Automated evidence does not move or focus an operating-system window;
  that remains outside the aggregate suite and inside the separately authorized manual native-window
  boundary.

Desktop reads the terminal machine's hostname once at the trusted Main composition boundary,
validates it as runtime configuration, and includes it in every Electron client-session
bootstrap. The shared Web renderer combines that hostname with committed Host title/CWD metadata
and applies the Workbench-owned Warp-scoped policy. Renderer input, Browser location, and transport
peer names cannot replace this hostname. Natural title and current CWD remain per-page transient
observations; only fallback and explicit custom Tab titles enter the shared-document coordinator.

Each successful request receives a fresh AttachmentId, complete capability bootstrap, positive capability revision, and private `MessagePort`. Desktop forwards that revision through the Workbench bootstrap, and every projected raw or semantic input command returns it to Host. For one running presentation, every writable attachment receives read/input/resize capability, while serialized opens request the initial resize lease only when no active owner exists. An admitted `focused: true`, key press/repeat, committed text or paste, application mouse press, or terminal-selection start records that writable projection as the latest desired owner; attachment, background state, viewport resize, pointer motion, release/repair input, and `focused: false` do not. Only an attachment that currently remains input- and resize-capable on a running terminal may generate lease activity or claim; an input downgrade invalidates its queued ownership intent. Desktop coalesces concurrent intent and serializes one explicit Host `ClaimResizeLease` at a time without delaying input. A committed transfer reaches Main as the old owner's revocation followed by the new owner's grant; the promoted renderer immediately measures its current surface and submits that geometry. On owner detach, Host promotes the remaining resize-capable attachment with the newest successful claim, breaking an unclaimed tie by earliest admission; Desktop reclaims for any newer remaining activity it recorded. Open, input-capability mutation, release, and retirement operations are serialized per presentation. Only the Main-owned projection lease can request a live input change; the broker submits the exact next revision to Host, validates the complete returned attachment identity, immutable read/resize state, resize lease, new input value, and revision, and only then publishes the state downstream. Repeated desired state is a no-op. A renderer command cannot escalate its own capability, and an invalid or unpublishable Host result retires the attachment rather than granting client authority. Main returns a correlated release acknowledgement only after pending capability work and detach complete. Renderer-local release may cross an already published asynchronous open before Main observes that release; an exact correlated late open with one transferred port closes only that port and keeps the shared control plus unrelated requests available, while invalid identity, port cardinality, or duplicate open still retires the shared control. While the Host request transport remains available, a cleanup failure retains the same request for an exact later retry and cannot invalidate another projection request. Once connection end is confirmed, Host connection teardown is cleanup evidence for that connection's attachments: Desktop sends no further Host cleanup request and settles the connection-scoped local lease/detach owners without changing the retained terminal. Exited and failed terminals attach read-only with their retained final revision and typed outcome. Multiple visible split panels and page clients may hold independent attachments, with no global attachment threshold.

Each `ProjectedTerminalBroker` also owns one bounded resize-command serializer for its attachment.
The serializer is either idle or has exactly one Host resize request active; while active, each newer
validated renderer measurement replaces the single pending measurement instead of starting a
competing PTY resize. Success or typed failure of the active request returns the serializer to idle
and immediately admits the latest pending measurement when the attachment remains live. The active
failure is still published through the existing typed renderer-failure path and is never silently
replayed; only a measurement that arrived after it began can drive the next request. Disposal or
attachment cleanup drops the pending measurement, other renderer commands retain their existing
concurrency, and a pending resize occupies no Host request-window slot until dispatch. Therefore a
burst of container measurements uses constant memory, submits at most one resize at a time, and
makes the newest pending geometry the next Host request without allowing an intermediate request to
overtake it.

`HostConnection` reads and validates the fixed terminal-frame header once, then routes the binary record directly by AttachmentId. Main performs no VT parsing, row reconstruction, Kitty image loading, JSON/base64 conversion, or per-cell pass. At the Electron capability boundary, the broker copies only the routed payload into an exact-size `ArrayBuffer` before structured clone, so envelope headers and coalesced neighboring records never cross the renderer port. The renderer strictly decodes bounded OBKG raw pixels inside OBTF and paints all three signed-z layers in Canvas2D or WebGPU; that frame path carries no URL, temporary-file handle, or shared-memory capability. OBRW format `0.0.3` local history render buffers carry their own OBKG snapshot and use the OBPB structural-view boundary. History, selection, copy, link, and accessibility behavior remains attachment-scoped. Normal bottom-following use creates no history cache or search Worker; these resources are activated only by their existing interaction contracts.

A correlated Host `TypedFailure` remains a typed `HostRequestError`; `HostConnection` retains the
response correlation ID, and `ProjectedTerminalBroker` adds the exact renderer-command operation
without replacing the Host code, message, or retryability. A failed row read also retains its
renderer request sequence so only that history flight can settle. Client-owned copy and link effects
use their request sequence instead. Electron forwards the same bounded failure contract,
and neither broker retries user effects from that diagnostic identity.

Electron attachments request only the protocol's literal `plain` selection export and commit the
decoded UTF-8 text through Main's clipboard. Neither projection command nor
platform effect exposes an HTML or raw VT format selector. Every composition opens allowlisted
`http`, `https`, or `mailto` targets through Electron or a local Browser gesture. A Desktop-effect
attachment additionally receives one optional local-file capability; the Browser never receives a
generic Electron effect port. Attachment transport backpressure retires only that attachment; the
visible surface requests a fresh projection and full snapshot.

That local-file capability accepts only a typed file target already derived from generation-pinned
terminal text. Main treats the path as untrusted: it expands a leading home marker, requires an
absolute path without control characters, and verifies that an explicit open target currently names
a regular file or directory before delegating to Electron's default application. Explicit open
failure returns the bounded typed link failure used by the localized renderer result. The capability
does not enumerate directories, resolve a Browser Preview path, or become a general preload API.

Image preview is narrower still. Main opens the requested path read-only and nonblocking, validates
the opened handle as a non-empty regular file no larger than 8 MiB, reads exactly that bounded
content, and requires the extension and magic signature to agree for PNG, JPEG, GIF, WebP, AVIF, or
BMP. SVG, HEIC, TIFF, unknown, mismatched, oversized, missing, and non-regular targets remain
external-open-only or unavailable. Each broker keeps at most one current preview request; replacement,
explicit cancellation, attachment cleanup, and disposal make late completion inert. Admitted bytes
cross only the attachment port in an exact `ArrayBuffer`, are not cached or persisted, and never
enter an archive. Preview unavailability is silent, while deliberate open failure remains
localized and visible. Removing the optional attachment capability rolls back both file activation
and preview without changing URL behavior.

An unexpected shared Host-connection close publishes one retryable `host-connection-unavailable`
failure to every still-live attachment before requesting its cleanup. Each visible surface freezes only
its last committed projection and enters the same attachment-local recovery path; it does not wait for
another user command to discover that the Host transport has already ended.

A Host-admitted OSC 52 plain-text write is different from user-initiated selection copy. Main routes
the bounded attachment event without interpreting its text. The shared renderer automatically
processes the exact request. A Desktop-effect attachment delegates the single privileged write to Main;
a client-effect attachment first calls its already scoped platform clipboard effect and reports
whether that local write completed. Attachment retirement, renderer replacement, or an unavailable
client-local effect produces no late or duplicate write. Neither Main nor the renderer persists
request text or a trust decision; Host delivery to
one attachment means another Desktop page cannot receive the same request after handoff.
Bell delivery is deduplicated by opaque terminal identity and Host sequence in the shared Workbench
lifetime before one local visual surface cue is admitted. Desktop currently registers neither the
separate opt-in sound effect nor the separate opt-in operating-system notification effect, so Bell
requests neither window focus, audio, nor a system notification by default. OBTF progress remains
quiet presentation metadata. Semantic command completion is separate:
after the user enables the default-off device-local policy, the renderer may present a localized
terminal status and use its page-local notification capability. PTY output cannot request
notification permission, and the completion payload contains no command text.

Known Host `TerminalExited` events enter the Main-owned retry/reconciliation path before they are resolved through the authoritative runtime registry, persisted to the exact binding, and published to Workbench by PresentationId. An identity that remains unresolvable is retained as a durable, observable reconciliation record and is promoted only when its exact identity is observed (or by the retryable known-entry paths); it is not spun in an unbounded unknown-identity timer loop. A recovered exited binding is restored into Workbench only until its identity-checked bootstrap event enters the managed natural-exit removal protocol. Its final revision remains readable during that interval; the resulting close intent disposes the Host terminal without process termination, then Workbench removes or promotes the exact Presentation according to its Panel topology. The reconciler reservation, adapter commit, publication-ledger record, and visible event are separate steps: a bootstrap can see the event only after all durable and presentation ownership steps succeed. Durability is confirmed before the first exit publication is consumed.

The Workbench `terminal-presentation-event-published` message is the single Main-owned lifecycle publication. A typed Host failure becomes a non-retryable `terminal-failed` presentation outcome; the reconciler retains it for later client bootstrap, and Workbench keeps the terminal projectable and read-only until explicit close. `ProjectedTerminalBroker` may also forward an identity-checked, attachment-local raw `terminal-exited` or `terminal-failed` notification so a live projection can synchronize with Host state; that renderer transport has its own monotonic revision filter, is outside the coordinator sequence, and is not a second Workbench publication. The two channels therefore do not provide a cross-channel exactly-once claim.

The stable lifecycle witnesses below map the cross-owner compatibility names to existing owner
tests and models. Unit/model rows control injected transports, clocks, storage, identities, and
failures. The exact headed rows use the ordinary Desktop Playwright suite and state their platform
limits; none authorizes the separate manual focus-stealing native-window test.

| Stable witness                                                                                                   | Executable owner evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| ---------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `terminal-parity/LC-01-open-lifecycle`                                                                           | [`../../packages/terminal/src/projected/__tests__/projected-terminal.unit.test.ts`](../../packages/terminal/src/projected/__tests__/projected-terminal.unit.test.ts), [`../web/src/workbench-client/projected-terminal-renderer/__tests__/startProjectedTerminalRenderer.unit.test.ts`](../web/src/workbench-client/projected-terminal-renderer/__tests__/startProjectedTerminalRenderer.unit.test.ts), and the Desktop lifecycle-adapter tests execute open, rollback, same-instance retry, duplicate open, disposal, and DOM ownership.                                                                                                                                                                                                                    |
| `terminal-parity/LC-02-failure-presentation`, `terminal-parity/LC-10-outcomes`, `terminal-parity/LC-11-recovery` | Web projected-surface/renderer tests and [`../web/src/workbench-client/projected-terminal-surface/model/__tests__/ProjectedTerminalRecoveryController.unit.test.ts`](../web/src/workbench-client/projected-terminal-surface/model/__tests__/ProjectedTerminalRecoveryController.unit.test.ts), plus Desktop recovery, natural-exit, renderer-reload, and Main-restart traces, execute failure/exit/stop/reconnect precedence, last-frame retention, bounded automatic retry, manual retry, and localized actionable states.                                                                                                                                                                                                                                  |
| `terminal-parity/LC-03-dual-client`, `terminal-parity/LC-09-client-isolation`                                    | Controlled [session-coordinator](src/main/client-session/__tests__/DesktopClientSessionCoordinator.unit.test.ts) and [projection-service](src/main/terminal-projection-control/__tests__/DesktopTerminalProjectionService.unit.test.ts) tests exercise independent sessions, canonical publication, scoped attachments, and disposal. These names have no current Electron-plus-authenticated-Browser headed tests; Browser Preview is a separate in-memory composition.                                                                                                                                            |
| `terminal-parity/LC-04-environment`                                                                              | [`src/main/composition/__tests__/resolve-desktop-runtime-configuration.unit.test.ts`](src/main/composition/__tests__/resolve-desktop-runtime-configuration.unit.test.ts) covers every platform branch and inherited-locale decision; the exact POSIX headed witness gives the supervised Host a conflicting ambient locale, then observes exported UTF-8 `LANG`, the effective character map, `TERM`, `COLORTERM`, and `TERM_PROGRAM` through a real Desktop → Host → PTY path. The cross-platform `terminal-parity/LC-04-color-policy` witness supplies explicit inherited color policy and verifies that all three values reach the PTY unchanged. These bounded witnesses do not claim a Windows locale shell or automatic remote environment forwarding. |
| `terminal-parity/LC-06-metadata-bridge`                                                                          | The exact headed OSC title/current-directory witness, Workbench metadata-resolution tests, Desktop client-session tests, and controlled implementation tests execute frame correlation, natural/custom title policy, background delivery, stale rejection, and fresh-session clearing.                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `terminal-parity/LC-12-feature-failures`                                                                         | Desktop Host/broker error tests, Workbench client-contract tests, and projected-renderer/surface tests execute exact operation/correlation identity, retryability, transport preservation, terminal-control sealing, and localized user consequence.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |

Focused traces cover:

- strict client-session and projection boundary validation, malformed-message retirement, exact proposal correlation, and page attachment disposal;
- atomic bootstrap, FIFO complete-document proposals, first-writer-wins races, stale rollback with Client View preservation, invalid-file preservation, and one ordered publication stream;
- first-commit and per-command write-ahead ordering, Main-owned restore-only recovery, failed-store effect suppression, and post-effect managed container cleanup;
- fresh provisioning, multi-binding store preservation, recovery, exited recovery-to-disposal, orphan and conflict handling, durable close, and presentation-registry reconstruction;
- unrelated terminal concurrency, exact close targeting, natural-exit Tab removal with native-window retention, Tab-close result ordering, and stale result rejection;
- two recovered Workbench tabs where both lifecycle records reconcile but only the active visible Tab has an attachment, followed by exact release/open behavior on Tab switch; a separate controlled trace drives 80 rapid first/second-Tab shortcuts, proves later input through fresh attachments, closes the active second Tab, and samples every presentation frame for request-local control survival and no empty top layer;
- exact `terminal-parity/SU-16-startup-geometry` real Electron/Host evidence: a controlled POSIX
  shell records its PTY size before handing control to `/bin/sh`, and that first-child geometry
  matches the independently derived mounted grid and the sole first client frame without an
  intermediate `80x24` frame; owner-local traces cover no-child deferred creation,
  start-before-attach ordering, duplicate/conflicting start, failed start, and pending disposal.
  The system witness is POSIX-only and does not claim Windows ConPTY startup or a later resize;
- exact `terminal-parity/SU-18-font-size-shortcuts` real Electron/Host evidence: the platform `Mod`
  increase, decrease, and reset chords update two mounted Panel terminals by the two-point product
  step, restore the default, and preserve both attachment identities. The witness exercises the
  current runner platform and does not claim excluded keypad or shifted-plus aliases;
- exact `terminal-parity/SU-13-cursor-defaults` real-PTY projection evidence for the explicit
  block/non-blinking profile default, a blinking-bar DECSCUSR override, and RIS restoration. Host
  runtime tests additionally cover application Reset and non-default profiles; the system witness
  observes canonical projected metadata rather than treating a platform screenshot as cursor
  authority;
- concurrent viewers with exactly one resize owner, last-meaningful-activity handoff, recency fallback, dynamic renderer ownership, and serialized release/open ordering;
- deferred-Host resize traces proving one active request per attachment, constant-space replacement
  of intermediate measurements, and immediate dispatch of the newest geometry after settlement;
- exact headed `terminal-parity/IN-10-read-only` capability evidence for Host-committed live
  downgrade/upgrade, immediate client repair, read-only local output interaction, no stale replay,
  and serialized mutation-before-detach ordering;
- exact headed `terminal-parity/IN-14-shared-focus` evidence for two projected attachments'
  independent focus claims, one shared-terminal aggregate focus interval, target-only visibility
  repair, and activation without claim replay. The fixture controls application-layer focus and
  does not claim operating-system multi-window focus;
- exact headed `terminal-parity/PR-11-bell`, `terminal-parity/IN-07-pre-input`,
  `terminal-parity/IN-09-action-scope`, `terminal-parity/IN-11-context`, and
  `terminal-parity/IN-12-actions` Canvas evidence for the shared bell coordinator, preservation of
  history and selection during reported buttonless application motion, Host-selection preservation
  when window focus loss collapses an explicit assistive-technology-origin accessibility Range,
  containing-Workbench Action scope,
  pinned/revalidated semantic context, synchronous availability, typed execution, and localized
  feedback. Captured effects do not claim native audio, notification, URL-handler, or clipboard
  delivery;
- exact `terminal-parity/IN-08-drop` protected file-marker admission, Desktop backing-path
  resolution, bounded Ghostty-compatible POSIX escaping, direct middle-of-line insertion, and no
  review or implicit Enter;
- exact headed `terminal-parity/FS-05-file-links`, `terminal-parity/FS-06-url-policy`, and
  `terminal-parity/FS-07-keyboard-hints` Browser-composition witnesses for disabled filesystem
  activation; the shared `http`/`https`/`mailto` detection, hover, context, and activation boundary;
  and visible URL/path/file-location/hash labels with exact insertion, captured URL opening,
  cancellation, and zero label-key terminal input;
- exact headed `terminal-parity/FS-19-local-file-links` and
  `terminal-parity/FS-20-image-preview` local-capability witnesses for relative-path resolution,
  modifier activation, 300 ms dwell, replacement cancellation, stale-completion rejection, and the
  production pointer-transparent preview card. They use a fake opener and synthetic image bytes and
  do not automate an operating-system application;
- exact headed `terminal-parity/FS-08-command-output-viewer` and
  `terminal-parity/FS-09-scrollback-viewer` Browser-composition witnesses for the production shared
  immutable terminal-text viewer, exact retained snapshots, visible-top anchoring, bounded search,
  source-specific Copy Actions, focus restoration, and unchanged attachment identity;
- exact headed `terminal-parity/FS-10-active-tab-movement` and
  `terminal-parity/FS-11-tab-title` Browser-composition witnesses for Kitty-aligned circular active-Tab
  movement inside the pinned partition, stable explicit tab-title targeting, pre-filled custom-title
  editing, dynamic-title preservation beneath the custom value, blank reset to the latest natural
  title, Action-search discovery, focus restoration, and unchanged terminal input identity;
- exact headed `terminal-parity/FS-12-panel-resize` Browser-composition witness for the Kitty-aligned
  resize-mode entry key and `W`/`N`/`T`/`S`/`R` vocabulary, Control-modified larger steps, nearest
  matching split-axis mutation, whole-Tab equal-weight reset, menu and Action-search discovery,
  stale-target closure, focus restoration, unchanged attachment identity during resize, and stable
  terminal/creation identity across Tab handoff;
- exact headed `terminal-parity/FS-13-directional-panel-movement` Browser-composition witness for
  Kitty-aligned directional neighbor selection and leaf exchange across a nested four-Panel layout,
  all four movement directions, outer-edge event fallthrough, moved-source focus restoration,
  invariant Panel/presentation/terminal identity, and Action-search plus Settings
  discovery;
- exact headed `terminal-parity/FS-14-panel-equalization` Browser-composition witness for Kitty's
  proportional same-axis Panel-unit semantics across three nested columns, menu and Action-search
  discovery, configurable shortcut presentation, available-versus-unavailable event prevention,
  stable Panel ordering, terminal identity, attachment identity, and input focus;
- exact headed `terminal-parity/FS-15-current-directory-tab` real-PTY/Electron witness for trusted
  OSC 7 current-directory delivery, terminal context-menu and Action-search discovery, configurable
  shortcut dispatch, exact `pwd` inheritance in the new Tab, and unchanged source terminal identity,
  session, focus restoration, and Settings projection;
- exact headed `terminal-parity/FS-16-visible-panel-selection` Browser-composition witness for
  two-Panel direct focus and swap, three-Panel source-excluding labels, invalid-key containment,
  modifier-held physical selection, primary-pointer swap, Action-search and Settings discovery,
  source/target focus restoration, canonical leaf exchange, and unchanged Panel, terminal
  presentation, creation, and Host terminal identities;
- exact headed `terminal-parity/FS-17-panel-position-focus` Browser-composition witness for
  canonical depth-first Panel numbering, physical `Ctrl+Shift+1/2/3/0` dispatch, current-target
  consumption, missing-position and maximized-layout fallthrough, focused-textbox restoration,
  stable Panel/presentation/document identities, and Settings discovery of positions one and ten;
- exact headed `terminal-parity/FS-18-panel-broadcast-input` Browser-composition witness for one
  immutable three-Panel target set, exactly-once committed IME text, paste exclusion, serialized
  FIFO text, unmodified Escape delivery, visibility and end controls, visible target shields, and
  non-atomic partial delivery that stops without retry or replay;
- exact headed `terminal-parity/FS-19-workbench-region-focus` real-PTY/Electron witness for forward
  and reverse modified-key cycling across the active terminal, Tab Strip, and visible sidebar; exact
  keyboard Tab-menu entry and return focus; stable presentation and attachment identities; and live
  PTY input after the round trip;
- exact headed `terminal-parity/LC-05-lan-capability` Browser-composition evidence for per-operation secure-context, transient-user-activation, and API checks; synchronous Copy availability; one admitted writer call; rejection without retry; localized failure; and no privileged insecure-LAN path;
- exact `terminal-parity/HS-07-selection-export` real-PTY/Electron evidence that styled terminal
  selection reaches the platform clipboard as the exact visible plain text, paired with protocol,
  Host, and projected-command regressions that expose no HTML or raw VT format;
- exact `terminal-parity/IN-03-image-paste` real-PTY/Electron evidence that an idle default paste
  chord preserves the native image-only event and publishes exactly one `Control+V` byte to the
  child process without a clipboard-text Action or failure banner;
- exact `terminal-parity/IN-13-selection-copy-paste` real-PTY/Electron evidence that an active
  selection followed by immediate Copy then Paste waits for the Main-owned copy commit, reads the
  newly selected text instead of a deliberately installed stale clipboard sentinel, clears the
  selection through the shared paste path, and appends the copied token to live terminal input
  without manually restoring focus;
- exact `terminal-parity/IN-18-native-selection-paste` real-PTY/Electron evidence that an active
  ordinary pointer selection leaves the hidden textarea focused and creates no non-collapsed
  terminal DOM Range, then an idle default native Paste clears the Host selection and appends the
  preexisting clipboard payload without a manual Copy or focus repair;
- fixed-header frame routing to only the matching AttachmentId, with exact-size structured-clone payload isolation;
- complete theme bootstrap, identity-scoped apply/reconcile forwarding, lifecycle refresh,
  unknown-outcome preservation, and exact headed `terminal-parity/SU-09-theme` hold/release
  evidence that exposes no mixed app/terminal revision;
- exact headed `terminal-parity/CF-03-visual-environment` evidence with repository product fonts,
  Canvas 2D forced through the production renderer boundary, a `900x500` viewport, DPR `1`,
  `en-US`, UTC, light color scheme, and reduced motion. It asserts exact semantic state and performs
  no pixel-tolerance normalization; it is not a WebGPU, cross-browser, native-DPI, or
  operating-system font-rasterization golden;
- sandboxed end-to-end input, output, history, search, selection, copy, links, exit, renderer reload, and Electron Main restart;
- development and packaged Shell-resource resolution, resource presence and privacy checks, real Zsh
  OSC 133 output, and one macOS Electron path from manual Settings policy through a long-command
  completion reminder without replacing the projected attachment;
- exact `terminal-parity/SI-02-prompt-navigation` real-PTY/Electron evidence that three explicit
  OSC 133 prompt rows reach the production Host projection and that repeated `Ctrl+Shift+Z`, then
  `Ctrl+Shift+X`, select the nearest rows in both directions without replacing the attachment. The
  witness does not claim automatic Shell Integration injection;
- exact `terminal-parity/SI-03-automatic-shell-integration` real-PTY/Electron evidence that a fresh
  local Desktop Zsh preserves user `.zshenv`, `.zprofile`, `.zshrc`, and `ZDOTDIR` state, clears its
  one-shot marker, and emits a command-completion fact without manual setup or attachment
  replacement;
- exact `terminal-parity/HS-11-keyboard-scrollback` real-PTY/Electron evidence that
  `Ctrl+Shift+Arrow`, `Ctrl+Shift+PageUp/PageDown`, and `Ctrl+Shift+Home/End` move one production
  retained-history viewport by line, overlapping page, top, and bottom without replacing the
  attachment. The witness does not claim Kitty's secondary aliases, alternate-screen behavior, or
  keyboard smooth-animation timing;
- exact `terminal-parity/HS-12-sustained-output-interaction` real-PTY/Electron evidence that large
  and small wheel deltas enter and continue moving retained history during 2-millisecond in-place
  status redraws, fine-wheel progress and raw-TTY input acknowledgement each remain within 150
  milliseconds, a captured drag survives one matching buttonless intermediate move until explicit
  release, the selected word reaches the platform clipboard, terminal revision continues advancing,
  and no history failure is published;
- controlled independent ClientSession and attachment traces covering canonical publication, disconnect, rejection, and scoped disposal; Browser Preview remains separate from the Desktop runtime;
- user-quit preserve/cancel/stop, automatic preserve for idle running terminals, exact reuse of Panel-close protection, hidden-terminal stop, failed-CAS suppression, partial-stop retry, unreferenced-binding drain, unknown-Host-orphan refusal, update-restart preserve, development Host stop/stage/ready ordering, and fresh identity after explicitly starting a stopped session.

[Lifecycle adapter tests](src/main/terminal-document-control/__tests__/DesktopTerminalLifecycleAdapter.unit.test.ts)
cover exact retry identity, restore-only admission, and provision/close serialization.
[Projection-service tests](src/main/terminal-projection-control/__tests__/DesktopTerminalProjectionService.unit.test.ts)
cover measured start-before-attach, attachment suppression after start failure, and concurrent-open serialization.

Desktop retains formal cores for the serialized shared-document CAS and projected-terminal IPC. The document-store core preserves the same two-client, revision-through-2 transition system and properties while expressing commit/revision checks as pre/post action properties instead of retaining decision-history variables. This removes redundant history states; it does not remove a commit outcome. Lifecycle, binding recovery, write-ahead, resize leases and folder/worktree workflows use the deterministic and generated implementation tests named in their owning references. Those tests cover declared traces, not an exhaustive Cartesian product of the Desktop workflows.

The projection-service suite also controls concurrent resize claims, activity coalescing, read-only
downgrades, detach fallback, and release/open ordering. [Broker tests](src/main/broker/__tests__/ProjectedTerminalBroker.unit.test.ts)
check one active resize plus the newest pending measurement, typed failure settlement, and dropping
pending geometry at cleanup. These are implementation traces, not a retained resize-workflow model.

[Archive capture tests](src/main/terminal-archive/__tests__/TerminalArchiveCaptureService.unit.test.ts)
cover final-history capture, byte limits, deadlines, and late-lease release.
[Archive store tests](src/main/terminal-archive/__tests__/terminal-archive.unit.test.ts) cover corruption,
eviction, and orphan reconciliation; [archive projection tests](src/main/terminal-archive/__tests__/ArchivedTerminalProjectionService.unit.test.ts)
cover read-only history/search/copy and missing-archive fallback. [Workbench stop tests](../../packages/workbench/src/workbench/model/__tests__/terminal-session-stop.unit.test.ts)
cover retained product structure, hydration without implicit restart, and fresh Presentation identity.
These finite implementation cases do not establish exhaustive archive or application-stop interleavings.

## Development

The packaged observation Capsule includes its README files in its immutable payload digest. Their references to retired workflow models describe that packaged revision; current observation evidence is maintained in [Agent observation ingress](docs/agent-observation-ingress.md). Updating the verification portfolio does not republish a Capsule. Desktop retains only the document-store and projected-terminal IPC TLA protocols; local workflow correctness uses the owning implementation suites.

Windows unit verification uses at most four Vitest workers for process-heavy fixtures. POSIX socket, login-shell, permission-mode, and `/bin/sh` syntax witnesses run on POSIX hosts. Portable subprocess and ACK protocol checks also run on Windows, with separate evidence for disabled Desktop observation ingress. Managed Desktop Agent integration remains POSIX-only.

On Linux, Desktop's required shell-integration tests need Bash at `/bin/bash` and Zsh at `/bin/zsh`.
Install both before running the unit suite or root merge gate; Linux CI provisions Zsh explicitly.

The Rsbuild configuration can be inspected without workspace build artifacts or an installed Electron executable. Development dependencies and Electron resolution are loaded inside the development startup hook.

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
Deterministic runtime tests own command order, adapter failures, aborted preparation,
update coalescing, workspace cleanup, and captured-binary immutability. Forced operating-system
termination, compiler correctness, and PTY migration across a Host process remain outside these
claims.

The development Host build watcher closes its file watcher, terminates the supervised build, and drains pending observation work even when an earlier step fails. It reports the first original failure; [build-watch cleanup tests](src/development/__tests__/NodeHostBuildWatch.unit.test.ts) cover synchronous watcher failures and later asynchronous termination failures.

The development runtime removes inherited `PAGER` and `GIT_PAGER` overrides only from the supervised
Terminal Host environment. Interactive shells therefore select their configured or default pager,
while Electron, build processes, packaged launches, and the invoking developer environment remain
unchanged.

On POSIX, the development supervisor creates one random owner-only workspace directly beneath
`/tmp`. This controlled path keeps the complete endpoint beneath Darwin's Unix-socket pathname bound;
the in-process native adapter still validates its no-follow directory descriptor, owner, mode, ACL,
endpoint, and actual peer credentials. Packaged product rendezvous remains beneath the trusted
per-user runtime location described above and never uses this development workspace.

Start the canonical Web development server and Desktop together:

```sh
pnpm desktop
```

`pnpm desktop` builds shared dependencies and Web resources, then runs the Web development server and Desktop-owned native/Electron development lifecycle. Electron defaults to `http://127.0.0.1:3000/`; `ORCHBEE_DESKTOP_RENDERER_URL` can select an explicit loopback Web server.

On macOS, the command materializes and ad-hoc signs a content-addressed `Orchbee Dev.app` beneath
`apps/desktop/out/development-runtime/`, then launches its exact executable. Its application name,
checkout-scoped bundle identifier, Electron default icon, and checkout-scoped data root are
separate from the installed release application. Development diagnostics must target that exact
bundle path or identifier and never fall back to `/Applications/Orchbee.app`.

### Distribution profiles

Runtime identity and descriptor parsing belong to `src/distribution/desktop-distribution-profile.ts`; packaging-only argument parsing, descriptor encoding and icon construction live in `src/distribution/tooling/`. Both use the same application-name validation. These source-executed commands require Node native type stripping (enabled by default in Node 22.18 and newer). Node-executed scripts import source modules through explicit `.ts` paths under the [TypeScript module contract](../../docs/TYPESCRIPT.md#module-syntax); the no-emit Desktop typecheck enables `allowImportingTsExtensions`, and the distribution tests include a real Node import witness. `@electron/packager` and `node-api-headers` are development dependencies.

Desktop owns exactly three explicit distribution profiles:

| Command                        | Application         | Bundle identifier                          | Data root                       | Icon             | Output                      |
| ------------------------------ | ------------------- | ------------------------------------------ | ------------------------------- | ---------------- | --------------------------- |
| `pnpm desktop:package:dev`     | `Orchbee Dev.app`   | `com.electron.orchbee.dev.d<scope-digest>` | `~/.orchbee-dev-<scope-digest>` | Electron default | `apps/desktop/out/dev/`     |
| `pnpm desktop:package:local`   | `Orchbee Local.app` | `com.electron.orchbee.local`               | `~/.orchbee-local`              | Electron default | `apps/desktop/out/local/`   |
| `pnpm desktop:package:release` | `Orchbee.app`       | `com.electron.orchbee`                     | `~/.orchbee`                    | Electron default | `apps/desktop/out/release/` |

`ORCHBEE_DESKTOP_DEVELOPMENT_APPLICATION_NAME` optionally overrides the default `Orchbee Dev` name
for `pnpm desktop` and `pnpm desktop:package:dev`.
`ORCHBEE_DESKTOP_LOCAL_APPLICATION_NAME` independently overrides the default `Orchbee Local` name
for `pnpm desktop:package:local`. Each value must be 1–64 characters, have no leading or trailing
whitespace, and contain no path separators, control characters, or platform-invalid filename
characters. The selected name is persisted in the corresponding distribution descriptor so the
packaged application keeps it without the shell variable. Neither override changes its profile's
bundle identifier or data root, and the `release` profile ignores both variables.

```sh
ORCHBEE_DESKTOP_DEVELOPMENT_APPLICATION_NAME="Orchbee Dev Canary" \
pnpm desktop:package:dev

ORCHBEE_DESKTOP_LOCAL_APPLICATION_NAME="Terminal" \
pnpm desktop:package:local
```

`pnpm desktop` uses the `dev` identity without creating a packaged application. Packaging requires an explicit dev/local/release profile. Each command builds Terminal Host, the current-platform endpoint adapter, Web resources, and Main/preload bundles. Host and native endpoint resources remain outside `app.asar`; shell integration and Agent observation adapters retain their existing ownership. Packaging is independent of the current native CLI/TUI feature-development workflow.

All three package commands write beneath their profile-specific directory in `apps/desktop/out/`.
They do not perform Electron code signing, notarization, installer creation, publication, or update
distribution.

`ORCHBEE_TERMINAL_HOST_BINARY` selects an absolute development/test Host binary. `ORCHBEE_TERMINAL_SCROLLBACK_BYTES` selects the startup scrollback budget in decimal bytes from `0` through `1000000000`, defaulting to `10000000`. `ORCHBEE_DESKTOP_RUNTIME_DIRECTORY`, `ORCHBEE_DESKTOP_ENDPOINT_ID`, and `ORCHBEE_DESKTOP_RECOVERY_DIRECTORY` isolate controlled development and system-test state. The POSIX runtime retains its product-scoped trusted per-user location and native endpoint-security validation.

`pnpm desktop` internally sets `ORCHBEE_TERMINAL_HOST_SUPERVISED=1` so Main retries only the Host process owned by the development runtime, and supplies an owner-only `ORCHBEE_DESKTOP_DEVELOPMENT_RESTART_PLAN` path for the generation readiness handshake. These two variables are private development composition controls, not supported product configuration or a production update mechanism.

Run package checks from the repository root:

```sh
pnpm --filter @orchbee/desktop typecheck
pnpm --filter @orchbee/desktop test
pnpm --filter @orchbee/web build
pnpm --filter @orchbee/desktop build
pnpm --filter @orchbee/desktop test:system
```

### Manual native-window test

`test:system` excludes the `@manual-native-window` test from every aggregate, automated, CI, and
agent-initiated run. That test changes operating-system focus while verifying minimize, activation,
maximize, full-screen, and restart behavior. Run it only after a human explicitly approves that
exact test:

```sh
ORCHBEE_DESKTOP_NATIVE_WINDOW_TEST_APPROVAL=I_ACKNOWLEDGE_FOCUS_INTERRUPTION \
  pnpm --filter @orchbee/desktop test:system:manual:native-window
```

An AI agent must not invoke the command or set the approval variable unless the current human
request explicitly authorizes this exact focus-stealing test. No aggregate or full-suite command
may invoke the manual script.

The optional throughput harness requires an absolute large-output fixture and at least five runs:

```sh
pnpm --filter @orchbee/desktop benchmark -- \
  --file /absolute/path/to/large-output.txt \
  --runs 5
```

Keep `pnpm web` running while using the benchmark, or set `ORCHBEE_DESKTOP_RENDERER_URL` to another loopback Web development server.

Interaction targets stay scoped to the same terminal presentation. The harness positions the
pointer from the stable presentation-root geometry and observes wheel start there, so transient
history snapshot canvases and canvas transforms are measured behavior rather than actionability
prerequisites. Its output command keeps the shell open behind a rendered completion marker until
history, search, and copy measurements finish; process exit therefore cannot replace the live
terminal with a stopped presentation during those interactions. After the terminal-exit trace is
captured, the harness terminates its isolated temporary Electron instance without exercising the
product's interactive user-quit workflow. The report summarizes Browser, renderer (`Tab`), GPU,
and aggregate Electron working sets from `app.getAppMetrics()` in KiB. Those process figures
support same-machine comparisons; they do not account for every driver-owned GPU allocation or
define whole-system RSS.

It can also collect controlled Ghostty and Host-only references. Results are diagnostic evidence; they are not a merge or slice-completion gate in the current development phase.
