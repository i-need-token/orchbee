# Keyboard shortcuts

`@orchbee/web` owns client-local overrides for a focused catalog of frequent semantic shortcuts.
Workbench and projected-terminal features continue to own their Action inputs and platform
defaults. The Web composition combines those declarations, validates one complete effective table,
and injects that same source into the mounted Workbench and projected terminals. Menus, tooltips,
the Action search surface, and shortcut settings project labels from that effective table; none of
those surfaces owns another binding copy.

## Initial catalog

| Shortcut ID                                  | Scope                             | Apple default         | Standard default      |
| -------------------------------------------- | --------------------------------- | --------------------- | --------------------- |
| `workbench.search.actions.open`              | Workbench                         | `Cmd+Shift+P`         | `Ctrl+Shift+P`        |
| `workbench.terminal.create`                  | Workbench                         | `Cmd+T`               | `Ctrl+Shift+T`        |
| `workbench.terminal.font-size.increase`      | Workbench                         | `Cmd+=`               | `Ctrl+=`              |
| `workbench.terminal.font-size.decrease`      | Workbench                         | `Cmd+-`               | `Ctrl+-`              |
| `workbench.terminal.font-size.reset`         | Workbench                         | `Cmd+0`               | `Ctrl+0`              |
| `workbench.tab.select.1` … `.9`              | Workbench                         | `Cmd+1` … `Cmd+9`     | `Ctrl+1` … `Ctrl+9`   |
| `workbench.tab.select-previous`              | Workbench                         | `Cmd+Shift+[`         | `Ctrl+PageUp`         |
| `workbench.tab.select-next`                  | Workbench                         | `Cmd+Shift+]`         | `Ctrl+PageDown`       |
| `workbench.tab.active.move-backward`         | Workbench                         | `Ctrl+Shift+,`        | `Ctrl+Shift+,`        |
| `workbench.tab.active.move-forward`          | Workbench                         | `Ctrl+Shift+.`        | `Ctrl+Shift+.`        |
| `workbench.terminal.active.close`            | Active terminal                   | `Cmd+W`               | `Ctrl+Shift+W`        |
| `workbench.panel.active.focus-down`          | Active panel                      | `Cmd+Option+Down`     | `Ctrl+Alt+Down`       |
| `workbench.panel.active.focus-left`          | Active panel                      | `Cmd+Option+Left`     | `Ctrl+Alt+Left`       |
| `workbench.panel.active.focus-right`         | Active panel                      | `Cmd+Option+Right`    | `Ctrl+Alt+Right`      |
| `workbench.panel.active.focus-up`            | Active panel                      | `Cmd+Option+Up`       | `Ctrl+Alt+Up`         |
| `workbench.panel.active.maximization.toggle` | Active panel                      | `Cmd+Shift+Enter`     | `Ctrl+Shift+Enter`    |
| `workbench.panel.active.split-right`         | Active panel                      | `Cmd+D`               | `Ctrl+Shift+D`        |
| `workbench.panel.active.split-below`         | Active panel                      | `Cmd+Shift+D`         | `Ctrl+Shift+E`        |
| `workbench.search.all.open`                  | Workbench                         | `Cmd+Shift+F`         | `Ctrl+Shift+K`        |
| `workbench.settings.open`                    | Workbench                         | `Cmd+,`               | `Ctrl+,`              |
| `workbench.sidebar.toggle`                   | Workbench                         | `Cmd+B`               | `Ctrl+Shift+B`        |
| `terminal.search.open`                       | Focused terminal or Find          | `Cmd+F`               | `Ctrl+Shift+F`        |
| `terminal.hints.open`                        | Focused terminal                  | `Cmd+Shift+O`         | `Ctrl+Shift+O`        |
| `terminal.output.show-last`                  | Focused terminal                  | `Ctrl+Shift+G`        | `Ctrl+Shift+G`        |
| `terminal.history.show`                      | Focused terminal                  | `Ctrl+Shift+H`        | `Ctrl+Shift+H`        |
| `terminal.selection.copy`                    | Focused terminal with a selection | `Cmd+C`               | `Ctrl+Shift+C`        |
| `terminal.clipboard.paste`                   | Focused terminal                  | `Cmd+V`               | `Ctrl+Shift+V`        |
| `terminal.history.prompt-previous`           | Focused terminal                  | `Ctrl+Shift+Z`        | `Ctrl+Shift+Z`        |
| `terminal.history.prompt-next`               | Focused terminal                  | `Ctrl+Shift+X`        | `Ctrl+Shift+X`        |
| `terminal.history.scroll-line-up`            | Focused terminal                  | `Ctrl+Shift+Up`       | `Ctrl+Shift+Up`       |
| `terminal.history.scroll-line-down`          | Focused terminal                  | `Ctrl+Shift+Down`     | `Ctrl+Shift+Down`     |
| `terminal.history.scroll-page-up`            | Focused terminal                  | `Ctrl+Shift+PageUp`   | `Ctrl+Shift+PageUp`   |
| `terminal.history.scroll-page-down`          | Focused terminal                  | `Ctrl+Shift+PageDown` | `Ctrl+Shift+PageDown` |
| `terminal.history.scroll-top`                | Focused terminal                  | `Ctrl+Shift+Home`     | `Ctrl+Shift+Home`     |
| `terminal.history.scroll-bottom`             | Focused terminal                  | `Ctrl+Shift+End`      | `Ctrl+Shift+End`      |
| `terminal.selection.select-all`              | Focused terminal                  | `Cmd+A`               | `Ctrl+Shift+A`        |
| `terminal.clear`                             | Focused terminal                  | `Cmd+K`               | `Ctrl+Shift+L`        |

