# @orchbee/web

Browser composition entry for the Orchbee Workbench.

## Composition boundary

Electron supplies the strict `agent-integration-management` port from `@orchbee/desktop-client-protocol`. It projects a read-only ten-Agent local inventory and accepts explicit Settings refresh, enable, and disable requests. Browser Preview omits this capability.

`apps/web` is the single runnable product UI for both a normal browser page and Electron's renderer. Environment adapters may provide different transport and privileged capabilities, but they compose the same Workbench, Workspace, Tab, Panel, and projected-terminal application. Desktop must not maintain a parallel renderer UI. The shared Workbench client runtime owns shared-document coordination, projected-terminal presentation, search, actions, and its narrow client-session and projection ports. It has no Electron, `MessagePort`, network, or application-lifetime dependency. The runtime provides each projected renderer session with one stable `TerminalPresentationConfigurationSource`. Its initial complete value opens the surface, and later complete values update that same session and attachment without changing React capability identity or reopening projection. The default source retains unbalanced padding, fixes `fontSize` at 13 to match Ghostty's default 13-point setting, and replaces the source-neutral package face with the bundled Ghostty-aligned JetBrains Mono 2.304 variable faces plus Symbols Nerd Font 3.4.0 fallback. Application startup registers those immutable assets before opening any surface; the same bundle and license links serve Browser Preview and Electron renderers without depending on machine-installed fonts. Each application composition replaces that fallback with one client-local settings controller that restores a strict complete preference record, exposes the same stable presentation source, and publishes complete runtime updates without reopening projection.

The shared client runtime closes local admission before releasing resources. Session retirement
settles pending proposals before invoking transport cleanup; a subscription returned after
synchronous retirement is released immediately. Document startup releases previously acquired
subscriptions when acquisition fails and never reports readiness after disposal. A thrown or
rejected proposal completion supplies no proof that the document was not committed: the coordinator
becomes `control-unavailable`, settles pending flushes, and never automatically resubmits that
proposal. Cleanup attempts every independent owned resource once and preserves single or aggregated
failures. Retired surface capabilities cannot apply late terminal metadata. Deterministic capability
traces cover these local ownership and failure boundaries; they do not claim remote rollback or
exactly-once document delivery.

Browser Preview and Desktop renderer disposal retire their local lifetime before cleanup. A React
unmount failure cannot prevent the remaining routing, settings, transport, and application-lifetime
cleanup attempts. These composition and Workbench cleanup paths share `runApplicationCleanup`:
repeated or reentrant disposal is inert, and a single original error or an ordered aggregate is
reported after all independent steps have been attempted. Composition tests inject failure at the
React root boundary; Desktop also exercises its real DOM render and all eleven capability ports.

Browser Preview uses an in-memory composition. Desktop supplies MessagePort adapters for shared client sessions and local terminal projection plus the application lifetime, Agent observation and integration management, local Project Folder and Worktree, terminal launch profile, terminal defaults, and clipboard capabilities. Its `local-host` projection transport retains strict OBPB structural validation and direct typed render-buffer views. The shared terminal surface retains measured cell geometry, complete background coverage, clipped direct graphics, bounded bell indication, connection notices, and terminal-outcome precedence. Workbench payloads come from `@orchbee/workbench-client-protocol`; Electron capability contracts come from `@orchbee/desktop-client-protocol`.

Electron Desktop additionally supplies Agent-observation and Agent-integration-management ports in
the fixed bootstrap bundle; Browser Preview omits both. The Agent-observation adapter
requests one strict full snapshot before it mounts the application, rejects malformed or
uncorrelated publications, and applies only increasing revisions. It forwards strictly parsed
canonical semantic events independently from snapshots, after Main has published the corresponding
state revision. A ready adapter supplies Workbench with the narrow `AgentSnapshotSource`; an
unavailable adapter leaves the existing terminal application unchanged. Workbench projects exact
Panel and Tab status and offers attention, success, and failure candidates to the terminal-feedback
controller. That controller defaults Agent notifications off, never requests permission from an
event, suppresses visible destinations, deduplicates semantic event identities, and shows only
localized content-free copy when the user has enabled the setting and permission is already
granted. Notification activation focuses the application and invokes Workbench's navigation-only
callback. The renderer owns no canonical Agent transition and cannot submit Agent events,
responses, approvals, or decisions. Renderer reload creates a new adapter and recovers from Desktop
Main's current full snapshot rather than replaying deltas or old notification events.

