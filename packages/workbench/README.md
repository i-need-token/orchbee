# @orchbee/workbench

Shared product-semantic React shell for Orchbee applications.

## Scope

Workbench registers its product operations in the application-owned `ActionRuntime`. Sidebar
controls, shortcuts, tab-strip commands, panel buttons, and menus invoke those registrations; the
registered handlers are the only writers for their semantic results. Continuous resize and drag
preview remain reversible interaction state, while the final drag result invokes the corresponding
Action.

The package owns a full-height, horizontally resizable two-panel shell. The left sidebar defaults
to `250px` and is constrained to `200px`–`400px`; the right panel fills the remaining width and
renders the host-owned terminal tab model through `@orchbee/tab-strip`. The sidebar control and
`Mod+B` shortcut hide or restore the sidebar. The control remains mounted at one fixed shell
position in both states. While the sidebar is hidden, a transparent leading position inside the
tab-strip surface reserves its footprint so the control never overlaps the first tab or paints a
separate background. The localized sidebar provides `New Terminal`, `Search`, a `Project` section
heading, and a bottom-anchored `Settings` action.

Right-clicking blank tab-strip space exposes a localized orientation action. The Workbench switches
the strip and content layout between a top horizontal strip and a left vertical strip while tabs,
groups, and the new-tab control keep their existing context menus.

The shell composes primitives from `@orchbee/ui` without owning terminal sessions, settings
destinations, project, authentication, routing, or agent orchestration lifecycles. Workbench owns
the unified search state and presentation, while semantic owners remain responsible for searchable
read models and Action registrations. The machine-readable scope contract lives in
[`project.json`](project.json).

## Unified search

The sidebar Search control and `Mod+K` open one upper-viewport modal with the `All` filter and an
empty query. `Mod+Shift+P` opens the same surface with the `Actions` filter. Each global open starts
a new in-memory session; switching among `All`, `Agents`, `Actions`, and `Settings` preserves the
current query, while closing discards the session. File search is intentionally absent.

The current local provider projects Actions by requesting subtree discovery from the mounted
application `ActionRuntime`. It keeps every runtime-owned prepared or unbound invocation intact,
groups results by localized Action category, explains disabled capabilities, displays resolved
Workbench shortcuts, and invokes a selected ready result through `executePrepared(...)`. The
tab-strip-orientation Action demonstrates nested input: selecting the command opens an in-surface
horizontal/vertical picker, binds the selected value through `prepareInvocation(...)`, and then
executes that same scope-bound token. The Settings provider projects the stable root Settings
destination through `workbench.settings.open`.

`All` shows curated recommended Actions plus Settings before a query, then searches the complete
local catalog after typing. Matching normalizes Unicode, case, and repeated whitespace and ranks
exact localized titles, title prefixes, title-token prefixes, localized keywords, canonical English
titles, and fuzzy subsequences in deterministic tiers. The `Agents` filter is present as the
accepted navigation slot but remains empty until an agent-owned searchable read model and
navigation Action are composed. Async providers, file search, Agent fallback, persistent MRU, and
user-editable keybindings remain outside the current implementation.

The dialog keeps DOM focus in its combobox. Up and Down move `aria-activedescendant` through
inspectable results, Enter opens or runs the highlighted row, Escape returns from a nested picker
or closes the surface, and pointer movement updates the same visual highlight. Input, filters, and
footer remain fixed while grouped results scroll. Closing occurs before a selected Action begins;
it does not cancel or assume ownership of that Action's asynchronous lifecycle.

### Search modeling record

- **Problem and inputs:** A person needs to discover and invoke product capabilities without
  knowing a menu location or shortcut. Trusted inputs are a reviewed open filter, the mounted
  Action scope, localized Action descriptors, and pointer or keyboard events. Query text is local
  user input. Missing Action input remains an unbound runtime token and is never guessed.
- **Data and ownership:** `WorkbenchSearchState` is the sole owner of the open session, filter,
  query revision, query, highlight, and nested parent snapshot. Semantic owners remain
  authoritative for Action availability and effects. Projected results carry stable provider,
  group, result, Action, and scope identity plus a runtime-owned prepared or unbound invocation.
