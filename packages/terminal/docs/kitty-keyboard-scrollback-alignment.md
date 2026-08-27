# Kitty keyboard-scrollback alignment

## Reference scope

This contract adopts Kitty's main-screen line, page, top, and bottom keyboard-scrollback entry
points from commit `6d5d0c440603ad9bdf6dcd599f73f6dde21acb44`, observed on 2026-08-27. Kitty's
`kitty/options/definition.py` binds `kitty_mod+up`, `kitty_mod+down`, `kitty_mod+page_up`,
`kitty_mod+page_down`, `kitty_mod+home`, and `kitty_mod+end`; `kitty_mod` defaults to `ctrl+shift`.
Its `kitty/screen.c` page operation moves by the visible line count minus one, preserving one row of
context.

The parity claim covers those six primary bindings, their main-screen admission, their direction
and boundaries, and the one-row page overlap. It does not claim Kitty's additional `kitty_mod+j/k`
or macOS Command aliases, multiple bindings per Action, keyboard-driven smooth line animation,
pixel-duration tuning, command-output paging, or alternate-screen application behavior. Orchbee's
single effective binding per Action remains customizable, so an alias can replace a default but
cannot coexist with it in this catalog.

## Domain contract

`ProjectedTerminal` owns the local retained-history viewport. Terminal Host remains authoritative
for retained rows, total-row bounds, generation, revision, and row-window contents. A keyboard
Action can scroll only while `canScrollHistory` is true: the terminal is live, has current
metadata, and is presenting the primary screen. Read-only input capability does not disable local
history reading.

The operations consume the current visible row count and history position:

- line up or down moves by one absolute row;
- page up or down moves by `max(rows - 1, 0)` rows, retaining one context row;
- top requests absolute row zero;
- bottom returns to the live following-bottom projection.

Every admitted movement delegates to the existing bounded history-intent and Host row-window
workflow. Repeated input during a transition or viewport read is coalesced by that owner; it does
not create another scroll queue. The operation returns `true` when it publishes or queues an
intent, and `false` when the viewport cannot move. It never mutates Host history or emits PTY
input.

## Decisions and invariants

| Action ID                           | Apple and Standard default | Viewport transition              |
| ----------------------------------- | -------------------------- | -------------------------------- |
| `terminal.history.scroll-line-up`   | `Ctrl+Shift+Up`            | One row toward retained history  |
| `terminal.history.scroll-line-down` | `Ctrl+Shift+Down`          | One row toward the live bottom   |
| `terminal.history.scroll-page-up`   | `Ctrl+Shift+PageUp`        | One overlapping page upward      |
| `terminal.history.scroll-page-down` | `Ctrl+Shift+PageDown`      | One overlapping page downward    |
| `terminal.history.scroll-top`       | `Ctrl+Shift+Home`          | Absolute retained-history origin |
| `terminal.history.scroll-bottom`    | `Ctrl+Shift+End`           | Live following-bottom projection |

The workflow preserves these invariants:

- Apple uses explicit Control, not the platform `Mod` abstraction, for the six pinned defaults;
- a focused primary-screen terminal consumes an exact effective chord even at a boundary, where
  the Action completes as `unchanged` and the viewport remains stable;
- an alternate-screen or disposed terminal disables all six Actions, so dispatch does not call
  `preventDefault()` and the terminal application can handle the key;
- local history navigation remains available without PTY input authority and publishes no input;
- every movement retires older prompt-navigation commit authority and preserves the existing
  generation, revision, request-sequence, and row-window validation;
- line, page, top, and bottom API navigation is immediate; existing wheel-only smooth presentation
  is neither reused nor expanded by these keyboard Actions;
- top and bottom are idempotent at their respective boundaries, and every amount is bounded by the
  existing history owner before publication.

## Actions and persistence

Workbench owns the six semantic Actions. Web registers them in the focused terminal scope with one
replace-latest history-scroll concurrency key and projects the same binding table into dispatch,
Action search, labels, and Settings. Synchronous availability is checked before execution and key
consumption.

Adding the six shortcut IDs originally extended Web's strict durable keyboard catalog to version 7. The current version-8 writer still reads an exact v7 record against that prior catalog and
defaults before adding later font-size shortcuts; it removes overrides equal to current defaults,
rejects migration conflicts, and leaves v7 unchanged as the rollback snapshot. Invalid current
records do not fall back to older versions.

## Verification

| Claim                                                                | Evidence                                                  | Limit                                                               |
| -------------------------------------------------------------------- | --------------------------------------------------------- | ------------------------------------------------------------------- |
| Line, overlapping-page, top, bottom, boundary, and screen admission  | `src/projected/__tests__/projected-terminal.unit.test.ts` | Deterministic projected-terminal and mocked Host row windows        |
| Action availability, typed results, shared concurrency, and dispatch | Web projected-terminal Action and shortcut unit tests     | In-process Action runtime and synthetic keyboard events             |
| Catalog labels, conflict freedom, customization, and v7 recovery     | Web catalog, Settings, controller, and storage unit tests | Synthetic device-local storage                                      |
| Six physical chords move one production retained-history viewport    | `terminal-parity/HS-11-keyboard-scrollback`               | One real Desktop attachment and Terminal Host on the primary screen |
