# @orchbee/web

Browser composition entry for the Orchbee Workbench.

## Composition boundary

`apps/web` is the single runnable product UI for both a normal browser page and Electron's renderer. Environment adapters may provide different transport and privileged capabilities, but they compose the same Workbench, Workspace, Tab, Panel, and projected-terminal application. Desktop must not maintain a parallel renderer UI. The shared Workbench client runtime owns shared-document coordination, projected-terminal presentation, search, actions, and its narrow client-session and projection ports. It has no Electron, `MessagePort`, network, or application-lifetime dependency. The runtime provides each projected renderer session with one stable `TerminalPresentationConfigurationSource`. Its initial complete value opens the surface, and later complete values update that same session and attachment without changing React capability identity or reopening projection. The default source retains unbalanced padding, fixes `fontSize` at 13 to match Ghostty's default 13-point setting, and replaces the source-neutral package face with the bundled Ghostty-aligned JetBrains Mono 2.304 variable faces plus Symbols Nerd Font 3.4.0 fallback. Application startup registers those immutable assets before opening any surface; the same bundle and license links serve Browser, Gateway, and Electron renderers without depending on machine-installed fonts. Each application composition replaces that fallback with one client-local settings controller that restores a strict complete preference record, exposes the same stable presentation source, and publishes complete runtime updates without reopening projection.

The Browser Preview uses its in-memory composition. Desktop supplies `MessagePort` adapters for the shared client-session and projection ports, then layers its Electron-only application-lifetime, Browser-access, native Project Folder-acquisition, managed Project Worktree, device-local terminal-launch-profile, terminal-default-settings, and Main-owned clipboard-read capabilities beside the common application. Its capability-scoped local projection transport declares `local-host`, allowing OBPB format-`0.0.1` render buffers to become direct typed views after structural validation. The Gateway adapter uses one authenticated control WebSocket per page and one binary WebSocket per visible terminal attachment and declares `external`, preserving the full per-cell protocol scan. Both adapters compose the same `createWorkbenchClientRuntime`; neither implements another Workbench renderer. The shared projected-terminal surface reserves one transparent pixel at each inline edge while keeping its block edges flush, fits complete cells inside that boundary after every container resize, and mirrors the effective terminal default background across the complete Panel viewport so the canvas never exposes a mismatched application surface; Host mode uses the Host-projected default, while a client display override uses its selected default color and alpha. Background opacity applies to that default background and renderer clear, never to whole-root CSS opacity or terminal text. Supported direct Kitty graphics use the same strictly decoded live frame and a clipped, pointer-transparent sibling canvas; history presentation and empty graphics snapshots remove that overlay instead of replaying stale images. An admitted terminal bell appears as a brief pointer-transparent information wash across the surface instead of a focus-like edge; reduced-motion presentation keeps the same bounded duration without animation. Repeated accepted sequences restart the bounded visual indication without altering layout or focus; sound and notification policies remain independent. A connection notice visibly overlays the surface without consuming terminal-grid space or moving the final row outside the viewport. A committed or duplicate frame clears only a transient connection failure; a published terminal exit or runtime failure outranks later transport loss and retains its final-frame presentation state. Generic Workbench client payloads come from `@orchbee/workbench-client-protocol`; only Gateway authentication/framing comes from `@orchbee/gateway-protocol`, and only Electron bootstrap, application-lifetime, Browser-access, Project Folder-acquisition, Project Worktree, terminal-launch-profile, terminal-default-settings, and clipboard-read commands come from `@orchbee/desktop-client-protocol`.

For a Host-backed visual terminal, the shared surface does not open projection until it has loaded
the current configured fonts and measured a positive mounted viewport. It sends the resulting exact
columns, rows, physical cell size, padding-aware grid, and document DPR through the same Electron or
Gateway projection command, then pins that presentation configuration for the first renderer. A
resize observation or animation frame wakes an unmeasurable surface; release aborts that wait. No
timer starts the child and no 80x24 visual fallback exists.

The Desktop `MessagePort` projection adapter keeps each locally released request correlated until
Main acknowledges cleanup. If asynchronous Main open completion crosses that release, the adapter
closes only the late attachment port and keeps the shared projection control available; unrelated
pending and future projections continue normally. Invalid schemas, identities, port cardinality, or
duplicate opens still fail closed at the shared control boundary.