The new-Tab, active-terminal close, active-panel maximization, right-split, down-split, and paste
defaults adopt Warp's [documented desktop conventions](https://docs.warp.dev/getting-started/keyboard-shortcuts)
as observed on 2026-08-22; this is a pinned default-key selection, not a broader behavior or
visual-parity claim. Previous- and next-Tab defaults use the corresponding Warp bindings at
revision `086dade63ff7327426d6276d57f85b3f2f313af0`. Apple All search uses `Cmd+Shift+F`, leaving
the familiar `Cmd+K` to clear only the focused terminal. Standard Clear uses `Ctrl+Shift+L`;
Standard defaults deliberately leave unshifted `Ctrl+B/T/W/K/F/D/E/V/A/L` and `Ctrl+C` available
to the shell, terminal applications, and multiplexers. A user may deliberately record another
supported custom chord. Product conflicts remain prohibited even when an Action is temporarily
unavailable.

Previous- and next-prompt navigation preserve Kitty's exact `Ctrl+Shift+Z` and `Ctrl+Shift+X`
defaults at revision `6d5d0c440603ad9bdf6dcd599f73f6dde21acb44`. They use explicit Control on
Apple as well as Standard platforms, not the platform `Mod` abstraction. The behavior and bounded
Host-row contract are owned by the terminal package's
[Kitty prompt-navigation alignment](../../../packages/terminal/docs/kitty-prompt-navigation-alignment.md).

Line, page, top, and bottom history navigation likewise preserve Kitty's primary
`kitty_mod+Arrow/Page/Home/End` bindings at that revision. Both platforms use explicit
`Ctrl+Shift`; Kitty's additional macOS Command and `J`/`K` aliases remain available only by
replacing an effective custom binding because this catalog owns one binding per Action. Page
movement retains Kitty's one-row overlap. The behavior and bounded Host-row contract are owned by
the terminal package's
[Kitty keyboard-scrollback alignment](../../../packages/terminal/docs/kitty-keyboard-scrollback-alignment.md).

The three application-wide font-size defaults project Kitty's macOS `cmd+equal`, `cmd+minus`, and
`cmd+0` entry points from revision `6d5d0c440603ad9bdf6dcd599f73f6dde21acb44`. Standard
platforms use the same physical keys through `Mod`, producing `Ctrl+=`, `Ctrl+-`, and `Ctrl+0`.
This is a deliberately bounded common-key projection: Kitty's keypad, shifted-plus, and Backspace
aliases are excluded until the shortcut model can retain more than one binding per Action.

