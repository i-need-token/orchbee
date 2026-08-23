# @orchbee/terminal

Source-neutral browser client surface for a terminal whose authoritative VT state lives in Orchbee Terminal Host.

## Scope

This package owns one projected terminal surface:

- atomic packed-frame commit and snapshot recovery;
- WebGPU rendering with a Canvas 2D fallback;
- bounded history-window presentation;
- client-side search and bounded logical-line link discovery;
- current bounded semantic command-completion metadata;
- Host-owned selection interaction and copy correlation;
- keyboard, committed text, IME, mouse, focus, paste, resize, clear, and reset intents; and
- visible-viewport accessibility.

It never receives or parses PTY output, loads a VT engine, or exposes `write(ptyBytes)`. Terminal Host owns the PTY, Ghostty VT state, terminal modes, retained history, selection endpoints, and mode-dependent input encoding. The application owns transport, terminal inventory, product topology, clipboard effects, external navigation, and localized product UI. The machine-readable scope contract lives in [`project.json`](project.json).

The accepted [projected-terminal experience compatibility proposal](../../proposals/terminal/projected-terminal-experience-compatibility.md)
defines the remaining cross-owner recovery contract; it is not current runtime authority.

Packed metadata carries only the latest optional command-completion sequence, duration, completion
timestamp, and normalized exit code decoded from the Host frame. This package neither interprets
Shell output nor owns reminder policy, notification permission, focus rules, freshness,
deduplication, rate limiting, or product copy. Projection-buffer inputs have no completion fact;
snapshot and delta inputs preserve the strictly decoded optional value.

`clear()` and `reset()` are semantic Host requests. `clear()` erases the active screen and homes its
cursor while preserving terminal modes. `reset()` first repairs client-owned composition, pressed
input, selection, search, hover, and history-viewport state, then asks the Host to perform Ghostty's
full reset. Neither method injects terminal escape bytes or reuses projection `store-reset`
recovery.

Input repair keeps native composition lifetime separate from local commit eligibility. An active
composition may be eligible, or it may be cancelled while the browser still owes a terminal
`compositionend`. Reset, textarea blur, window blur, hidden-document transition, and disposal
clear the textarea and renderer preedit immediately, revoke commit eligibility, and clear key,
composition, `beforeinput`, `input`, paste, and mouse correlation records. A committed DOM `input`
event is a fallback for browsers and IMEs that do not provide a usable `beforeinput` payload; its
textarea value is consumed once and then cleared. Late composition updates and either late
`beforeinput`/`input`/`compositionend` order cannot republish preedit or text; the late end only
settles the cancelled native work. A new `compositionstart` supersedes cancelled work even when its
old `compositionend` never arrives. Refocus, a non-composing key, or standalone non-composition
`beforeinput` also restores local progress without waiting for that end. In an uncancelled
composition, `beforeinput`, `input`, and `compositionend` in either browser order commit the text
at most once, and the state transition is complete before the synchronous command-port send.
The macOS `keyCode=229` path defers interim independent `beforeinput` text until the native textarea
value settles, preserving IME-produced punctuation instead of publishing the physical ASCII key.
Unmodified punctuation and digit keydowns also yield to the native textarea so third-party macOS
input methods can publish full-width punctuation; if no native value arrives within 30 milliseconds,
the original ASCII character is sent as a deterministic fallback. Native settlement participates in
the same ordered input dispatcher: an intervening key, paste, or pointer input seals the current
native batch before that later input is admitted, so deferred digits and punctuation cannot move
past subsequent physical keys. The same short-lived settlement marker handles browser event orders
where the native `input` arrives before `keydown`.

On macOS, the projected terminal also consumes a source-neutral Option-as-Alt mode: `off`, `left`,
`right`, or `both`. It tracks the physical Option side from `AltLeft` and `AltRight`, resolves one
boolean semantic intent per admitted key event, and updates that mode live without recreating the
terminal. It never prepends Escape to committed text. Terminal Host consumes the event intent and
continues to own the actual Ghostty keyboard encoding for the current terminal modes.

Live preedit is a reversible presentation that begins at the projected Host cursor and wraps in
row-major order across the visible grid. Narrow and wide cells remain atomic, overflow beyond the
last visible row is clipped without scrolling or mutating Host state, and the hidden native input
anchor follows the same wrapped caret inside a positioned helper stacking context so the platform
IME candidate window remains aligned.

Normal deduplication records are single-use. The first `beforeinput` check consumes the preceding
key record even when it does not match; matching composition, `beforeinput`, and opposite-source
paste checks consume their record. Their window is strictly less than 100 milliseconds. A
cancelled or end-first composition uses a separate settlement guard that repair does not clear.
Only a composition-origin `beforeinput`, identified by `isComposing`, `insertCompositionText`, or
`insertFromComposition`, consumes that guard; later independent `insertText` with the same text is
not suppressed. Composition-origin special input types are settled before paste, line-break, or
deletion handling, so cancelled native work cannot enter those branches. A new `compositionstart`
establishes ownership of the next composition events and replaces any older unsettled native work
because browser events provide no composition identifier.

A final mouse release and every repair clear the retained mouse sample. Repair closes a local input
admission gate and starts one cleanup transaction before clearing ownership or calling any helper
that can synchronously send a command. Input handlers also retain the repair epoch across history
recovery, selection clearing, and focus reporting; a repair that closes and later reopens admission
invalidates the older continuation before it can publish a key press, preedit, text, mouse input, or
stale correlation. A composition text commit additionally retains the exact staged input-state
identity, so returning from its synchronous send cannot clear textarea or preedit state installed by
a reentrant repair and newer composition.

Ordinary admitted input remains FIFO within one dispatcher generation, including while paste review
is pending. Lifecycle repair closes admission and invalidates that generation before snapshotting
cleanup, so reset, blur, visibility loss, and disposal do not wait for a paste review that may never
settle. Queued commands and late review resolutions or rejections from an invalidated generation are
observed but inert. A newly opened generation progresses independently after repair.

Paste inspection retains the original text before replacing unsafe C0/C1 controls with spaces, and
detects an embedded bracketed-paste terminator in that original text. Multiline text without
bracketed protection, replaced controls, and embedded terminators require the application-owned
localized review surface; approval sends only the sanitized text. The review snapshots projected
bracketed-paste mode and carries that expectation with the semantic input. Terminal Host compares it
with authoritative Ghostty mode while holding the terminal lock and encodes only on an exact match;
a mismatch rejects the paste without queue admission or automatic replay so a new paste can be
inspected against current state.
When an admitted clipboard event has no plain text but contains an image item, the adapter leaves
the system clipboard untouched and emits exactly one synthetic Control+V press/release pair. This
matches the Agent paste convention without reading image bytes, creating a file, or detecting a
particular Agent. Plain text remains authoritative when both text and image items are present.

