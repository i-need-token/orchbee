# @orchbee/terminal

Ghostty's VT engine compiled to WebAssembly, exposed through an xterm-compatible browser API and rendered with WebGPU.

## Scope

This package owns terminal parsing, rendering, input, selection, and browser host integration. It does not own application PTY processes, session transport, Electron menus, clipboard permissions, paste dialogs, or external navigation. The machine-readable scope contract lives in [`project.json`](project.json).

## Install

```sh
pnpm add @orchbee/terminal
```

## Use

```ts
import { init, Terminal } from '@orchbee/terminal';

await init();

const terminal = new Terminal({
	cols: 80,
	rows: 24,
	// Ghostty measures scrollback page storage in bytes.
	scrollback: 10_000_000,
	cursorBlink: true,
	fontWeight: 'normal',
	fontWeightBold: 'bold',
	fontLigatures: true,
	paddingBalance: 'equal',
	fontMetricModifiers: {
		cellHeight: '10%',
		iconHeight: -1,
	},
});

await terminal.open(document.querySelector('#terminal')!);
terminal.onData((data) => pty.write(data));
terminal.onBinary((data) => pty.write(Buffer.from(data, 'binary')));
pty.onData((data) => terminal.write(data));
```

Pass an explicit WASM URL when your bundler does not preserve package asset URLs:

```ts
await init({ source: '/assets/ghostty-vt.wasm' });
```

Route optional Ghostty diagnostics through an application-owned logger. Initialization remains silent when `log` is omitted.

```ts
await init({
	source: '/assets/ghostty-vt.wasm',
	log: (message) => applicationLogger.debug(message),
});
```

WebGPU is used when available; Canvas 2D is the fallback. The package includes selection, scrollback, mouse and focus reporting, Ghostty key encoding, OSC 8 and plain-URL links, terminal query responses, the buffer API, and `FitAddon`.

The comparison demos load JetBrains Mono 2.304 variable fonts from Ghostty's pinned dependency and include the upstream OFL-1.1 license.

## Surface API

Platform effects are requested through `TerminalHost`; the terminal core does not own Electron menus, clipboard permissions, paste dialogs, or external navigation.

```ts
const terminal = new Terminal({
	host: {
		capabilities: { contextMenu: true, openTarget: true },
		clipboard: applicationClipboard,
		confirmPaste: (request) => showPasteDialog(request),
		resolveDrop: (request) => resolveDroppedFiles(request.files, request.cwd),
		showContextMenu: (request) => showApplicationMenu(request),
		openTarget: (request) => openAllowedTarget(request.target),
	},
});
```

Selection is backed by Ghostty tracked grid references and supports plain, HTML, and VT copy formats, rectangle/word/line/output selection, and endpoint adjustment. A selection is cleared if scrollback eviction removes either endpoint, so copy operations cannot silently target replacement content. OSC 133 prompt/input/output metadata is available on every `CellData` and in context-menu requests, including requests over visible scrollback; delayed context-menu actions keep using the captured absolute buffer cell even if the viewport moves.

`getSelectionPosition()` reports stable absolute active-buffer coordinates, including when later output moves the selected cells out of the viewport; viewport-relative coordinates remain internal to rendering and accessibility.

```ts
const matches = terminal.findAll('failed', { caseSensitive: false, wholeWord: true });
terminal.findNext('failed');
terminal.highlightSearch('failed');
terminal.selectOutputAt(column, row);
terminal.adjustSelection('endOfLine');

const visibleText = terminal.getAccessibleText();
const visibleRange = terminal.getAccessibleVisibleRange();
terminal.selectAccessibleRange({ start: visibleRange.start, end: visibleRange.start + 5 });

// Navigation stays ordered when each match is revealed from scrollback.
terminal.findPrevious('failed');

const telemetry = terminal.onRendererTelemetry((frame) => {
	recordTerminalFrame(frame.durationMs, frame.atlasUploadBytes, frame.reasons);
});

const shortcut = terminal.registerKeyEventHandler({
	id: 'application.open-command-palette',
	priority: 100,
	scope: 'application',
	handle: (event) => {
		if (!event.metaKey || event.code !== 'KeyK') return false;
		openCommandPalette();
		return true;
	},
});
```