Agent-observation callbacks use a captured cohort for each publication. An individual observer
failure cannot strand a correlated initialization receipt or suppress the remaining snapshot or
semantic observers. Disposal during publication still settles initialization as unavailable.
The [adapter tests](src/desktop-client/agent-observation/__tests__/DesktopAgentObservationClient.unit.test.ts)
exercise these synchronous callback and receipt orderings without claiming native transport evidence.

For a Host-backed visual terminal, the shared surface does not open projection until it has loaded
the current configured fonts and measured a positive mounted viewport. It sends the resulting exact
columns, rows, physical cell size, padding-aware grid, and document DPR through the same Electron projection command, then pins that presentation configuration for the first renderer. A
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

The recovery controller reserves each opening attempt before notifying observers or acquiring its
request. Synchronous retention, resumption, or disposal cannot overwrite a replacement attempt,
leak a late request result, or continue promotion after its owner retires. Resource ownership moves
before cleanup callbacks run, including input unsubscription and frozen-frame replacement. Disposal
attempts every owned cleanup once, preserves cleanup failures, and respects listeners cancelled
during the final notification. Controlled recovery-controller tests cover these reentrant traces,
late results, and throwing cleanup alongside the bounded retry policy; they do not model-check
arbitrary callback programs or replace real renderer and transport integration evidence. The mounted
surface uses the same cleanup discipline for its paste review, mobile-input subscription, broadcast
registration, and controller: a throwing registration disposer cannot retain the renderer or
projection request. Hook integration tests cover single and multiple cleanup failures.

Each projected surface binds one transient broadcast-input sink to its exact Workbench
Presentation. The sink reports ready only while the recovery controller owns an active transport
and the projected terminal exposes the matching attachment capability revision. Capability change,
retention, transport loss, candidate replacement, freeze, and disposal notify the Workbench before
later input can reuse that sink. One accepted send delegates once to the projected terminal's
attachment-local injection path with the snapshot revision as a compare-and-send guard; unavailable
legacy or test renderers and a changed revision fail closed. The adapter does not coordinate
targets, retry, roll back, or replay a multi-terminal delivery.

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

Initial active surfaces also report display readiness from the same painted-frame or non-canvas
outcome evidence. Workbench uses that witness to acknowledge Agent unread activity only after the
terminal is actually displayed. Retained surfaces cannot supply this read witness.

The active terminal adapter reports `onTerminalEdit` only after typed editing input reaches its
transport port successfully. Text, paste and content-editing keys retire the Client-local completed
Agent display; focus, mouse, cursor movement, copy, rejected input and inactive layers cannot.
Workbench owns this presentation override and leaves canonical Agent outcomes and notifications intact.

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
contract is identical in Electron clients; it is not a second acknowledgement
to Terminal Host. Runtime presentation diagnostics additionally expose request, committed, and
drawn epochs on the same surface. They distinguish independently ordered font preparation from the
single local commit and completed current repaint without creating another attachment lifecycle.

Mounting a projected renderer never moves DOM focus. Workbench retains the active product control;
pointer activation, an explicit terminal focus request, and closing the Find or terminal-text viewer
surface are the only adapter paths that move focus into the hidden terminal input. The projected
root contains that input and both surfaces, so moving among them does not publish a terminal focus
transition.

### Mobile browser terminal interaction