The adapter tracks admitted desired ownership separately from ownership already published to the
Host. Physical key presses and pointer buttons are persistent ownership; synthetic
`beforeinput` Enter/Backspace/Delete commands and wheel button 4/5/6/7 reports are stateless and
never enter a held ledger. The adapter stages local desired state before any synchronous boundary,
calls the command port under one publication gate, and commits Host-published ownership only after
`port.send` returns synchronously. A lifecycle callback during that call is coalesced until the
outcome is known. A synchronous throw is the existing browser-adapter pre-acceptance failure
boundary: a failed press rolls back its local ownership and correlation, while a failed release or
focus-out retains known Host ownership for a later repair attempt. A returned promise may still
reject diagnostically, but synchronous return records local transport acceptance and makes no claim
about remote delivery.

Each physical pointer button snapshots its owner when pressed: current Host-projected mouse
tracking owns the button unless Shift is held, and otherwise terminal selection or link interaction
retains it. Later modifier and projected-mode changes cannot transfer an application-owned press.
Application-owned presses capture their pointer, derive drag state only from the application-owned
button ledger, and release outside the canvas normally. A buttonless move, pointer cancellation, or
lost capture queues one semantic release for every admitted or Host-published application button
before clearing capture, so later lifecycle repair cannot duplicate the release. While any mouse
tracking mode is projected, local link lookup and activation require Shift; activation rechecks that
gate at click time, and a completed application-owned primary press cannot activate a stale link.
Wheel ownership remains a stateless decision for each wheel event. Application-owned wheel input
normalizes pixel, line, and page deltas against the projected cell and grid dimensions, accumulates
sub-cell motion independently per axis, and reports vertical button 4/5 steps before horizontal
button 7/6 steps. Shift bypasses application mouse ownership. On the primary screen, normalized
vertical steps navigate bounded retained history; on the alternate screen, writable input maps them
to matched semantic Arrow Up/Down taps. A read-only alternate screen consumes vertical wheel
input without emitting Host input, while local horizontal-only wheel input remains browser-owned.

The canvas pointer uses the same ownership policy rather than independent link-only styling. A
fixed application-owned gesture and unmodified projected mouse tracking use the default pointer;
terminal selection uses the text pointer; rectangular selection uses a crosshair with Option on
Apple browser platforms or Alt plus Control/Meta elsewhere; and an enabled, permitted link uses the
pointer cursor only while terminal interaction owns the pointer. Projected mode commits, modifier
changes, gesture transitions, and link presentation all re-evaluate this policy, so clearing a link
restores the derived base pointer instead of an empty CSS override.

Browser key normalization maps legacy `OSLeft`/`OSRight` codes to Ghostty's
`MetaLeft`/`MetaRight`, recognizes complete W3C key strings instead of measuring UTF-16 code units,
and records consumed modifiers only when the event carries printable text and only as a subset of
the active modifiers. A key release uses held ownership only to identify the physical key; its text,
modifier, and consumed-modifier fields come from the current `keyup` after removing the released
right-side modifier from browser-owned tracking. A `keyup` that reenters a synchronous input
publication is deferred until the owning press outcome is known, or is subsumed by a concurrent
lifecycle repair, so it cannot leave a published press without its release.
After application and terminal semantic shortcuts have had their capture opportunity, an otherwise
unclaimed Apple Command-modified key is left to Browser/Electron native handling. It is neither
published as terminal input nor cancelled through a key-name allowlist.
Synthetic Enter, Backspace, Delete, and alternate-screen arrow input use the same held publication
path as matched press/release taps. They retain no steady-state client ownership, while a transport
loss between the pair remains visible to the Host attachment ledger for detach repair.

Shift-only Page Up and Page Down, including their keypad codes, remain local retained-history
navigation and never become Host key input. Every admitted input-producing path except buttonless
application-mouse motion shares one ordered surface transition: it first requests the live bottom
when history is visible, then clears an active Host selection, and only then publishes the semantic
key press or repeat, text, approved paste, or application-mouse input. A buttonless motion report
admitted by any-event mouse tracking is still published to the Host, but preserves the retained
history position and active Host selection. Composition activation performs the shared transition
before presenting preedit. A physical or synthetic key release closes existing Host input ownership
without starting that transition, so a selection established between its press and release remains
visible. Focus reports, lifecycle and pointer-gesture repair, rejected or aborted paste review, and
local history navigation preserve the current history and selection surface.

`terminal-parity/IN-07-pre-input` drives a real buttonless `PointerEvent` through the headed Canvas
surface while retained history and a Host selection are active. It requires one application motion
report, no return-to-bottom or selection-clear command, and a preserved readable selection. The
same trace then synthesizes window focus loss and collapses the real accessible DOM range; no Host
selection-clear command is published and the readable Host selection remains active. Owner-local
regressions check both repairs while retaining the existing button-press transition requirement.

Cleanup releases only published ownership: a queued release cannot hide a Host-visible press, while
a queued or failed press cannot produce an orphan release. Blur and visibility loss release
published keys and mouse buttons once in press order, then publish focus-out only when this session
published focus-in. Disposal closes admission permanently and follows the same order, omitting
focus-out when already unfocused. Each disposal cleanup command is attempted once; a synchronous
failure is reported and retained as known ownership, but teardown cannot retry after its DOM and
listeners are removed. Reset itself requires no focus-out and retains focus. If reset invalidates a
queued or failed focus-in, its replacement generation attempts focus reconciliation before later
key or mouse publication; a synchronous reconciliation failure remains retryable by the next input.
A projected terminal treats its hidden textarea, accessibility descendants, and application-owned
controls mounted under the same root as one focus boundary. Movement within that boundary neither
repairs input nor repeats focus reporting; leaving it releases published key and mouse ownership
before one focus-out, while entering through a non-textarea descendant publishes the matching
focus-in. The application decides when to call `focus()`; opening the package surface does not claim
initial product focus.
Desktop and pen pointer activation focuses the hidden textarea on pointer down. Touch pointer
handling completes before a non-passive canvas `touchend` prevents native compatibility mouse and
caret handling, then focuses the textarea; a writable textarea opens the mobile virtual keyboard.
Touch activation alone publishes no PTY input and preserves Host selection. The resulting
`beforeinput` and composition events use the same shared pre-input transition and exactly-once
deduplication as other browser input, while a read-only attachment remains focusable without
admitting mobile text.
Every raw, key, committed-text, paste, application-mouse, focus, and repair command carries the
current positive attachment capability revision. Direct construction defaults to the initial
revision `1`, while Desktop and Gateway bootstraps forward the Host-returned revision.
`applyInputCapability` accepts only the exact attachment and terminal identity, an exact replay, or
the next monotonic revision with a changed input value. A stale revision is inert; an identity
mismatch, same-revision conflict, skipped revision, or changed revision with unchanged authority
fails closed to read-only. Downgrade invalidates the dispatcher generation and immediately clears
composition, native text settlement, paste review, held key/mouse/focus ownership, and pointer
capture without sending client-side releases that duplicate the Host repair. Upgrade removes the
native read-only guard but restores no prior input or queued work; the next admitted input is
stamped only with the new revision.
If the projected client disappears before this cleanup completes, Terminal Host releases only that
attachment's accepted key, application-mouse, and focus ownership before revoking its capability;
another attachment is unaffected.