Search navigation remains ordered while matches are revealed from scrollback. Terminal output invalidates rendered search highlights so overwritten or evicted coordinates cannot color unrelated text; callers can run `highlightSearch` again for the updated buffer.

`terminal.capabilities` reports the active renderer and application-provided host capabilities without advertising unsupported graphics protocols.

`scrollback` is Ghostty's page-storage budget in bytes, not a row count. It defaults to 10 MB, is allocated lazily, and accepts `0` to disable history.

`paddingBalance` accepts `false`, `true`, or `"equal"`. The `true` mode caps top padding and shifts excess space to the bottom; `"equal"` centers the grid. CSS padding on the terminal parent remains the explicit minimum. `fontMetricModifiers` mirrors Ghostty's `adjust-*` behavior: integer values add CSS pixels, percentage strings such as `"10%"` change the measured value proportionally, and cell-height changes recenter the face without implicitly resizing the cursor.

`fontWeight` and `fontWeightBold` accept CSS `normal`/`bold` keywords or numeric variable-font weights. The corresponding `FontWeight` type is exported from the package entry point. Both the WebGPU glyph atlas and Canvas 2D fallback use the configured values, so applications can compensate for platform rasterizer differences without substituting a different font face or changing terminal cell geometry.

The exported `fontSizeAdjustmentScale` helper implements Ghostty's fallback-face normalization for `ic-width`, `ex-height`, `cap-height`, and `line-height`. Browser font discovery remains owned by `FontFaceSet`; callers that manage explicit fallback faces can apply the returned scale when constructing those faces.

## Browser behavior