| Field                   | Contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem and scope       | A Browser terminal surface on a coarse, non-hover pointer must expose terminal input, scrolling, and essential keys without redesigning the complete Workbench. Whole-shell responsive layout, native selection handles, multi-touch, pinch zoom, Panel gestures, and configurable key rows are excluded.                                                                                                                                                                                                           |
| Inputs and dependencies | The application consumes the current projected-attachment input readiness, terminal textarea focus, localized messages, `(hover: none) and (pointer: coarse)`, `VisualViewport` geometry, layout-viewport fallback, and semantic key intents. It never infers Host modes or encodes terminal bytes.                                                                                                                                                                                                                 |
| State and ownership     | One mounted projected-terminal renderer owns `MobileTerminalInputState`: attachment availability, native keyboard focus, and a subset of one-shot `control`/`alt` modifiers. The React surface only renders that prepared state. The Web composition adapter owns current visual-viewport width, height, and offsets as reversible presentation geometry; it owns no terminal or Workbench state.                                                                                                                   |
| Modifier transitions    | Tapping Control or Alt toggles that one-shot modifier. The next eligible toolbar key, physical key, or independent single printable ASCII native input consumes all armed modifiers before publication. Composition start, paste, multi-character or non-ASCII committed text, blur, read-only downgrade, attachment replacement, recovery, and disposal clear them without transforming or replaying input. A blocked or failed send still consumes the snapshot so stale modifiers cannot affect a later command. |
| Key row and focus       | The canonical `projected-terminal.mobile-input-bar` appears only for the coarse non-hover media condition and contains Show/Hide Keyboard, Escape, Tab, Control, Alt, and four Arrow controls. Its key controls retain textarea focus and use the exact active attachment revision; the keyboard toggle reflects actual textarea focus and never claims that the operating system displayed a keyboard. Unavailable or read-only input disables input-producing controls.                                           |
| Viewport projection     | The composition applies each finite positive `VisualViewport` resize or scroll sample to the application root before terminal measurement; an unavailable or invalid API uses the layout viewport. The root respects safe-area insets, and the existing terminal `ResizeObserver` remains the only writer of projected grid resize. Keyboard close restores the latest full visual viewport without persisting geometry.                                                                                            |
| Invariants and failure  | Touch scrolling never arms a modifier; controls never publish to a replaced attachment; no mobile state enters the shared Workbench document, browser storage, or Terminal Host. Missing browser focus or viewport capabilities degrade to an unchanged layout and typed unavailable input rather than synthetic success. Desktop pointer and hardware-keyboard behavior remain unchanged.                                                                                                                          |
| Verification and limits | Deterministic state tests cover every modifier transition and replacement. Renderer and React tests cover native capture ordering, revision pinning, localization, and focus retention. A controlled headed mobile-viewport trace remains required to verify media-conditioned visibility, touch tap/scroll, viewport shrink/restore, grid resize, and both locales; real iOS and Android keyboard appearance remains manual release evidence.                                                                      |

Each attachment publishes only its own focus claim and projection demand. Terminal Host combines
eligible claims for a shared terminal, so one attachment losing focus or visibility cannot emit a
terminal-wide `FocusOut` while another eligible claim remains; reactivation requires a fresh claim.
Every opening or explicit reopening of that compact find surface replaces an empty or retained
query with the bounded preview of the current active Host selection when one is available; without
one, it preserves the current query. Search-owned active ranges remain authoritative Host selections
rather than a second application selection model. Each such invocation focuses and selects the
complete query, while Escape from any find control closes the surface and restores terminal focus.
Match options live inside the query field, ordinary search progress uses an
inline localized count, Left and Right Arrow move between option toggles, and pointer option changes
return focus to the query. Only actionable failures, timeouts, or retained-history notices expand the
toolbar with complete localized feedback.