A blur, visibility loss, or disposal that reenters while reset is staging or sending cleanup is
coalesced into the same transaction; the transaction drains key release, mouse release, and any
newly required focus-out before it reopens admission or disposes. The Host reset follows that
cleanup in the replacement dispatcher generation. Current-generation DOM paste-review rejection is
reported through the existing command error channel and does not stall later input; rejection from
an invalidated generation remains inert. The versioned mature correlation trace and its pinned
source revision are maintained as strictly parsed, data-driven replay input in
[`docs/input-repair-conformance-v0.0.1.json`](docs/input-repair-conformance-v0.0.1.json).

`ProjectedTerminal` represents exactly one attachment. Project, Workspace, Tab, Panel, and visibility policy remain above this package; a hidden terminal does not require a live renderer surface, and projection/history demand can be paused independently of the Host runtime.

Host bell effects arrive through `ProjectedTerminal.onBell` as attachment-scoped monotonic events.
The package suppresses stale or duplicate sequences but performs no sound or visual effect; the
embedding Desktop or Browser application owns that policy.

The headed `terminal-parity/PR-11-bell` witness composes three real Canvas 2D projected surfaces
with distinct attachment identities and one shared Workbench bell coordinator. It proves one effect
for a duplicate sequence, acceptance of distinct interleaved sequences, suppression after an
attachment reconnect, and independent sound/notification failure boundaries. The controlled
fixture does not claim native audio output, operating-system notification delivery, WebGPU, or
cross-browser behavior.

## Install

```sh
pnpm add @orchbee/terminal
```

## Use

The caller provides opaque Host identity, localized accessibility labels, and a transport-neutral command port. Binary frame and row-window payloads are forwarded without VT parsing or per-cell broker reconstruction.

```ts
import { ProjectedTerminal } from '@orchbee/terminal';

const terminal = new ProjectedTerminal({
	attachmentId,
	capabilityRevision: '1',
	creationId,
	hostEpoch,
	terminalId,
	canResize: true,
	labels: {
		input: i18n.t('terminal.input'),
		screen: i18n.t('terminal.screen'),
	},
	port: {
		send: (command) => hostConnection.send(command),
	},
});

terminal.onRendererHealth((event) => rendererDiagnostics.record(event));
terminal.onFrameTelemetry((frame) => rendererMetrics.record(frame));
await terminal.open(document.querySelector('#terminal')!);

hostConnection.onFrame((frameBuffer) => terminal.applyFrame(frameBuffer));
hostConnection.onRows((rowBuffer) => terminal.applyRows(rowBuffer));
hostConnection.onTerminalExited((event) => terminal.notifyExited(event));
```

Applications that must start a visual terminal before creating its attachment can call
`measureMountedTerminalGeometry(root, configuration)`. The helper waits for the configured faces
and uses the same font metrics, padding, complete-cell grid, owner-document DPR, protocol bounds,
and PTY pixel limits as the first projected surface. It returns `undefined` while the element has no
positive mounted content box or padding consumes the grid; callers observe the surface again rather
than substituting a timeout or default geometry. The configuration used for this measurement must
remain the initial configuration used to open the projected renderer.

Opening waits for the configured face with representative Latin, CJK, emoji, and Nerd Font glyphs, then installs frame, renderer, input, resize, and accessibility handling before activating projection demand. Each open attempt transactionally owns its canvas, input, accessibility nodes, renderer, observers, and listeners: setup failure removes only those resources and returns the same instance to `constructed` for retry; duplicate opens reject without changing the current surface, and disposal remains terminal. The package default remains browser-generic `monospace`; an application may provide a provisioned product face, and Orchbee's shared Web/Desktop application registers its pinned Ghostty-aligned primary and symbol fallback before opening a surface instead of depending on a machine-installed font. Surface sizing subtracts computed padding from the available grid, retains the mature two-column minimum, and proposes no PTY geometry when padding consumes either content dimension. Padding balance remains opt-in. A measured resize is sent before active projection demand and is deduplicated only while it agrees with the latest Host-committed geometry or remains inside the short observer re-entry window. A Host geometry change, broker rejection, transport failure, or later unchanged observation makes the proposal eligible for retry. The hidden IME control uses the same font and follows the projected cursor so the native candidate window stays aligned. Device-pixel-ratio changes rebuild renderer geometry and terminal pixel dimensions instead of leaving a stale atlas. A frame commits into a bounded row-partitioned structure-of-arrays store and is acknowledged only after draw scheduling. `FrameApplied` acknowledges the client commit, not completed painting. Exact duplicates are re-acknowledged; invalid identity, sequence, generation, dimension, or delta-base input requests a complete snapshot without mutating the committed grid.

A valid frame that arrives while font or GPU initialization is still pending remains the single
committed opening frame. Its `FrameApplied` acknowledgement is withheld until renderer readiness
admits a draw of that commit; initialization then schedules the retained frame before acknowledging
it. Disposal clears the pending acknowledgement instead of publishing it from a retired surface.

Every open surface observes its container and adapts local padding, rendering, pointer, and hidden
input geometry, including when it is a resize observer rather than the lease owner. Only the
surface with current resize ownership may turn that measurement into a Host resize request;
observers continue to consume the Host's canonical frame geometry and never reinterpret local
clipping as competing PTY geometry. `setResizeOwnership` applies an attachment-scoped broker event.
Promotion clears resize deduplication and immediately measures the existing surface, so ownership
does not require reopening the attachment.