Visible keyboard Hints use `Mod+Shift+O` on both platforms. The unified binding avoids Standard
`Ctrl+Shift+E`, which already owns Split Below, while retaining a single discoverable Action for
Kitty's frequent visible URL, path, file-location, and hash selection workflow. Detection, effects,
cancellation, and exclusions are owned by the terminal package's
[Kitty keyboard-hints alignment](../../../packages/terminal/docs/kitty-keyboard-hints-alignment.md).

Show Last Command Output preserves Kitty's `Ctrl+Shift+G` default at revision
`6d5d0c440603ad9bdf6dcd599f73f6dde21acb44` on both platforms. It opens an immutable viewer
for the latest bounded shell-integration output; the extraction, search, copying, cancellation,
and exclusion contracts are owned by the terminal package's
[Kitty command-output-viewer alignment](../../../packages/terminal/docs/kitty-command-output-viewer-alignment.md).

Show Scrollback preserves Kitty's `Ctrl+Shift+H` default at the same revision on both platforms.
It opens one immutable document containing the complete retained primary-screen history and starts
at the terminal's captured visible top. Exact extraction, wrap projection, search, source-specific
copying, cancellation, and exclusions are owned by the terminal package's
[Kitty scrollback-viewer alignment](../../../packages/terminal/docs/kitty-scrollback-viewer-alignment.md).

Active-Tab movement preserves Kitty's `Ctrl+Shift+,` and `Ctrl+Shift+.` defaults at revision
`6d5d0c440603ad9bdf6dcd599f73f6dde21acb44` on both platforms. Workbench keeps explicit
Control on Apple because Kitty's `kitty_mod` default is Control+Shift there too; the movement
model below defines the adopted behavior beyond the key selection.

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
Main-owned clipboard text, produces no Action status, and delegates text/image precedence to the
terminal adapter. Action routing never reads image bytes or expands the Desktop clipboard-read
contract beyond bounded plain text.

Directional panel focus adopts Ghostty's Apple `Command+Option+Arrow` and Standard
`Control+Alt+Arrow` convention at revision `91f66da24527fa02d92b5fd0b41cd020f553a64c`, also
cross-checked against Warp revision `086dade63ff7327426d6276d57f85b3f2f313af0`. The binding
selection is the only application-level concern here; Workbench owns spatial neighbor resolution.

Commands without an effective binding do not render an empty shortcut affordance. Reset,
new-child-terminal, split-left, and split-above remain intentionally unbound in this catalog.
Adding nullable or user-created bindings for initially unbound commands is outside this model.

## Workbench dispatch scope

Web publishes whether Workbench is the active product surface. The mounted Workbench owns one
capture listener on its document and participates only while that semantic scope is active. Focus
inside a terminal, Tab strip, sidebar control, Workbench interaction surface, or the document body
does not change Workbench binding applicability. Opening the full-screen Settings surface makes the
still-mounted Workbench inactive; closing Settings activates it again. Unmounting Workbench removes
the listener.

| Workbench surface state      | Binding decision                                                                   |
| ---------------------------- | ---------------------------------------------------------------------------------- |
| Active with one or more Tabs | Resolve Workbench bindings independently of the focused Workbench region           |
| Active with no Tabs          | New Tab remains enabled; target-dependent close and Panel commands are unavailable |
| Settings is open             | Skip every Workbench binding and preserve event defaults                           |
| Workbench is unmounted       | No Workbench listener or binding participates                                      |

A resolved binding still runs only when its Action is enabled. Successful New Tab execution makes
the new Tab active and projects its first Panel terminal as the sole `autoFocus` target; the
injected terminal surface claims real input when ready. Active-terminal and active-Panel commands
resolve from the active Tab's remembered focused Panel rather than the keyboard event target, so a
sidebar or document-body focus does not redirect them. An unavailable Action preserves state and
does not call `preventDefault()`.

`Mod+1` through `Mod+9` invoke `workbench.tab.select-by-position` with the matching one-based
position. Position is resolved from the active terminal context's canonical `TabModel.tabs` order:
the pinned prefix comes first, followed by the current grouped and ungrouped order. Tabs inside a
collapsed group still occupy positions, and the delegated `tabs.tab.select` Action expands the
selected Tab's group through the ordinary selection transition. A missing position is unavailable
and leaves the event untouched. `Mod+9` means the ninth Tab, never the last Tab.

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
  `last` or `first`, matching Kitty's circular movement without violating the pinned-prefix
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
  no decision. This adopts Kitty's active-Tab direction, circular boundary, and exact defaults at
  revision `6d5d0c440603ad9bdf6dcd599f73f6dde21acb44`. Kitty's lack of pinned partitions and groups,
  cross-window movement, drag behavior, animation, and visual parity are excluded.