A painted surface entering Workbench's managed `closing`, `close-failed`, or `closed` lifecycle is
retained as a frozen, non-interactive frame instead of disposing its DOM. Entering that lifecycle
releases the projection request, attachment, transport, input, and resize ownership without opening
or retrying another attachment. The frozen renderer is disposed only when Workbench removes the
surface after a surviving Tab crosses its display barrier, or clears the final Tab normally.

A natural exit keeps its final frame readable only while Workbench's managed natural-exit removal
awaits disposal; the projected attachment rejects further PTY input, resize, clear, and reset
control, and the exited presentation is not retained as a Tab after disposal. Stopped presentations
remain a separate Workbench state. Its
Start New Session control returns the Workbench transition result through the shared composition;
an accepted result replaces the presentation through the normal durable workflow, while a rejected
or thrown start remains on the stopped presentation and displays localized failure feedback.

An unexpected attachment loss freezes the last completed canvas and accessible rows while releasing
its transport and interaction ownership. The shared surface retries a fresh attachment after 250,
500, and 1,000 milliseconds, and replaces the frozen projection only after the candidate paints its
first frame. Exhausting that budget leaves the honest last frame visible and exposes one localized
manual Retry action; each attempt owns and releases exactly one projection request. A typed
`renderer-backpressure`, `host-transport-backpressure`, or `host-connection-unavailable` failure settles the same attachment-local
transport-unavailable signal after preserving its original operation and correlation diagnostics;
the surface never retries the rejected command or keeps using the causally incomplete attachment.
Transport loss also races font and GPU startup explicitly: it aborts that candidate before startup
can stall recovery, and promotion checks the candidate's current transport availability both before
and after its first-frame wait.

Active-Tab handoff uses the same frozen-session discipline without keeping inactive terminal DOM or
attachments mounted. When a surface becomes retained, it captures only its exact last-painted
semantic presentation, freezes the canvas, and releases transport, input, resize, request, and
attachment ownership. The application-scoped `TerminalPresentationSnapshotCache` retains those
display-only snapshots in a byte-accounted `32 MiB` least-recently-used budget and clears them when
the shared client runtime is disposed. An incoming surface may paint a cached snapshot before its
fresh live attachment produces a frame. That cached draw satisfies only Workbench's display-ready
barrier; input, pointer interaction, and `autoFocus` remain disabled until the candidate paints its
first live frame and satisfies activation-ready. On a miss or oversized snapshot, the outgoing
frozen layer remains visible until live, stopped, or honest failure content is ready. Returning to a
still-frozen surface reuses its canvas for the display barrier while attaching a fresh candidate
behind it. Superseded or inactive sessions cannot acknowledge frames or publish metadata, and each
surface still releases its per-terminal Canvas, atlas, buffers, and renderer after handoff.

An attachment feature failure remains distinct from transport loss and terminal outcome. The shared
surface retains its operation, code, retryability, Host correlation ID or client request sequence as
bounded diagnostics and applies the matching Terminal consequence. Background input acknowledgement,
history, and resize failures never create a global terminal-content notice: rejected input remains in
the bounded diagnostic and required transport-recovery paths, history feedback belongs to the find
surface, and resize remains in its bounded retry and diagnostic path. Connection, runtime, and
explicit user-action failures retain localized visible feedback. The application does not
automatically replay terminal input, control, clipboard, or navigation effects.

The shared adapter exposes the last completed draw checkpoint as
`data-projected-terminal-rendered-generation` and
`data-projected-terminal-rendered-revision`. Both values come from one renderer telemetry event and
identify the packed Host commit actually drawn after any animation-frame coalescing. This diagnostic
contract is identical in Electron and Gateway Browser clients; it is not a second acknowledgement
to Terminal Host. Runtime presentation diagnostics additionally expose request, committed, and
drawn epochs on the same surface. They distinguish independently ordered font preparation from the
single local commit and completed current repaint without creating another attachment lifecycle.

