# Keyboard shortcuts

`@orchbee/web` owns client-local overrides for a focused catalog of frequent semantic shortcuts.
Workbench and projected-terminal features continue to own their Action inputs and platform
defaults. The Web composition combines those declarations, validates one complete effective table,
and injects that same source into the mounted Workbench and projected terminals. Menus, tooltips,
the Action search surface, and shortcut settings project labels from that effective table; none of
those surfaces owns another binding copy.

## Initial catalog

| Shortcut ID                                   | Scope                             | Apple default            | Standard default       |
| --------------------------------------------- | --------------------------------- | ------------------------ | ---------------------- |
| `workbench.search.actions.open`               | Workbench                         | `Cmd+Shift+P`            | `Ctrl+Shift+P`         |
| `workbench.focus.next-region`                 | Workbench                         | `Cmd+F6`                 | `Ctrl+F6`              |
| `workbench.focus.previous-region`             | Workbench                         | `Cmd+Shift+F6`           | `Ctrl+Shift+F6`        |
| `workbench.terminal.create`                   | Workbench                         | `Cmd+T`                  | `Ctrl+Shift+T`         |
| `workbench.terminal.create-current-directory` | Active terminal                   | `Option+Cmd+Enter`       | `Ctrl+Alt+Enter`       |
| `workbench.terminal.font-size.increase`       | Workbench                         | `Cmd+=`                  | `Ctrl+=`               |
| `workbench.terminal.font-size.decrease`       | Workbench                         | `Cmd+-`                  | `Ctrl+-`               |
| `workbench.terminal.font-size.reset`          | Workbench                         | `Cmd+0`                  | `Ctrl+0`               |
| `workbench.tab.select.1` … `.9`               | Workbench                         | `Cmd+1` … `Cmd+9`        | `Ctrl+1` … `Ctrl+9`    |
| `workbench.tab.select-previous`               | Workbench                         | `Cmd+Shift+[`            | `Ctrl+PageUp`          |
| `workbench.tab.select-next`                   | Workbench                         | `Cmd+Shift+]`            | `Ctrl+PageDown`        |
| `workbench.tab.active.move-backward`          | Workbench                         | `Ctrl+Shift+,`           | `Ctrl+Shift+,`         |
| `workbench.tab.active.move-forward`           | Workbench                         | `Ctrl+Shift+.`           | `Ctrl+Shift+.`         |
| `workbench.tab.title.set`                     | Workbench                         | `Cmd+Shift+I`            | `Ctrl+Alt+Shift+T`     |
| `workbench.agent.start`                       | Workbench                         | `Cmd+Shift+G`            | `Ctrl+Alt+Shift+G`     |
| `workbench.terminal.active.close`             | Active terminal                   | `Cmd+W`                  | `Ctrl+Shift+W`         |
| `workbench.panel.active.focus-down`           | Active panel                      | `Cmd+Option+Down`        | `Ctrl+Alt+Down`        |
| `workbench.panel.active.focus-left`           | Active panel                      | `Cmd+Option+Left`        | `Ctrl+Alt+Left`        |
| `workbench.panel.active.focus-next`           | Active panel                      | `Ctrl+Shift+]`           | `Ctrl+Shift+]`         |
| `workbench.panel.active.focus-previous`       | Active panel                      | `Ctrl+Shift+[`           | `Ctrl+Shift+[`         |
| `workbench.panel.focus-last`                  | Workbench                         | `Ctrl+Shift+;`           | `Ctrl+Shift+;`         |
| `workbench.panel.active.focus-right`          | Active panel                      | `Cmd+Option+Right`       | `Ctrl+Alt+Right`       |
| `workbench.panel.active.focus-up`             | Active panel                      | `Cmd+Option+Up`          | `Ctrl+Alt+Up`          |
| `workbench.panel.active.move-down`            | Active panel                      | `Cmd+Option+Shift+Down`  | `Ctrl+Alt+Shift+Down`  |
| `workbench.panel.active.move-left`            | Active panel                      | `Cmd+Option+Shift+Left`  | `Ctrl+Alt+Shift+Left`  |
| `workbench.panel.active.move-right`           | Active panel                      | `Cmd+Option+Shift+Right` | `Ctrl+Alt+Shift+Right` |
| `workbench.panel.active.move-up`              | Active panel                      | `Cmd+Option+Shift+Up`    | `Ctrl+Alt+Shift+Up`    |
| `workbench.panel.active.equalize`             | Active panel                      | `Ctrl+Cmd+=`             | `Ctrl+Alt+=`           |
| `workbench.panel.active.maximization.toggle`  | Active panel                      | `Cmd+Shift+Enter`        | `Ctrl+Shift+Enter`     |
| `workbench.panel.active.resize`               | Active panel                      | `Cmd+R`                  | `Ctrl+Shift+R`         |
| `workbench.panel.active.split-right`          | Active panel                      | `Cmd+D`                  | `Ctrl+Shift+D`         |
| `workbench.panel.active.split-below`          | Active panel                      | `Cmd+Shift+D`            | `Ctrl+Shift+E`         |
| `workbench.search.all.open`                   | Workbench                         | `Cmd+Shift+F`            | `Ctrl+Shift+K`         |
| `workbench.settings.open`                     | Workbench                         | `Cmd+,`                  | `Ctrl+,`               |
| `workbench.shortcuts.open`                    | Workbench                         | `Cmd+/`                  | `Ctrl+Shift+/`         |
| `workbench.workspace.create`                  | Workbench                         | `Cmd+Shift+N`            | `Ctrl+Shift+N`         |
| `workbench.sidebar.toggle`                    | Workbench                         | `Cmd+B`                  | `Ctrl+Shift+B`         |
| `terminal.search.open`                        | Focused terminal or Find          | `Cmd+F`                  | `Ctrl+Shift+F`         |
| `terminal.hints.open`                         | Focused terminal                  | `Cmd+Shift+O`            | `Ctrl+Shift+O`         |
| `terminal.selection.copy`                     | Focused terminal with a selection | `Cmd+C`                  | `Ctrl+Shift+C`         |
| `terminal.clipboard.paste`                    | Focused terminal                  | `Cmd+V`                  | `Ctrl+Shift+V`         |
| `terminal.history.prompt-previous`            | Focused terminal                  | `Ctrl+Shift+Z`           | `Ctrl+Shift+Z`         |
| `terminal.history.prompt-next`                | Focused terminal                  | `Ctrl+Shift+X`           | `Ctrl+Shift+X`         |
| `terminal.history.scroll-line-up`             | Focused terminal                  | `Ctrl+Shift+Up`          | `Ctrl+Shift+Up`        |
| `terminal.history.scroll-line-down`           | Focused terminal                  | `Ctrl+Shift+Down`        | `Ctrl+Shift+Down`      |
| `terminal.history.scroll-page-up`             | Focused terminal                  | `Ctrl+Shift+PageUp`      | `Ctrl+Shift+PageUp`    |
| `terminal.history.scroll-page-down`           | Focused terminal                  | `Ctrl+Shift+PageDown`    | `Ctrl+Shift+PageDown`  |
| `terminal.history.scroll-top`                 | Focused terminal                  | `Ctrl+Shift+Home`        | `Ctrl+Shift+Home`      |
| `terminal.history.scroll-bottom`              | Focused terminal                  | `Ctrl+Shift+End`         | `Ctrl+Shift+End`       |
| `terminal.selection.select-all`               | Focused terminal                  | `Cmd+A`                  | `Ctrl+Shift+A`         |
| `terminal.clear`                              | Focused terminal                  | `Cmd+K`                  | `Ctrl+Shift+L`         |

