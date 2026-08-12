# @orchbee/terminal

Source-neutral browser client surface for a terminal whose authoritative VT state lives in Orchbee Terminal Host.

## Scope

This package owns one projected terminal surface:

- atomic packed-frame commit and snapshot recovery;
- WebGPU rendering with a Canvas 2D fallback;
- bounded history-window presentation;
- client-side search and bounded logical-line link discovery;
- Host-owned selection interaction and copy correlation;
- keyboard, committed text, IME, mouse, focus, paste, resize, clear, and reset intents; and
- visible-viewport accessibility.

It never receives or parses PTY output, loads a VT engine, or exposes `write(ptyBytes)`. Terminal Host owns the PTY, Ghostty VT state, terminal modes, retained history, selection endpoints, and mode-dependent input encoding. The application owns transport, terminal inventory, product topology, clipboard effects, external navigation, and localized product UI. The machine-readable scope contract lives in [`project.json`](project.json).

The accepted [projected-terminal experience compatibility proposal](../../proposals/terminal/projected-terminal-experience-compatibility.md)
defines the remaining cross-owner recovery contract; it is not current runtime authority.

`clear()` and `reset()` are semantic Host requests. `clear()` erases the active screen and homes its
cursor while preserving terminal modes. `reset()` first repairs client-owned composition, pressed
input, selection, search, hover, and history-viewport state, then asks the Host to perform Ghostty's
full reset. Neither method injects terminal escape bytes or reuses projection `store-reset`
recovery.

Input repair keeps native composition lifetime separate from local commit eligibility. An active
composition may be eligible, or it may be cancelled while the browser still owes a terminal
`compositionend`. Reset, textarea blur, window blur, hidden-document transition, and disposal
clear the textarea and renderer preedit immediately, revoke commit eligibility, and clear key,
composition, `beforeinput`, paste, and mouse correlation records. Late composition updates and
either late `beforeinput`/`compositionend` order cannot republish preedit or text; the late end only
settles the cancelled native work. A new `compositionstart` supersedes cancelled work even when its
old `compositionend` never arrives. Refocus, a non-composing key, or standalone non-composition
`beforeinput` also restores local progress without waiting for that end. In an uncancelled
composition, both browser orders (`beforeinput` then `compositionend`, or the reverse) commit the
text at most once, and the state transition is complete before the synchronous command-port send.

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
Synthetic Enter, Backspace, Delete, and alternate-screen arrow input use the same held publication
path as matched press/release taps. They retain no steady-state client ownership, while a transport
loss between the pair remains visible to the Host attachment ledger for detach repair.

Shift-only Page Up and Page Down, including their keypad codes, remain local retained-history
navigation and never become Host key input. Every admitted user-input path shares one ordered
surface transition: it first requests the live bottom when history is visible, then clears an active
Host selection, and only then publishes the semantic key, text, approved paste, or application-mouse
input. Composition activation performs the same transition before presenting preedit. Focus reports,
lifecycle and pointer-gesture repair, rejected or aborted paste review, and local history navigation
preserve the current history and selection surface.

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
If the projected client disappears before this cleanup completes, Terminal Host releases only that
attachment's accepted key and application-mouse ownership before revoking its capability; another
attachment is unaffected.

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

Opening waits for the configured face (including representative Latin, CJK, and emoji glyphs), then installs frame, renderer, input, resize, and accessibility handling before activating projection demand. Each open attempt transactionally owns its canvas, input, accessibility nodes, renderer, observers, and listeners: setup failure removes only those resources and returns the same instance to `constructed` for retry; duplicate opens reject without changing the current surface, and disposal remains terminal. The source-neutral default face is browser-generic `monospace`; an application may provide a provisioned product face, but the shared Desktop/Browser composition does not silently substitute a system-only font. Surface sizing subtracts computed padding from the available grid, retains the mature two-column minimum, and proposes no PTY geometry when padding consumes either content dimension. Padding balance remains opt-in. A measured resize is sent before active projection demand and is deduplicated only while it agrees with the latest Host-committed geometry or remains inside the short observer re-entry window. A Host geometry change, broker rejection, transport failure, or later unchanged observation makes the proposal eligible for retry. The hidden IME control uses the same font and follows the projected cursor so the native candidate window stays aligned. Device-pixel-ratio changes rebuild renderer geometry and terminal pixel dimensions instead of leaving a stale atlas. A frame commits into a bounded row-partitioned structure-of-arrays store and is acknowledged only after draw scheduling. `FrameApplied` acknowledges the client commit, not completed painting. Exact duplicates are re-acknowledged; invalid identity, sequence, generation, dimension, or delta-base input requests a complete snapshot without mutating the committed grid.