Mounting a projected renderer never moves DOM focus. Workbench retains the active product control;
pointer activation, an explicit terminal focus request, and find-surface close are the only adapter
paths that move focus into the hidden terminal input. The projected root contains both that input and
its find surface, so moving between them does not publish a terminal focus transition.
Each attachment publishes only its own focus claim and projection demand. Terminal Host combines
eligible claims for a shared terminal, so one attachment losing focus or visibility cannot emit a
terminal-wide `FocusOut` while another eligible claim remains; reactivation requires a fresh claim.
Opening that compact find surface seeds an empty or retained query from the bounded preview of an
active Host selection when one is available; search-owned active ranges remain authoritative Host
selections rather than a second application selection model. Opening or explicitly reopening Find
focuses and selects the complete query, while Escape from any find control closes the surface and
restores terminal focus. Match options live inside the query field, ordinary search progress uses an
inline localized count, Left and Right Arrow move between option toggles, and pointer option changes
return focus to the query. Only actionable failures, timeouts, or retained-history notices expand the
toolbar with complete localized feedback.

After a projected frame commits, the renderer forwards its complete nullable title,
current-directory metadata, and transient `idle`/`active`/`unknown` close activity with the exact
`TerminalPresentationId`. The shared client runtime
normalizes that observation with the trusted terminal-machine hostname from the client-session
bootstrap and applies it to the matching Workbench presentation. Natural titles and accepted
current CWD remain transient; durable fallback/custom Tab-title policy and shared-document
projection remain Workbench-owned. Electron and Gateway Browser clients therefore use the same
metadata policy, and the page never derives terminal trust from its own URL hostname.

Suspicious paste text opens one localized review dialog owned by the shared projected-terminal
surface. It presents a bounded, control-visible preview of the retained original text and identifies
multiline input, controls that will be replaced, and an embedded bracketed-paste terminator. Cancel
publishes nothing; approval sends the sanitized text together with the mode that was reviewed, while
Terminal Host rejects a mode change before encoding or queue admission.

Each projected surface registers the Workbench-owned minimal terminal Action catalog into the
viewport's application runtime and scope: Copy, Paste, Select All, Find, Find Next, Find Previous,
Scroll to Bottom, Clear, and Reset. Availability is synchronous and remains authoritative when a
prepared invocation executes. Context-menu and custom-shortcut Paste read text only through an
injected client capability. An idle platform-default paste chord preserves native DOM delivery so
the terminal adapter can apply its plain-text-first and image-only `Control+V` policy without
reading image bytes. When a manual selection Copy or another Paste Action is still in flight, the
default chord uses the Action path; Paste waits for the exact Copy operation to settle before it
reads the current clipboard, then reuses the same terminal paste sanitation and review path. Every
execution settles with a typed
Action result; completed, unchanged, blocked, and failed outcomes that need perceivable feedback
use the shared localized terminal status surface. These immediate terminal Actions add no
confirmation step. An unavailable or unbound Apple Command shortcut remains native instead of
being suppressed.

The stable headed Canvas 2D witnesses `terminal-parity/IN-07-pre-input`,
`terminal-parity/IN-09-action-scope`, `terminal-parity/IN-11-context`, and
`terminal-parity/IN-12-actions` compose the production projected surface with its real containing
Workbench runtime. They assert that buttonless application motion preserves retained history and
Host selection while remaining reported, plus claimed/unclaimed and available/unavailable shortcut
handling, application-mouse ownership, exact pinned context and stale-link rejection, the complete
minimal catalog, synchronous availability, typed outcomes, no new confirmation, and both `en-US`
and `zh-CN` feedback. Their clipboard and external-open effects are captured; they do not claim an
operating-system menu, clipboard, or URL handler.

The projected attachment stream can replace input authority live only after its environment owner
commits an exact next capability revision in Terminal Host. The shared renderer immediately makes
the existing textarea read-only, invalidates composition, native settlement, paste review, held
key/mouse/focus state, and recalculates terminal Action availability from the live terminal. Find,
history, selection, copy, retained output, and approved links remain available. A later upgrade
admits only future input carrying the new revision; it does not replay pre-downgrade DOM or queued
work. Electron and Gateway use the same renderer contract, and no renderer command can request its
own upgrade.

