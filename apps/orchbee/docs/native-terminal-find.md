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
| Panel menu row "Find in terminal"          | Focuses that Panel, then opens the bar on it                             |
| Global menu (`Ctrl+b F2`) row, Panel group | Opens the bar on the focused Panel                                       |

Shortcut help and both menu rows name the action "Find in terminal" / "在终端中查找"; the rows sit
between Copy selection and Clear terminal in the Panel menu and before copy mode in the global
menu, show the effective shortcut, and are found by find, search / 查找, 搜索. Opening while the bar
is open keeps the bar and its query. Opening while keyboard copy mode is active keeps copy mode. A
Panel without a connected frame refuses with the notice "Terminal disconnected" and sends nothing;
both menu rows are disabled by the same rule with that reason. A Panel narrower than 20 columns or
shorter than 2 rows refuses with the notice "Panel too small" / "面板太小".

The bar opens with the remembered query selected, so typing replaces it, and searches it at once.
Opening never changes the Panel's selection or scroll position: a live Panel stays live and keeps
painting.

## The bar

The bar covers the first row of the Panel's content, right-aligned, `surface-overlay`, width
`min(48, Panel width - 2)`; in a Panel under 40 columns it spans the whole row. It never resizes
the terminal. From the start: the label `Find:` / `查找:`, the draft holding the terminal cursor,
the counter, then `Aa`, previous `[^]`, next `[v]` and close `[x]`. A Panel under 40 columns drops
`Aa`, and one under 28 columns also drops `[^] [v]`; the label, the draft and the counter always
remain. A Panel that shrinks while the bar is open keeps it open at the narrower tier.

A plain left release on a control acts as its key: `Aa` toggles Match case, `[^]` and `[v]` step,
`[x]` closes. Controls take the shared hover and pressed states; disabled previous and next take
neither. The bar shows no tooltips, like the other mode bars of the native Client.

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
to 1024 matches around the active one stay settled for stepping and painting. A search covers
every row the Panel retains: it reads, searches and drops the rows page by page at one content
revision, so the length of the scrollback never makes it fail. The copy query lane's five-second
job budget is what ends a search that has not finished.

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
counts as Enter, and a host that sends Shift+Enter as plain Enter steps to the next match.

## Pointer

| Input while the bar is open                    | Result                                                     |
| ---------------------------------------------- | ---------------------------------------------------------- |
| Plain left release on a control                | The control acts                                           |
| Press on the bar off its controls              | Nothing; the bar stays                                     |
| Wheel over the Panel or the bar                | Scrolls the history three rows; the bar and the query stay |
| Plain press on the Panel's content             | Closes the bar; the press and its release reach no program |
| Press on another Panel, the Tab strip, sidebar | Acts as it always does; the bar closes if focus moves      |

A modified press on the Panel's content reaches no program and keeps the bar open.

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
| Bar stays open while content is clicked | A press on the content closes the bar           | One keyboard owner, always indicated                        |
| Control tooltips                        | None                                            | Native mode bars show no tooltips                           |