- **Outputs:** Opening renders and focuses one modal session; filter and query changes produce a
  deterministic local projection; a disabled selection announces its reason without invoking;
  an unbound supported result enters a nested picker; a final enabled selection reaches at most
  one Action execution. Escape and outside dismissal commit no Action.
- **Transitions:** Open resets to the requested preset. Query changes increment the revision.
  Filter changes retain query text. Result changes retain a still-visible highlight and otherwise
  select the first enabled result, falling back to the first inspectable row. Nested Back restores
  the complete parent filter, query, revision, and highlight snapshot. Close retains only the next
  monotonic session identity.
- **Invariants:** A rendered highlight always names one current visible row; hidden Actions never
  render; disabled or unsupported unbound rows never execute; prepared input is bound only through
  the originating runtime token; one accepted row reaches at most one `executePrepared(...)`
  call; and closing or nested cancellation performs no domain mutation.
- **Dependencies and exclusions:** React owns the mounted dialog lifecycle and focus adapter;
  `@orchbee/actions` owns registration, discovery, preparation, availability, confirmation, and
  execution; `@orchbee/i18n` owns product copy; and design tokens own geometry and appearance.
  Network, storage, telemetry, process state, Agent data, and async provider coordination are
  excluded until their semantic owners expose the required contracts.
- **Verification:** Pure tests cover open/close, query and filter transitions, highlight movement,
  nested restoration, matching tiers, deterministic grouping, Settings de-duplication, and
  argument filtering. Runtime projection tests cover prepared and unbound results. Component tests
  cover combobox focus, Actions filtering, nested orientation input, Action execution, and close.
  Storybook supplies open All and Chinese Actions states; browser verification exercises both
  shortcuts, four filters, keyboard and pointer selection, nested input, dismissal, and both
  tab-strip orientations.

## Panel workspace

Each terminal tab owns an in-memory panel workspace below its tab strip. A new tab starts with one
full-size panel and one special primary terminal. The single-panel primary view omits panel chrome.
Right-clicking the terminal viewport can create a one-level child terminal or split the current
panel above, below, left, or right. A child terminal occupies the same viewport as its primary
terminal; child terminals appear in a real `40px` bottom strip, while the primary terminal never
appears in that strip. Activating a child adds a `32px` header with a return action, geometrically
centered terminal title, and close action.

Once a tab has multiple panels, every panel displays the header. Its panel-options menu currently
contains only Maximize Panel or Minimize Panel. Maximizing replaces the visible split projection
with one panel without destroying the underlying tree. Splitting a maximized panel first restores
the tree and then applies the split. Exactly one panel is focused; that panel retains the normal
terminal surface while every other panel receives a non-interactive inactive mask. Pointer or
keyboard focus anywhere inside a panel transfers focus and the mask to the other panels. Dragging
a primary-terminal panel header lifts a scaled visual snapshot cloned from that panel's current DOM,
removes the source from a reversible layout projection, and immediately reflows the remaining grid.
The snapshot preserves current panel chrome and terminal presentation without mounting a second
React terminal instance. A header showing an active child terminal is not draggable. Moving over
another panel's directional edge projects the complete final grid with an empty framed panel at the
destination. Dropping replaces that placeholder with the complete source panel and splits the
target above, below, left, or right without replacing any terminal identity or state. The center of
a target is intentionally inactive so a release without a directional placeholder cancels and
restores the canonical grid. Closing an active child affects only that child and returns to the
primary terminal; closing a primary view affects the panel and all of its children. A localized
confirmation is required whenever the affected logical terminal set contains a `running` or
`provisioning` terminal. The last panel in a tab is not closable through the panel lifecycle because
the tab lifecycle owns that case.

Terminal process rendering and terminal-search execution remain host capabilities. Terminal search
does not occupy panel-header chrome and will be exposed through a keyboard command when that
product action is introduced.

### Modeling record

- **Problem and inputs:** A person or future command adapter needs deterministic local panel
  composition without coupling geometry to terminal-process ownership. Trusted inputs are the
  immutable `TabModel` snapshot and semantic UI events. A missing active tab renders no panel
  workspace; malformed external persistence is out of scope because this model is not persisted.