The stable headed Canvas 2D `terminal-parity/IN-10-read-only` witness controls exact attachment
capability revisions and observes immediate local repair, retained search/selection/history/copy,
zero stale input commands, and only fresh upgraded input. Terminal Host's owner-local tests prove
the authoritative rejection and repair ordering; this Browser fixture does not grant a renderer
capability mutation or claim native permission behavior.

Desktop file drop is an optional capability of that same surface, not a browser filesystem API.
The Desktop adapter may resolve only user-supplied disk `File` objects to backing paths; one object
may represent a file or directory, but no child is enumerated. File drag recognition uses the
`Files` marker or file-kind items while the drag data store is protected, then the ordered `FileList`
or non-null file-kind items at `drop`. The deterministic preparer accepts at most 32 absolute POSIX
paths, preserves their order, applies the pinned Ghostty live-buffer backslash escaping set, and
joins them with one space. It rejects the complete drop when any path is missing, relative, contains
a C0/C1 control character, or would make the escaped input exceed 4,096 Unicode code points. A
ready drop enters the existing FIFO paste path directly, so a simple path remains unchanged and a
shell receives the text at its current editing cursor without Paste Review or an Enter key. The
local path may not exist in SSH, containers, or another foreground environment; direct insertion
deliberately adds no warning or remote-path translation. Replacement, lifecycle abort, stale
preparation, and an unavailable active attachment publish nothing. Browser and Gateway clients
receive no backing-path capability, and Windows remains unsupported until the active shell dialect
has an equally bounded escaping contract.

The stable headed `terminal-parity/IN-08-drop` witness uses a real Electron `File` and backing-path
bridge on the current POSIX Desktop platform. Its protected `dragover` fixture exposes a `Files`
marker without a readable `FileList`; the final `drop` supplies the disk-backed object. It observes
direct insertion into the middle of an existing shell line, no Paste Review, no automatic
execution, and execution only after a separate explicit Enter. Owner-local tests cover FileList
order and file-kind item fallback, Ghostty-compatible escaping, the 32-file and 4,096-code-point
bounds, Unicode, every control range, missing or malformed bridge values, unsupported platforms,
stale preparation, and Gateway capability absence. The witness does not automate Finder or claim
every native operating-system payload variant, Windows support, Browser/LAN path access, directory
enumeration, file-content reads, uploads, or filesystem-link activation.

The packaged `orchbee:` origin and Electron User-Agent token select Desktop mode. The User-Agent is adapter discovery for loopback development, never authentication; Desktop authority still arrives only through the validated sandboxed bootstrap. The explicit `?orchbee-runtime=gateway` marker selects an authenticated process-lifetime Gateway client on loopback or an explicitly enabled trusted LAN. An ordinary HTTP(S) browser origin selects Browser Preview. Gateway port, version, and lifetime capability must appear exactly once in the fragment. Invalid, duplicate, or incomplete values fail closed without falling back.

## Navigation

All three compositions use one TanStack Browser Router and the same path contract. Project, Workspace, and active Tab identity appear in the address. Structurally incomplete routes such as `/projects` or `/projects/$projectId/workspaces/$workspaceId` replace themselves with the first ordered available descendants; explicit stale identifiers retain the address and display the localized unavailable state. In-app navigation pushes history, while canonicalization and reconciliation replace it, so Back and Forward reproduce the displayed Client View without publishing shared-document changes. See [`docs/navigation-routing.md`](docs/navigation-routing.md).

## Settings