An open attachment accepts one complete `TerminalPresentationConfiguration` transaction containing
`fontFamily`, `fontSize`, `fontWeight`, `fontWeightBold`, `fontLigatures`,
`fontMetricModifiers`, `paddingBalance`, and `allowTransparency`. There are no runtime partial
setters. `updatePresentationConfiguration` validates and isolates the whole candidate, awaits its
regular and bold font descriptions, and then prepares renderer measurements before publishing any
field. Requests are latest-wins: superseded or disposed preparation returns a typed cancellation,
and validation, font, or renderer preparation failure leaves the previous configuration visible.

One successful local commit replaces all eight fields, remeasures once, resets the glyph atlas
once, invalidates the whole surface, and schedules one current repaint. The same commit realigns
the hidden textarea and active IME preedit anchor, invalidates geometry-dependent link hover,
repairs local geometry gestures without introducing a copy effect, and computes at most one new
resize proposal with the current device-pixel ratio. Packed rows, Host selection, search,
history, accessibility rows and DOM selection, attachment identity, subscriptions, and backend
remain in place; the attachment and packed store are not reopened or reset. Host resize completion
continues asynchronously through the existing resize path. A post-commit render or resize failure
therefore retains the committed presentation and follows the existing typed recovery. A hidden or
paused surface retains the new configuration and draws it once when presentation resumes.

Host-driven frame geometry, including terminal-originated 80/132-column transitions, updates the
packed store and renderer directly. Applying that snapshot never emits a reverse `ResizeTerminal`;
only a measured container or device-pixel-ratio change may request a new attachment resize.

Document visibility and explicit presentation demand jointly suspend renderer animation and queued
draw work. Hidden commits still advance and acknowledge the packed store; becoming visible schedules
one draw of the newest committed state rather than replaying stale destinations.

The renderer consumes packed rows directly. It never reconstructs a local VT state or a per-cell
object graph. After a draw completes, frame telemetry records the exact committed Host generation
and revision read from the visible packed store. Multiple commits may coalesce behind one animation
frame; that checkpoint names the newest commit actually drawn, never an earlier scheduled revision.

Row-window input is strictly decoded by default. An application adapter may set `projectionBufferTrust: 'local-host'` only when it owns an immutable, same-user, capability-scoped Terminal Host transport. That path retains the OBPB format-`0.0.1` buffer and creates typed-array views without rescanning or repacking every cell. Gateway, network, plugin, and persisted input must retain the default `external` behavior.

## History, search, selection, and links

Bottom-following use retains only the current packed grid. Entering history pauses live projection
and commits one bounded row window whose first row is the visible logical top, with limited forward
overscan. `RowsApplied` lets the Host pin that top; while the anchor remains valid, navigation and
reflow refreshes use the committed origin and treat the returned row-window start as authoritative.
Clear, screen replacement, eviction, or any other invalid anchor resumes bottom projection while a
fresh snapshot replaces the last readable history window.

Retained history exposes a client-owned in-canvas scrollbar overlay. Its rightmost 16 CSS pixels form the interaction area; the shared Canvas 2D and WebGPU projection draws only an 8 CSS-pixel thumb inset by 4 CSS pixels, with a 20 CSS-pixel minimum height. History navigation and scrollbar interaction make it immediately visible, and inactivity removes its opacity after 1,500 milliseconds. Track presses jump proportionally, and captured scrollbar drags coalesce into bounded absolute row-window requests. The initiating primary pointer owns the drag until it ends; concurrent and secondary pointer events and wheel input are consumed without replacing its capture or anchor, and its synthesized release click is consumed once. Scrollbar ownership takes precedence over application mouse reporting, selection, and links, and is repaired on buttonless movement, cancellation, capture loss, focus or visibility loss, reset, and disposal. Navigation during a drag keeps the thumb fully visible, and the fade delay begins only after ownership ends or is repaired. The canvas remains `aria-hidden` and exposes no DOM scrollbar role; the existing keyboard and API history navigation remain the accessibility path.

Search is client-side. The first non-empty query lazily creates a Worker and streams bounded text-detail pages through a source-neutral row reader. One Worker page and one prefetched row request bound the pipeline; page credits refresh an inactivity watchdog, and the 16 MiB UTF-8 source limit publishes either one complete immutable result or no matches. Navigation requested during debounce or scanning remains queued across same-query revision invalidation. Decorations and result counts remain capped, while directional navigation independently scans the admitted retained search domain from the latest Host selection anchor and wraps without treating that decoration cap as the final match. The active match is reported separately, selected through a revision-pinned Host `set-range` command, and revealed even when it falls outside the decoration collection. Query text and matches never enter Terminal Host or an Electron broker.

