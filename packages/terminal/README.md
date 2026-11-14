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

Cross-owner runtime authority remains in the Terminal Host contract and the owning application and
protocol guides. This guide retains the source-neutral behavior and stable compatibility-evidence
index without overriding those owners.

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
Printable punctuation and digit keydowns with no modifier other than optional Shift also yield to
the native textarea so third-party macOS input methods can publish their exact Unicode result. Shift
letters and Space remain on the physical-key path; every Control, Meta, or Alt chord preserves its
existing shortcut or physical-key behavior outside native-text settlement. The adapter never maps
punctuation itself. If no native value arrives within 30 milliseconds, the
`KeyboardEvent.key` text is sent as a deterministic committed-text fallback; this fallback does not
preserve physical key identity, including for Shift-produced punctuation. Native settlement
participates in the same ordered input dispatcher: an intervening key, paste, or pointer input seals
the current native batch before that later input is admitted, so deferred digits and punctuation
cannot move past subsequent physical keys. The same short-lived settlement marker handles browser
event orders where the native `input` arrives before `keydown`. The bounded native-settlement model
compares the actual textarea value with its captured baseline across two requests, two repair-epoch
advances, and two value transitions, including a changed-then-restored ABA order. An independent
beforeinput observed during that window is suppressed and receives no separate later publication;
the current timer owns the sole native, fallback, or no-op outcome. A cancelled timer identity and
epoch are inert after repair and re-admission. Empty or over-byte-limit sealed values and concrete
keyboard-admission partitions remain deterministic adapter evidence.

On macOS, the projected terminal also consumes a source-neutral Option-as-Alt mode: `off`, `left`,
`right`, or `both`. It tracks the physical Option side from `AltLeft` and `AltRight`, resolves one
boolean semantic intent per admitted key event, and updates that mode live without recreating the
terminal. It never prepends Escape to committed text. Terminal Host consumes the event intent and
continues to own the actual Ghostty keyboard encoding for the current terminal modes.

The complete source-neutral presentation configuration includes font family, integer font size,
regular and bold CSS weights, ligature enablement, uniform terminal padding from `0` through `32`
CSS pixels, color overrides, and background opacity. The current Settings source admits
integer-hundred weights from `100` through `900`; its defaults use weights `400` and `700`, ligatures
enabled, and zero padding. A complete live update changes the mounted renderer and recomputes usable
terminal geometry without replacing its Host attachment. Uniform padding is additional to the
mounted surface's measured CSS border and padding.

The separate source-neutral interaction configuration contains `copyOnSelect`, defaults it to
`false`, and updates an installed terminal through `setCopyOnSelect` without recreating the surface.
Presentation and interaction sources have independent lifecycles and persistence owners.

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
observed but inert. A newly opened generation progresses independently after repair. Its finite
queue abstraction covers one representative retained item plus focus-in before key; arbitrary queue
depth and queued text payloads remain OrderedInputDispatcher and ProjectedTerminal test evidence.

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
Buttonless hover in a non-pixel mouse format is published at most once for the same clamped terminal
cell, applying Ghostty's terminal-cell coalescing rule before the Host queues input. SGR pixel hover preserves
every finite position; drag, press, release, and wheel input remain ordered and uncoalesced.
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
On macOS, Shift modifier down and up events that carry no Alt, Control, or Meta modifier remain
published as Host key ownership but leave the Browser default uncancelled, allowing the system or
a third-party input method to decide whether a Shift tap changes input mode. Before either event
continues into native input handling, the surface remeasures and commits the hidden-input anchor;
other platform and modifier ownership is unchanged.
Synthetic Enter, Backspace, Delete, and alternate-screen arrow input use the same held publication
path as matched press/release taps. They retain no steady-state client ownership, while a transport
loss between the pair remains visible to the Host attachment ledger for detach repair.

Shift-only Page Up and Page Down, including their keypad codes, remain local retained-history
navigation and never become Host key input. Every admitted input-producing path except buttonless
application-mouse motion shares one ordered surface transition: it first requests the live bottom
when history is visible, then clears an active Host selection, and only then publishes the semantic
key press or repeat, text, approved paste, or application-mouse input. An eligible buttonless motion
report admitted by any-event mouse tracking is published to the Host without changing the retained
history position and active Host selection. Physical modifier-only presses and repeats likewise
preserve that surface while maintaining Host key ownership, so an application shortcut can resolve
without first dismissing retained history; the first unclaimed non-modifier input still performs the
shared transition. Composition activation performs the transition before presenting preedit. A
physical or synthetic key release closes existing Host input ownership without starting that
transition, so a selection established between its press and release remains visible. Focus reports,
lifecycle and pointer-gesture repair, rejected or aborted paste review, and local history navigation
preserve the current history and selection surface.

`terminal-parity/IN-07-pre-input` drives a real buttonless `PointerEvent` through the headed Canvas
surface while retained history and a Host selection are active. It requires one application motion
report, no return-to-bottom or selection-clear command, and a preserved readable selection. The
same trace then creates an assistive-technology-origin Range over that Host selection before it
synthesizes window focus loss and collapses the Range; no Host selection-clear command is published
and the readable Host selection remains active. Owner-local regressions check both repairs while
retaining the existing button-press transition requirement.

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
focus-in. The application decides when to call `focus()`; an open surface synchronizes its local
padding, scrollbar, hidden-input anchor, and current resize proposal before moving native focus to
the hidden textarea. Opening the package surface does not claim initial product focus.
Desktop and pen pointer activation focuses the hidden textarea on pointer down. TerminalInputRepair
proves key-before-mouse-before-focus cleanup for one representative key and one representative mouse
button; plural identity completeness and within-class press order remain deterministic
ProjectedTerminal evidence.

### Mobile touch and native-input model

| Field                      | Contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Problem and scope          | A coarse touch pointer must operate a projected terminal without inheriting desktop selection timing or relying on a hardware keyboard. Mouse, pen, touchpad, hardware-keyboard, multi-touch, pinch-zoom, and native selection-handle behavior are unchanged or excluded.                                                                                                                                                                                                                                                                        |
| Inputs                     | The adapter normalizes one primary `pointerType === 'touch'` stream, finite CSS-pixel coordinates, current cell geometry, projected application-mouse ownership, active-screen mode, input capability, `touchend`, native input/composition events, and lifecycle cancellation. Missing geometry, stale pointer identity, additional touches, and closed input admission are intentional no-ops or cancellation.                                                                                                                                 |
| State and model            | `TerminalTouchGestureState` is `idle`, `pending-tap`, or `scrolling` and is owned by one mounted `ProjectedTerminal`. `transitionTerminalTouchGesture(...)` is the deterministic owner of activation, the eight-CSS-pixel movement threshold, vertical-axis dominance, accumulated scroll displacement, completion, and cancellation. DOM capture, focus, and Host commands are adapter effects returned by that model.                                                                                                                          |
| Transitions                | Primary touch down enters `pending-tap` without selection, focus, or Host mouse input. Motion remains pending below the threshold; vertically dominant motion at or beyond it enters `scrolling`. Pending release produces one tap at the final cell; scrolling release produces no tap. Pointer cancel, lost capture, focus/visibility repair, capability replacement, and disposal return to `idle` without a tap.                                                                                                                             |
| Outputs and invariants     | A completed tap focuses the writable textarea from the following non-passive `touchend`; a projected application-mouse owner also receives one final-cell press/release pair. A scroll never focuses, selects, clicks, or publishes text. Natural vertical displacement reuses wheel ownership: application mouse receives buttons 4/5, the writable alternate screen receives Arrow Up/Down, and the primary screen navigates retained history. Host selection is preserved unless later admitted input uses the existing pre-input transition. |
| Native input               | Mobile focus keeps a deletable sentinel in the textarea so empty-prompt Backspace remains observable, strips that sentinel from every committed/preedit fallback, and uses an effective native input font size of at least 16 CSS pixels to avoid focus zoom. Enter, Backspace, Delete, independent text, and IME composition retain the existing ordered, exactly-once input contract. Read-only attachments never admit mobile text.                                                                                                           |
| Ownership and dependencies | This package owns gesture and native-input correctness. The application owns mobile-control visibility, localized controls, one-shot modifier policy, visual-viewport geometry, and platform validation. Terminal Host remains the sole owner of VT input encoding, retained history, and selection.                                                                                                                                                                                                                                             |
| Verification and limits    | Pure transition tests cover threshold minus/equal/plus one, axis ties, natural direction, stale identities, completion, and every cancellation. Projected-terminal adapter tests cover tap, application mouse, scrolling, sentinel deletion, IME, read-only, and repair. A controlled headed browser trace remains required to verify DOM ordering and geometry; real iOS and Android keyboard appearance remains release validation rather than an automated operating-system claim.                                                            |