The common Settings action opens an application-owned full-window overlay rather than navigating to another route. The Workbench subtree becomes inert, but its React tree and every projected terminal attachment remain mounted. Client-local application theme, terminal font, effective default terminal colors, and default-background opacity apply immediately and persist as one strict versioned record for the current browser origin. On macOS Desktop, a separate strict local record selects whether neither, left, right, or both Option keys act as terminal Alt; the default is off and accepted changes update every mounted projected terminal without replacing its attachment. Eighteen frequent Workbench and focused-terminal shortcuts, including four directional Panel-focus commands, use a separate strict local override record; accepted changes update the mounted Workbench and every projected terminal without replacing an attachment or connection. Standard defaults reserve unshifted terminal control sequences, while runtime Action availability remains authoritative before any key is consumed. Application theme remains separate from terminal display theme, and terminal overrides never mutate Host metadata, palette, cursor, VT, or OSC state. Local Desktop additionally injects a semantic shell-profile control. Its explicit editor can create, rename, select a system or custom executable, preserve literal argument order, select the default, delete, and confirm reset; every accepted change affects only terminals created after the Main-owned catalog commits. Gateway and Browser compositions receive no profile capability. Shell integration is manual-only: Settings shows exact shipped Zsh, Bash, and Fish source commands, never edits startup files, and owns a separate default-off device-local completion-reminder policy whose notifications never include command text. A paste-only Ghostty migration dry-run reports lossless current targets, manual-review cases, ignored keys, and invalid lines without reading files, applying values, or persisting source text. The Remote development category is another read-only projection: it explains the `xterm-256color` baseline, exposes one constant shell diagnostic, distinguishes observed environment and terminfo outcomes, and states that Orchbee does not silently forward extra environment variables or modify the remote host. Opening, searching, or switching to this category starts no process, writes no terminal input, and persists no remote data. Desktop also injects a semantic Browser-access entry without exposing Electron ports or controllers to the generic Settings module. See [`docs/terminal-settings.md`](docs/terminal-settings.md) and [`docs/keyboard-shortcuts.md`](docs/keyboard-shortcuts.md).

The exact headed `terminal-parity/SU-09-theme` witness holds a prepared Host result and its
revision-tagged frame before one release, then observes the terminal defaults, palette, cursor,
overlays, pixels, and application surface publish one matching revision. It uses the controlled
runtime-presentation Canvas fixture and does not claim a persisted settings migration, a native
operating-system theme event, or a renderer-owned terminal palette.

## Local development preview

`pnpm web` renders the Workbench shell with its localized sidebar, standalone terminal context, Project and Workspace navigation, and per-tab Panel workspace. The browser composition creates one `ActionRuntime` and one in-memory `WorkbenchProjectWorkspaceRuntime`. It starts with only the standalone Terminal context. A person can create an in-memory Project by entering a folder path; the Project add control first preserves the common creation menu, then `Select Folder…` opens the browser-only path entry in place of a native picker. The browser uses that path as the initial Project Folder and terminal CWD without claiming that the path exists. `New Folder…` remains visible but disabled because the composition performs no filesystem, Git, PTY, or durable-storage effect.

The composition also owns locale and shortcut-platform setup. Electron Desktop injects its native existing-Folder acquisition and managed-Worktree capabilities into this same application; `Select Folder…` therefore opens the operating-system picker, the Project manager's `Add Folder` uses the same capability without rendering a manual path field, and `New Worktree…` opens the fixed-location Git form. Ordinary Browser Preview and authenticated Gateway pages receive neither native capability and retain manual Folder path entry; New Worktree is absent there. New Folder remains unavailable until an environment adapter owns that effect. An ordinary HTTP(S) URL without the Gateway marker remains the in-memory Preview and does not connect to Desktop Main. A capability-bearing URL opened or copied by Desktop Main creates a fresh page session and Client View over the same canonical Workbench document and terminal presentations as Electron and every other authenticated page. Closing that page releases only its attachments; reopening the URL creates a fresh session and full projection snapshots.

Gateway selection copy obtains bounded UTF-8 selection bytes from Terminal Host and may commit them with the Browser Clipboard API. Every write dynamically resolves the current API and requires both `window.isSecureContext` and active transient `navigator.userActivation`; Copy Action availability consumes the same live check, and the final effect repeats it after the Host round trip. A missing API, expired activation, insecure context, capability-probe failure, or rejected write fails closed with one bounded localized surface result. The Browser path performs no retry, `execCommand`, server relay, hidden downgrade, clipboard read, or image-byte read. The current insecure-LAN composition installs no manual-copy presentation, so it exposes no privileged copy path and keeps Copy unavailable; a future reviewed manual-copy presentation must remain an explicit non-Clipboard interaction. Automatic and manual admitted copy attempts share one localized surface-level completion or failure announcement after the owning clipboard effect resolves; copy feedback does not depend on the find surface being open. Gateway links use a local Browser effect restricted to `http`, `https`, and `mailto`; automatic detection, projected hover/context, Workbench Action availability, and this final effect all consume the terminal package's one normalization policy. The page receives no Electron Main effect capability. Electron alone injects a correlated, terminal-input-bounded plain-text reader over its ninth bootstrap port; it continues to use its Main-owned clipboard and independently revalidates the same narrow schemes at its privileged boundary.