An open attachment accepts one complete `TerminalPresentationConfiguration` transaction containing
`fontFamily`, `fontSize`, `fontWeight`, `fontWeightBold`, `fontLigatures`,
`fontMetricModifiers`, `paddingBalance`, `allowTransparency`, `backgroundOpacity`, and
`defaultColorOverride`. The override is either `null` or a strict, normalized lowercase `#RRGGBB`
foreground/background pair. `backgroundOpacity` is a source-neutral `0..1` configuration value;
the composing client applies it to its own default-background root rather than to the terminal
surface. The override remains a source-neutral compatibility projection for clients that do not
participate in the Host runtime-theme transaction. Orchbee's shared Web/Desktop composition always
sets it to `null`; its Light, Dark, and Custom choices become a complete Host-owned terminal theme
instead. There are no runtime partial setters. `updatePresentationConfiguration` validates and
isolates the whole candidate, awaits its regular and bold font descriptions, and then prepares
renderer measurements before publishing any field. Requests are latest-wins: superseded or
disposed preparation returns a typed cancellation, and validation, font, or renderer preparation
failure leaves the previous configuration visible.

One successful local commit replaces all ten fields, remeasures once, resets the glyph atlas once,
invalidates the whole surface, and schedules one current repaint. The same commit realigns the
hidden textarea and active IME preedit anchor, invalidates geometry-dependent link hover, repairs
local geometry gestures without introducing a copy effect, and computes at most one new resize
proposal with the current device-pixel ratio. Packed rows, Host selection, search, history,
accessibility rows and DOM selection, attachment identity, subscriptions, and backend remain in
place; the attachment and packed store are not reopened or reset. Host resize completion continues
asynchronously through the existing resize path. A post-commit render or resize failure therefore
retains the committed presentation and follows the existing typed recovery. A hidden or paused
surface retains the new configuration and draws it once when presentation resumes.

Runtime terminal colors and client overlays use a separate revision barrier. A composing client
calls `prepareThemePresentation` with one operation identity and one complete overlay snapshot
before requesting the Host mutation. While that operation is active, frames are acknowledged into
a private packed store instead of replacing the visible store. `waitForThemePresentation` becomes
ready only when the staged frame carries the exact Host-assigned `themeRevision`; a higher revision
fails the stage rather than guessing which snapshot it belongs to. After synchronous
`canCommitThemePresentation` preflight, `commitThemePresentation` swaps the staged store and
prepared renderer overlays/cursor in one client commit. Metadata listeners and drawing observe the
new revision only after that swap. Supersession, disposal, renderer preparation failure, or an
explicit abort discards or quarantines the unpublished stage and leaves the prior complete
presentation visible. Hidden, paused, and retained-history surfaces defer readiness, then expose
only the newest eligible complete revision when live presentation resumes.

Host-driven frame geometry, including terminal-originated 80/132-column transitions, updates the
packed store and renderer directly. Applying that snapshot never emits a reverse `ResizeTerminal`;
only a measured container or device-pixel-ratio change may request a new attachment resize.

Document visibility and explicit presentation demand jointly suspend renderer animation and queued
draw work. Hidden commits still advance and acknowledge the packed store; becoming visible schedules
one draw of the newest committed state rather than replaying stale destinations.

`freeze()` is the application recovery handoff: it retires input, transport-facing listeners,
animation, resize, search, and in-flight side effects while retaining the last painted canvas and
accessible row list. Frozen instances reject further frame and command work. A later `dispose()`
releases the retained renderer resources and DOM after the application has painted a replacement.

WebGPU initialization and recovery fence every asynchronous completion against that lifecycle:
retired work destroys its uncommitted device and resources and cannot configure a context, activate
Canvas fallback, or publish renderer health.

`notifyBrokerFailure` preserves the broker's typed code, semantic operation, retryability, optional
Host correlation ID, and optional client request sequence on `onError`. It also applies the owned
consequence: resize rejection re-admits unchanged geometry, history rejection completes or repairs
the exact row request, selection rejection releases gesture scheduling, a reflow-stale gesture
rejection is absorbed after that repair, and correlated copy or link failure completes only that
side effect. The package never turns retryability into automatic replay.

The renderer consumes packed rows directly. It never reconstructs a local VT state or a per-cell
object graph. After a draw completes, frame telemetry records the exact committed Host generation
and revision read from the visible packed store. Multiple commits may coalesce behind one animation
frame; that checkpoint names the newest commit actually drawn, never an earlier scheduled revision.

Committed live-frame metadata maps the optional close-activity extension to `idle`, `active`, or
`unknown`; absence, duplication, and malformed payloads resolve conservatively to `unknown`.
Historical row windows have no live process observation, so retained-history presentation keeps the
latest live store's close activity while using the history window for visual metadata and rows. An
accepted non-focus key, text, paste, mouse, or equivalent raw input immediately invalidates the
latest observation and synchronously publishes transient `unknown`; focus-only input and a
synchronously rejected publication do not. The override remains authoritative across retained
history, staged frames, snapshot requests, failures, and disposal. A newly arriving `idle` frame
may have entered flight before that input, so the client sends `NeedSnapshot(activity-stale)` for
its exact sequence instead of `FrameApplied` and retains `unknown`. The next complete snapshot is
causally newer than the input invalidation, but it proves neither that the Host accepted the input
nor that the shell consumed it. Under wire protocol `0.0.19`, Host-accepted non-focus input
invalidates the Host cache immediately, while a later current-state sample may report `active`,
`idle`, or `unknown`. A causally eligible replacement may therefore restore fresh `idle` in the
same terminal lifecycle. A stale theme-stage frame is fenced at apply time and cannot make the
stage ready; only a causally eligible staged frame clears the local override when its conservative
presentation actually commits.

Committed live frames may also carry the bounded Kitty graphics extension. A graphics snapshot
atomically replaces decoded in-memory image resources and placements; a placement-only update is
accepted only against the exact committed image revision. Invalid bytes or a missing resource
reference reject the whole terminal frame and request a new snapshot before text or graphics can
be partially committed. The renderer presents supported `z >= 0` placements on a
pointer-transparent overlay canvas, converts raw gray/gray-alpha/RGB/RGBA pixels once per image
generation, and reuses that raster for crop, scale, and movement. Empty snapshots, deletion,
reset, history presentation, and disposal clear both pixels and DOM resources. The package does
not parse PNG, load paths or URLs, persist images, or implement virtual, negative-z, glyph, or
animated graphics.

Row-window input is strictly decoded by default. An application adapter may set `projectionBufferTrust: 'local-host'` only when it owns an immutable, same-user, capability-scoped Terminal Host transport. That path retains the OBPB format-`0.0.2` buffer and creates typed-array views without rescanning or repacking every cell. Gateway, network, plugin, and persisted input must retain the default `external` behavior.

## History, search, selection, and links