- Hidden-textarea IME input compensates for the browser's composition caret and anchors the candidate UI at the Ghostty-style cursor midpoint; canvas preedit uses one underline without marked-range inversion.
- Cursor bars, underlines, preedit underlines, and unfocused hollow blocks use one physical pixel; focus restores the configured cursor style and blink behavior.
- Unsafe paste is classified, sanitized, ordered with keyboard input, optionally confirmed by the host, and rechecked against read-only state after asynchronous confirmation.
- Entering read-only mode first repairs application key and mouse presses; those ordered release records remain deliverable without changing the viewed scrollback or local selection, so the remote application cannot be left in a stuck input state. IME composition, `beforeinput`, and native paste received while already read-only are rejected without emitting application input.
- Read-only mode returns mouse ownership to the terminal surface even when the active application left mouse reporting enabled, preserving local scrollback navigation, selection, links, and context menus.
- Key contributions use stable IDs, priority, ownership scope, and state predicates; focus loss repairs missing Kitty key releases and application mouse releases.
- `Shift+PageUp` and `Shift+PageDown` navigate scrollback locally; unmodified paging keys remain available to the terminal application.
- Keyboard input and the start of an IME composition return a scrolled viewport to the active prompt before presenting or emitting user text.
- DEC focus reports preserve the viewed scrollback position and local selection because they are protocol state, not user text input. Moving focus between the terminal host and its hidden input remains inside one focus boundary and does not emit a duplicate focus-in report.
- Terminal-driven scrollback removal, including `CSI 3 J`, clamps the current and target viewport offsets to retained history.
- Resizing while viewing history tracks the visible top cell through Ghostty reflow, preserving the logical reading position when wrapped-row counts change and emitting the resulting numeric viewport offset to scroll listeners.
- Resetting from an alternate screen updates buffer state and emits the primary-buffer transition immediately.
- Content, viewport, and geometry changes invalidate hovered links so a modifier-click cannot activate a target that moved away from the pointer; both direct activation and context-menu link actions require the requested buffer cell to remain inside the resolved link range.
- The browser click synthesized after a selection drag is consumed before link activation, preventing a drag across a direct link from opening its target.
- Dragging a selection through either vertical edge continues on document-level pointer movement and autoscrolls the browser-owned viewport without moving Ghostty's independent internal viewport; releasing outside the canvas preserves the completed selection.
- Losing focus, hiding the page, or observing pointer movement without the primary button cancels incomplete selection, scrollbar, and application-mouse drags while preserving completed selection text.
- Hiding the page also stops smooth scroll at the currently rendered offset, so the next wheel gesture cannot resume from an invisible stale target.
- Concurrent `open()` calls are rejected while font or GPU initialization is pending, and disposal during either await aborts the remaining surface setup.
- Disposal releases terminal-owned component references after cleanup, allowing removed canvases, input surfaces, selection state, and renderer caches to be collected even if the host retains the disposed Terminal object.
- Each `resize()` emits one final geometry event; synchronizing `options.cols` and `options.rows` does not expose intermediate dimensions to PTY transports, and `open()` applies a cols/rows option pair atomically.
- `FitAddon.fit()` always compares its proposed geometry with the terminal's current dimensions, so it restores container fit after an application-driven resize even when the container itself did not change.
- `FitAddon` uses the full container content width because the terminal scrollbar is an in-canvas overlay, not an external DOM scrollbar that needs a reserved gutter.
- ResizeObserver notifications that arrive during FitAddon's short resize reentry window remain debounced for a follow-up fit instead of being discarded.
- Runtime theme replacement resolves omitted foreground, background, cursor, and palette entries from the default theme in both Ghostty and the renderer, so assigning an empty theme fully resets prior custom colors.
- While stdin is writable, application mouse tracking owns unshifted pointer gestures. Shift keeps an entire drag or wheel gesture terminal-owned even if the modifier is released mid-drag; Shift+right-click and Shift+link activation remain explicit host overrides.
- Dropped text uses the paste safety pipeline; dropped files require host-owned path resolution and shell quoting.
- Canvas content has an off-screen accessibility mirror with stable UTF-16/cell offsets, two-way DOM selection, cursor position, and live output.
- WebGPU glyphs use physical-pixel rasterization and rebuild when browser DPR changes.
- Regular, italic, bold, and bold-italic cells use the configured regular and bold font weights consistently across WebGPU, Canvas 2D, font measurement, preedit text, and the hidden input surface.
- The configured primary and fallback font list is loaded through `FontFaceSet` before cell metrics are measured; failures surface through `onHostError` without preventing startup.
- Glyph quads preserve measured ink bounds and bearings instead of clipping italic overhangs or forcing every glyph into one cell-sized rectangle.
- Nerd Font glyphs use Ghostty's generated constraint table, including adjacent-space width allowance, icon sizing, and grid-aligned stretching. Selected box-drawing, block-element, corner-triangle, and core Powerline characters use Ghostty-compatible procedural sprites in both renderers.
- Cell, decoration, cursor, and icon metric modifiers share Ghostty's percentage, absolute-delta, clamping, and cell-height recentering rules.
- Balanced padding distributes fractional grid space around the canvas while respecting explicit parent padding.
- New WebGPU glyphs upload a merged dirty atlas region without replacing existing texels; colored emoji preserve their RGB while ordinary glyphs remain terminal-tintable.
- Bounded programming-ligature runs are rasterized as one glyph sequence in both renderers and can be disabled through `fontLigatures`.
- Frame telemetry exposes timing, reasons, visited cells, instances, shaping runs, draw calls, upload bytes, and atlas pressure.
- WebGPU device loss rebuilds the device, pipelines, atlas texture, buffers, and canvas configuration once.
- SGR blink, faint, inverse, invisible, overline, strikethrough, underline variants, and underline colors render in both backends.
- X10, UTF-8, SGR, URXVT, and SGR-pixel mouse reports include horizontal wheel and pixel coordinates.
- Pointer shape follows Ghostty for mouse reporting, Shift selection override, rectangular selection, and modifier-enabled links.
- VT output and reset refresh pointer ownership immediately, without waiting for the user to move the mouse.
- Plain URLs and CWD-relative file paths can span soft-wrapped rows; file links preserve line/column suffixes.

Current Ghostty alignment and remaining gaps are tracked in [Ghostty Terminal Panel Conformance](../../docs/terminal/ghostty-surface-test-provenance.md).

## Development

```sh
pnpm --filter @orchbee/terminal build
pnpm --filter @orchbee/terminal test
pnpm --filter @orchbee/terminal test:exports
pnpm audit:ghostty-panel # Requires ../ghostty or GHOSTTY_SOURCE.
pnpm verify
```

The Rslib build emits ESM, CommonJS, TypeScript declarations, and source maps under `dist/`.

When upgrading Ghostty, run `pnpm generate:ghostty-panel-data` to regenerate the Nerd Font constraint table. The panel audit rejects a stale generated table.