An OSC 52 write never shares the selection-copy fast path. The shared renderer automatically processes
each exact bounded UTF-8 request without presenting a confirmation surface. Its decision repeats the
exact attachment, Host sequence, and text; Desktop Main performs the privileged Electron effect,
while a Gateway Browser reports success only after its own local Clipboard API succeeds. A failed
client-local effect reports failure and consumes the request without asking Main to duplicate the
write; the Gateway path retains its secure-context and transient-user-activation requirements.
Attachment replacement, terminal retirement, and renderer disposal suppress late decisions; request
text and decisions never enter browser storage or the shared Workbench document. This intentionally
allows a Host-admitted terminal program to replace the current Desktop clipboard without another user
gesture; restoring per-request confirmation is the rollback boundary. Each attachment Bell reports
the opaque terminal
identity and Host sequence to the Workbench-lifetime coordinator. Only the first admitted delivery
for that terminal sequence produces the short tokenized visual surface cue; duplicate, stale,
reconnected, and replayed deliveries produce no effect, including when distinct attachment streams
interleave. Optional sound and operating-system notification callbacks are separate explicit
application opt-ins, are absent by default, and fail independently after admission. Neither effect
requests focus. The current OBTF progress sample remains a quiet bottom-edge determinate or
indeterminate indicator, and both visual presentations respect reduced motion.

Terminal link hover reads only generation/revision-pinned Host rows. When application mouse
tracking owns the surface, ordinary context intent stays terminal-owned; Shift explicitly returns
that intent to the product. The projected surface captures only the link target and its exact
generation, revision, link epoch, row, and column when the Workbench menu opens. The prepared
Workbench Action applies the `http`/`https`/`mailto` allowlist and revalidates that complete context
immediately before selecting the Browser-local or Electron Main-owned effect. A changed context
settles as a localized blocked result without opening the stale target. Modifier-assisted direct
click uses the same request and revalidation path. The Host-projected hyperlink is data, never an
effect capability.

The headed Canvas 2D witnesses `terminal-parity/FS-05-file-links` and
`terminal-parity/FS-06-url-policy` compose the production projected surface, Workbench Action, and
Browser effect. They prove that `http`, `https`, and `mailto` alone expose hover, context, and direct
activation, while automatic FTP text, unsupported explicit metadata, explicit file metadata,
detected paths, and a direct file-effect request remain inactive. The controlled fixture uses
synthetic generation-pinned Host row windows and a captured Browser opener; it does not exercise an
operating-system URL handler or claim filesystem existence.

The headed Canvas 2D witness `terminal-parity/LC-05-lan-capability` composes the production
projected surface, Workbench Copy Action, Host-selection-content path, and Browser effect. On the
loopback test origin it observes the real Chromium secure-context and transient-user-activation
signals: no activation disables Copy without a Host request, while a trusted click admits one Host
request and one captured plain-text writer call. It then proves that one rejected writer receives no
retry, and that dynamically missing and controlled insecure capabilities disable the Action, invoke
neither Host export nor writer, and retain localized feedback. The fixture captures rather than
mutates the operating-system clipboard and controls the capability-loss and insecure-context probes;
it does not claim browser permission-UI behavior or LAN transport security.

Browser-owned request and presentation identifiers use `crypto.getRandomValues`, which remains available on the explicitly accepted HTTP LAN-preview origin. Runtime code does not depend on the secure-context-only `crypto.randomUUID` API.

For the complete local Desktop flow, run:

```sh
pnpm desktop
```

Electron loads the Web development URL and is detected from its User-Agent. Packaged Electron serves this application's production build from the standard secure `orchbee://app` origin; `apps/desktop` owns packaging and resource delivery but no second renderer implementation. In Desktop, open **Settings → Browser access**. The default action starts loopback-only access and opens the system Browser. Trusted-LAN preview must be enabled explicitly and always displays that HTTP is unencrypted and possession of the link grants full terminal control. The renderer receives sanitized Origins only; Desktop Main owns the secret-link copy and open effects.