Bottom-following use retains only the current packed grid. Entering history pauses live projection
and commits one bounded row window whose first row is the visible logical top, with limited forward
overscan. `RowsApplied` lets the Host pin that top; while the anchor remains valid, navigation and
reflow refreshes use the committed origin and treat the returned row-window start as authoritative.
When a newer reflow event supersedes an in-flight viewport read, its completion only settles that
old flight; the readable window remains visible and one event-driven refresh uses the newest
generation, revision, and committed pin without surfacing the obsolete failure.
Superseded background row reads likewise settle their bounded stale result without presenting a
user-visible history failure.
Clear, screen replacement, eviction, or any other invalid anchor resumes bottom projection while a
fresh snapshot replaces the last readable history window.

Wheel-originated history navigation presents each newly committed Host row window with one
client-owned, reversible canvas transaction. It snapshots the previously committed canvas before
the replacement draw, bounds displacement to one visible viewport, and converges to the canonical
canvas with the pinned 100-millisecond exponential motion. A new navigation intent cancels the old
transaction before the next committed direction begins. Reduced-motion preference, document or
projection suspension, presentation-geometry changes, unavailable capture or frame scheduling,
render failure, freeze, and disposal all remove the snapshot and leave the committed canvas at its
canonical position. Programmatic line/page/bottom navigation, scrollbar navigation, search reveal,
and selection autoscroll remain immediate. This presentation transaction neither predicts Host
rows nor changes history demand, row-window correlation, protocol state, or the public API.

Retained history exposes a client-owned DOM scrollbar overlay positioned against the complete terminal viewport rather than the fitted cell-grid canvas. Its rightmost 16 CSS pixels form the interaction area; an 8 CSS-pixel fully rounded thumb remains exactly 2 CSS pixels from the Panel-relative right edge, independent of unused grid remainder pixels, and retains 4 CSS pixels of vertical track padding plus a 20 CSS-pixel minimum height. History navigation and scrollbar interaction make it immediately visible, and inactivity removes its opacity after 1,500 milliseconds. Track presses jump proportionally, and captured scrollbar drags coalesce into bounded absolute row-window requests. The initiating primary pointer owns the drag until it ends; concurrent and secondary pointer events and wheel input are consumed without replacing its capture or anchor, and its synthesized release click is consumed once. Scrollbar ownership takes precedence over application mouse reporting, selection, and links, and is repaired on buttonless movement, cancellation, capture loss, focus or visibility loss, reset, and disposal. Navigation during a drag keeps the thumb fully visible, and the fade delay begins only after ownership ends or is repaired. The scrollbar overlay and canvas remain `aria-hidden` and expose no DOM scrollbar role; the existing keyboard and API history navigation remain the accessibility path.

Search is client-side. The first non-empty query lazily creates a Worker and streams bounded text-detail pages through a source-neutral row reader. One Worker page and one prefetched row request bound the pipeline; page credits refresh an inactivity watchdog, and the 16 MiB UTF-8 source limit publishes either one complete immutable result or no matches. Viewport, search, and general-buffer reads use independent single-flight lanes, so scrolling or link inspection cannot cancel a search page; request-sequence correlation permits their replies to settle in any order while bounding the client to three simultaneous row reads. Navigation requested during debounce or scanning remains queued across same-query revision invalidation. Decorations and result counts remain capped, while directional navigation independently scans the admitted retained search domain from the latest Host selection anchor and wraps without treating that decoration cap as the final match. The active match is reported separately, selected through a revision-pinned Host `set-range` command, and revealed even when it falls outside the decoration collection. Query text and matches never enter Terminal Host or an Electron broker.

```ts
terminal.scrollLines(-1);
terminal.updateSearch('failed', { caseSensitive: false, wholeWord: true });
terminal.findNext();
```

Selection endpoints and exported content remain authoritative in Terminal Host. The browser derives a one-, two-, or three-click sequence from a 500-millisecond interval and a one-cell surface-distance limit instead of trusting DOM click counts, then sends bounded surface position and cell geometry for Ghostty's word, line, rectangle, and 60% within-cell endpoint rules. A plain click does not synthesize a one-cell selection. Dragging beyond the viewport advances retained history through bounded row-window requests and continues the same Host gesture at the newly exposed row; document-level move and release fallbacks retain that behavior when pointer capture is unavailable. A modifier-assisted triple click selects Ghostty's semantic command output and dragging expands across output blocks. Only an ordinary primary-pointer release completes the gesture and requests copy by default. The serialized command queue retains the latest coalesced drag update ahead of that release even when the initial press reply is still in flight. Buttonless motion, pointer cancellation or capture loss, surface focus or document visibility loss, and presentation-geometry replacement cancel the pinned gesture without copying or clearing the latest Host selection. Serialized gesture replies make a normal release or cancellation produce exactly one copy consequence; construction-time `copyOnSelect: false` and runtime `setCopyOnSelect(false)` both opt out of normal-release copy. Every admitted automatic or manual copy publishes one source-tagged completion or failure through `onSelectionCopyResult` after the application-owned clipboard effect resolves. Programmatic range and semantic-output selection use absolute `bigint` rows. `adjustSelectionEndpoint` preserves the latest Host-published start endpoint and rectangle mode while replacing only the active end endpoint; it never reconstructs Ghostty text semantics in the client. Assistive-technology DOM ranges use the same explicit cell-range command, with UTF-16 offsets mapped across combining, wide, and surrogate-pair graphemes. Host selection is mirrored back into the visible-viewport accessibility tree when both endpoints are visible. When the document-global DOM range moves to another surface or disappears during sibling focus, this surface relinquishes only its accessibility mirror ownership; the attachment's Host selection and canvas overlay remain active. Surface or window focus loss relinquishes the same mirror ownership before the browser can collapse it; that collapse is presentation-only and cannot publish a Host clear command. An attachment constructed or dynamically downgraded with no input authority keeps its hidden textarea focusable but natively read-only. Shift-only page navigation, wheel history, search, selection, copy, and approved links remain available without Host input capability; defensive `beforeinput`, composition, paste, key, and mouse listeners own rejected browser editing and clear residual input state without emitting PTY input. An identity-checked natural-exit outcome permanently seals PTY input, clear, reset, and resize and makes the existing textarea read-only. The retained final frame and local output interactions remain available until disposal. The application performs the clipboard side effect and reports only its outcome.

A search-owned active-match selection is the DOM-mirroring exception: it remains authoritative in Terminal Host and visible on the canvas without occupying the document-global DOM range. This preserves keyboard focus in the application-owned find input while incremental results settle.

An identity-checked runtime-failure outcome applies the same terminal-control seal as natural exit:
PTY input, clear, reset, and resize become unavailable immediately while retained output, history,
search, selection, copy, and approved-link interaction remain readable.