No distribution offers `workbench.search.all.open`; see [release-withheld Search](workbench.md#release-withheld-search).

The application registers separate defaults for new Tab, active-terminal close, Panel maximization, splits, paste and Tab navigation. Apple All search uses `Cmd+Shift+F`, leaving
the familiar `Cmd+K` to clear only the focused terminal. Standard Clear uses `Ctrl+Shift+L`;
Standard defaults deliberately leave unshifted `Ctrl+B/T/W/K/F/D/E/V/A/L` and `Ctrl+C` available
to the shell, terminal applications, and multiplexers. A user may deliberately record another
supported custom chord. Product conflicts remain prohibited even when an Action is temporarily
unavailable.

New Workspace and the shortcut reference carry the TUI's `Ctrl+B Shift+N` and `Ctrl+B ?` with the platform modifier: `Cmd+Shift+N` and `Ctrl+Shift+N` create a Workspace as the sidebar's New workspace row does, where the focused Panel stands, and `Ctrl+Shift+/` opens Settings on the shortcut reference, as the sidebar's Shortcuts button does. Apple takes `Cmd+/` because `Cmd+?` opens the system's Help search. The sidebar toggle stays `Cmd+B` on Apple but `Ctrl+Shift+B` on Standard, where `Ctrl+B` is the TUI's prefix and readline's backward-char. A browser that keeps `Ctrl+Shift+N` for its own private window never delivers it to the page; Desktop receives it, and the row and Search still create a Workspace.

Prompt navigation uses physical `Ctrl+Shift+Z` and `Ctrl+Shift+X`. Line, page, top and bottom history navigation uses `Ctrl+Shift+Arrow/Page/Home/End`, with one row of page overlap. Each Action has one customizable effective binding. The terminal package owns [Prompt navigation](../../../packages/terminal/docs/prompt-navigation.md) and [Keyboard scrollback](../../../packages/terminal/docs/keyboard-scrollback.md).

Application font increase, decrease and reset use `Mod+=`, `Mod+-` and `Mod+0`; Mod is Command on Apple and Control on Standard. Visible keyboard hints use `Mod+Shift+O`, leaving Standard `Ctrl+Shift+E` for Split Below. [Keyboard hints](../../../packages/terminal/docs/keyboard-hints.md) owns targets, effects and cancellation.

Start Agent uses Apple `Cmd+Shift+G` and Standard `Ctrl+Alt+Shift+G`, Rename Tab's chord shape with G for Agent. It opens the dialog only while the Session can start an Agent.

Find in terminal uses Apple `Cmd+F` and Standard `Ctrl+Shift+F`, the counterpart of the TUI's `Ctrl+B F`. The keys inside the Find surface are fixed, not catalog entries, and read the same in both clients: Enter and Down go to the next match, Shift+Enter and Up to the previous one, Escape closes, and `Alt+C` toggles Match case. `Alt+W` and `Alt+R` toggle Match whole word and Use regular expression, which the TUI does not have. Apple uses `Option+Cmd+C`, `Option+Cmd+W` and `Option+Cmd+R` for the toggles because Option with a letter types a character there.

Last Panel uses `Ctrl+Shift+;` on both platforms, the TUI's `Ctrl+B ;` with the Panel-focus family's `Ctrl+Shift`. It returns to the most recent other Panel this window focused that still exists, in any Tab or Workspace, and falls through when there is none. Active-Tab movement uses `Ctrl+Shift+,` and `Ctrl+Shift+.`. Cyclic Panel focus uses `Ctrl+Shift+]` and `Ctrl+Shift+[` in canonical depth-first leaf order within the current Tab, wrapping with at least two Panels. Positional focus uses `Ctrl+Shift+1` through `Ctrl+Shift+0`, where `0` means ten; missing positions never clamp. Apple `Command+digit` remains Tab selection.

Visible Panel focus and swap use `Ctrl+Shift+F7` and `Ctrl+Shift+F8`. Two Panels select the other Panel immediately; larger layouts use a source-excluding alphanumeric selector. Workbench owns order, multi-character labels, stale cancellation and Action delegation. Rename Tab uses Apple `Shift+Cmd+I` and Standard `Ctrl+Shift+Alt+T`, sharing one semantic Action with the Tab menu's Rename row and Action search.

Resize Panel uses Apple `Cmd+R` and Standard `Ctrl+Shift+R`. The modal uses `W` wider, `N` narrower, `T` taller, `S` shorter, `R` reset and Escape exit. Steps are five percentage points, doubled by Control, retaining at least five percent per branch; reset gives every split equal child weights.

Equalize Panels uses Apple `Control+Command+=` and Standard `Ctrl+Alt+=`. Same-axis nested Panels receive equal normalized extents; a perpendicular subtree counts as one parent-axis unit and is recursively equalized on its own axis. This differs from resize mode's `R`, which gives each binary split equal child weights.

## Terminal paste routing

The platform-default `terminal.clipboard.paste` chord has a native-event fallback because
`@orchbee/terminal` owns the MIME-aware DOM `paste` contract: plain text is authoritative when
present, while an image-only event emits exactly one synthetic `Control+V` press/release pair for
the running Agent without reading image bytes. A custom paste chord cannot cause that browser
event and therefore invokes the Paste Action.

The terminal's hidden textarea remains the sole browser-editing and IME owner while an ordinary
Host selection is active; Canvas and accessibility descriptions project that selection without
creating a document-global DOM Range. Native paste may arrive at the textarea or at a
package-owned terminal-presentation descendant, so the terminal root provides a bounded fallback
that runs the shared paste path exactly once. Application-owned descendants such as the Find input
retain their own native editing behavior and are never consumed by that fallback.

The projected-terminal clipboard Action coordinator owns whether one exact selection Copy or Paste
Action is pending. Shortcut resolution combines the normalized chord with that synchronous state:

| Effective paste chord | Selection Copy | Paste Action | Route                                                               |
| --------------------- | -------------- | ------------ | ------------------------------------------------------------------- |
| Platform default      | Idle           | Idle         | Preserve the key event default for one native `paste` event         |
| Platform default      | Pending        | Any          | Invoke Paste Action and wait for that exact Copy                    |
| Platform default      | Idle           | Pending      | Invoke Paste Action so its concurrency policy rejects the duplicate |
| Custom                | Any            | Any          | Invoke Paste Action                                                 |

An Action route consumes the key only while the Action is enabled; otherwise browser or terminal
behavior remains available. A selection Copy remains pending until its exact Promise settles. The
Paste Action snapshots and awaits that Promise before reading current clipboard text, rechecks
cancellation, and performs no retry. Copy failure retains its own result and Paste reads the current
clipboard after settlement. The Paste Action remains pending through preparation and terminal
publication so a second default chord cannot escape through the native path and duplicate input.
Context-menu Paste always uses the Action route.

Each admitted shortcut takes exactly one route. Native routing invokes no Action, reads no
clipboard text, produces no Action status, and delegates text/image precedence to the
terminal adapter. Action routing never reads image bytes or more than
bounded plain text.

Directional Panel focus uses Apple `Command+Option+Arrow` and Standard `Control+Alt+Arrow`. The binding
selection is the only application-level concern here; Workbench owns spatial neighbor resolution.

Directional Panel movement adds Shift to the existing focus chords and uses the current weighted-rectangle neighbor. It swaps leaves while preserving source focus and never wraps at an outer edge.

Commands without an effective binding do not render an empty shortcut affordance. Reset,
split-left, and split-above remain intentionally unbound in this catalog.
Adding nullable or user-created bindings for initially unbound commands is outside this model.

## Workbench dispatch scope

Web publishes whether Workbench is the active product surface. The mounted Workbench owns one
capture listener on its document and participates only while that semantic scope is active. Focus
inside a terminal, Tab strip, sidebar control, Workbench interaction surface, or the document body
does not change Workbench binding applicability. Opening the full-screen Settings surface makes the
still-mounted Workbench inactive; closing Settings activates it again. Unmounting Workbench removes
the listener.

| Workbench surface state      | Binding decision                                                                               |
| ---------------------------- | ---------------------------------------------------------------------------------------------- |
| Active with one or more Tabs | Resolve Workbench bindings independently of the focused Workbench region                       |
| Active with no Tabs          | New Tab remains enabled; current-directory creation, close, and Panel commands are unavailable |
| Settings is open             | Skip every Workbench binding and preserve event defaults                                       |
| Workbench is unmounted       | No Workbench listener or binding participates                                                  |

A resolved binding still runs only when its Action is enabled. Successful New Tab execution makes
the new Tab active and projects its first Panel terminal as the sole `autoFocus` target; the
injected terminal surface claims real input when ready. Active-terminal and active-Panel commands
resolve from the active Tab's remembered focused Panel rather than the keyboard event target, so a
sidebar or document-body focus does not redirect them. An unavailable Action preserves state and
does not call `preventDefault()`.

While a visible Panel selector is active, its workspace root owns DOM focus and publishes an
exclusive keyboard-capture marker. The document-level Workbench shortcut listener skips events
inside that marker, then the workspace consumes every physical label, Backspace, Escape,
composition, repeat, modifier-held, and invalid key event. This prevents recursive shortcut
dispatch and terminal input while leaving outside-pointer cancellation available to the intended
destination.

`Mod+1` through `Mod+9` invoke `workbench.tab.select-by-position` with the matching one-based
position. Position is resolved from the active terminal context's canonical `TabModel.tabs` order:
the pinned prefix comes first, followed by the current grouped and ungrouped order. Tabs inside a
collapsed group still occupy positions, and the delegated `tabs.tab.select` Action expands the
selected Tab's group through the ordinary selection transition. A missing position is unavailable
and leaves the event untouched. `Mod+9` means the ninth Tab, never the last Tab.

`Ctrl+Shift+1` through `Ctrl+Shift+9`, plus `Ctrl+Shift+0`, invoke
`workbench.panel.active.focus-by-position` with exact semantic positions one through ten. The
position is resolved from the active Tab's canonical depth-first Panel-leaf order. An existing
target delegates to the ordinary Panel focus Action; the already-focused target is an intentional
available `unchanged` result and still consumes the event. A missing position, maximized layout,
stale source, invalid order, or missing lower Action is unavailable and leaves the event untouched.

### Current-directory Tab creation model

- **Problem and input:** A person needs a new Tab rooted in the directory of the terminal they are
  actually viewing. An invocation carries either no target, which resolves the active Tab's focused
  Panel, or one exact Panel target.
- **Data and ownership:** Workbench captures the displayed primary or child Presentation identity
  and reads its latest trusted non-empty `currentCwd`. The Server creates the Tab through `authority.createTab`; the Host remains
  authoritative for observed CWD and process launch.
- **Transition and revalidation:** Execution revalidates terminal context, Tab, Panel, and
  Presentation before resolving the same terminal's newest CWD and creating one ordinary terminal
  Tab with an explicit launch directory. Missing CWD, a stale identity, or an unavailable Host
  capability makes the Action unavailable, creates nothing, and preserves the key event.
- **Invariants and exclusions:** Success yields distinct Tab, Panel, and Presentation identities,
  focuses the new terminal through the keyboard handover, and preserves the source session. Ordinary
  New Tab keeps its Workspace/default CWD. Environment, executable, foreground process, output,
  attachment, SSH state, and remote-shell directory discovery are never cloned.
- **Evidence:** Resolver, runtime, Action, menu, shortcut, and Settings tests cover target selection and current-directory Tab creation from trusted terminal metadata; the Desktop `terminal-parity/FS-15-current-directory-tab` trace retired with the Desktop Electron suite.

### Adjacent Tab selection model

- **Problem and inputs:** A person needs to traverse the active terminal context without knowing a
  Tab position. Trusted input is the latest canonical `TabModel` snapshot plus the semantic
  `previous` or `next` direction; the Action accepts no dynamic input.
- **Data, model, and ownership:** Workbench resolves one adjacent Tab ID from the ordered
  `TabModel.tabs` projection. `@orchbee/tab-strip` remains the sole selection-state owner and the
  existing prepared `tabs.tab.select` Action remains the only writer.
- **Outputs and transitions:** Zero or one Tab, a missing active Tab, or a missing lower Action is
  unavailable and preserves the key event. With at least two Tabs, previous selects index `i - 1`
  and next selects `i + 1`; crossing either boundary wraps to the opposite end. Selection uses
  `replace`, so grouped, collapsed, pinned, focus, and navigation behavior stay with the ordinary
  Tab-selection transition.
- **Invariants and revalidation:** The target differs from the active Tab, belongs to the same
  active terminal context, and follows canonical order rather than MRU order. Availability and
  execution each resolve from a current snapshot; a removed target or replaced lower registration
  blocks without direct mutation.
- **Verification and exclusions:** Pure resolver tests cover empty, singleton, interior, both wrap
  boundaries, reversed direction, and a missing active identity. Action tests cover grouped and
  pinned order, lower registration removal, and execution-time re-resolution. Shortcut tests cover
  both platforms, extra modifiers, repeat, composition, disabled fallthrough, and custom bindings.
  Reordering, cross-Workspace traversal, cross-window traversal, and MRU traversal are excluded.

### Active Tab movement model

- **Problem and inputs:** A person needs to reorder the active Tab without leaving terminal input.
  Trusted input is the latest canonical `TabModel` snapshot plus the semantic `backward` or
  `forward` intent; each Workbench Action accepts no dynamic input.
- **Data, model, and ownership:** Workbench resolves the active Tab ID and one lower
  `tabs.tab.move` direction. `@orchbee/tab-strip` remains the sole owner and writer of Tab order,
  pinned partitions, group membership, selection, and active identity.
- **Outputs and transitions:** Within the active Tab's pinned or unpinned partition, backward and
  forward delegate one relative move. Crossing that partition's first or last boundary delegates
  `last` or `first`, preserving circular movement without violating the pinned-prefix
  invariant. A missing active Tab, a partition with fewer than two Tabs, or a missing lower Action
  is unavailable and preserves the key event. The prepared lower Action owns the synchronous
  commit and result.
- **Invariants and revalidation:** The same active Tab remains active and selected, every Tab
  identity remains present exactly once, pinned Tabs remain a contiguous prefix, and all grouping
  side effects follow the existing `TabModel` movement transition. Availability and execution each
  resolve from a current snapshot, so stale identities or registrations never trigger a direct
  fallback mutation.
- **Decisions and reference scope:** An interior Tab delegates `previous` or `next`; the first or
  last Tab of a multi-Tab pin partition delegates `last` or `first`; every unavailable state yields
  no decision. Movement retains active-Tab direction, circular boundaries and the exact defaults above; numeric, first/last, remote-control and cross-window movement are excluded.
- **Verification:** Pure resolver tests cover missing, singleton, interior, both boundaries, both
  pinned partitions, and a missing active identity. Prepared-Action tests cover execution-time
  re-resolution, lower registration removal, unchanged active identity, and exact lower input.
  Shortcut, settings, and Action-search tests cover both defaults, disabled fallthrough, custom
  bindings, and visible discovery; the Desktop trace of final order, focus, and terminal input
  continuity retired with the Desktop Electron suite.

### Tab title model

- **Problem and inputs:** A person needs to give the active or pointer-invoked Tab a stable name
  without losing the terminal's newer dynamic title. Opening accepts either no input, which resolves
  the active Tab, or an explicit trusted terminal-context/Tab identity captured by the menu. Commit
  carries that same identity pair and a string or reset intent.
- **Data, model, and ownership:** Workbench owns the exact `terminalContextId`/`tabId` target and
  sends the name to the Server as `tab-renamed`; the Server holds the Tab's nullable custom title.
  Natural OSC title, current CWD, and the published fallback continue to update independently
  beneath it.
- **Outputs and transitions:** Opening pre-fills the effective displayed title and selects it. A
  trimmed non-empty submission sets the custom title, including when it equals the current natural
  title. A shared name has no way back to following the terminal, so an empty or whitespace-only
  submission asks for nothing. Save and cancel close the modal and restore prior terminal focus.
- **Invariants and revalidation:** Opening and commit independently resolve the exact context and Tab
  from the current snapshot. Switching active Tabs while the editor is open cannot redirect the
  write; removal of either identity blocks it. Dynamic metadata remains recorded while custom text
  is visible, and a rename never writes terminal-owned metadata.
- **Decisions:** Each platform has one default chord. A shared localized modal starts with the current title, blank submission changes nothing, and the Server retains the result. Window-title editing, remote control and multiple simultaneous bindings are excluded.
- **Verification:** Runtime tests cover an unnamed Tab following its terminal's natural title,
  current directory, and fallback. Action and React tests cover active and explicit targets, stale
  rejection, keyboard/context/Action-search entry points, blank submission, focus restoration, and
  unchanged terminal input. Settings and storage validation cover both defaults and user overrides;
  the Desktop `terminal-parity/FS-11-tab-title` trace retired with the Desktop Electron suite.

### Terminal font-size transition model

- **Problem and inputs:** A person needs to resize every terminal without opening Settings. Input
  is one semantic `increase`, `decrease`, or `reset` Action while the Workbench shortcut scope is
  active; no terminal focus or Panel identity is required.
- **Data, model, and ownership:** Web's `TerminalSettingsController` sends each step as one edit
  of `terminal.font_size`, and steps accumulate until the settings file answers. Increase and
  decrease use a two-point step, clamp to the existing inclusive `8`–`48` range, and reset
  restores only the product-default font size.
- **Outputs and transitions:** A changed value is one edit; the snapshot it publishes gives every
  mounted terminal one complete presentation configuration, and a refused edit keeps the file's
  value. A boundary or already-default request is `unchanged`.
- **Invariants and revalidation:** All mounted terminals observe one application-wide value;
  unrelated appearance settings never change. An admitted exact chord is consumed even for an
  unchanged boundary result, so it cannot become browser page zoom. Settings deactivates the
  Workbench shortcut scope and preserves its own editing keys.
- **Verification and exclusions:** Pure tests cover step, clamp, reset, publication, the edit and
  disposal. Action and application tests cover all three intents, physical dispatch, and
  Settings fallthrough. Per-Panel zoom, browser-page zoom, keypad aliases, shifted-plus aliases,
  and multiple simultaneous bindings are excluded.

## Active-target command model

The nineteen `workbench.*.active.*` Actions are context commands. Their Action invocation may carry an
explicit panel target from a menu; otherwise the active Tab's focused panel is the target. The
target is resolved again when execution starts, so a stale or absent target is unavailable and
does not consume the keyboard event.

`workbench.terminal.active.close` resolves exactly one lower-level close operation:

| Active target state                                                             | Prepared operation                             |
| ------------------------------------------------------------------------------- | ---------------------------------------------- |
| The authority reports that the active Tab's close stopped where it may continue | Resume closing that Tab                        |
| A target panel exists, whichever terminal it shows                              | Close that panel and every terminal it holds   |
| No valid active Tab or target panel exists                                      | Unavailable; preserve state and event defaults |

Closing a Tab's last panel also closes that Tab, as the Server does for `panel.close`. The context
Action executes the already-registered panel close Action as a prepared
invocation. Those owners retain availability, running-process confirmation, cancellation,
serialization, and the authoritative commit. The context Action reports the lower execution's
result and never mutates panel or Tab state directly. Confirmation cancellation leaves topology and
terminal sessions unchanged. Cancelling the context execution cancels its prepared lower execution;
if the lower operation has already committed, its completed result remains authoritative. A
missing or stale lower registration blocks the context Action without a direct fallback mutation.
Resume asks its own question, so the context Action starts it as a separate execution and reports
no change of its own.

Maximization resolves only when the active Tab has at least two panels. Right and below split
Actions resolve whenever a valid target panel exists. Each command prepares and executes the
existing panel Action, so menus, keyboard dispatch, and the Action search surface share the same
transition and availability result. An unavailable command is neither executed nor passed to
`preventDefault()`.

The four directional focus Actions resolve from the weighted split layout only while no Panel is
maximized. Workbench requires a neighboring Panel in the requested direction with positive overlap
on the perpendicular axis, never wraps at an outer edge, and delegates the result through the
existing Panel focus Action. A missing directional neighbor leaves the shortcut event available to
the terminal or browser.

The previous and next focus Actions flatten only the current Tab's canonical layout in depth-first
leaf order, choose the adjacent Panel, wrap at both boundaries, and delegate that exact target
through the same Panel focus Action. A singleton, stale source, or maximized Panel is unavailable,
so its chord falls through without crossing a Tab, terminal context, or window.

The positional focus Action validates one exact input from `1` through `10`, flattens only the
current Tab into canonical depth-first Panel-leaf order, and prepares the ordinary Panel focus
Action for the leaf at `position - 1`. It never clamps. Selecting the current Panel remains
available and returns the lower Action's `unchanged` result, including position one in a singleton;
a missing position, stale source, duplicate Panel identity, maximized layout, or missing lower
registration preserves state and the key event.

The four directional movement Actions reuse that exact neighbor resolver, then prepare one exact
source/target `workbench.panel.swap` Action. The lower transition exchanges the complete Panel
leaves while preserving split topology, weights, slot IDs, terminal identities, counters, and Panel
membership; the moved source remains focused. A missing neighbor, singleton layout, maximized Panel,
stale target, or missing lower registration performs no mutation and preserves the key event.

Panel equalization resolves only for an exact live target in a multi-Panel active Tab with no
maximized Panel, then prepares the lower `workbench.panel.equalize` Action. Its recursive unit model
preserves topology, order, Panel, Split, slot and terminal identities, membership, focus, counters,
and maximization state while assigning proportional weights at every split. An already equal layout
is an intentional available `unchanged` result and still consumes the admitted invocation; a
singleton, maximized, stale, or missing lower target is unavailable and preserves the key event.

Resize mode captures the exact active Tab and Panel when it opens. Every resize key revalidates that
identity, delegates one mutation through the lower Panel Action, and adjusts the nearest ancestor
whose split axis matches the requested direction. A missing axis or reached boundary is unchanged;
maximized Panels reject mutations; switching or removing the captured Tab or Panel closes the mode.
Action search and the current configurable shortcut open the same mode; the panel menus leave
resizing to them and to dragging a divider, as the TUI's pane menu does. Electron keeps native Reload and Force Reload menu commands clickable but does not register
their accelerators, allowing the renderer-owned current binding to resolve first.

## Shortcut settings discovery model

- **Problem and inputs:** The Settings catalog contains enough shortcuts that scanning every row is
  not a reliable discovery path. The local discovery surface accepts ordinary text, a typed shortcut
  representation, or one captured physical `KeyboardEvent`; the current localized row projection and
  complete effective binding table are trusted inputs, while input text and key events are transient
  and untrusted. An absent or whitespace-only query means the complete catalog.
- **Data, model, and ownership:** Web owns one ephemeral discovery state consisting of a mode
  (`text` or `recording`) and a query (`text` or one normalized chord). The pure resolver preserves
  catalog order and projects matching shortcut IDs. `KeyboardShortcutSettingsController` remains the
  sole binding writer; discovery receives its snapshot read-only and owns no storage, Action, or
  shortcut mutation capability.
- **Outputs and invariants:** Empty text returns every row. Ordinary text uses case-insensitive
  conjunctive terms across the localized action name, visible scope, availability and source, stable
  Action and shortcut IDs, and the formatted current chord. Modifier names, common Apple glyphs,
  physical key codes, and common key labels are searchable aliases. A captured chord compares the
  same canonical chord identity used by conflict detection and therefore returns only exact current
  matches. Zero matches produce the localized empty state; every nonempty result retains catalog
  order. Searching, capturing, clearing, cancelling, and unmounting never changes a binding or
  persistence record.

| Current mode | Event                                 | Next mode | Query and observable result                                                          |
| ------------ | ------------------------------------- | --------- | ------------------------------------------------------------------------------------ |
| Text         | Text changes                          | Text      | Replace with a text query and immediately project matching rows                      |
| Text         | Start recording                       | Recording | Preserve the current query, focus the search field, and wait for one supported chord |
| Recording    | Supported chord                       | Recording | Replace with an exact chord query and show its current matching Action rows          |
| Recording    | Modifier-only, repeat, or composition | Recording | No-op; preserve the prior query                                                      |
| Recording    | Unsupported or modifier-free key      | Recording | Preserve the prior query and expose the localized invalid-chord guidance             |
| Recording    | Escape, Tab, or explicit stop         | Text      | Stop capture, retain the current query, and let Tab continue ordinary focus movement |
| Either       | Clear                                 | Unchanged | Replace the query with empty text and show the complete catalog                      |

Normal-mode Escape clears a nonempty query before the enclosing Settings surface may handle
Escape; empty-query Escape remains available to Settings. Starting a row's existing shortcut editor
stops discovery recording, and starting discovery recording cancels any row editor, so one focused
control owns physical-key capture at a time. The search field reports its result count as a polite
status and remains labeled independently of its placeholder.

The interaction adopts VS Code revision `cd429513258458bcbe37b17fe714874197fe2adf`'s distinction
between ordinary keybinding search and an explicit Record Keys mode. Orchbee deliberately supports
one physical chord rather than VS Code's chord sequences, uses its own effective catalog and
localized metadata, and makes the recording path discovery-only rather than an editing command.

New Worktree and Open Worktree from the focused Panel (`workbench.worktree.active.create` and `workbench.worktree.active.open`), the counterparts of the TUI's `Ctrl+B W` and `Ctrl+B O`, have no default key and run from Action search. Standard `Ctrl+Shift+W` already closes the active terminal and `Mod+Shift+O` shows keyboard hints, and a `Ctrl+Alt` chord types an AltGr character on many Windows layouts, so no chord carries the TUI's mnemonic without taking one from the terminal or another command.

## Text field editing

Browser and Desktop text inputs keep the platform's native editing. The TUI's shared readline keys
(`Ctrl+U`, `Ctrl+K`, `Ctrl+W`, `Ctrl+Y`, `Alt+B`, `Alt+F`, `Alt+D`; see the
[native line editor](../../orchbee/docs/native-line-editor.md)) are a host difference, not a gap:
browsers on Windows provide none of them, emulating them would take Control chords from the page
and Alt chords from AltGr layouts, and native selection, undo and input-method behavior would
diverge from the platform. No catalog entry claims them.

## Chord and conflict model

`@orchbee/keyboard-shortcuts` provides only the pure physical-chord mechanism. A chord contains one
supported `KeyboardEvent.code` and a normalized exact modifier set. `Mod` maps to Command on Apple
and Control on Standard platforms. Matching rejects extra modifiers, composition (including legacy
key code 229), repeats, unsupported codes, and modifier-only events. Formatting and conflict
identity come from the same normalized chord.

All catalog declarations can overlap: a Workbench shortcut and the shortcut for its focused
terminal can both be applicable. Their effective chords must therefore be globally unique. A
conflicting edit changes neither memory nor storage and identifies the conflicting Action, scope,
binding source, and availability condition. During dispatch, the owning Action adapter resolves
current availability before execution or `preventDefault()`; an unavailable binding falls through
to the terminal or browser.

A configured command's direct key sits outside this catalog and is not a conflicting edit: it takes
a default's chord and yields to one the person set (see [Workbench](workbench.md)). Web resolves that
once, from the claims `keyboardShortcutClaims` projects, and hands the one result to both the
Workbench and Settings. A Settings row whose default a configured command holds keeps its chord and
recorder and says which command uses that chord, in the inline conflict text; recording an override
gives the person's chord back to the shortcut and removes the note, and Reset brings the note back.

Workbench owns formatting for its bindings. Projected-terminal owns its binding defaults and
dispatch, while Web projects only its current action-to-label map through a read-only Workbench
port. This preserves dependency direction: Workbench never imports an application-owned shortcut
catalog. A command menu entry stores an optional effective label and both shared menu renderers use
their UI primitive's trailing shortcut slot. Action search receives the same projector, including
terminal Actions discovered from descendant scopes.

## Persistence and recovery

New writes use `orchbee:web:keyboard-shortcut-overrides:v1` and a strict record:

```json
{
	"version": 1,
	"overrides": [
		{
			"shortcutId": "workbench.search.all.open",
			"chord": { "code": "KeyY", "modifiers": ["mod", "shift"] }
		}
	]
}
```

Only Custom values are stored, ordered by shortcut ID. Unknown fields or IDs, duplicates, unsorted
entries, noncanonical modifiers, unsupported codes, stored defaults, excessive entries, and an
effective conflict invalidate the complete v1 record and restore platform defaults. A write
failure keeps the immutable session binding active, reports that it was not saved, and retries on
the next legal change. Restoring one item removes its override.

A legal edit commits its complete immutable session binding before calling storage. Writes are
serialized per controller: storage reentry derives the next edit from that committed binding, and
only the latest pending complete record is written after the active call returns. A superseded or
disposed write cannot replace the current snapshot or clear a newer failure. An update returning
while its write is still queued reports `applied-session-only`; disposal drops queued writes.
Controller tests cover combined reentrant edits, final stored values, newer failures, and disposal.

Only the current key is read. Missing data uses platform defaults; invalid records report an issue and use defaults. Old keys are neither read, migrated nor deleted. The `keyboard-shortcut-overrides` name avoids accepting a shape-compatible development record from the old `keyboard-shortcuts:v1` key.

The Desktop composition creates and disposes one controller against its origin-local storage and
passes the controller's narrow binding source through
`ProjectedTerminalSurfaceCapability`. Event handlers read the current table at dispatch time, so a
change does not remount Workbench, replace an attachment, or reopen a Host connection.

## Claims and evidence

| Claim                                                                                                         | Deterministic evidence                                                                      |
| ------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| One valid target produces exactly one panel close or Tab close resume operation                               | Active-target resolver decision-table tests                                                 |
| Confirmation, cancellation, and commit remain owned by the prepared lower Action                              | Workbench Action integration tests with panel and Tab registrations                         |
| Active Workbench bindings do not depend on a surviving focused descendant                                     | Document-capture lifecycle and Web application integration tests                            |
| Empty-Tab New Tab creates and focuses the new terminal; Settings suppresses it                                | Web application focus and Settings-scope integration trace                                  |
| Current-directory Tab creation uses one exact displayed trusted CWD and preserves its source                  | Resolver, runtime, Action, menu, shortcut, and Settings tests                               |
| Apple and Standard defaults match the catalog and custom edits affect dispatch                                | Shortcut definition, resolver, and controller tests                                         |
| Idle platform-default terminal Paste preserves MIME-aware native delivery                                     | Shortcut routing and installer tests and terminal image-paste tests                         |
| Active ordinary terminal selection preserves native paste and input ownership                                 | Projected-terminal selection/paste tests                                                    |
| Pending Copy and Paste work cannot admit stale or duplicate shortcut input                                    | Clipboard coordinator and Action tests                                                      |
| Directional focus selects a spatial neighbor and falls through at outer edges                                 | Four-Panel resolver, Action, and shortcut dispatch tests                                    |
| Cyclic focus follows canonical Panel order, wraps, and falls through when unavailable                         | Generated resolver, prepared Action, shortcut dispatch, Settings, tests                     |
| Positional focus selects one exact canonical Panel and never clamps a missing position                        | Generated resolver, prepared Action, and shortcut/Settings tests                            |
| Visible focus and swap select one exact other Panel without leaking owned input                               | Model, Action, React, shortcut, and Settings tests                                          |
| Directional movement swaps exact neighboring leaves and retains source focus                                  | Model, bounded exploration, Action, shortcut, and Settings tests                            |
| Equalization assigns proportional same-axis Panel extents without identity changes                            | Model, generated layouts, Action, menu, shortcut, and Settings tests                        |
| Keyboard resize targets the nearest matching split and revalidates captured identity                          | Resolver, state, Action, React, Settings, and menu tests                                    |
| Positional selection follows canonical Tab order and falls through out of range                               | Grouped and pinned Action integration plus shortcut dispatch tests                          |
| Adjacent selection follows canonical Tab order and wraps only with multiple Tabs                              | Pure resolver, prepared Action integration, and shortcut fallthrough tests                  |
| Active-Tab movement wraps within its pin partition through one lower Action                                   | Pure resolver, prepared Action integration, and shortcut/settings tests                     |
| Tab-title editing preserves dynamic metadata and revalidates the exact invoked Tab                            | Workbench model/Action/React tests and storage validation                                   |
| Visible Hints consume one focused binding and never leak label keys to terminal input                         | Terminal, Action, shortcut, Settings, and Web system `terminal-parity/FS-07-keyboard-hints` |
| Menus, Action search, and settings display the same current binding                                           | Projection and rendered-surface tests                                                       |
| Shortcut Settings finds localized text, typed key aliases, and exact recorded chords without editing bindings | Pure discovery resolver and transition tests plus rendered search/capture integration tests |
| Invalid current data never consults another key                                                               | Storage corruption and strict-shape tests                                                   |

The tests use synthetic platform flags, Action scopes, storage, document capture, injected terminal
focus, and keyboard events. They do not claim operating-system global-shortcut behavior. The Desktop
traces these rows also cited, which drove the Desktop application through its former Main-owned
runtime, retired with the Desktop Electron suite.

## Maintenance checks

```sh
pnpm --filter @orchbee/keyboard-shortcuts typecheck
pnpm --filter @orchbee/keyboard-shortcuts test
pnpm --filter @orchbee/web typecheck
pnpm --filter @orchbee/web test
```