The Find controller publishes selection-seeded opening as one complete snapshot. Query, option,
and lifecycle transitions fence later search effects across synchronous subscriber and driver
callbacks, so an obsolete call cannot restart or clear newer work. Independent notices and
same-query refocusing preserve pending search admission. The
[controller tests](src/workbench-client/terminal-find-surface/model/__tests__/TerminalFindController.unit.test.ts)
cover replacement, close, disposal, reentrant reopening, and those admission-preserving updates.

`Ctrl+Shift+G` opens the last shell-marked command output in an application-owned modal viewer. One
replace-latest Action prepares an exact generation-and-revision plain-text document through the
bounded general-buffer lane; stale, evicted, alternate-screen, unavailable, or over-limit reads
never publish partial output. The prepared document remains immutable while literal
case-insensitive search, Enter/Shift+Enter navigation, native text selection, and the separate
`terminal.output.copy` Action operate locally. The viewer traps focus, restores terminal focus on
close, and deliberately does not launch an external pager or re-render ANSI styling. See the
[Kitty alignment contract](../../packages/terminal/docs/kitty-command-output-viewer-alignment.md)
and the `projected-terminal.text-viewer` experience component.

`Ctrl+Shift+H` opens the complete retained primary-screen history through that same private viewer.
The exact snapshot projector joins soft wraps, preserves hard row boundaries, records the captured
visible-top offset, and publishes either one complete document or a typed empty/unavailable result.
Command-output and scrollback preparation replace each other before publication; their immutable
documents retain source-specific copy Actions. See the
[Kitty scrollback-viewer contract](../../packages/terminal/docs/kitty-scrollback-viewer-alignment.md).

Prompt navigation and text-viewer preparation each own a replace-latest coordinator for one Action
registration lifetime. Ownership changes before cancellation invokes observers, so a replacement
started by an abort listener remains current and cancellable. Disposal closes admission before
aborting current work; an already-cancelled caller cannot displace an active request. The controlled
[navigation](src/workbench-client/projected-terminal-actions/__tests__/createProjectedTerminalPromptNavigationCoordinator.unit.test.ts)
and [preparation](src/workbench-client/projected-terminal-actions/__tests__/createProjectedTerminalTextViewerPreparationCoordinator.unit.test.ts)
tests cover reentrant replacement, disposal, and cancelled admission.

After a projected frame commits, the renderer forwards its complete nullable title,
current-directory metadata, and transient `idle`/`active`/`unknown` close activity with the exact
`TerminalPresentationId`. The shared client runtime
normalizes that observation with the trusted terminal-machine hostname from the client-session
bootstrap and applies it to the matching Workbench presentation. Natural titles and accepted
current CWD remain transient; durable fallback/custom Tab-title policy and shared-document
projection remain Workbench-owned. Electron clients therefore use the same
metadata policy, and the page never derives terminal trust from its own URL hostname.

Suspicious paste text opens one localized review dialog owned by the shared projected-terminal
surface. It presents a bounded, control-visible preview of the retained original text and identifies
multiline input, controls that will be replaced, and an embedded bracketed-paste terminator. Cancel
publishes nothing; approval sends the sanitized text together with the mode that was reviewed, while
Terminal Host rejects a mode change before encoding or queue admission.

Each projected surface registers the Workbench-owned minimal terminal Action catalog into the
viewport's application runtime and scope: Copy, Paste, Select All, Find, Find Next, Find Previous,
Show Last Command Output, Copy Command Output, Show Scrollback, Copy Scrollback, Scroll to Bottom,
Clear, and Reset. Availability is
synchronous and remains authoritative when a prepared invocation executes. Context-menu and custom-shortcut Paste read text only through an
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
work. Electron uses the same renderer contract, and no renderer command can request its
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
preparation, and an unavailable active attachment publish nothing. Browser Preview clients
receive no backing-path capability, and Windows remains unsupported until the active shell dialect
has an equally bounded escaping contract.