Terminal-originated clipboard writes use a separate event path. `ProjectedTerminal` accepts only a
schema-validated request for its exact attachment, suppresses a duplicate or stale sequence, and
publishes the bounded text to the application without performing an effect. Disposal removes the
listener and prevents later delivery. The application must own the commit policy and platform
clipboard capability; selection copy does not implicitly commit an OSC 52 request.

Frame metadata exposes one strictly decoded current progress sample. The packed store commits
`remove`, `set`, `error`, `indeterminate`, or `pause` atomically with the frame and defaults an absent
extension to `remove`. Bell remains a separately sequenced attachment event. Neither progress nor
Bell changes terminal lifecycle, input availability, focus, or application notification policy.

Frame metadata also exposes the latest strictly decoded semantic command completion, when present:
terminal-local sequence, duration milliseconds, wall-clock completion milliseconds, and normalized
exit code. The packed store never reconstructs or retains command text. It only commits the Host
fact atomically; the application owns freshness, focus, deduplication, rate limiting, permission,
and any notification effect.

```ts
terminal.selectAll();
terminal.setSelectionRange({ column: 0, row: 12n }, { column: 7, row: 12n });
terminal.selectOutputAt({ column: 4, row: 12n });
terminal.adjustSelectionEndpoint({ column: 10, row: 12n });
const result = await terminal.copySelectionAsync();
if (result.kind === 'failed') reportCopyFailure(result.code);
```

Link discovery expands bounded generation/revision-pinned row reads until it has one complete
soft-wrapped logical line. The admitted line is capped by the protocol row, cell, and byte limits;
an over-limit line produces no partial detection. OSC 8 targets and OSC 133 cell/row semantics
arrive from Host-owned Ghostty state; this package never reparses their escape sequences. Explicit
links take precedence over detected URL and path text. Automatic URL detection admits only
`http`, `https`, and `mailto`; `normalizePermittedExternalTerminalUrl(...)` is the public policy
operation used again by Workbench Action and Browser effect adapters. Unsupported explicit OSC 8
metadata and detected filesystem targets may remain in the bounded link cache, but they never
receive pointer hover, product context, or an activation request. Content revisions, viewport
transitions, surface resize, device-pixel-ratio changes, and presentation geometry changes retire
the hover cache. A modifier-assisted click revalidates the hovered generation, revision, hover
epoch, and clicked cell before `ProjectedTerminal` emits one approved URL request; the application
decides whether and how to open it. Filesystem and unknown targets are not opened by this package.

Plain-text drops follow the same sanitization, review, ordering, and Host input path as paste. The
`file-drop` paste source follows the same hazard policy, but its source alone does not force review:
safe single-line text is published directly, while controls, embedded bracket terminators, and
unprotected multiline input retain the ordinary review rules. Resolving and safely preparing file
paths remains an application-owned capability, so this package receives no filesystem authority.

The off-screen accessibility mirror is a labeled `list` whose direct `listitem` rows preserve spaces, grapheme-to-column mappings, and absolute `aria-posinset`/`aria-setsize` positions. It labels rows and the cursor with caller-provided localized text. A separate polite status region announces only changed visible output, including localized deletion announcements. Retained-history inspection disables live updates without replacing the live-output baseline, so returning to the bottom announces only output changed since the last live frame.

## Bounded row API

The asynchronous row reader exposes only revision- and generation-pinned pages. It intentionally has no synchronous whole-buffer string API.

```ts
const snapshot = terminal.getBufferRowsSnapshot();
const rows = await terminal.readBufferRows(
	{
		detail: 'text',
		generation: snapshot.generation,
		maximumBytes: 2 * 1024 * 1024,
		revision: snapshot.revision,
		rowCount: 256,
		startRow: 0n,
	},
	new AbortController().signal,
);
```

## Renderer behavior

WebGPU is preferred when available; Canvas 2D is the observable fallback. Both paths consume the
same packed projection, font configuration, application-owned selection/search overlays, preedit
state, and cursor metadata. Host frame metadata remains authoritative and unmodified. A non-null
client `defaultColorOverride` changes only cells whose packed foreground or background token is
`TerminalColorEncoding.default`; explicit RGB (including black), Host-projected palette RGB, and
`none` retain their Host-projected behavior. Inverse swaps already-resolved display roles, a
default underline uses the effective foreground, and the cursor always uses the Host cursor color
(falling back to Host foreground). An explicit client cursor accent affects only text inside a
block cursor. This override is outside the revisioned Host-theme path and is always `null` in the
shared Orchbee Web/Desktop composition. `allowTransparency` alone controls transparent renderer clear; a composing client
uses `backgroundOpacity` for its effective default-background root and never applies whole-surface
CSS opacity. Canvas remains transparent before the first frame, then reuses the latest effective
default background for opaque fills and resize. Packed graphemes preserve CJK, combining, and emoji
cell widths; explicit foreground/background and underline colors, inverse, bold, faint, italic,
invisible, blink, underline variants, strikethrough, and overline use the projected Ghostty styles.
Renderer health reports initialization, fallback, device loss, recovery, and recovery failure.
The WebGPU path retains and clears one instance array plus one geometrically grown `Float32Array`
upload buffer for each background, text, and overlay stream. Once a stream reaches the capacity of
its largest prepared frame, later frames at or below that size reuse the same underlying CPU
storage instead of allocating another frame-sized buffer. Disposal releases the retained staging
storage. Per-cell foreground/background resolution also reuses one renderer-owned scratch
projection while callers copy only values that must outlive the current cell. These rules bound the
two full-viewport allocation hot paths by peak prepared viewport content; they do not define a
GPU-driver, browser, or whole-process RSS ceiling.

Dirty glyph-atlas rectangles transfer directly from the Canvas atlas into the WebGPU texture, so
incremental uploads do not materialize an intermediate CPU pixel array. Frame telemetry maintains
atlas occupancy when glyphs are admitted or reset instead of rescanning the glyph map. The hidden
IME textarea reuses the Canvas-to-surface offset between geometry changes and remeasures it after
resize, device-pixel-ratio, or presentation-configuration changes.

WebGPU bind groups for the uniform and atlas resources persist until their owning device or atlas
is recreated instead of being rebuilt for every submitted frame. A packed row view also caches
decoded non-empty graphemes for the lifetime of its current backing, allowing accessibility and
rendering to share one UTF-8 decode; replacing the row backing clears that cache. The cache remains
bounded by the cells in the currently retained grid.

WebGPU shaping runs are split at a terminal-column capacity derived from the device texture limit,
device-pixel ratio, cell advance, atlas padding, and an ink-overhang reserve. Normal runs retain
their complete shaping boundaries; a row that exceeds one atlas entry is emitted as ordered bounded
runs so the device limit cannot silently remove valid projected text. Canvas shaping remains
unbounded by WebGPU texture capacity.