- **Data:** `PanelWorkspaceState` owns an ordered workspace per tab ID. Each workspace has one
  canonical `PanelLayoutNode` tree, one focused panel ID, and at most one maximized panel ID. A leaf
  owns exactly one primary `PanelTerminal`, an ordered one-level child list, and zero or one active
  child ID. Split children carry stable globally unique layout slot IDs, independent from Panel and
  Split identity, plus non-negative percentage weights.
  Terminal presentation records stable IDs, title, phase, and nullable CWD; it never contains
  recursive child terminals.
- **Outputs:** `transitionPanelWorkspace` returns the complete next state plus `applied`,
  `unchanged`, or a named rejection. It performs no process, DOM, storage, time, random, or network
  effects. Unknown tabs, panels, splits, or children reject; repeated selection and equivalent
  resize events are intentional no-ops. Provisioning failure is represented by a terminal phase
  transition and retains the owning panel for later inspection or retry.
- **Transitions:** Tab reconciliation preserves workspaces by tab ID and creates or removes only
  the changed tab projections. Four-direction split, adjacent resize, focus, primary/child
  selection, child creation/close, panel move/close, and maximize/minimize are the maintained
  semantic events. Terminal phase changes update presentation without changing membership.
  Same-axis splits insert locally by halving only the source weight; cross-axis splits create a
  nested group. Panel move removes and normalizes the source location before inserting the same
  leaf around the target, and never allocates a panel or terminal identity. The drag projection
  applies that same pure tree transformation without committing it, allowing cancellation to
  restore the untouched canonical tree. Moving an already adjacent Panel back to its current side
  is an intentional no-op that preserves split identity, slot identity, and resized proportions.
  Closing a non-focused Panel preserves focus; closing the focused Panel selects the next visual
  Panel, falling back to the previous one. New primary and child terminals inherit the displayed
  source terminal's nullable CWD.
- **Invariants:** Every tab has at least one panel; panel, split, and terminal IDs are unique;
  focused and maximized IDs resolve within their tab; active child IDs resolve within their panel;
  every split has at least two children, layout slot IDs remain globally unique, child weights total
  exactly `100` within numeric tolerance, repeated split orientations are flattened, and a resize
  changes only the two children adjacent to its separator. Closing or lifting a panel preserves the
  parent slot identity while normalizing collapsed splits and retains a deterministic next visual
  neighbor.
- **Decisions:** Splitting the right half of `[50, 50]` produces `[50, 25, 25]`, not thirds.
  Split creation remains valid even when the resulting viewport is narrower than `30px`; this keeps
  semantic split commands deterministic instead of letting a rendering constraint reject the
  model. During separator resize, both adjacent children are constrained to at least `30px` along
  the active axis; nested orthogonal splits therefore protect both panel width and height.
  Separators keep a single opaque one-CSS-pixel color through intersections. Panel header and
  child-strip space is never overlaid on the terminal viewport.
- **Ownership and dependencies:** Workbench is the sole writer of the panel layout and logical
  terminal-presentation state. `TabModel` remains authoritative for tab order and active tab.
  Hosts remain authoritative for PTY/process lifecycle, real CWD discovery, terminal rendering,
  and eventual CLI/agent command transport. Future header metadata such as panel status, Node
  version, Git branch, and change counts is deliberately excluded until an owning data contract
  exists; the current three-zone header preserves a composition point for it.
- **Correctness and verification:** Example tests cover direction, local weighting, nested-tree
  normalization, CWD inheritance, child selection, maximization, last-panel rejection, and per-tab
  isolation. Move examples additionally cover all four directions, identity preservation, collapsed
  source-slot reinsertion, equivalent-position no-op, and rejection behavior. Fixed-seed stateful
  property tests generate mixed hierarchy/action sequences, including every maintained single-tab
  action, and assert every invariant after every accepted transition. Bounded exploration covers
  all `384` four-Panel split layouts, all `18,432` possible moves, and `25,728` focus, maximize,
  terminal, close, and resize actions. Tab reconciliation separately covers every ordered subset of
  three tab IDs plus deeper generated reconciliation sequences. Move safety additionally has a
  fixed nested-tree regression and deeper generated layout exploration. The resizable adapter has
  a separate projection oracle proving that unrelated sibling weights remain unchanged. React
  integration tests cover the initial single-panel projection; Storybook and browser review
  exercise real context menus, resize handles, headers, panel drops, and dialogs. Concurrency and
  asynchronous process completion are excluded from this release, so no TLA+ claim is made.