```ts
terminal.scrollLines(-1);
terminal.updateSearch('failed', { caseSensitive: false, wholeWord: true });
terminal.findNext();
```

Selection endpoints and exported content remain authoritative in Terminal Host. The browser derives a one-, two-, or three-click sequence from a 500-millisecond interval and a one-cell surface-distance limit instead of trusting DOM click counts, then sends bounded surface position and cell geometry for Ghostty's word, line, rectangle, and 60% within-cell endpoint rules. A plain click does not synthesize a one-cell selection. Dragging beyond the viewport advances retained history through bounded row-window requests and continues the same Host gesture at the newly exposed row; document-level move and release fallbacks retain that behavior when pointer capture is unavailable. A modifier-assisted triple click selects Ghostty's semantic command output and dragging expands across output blocks. Only an ordinary primary-pointer release completes the gesture and requests copy by default. Buttonless motion, pointer cancellation or capture loss, surface focus or document visibility loss, and presentation-geometry replacement cancel the pinned gesture without copying or clearing the latest Host selection. Serialized gesture replies make a normal release or cancellation produce exactly one copy consequence; construction-time `copyOnSelect: false` and runtime `setCopyOnSelect(false)` both opt out of normal-release copy. Every admitted automatic or manual copy publishes one source-tagged completion or failure through `onSelectionCopyResult` after the application-owned clipboard effect resolves. Programmatic range and semantic-output selection use absolute `bigint` rows. `adjustSelectionEndpoint` preserves the latest Host-published start endpoint and rectangle mode while replacing only the active end endpoint; it never reconstructs Ghostty text semantics in the client. Assistive-technology DOM ranges use the same explicit cell-range command, with UTF-16 offsets mapped across combining, wide, and surrogate-pair graphemes. Host selection is mirrored back into the visible-viewport accessibility tree when both endpoints are visible. An attachment constructed with `inputEnabled: false` keeps its hidden textarea focusable but natively read-only. Shift-only page navigation, wheel history, and local selection remain available without Host input capability; defensive `beforeinput`, composition, and paste listeners own rejected browser editing and clear any residual textarea or preedit state without emitting PTY input. Input capability is fixed for the projected-terminal instance; live capability mutation is not supported. The application performs the clipboard side effect and reports only its outcome.

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
links take precedence over detected URL and path text. Content revisions, viewport transitions,
surface resize, device-pixel-ratio changes, and presentation geometry changes retire the hover
cache. A modifier-assisted click revalidates the hovered generation, revision, hover epoch, and
clicked cell before `ProjectedTerminal` emits one approved URL request; the application decides
whether and how to open it. Filesystem and unknown targets are not opened by this package.

Plain-text drops follow the same sanitization, review, ordering, and Host input path as paste. File drops are intentionally left to an application-owned capability that can resolve paths without exposing filesystem authority to the projected client.

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

WebGPU is preferred when available; Canvas 2D is the observable fallback. Both paths consume the same packed projection, font configuration, application-owned selection/search overlays, preedit state, and cursor metadata. Host frame metadata is the only terminal default-color authority: projected cells already contain resolved RGB values, the Host cursor color falls back to the Host foreground, and an explicit client cursor accent affects only text inside a block cursor. Canvas remains transparent before the first frame, then reuses the latest Host background for complete fills and resize. Packed graphemes preserve CJK, combining, and emoji cell widths; explicit foreground/background and underline colors, inverse, bold, faint, italic, invisible, blink, underline variants, strikethrough, and overline use the projected Ghostty styles. Renderer health reports initialization, fallback, device loss, recovery, and recovery failure.

Ghostty remains the reference for selected font metrics, shaping boundaries, complete Unicode sprite-face routing, cursor geometry, and terminal-panel rendering behavior. Deterministic generated data supplies every pinned sprite from Ghostty's four padded odd/even metric reference rasters, while selected sprites scale procedurally between those reference geometries. That reference does not make this package a Ghostty VT owner. Maintenance provenance is recorded in [`docs/ghostty-surface-test-provenance.md`](docs/ghostty-surface-test-provenance.md).

Search behavior follows the bounded contract in [`docs/vscode-search-alignment.md`](docs/vscode-search-alignment.md). The application owns the find surface, query history, shortcuts, and localized labels.

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
