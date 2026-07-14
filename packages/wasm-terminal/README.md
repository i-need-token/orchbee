# @orchbee/wasm-terminal

Ghostty's VT engine compiled to WebAssembly, exposed through an xterm-compatible browser API and rendered with WebGPU.

## Install

```sh
pnpm add @orchbee/wasm-terminal
```

## Use

```ts
import { init, Terminal } from "@orchbee/wasm-terminal";

await init();

const terminal = new Terminal({
  cols: 80,
  rows: 24,
  scrollback: 10_000,
  cursorBlink: true,
  fontLigatures: true,
});

await terminal.open(document.querySelector("#terminal")!);
terminal.onData((data) => pty.write(data));
terminal.onBinary((data) => pty.write(Buffer.from(data, "binary")));
pty.onData((data) => terminal.write(data));
```

Pass an explicit WASM URL when your bundler does not preserve package asset URLs:

```ts
await init({ source: "/assets/ghostty-vt.wasm" });
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

Selection is backed by Ghostty tracked grid references and supports plain, HTML, and VT copy formats, rectangle/word/line/output selection, and endpoint adjustment. OSC 133 prompt/input/output metadata is available on every `CellData` and in context-menu requests.

```ts
const matches = terminal.findAll("failed", { caseSensitive: false, wholeWord: true });
terminal.findNext("failed");
terminal.highlightSearch("failed");
terminal.selectOutputAt(column, row);
terminal.adjustSelection("endOfLine");

const visibleText = terminal.getAccessibleText();
const visibleRange = terminal.getAccessibleVisibleRange();
terminal.selectAccessibleRange({ start: visibleRange.start, end: visibleRange.start + 5 });

const telemetry = terminal.onRendererTelemetry((frame) => {
  recordTerminalFrame(frame.durationMs, frame.atlasUploadBytes, frame.reasons);
});

const shortcut = terminal.registerKeyEventHandler({
  id: "application.open-command-palette",
  priority: 100,
  scope: "application",
  handle: (event) => {
    if (!event.metaKey || event.code !== "KeyK") return false;
    openCommandPalette();
    return true;
  },
});
```

`terminal.capabilities` reports the active renderer and application-provided host capabilities without advertising unsupported graphics protocols.

## Browser behavior

- Hidden-textarea IME input compensates for the browser's composition caret and anchors the candidate UI at the Ghostty-style cursor midpoint; canvas preedit uses one underline without marked-range inversion.
- Cursor bars, underlines, preedit underlines, and unfocused hollow blocks use one physical pixel; focus restores the configured cursor style and blink behavior.
- Unsafe paste is classified, sanitized, ordered with keyboard input, and optionally confirmed by the host.
- Key contributions use stable IDs, priority, ownership scope, and state predicates; focus loss repairs missing Kitty key releases.
- Dropped text uses the paste safety pipeline; dropped files require host-owned path resolution and shell quoting.
- Canvas content has an off-screen accessibility mirror with stable UTF-16/cell offsets, two-way DOM selection, cursor position, and live output.
- WebGPU glyphs use physical-pixel rasterization and rebuild when browser DPR changes.
- The configured primary and fallback font list is loaded through `FontFaceSet` before cell metrics are measured; failures surface through `onHostError` without preventing startup.
- Glyph quads preserve measured ink bounds and bearings instead of clipping italic overhangs or forcing every glyph into one cell-sized rectangle.
- New WebGPU glyphs upload a merged dirty atlas region without replacing existing texels; colored emoji preserve their RGB while ordinary glyphs remain terminal-tintable.
- Bounded programming-ligature runs are rasterized as one glyph sequence in both renderers and can be disabled through `fontLigatures`.
- Frame telemetry exposes timing, reasons, visited cells, instances, shaping runs, draw calls, upload bytes, and atlas pressure.
- WebGPU device loss rebuilds the device, pipelines, atlas texture, buffers, and canvas configuration once.
- SGR blink, faint, inverse, invisible, overline, strikethrough, underline variants, and underline colors render in both backends.
- X10, UTF-8, SGR, URXVT, and SGR-pixel mouse reports include horizontal wheel and pixel coordinates.
- Pointer shape follows Ghostty for mouse reporting, Shift selection override, rectangular selection, and modifier-enabled links.
- Plain URLs and CWD-relative file paths can span soft-wrapped rows; file links preserve line/column suffixes.

Current Ghostty alignment and remaining gaps are tracked in [Ghostty Terminal Panel Conformance](../../docs/terminal/ghostty-surface-test-provenance.md).

## Development

```sh
pnpm build
pnpm --filter @orchbee/wasm-terminal test
```