The stable headed `terminal-parity/IN-08-drop` witness uses a real Electron `File` and backing-path
bridge on the current POSIX Desktop platform. Its protected `dragover` fixture exposes a `Files`
marker without a readable `FileList`; the final `drop` supplies the disk-backed object. It observes
direct insertion into the middle of an existing shell line, no Paste Review, no automatic
execution, and execution only after a separate explicit Enter. Owner-local tests cover FileList
order and file-kind item fallback, Ghostty-compatible escaping, the 32-file and 4,096-code-point
bounds, Unicode, every control range, missing or malformed bridge values, unsupported platforms,
stale preparation, and Browser Preview capability absence. The witness does not automate Finder or claim
every native operating-system payload variant, Windows support, Browser/LAN path access, directory
enumeration, file-content reads, uploads, or filesystem-link activation.

The packaged `orchbee:` origin and Electron User-Agent token select Desktop mode. The User-Agent discovers the adapter for loopback development; Desktop authority arrives only through the validated sandboxed bootstrap. An ordinary HTTP(S) origin selects the in-memory Browser Preview. Unsupported runtime markers fail closed.

Desktop bootstrap claims its single renderer slot before invoking the renderer factory. Reentrant
bootstrap messages close their unadmitted ports. Unload retires the owner before cleanup; a renderer
returned after that retirement is released immediately and cannot publish bootstrap security markers.
Composition tests cover duplicate startup, late acquisition, and failed bootstrap cleanup.

## Navigation

All compositions use one TanStack Browser Router. Canonical Workspace routes use `/workspaces/$workspaceId[/tabs/$tabId]`, independent of nullable Project membership. Old Project-qualified Workspace paths are not admitted. Group-only links resolve current membership; unknown Workspace/Tab identities remain unavailable. Navigation pushes history; canonicalization replaces it. Renderer focus recovery restores the exact surviving Workspace, Tab, Panel and child before explicit route reconciliation. See [`docs/navigation-routing.md`](docs/navigation-routing.md).

## Settings

Local Desktop Settings includes Agent Integrations when that capability is present. Startup,
search, category navigation, and reminder rendering perform no mutation. After a supported Agent is
directly launched in an integrated local shell, Main lazily inspects only that provider and the
one-per-session reminder can open Agent Integrations Settings. Only an explicit switch action there
can mutate provider configuration. Reminder dismissal stores only the Agent ID, and remote setup
remains manual-only.

General offers System, English and Simplified Chinese. Desktop reads the same confirmed device
preference as TUI; Browser Preview keeps an origin-local record. System follows declared locale
inputs, while explicit locale wins. Confirmed changes update mounted product copy without reload.

The common Settings action opens an application-owned full-window overlay rather than navigating to another route. The Workbench subtree becomes inert, but its React tree and every projected terminal attachment remain mounted. Desktop common preferences use the shared device store with source-bound drafts, confirmed saves, conflict and unknown-outcome recovery. Host typography, pixel padding and opacity remain renderer-local. Browser Preview retains its independent strict origin records. On macOS Desktop, a separate strict local record selects whether neither, left, right, or both Option keys act as terminal Alt; the default is off and accepted changes update every mounted projected terminal without replacing its attachment. Sixty-seven frequent Workbench and focused-terminal shortcuts, including application-wide font-size increase/decrease/reset, creating a terminal Tab in the exact displayed terminal's current directory, four directional Panel-focus commands, previous/next cyclic Panel focus, ten exact positional Panel-focus commands, visible Panel focus and swap, four directional Panel-movement commands, proportional Panel equalization, keyboard Panel resize mode, circular active-Tab movement within its pinned or unpinned section, visible terminal Hints, Show Last Command Output, and Show Scrollback, use a separate strict local override record; accepted changes update the mounted Workbench and every projected terminal without replacing an attachment or connection. Standard defaults reserve unshifted terminal control sequences, while runtime Action availability remains authoritative before any key is consumed. Application theme remains separate from terminal display theme, and terminal overrides never mutate Host metadata, palette, cursor, VT, or OSC state. Local Desktop additionally injects a semantic shell-profile control. Its explicit editor can create, rename, select a system or custom executable, preserve literal argument order, select the default, delete, and confirm reset; every accepted change affects only terminals created after the shared catalog commits. Browser Preview compositions receive no profile capability. Desktop automatically loads Shell integration for supported new local Zsh, non-login Bash, and Fish terminals without editing startup files; Settings explains that boundary, shows exact shipped source commands as manual fallbacks, and owns a separate default-off device-local completion-reminder policy whose notifications never include command text. A paste-only Ghostty migration dry-run reports lossless current targets, manual-review cases, ignored keys, and invalid lines without reading files, applying values, or persisting source text. The Remote development category is another read-only projection: it explains the `xterm-256color` baseline, exposes one constant shell diagnostic, distinguishes observed environment and terminfo outcomes, and states that Orchbee does not silently forward extra environment variables or modify the remote host. Opening, searching, or switching to this category starts no process, writes no terminal input, and persists no remote data. See [`docs/terminal-settings.md`](docs/terminal-settings.md) and [`docs/keyboard-shortcuts.md`](docs/keyboard-shortcuts.md).

