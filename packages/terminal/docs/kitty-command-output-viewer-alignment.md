# Kitty command-output viewer alignment

## Reference scope

This contract adopts Kitty's frequent `show_last_command_output` workflow at commit
`6d5d0c440603ad9bdf6dcd599f73f6dde21acb44`, observed on 2026-08-27. Kitty maps
`kitty_mod+g` in `kitty/options/definition.py`; `kitty/window.py` extracts the last shell-command
output; and `kitty/boss.py` sends ANSI text to a configurable external pager in an overlay window.

Orchbee adopts the single-action workflow, shell-semantic boundary, searchable read-only view,
copyability, cancellation, and `Ctrl+Shift+G` default. It deliberately uses an application-owned
viewer so the same interaction can work across Desktop, Web, and later mobile clients. The parity
claim excludes Kitty's external pager process, pager configuration and environment, ANSI styling,
separate pager history, full-scrollback browsing, first/last-visited/non-empty output variants, and
arbitrary program launch.

## Domain contract

`ProjectedTerminal.readLastCommandOutput()` is the only public preparation operation. It is
available only for a mounted primary-screen attachment with retained projected history. One
invocation captures the exact Terminal Host generation, revision, and total-row count, then reads
render-detail rows newest-first through the existing general-buffer lane. Every accepted page must
match that snapshot, requested interval, detail, and byte allowance. The aggregate encoded source
allowance is 16 MiB; the operation never publishes a partial result when the lower semantic boundary
lies beyond that allowance.

The deterministic extractor ignores unwritten cells and uses Host-authored OSC 133 prompt rows plus
cell semantic-content classes:

1. Written output after the newest prompt is the current last-command output region.
2. Otherwise, the newest prompt is the exclusive upper boundary and the preceding prompt is the
   lower boundary of the last completed command.
3. When the older prompt has been evicted but a newer prompt proves shell integration, retained
   output up to the buffer start is returned with `truncatedStart: true`.
4. A buffer with no prompt evidence produces `no-shell-integration`; a bounded region with no
   written output produces `no-output`.
5. Only written cells classified as output contribute text. Physical rows joined by Ghostty soft
   wrap remain one logical line; hard row boundaries become line feeds; prompt and input cells never
   enter the result.

The result is exactly one of `output`, `empty`, `cancelled`, or `unavailable`. `output` carries the
plain UTF-8 text snapshot, inclusive retained row bounds, and the truncation fact. Empty reasons are
stable semantic values. Stale or evicted snapshots, alternate-screen use, malformed pages, disposal,
single-row overflow, transport failure, or source exhaustion produce a typed unavailable reason and
never reuse a prior result.

## Viewer and Action model

Web owns one private command-output viewer per projected terminal mount. The viewer owns only its
immutable prepared result and reversible query/navigation state. Opening a new result replaces the
old snapshot, resets the query, and focuses the query field. Escape or the visible Close control
closes the viewer and restores terminal focus. Closing never cancels, clears, selects, or scrolls
Terminal Host state.

Literal search is case-insensitive, ordered by UTF-16 source offset, non-overlapping, and capped at
1,000 rendered matches. Enter and Shift+Enter cycle forward and backward. `Mod+F` focuses and selects
the viewer query while the viewer owns focus. Search never mutates the terminal's separate retained-
buffer search session. The content remains native selectable text; Copy Output uses the application-
owned clipboard capability and reports its effect before crossing the Action commit point.

Workbench owns `terminal.output.show-last` and `terminal.output.copy`. Web registers both in the
mounted terminal scope. Show Last Command Output uses replace-latest concurrency: a replacement
cancels the older exact row read, and the handler rechecks cancellation before publishing the
prepared result. Copy Output is enabled only for an open non-empty result and an available clipboard
writer. The viewer is a new private business component because the canonical terminal Find Surface
searches a live Host buffer and cannot own an immutable prepared output snapshot.

## Binding and persistence

Apple and Standard platforms both default Show Last Command Output to `Ctrl+Shift+G`, matching
Kitty's default `kitty_mod+g` when Kitty's modifier is its default Control+Shift. The binding applies
to a focused terminal and is projected into the terminal context menu, Action search, shortcut
Settings, and localized reference text.

Adding the binding extends Web's strict device-local shortcut record from version 9 to version 10.
The version-10 reader migrates an exact v9 catalog and defaults, rejects a conflict with the new
default, writes only the canonical v10 record, and preserves v9 unchanged as a rollback snapshot.

## Verification

| Claim                                                                                | Evidence                                                                        | Limit                                                                |
| ------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| Boundary choice, semantic filtering, wrapping, bounds, and source limit              | `src/projected/__tests__/projected-command-output.unit.test.ts`                 | Synthetic render rows and deterministic readers                      |
| Exact snapshot reads, cancellation, stale rejection, and primary-screen admission    | `src/projected/__tests__/projected-terminal.unit.test.ts` plus row-flight model | Mocked Host row windows and the existing bounded general-buffer lane |
| Viewer search, focus, accessibility, copy effect, Actions, binding, and v9 migration | Web controller, component, Action, shortcut, Settings, i18n, and storage tests  | In-process DOM, effects, and device-local storage                    |
| Shortcut and visible controls reach the same production surface                      | `terminal-parity/FS-08-command-output-viewer`                                   | Chromium with synthetic OSC 133 render rows                          |
