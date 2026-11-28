# Sub-Agents and Child Terminals

| Field             | Value                                                                                                                                                                                                                                                                           |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status            | Proposed. On 2026-11-28 the owner chose this design from an interactive draft and will try the draft again before implementation; implementation starts on the owner's word                                                                                                     |
| Owner             | `@orchbee/web` Workbench maintainers; `@orchbee/i18n` maintainers for copy; the Server's Workbench authority maintainers for [closing one child terminal](#closing-one-child-terminal); native TUI maintainers for the [TUI counterpart](#tui-counterpart)                      |
| Current authority | [`apps/web/docs/project-workspace.md`](../../apps/web/docs/project-workspace.md#workspace-agent-overview), [`apps/web/docs/workbench.md`](../../apps/web/docs/workbench.md#panel-workspace), and the Panel entry in [`docs/GLOSSARY.md`](../../docs/GLOSSARY.md#product-scopes) |
| Exit condition    | Deliver the GUI, land or record the TUI counterpart, move maintained behavior into the current authorities and the glossary, then delete this proposal                                                                                                                          |

## Scope

- A Panel's main terminal can run a lead Agent that starts sub-Agents in the Panel's child terminals. This design shows that relationship in the Sidebar, gives every terminal of a Panel its state and name in the Panel's bottom bar, and lets a Workspace with one Tab use its top row for the shown terminal's name and the two ways to add work.
- The Server already holds the relationship: a Panel's main terminal and its ordered child terminals, at most 16 ([Panel lifecycle](../../apps/orchbee/docs/native-panel-lifecycle.md#creating-a-panel-terminal)). The Sidebar, the bottom bar and the title row are Client projections. Apart from [closing one child terminal](#closing-one-child-terminal), Workbench state, Agent detection, states, names, unread and notifications do not change.
- The Tab Strip is the Chromium port in [`@orchbee/tab-strip`](../../packages/tab-strip/README.md), with Tab Groups, pinning and drag. It is drawn whole or not at all, and nothing joins its track.
- No child terminal overview. On 2026-11-28 the owner decided against a fixed screen of terminal cards; if a Panel later gets a summary page, it should be one people define for their own workflow, such as a dashboard.

| Term           | zh-CN       | Meaning                                                                                                                                                                                                                                      |
| -------------- | ----------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Main Agent     | 主 Agent    | The Agent in a Panel's main terminal                                                                                                                                                                                                         |
| Sub-Agent      | 子 Agent    | An Agent in one of that Panel's child terminals                                                                                                                                                                                              |
| Child terminal | 子终端      | A terminal a Panel holds besides its main terminal. Product copy keeps "child" in code today and calls a child Terminal N (终端 N); this design names the add and close actions with the term, so the glossary's Panel entry changes with it |
| Main Agent row | 主 Agent 行 | The Sidebar row of one Panel that holds an Agent in its main terminal or in a child terminal                                                                                                                                                 |
| Status line    | 状态行      | A line of state glyphs that stands in for folded rows                                                                                                                                                                                        |
| Simple mode    | 简洁模式    | How a Workspace with exactly one main Agent row shows that row on the Workspace row                                                                                                                                                          |
| Bottom bar     | 底部条      | The bar along a Panel's bottom edge that lists its terminals while it holds a child terminal; today's `40px` child strip                                                                                                                     |
| Title row      | 标题行      | What the top row shows instead of the Tab Strip while a Workspace has one Tab                                                                                                                                                                |

zh-CN keeps "Agent" in these terms, as the Sidebar's copy does (`{count} 个 Agent`).

## Sidebar

### Main Agent rows

- A Workspace lists one main Agent row per Panel that holds an Agent, in canonical Tab and Panel order, where today it lists one row per Agent. Sub-Agents sit under their main Agent row; a child terminal without an Agent stays out of the Sidebar.
- A main Agent row names the main terminal as today's Agent row names an Agent: state glyph, name, unread dot and provider mark. When the main terminal runs no Agent, the row shows the terminal icon and the terminal's name, and activating it shows that terminal without the Agent check.
- A main Agent row with sub-Agents folds on its own and starts folded. The fold is client-local and lasts while its Panel exists, through Panel moves, a folded Workspace and a hidden Sidebar.
- The row's state glyph is its fold control: while the pointer is over the row or the row has keyboard focus, the glyph turns into a disclosure arrow, pointing right while folded and down while unfolded, and a click on it folds or unfolds the row. A click elsewhere on the row opens the main terminal. A row without sub-Agents has no fold control.
- Unfolded, the sub-Agent rows follow one level deeper, each with today's Agent row content.
- Folded, a status line follows the row: its sub-Agents' state glyphs, most urgent first (blocked, error, done, working, idle, unknown) and otherwise in canonical order, one gap apart. The line starts where the row's name starts, runs to the row's end inset, and wraps onto further lines that start at the same place.
- A click on a status-line glyph opens that sub-Agent's terminal, as its row would; the glyph's tooltip and accessible name are its row's, and a press on it never starts a drag.
- A folded main Agent row carries its sub-Agents' unread dot, and its accessible name adds their states and unread, as a folded Workspace row does.
- The Sidebar's keys treat a main Agent row with sub-Agents as a foldable row: → unfolds it and then moves into its sub-Agent rows; ← folds it and then moves to its Workspace.

### Folded Workspace

- A folded Workspace replaces its second line with one status line per main Agent row, in canonical order. A line holds the main terminal's glyph, which is its Agent's state glyph or else the terminal icon, then a thin vertical divider and the sub-Agents' glyphs, most urgent first. A main Agent row without sub-Agents shows its glyph alone.
- The lines start where the Workspace name starts and run under the end area to the row's end inset. Glyphs that do not fit wrap onto further lines that start after the divider; the next main Agent row starts a new line.
- Glyphs behave as a status line's do. This replaces today's single line of every Agent's glyph, most urgent first.

### Simple mode

- A Workspace with exactly one main Agent row shows that row on the Workspace row. Simple mode is not a setting.
- Unfolded, the Workspace row's second line holds the main terminal's glyph, its name and its type, the provider's product name in quieter text, starting where the Workspace name starts. The branch or folder that line held moves to the name line, after the name, in quieter text that truncates before the name does, and takes its tooltip along. The sub-Agent rows sit directly beneath the Workspace at today's Agent row level. A click on the Workspace row opens the main terminal.
- Folded, the second line is that row's status line without the name: the main terminal's glyph, the divider, then the sub-Agents' glyphs.
- The Workspace's fold control folds the sub-Agent rows. A Workspace whose one main Agent row has no sub-Agent has no fold control.
- A second main Agent row turns the Workspace into the two-level list with every main Agent row folded; dropping back to one returns it to simple mode. Neither change alters the Workspace's own fold.

### Current row

- The row of the shown terminal takes the current fill, as today. When that row is hidden under a folded main Agent row or a folded Workspace, the visible row that holds it takes the current fill and the shown terminal's glyph in its status line carries a short underline.
- In simple mode the Workspace row stands for the main terminal, so it keeps the current fill while the main terminal is shown.

## Bottom bar

- A Panel shows the bottom bar while it holds a child terminal, as today. The bar lists the main terminal first, then a thin divider, each child terminal in canonical order, and a + button.
- Each item shows the terminal's state glyph when it runs an Agent, else the terminal icon, then its name by the Tab's terminal-title function, then the unread dot. A child item also has a close button; the main item has none. The shown terminal's item is selected, a click on an item shows its terminal in the Panel, and a right-click opens the menu of the Sidebar row the item stands for ([Menus](#menus)).
- The bar never scrolls. As browser Tabs do, its items share the bar's width, each up to `164px` and down to `22px`: below `96px` an item drops its close button and below `64px` its name, keeping its glyph and unread dot. The + button follows the last item. When even glyphs do not fit, the trailing items leave the bar for a `+N` item before the + button, which lists them with their glyphs and names; choosing one shows it. The main item and the shown terminal's item always stay on the bar.
- The bar replaces the `32px` header that a shown child adds today. The main item is the way back, so the Back to main terminal action goes. A split Panel's header names a shown child after the main terminal's name, as `Main name › Child name`. In a single-Panel Tab the title row names it while the Workspace has one Tab, and the selected item names it otherwise. Panel options stay in the Panel header and the viewport's menu, which gain New child terminal ▸ ([Menus](#menus)).
- The + button opens the new-Tab menu's choices, its Launch Profiles and then its Agents, and creates the choice as a child terminal of this Panel through `panel.create-terminal` with a `child` target. An Agent starts as the child shell's first command, as it does in a new Tab. With 16 child terminals the button is unavailable and its tooltip says the Panel is full. Today the window creates no child terminal; this button and the title row's `+ Child terminal` are its first ways to.

### Closing one child terminal

Today's Close scope is a Panel with its main terminal and every child terminal ([Tab and Panel close](../../apps/orchbee-server/docs/terminal-tab-close.md)). A child item's close button and a sub-Agent row's Close need a Server close scoped to one child terminal, which asks first when that terminal runs something, as Panel Close asks. The Server's Workbench authority adds that close. Until it does, child items have no close button, and a sub-Agent row's menu leaves Close out, because today's Close would also end the main Agent.

## Title row

- While a Workspace has exactly one Tab, the top row draws no Tab. After the Sidebar control it shows the focused Panel's shown terminal: its glyph or the terminal icon, its name, and for an Agent its provider's product name in quieter text, preceded by `Main name ›` while a child terminal is shown. `+ Child terminal` and `+ Tab` follow, then the trailing actions and the room the window controls keep, as in the strip.
- `+ Child terminal` opens the bottom bar's + choices for the focused Panel. `+ Tab` opens the new-Tab menu, whose first choice keeps its shortcut.
- A right-click on the shown name opens the hidden Tab's menu, so Rename, pinning, grouping and Close stay reachable.
- From the second Tab the row is today's Tab Strip, unchanged; closing back to one Tab returns the title row. The bottom bar stays where it is through both changes.
- A Workspace with no Tab keeps today's strip.

## Menus

Each menu keeps today's rows, order and conditions except as listed.

| Menu           | Opens from                                                                                                      | Change                                                                                                                                                                                                                                                                                             |
| -------------- | --------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Panel menu     | The Panel header's options button and right-click, and below the terminal's own commands in the viewport's menu | Adds New child terminal ▸ (新建子终端) after Split down. It lists the bottom bar's + choices for this Panel and is unavailable, with the full-Panel reason, at 16 child terminals. The pane menu has no GUI-only row, so the TUI pane menu gains the row too ([TUI counterpart](#tui-counterpart)) |
| Main Agent row | The row, simple mode's unfolded second line, and the bottom bar's main item                                     | Rows unchanged: Rename, Allow confirmations ▸, then Close, which still ends the whole Panel with its child terminals and asks first as today. The second line and the main item are new places                                                                                                     |
| Sub-Agent row  | The row and a child item in the bottom bar                                                                      | Close closes only that child terminal ([Closing one child terminal](#closing-one-child-terminal)), and until the Server's close lands the row is left out. A child terminal without an Agent has Close alone, so its item opens no menu until then                                                 |
| Tab menu       | A Tab, and the title row's name                                                                                 | Rows unchanged; the title row's name is a new place                                                                                                                                                                                                                                                |
| Workspace menu | The Workspace row                                                                                               | Unchanged. Status-line glyphs have no menu of their own: a right-click on a status line opens the menu of the row it belongs to, the Workspace's for a folded Workspace and the main Agent row's for a folded main Agent row                                                                       |

The bottom bar's + and the title row's `+ Child terminal` open the new-Tab menu's choices for a child terminal of the focused Panel, `+ Tab` opens the new-Tab menu unchanged, and the bottom bar's `+N` lists the terminals that left the bar.

## Unchanged

- Outside simple mode's two lines, Workspace rows keep their layout: no leading icon, the fold chevron after the name, and Options and Close as their tools.
- Agent rows gain no hover tools; their actions stay in the right-click menu.
- The Tab Strip keeps its Tab Groups, pinning, drag and menus.

## TUI counterpart

The owner asked for the GUI first. The TUI keeps its flat Agent rows, folded-Workspace status line, Tab strip and [Presentation selector](../../apps/orchbee/docs/native-mouse-input.md#presentation-selector) until its counterpart lands.

- Missing: main Agent rows and status lines, simple mode, the name and state of each terminal in a Panel, the title row, and the pane menu's New child terminal ▸. Until the pane menu row lands, the GUI's row is a staged difference from the shared composition.
- Owner: native TUI maintainers in `apps/orchbee`.
- Completion: the TUI Sidebar draws the same rows and status lines in cells, with the same glyphs, a `│` divider and the same wrap start; its Panel control names each terminal with its state; its one-Tab strip follows the title row rule. Any difference needs a recorded reason the owner accepts.

## Open decisions

- Should Start Agent's Opens in (打开位置) offer a child terminal as a place?
- Which default keys, if any, switch between a Panel's terminals? Neither Client has one today.

## Verification

- Browser evidence in both themes and both locales for: a Workspace with one main Agent row, with and without sub-Agents, folded and unfolded; a Workspace with two main Agent rows, folded and unfolded, whose status lines wrap; a shown sub-Agent under a folded row; a bottom bar with 1, 4, 7 and 16 child terminals in a single Panel and in three side-by-side Panels, showing names, glyph-only items and the `+N` item without scrolling; a Workspace going from one Tab to two and back.
- Each menu in [Menus](#menus) opened from each place it opens, with its rows in both locales.
- The Sidebar's keys on main Agent rows, and the keyboard returning to the terminal after each bottom bar, title row and menu action.
- The [local gate](../../docs/VERIFICATION.md#local-gate).