## Use

Import the UI compatibility stylesheet once before the Workbench stylesheet at the application
composition boundary:

```ts
import '@orchbee/ui/styles.css';
import '@orchbee/tab-strip/styles.css';
import '@orchbee/workbench/styles.css';
```

Render the shell at the same boundary:

```tsx
import {
	Workbench,
	type WorkbenchHostActionCallback,
	type WorkbenchLayoutStorage,
	type WorkbenchShortcutPlatform,
	type WorkbenchTerminalCreateCallback,
} from '@orchbee/workbench';
import { ActionRuntime } from '@orchbee/actions';
import type { TabModel } from '@orchbee/tab-strip';

export function ProductInterface({
	actionRuntime,
	layoutStorage,
	onNewTerminal,
	onSettings,
	shortcutPlatform,
	tabModel,
}: {
	readonly actionRuntime: ActionRuntime;
	readonly layoutStorage: WorkbenchLayoutStorage;
	readonly onNewTerminal: WorkbenchTerminalCreateCallback;
	readonly onSettings: WorkbenchHostActionCallback;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
	readonly tabModel: TabModel;
}) {
	return (
		<Workbench
			actionRuntime={actionRuntime}
			layoutStorage={layoutStorage}
			onNewTerminal={onNewTerminal}
			onSettings={onSettings}
			shortcutPlatform={shortcutPlatform}
			tabModel={tabModel}
		/>
	);
}
```

Create one `ActionRuntime` for each mounted application composition and dispose it with that
composition. `WorkbenchTerminalCreateCallback` receives the execution context plus input carrying
the requested group and optional insertion index, so the visible new-tab control and tab context
menus reach the same terminal-creation handler without losing placement. Host callbacks return a
structured `ActionResult`, may return it asynchronously, observe `context.signal` before commit, and
call `context.markCommitted()` immediately before their authoritative mutation. Workbench does not
convert a rejected host promise into a false success or claim commit on the host's behalf.

When storage is provided, Workbench restores `orchbee:workbench:left-panel-width:v1`,
`orchbee:workbench:left-panel-visibility:v1`, and
`orchbee:workbench:tab-strip-orientation:v1`. It parses complete JSON records with feature-owned
strict Zod schemas, saves width only after a completed user resize, and saves visibility or tab-strip
orientation only after the corresponding semantic toggle action. Missing, malformed, unavailable,
or out-of-range storage falls back to a visible `250px` sidebar and a horizontal tab strip without
blocking rendering. The repository-wide ownership and key contract lives in
[`docs/frontend/browser-storage.md`](../../docs/frontend/browser-storage.md).

The host owns `TabModel` and terminal-session lifecycles. Workbench renders that model and its
terminal-creation Action calls `onNewTerminal`; when the callback is absent, the Action is disabled
and the tab strip retains its local fallback behavior. A desktop host may also pass
`createCrossWindowDragRuntime` to connect
the strip's pointer lifecycle to host-owned live-window discovery, compatibility checks, native
window movement, capture, and move-loop behavior. Workbench and the browser application do not
fabricate those operating-system capabilities.

Workbench owns `Mod+B` for `workbench.sidebar.toggle`, `Mod+N` for
`workbench.terminal.create`, `Mod+K` and `Mod+Shift+P` for the All and Actions inputs of
`workbench.search.open`, and `Mod+,` for `workbench.settings.open`. Apple platforms display `⌘B`,
`⌘N`, `⌘K`, `⌘⇧P`, and `⌘,`; standard platforms display `Ctrl B`, `Ctrl N`, `Ctrl K`,
`Ctrl Shift P`, and `Ctrl+,`. A shortcut is consumed only when its Action is currently enabled;
missing host capabilities remain visible but disabled.

## Development

```sh
pnpm --filter @orchbee/workbench... build
pnpm --filter @orchbee/workbench typecheck
pnpm --filter @orchbee/workbench test
pnpm --filter @orchbee/workbench test:exports
```
