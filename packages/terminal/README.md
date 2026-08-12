# @orchbee/terminal

Source-neutral browser client surface for a terminal whose authoritative VT state lives in Orchbee Terminal Host.

## Scope

This package owns one projected terminal surface:

- atomic packed-frame commit and snapshot recovery;
- WebGPU rendering with a Canvas 2D fallback;
- bounded history-window presentation;
- client-side search and nearby-row link discovery;
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

Opening waits for the configured face (including representative Latin, CJK, and emoji glyphs), then installs frame, renderer, input, resize, and accessibility handling before activating projection demand. The source-neutral default face is browser-generic `monospace`; an application may provide a provisioned product face, but the shared Desktop/Browser composition does not silently substitute a system-only font. Surface sizing subtracts computed padding from the available grid, retains the mature two-column minimum, and proposes no PTY geometry when padding consumes either content dimension. Padding balance remains opt-in. A measured resize is sent before active projection demand and is deduplicated only while it agrees with the latest Host-committed geometry or remains inside the short observer re-entry window. A Host geometry change, broker rejection, transport failure, or later unchanged observation makes the proposal eligible for retry. The hidden IME control uses the same font and follows the projected cursor so the native candidate window stays aligned. Device-pixel-ratio changes rebuild renderer geometry and terminal pixel dimensions instead of leaving a stale atlas. A frame commits into a bounded row-partitioned structure-of-arrays store and is acknowledged only after draw scheduling. `FrameApplied` acknowledges the client commit, not completed painting. Exact duplicates are re-acknowledged; invalid identity, sequence, generation, dimension, or delta-base input requests a complete snapshot without mutating the committed grid.

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

Bottom-following use retains only the current packed grid. Entering history pauses live projection and commits one bounded row window with limited overscan. Stale, evicted, or invalid windows recover to a fresh bottom snapshot.

Retained history exposes a client-owned in-canvas scrollbar overlay. Its rightmost 16 CSS pixels form the interaction area; the shared Canvas 2D and WebGPU projection draws only an 8 CSS-pixel thumb inset by 4 CSS pixels, with a 20 CSS-pixel minimum height. History navigation and scrollbar interaction make it immediately visible, and inactivity removes its opacity after 1,500 milliseconds. Track presses jump proportionally, and captured scrollbar drags coalesce into bounded absolute row-window requests. The initiating primary pointer owns the drag until it ends; concurrent and secondary pointer events and wheel input are consumed without replacing its capture or anchor, and its synthesized release click is consumed once. Scrollbar ownership takes precedence over application mouse reporting, selection, and links, and is repaired on buttonless movement, cancellation, capture loss, focus or visibility loss, reset, and disposal. Navigation during a drag keeps the thumb fully visible, and the fade delay begins only after ownership ends or is repaired. The canvas remains `aria-hidden` and exposes no DOM scrollbar role; the existing keyboard and API history navigation remain the accessibility path.

Search is client-side. The first non-empty query lazily creates a Worker and streams bounded text-detail pages through a source-neutral row reader. Query text and matches never enter Terminal Host or an Electron broker.

```ts
terminal.scrollLines(-1);
terminal.updateSearch('failed', { caseSensitive: false, wholeWord: true });
terminal.findNext();
```

Selection endpoints and exported content remain authoritative in Terminal Host. Pointer drags use gesture messages; a plain click does not synthesize a one-cell selection. Dragging beyond the viewport advances retained history through bounded row-window requests and continues the same Host gesture at the newly exposed row. A modifier-assisted triple click requests Ghostty's semantic command-output selection. Completed pointer and semantic-output selections request copy by default; set `copyOnSelect: false` to opt out. Assistive-technology DOM ranges use one explicit cell-range command, with UTF-16 offsets mapped across combining, wide, and surrogate-pair graphemes. Host selection is mirrored back into the visible-viewport accessibility tree when both endpoints are visible. Read-only attachments retain local focus, scrolling, and selection behavior without emitting PTY input. The application performs the clipboard side effect and reports only its outcome.

```ts
terminal.selectAll();
const result = await terminal.copySelectionAsync();
if (result.kind === 'failed') reportCopyFailure(result.code);
```

Link discovery reads only bounded generation/revision-pinned nearby rows. OSC 8 targets and OSC 133
cell/row semantics arrive from Host-owned Ghostty state; this package never reparses their escape
sequences. Explicit links take precedence over detected URL and path text. A modifier-assisted
click revalidates the exact hovered cell before `ProjectedTerminal` emits one approved URL request;
the application decides whether and how to open it. Filesystem and unknown targets are not opened
by this package.

Plain-text drops follow the same sanitization, review, ordering, and Host input path as paste. File drops are intentionally left to an application-owned capability that can resolve paths without exposing filesystem authority to the projected client.

The off-screen accessibility mirror labels absolute rows and the cursor with caller-provided localized text. A separate polite status region announces only changed visible output, including localized deletion announcements, while live updates are disabled during retained-history inspection.

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

Ghostty remains the reference for selected font metrics, shaping boundaries, procedural sprites, cursor geometry, and terminal-panel rendering behavior. That reference does not make this package a Ghostty VT owner. Maintenance provenance is recorded in [`docs/ghostty-surface-test-provenance.md`](docs/ghostty-surface-test-provenance.md).

Search behavior follows the bounded contract in [`docs/vscode-search-alignment.md`](docs/vscode-search-alignment.md). The application owns the find surface, query history, shortcuts, and localized labels.

## Development

Run from the repository root:

```sh
pnpm --filter @orchbee/terminal typecheck
pnpm --filter @orchbee/terminal build
pnpm --filter @orchbee/terminal test
pnpm --filter @orchbee/terminal test:exports
pnpm verify
```

`pnpm audit:ghostty-panel` compares the maintained renderer/font provenance with the pinned Ghostty source when `../ghostty` or `GHOSTTY_SOURCE` is available. Run `pnpm generate:ghostty-panel-data` only when the pinned Nerd Font constraint table changes.
