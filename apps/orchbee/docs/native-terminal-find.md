# Native Find in terminal

## Scope and ownership

Find in terminal searches the focused Panel's retained output from a one-row bar over the Panel.
This reference owns its entry points, bar states, keys, closing rules, special Panels and its
differences from the web and desktop Find. It follows [keyboard input](../../../docs/tui/keyboard-input.md)
and [overlays and status](../../../docs/tui/overlays-and-status.md).

`WorkbenchClientUi` holds the bar as a copy episode with the Find origin, so it shares copy mode's
capture slot, pane fence and single-flight query lane, and never runs beside copy mode. The
deterministic `terminal_find` core owns the query draft, Match case, the search delay, the one
search it still expects and the last settled matches. `ClientView` only mirrors whether the copy
slot holds the bar. Reads go through `terminal_copy_query` on the independent read-only content
lane, so the bar has no terminal input, selection or mutation capability. Nothing is persisted
except, per Client until it exits, the last query and Match case.

## Entry points

| Entry                                      | Result                                                                   |
| ------------------------------------------ | ------------------------------------------------------------------------ |
| `Ctrl+b f`                                 | Opens the bar on the focused Panel                                       |
| `[keys] find`                              | Replaces or disables that entry; a safe direct chord uses the same route |
| `/` in an Entering or Viewing history view | Opens the same bar; nothing reaches the shell                            |

Shortcut help names the action "Find in terminal" / "在终端中查找". Opening while the bar is open
keeps the bar and its query. Opening while keyboard copy mode is active keeps copy mode. A Panel
without a connected frame refuses with the notice "Terminal disconnected" and sends nothing.

The bar opens with the remembered query selected, so typing replaces it, and searches it at once.
Opening never changes the Panel's selection or scroll position: a live Panel stays live and keeps
painting.

## The bar

The bar covers the first row of the Panel's content, right-aligned, `surface-overlay`, width
`min(48, Panel width - 2)`. It never resizes the terminal. From the start: the label `Find:` /
`查找:`, the draft holding the terminal cursor, the counter, then `Aa`, previous `[^]`, next `[v]`
and close `[x]`. When the full bar does not fit, the label, the draft and the counter remain. The
controls are drawn without pointer routing; their keys act.

| State      | Counter cell | Other                                                                                           |
| ---------- | ------------ | ----------------------------------------------------------------------------------------------- |
| Empty      | blank        | No read is made; previous and next are disabled                                                 |
| Searching  | `…`          | The previous highlights stay until the result arrives                                           |
| Matches    | `3/17`       | Active match painted `surface-selection`, other visible matches `surface-related`               |
| No matches | `0/0`        | Previous and next are disabled; the view does not move                                          |
| Time limit | blank        | Notice "Search stopped after reaching the time limit" (short: Search stopped)                   |
| Read error | blank        | Notice "Retained terminal output could not be searched" (short: Search failed); the query stays |

The status chip shows the indicator `FIND` / `查找` with the hints `Esc` Close and `Enter` Next.
A Find notice leads the chip while it shows, and the prefix indicator replaces `FIND` while the
prefix is pending.

## Search

Search is literal over the whole retained scrollback, wrapped rows joined into logical lines. It
is case-insensitive unless Match case is on; Find never uses smart case. The total is exact and up
to 1024 matches around the active one stay settled for stepping and painting. Each search is
bounded by the copy query lane's five-second job budget.

A search runs 150 ms after the last edit; a newer edit or Match case change cancels the search in
flight. The first match is the newest one. When the active match is not visible it is revealed
centred: a live primary Panel enters its history at that window, and a history view moves to it.
An alternate screen shows everything it retains and never moves. When the Panel's content
generation, screen or geometry changes, the bar keeps the query, drops its matches and searches
again. New output does not search again; the counter refreshes on the next search.

## Keys

| Input                                                       | Result                                         |
| ----------------------------------------------------------- | ---------------------------------------------- |
| Printable keys, paste                                       | Edit the query (512 characters)                |
| Left, Right, Home, End, Backspace, Delete, line-editor keys | Edit the draft                                 |
| `Enter`, `Down`                                             | Next match, toward newer, wrapping             |
| `Shift+Enter` where the host sends it, `Up`                 | Previous match, toward older, wrapping         |
| `Alt+c`                                                     | Toggle Match case and search again             |
| `PageUp`, `PageDown`                                        | Scroll the history; the bar and the query stay |
| `Esc`                                                       | Close                                          |
| The prefix                                                  | Stays live; prefix actions run over the bar    |

The bar is an exclusive keyboard capture: nothing typed reaches the program in the Panel, which
keeps running and painting. Keys match through the standard bindings, so Enter delivered as CR
counts as Enter, and a host that sends Shift+Enter as plain Enter steps to the next match. Pointer
input over the bar and the Panel is swallowed while the bar is open.

## Closing

`Esc` closes the bar and removes its highlights. The Panel stays where Find scrolled it, as an
ordinary history view; if Find never moved it, the Panel is exactly as before. The bar also closes
when its Panel loses focus, its context changes or the Client retires.

## Differences from the web and desktop Find

| Web and desktop                         | Native Client                                   | Host reason                                                 |
| --------------------------------------- | ----------------------------------------------- | ----------------------------------------------------------- |
| `Ctrl+Shift+F` / `Cmd+F` opens Find     | `Ctrl+b f`; direct chords only by configuration | Terminal hosts do not reliably deliver the GUI chord        |
| Up and Down recall earlier queries      | Up and Down step through matches                | The bar's arrows are the only visible meaning of those keys |
| Whole word and regular expression       | Literal search with Match case only             | One row of cells holds one toggle                           |
| Ten-second search limit                 | Five-second job budget                          | The shared copy query lane owns the budget                  |
| Error row inside the bar                | Status notice with the same sentence            | The bar is one row                                          |
| Clicks on the bar controls              | Keys only                                       | Pointer routing for the bar is not admitted yet             |
| Bar stays open while content is clicked | Pointer input is swallowed while open           | One keyboard owner, always indicated                        |
