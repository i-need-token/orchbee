# Scrollback viewer

## Scope

One focused-terminal Action opens a frozen view of retained primary-screen text at the visible top with `Ctrl+Shift+H`. The application-owned viewer supports literal search, native text selection, copy, cancellation and focus restoration. External pager/editor processes, ANSI styling, pipes, a second history store, history beyond Host retention and simultaneous terminal input are excluded.

## Change authorization and ownership

This is a modeling-tier compatibility change accepted by the product owner through the ordered
terminal-parity implementation request. Terminal Host remains authoritative for VT state and
retained rows. `@orchbee/terminal` owns exact snapshot preparation. Workbench owns semantic Action
identifiers. Web owns the private viewer, shortcut projection, localized presentation, and
device-local shortcut migration. The application-owned clipboard capability remains the only
clipboard writer.

The accepted outcome is a complete immutable plain-text view of one retained snapshot. Non-goals
are the excluded pager capabilities above, changing retention, mutating startup files,
scrolling or selecting the live terminal, and adding another modal interaction implementation.

## Domain model

`ProjectedTerminal.readScrollback()` is the only public preparation operation. Its inputs are one
`AbortSignal`, the mounted terminal's live primary-screen metadata, its currently presented visible
top, and one exact Terminal Host generation/revision/total-row snapshot. Missing metadata,
disposal, and the alternate screen are unavailable rather than empty.

The operation reads text-detail rows oldest-first through the existing general-buffer lane. At most
one page is in flight. Every accepted page must match the captured generation, revision, total-row
count, requested start, text detail, consecutive row identities, and remaining byte allowance.
The page size is bounded by protocol row and cell limits. The aggregate encoded source allowance is
16 MiB; exhaustion, including a single-row overflow, returns `source-limit` and never publishes a
prefix.

The deterministic projector produces rows in ascending absolute order:

1. Every retained row contributes its trimmed Host-projected plain text, including an empty row.
2. A Ghostty soft-wrapped row is joined directly to its successor; every other boundary becomes a
   line feed.
3. `initialTopOffset` is the UTF-16 offset immediately before the row that was visibly at the top
   when preparation began. It is clamped to the retained snapshot and never inferred by searching
   text.
4. A zero-row snapshot produces `empty/no-scrollback`. A nonzero snapshot produces `scrollback`
   even when its projected text is empty, because retained blank rows are an inspectable snapshot.

`ProjectedScrollbackReadResult` is exactly one of `scrollback`, `empty`, `cancelled`, or
`unavailable`. A scrollback result carries generation, revision, inclusive row bounds, plain text,
and `initialTopOffset`. Stale or evicted pages map to `unavailable/stale`; malformed pages,
transport failure, disposal, or unsupported state map to a typed unavailable reason. After the
last page, `ProjectedTerminal` rechecks screen admission and the exact snapshot before returning a
publishable result. Cancellation and every failure leave Terminal Host, the live viewport,
selection, attachment identity, and any previously open viewer document unchanged.

## Viewer and Action model

Web owns one private terminal-text viewer per projected terminal mount. Its document is a tagged
immutable union whose source is `command-output` or `scrollback`; source-specific adapters convert
terminal read results before opening it. One viewer controller owns reversible query, active-match,
open, and focus-request state. Opening either source replaces the previous document and resets
search. Closing restores terminal focus without changing Host state.

Literal search remains case-insensitive, non-overlapping, ordered by UTF-16 source offset, limited
to 1,024 query characters and 1,000 rendered matches, and independent of live-buffer search. The
scrollback document renders an inert anchor at `initialTopOffset` and scrolls that anchor to the top
on initial presentation. Enter and Shift+Enter cycle matches, `Mod+F` focuses the query, Escape
closes, and the modal focus cycle remains contained. Copy invokes the source-specific semantic
Action and writes exactly the immutable plain text through the application clipboard capability.

Workbench owns `terminal.history.show` and `terminal.history.copy` in addition to the existing
command-output Actions. Web registers both in the mounted terminal scope. Their viewer-local
latest-preparation coordinator cancels the older general-buffer read when either Show Action starts;
each Action's replace-latest policy independently handles repetition of that same Action. A handler
checks cancellation before closing live Find and replacing the viewer document. Scrollback Copy is
enabled only for an open scrollback text document and an available clipboard writer.

## Binding and persistence

Apple and Standard platforms both default Show Scrollback to `Ctrl+Shift+H`. The binding applies only
to a focused terminal and is projected into the terminal context menu, Action search, shortcut
Settings, and localized reference text. Viewer-owned focus suppresses terminal shortcuts.

Adding the binding extends Web's strict device-local shortcut record from version 10 to version 11.
The version-11 reader migrates an exact v10 catalog and defaults, rejects a conflict with the new
default, writes only the canonical v11 record, and preserves v10 unchanged as a rollback snapshot.

## Correctness and verification

No new concurrency primitive is introduced. Controlled row-flight and Action runtime tests cover exact single-flight reads, stale responses and replace-latest cancellation before publication. The viewer coordinator reuses the AbortController handoff from prompt navigation and has cross-source replacement/disposal cases. Projector/controller logic has no time, process, network, storage or DOM dependency; focus and anchor placement remain adapter evidence. These bounded implementation traces do not claim exhaustive TLA assurance or a Lean theorem.

| Claim                                                                                       | Evidence                                                                        | Limit                                                        |
| ------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------- | ------------------------------------------------------------ |
| Ordered row coverage, wrap joining, exact anchor offset, page validation, and source limit  | `src/projected/__tests__/projected-scrollback.unit.test.ts`                     | Synthetic text rows and deterministic readers                |
| Primary-screen admission, visible-top capture, cancellation, and final stale rejection      | `src/projected/__tests__/projected-terminal.unit.test.ts` plus row-flight model | Mocked Host row windows and the existing general-buffer lane |
| Tagged viewer replacement, search, anchor, focus, copy, Actions, binding, and v10 migration | Web controller, component, Action, shortcut, Settings, i18n, and storage tests  | In-process DOM, effects, and device-local storage            |
| Shortcut and visible controls reach the same production viewer                              | `terminal-parity/FS-09-scrollback-viewer`                                       | Chromium with a bounded synthetic retained-history snapshot  |