The exact headed `terminal-parity/SU-09-theme` witness holds a prepared Host result and its
revision-tagged frame before one release, then observes the terminal defaults, palette, cursor,
overlays, pixels, and application surface publish one matching revision. It uses the controlled
runtime-presentation Canvas fixture and does not claim a persisted settings migration, a native
operating-system theme event, or a renderer-owned terminal palette.

## Local development preview

Locale preference, automatic fallback resolution, and shortcut-platform setup belong to the Web
composition shared by Browser Preview and Desktop.

`pnpm web` renders the Workbench shell with its localized sidebar, standalone terminal context, Project and Workspace navigation, and per-tab Panel workspace. The browser composition creates one `ActionRuntime` and one in-memory `WorkbenchProjectWorkspaceRuntime`. It starts with only the standalone Terminal context. A person can create an in-memory Project by entering a folder path; the Project add control first preserves the common creation menu, then `Select Folder…` opens the browser-only path entry in place of a native picker. The browser uses that path as the initial Project Folder and terminal CWD without claiming that the path exists. `New Folder…` remains visible but disabled because the composition performs no filesystem, Git, PTY, or durable-storage effect.

Electron Desktop injects native existing-Folder acquisition and managed-Worktree capabilities into the shared application. `Select Folder…` uses the operating-system picker, local Project Folder attachment uses that same capability, and `New Worktree…` opens the fixed-location Git form. Browser Preview retains manual Folder path entry and receives no native filesystem or Worktree effect. `New Folder…` selects a native parent and explicitly creates one absent child, then observes the existing Project Action, exact canonical publication and Workspace activation. The existing route coordinator reports its accepted current target separately from pending navigation, so creation never acknowledges an unrelated route. Created or uncertain filesystem outcomes are preserved for Select Folder recovery; no new provisioning or filesystem API is introduced.

An OSC 52 write never shares the selection-copy fast path. The shared renderer automatically processes
each exact bounded UTF-8 request without presenting a confirmation surface. Its decision repeats the
exact attachment, Host sequence, and text; Desktop Main performs the privileged Electron effect,
while the portable Browser platform-effects adapter reports success only after its local Clipboard API succeeds. A failed
client-local effect reports failure and consumes the request without asking Main to duplicate the
write; the Browser adapter retains its secure-context and transient-user-activation requirements.
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
that intent to the product. The projected surface captures only the typed link target and its exact
generation, revision, link epoch, row, and column when the Workbench menu opens. Every composition
keeps URL activation within the `http`/`https`/`mailto` policy. A detected file or directory target
becomes interactive only when that exact attachment bootstrap carries the optional Desktop-local
file-effects capability; Browser Preview, archived, and source-neutral attachments keep it inert.
The prepared Workbench Action revalidates the complete context immediately before selecting its
environment-owned effect. A changed context settles as a localized blocked result without opening
the stale target. Command-click on Apple platforms or Control-click elsewhere uses the same typed
request and revalidation path. Host-projected text and hyperlink metadata remain data, never an
effect capability.