Every raw, key, committed-text, paste, application-mouse, focus, and repair command carries the
current positive attachment capability revision. Direct construction defaults to the initial
revision `1`, while Desktop bootstraps forward the Host-returned revision.
`applyInputCapability` accepts only the exact attachment and terminal identity, an exact replay, or
the next monotonic revision with a changed input value. A stale revision is inert; an identity
mismatch, same-revision conflict, skipped revision, or changed revision with unchanged authority
fails closed to read-only. Downgrade invalidates the dispatcher generation and immediately clears
composition, native text settlement, paste review, held key/mouse/focus ownership, and pointer
capture without sending client-side releases that duplicate the Host repair. Upgrade removes the
native read-only guard but restores no prior input or queued work; the next admitted input is
stamped only with the new revision. A surface that holds document focus at that moment makes a new
focus claim, because the Host replays none across an upgrade.
If the projected client disappears before this cleanup completes, Terminal Host releases only that
attachment's accepted key, application-mouse, and focus ownership before revoking its capability;
another attachment is unaffected. The formal input-repair model owns valid next-revision
replacements, including a downgrade re-entered from an active send. Malformed, stale, replayed,
conflicting, skipped, or non-advancing updates and the committed-versus-local admission split remain
deterministic ProjectedTerminal evidence.

`injectInput(...)` is the narrow bridge for a trusted, application-owned live input surface. It
accepts either committed text or one physical key tap together with the exact opaque revision
previously read from `injectedInputReadiness`. The revision must still match both at admission and
when the ordered dispatcher executes; a read-only-to-write-ready cycle therefore cannot revive
queued input from an older authority. Accepted input then uses terminal-local key normalization,
Option-as-Alt policy, and the shared pre-input surface transition. The readiness event changes
across attachment authority, opening, capability revocation, freeze, and disposal. Injection does
not accept paste, drop, mouse, history, or replay semantics.
A `sent` result means the command port accepted the attempted command, `failed` means delivery may
be uncertain, and `blocked` means this surface did not publish it.

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

Opening waits for the configured face with representative Latin, CJK, emoji, and Nerd Font glyphs, then installs frame, renderer, input, resize, and accessibility handling before activating projection demand. Each open attempt transactionally owns its canvas, input, accessibility nodes, renderer, observers, and listeners: setup failure removes only those resources and returns the same instance to `constructed` for retry; duplicate opens reject without changing the current surface, and disposal remains terminal. The package default remains browser-generic `monospace`; an application may provide a provisioned product face, and Orchbee's shared Web/Desktop application registers its pinned Ghostty-aligned primary and symbol fallback before opening a surface instead of depending on a machine-installed font. Surface sizing subtracts computed padding from the available grid, retains the mature two-column minimum, and proposes no PTY geometry when padding consumes either content dimension. Padding balance remains opt-in. A measured resize is sent before active projection demand and is deduplicated only while it agrees with the latest Host-committed geometry or remains inside the short observer re-entry window. A Host geometry change, broker rejection, transport failure, or later unchanged observation makes the proposal eligible for retry. The hidden IME control uses the same font and follows the projected cursor so the native candidate window stays aligned. Device-pixel-ratio changes rebuild renderer geometry and terminal pixel dimensions instead of leaving a stale atlas. A frame commits into a bounded row-partitioned structure-of-arrays store and is acknowledged only after draw scheduling. `FrameApplied` acknowledges the client commit, not completed painting. Exact fully decoded duplicates are re-acknowledged. After a trusted, correctly scoped fixed header, body rejection requests a snapshot for that header's exact sequence; an uninspectable or unscoped header makes the connection terminal, emits no guessed request, and requires the application to retire the scoped transport. Delta continuity includes columns, grid rows, cell-pixel width, and cell-pixel height, and every sparse candidate is checked against retained neighboring wrap, spacer-head, and wide-cell topology before any row mutates. Every retained grapheme, hyperlink, metadata-string, and style occurrence contributes to a conservative 36-MiB aggregate charge; the separately tracked canonical complete OBTF snapshot is no larger and remains within 4,096 distinct strings and 1,024 distinct styles. Incremental candidate accounting rejects an over-limit delta atomically with a typed terminal outcome. Sequence `u64::MAX` never implies a successor. Once the store enters `connection-terminal`, that phase is absorbing until disposal; every frame and snapshot-replacement entry point preserves the last committed presentation and refuses to revive the store.

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