Font measurement samples the selected browser face at a large CSS size to recover its unrounded
advance, ascent, descent, and cap-height ratios. Grid width and height retain stable CSS geometry,
while the baseline is rounded at the current physical device-pixel ratio before both backends
rasterize it. This prevents CSS-first rounding from shifting high-DPI glyphs by one physical pixel
relative to Ghostty.

Every Canvas font context requests `geometricPrecision` text rendering, including the WebGPU glyph
atlas, its replacement after growth, and the Canvas fallback. This suppresses Chromium's
small-size legibility thickening and matches Ghostty's unthickened CoreText coverage without
changing the selected variable-font outline or configured weight.

Browser shaping runs contain only single-column Host cells. Wide CJK, emoji, and other fallback
graphemes rasterize once from their authoritative cell head instead of inheriting the fallback
face's natural run advance, while continuation cells cannot absorb a following combining cluster.
Within each narrow-cell run, Canvas letter spacing fills the difference between the face's
fractional natural advance and the rounded terminal-cell width. This preserves complex shaping and
programming ligatures without cumulative grid drift when terminal style boundaries split a row.
WebGPU atlas entries keep their textured quads on physical-pixel boundaries while preserving the
horizontal subpixel crop remainder inside the raster. A cursor shaping boundary can therefore
change the run's first ink bearing without moving its shared cell baseline; only intentional
ligature reshaping changes glyph geometry.
Base foreground changes and selection/search overlays reuse the original shaped run, clip it at cell
boundaries, and recolor only the covered portions. Moving an animated foreground boundary therefore
cannot change the glyph raster or its origin; font, style, underline, and color-glyph classification
remain shaping boundaries.

Ghostty remains the reference for selected font metrics, shaping boundaries, complete Unicode sprite-face routing, cursor geometry, and terminal-panel rendering behavior. Deterministic generated data supplies every pinned sprite from Ghostty's four padded odd/even metric reference rasters, while selected sprites scale procedurally between those reference geometries. That reference does not make this package a Ghostty VT owner. Maintenance provenance is recorded in [`docs/ghostty-surface-test-provenance.md`](docs/ghostty-surface-test-provenance.md).

Search behavior follows the bounded contract in [`docs/vscode-search-alignment.md`](docs/vscode-search-alignment.md). The application owns the find surface, query history, shortcuts, and localized labels.

### Cursor blink presentation

Host cursor metadata remains authoritative for the cursor coordinate, visibility, shape, and
whether blinking is enabled. The renderer owns only the local blink phase and its 530-millisecond
timer. A committed coordinate change immediately presents an enabled blinking cursor in its
visible phase and restarts that timer; repeated changes inside one interval therefore keep the
cursor visible. An unchanged coordinate never restarts the timer, so the cursor resumes alternating
between visible and hidden after one idle interval. Disabling blinking stops the timer and leaves
the cursor visible. Suspending the renderer stops animation in the visible phase, and resuming
starts a fresh interval. The unfocused presentation remains a non-blinking hollow block.

This presentation observes committed Host coordinates rather than keyboard intent: input that does
not move the cursor has no effect, while a cursor move caused by terminal output has the same reset
behavior. It never rewrites Host metadata or introduces a second cursor coordinate. Deterministic
renderer tests control the timer and cover hidden-to-moving visibility plus the first idle toggle;
Host/profile cursor-mode evidence remains separate.

### Cursor-default and mounted-start evidence

The exact `terminal-parity/SU-13-cursor-defaults` witness projects a real Terminal Host/PTY profile
through the production surface. It observes the explicit block/non-blinking default, a
blinking-bar DECSCUSR override, and RIS restoration in committed frame metadata. Deterministic
Canvas and WebGPU owner tests separately prove that the same metadata selects the corresponding
cursor geometry and blink scheduler; the system witness does not use a platform screenshot as a
second cursor authority. Host runtime evidence additionally covers application Reset and an
explicit non-default profile.

`measureMountedTerminalGeometry(...)` loads the selected font and applies the same grid and physical
cell rules as the first projected resize. The POSIX-only exact
`terminal-parity/SU-16-startup-geometry` witness independently derives that mounted grid from the
real Electron surface, records the child PTY's initial `stty size` before the shell starts, and
requires the sole first client frame to carry the same columns, rows, and physical cell dimensions
without an intermediate `80x24` frame. Desktop and Host owner tests cover deferred identity without
a child, start-before-attach ordering, failed/duplicate/conflicting start, and Core synchronization;
the headed witness does not claim Windows ConPTY or later-resize behavior.

### Live input-capability evidence

The exact headed `terminal-parity/IN-10-read-only` witness drives a monotonic downgrade and upgrade
through a real Canvas 2D projected surface. It observes immediate composition, native-settlement,
paste-review, and textarea repair; continued local search, selection, copy, and history; no stale
input publication; and only fresh input at the upgraded revision. The fixture controls the
application delivery stream, so Terminal Host owner tests separately prove authoritative stale and
downgraded PTY rejection, per-attachment repair ordering, and no queued replay. It does not claim a
renderer-owned capability upgrade or native browser permission policy.

### Terminal Action context evidence

`ProjectedTerminal.captureActionContext(...)` is the source-neutral boundary for a containing
product menu. It returns `terminal-owned` while application mouse tracking owns the surface unless
Shift is held. Otherwise it exposes only a permitted URL target at the requested cell together with
the exact history generation, history revision, link epoch, absolute row, and column captured at
that moment. `linkOpenRequestIsCurrent(...)` reparses those canonical values, requires the complete
revision tuple to remain current, resolves the same cached cell again, and compares the exact URL
before an application may admit an effect. It does not open links or own an Action runtime.

The headed witnesses compose a real Canvas 2D `ProjectedTerminal` into one application-owned
Workbench Action scope. `terminal-parity/IN-09-action-scope` checks available, unavailable, and
unbound Apple Command handling. `terminal-parity/IN-11-context` checks application mouse ownership,
the bounded pinned input, revision invalidation, and zero external effects. `terminal-parity/IN-12-actions`
checks the complete minimal catalog, synchronous availability, typed results, no new confirmation,
and localized perceivable feedback. `terminal-parity/FS-05-file-links` checks that explicit file
metadata, detected paths, and a direct Browser-effect request remain inactive.
`terminal-parity/FS-06-url-policy` checks the same real surface's hover, context, and direct
activation path for exactly `http`, `https`, and `mailto`, including unsupported automatic and
explicit URL cases. Owner-local unit tests additionally cover malformed context, exact-input
copying, normalization, automatic-detection filtering, and revalidation immediately before
execution.