With that Desktop-local capability, a fine-pointer hover over a detected PNG, JPEG, GIF, WebP, AVIF,
or BMP path starts a 300 ms dwell without probing unrelated paths. The resulting request remains
bound to the attachment, target, generation, revision, link epoch, row, and column. Replacement,
pointer leave or press, wheel input, geometry or revision change, attachment retirement, and
disposal cancel the pending presentation; a late completion cannot replace the latest hover. The
surface renders admitted bytes in one pointer-transparent card anchored next to the source cell and
keeps terminal focus and selection unchanged. Missing, unsupported, invalid, or unreadable images
fail silently. Browser Preview never requests or receive local image bytes.

The headed Canvas 2D witnesses `terminal-parity/FS-05-file-links` and
`terminal-parity/FS-06-url-policy` compose the production projected surface, Workbench Action, and
Browser effect. They prove that `http`, `https`, and `mailto` alone expose hover, context, and direct
activation, while automatic FTP text, unsupported explicit metadata, explicit file metadata,
detected paths, and a direct file-effect request remain inactive. The controlled fixture uses
synthetic generation-pinned Host row windows and a captured Browser opener; it does not exercise an
operating-system URL handler or claim filesystem existence.

The headed Canvas 2D witnesses `terminal-parity/FS-19-local-file-links` and
`terminal-parity/FS-20-image-preview` enable the optional local capability against the same real
projected surface with a fake Desktop effect. They prove relative-path resolution, modifier opening,
300 ms image intent, replacement cancellation, rejection of a late completion, and the production
anchored card's final PNG source and pointer-transparent interaction. Their captured opener and
synthetic bytes do not launch an operating-system application, automate Finder, claim native image
decoding, or grant Browser filesystem access.

The headed Canvas 2D witness `terminal-parity/FS-07-keyboard-hints` uses that same production
surface and bounded row fixture. `Mod+Shift+O` labels visible URL, path, file-location, and hash
targets; label keys never reach terminal input, selecting the hash inserts its exact text, selecting
the URL reaches the captured Browser opener, and Escape removes the overlay without an effect. The
witness does not claim off-screen discovery, filesystem/editor activation, Kitty customization, or
operating-system URL delivery.

The headed Canvas 2D witness `terminal-parity/FS-08-command-output-viewer` uses the production
surface with synthetic OSC 133 prompt rows. `Ctrl+Shift+G` opens the exact retained command output;
search and visible match navigation stay inside the immutable document, Copy reaches one captured
plain-text writer with the same content, and Escape restores terminal input focus. The witness does
not claim external-pager execution, ANSI preservation, complete scrollback, or operating-system
clipboard mutation.

The headed Canvas 2D witness `terminal-parity/FS-09-scrollback-viewer` uses the production surface
with a bounded synthetic retained-history snapshot. `Ctrl+Shift+H` opens all retained primary-screen
text at the captured visible top; search, match navigation, source-specific Copy, Escape focus
restoration, and unchanged attachment identity use the production viewer and Actions. The witness
does not claim history beyond Host retention, ANSI preservation, an external pager, or operating-
system clipboard mutation.

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

Electron loads the Web development URL and discovers its adapter from the User-Agent. Packaged Electron serves this production build from the standard secure `orchbee://app` origin; `apps/desktop` owns packaging and resource delivery. Browser Preview remains an independent in-memory development surface.

## Unit verification

Run `pnpm --filter @orchbee/web test` for the unit suite. On Windows, Vitest uses at most four workers (or the available CPU count if smaller), bounding process creation and filesystem contention when other workspace suites run concurrently. All files, assertions and deadlines remain enabled; other platforms retain the runner defaults.