- **Verification:** Pure resolver tests cover missing, singleton, interior, both boundaries, both
  pinned partitions, and a missing active identity. Prepared-Action tests cover execution-time
  re-resolution, lower registration removal, unchanged active identity, and exact lower input.
  Shortcut, settings, Action-search, and Desktop traces cover both defaults, disabled fallthrough,
  custom bindings, visible discovery, final order, focus, and terminal input continuity.

### Terminal font-size transition model

- **Problem and inputs:** A person needs to resize every terminal without opening Settings. Input
  is one semantic `increase`, `decrease`, or `reset` Action while the Workbench shortcut scope is
  active; no terminal focus or Panel identity is required.
- **Data, model, and ownership:** Web's `TerminalSettingsController` remains the sole writer for
  the device-local appearance snapshot. Increase and decrease use a two-point step, clamp to the
  existing inclusive `8`–`48` range, and reset restores only the product-default font size.
- **Outputs and transitions:** A changed value publishes one complete presentation configuration
  to every mounted terminal and persists the existing appearance record. A persistence failure
  keeps the session value. A boundary or already-default request is `unchanged`.
- **Invariants and revalidation:** All mounted terminals observe one application-wide value;
  unrelated appearance preferences never change. An admitted exact chord is consumed even for an
  unchanged boundary result, so it cannot become browser page zoom. Settings deactivates the
  Workbench shortcut scope and preserves its own editing keys.
- **Verification and exclusions:** Pure tests cover step, clamp, reset, publication, persistence,
  and disposal. Action and application tests cover all three intents, physical dispatch, and
  Settings fallthrough. Per-Panel zoom, browser-page zoom, keypad aliases, shifted-plus aliases,
  and multiple simultaneous bindings are excluded.

## Active-target command model

The eight `workbench.*.active.*` Actions are context commands. Their Action invocation may carry an
explicit panel target from a menu; otherwise the active Tab's focused panel is the target. The
target is resolved again when execution starts, so a stale or absent target is unavailable and
does not consume the keyboard event.

`workbench.terminal.active.close` resolves exactly one lower-level close operation:

| Active target state                                                             | Prepared operation                             |
| ------------------------------------------------------------------------------- | ---------------------------------------------- |
| The displayed terminal is a child terminal                                      | Close that child terminal                      |
| The displayed terminal is the primary and the active Tab has two or more panels | Close that panel                               |
| The displayed terminal is the primary and the active Tab has one panel          | Close the active Tab                           |
| No valid active Tab or target panel exists                                      | Unavailable; preserve state and event defaults |

The context Action executes the already-registered panel or Tab close Action as a prepared
invocation. Those owners retain availability, running-process confirmation, cancellation,
serialization, and the authoritative commit. The context Action reports the lower execution's
result and never mutates panel or Tab state directly. Confirmation cancellation leaves topology and
terminal sessions unchanged. Cancelling the context execution cancels its prepared lower execution;
if the lower operation has already committed, its completed result remains authoritative. A
missing or stale lower registration blocks the context Action without a direct fallback mutation.

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

Workbench owns formatting for its bindings. Projected-terminal owns its binding defaults and
dispatch, while Web projects only its current action-to-label map through a read-only Workbench
port. This preserves dependency direction: Workbench never imports an application-owned shortcut
catalog. A command menu entry stores an optional effective label and both shared menu renderers use
their UI primitive's trailing shortcut slot. Action search receives the same projector, including
terminal Actions discovered from descendant scopes.

## Persistence and recovery

New writes use `orchbee:web:keyboard-shortcuts:v12` and a strict record:

```json
{
	"version": 12,
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
effective conflict invalidate the complete v12 record and restore platform defaults. A write
failure keeps the immutable session binding active, reports that it was not saved, and retries on
the next legal change. Restoring one item removes its override.

When v12 is absent, Web first reads `orchbee:web:keyboard-shortcuts:v11` with its exact prior catalog
and defaults. If v11 is also absent, it tries v10, v9, v8, v7, v6, v5, v4, v3, v2, and then v1. Every migration carries
explicit overrides into the expanded catalog, removes an override equivalent to a current default,
rejects any resulting conflict, and writes the canonical v12 record. The v1, v2, v3, v4, v5, v6,
v7, v8, v9, v10, and v11 values remain unchanged as rollback snapshots. An invalid current record never falls back to an
older record, and an invalid migration source never falls through to the next older version,
because either fallback would hide data corruption. Rolling back restores the latest record that
the older build understands; edits made only in v12 are intentionally unavailable to it.

Browser Preview, Gateway Browser, and Electron each create and dispose one controller against their
own origin-local storage. Gateway and Electron pass the controller's narrow binding source through
`ProjectedTerminalSurfaceCapability`. Event handlers read the current table at dispatch time, so a
change does not remount Workbench, replace an attachment, or reopen a Host or Gateway connection.

## Claims and evidence

| Claim                                                                                 | Deterministic evidence                                                                                                  |
| ------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| One valid target produces exactly one child, panel, or Tab close operation            | Active-target resolver decision-table tests                                                                             |
| Confirmation, cancellation, and commit remain owned by the prepared lower Action      | Workbench Action integration tests with panel and Tab registrations                                                     |
| Active Workbench bindings do not depend on a surviving focused descendant             | Document-capture lifecycle and Web application integration tests                                                        |
| Empty-Tab New Tab creates and focuses the new terminal; Settings suppresses it        | Web application focus and Settings-scope integration trace                                                              |
| Apple and Standard defaults match the catalog and custom edits affect dispatch        | Shortcut definition, resolver, and controller tests                                                                     |
| Idle platform-default terminal Paste preserves MIME-aware native delivery             | Shortcut routing and installer tests, terminal image-paste tests, and exact Desktop `terminal-parity/IN-03-image-paste` |
| Active ordinary terminal selection preserves native paste and input ownership         | Projected-terminal selection/paste tests and exact Desktop `terminal-parity/IN-18-native-selection-paste`               |
| Pending Copy and Paste work cannot admit stale or duplicate shortcut input            | Clipboard coordinator and Action tests plus exact Desktop `terminal-parity/IN-13-selection-copy-paste`                  |
| Directional focus selects a spatial neighbor and falls through at outer edges         | Four-Panel resolver, Action, and shortcut dispatch tests                                                                |
| Positional selection follows canonical Tab order and falls through out of range       | Grouped and pinned Action integration plus shortcut dispatch tests                                                      |
| Adjacent selection follows canonical Tab order and wraps only with multiple Tabs      | Pure resolver, prepared Action integration, and shortcut fallthrough tests                                              |
| Active-Tab movement wraps within its pin partition through one lower Action           | Pure resolver, prepared Action integration, shortcut/settings tests, and Desktop system trace                           |
| Visible Hints consume one focused binding and never leak label keys to terminal input | Terminal, Action, shortcut, Settings, and exact Desktop `terminal-parity/FS-07-keyboard-hints`                          |
| Show Scrollback consumes one focused binding and opens the exact immutable viewer     | Terminal, Action, shortcut, Settings, and exact Desktop `terminal-parity/FS-09-scrollback-viewer`                       |
| Menus, Action search, and settings display the same current binding                   | Projection and rendered-surface tests                                                                                   |
| Valid v1 through v11 records migrate without losing explicit legacy overrides         | Storage migration tests, including new-default conflicts                                                                |
| Invalid current or migration-source data does not consult an older record             | Storage corruption and precedence tests                                                                                 |

The tests use synthetic platform flags, Action scopes, storage, document capture, injected terminal
focus, and keyboard events. They do not claim operating-system global-shortcut behavior or full
Warp compatibility. The manual Desktop native-window focus test is excluded from autonomous
verification.

## Maintenance checks

```sh
pnpm --filter @orchbee/keyboard-shortcuts typecheck
pnpm --filter @orchbee/keyboard-shortcuts test
pnpm --filter @orchbee/workbench test
pnpm --filter @orchbee/web typecheck
pnpm --filter @orchbee/web test
```
