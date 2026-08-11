# @orchbee/terminal

Source-neutral browser client surface for a terminal whose authoritative VT state lives in Orchbee Terminal Host.

## Scope

This package owns one projected terminal surface:

- atomic packed-frame commit and snapshot recovery;
- WebGPU rendering with a Canvas 2D fallback;
- bounded history-window presentation;
- client-side search and nearby-row link discovery;
- Host-owned selection interaction and copy correlation;
- keyboard, committed text, IME, mouse, focus, paste, and resize intents; and
- visible-viewport accessibility.

It never receives or parses PTY output, loads a VT engine, or exposes `write(ptyBytes)`. Terminal Host owns the PTY, Ghostty VT state, terminal modes, retained history, selection endpoints, and mode-dependent input encoding. The application owns transport, terminal inventory, product topology, clipboard effects, external navigation, and localized product UI. The machine-readable scope contract lives in [`project.json`](project.json).

`ProjectedTerminal` represents exactly one attachment. Project, Workspace, Tab, Panel, and visibility policy remain above this package; a hidden terminal does not require a live renderer surface, and projection/history demand can be paused independently of the Host runtime.

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

Opening waits for the configured face (including representative Latin, CJK, and emoji glyphs), then installs frame, renderer, input, resize, and accessibility handling before activating projection demand. Surface sizing subtracts computed padding from the available grid. Every observed container change rebalances the canvas and input anchor immediately, while a PTY resize is emitted only when the resulting cell or pixel geometry changes. The hidden IME control uses the same font and follows the projected cursor so the native candidate window stays aligned. Device-pixel-ratio changes rebuild renderer geometry and terminal pixel dimensions instead of leaving a stale atlas. A frame commits into a bounded row-partitioned structure-of-arrays store and is acknowledged only after draw scheduling. Exact duplicates are re-acknowledged; invalid identity, sequence, generation, dimension, or delta-base input requests a complete snapshot without mutating the committed grid.

The renderer consumes packed rows directly. It never reconstructs a local VT state or a per-cell object graph. Frame telemetry records the exact committed Host revision drawn by the renderer.

Row-window input is strictly decoded by default. An application adapter may set `projectionBufferTrust: 'local-host'` only when it owns an immutable, same-user, capability-scoped Terminal Host transport. That path retains the OBPB format-`0.0.1` buffer and creates typed-array views without rescanning or repacking every cell. Gateway, network, plugin, and persisted input must retain the default `external` behavior.

## History, search, selection, and links

Bottom-following use retains only the current packed grid. Entering history pauses live projection and commits one bounded row window with limited overscan. Stale, evicted, or invalid windows recover to a fresh bottom snapshot.

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

Link discovery reads only bounded nearby rows. `ProjectedTerminal` emits an approved URL request; the application decides whether and how to open it. Filesystem and unknown targets are not opened by this package.

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

WebGPU is preferred when available; Canvas 2D is the observable fallback. Both paths consume the same packed projection, theme, font configuration, selection/search overlays, preedit state, and cursor metadata. Packed graphemes preserve CJK, combining, and emoji cell widths; bold, faint, italic, underline variants, strikethrough, and overline use the projected Ghostty style flags. Renderer health reports initialization, fallback, device loss, recovery, and recovery failure.

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