`terminal-parity/HS-07-selection-export` sends SGR-styled output through a real PTY and Terminal
Host, selects the styled word through the production Canvas surface, and observes that Electron's
clipboard receives exactly the visible plain text. The terminal protocol's generated TypeScript/Zig
conformance corpus separately proves that `plain` is the only admitted request and response value
and that HTML and raw VT requests fail before Host runtime access. The strict projected-client
command exposes no format field. The headed witness runs the current macOS Electron clipboard path;
it does not claim another operating system's native clipboard format registry.

### Shared focus evidence

The stable headed `terminal-parity/IN-14-shared-focus` trace opens two real `ProjectedTerminal`
Canvas 2D surfaces with distinct attachments bound to one terminal identity. Both publish active
projection demand and independent focus claims. Releasing the first claim leaves one active claim;
pausing the second attachment clears the aggregate, and reactivation publishes no implicit claim.
The fixture's small observation reducer checks only the application command trace. Terminal Host's
native two-attachment real-PTY test independently proves the authoritative aggregate emits one
`FocusIn`/`FocusOut` interval.

| IN-14 claim                                                                                                                | Projected integration                                                                  | Host integration                                                                                                          | Headed system trace                  | Limits                                                                                                                                                        |
| -------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- | ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Distinct visible attachments claim focus independently while the terminal observes only aggregate zero/nonzero transitions | `ProjectedTerminal` focus, blur, and projection-demand commands from two live surfaces | `two visible attachments produce one real PTY focus interval` plus stale, detach, downgrade, and repair-order regressions | `terminal-parity/IN-14-shared-focus` | Headed Chromium Canvas 2D with synthetic application-layer focus events; no OS multi-window focus, WebGPU, cross-browser, or native-browser-throttling claim. |

### Smooth retained-history evidence

The stable headed `terminal-parity/SU-11-smooth-scroll` trace drives wheel history through a
real Desktop attachment and Terminal Host, controls Chromium animation time, and observes the
application-layer canvas transaction. It proves bounded upward motion, progress toward the
committed canvas, cancellation followed by downward recovery, and visibility cancellation without
stale resumption. The owner-local transaction tests cover both directions, settlement, reduced
motion, invalid geometry, and over-viewport rejection; the `ProjectedTerminal` regression covers
row-window correlation and recovery integration.

| SU-11 claim                                                                | Owner-local transaction evidence                                                                                                                            | Projected integration                                                                                                                                                    | Headed system trace                   | Limits                                                                                                                                      |
| -------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| Wheel history motion is bounded, reversible, and cancelled while suspended | `projects bounded motion in both directions and settles on the committed canvas`; `keeps reduced, invalid, and over-viewport motion on the canonical frame` | `smooths wheel history motion, reverses on a committed target, and cancels when hidden`; `keeps a later programmatic history intent immediate after queued wheel motion` | `terminal-parity/SU-11-smooth-scroll` | Desktop Chromium main-text canvas and synthetic application-layer visibility; no OS throttling, WebGPU snapshot, or graphics-history claim. |

### Visibility and projection-demand evidence

The stable headed `terminal-parity/SU-15-visibility` trace draws an initial frame through a real
`ProjectedTerminal` and Chromium Canvas 2D surface, then combines document visibility with explicit
projection demand. While both causes are paused, it proves a queued draw and blinking animation stay
suspended as multiple revisions commit and emit `FrameApplied`; clearing visibility alone remains
suspended, and clearing demand schedules exactly one draw of the newest generation and revision.
The trace also verifies a second draw is not queued and post-disposal visibility, demand, frame, and
animation activity is inert.

The headed fixture overrides `document.visibilityState` only inside its test document and dispatches
the real application-layer `visibilitychange` event. It does not exercise operating-system tab
backgrounding or browser throttling policy, and it makes no WebGPU or cross-browser claim.

| SU-15 claim                                                                                                                                       | Renderer evidence                                      | Projected integration                                                                                | Headed system trace                | Limits                                                                                                                                       |
| ------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------ | ---------------------------------------------------------------------------------------------------- | ---------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| Visibility and explicit demand retain independent pause causes; commits and acknowledgements continue while draws coalesce until the final resume | `suspends packed blinking-cell animation while hidden` | `coordinates document visibility and projection demand before drawing the newest acknowledged frame` | `terminal-parity/SU-15-visibility` | Headed Chromium Canvas 2D with synthetic application-layer document visibility; no OS background-throttling, WebGPU, or other-browser claim. |

### Device-pixel-ratio evidence

The stable headed `terminal-parity/SU-08-dpr` trace commits and draws a frame through a real
`ProjectedTerminal` and Chromium Canvas 2D surface before changing device scale from `1` to `1.5`.
It checks unchanged logical and CSS geometry, current-DPR Canvas backing dimensions, raster-atlas
invalidation and current-frame rerasterization, matching resize cell and derived terminal pixel
dimensions, the hidden input anchor, resolution-query re-arming, and inert post-disposal changes.
The trace uses Chromium DevTools device metrics plus a one-pixel viewport-width change so the real
browser `resize` listener observes the new ratio. This is a controlled synthetic browser transition,
not evidence of native monitor transfer, operating-system scaling UI, or another browser engine.

| SU-08 claim                                                                             | Renderer evidence                                                                                                                                                     | Projected integration                                                                                                                                                           | Headed system trace         | Limits                                                                                                                       |
| --------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| DPR changes rebuild surface geometry, raster state, and PTY pixel dimensions coherently | `remeasures and fully repaints Canvas geometry at a fractional device-pixel ratio`; `reconfigures WebGPU and recreates its atlas for a clean device-pixel transition` | `repaints a committed frame and updates PTY and input geometry for fractional DPR changes`; `suspends hidden rendering and resumes with one draw of the newest committed frame` | `terminal-parity/SU-08-dpr` | Selected `1` to `1.5` Canvas browser trace; deterministic Canvas and WebGPU backend tests; no native monitor-transfer claim. |

## Development

Run from the repository root:

```sh
pnpm --filter @orchbee/terminal typecheck
pnpm --filter @orchbee/terminal build
pnpm --filter @orchbee/terminal test
pnpm --filter @orchbee/terminal test:tla
pnpm --filter @orchbee/terminal test:exports
pnpm verify
```

`test:tla` checks the package-owned runtime-presentation and input-repair models documented in
[`formal/README.md`](formal/README.md). They are change-triggered correctness evidence and are not
part of the ordinary package test command.

`pnpm audit:ghostty-panel` compares the maintained renderer/font provenance and generated data with the pinned Ghostty source when `../ghostty` or `GHOSTTY_SOURCE` is available. Run `pnpm generate:ghostty-panel-data` only when the pinned Nerd Font constraints, sprite ranges, or sprite reference rasters change.