A surface smaller than the drawn grid crops it as the native Client's live view does, by the rule
[Server takeover](../../apps/orchbee-server/docs/terminal-controller-takeover.md#sizing-across-clients)
states for every Client: it shows the cells around the visible cursor, or the bottom-left corner
while the cursor is hidden, and a larger surface keeps drawing from the top-left corner.
`cropLiveGrid` chooses those cells after each drawn frame and each surface, device-pixel-ratio, or
configuration change. The canvas keeps the whole grid and only moves and clips, so pointer,
selection, mouse-report, link, and IME geometry, which read the canvas bounds, keep addressing the
cells shown beneath them, and link hints label only shown cells. While cropped, a screen without
scrollback can still enter history, a history window is no taller than the surface, and a page is
the shown row count. A Viewer drops the Agent tail window while cropped; the controller keeps it
until its own resize lands, so its content does not jump during a pane resize. `liveCropped` and
`onLiveCropChange` report the state so the composing Client can say the view is cropped.

An open attachment accepts one complete `TerminalPresentationConfiguration` transaction containing
`fontFamily`, `fontSize`, `fontWeight`, `fontWeightBold`, `fontLigatures`,
`fontMetricModifiers`, `terminalPadding`, `paddingBalance`, `allowTransparency`, `backgroundOpacity`, and
`defaultColorOverride`. The override is either `null` or normalized `#RRGGBB` foreground/background with an optional
cursor, its covered text color, and a sixteen-entry packed RGB `ansi` palette. These are Client-local presentation values;
Host frame and query metadata remain unchanged. Server-backed Orchbee Web/Desktop terminals use
this source so existing cells recolor immediately, while the controlling Client separately reports
its effective query colors. `backgroundOpacity` is a source-neutral `0..1` configuration value;
the composing client applies it to its own default-background root rather than to the terminal
surface. `terminalColorThemes` carries every shipped color theme's canvas, cursor, selection, and
ANSI 0–15, generated from [`crates/color-themes`](../../crates/color-themes/README.md#gui-files);
`resolveTerminalColorTheme` turns a stored slot id into a theme, falling back to the Orchbee theme of
the slot's polarity, and `terminalColorThemeAnsi` lists its ANSI colors in palette order. There are no runtime partial setters. `updatePresentationConfiguration` validates and
isolates the whole candidate, awaits its regular and bold font descriptions, and then prepares
renderer measurements before publishing any field. Requests are latest-wins: superseded or
disposed preparation returns a typed cancellation, and validation, font, or renderer preparation
failure leaves the previous configuration visible.

One successful local commit replaces all eleven fields, remeasures once, resets the glyph atlas once,
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
draw work. A document-hidden WebGPU surface also unconfigures its canvas and releases its
per-surface GPU residency and shared-device lease while retaining the packed store and CPU
presentation caches. Hidden commits still advance and acknowledge the packed store. Visibility
restoration reacquires the current shared-device generation and draws the newest committed state
once; that restoration draw is permitted even while explicit presentation demand remains paused,
after which the independent pause continues to suppress later draws. Canvas 2D visibility changes
remain timer-and-draw suspension only.

`freeze()` is the application recovery handoff: it publishes native text still settling, which no
later surface saw, and then retires input, transport-facing listeners, animation, resize, search,
and in-flight side effects while retaining the last painted canvas and accessible row list. Frozen
instances reject further frame and command work. A later `dispose()` releases the retained renderer
resources and DOM after the application has painted a replacement.

WebGPU initialization, visibility restoration, and device-loss recovery fence every asynchronous
completion against both lifecycle and visibility epochs. Hiding during pending initialization
retires that attempt; stale work releases its uncommitted lease and resources and cannot configure a
context, activate Canvas fallback, or publish renderer health. Showing the surface publishes no
backend by itself: a fresh visible initialization must complete first.

`notifyBrokerFailure` preserves the broker's typed code, semantic operation, retryability, optional
Host correlation ID, and optional client request sequence on `onError`. It also applies the owned
consequence: resize rejection re-admits unchanged geometry, history rejection completes or repairs
the exact row request, selection rejection releases gesture scheduling, and a stale in-flight
gesture rejection instead retires the complete local gesture and drops every unsent endpoint before
being absorbed. Correlated copy or link failure completes only that side effect. The package never
turns retryability into automatic replay.

The renderer consumes packed rows directly. It never reconstructs a local VT state or a per-cell
object graph. After a draw completes, frame telemetry records the exact committed Host generation
and revision read from the visible packed store. Multiple commits may coalesce behind one animation
frame; that checkpoint names the newest commit actually drawn, never an earlier scheduled revision.

Committed live frames may also carry the bounded Kitty graphics extension. A graphics snapshot
atomically replaces decoded in-memory image resources, resolved placements, and the monotonic
animation tick; a placement-only update is accepted only against the exact committed image
revision and cannot move the tick backward. Invalid bytes or a missing resource reference reject
the whole terminal frame and request a new snapshot before text or graphics can be partially
committed. Canvas2D and WebGPU convert raw gray/gray-alpha/RGB/RGBA pixels once per image generation,
reuse the resulting resource for crop, scale, and movement, and paint the exact three signed-z
layers below cell backgrounds, between backgrounds and text, and above text. Render-detail OBRW
replaces rows and its independent graphics snapshot atomically; text detail carries no
graphics. Text-only commits retain the same graphics identity, so a static placement does not turn
an otherwise dirty-row Canvas2D update into a full-surface repaint. Empty snapshots, deletion,
reset, replacement, and disposal release obsolete pixel,
Canvas, and GPU resources. This package deliberately never parses PNG or Kitty commands, loads a
path or shared-memory object, or resolves animation, virtual/relative placement, or history pins;
Terminal Host completes that work before owned decoded pixels and absolute placements cross the
protocol boundary.

Row-window input is strictly decoded by default. An application adapter may set `projectionBufferTrust: 'local-host'` only when it owns an immutable, same-user, capability-scoped Terminal Host transport. That path retains the OBPB format-`0.1.0` buffer and creates typed-array views without rescanning or repacking every cell. Network, plugin, and persisted input must retain the default `external` behavior.

## History, search, selection, and links

Bottom-following use retains only the current packed grid. Entering history pauses live projection
and requests one bounded `current-view` row window whose first row is the visible logical top, with
limited forward overscan. `RowsApplied` lets the Host pin that top; while the anchor remains valid,
navigation and reflow refreshes use `committed` or `tail` origins and treat the Host-returned start,
current revision, and total-row count as authoritative. Ordinary PTY output may advance revision
while a viewport request is in flight without invalidating it. Generation change still retires the
request, and exact-revision search and general-buffer pages retain their deterministic stale-result
handling. When a newer reflow event supersedes an in-flight viewport read, its completion only
settles that old flight; the readable window remains visible and one event-driven refresh uses the
newest generation and committed pin without surfacing the obsolete failure. Superseded background
row reads likewise settle their bounded stale result without presenting a user-visible history
failure.
When valid-anchor maintenance refresh and explicit viewport work are both pending, the latest user
line, page, scrollbar, search-reveal, or selection-autoscroll intent is dispatched first. Its
current-view request uses the newest Host revision and committed pin, so it also satisfies the older
maintenance refresh. History observation may race the Host's admission of that current-view read:
a same-generation response at the observed revision or newer establishes the newer pending Host
pin and supersedes a maintenance refresh or missing-anchor recovery queued before the response. A
missing-anchor event newer than the response remains authoritative and recovers to the bottom
before any queued history intent can run.
Clear, screen replacement, eviction, or any other invalid anchor resumes bottom projection while a
fresh snapshot replaces the last readable history window.

### Retained-history resize model

The resize owner may propose a newly measured grid while retained history is visible, but that
proposal does not become history-presentation authority until a newer Host generation reports the
committed anchor as valid. The client then refreshes from that anchor using the proposed visible
row count; after the row window commits, its metadata owns subsequent history navigation. The Host
preserves the committed logical top when a full window fits and otherwise moves the start upward
just enough to return a full window. Thus growing a surface never leaves an unmodeled blank band
below a valid retained-history viewport.

| Input and state                                                                            | Result                                                                                                               |
| ------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------- |
| Following bottom                                                                           | Live-frame geometry remains authoritative; no history refresh is introduced.                                         |
| Valid retained-history anchor and a newer Host generation matching a local resize proposal | Refresh with the proposal's visible row count, then commit the Host-returned start and row-window metadata.          |
| Committed anchor is too close to the retained tail for the enlarged row count              | Keep the anchor when possible, otherwise clamp the start to `totalRows - rowCount` so the returned viewport is full. |
| Resize is rejected, the attachment is not the resize owner, or the anchor is invalid       | Do not adopt the proposal; retain the last readable window or enter the existing bottom-recovery path.               |

This changes neither PTY resize ownership nor the row-flight state machine, protocol, overscan
bound, or invalid-anchor recovery. Deterministic projected-terminal and Terminal Host tests cover
the resize-event/row-window trace and the near-tail clamp.

Once a viewport read, exact buffer read, or asynchronous link effect installs its local
correlation, command publication is part of that operation's settlement boundary. A synchronous
refusal or throw, or a later send-Promise rejection, retires only the captured request sequence:
buffer reads settle `unavailable`, `openLinkAsync`/`openTargetAsync` settle retryable
`link-send-failed`, and viewport reads enter the existing history recovery or queued-transition
policy. The failed command is never retried automatically. A failure arriving after completion,
cancellation, disposal, or a newer request cannot replace the settled result or retire newer work;
the ordinary command-error diagnostic remains observable.

### Agent tail-row presentation

The application may enable a display-only trailing-blank-row policy for a terminal presentation.
The application owns whether the current presentation represents a live CLI Agent; this package
receives only the resulting boolean policy and never infers an Agent from process names, commands,
or terminal text. Ghostty and Terminal Host remain authoritative for the PTY geometry, active
screen, cursor, retained history, and packed cell contents. A missing policy source is equivalent to
`false`.

The model reads one committed live primary-screen grid, its visible cursor, graphics placements,
application-mouse modes, and retained-row count. It classifies only a contiguous suffix. A row is
not blank when it contains non-whitespace text, a visible cursor, a graphics placement, a prompt or
row flag, a non-default background, a style flag, or underline presentation. The policy is disabled
on the alternate screen and while application mouse tracking is active.

| Condition                                                                            | Presentation result                                                                                                                  |
| ------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------ |
| Policy disabled, alternate screen, application mouse active, or no safe blank suffix | Present the exact live grid.                                                                                                         |
| Safe suffix exists but retained history is shorter than that suffix                  | Replace only as many suffix rows as retained rows can fill; never move the gap to the top.                                           |
| Safe suffix exists and enough retained history is available                          | Request one exact Host tail window ending before the removable suffix and present the returned full-height window.                   |
| Read fails, becomes stale, is superseded, or its anchor becomes invalid              | Settle any Host row acknowledgement, discard the candidate, and fall back to the exact live grid until a later live frame can retry. |

The result is a presentation window, not a VT mutation: it never changes PTY rows, Ghostty state,
retained history, frame acknowledgements, or terminal output. Internal blank rows are preserved and
at least one live row remains. A committed semantic tail window uses the Host history anchor for
selection coordinate resolution while live projection remains active; its cursor is remapped from
the same live frame. Keyboard input continues to target the live terminal. Manual history
navigation takes ownership from that window, and disabling the policy, losing live-Agent
eligibility, changing screen or generation, resizing, or disposing restores ordinary
bottom-following state. Accessibility rows, links, search, selection, the scrollbar, and hidden
input geometry all consume the same committed visible window.

The Host-projected row window owns the visible-tail policy. Deterministic policy tests cover every content signal and no-op boundary;
projected-terminal tests cover exact-window admission, cursor remapping, failure fallback, runtime
disablement, alternate-screen and application-mouse exclusion, and transfer to manual history.

Primary-screen viewport progress and wheel presentation follow these rules: a valid pinned
primary-screen viewport continues accepting history motion while output advances, and each admitted
whole-row wheel delta commits the resulting viewport before the next render without a client-owned
snapshot, transform, easing curve, or synthetic momentum. Orchbee retains its Host-projected
row-window architecture; exact pixels, transport latency, operating-system momentum events, and
alternate-screen application input remain governed by their respective host and transport contracts.

Wheel pixel, line, and page deltas accumulate until they produce whole terminal rows. Each admitted
primary-screen movement then delegates to the same Host-authoritative history-intent workflow as
programmatic navigation, and the next committed row window replaces the canonical canvas
immediately. The client adds no animation phase; momentum exists only when the platform supplies
more wheel events. Line, page, bottom, scrollbar, search-reveal, and selection-autoscroll navigation
use the same immediate presentation rule.

Retained history exposes a client-owned DOM scrollbar overlay positioned against the complete terminal viewport rather than the fitted cell-grid canvas. Its rightmost 16 CSS pixels form the interaction area; an 8 CSS-pixel fully rounded thumb remains exactly 2 CSS pixels from the Panel-relative right edge, independent of unused grid remainder pixels, and retains 4 CSS pixels of vertical track padding plus a 20 CSS-pixel minimum height. History navigation and scrollbar interaction make it immediately visible, and inactivity removes its opacity after 1,500 milliseconds. Track presses jump proportionally, and captured scrollbar drags coalesce into bounded tail-relative `current-view` row-window requests. The initiating primary pointer owns the drag until it ends; concurrent and secondary pointer events and wheel input are consumed without replacing its capture or anchor, and its synthesized release click is consumed once. Scrollbar ownership takes precedence over application mouse reporting, selection, and links, and is repaired on buttonless movement, cancellation, capture loss, focus or visibility loss, reset, and disposal. Navigation during a drag keeps the thumb fully visible, and the fade delay begins only after ownership ends or is repaired. The scrollbar overlay and canvas remain `aria-hidden` and expose no DOM scrollbar role; the existing keyboard and API history navigation remain the accessibility path. The package entry exports the scrollbar's geometry, drag resolution, and fade delay so another list, such as the Workbench sidebar, scrolls with the same scrollbar counted in its own units.

Search is client-side. The first non-empty query lazily creates a Worker and streams bounded text-detail pages through a source-neutral row reader. One Worker page and one prefetched row request bound the pipeline; page credits refresh an inactivity watchdog, and the 16 MiB UTF-8 source limit publishes either one complete immutable result or no matches. Viewport, search, and general-buffer reads use independent single-flight lanes, so scrolling or link inspection cannot cancel a search page; request-sequence correlation permits their replies to settle in any order while bounding the client to three simultaneous row reads. Query input uses a 50-millisecond trailing debounce. Revision invalidation instead keeps an already scheduled deadline and lets an admitted exact-revision scan settle without cancellation. A result publishes only when the row-reader generation and revision still equal the captured source; an invalidated result or stale or evicted page publishes nothing, preserves accepted navigation, and schedules exactly one passive refresh after 200 milliseconds. Repeated output can therefore neither postpone admission by resetting that refresh deadline nor restart an admitted scan. Completion still requires one exact-revision attempt to settle before another revision change; pages from different revisions are never combined. Navigation requested during debounce or scanning remains queued across same-query revision invalidation. A passive revision refresh updates results without revealing the active match again, so manually reaching the live bottom remains stable; queued or later explicit navigation still reveals its target. Decorations and result counts remain capped, while directional navigation independently scans the admitted retained search domain from the latest Host selection anchor and wraps without treating that decoration cap as the final match. The active match is reported separately, selected through a revision-pinned Host `set-range` command, and explicit queries and navigation reveal it even when it falls outside the decoration collection. Active and regular search palettes both take precedence over an overlapping Host selection independently of find or terminal-input focus, while the active palette remains distinct. The selection palette composites a low-saturation blue accent at 40% over the effective terminal default background and uses its effective default foreground. Query text and matches never enter Terminal Host or an Electron broker.

Incremental query changes have one visual commit boundary. A `searching` event updates status but does not replace the renderer's last committed decorations or its search-owned active selection. The next complete result replaces both in one JavaScript turn; a complete no-match result, invalid query, timeout, unavailable search, explicit clear, or disposal removes them. This prevents the 50-millisecond debounce and asynchronous Worker scan from presenting an empty highlight frame between adjacent query strings while keeping status and result-count events tied to the requested query.

Search callbacks may synchronously replace the query or dispose its owner. Each continuation checks
the existing query generation before scheduling work, adopting a Worker, activating a result, or
publishing completion. Cancellation revokes that generation and detaches its timers, row-read
controller, progress waiter, and Worker before calling abort listeners, so an older cleanup cannot
clear a newly admitted query. A Worker returned after its factory retires the query is terminated
without receiving search input. [Search controller tests](src/search/__tests__/projected-search-controller.unit.test.ts)
exercise notification, activation, factory, and abort-listener reentry with a controlled clock and
Worker; these local traces do not add a protocol or an exhaustive model-checking claim.

Previous- and next-prompt navigation follows the bounded [behavior contract](docs/prompt-navigation.md). It scans Host-authored OSC 133 row classes from the current visible top through the general-buffer lane, nearest first, and commits only while the primary screen, pinned snapshot, selected visible or same-intent revision-lag repair anchor, and local history intent remain current. Each invocation admits at most 16 MiB of encoded row windows. Missing marks, alternate-screen use, bounds, source exhaustion, replacement, cancellation, and stale or unavailable reads leave the viewport unchanged. The package does not inject shell integration or infer prompts from terminal text.

Focused-terminal line, page, top, and bottom shortcuts follow the bounded
[Keyboard scrollback](docs/keyboard-scrollback.md). The terminal
admits them only on the primary screen, preserves one-row overlap for page movement, and
delegates every request to the existing Host-authoritative history-intent workflow. A boundary is
an unchanged local result; alternate-screen shortcuts remain available to the terminal application.

Visible keyboard Hints follow the bounded
[Keyboard hints](docs/keyboard-hints.md). One invocation reads only
the pinned visible row window, excludes logical lines clipped by a viewport edge, and labels at most
512 permitted URLs, paths, file locations, and lowercase hexadecimal hashes. The overlay owns label
key events until selection or cancellation. `ProjectedTerminal` returns the selected typed target;
the application owns URL opening or exact-text insertion through the normal paste boundary.

```ts
terminal.scrollLines(-1);
terminal.scrollPages(-1);
terminal.scrollToTop();
terminal.scrollToBottom();
await terminal.scrollToPreviousPrompt(signal);
await terminal.scrollToNextPrompt(signal);
await terminal.chooseVisibleHint(signal);
terminal.updateSearch('failed', { caseSensitive: false, wholeWord: true });
terminal.findNext();
```

Selection endpoints and exported content remain authoritative in Terminal Host. The browser derives a one-, two-, or three-click sequence from a 500-millisecond interval and a one-cell surface-distance limit instead of trusting DOM click counts, then sends a captured generation, bounded viewport-relative cell point, surface position, and cell geometry for Ghostty's word, line, rectangle, and 60% within-cell endpoint rules. The Host resolves the point at processing time against the current active viewport or the attachment's committed history anchor, so continuous revision changes do not starve the gesture. A plain click does not synthesize a one-cell selection. Dragging beyond the viewport advances retained history through bounded row-window requests and continues the same Host gesture at the newly exposed row; document-level move and release fallbacks retain that behavior when pointer capture is unavailable. A modifier-assisted triple click selects Ghostty's semantic command output and dragging expands across output blocks. Only an ordinary primary-pointer release completes the gesture, and it requests copy only while copy-on-select is on. The serialized command queue retains the latest coalesced drag update ahead of that release even when the initial press reply is still in flight. A generation-stale in-flight reply silently cancels the complete local gesture, drops unsent updates and endpoints, stops autoscroll, releases pointer capture, preserves the latest Host selection, and performs no automatic copy; the next pointer press starts a fresh gesture. Matching buttonless motion while the browser still owns the captured pointer does not infer release or cancel the gesture; the explicit primary release supplies the final endpoint even when no intermediate drag update was usable. Pointer cancellation or capture loss, surface focus or document visibility loss, presentation-geometry replacement, screen change, and anchor eviction cancel the pinned gesture without copying or clearing the latest valid Host selection. Serialized gesture replies make a normal release or cancellation produce exactly one copy consequence; copy-on-select is off unless construction-time `copyOnSelect: true` or runtime `setCopyOnSelect(true)` opts in to normal-release copy. Every admitted automatic or manual copy publishes one source-tagged completion or failure through `onSelectionCopyResult` after the application-owned clipboard effect resolves. Programmatic range and semantic-output selection remain exact-revision operations and use absolute `bigint` rows. `adjustSelectionEndpoint` preserves the latest Host-published start endpoint and rectangle mode while replacing only the active end endpoint; it never reconstructs Ghostty text semantics in the client.

The hidden textarea is the only browser-editing and IME owner. Selection and native paste use this ownership model:

| Browser origin                                                                            | Canonical and DOM state                                                                                      | Transition and output                                                                                                                                                             |
| ----------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Pointer, programmatic, or search-owned terminal selection                                 | Host endpoints remain canonical; Canvas and the accessibility description project them                       | Never creates or replaces the document-global DOM Selection, so the textarea remains the native input owner                                                                       |
| Non-collapsed DOM Range wholly inside visible accessibility rows                          | The document Range is the assistive-technology origin and is owned only while it remains inside this surface | UTF-16 offsets map across combining, wide, and surrogate-pair graphemes into one explicit Host cell-range command; an exact Host confirmation may normalize only that owned Range |
| Native paste targeted at the textarea or a package-owned terminal-presentation descendant | Clipboard MIME data remains browser-owned until the existing paste policy admits it                          | Runs the shared text/image paste path exactly once and stops propagation; textarea delivery cannot be processed again by the root fallback                                        |
| Native paste targeted at an application-owned descendant such as the find input           | The descendant retains native editing ownership                                                              | The terminal does not consume or cancel the event                                                                                                                                 |

A pointer or non-assistive programmatic selection first clears any Range still owned by the accessibility projection. When the document-global Range moves to another surface or disappears during sibling focus, this surface relinquishes only its accessibility ownership; the attachment's Host selection and Canvas overlay remain active. Surface or window focus loss relinquishes the same ownership before the browser can collapse the Range, so that presentation-only collapse cannot publish a Host clear command. Search-owned selection follows the ordinary Host-selection row and therefore never occupies the document-global Range. An attachment constructed or dynamically downgraded with no input authority keeps its hidden textarea focusable but natively read-only. Shift-only page navigation, wheel history, search, selection, copy, approved links, and the Server screen operations clear and reset remain available without Host input capability; defensive `beforeinput`, composition, paste, key, and mouse listeners own rejected browser editing and clear residual input state without emitting PTY input. An identity-checked natural-exit outcome permanently seals PTY input, clear, reset, and resize and makes the existing textarea read-only. The retained final frame and local output interactions remain available until disposal. The application performs the clipboard side effect and reports only its outcome.

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
metadata and detected filesystem targets may remain in the bounded link cache. A caller must grant
one attachment explicit local-filesystem-link capability before a filesystem target receives active
pointer styling, product context, or activation. The `localFileEffects` option grants it, and no
current composition sets that option, the Server path included. Content revisions, viewport transitions, surface
resize, device-pixel-ratio changes, presentation geometry changes, application-mouse ownership, and
gesture transitions retire the hover cache. A modifier-assisted click revalidates the hovered
generation, revision, hover epoch, clicked cell, exact typed target, and current capability before
`ProjectedTerminal` emits one approved URL or local-file request. The application decides whether
and how to perform that effect. This package never reads a path or invokes an operating-system
opener.

### Local filesystem link model

Local filesystem interaction is a per-attachment, default-off projection over the existing typed
link cache. URLs retain the `http`/`https`/`mailto` policy. An enabled local file or directory uses
Command plus primary click on Apple platforms and Control plus primary click elsewhere; ordinary
primary drag remains terminal selection, and application mouse tracking retains ownership unless
Shift returns the gesture to the product. Relative paths are resolved from the generation-pinned
current directory before the target is exposed. Quoted and escaped path syntax affects detection
only; the effect boundary receives the normalized path and separately validates it as untrusted
input.

Plain-text drops follow the same sanitization, review, ordering, and Host input path as paste. The
package accepts no file drops and receives no filesystem authority.

The off-screen accessibility mirror is a labeled `list` whose direct `listitem` rows preserve spaces, grapheme-to-column mappings, and absolute `aria-posinset`/`aria-setsize` positions. It labels rows and the cursor with caller-provided localized text. A separate polite status region announces only changed visible output, including localized deletion announcements. Retained-history inspection disables live updates without replacing the live-output baseline, so returning to the bottom announces only output changed since the last live frame.

## Bounded row API

The asynchronous row reader exposes only revision- and generation-pinned pages. It intentionally
has no synchronous whole-buffer string API. Local command-publication failure settles the exact
read as `unavailable` and releases its purpose lane for a later explicit read.

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
state, and cursor metadata. Host frame metadata remains unmodified. Default tokens use the effective Client defaults;
`TerminalColorEncoding.palette | n` uses its ANSI 0–15 palette. The presentation override's `ansi`
entries take precedence over the renderer theme, and direct projected theme publication installs
its complete ANSI set with the overlays. Palette-only changes invalidate retained rows. Explicit
RGB (including black), extended palette indexes and child OSC 4 overrides retain Host resolution;
OSC 104 restores tags. Inverse swaps already-resolved colors, faint keeps its existing intensity,
and underline tags use the same ANSI set. An absent underline inherits displayed text. Selection
and search overlays retain precedence. Cursor uses a Client cursor override when present, otherwise
the Host cursor RGB (or Host foreground fallback); cursor accent affects only its covered text.
These local values do not require a Host recolor to paint. `allowTransparency` alone controls transparent renderer clear; a composing client
uses `backgroundOpacity` for its effective default-background root and never applies whole-surface
CSS opacity. A newly opened Canvas remains transparent until it paints either an injected semantic
presentation snapshot or its first live frame, then reuses the latest effective default background
for opaque fills and resize. Packed graphemes preserve CJK, combining, and emoji
cell widths; explicit foreground/background and underline colors, inverse, bold, faint, italic,
invisible, blink, underline variants, strikethrough, and overline use the projected Ghostty styles.
Renderer health reports initialization, fallback, device loss, recovery, and recovery failure.

`capturePresentationSnapshot()` exposes only the exact packed source and metadata represented by
the most recently completed draw. If a newer Host commit is accepted before it is painted, capture
returns no snapshot until that revision is drawn; it never labels unseen state as visible. A
captured snapshot is immutable and display-only, structurally shares immutable row backing where
possible, accounts its retained bytes, and includes the visible graphics resources and placements.
Supplying one while opening a new `ProjectedTerminal` paints it before scheduled live rendering,
but that draw emits no live-frame telemetry or acknowledgement and grants no attachment, input,
resize, transport, metadata-publication, or durable-state authority. The terminal releases its
opening reference once the cached draw completes, while an external bounded cache may retain the
snapshot for later presentation.

`isPresentationGeometryCurrent()` compares the metadata represented by that most recently completed
draw with a synchronous measurement of the currently mounted surface using the same grid, padding,
cell, DPR, protocol, and PTY pixel-limit rules as resize submission. It returns `false` before any
draw, for an unmeasurable surface, and after a layout, presentation-configuration, or DPR change
until a matching Host frame has painted. An injected display snapshot may satisfy the comparison
only when its exact geometry already matches the target surface. The query is read-only: it never
resizes a canvas, submits PTY geometry, acknowledges a frame, or grants input authority. Application
handoffs may use it to keep an older meaningful layer visible until the target canvas is safe to
commit. A surface without the resize lease can stay mismatched for as long as another owner keeps
the terminal at a different size, so a handoff cannot wait on this alone for such a surface.

WebGPU adapter acquisition, device creation, preferred format, and the immutable background/text
pipeline pair are shared by renderers in one `Document`. Every configured renderer owns its glyph
atlas texture, sampler, uniform and vertex buffers, bind groups, Kitty textures, reconstructible CPU
staging, and one detachable shared-runtime lease. Document hiding unconfigures that canvas,
destroys those per-surface GPU resources, clears peak-sized staging arrays, and releases the lease;
the renderer retains its packed source, raw graphics data, glyph map, and CPU atlas Canvas so a
visible restoration can rebuild without changing terminal state. A pending initialization is
retired by the same visibility epoch. `freeze()` remains different: it preserves the already-painted
canvas and its resources for application recovery handoff. Device loss invalidates the document
generation; each affected visible renderer follows the typed recovery path against the next shared
generation, while Canvas 2D remains the observable fallback. The shared runtime owns the
generation's only pending `GPUDevice.lost` reaction. Releasing a lease removes its subscriber before
decrementing the reference count; releasing the final visible-or-frozen lease destroys the device
and discards shared pipelines. The `Document`-keyed weak map is only the concurrent discovery
boundary, so hidden or inactive terminals retain neither GPU surfaces nor an otherwise idle device.

The packed source remains the only terminal-content authority. WebGPU row preparation retains only
reconstructible shaping and background, text, and overlay instance segments for the current visible
grid. An entry is reusable only while its packed row is not dirty and its source identity, shaping
inputs, resolved default colors, atlas generation, and row-local cursor projection still agree, and
no global presentation invalidation is pending.
Dirty rows rebuild from the packed source; moving the cursor rebuilds its previous and current rows.
A complete Host redraw, surface or font geometry change, theme publication, device-pixel change,
selection, search, preedit, text-blink, or link-hover change conservatively invalidates every
affected prepared segment. Atlas reset or growth invalidates every cached text UV. The cache is
bounded by the visible rows and their current compact instance payload and never participates in
`markRendered()` ownership.

Each WebGPU instance stream retains the row-segment identities and lengths, one geometrically grown
`Float32Array` mirror, and one geometrically grown GPU vertex buffer. When changed row segments keep
their prior lengths, only their exact compact ranges are copied and submitted with `writeBuffer`.
The first length change moves later compact offsets, so that stream rewrites the suffix from that
row; a new or grown GPU buffer receives one complete upload. An unchanged stream performs no vertex
upload while the render pass still draws its complete compact instance count into the current
swapchain texture. Device loss destroys the GPU buffer, making the next visible draw a complete
upload even when CPU row preparation survived. Document-hidden parking and disposal release all
row preparation and peak-sized staging storage; the next visible full draw reconstructs it.

Per-cell foreground/background resolution reuses one renderer-owned scratch projection while row
preparation copies only values that outlive the current cell. These rules bound retained CPU staging
by peak prepared viewport content and reduce preparation and upload work without changing draw
order, pixels, Canvas fallback behavior, or device-loss recovery. They do not define a GPU-driver,
browser, real-GPU execution-time, or whole-process RSS ceiling.

Dirty glyph-atlas rectangles transfer directly from the Canvas atlas into the WebGPU texture, so
incremental uploads do not materialize an intermediate CPU pixel array. Frame telemetry maintains
atlas occupancy when glyphs are admitted or reset instead of rescanning the glyph map. The hidden
IME textarea reuses the Canvas-to-surface offset between geometry changes. While an injected,
display-only snapshot is the painted source and no live Host frame exists, that snapshot supplies
only the anchor geometry for its painted cursor and still grants no input authority. The surface
remeasures the offset after resize, device-pixel-ratio, or presentation-configuration changes and
before native macOS Shift input-mode handling.

The glyph atlas starts at 1024 physical pixels on each axis. Its CPU Canvas backing plus matching
RGBA8 GPU texture therefore has an 8 MiB nominal pixel payload before browser and driver overhead.
Admitting a complete raster reservation independently doubles only the axis whose current bound is
too small, up to the device texture limit; row-wide shaping can therefore consume additional width
without multiplying unused height. A 2048-by-1024 atlas has a 16 MiB nominal combined pixel payload.
A cache reset clears entries while retaining the current atlas dimensions. Document-hidden parking
retains that CPU atlas and glyph state but destroys its GPU texture; restoration marks the entire
atlas dirty and uploads it into the newly created texture. Freezing retains the last presented canvas
and resources, while disposal unconfigures the WebGPU canvas, destroys its per-surface GPU resources,
clears glyph state, and resets the renderer-owned atlas Canvas backing to zero pixels.
These are atlas and lifecycle allocation bounds, not a GPU-driver, browser, or whole-process RSS
ceiling.

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

Search behavior follows the bounded contract in [`docs/vscode-search-alignment.md`](docs/vscode-search-alignment.md). The application owns the find surface, shortcuts, and localized labels.

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

Deterministic Canvas and WebGPU owner tests prove that committed cursor metadata selects the
corresponding cursor geometry and blink scheduler. Host runtime evidence covers application Reset
and an explicit non-default profile. The real-PTY `terminal-parity/SU-13-cursor-defaults` witness,
which observed the explicit block/non-blinking default, a blinking-bar DECSCUSR override, and RIS
restoration through the production surface, retired with the Desktop Electron suite.

`measureMountedTerminalGeometry(...)` loads the selected font and applies the same grid and physical
cell rules as the first projected resize. Host owner tests cover the Host side of deferred start.
The Desktop owner tests of the start sequence and the POSIX-only
`terminal-parity/SU-16-startup-geometry` witness, which matched the child PTY's first size and the
sole first client frame to the mounted grid, retired with Desktop's Main-owned runtime.

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
explicit URL cases. `terminal-parity/FS-07-keyboard-hints` checks the bounded visible-row read,
four target kinds, prefix-free labels, exact hash insertion, URL opening, Escape cancellation, and
zero label-key input through the same production surface. Owner-local unit tests additionally cover malformed context, exact-input
copying, normalization, automatic-detection filtering, and revalidation immediately before
execution.

The terminal protocol's generated TypeScript/Zig conformance corpus proves that `plain` is the only
admitted request and response value and that HTML and raw VT requests fail before Host runtime
access. The strict projected-client command exposes no format field. The
`terminal-parity/HS-07-selection-export` witness, which checked the real PTY-to-Electron clipboard
result on macOS, retired with the Desktop Electron suite.

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

### Immediate retained-history evidence

The `ProjectedTerminal` regressions cover row-window correlation, queued wheel and programmatic
intent, and bottom recovery. The headed `terminal-parity/SU-11-wheel-scroll` trace, which drove
wheel history through a real Desktop attachment and Terminal Host and showed that committed history
rows change without a snapshot canvas or CSS transform, retired with the Desktop Electron suite.

| SU-11 claim                                                     | Projected integration                                                                                                                                     | Reference evidence                                                  | Limits                                                                                                   |
| --------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| Wheel history commits without a client-owned presentation phase | `commits wheel history immediately without a presentation transaction`; `coalesces queued wheel and programmatic history intents on the canonical canvas` | Pinned Ghostty `Surface.scrollCallback` and `Termio.scrollViewport` | No real-surface run; no OS momentum cadence, WebGPU, transport-latency, or alternate-screen input claim. |

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

Deterministic WebGPU tests separately verify that hiding unconfigures the context, destroys
per-surface resources, preserves CPU atlas state, releases the final shared lease, fences a pending
initialization, and rebuilds on visibility restoration. A projected integration test verifies the
single forced restoration draw uses the newest revision while explicit demand remains paused and
that later paused revisions do not draw.

| SU-15 claim                                                                                                                                       | Renderer evidence                                                                                                                                                                   | Projected integration                                                                                                                                                                         | Headed system trace                | Limits                                                                                                                                                                 |
| ------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Visibility and explicit demand retain independent pause causes; commits and acknowledgements continue while draws coalesce until the final resume | `suspends packed blinking-cell animation while hidden`; `parks document-hidden WebGPU residency and rebuilds it from retained CPU state`; pending-init and shared-lease regressions | `coordinates document visibility and projection demand before drawing the newest acknowledged frame`; `draws one current restoration frame while explicit presentation demand remains paused` | `terminal-parity/SU-15-visibility` | Headed Chromium Canvas 2D plus deterministic WebGPU ownership/refinement tests; no OS background-throttling, cross-browser, or synchronous physical-reclamation claim. |

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

### Owner-local compatibility evidence index

The stable identifiers below preserve the owner-local compatibility witnesses that do not require
their own headed system-test title. Each row maps the identifiers to executable suites run by the
named package gates. The deterministic TypeScript suites control semantic state, clocks, geometry,
and renderer adapters; they do not claim native font rasterization, operating-system UI, or another
browser engine. Native Host refinements and headed cross-owner witnesses retain the narrower limits
stated by their owning guides. Witnesses that ran only in Desktop's Electron suite, which drove the
Desktop application through its former Main-owned runtime, retired with it, so the rows below name
only evidence that still runs; witnesses that drive Web fixture pages run in the Web system suite.

| Stable witness                                                                                                                                                                                                              | Executable owner evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `terminal-parity/PR-01-graphemes`, `terminal-parity/PR-02-cell-topology`, `terminal-parity/PR-04-wrap-boundaries`, `terminal-parity/PR-05-styles`, `terminal-parity/PR-07-semantic-links`, `terminal-parity/PR-08-metadata` | [`src/projected/__tests__/ghostty-surface-parity.unit.test.ts`](src/projected/__tests__/ghostty-surface-parity.unit.test.ts), the Host-owned surface-fixture gate, and the controlled corpus and limits in [`docs/ghostty-surface-test-provenance.md`](docs/ghostty-surface-test-provenance.md).                                                                                                                                                                                                                                                                                                                                            |
| `terminal-parity/PR-03-wide-cursor`, `terminal-parity/SU-03-cursor`                                                                                                                                                         | [`src/rendering/__tests__/resolvePackedCursorPresentation.unit.test.ts`](src/rendering/__tests__/resolvePackedCursorPresentation.unit.test.ts) and [`src/rendering/__tests__/WebGPURenderer.unit.test.ts`](src/rendering/__tests__/WebGPURenderer.unit.test.ts) execute the shared Canvas/WebGPU geometry and one-physical-pixel presentation rules.                                                                                                                                                                                                                                                                                        |
| `terminal-parity/PR-06-colors`                                                                                                                                                                                              | [`src/projected/__tests__/ghostty-surface-parity.unit.test.ts`](src/projected/__tests__/ghostty-surface-parity.unit.test.ts) and [`src/rendering/__tests__/WebGPURenderer.unit.test.ts`](src/rendering/__tests__/WebGPURenderer.unit.test.ts), refined across owners by `terminal-parity/SU-09-theme`.                                                                                                                                                                                                                                                                                                                                      |
| `terminal-parity/PR-09-frame-commit`                                                                                                                                                                                        | [`src/projected/__tests__/packed-terminal-store.unit.test.ts`](src/projected/__tests__/packed-terminal-store.unit.test.ts), its 200-run shrinkable `fast-check` state trace, shared `presentation-store-transition.json` corpus, and [`src/projected/__tests__/projected-terminal.unit.test.ts`](src/projected/__tests__/projected-terminal.unit.test.ts) cover exact header-correlated recovery, connection-terminal failures, `u64::MAX`, four-field geometry continuity, retained-neighbor topology, graphics tick/reference boundaries, multi-store isolation, bounded sparse streams, acknowledgement, and completed-draw correlation. |
| `terminal-parity/SU-01-font-load`, `terminal-parity/SU-02-font-metrics`                                                                                                                                                     | [`../../apps/web/src/workbench-client/projected-terminal-renderer/__tests__/registerTerminalProductFonts.unit.test.ts`](../../apps/web/src/workbench-client/projected-terminal-renderer/__tests__/registerTerminalProductFonts.unit.test.ts), [`src/__tests__/font-metric-modifiers.unit.test.ts`](src/__tests__/font-metric-modifiers.unit.test.ts), and [`src/__tests__/buildTextShapingPlan.unit.test.ts`](src/__tests__/buildTextShapingPlan.unit.test.ts); the headed `terminal-parity/SU-02-font-metrics` witness retired, and native raster limits remain in the provenance guide.                                                   |
| `terminal-parity/SU-04-ime-geometry`, `terminal-parity/SU-05-overlays`, `terminal-parity/SU-06-fit`                                                                                                                         | [`src/input/__tests__/calculateInputAnchor.unit.test.ts`](src/input/__tests__/calculateInputAnchor.unit.test.ts), [`src/input/__tests__/preedit.unit.test.ts`](src/input/__tests__/preedit.unit.test.ts), [`src/rendering/__tests__/WebGPURenderer.unit.test.ts`](src/rendering/__tests__/WebGPURenderer.unit.test.ts), [`src/__tests__/surface-size.unit.test.ts`](src/__tests__/surface-size.unit.test.ts), and [`src/__tests__/measureMountedTerminalGeometry.unit.test.ts`](src/__tests__/measureMountedTerminalGeometry.unit.test.ts).                                                                                                 |
| `terminal-parity/SU-07-resize-convergence`, `terminal-parity/SU-12-backends`                                                                                                                                                | [`src/projected/__tests__/projected-terminal.unit.test.ts`](src/projected/__tests__/projected-terminal.unit.test.ts), [`src/rendering/__tests__/WebGPURenderer.unit.test.ts`](src/rendering/__tests__/WebGPURenderer.unit.test.ts), the Host geometry evidence, and headed `terminal-parity/LC-07-resize-lease`.                                                                                                                                                                                                                                                                                                                            |
| `terminal-parity/IN-01-keyboard`                                                                                                                                                                                            | [`src/projected/__tests__/projected-terminal.unit.test.ts`](src/projected/__tests__/projected-terminal.unit.test.ts), Terminal Protocol semantic-input conformance, and Host input-encoding tests cover normalized physical keys, current modifiers, keypad/Kitty behavior, and native Apple Command handoff.                                                                                                                                                                                                                                                                                                                               |
| `terminal-parity/IN-02-composition`                                                                                                                                                                                         | The [input-repair and native-settlement models](formal/README.md), [`src/projected/__tests__/projected-terminal.unit.test.ts`](src/projected/__tests__/projected-terminal.unit.test.ts), and exact headed `terminal-parity/IN-02-composition` cover composition repair, Shift-punctuation native text, deterministic text fallback, ordering, and duplicate suppression.                                                                                                                                                                                                                                                                    |
| `terminal-parity/IN-03-paste`, `terminal-parity/IN-18-native-selection-paste`                                                                                                                                               | [`src/projected/__tests__/projected-terminal.unit.test.ts`](src/projected/__tests__/projected-terminal.unit.test.ts), the Web Paste Review tests, and Host protocol/runtime paste tests cover sanitization, review, ordering, bracketed-mode agreement, capability, image-only Control+V, ordinary Host-selection isolation, and exactly-once native delivery through textarea or package-owned surface targets; the Desktop image-paste and native-selection-paste traces retired.                                                                                                                                                         |
| `terminal-parity/IN-04-focus`, `terminal-parity/IN-05-mouse-ownership`, `terminal-parity/IN-06-pointer`, `terminal-parity/IN-13-wheel`, `terminal-parity/IN-17-mobile`                                                      | [`src/projected/__tests__/projected-terminal.unit.test.ts`](src/projected/__tests__/projected-terminal.unit.test.ts) and [`src/input/__tests__/surface-mouse.unit.test.ts`](src/input/__tests__/surface-mouse.unit.test.ts) execute focus-boundary repair, fixed gesture ownership, pointer derivation, delta-mode/axis policy, alternate-screen fallback, and touch activation.                                                                                                                                                                                                                                                            |
| `terminal-parity/HS-01-history-window`, `terminal-parity/HS-02-reflow`, `terminal-parity/HS-03-history-transition`, `terminal-parity/HS-09-scrollback`                                                                      | [`src/projected/__tests__/projected-terminal.unit.test.ts`](src/projected/__tests__/projected-terminal.unit.test.ts), plus Terminal Host history-row, selection, and configuration tests linked by the [Host runtime contract](../../apps/terminal-host/docs/runtime-model.md), cover pinned windows, reflow, invalidation, eviction, and zero-retention bounds.                                                                                                                                                                                                                                                                            |
| `terminal-parity/HS-11-keyboard-scrollback`                                                                                                                                                                                 | Projected-terminal, Web Action, shortcut, Settings, and storage unit tests cover six primary-screen bindings, one-row page overlap, boundaries, and durable migration; the headed Desktop witness of unchanged attachment identity retired.                                                                                                                                                                                                                                                                                                                                                                                                 |
| `terminal-parity/HS-12-sustained-output-interaction`                                                                                                                                                                        | Projected-terminal buttonless-motion and revision-churn regressions, Host stale-row-acknowledgement tests, and the history-interaction model cover fine wheel accumulation, retained-history entry, and captured drag completion while revisions continue; the headed real-PTY/Electron witness of clipboard copy under live output retired.                                                                                                                                                                                                                                                                                                |
| `terminal-parity/HS-04-selection-gesture`, `terminal-parity/HS-05-selection-cancel`, `terminal-parity/HS-06-selection-eviction`, `terminal-parity/HS-08-selection-commands`, `terminal-parity/HS-10-copy`                   | [Selection implementation tests](src/projected/__tests__/projected-terminal.unit.test.ts), Host selection/export tests, and Web clipboard-result tests cover gesture derivation, stale retirement, cancellation, pinned export, absolute endpoints, and one perceivable copy outcome; the headed `terminal-parity/HS-04-selection-gesture` witness retired.                                                                                                                                                                                                                                                                                 |
| `terminal-parity/FS-01-search`, `terminal-parity/FS-02-search-scheduling`                                                                                                                                                   | [`src/search/__tests__/projected-search-controller.unit.test.ts`](src/search/__tests__/projected-search-controller.unit.test.ts), [`src/search/__tests__/projected-search-worker.unit.test.ts`](src/search/__tests__/projected-search-worker.unit.test.ts), and [`src/search/__tests__/ProjectedSearchAccumulator.unit.test.ts`](src/search/__tests__/ProjectedSearchAccumulator.unit.test.ts) cover bounded results, independent navigation, invalidation, cancellation, progress, and reveal.                                                                                                                                             |
| `terminal-parity/FS-03-links`, `terminal-parity/FS-04-link-invalidation`                                                                                                                                                    | [`src/__tests__/terminal-row-links.unit.test.ts`](src/__tests__/terminal-row-links.unit.test.ts) and [`src/projected/__tests__/projected-terminal.unit.test.ts`](src/projected/__tests__/projected-terminal.unit.test.ts) cover complete bounded logical lines, precedence, exact cell mapping, hover retirement, and activation revalidation.                                                                                                                                                                                                                                                                                              |
| `terminal-parity/FS-19-local-file-links`                                                                                                                                                                                    | Row-link and projected-terminal unit tests and the exact headed `terminal-parity/FS-19-local-file-links` witness cover typed path detection, capability gating, modifier activation, and absence without the capability. The traces use a captured opener; they do not launch an operating-system application, automate Finder, prove network-filesystem latency, or grant remote file access. No current composition supplies the local-file capability.                                                                                                                                                                                   |
| `terminal-parity/FS-07-keyboard-hints`                                                                                                                                                                                      | [`src/__tests__/terminal-hints.unit.test.ts`](src/__tests__/terminal-hints.unit.test.ts), projected-terminal, Web Action, shortcut, Settings, storage, and exact headed Web system-suite evidence cover visible target detection, overlay ownership, cancellation, effects, migration, and unchanged attachment identity.                                                                                                                                                                                                                                                                                                                   |
| `terminal-parity/AC-01-accessible-rows`, `terminal-parity/AC-02-dom-selection`, `terminal-parity/AC-03-announcements`, `terminal-parity/AC-04-deletion`                                                                     | [`src/accessibility/__tests__/accessible-text-model.unit.test.ts`](src/accessibility/__tests__/accessible-text-model.unit.test.ts) and [`src/projected/__tests__/projected-terminal.unit.test.ts`](src/projected/__tests__/projected-terminal.unit.test.ts) cover valid row structure, ordinary Host-selection isolation, assistive-technology-origin grapheme-safe bidirectional Range mapping, ordered live output, history suppression, and atomic deletion announcements.                                                                                                                                                               |
| `terminal-parity/CF-01-host-authority`                                                                                                                                                                                      | This is a justified architecture-negative witness: [`project.json`](project.json), dependency-boundary checks, and [`scripts/check-public-api.mjs`](scripts/check-public-api.mjs) admit only the projected API and no PTY-byte writer, browser VT, or synchronous whole-buffer authority. A differential runtime trace is not applicable because no second VT transition system exists.                                                                                                                                                                                                                                                     |
| `terminal-parity/CF-02-bounds`                                                                                                                                                                                              | The packed-store suite incrementally executes the conservative 36-MiB per-occurrence aggregate, exact canonical snapshot, 4,096-string, and 1,024-style retained budgets across long unique and repeated-value sparse-delta streams and proves atomic over-limit outcomes. The projected-terminal, search, link, accessibility, protocol-limit, and Host backpressure suites named above cover the remaining retained projection, row, result, flight, revision, native allocation, and queue limits.                                                                                                                                       |

## Development

Run from the repository root:

```sh
pnpm --filter @orchbee/terminal typecheck
pnpm --filter @orchbee/terminal build
pnpm --filter @orchbee/terminal test
pnpm --filter @orchbee/terminal benchmark:renderer
pnpm --filter @orchbee/terminal test:exports
pnpm verify
```

Client-owned runtime presentation, input repair, row-flight, search and selection workflows use the controlled implementation tests in [`formal/README.md`](formal/README.md). The Desktop-owned protocol core for cross-process IPC retired with Desktop's Main-owned runtime; this package exposes no empty TLA command.

`benchmark:renderer` measures deterministic JavaScript WebGPU row preparation and mocked queue
submission for a stable frame, one changing row, and a moving cursor. It is a same-machine
diagnostic for before/after comparisons; it does not measure real GPU execution, browser scheduling,
font rasterization, driver memory, or whole-process RSS.

`pnpm audit:ghostty-panel` compares the maintained renderer/font provenance and generated data with the pinned Ghostty source when `../ghostty` or `GHOSTTY_SOURCE` is available. Both audit and generation reject a checkout whose HEAD differs from the Terminal Host pin or whose worktree is dirty. Run `pnpm generate:ghostty-panel-data` only when the pinned Nerd Font constraints, sprite ranges, or sprite reference rasters change.

## Theme catalog integration

The [theme catalog](../../crates/color-themes/README.md) generates terminal palettes. Applications
resolve the active light/dark slot; this package renders it without owning preferences or OS
appearance. ANSI 0–15 tags resolve locally, so retained cells recolor without program output.
Host-only recoloring could not serve differently themed Clients viewing one terminal. Controlling
query-color reports are a separate application/Worker responsibility. The renderer has no
minimum-contrast filter rewriting explicit program RGB. Regenerate with `pnpm generate:color-themes`.
