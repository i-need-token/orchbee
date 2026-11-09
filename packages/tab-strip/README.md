# @orchbee/tab-strip

Private package entries export only symbols consumed in this repository, enforced by Knip `includeEntryExports`. Internal contracts, generated assets and independent conformance evidence retain their owners; their presence does not automatically expand the public entry. New clients add exports for admitted uses.

Complete reusable React tab-strip capability. The package combines the public component, its domain model, layout rules, accessibility projections, drag interactions, and recursively owned private component tree behind one package entry. It is an Rslib library, not a standalone session.

## Scope

The package owns tab and group transitions, selection invariants, pinned and grouped ordering, horizontal and vertical layout, drag geometry, accessibility projections, hover cards, menus, and deterministic React event integration. Consumers own application composition, terminal sessions, navigation, persistence, product localization, theme token values, and platform host implementations.

## Entry points

```ts
import type { ActionRuntime, ActionScope } from '@orchbee/actions';
import { TabModel, TabStrip, TabStripOrientation, type TabStripMessages } from '@orchbee/tab-strip';
import '@orchbee/ui/styles.css';
import '@orchbee/tab-strip/styles.css';
```

- `@orchbee/tab-strip` exposes the component plus the Action identifiers, close and selection input types needed to host it. `TabIcon` and `getVisibleTabTitle` let a host draw one Tab outside the strip, such as a mobile bar's current Tab, with the strip's icon order (status mark or glyph, identity icon, adornment icon, favicon, then the default terminal icon) and title.
- `@orchbee/tab-strip/model` exposes the React-free `TabModel` with `TabModelInit` and `TabModelSnapshot` for headless consumers such as the Workbench project-workspace runtime. Its runtime closure contains no React, DOM, or stylesheet module; the package root continues to export `TabModel` for component hosts.
- `@orchbee/ui/styles.css` provides the shared primitive styles and design-token registry used by the context menu.
- `@orchbee/tab-strip/styles.css` provides the component's CSS Module output for built consumers.

Product hosts may provide the complete `TabStripMessages` contract to localize visible controls, menus, accessibility names, and live-region announcements. The built-in English messages remain a reference fallback for package tests and non-product fixtures; Orchbee product surfaces supply the contract from `@orchbee/i18n`. Visible empty and loading Tab titles use `newTabTitle` and `loadingTabTitle`; the latter is optional for older message providers and retains the reference English fallback when omitted. Workbench supplies both titles in every shipped locale and updates them on locale changes without replacing user-authored titles.

Product compositions pass their mounted `ActionRuntime` and parent `ActionScope`. The tab strip registers one child scope whose actions own selection, creation, close, grouping, pinning, preference, and committed movement semantics. Buttons, keyboard interactions, and context menus invoke those same actions. A standalone fixture may omit the runtime; the component then owns and disposes an isolated fallback runtime that is not visible to the application command surface.

Keyboard focus uses one manual-activation roving composite across visible Tabs and group headers. Orientation-relative arrows wrap without changing selection, Home and End reach the boundaries, Enter and Space activate the focused item, and Shift+F10 or Menu opens that exact item's context menu. The New Tab control follows the composite in sequential focus order. Structural changes retain the same visible focus identity where possible, project a hidden active Tab through its collapsed group header, and otherwise reconcile to a nearby live item. A host may increment `focusRequest` to enter the composite at its current roving item; repeated values are no-ops.

The Tab and group menus follow the TUI's composition. A row acting on the Tab the menu opened on shows only its verb; when that Tab is one of several selected, the rows that take the selection name it with its count. Rename and Pin lead, Add to group and Remove from group follow, and red Close rows end the Tab menu; a group's menu offers New Tab in group, Rename, Group color, Ungroup, and a red Close. Rows that act on the active Tab rather than the menu's Tab, such as duplicating or opening a Tab beside it, belong to the product's command surface, not to these menus.

A product composition may provide `onSetTabTitle`, which receives exactly the invoked Tab ID while the product owner retains title policy, editing, persistence, and focus restoration; without it, the Tab menu has no Rename row. It may provide `onNameGroup`, which receives a `TabStripGroupNameRequest` from a Tab's New group… row or a group's Rename row. The request names its target and suggested name, and its `commit(title)` creates or renames the group, where a blank title leaves the group unnamed; without the callback, neither row is offered. Menus show no shortcut hints. `newTabChoices` is the New Tab button's menu, in order: each choice has a product icon, a label, optional trailing words such as the mark on the default, and the callback a pick runs. The button's own click already creates a Tab, so with fewer than two choices, or while that click is unavailable, a right-click opens no menu.

A host may provide an `identityIcons` map keyed by Tab identity. Each value is a stable, decorative React icon that remains independent from transient status; it takes precedence over a status fallback icon, `faviconUrl`, and the default terminal icon without changing the accessible Tab name. A separate `statusAdornments` map may supply a host-owned fallback icon, localized accessible label, and one semantic visual kind (`active`, `attention`, `success`, `error`, `degraded`, or `idle`). It may also supply a `mark`, a ReactNode that Tab Strip draws as given in the status slot where a `glyph` would draw; the mark takes precedence over the glyph and suppresses the status dot, and the strip adds no kind color or animation to it. Tab Strip composes the status indicator and accessible label independently from the identity icon without mutating `TabData`, overloading loading/crash/attention fields, or learning the host's business domain. The host remains responsible for identity selection, aggregation, localization, status lifetime, and meaning.

Local composition closes known Tab IDs synchronously through `TabModel`. A terminal-owning composition instead supplies `onCloseTabs`. Tab Strip asks no question before it, so a close that needs one asks it itself. The `tabs.tab.close` Action delegates its validated ID set and execution context without mutating `TabModel`; the owner marks the commit point and removes Tabs only after its terminal-close orchestration succeeds. A pending or partially failed close therefore cannot make Tab Strip delete topology early. Such a close outlasts the press that asked for it, and a middle click reports itself through both `pointerup` and `auxclick`, so the Action refuses a second request for a Tab set whose close is still running. Such a composition may also supply `closeResume` for a close that stopped where it may continue: the Tab menu reads `available(tabId)` as it opens and then offers Resume closing this Tab… right before its Close rows, and `resume(tabId)` receives exactly the Tab the menu opened on, even inside a selected set.

A composition whose URL is authoritative may supply `onSelectTab`. The `tabs.tab.select` Action delegates its validated `TabSelectionInput` and execution context before any local `TabModel` write. Validation remains owned by the registered Tab Strip Action. The owner commits navigation, then applies the selection when that route is observed. Omitting the callback preserves the package-owned local selection transition. This port does not move routing, history, or URL parsing into Tab Strip.

Pointer movement and drag geometry remain local gesture state. Model changes shown during a drag are reversible previews; accepting a local drop puts the Tabs and groups back, leaving the selection the press made, and invokes `tabs.layout.apply-arrangement` to commit the final arrangement through the Action lifecycle. Taking the selection back too, even for that moment, would send a routed composition's navigation after a Tab the person has already left.

A composition whose Tabs another authority decides supplies `onTabsChange`. Actions that arrange Tabs and groups, create a Tab in a new group, or duplicate Tabs then report one `TabStripChange` instead of changing `TabModel`, and the answer arrives as a new `model`. Selection and collapsing stay local, and a move that lands on a collapsed group says so, because that is the one thing the authority cannot know. A drag reports once, at release, with its boundary in the order it started from, derived from the arrangements before and after; a release that changed nothing reports nothing. The released arrangement stays on screen until the result settles and returns, keeping the current selection, unless that result completed. A new `model` under a held pointer ends a started drag as Escape does. A Tab press continues from the new model, and a group press gives up its drag and stays a click, because that drag was armed against the arrangement it saw. Movement announcements wait for a completed result, and creating a group with a Tab or duplicating refuses a second request while one runs, because an authority can take longer to answer than a person takes to press again. Grouping Tabs reports the new group's name and color with the Tabs it holds, and renaming a group reports one change when the host commits the name.

Internally created controllers and the fallback Action runtime belong to the mounted Tab Strip, even while an injected instance is selected. Final unmount clears their timers and requests cancellation of active executions; injected instances remain host-owned. Effect replay preserves original alert and combo-button deadlines rather than restarting semantic transitions. Replacing the combo-button controller reconciles the incoming instance with controlled Tab Search state; an already matching or closing instance retains its current deadline, and the outgoing instance is left to its owner.

During vertical edge scrolling, the mounted timer uses the latest committed drag callback. A suspended render cannot redirect the running gesture; unmounting stops the timer and removes the overflow gradient.

Horizontal hosts may provide `horizontalLeadingActions` for controls that must share the tab-strip surface. The tab track consumes only the remaining width, so hosted controls never overlap the first tab or require a separately painted sibling region. Horizontal layout uses a `36px` strip, item, and drag boundary; the `28px` new-tab control and `20px` group chips and separators are vertically centered within that row. Vertical geometry remains independent: rows are `30px`, pinned tiles `32px`. With `touchUi`, a vertical row, group header, and pinned tile are each a `44px` target and rows stand `2px` apart. `verticalFill` lets a vertical strip take its host's full inline size and surface, as a full-window Tab list does, instead of its own expanded or collapsed width and track. Its rows start `2px` below its top, groups keep the rows' inset instead of spanning the strip, and hover cards stay off, since no room is left beside the rows. A plain touch chooses its Tab on release instead of on press, because such a strip scrolls under the finger, and a pan that takes the touch chooses nothing.

New Tab and Close use the shared `ProductIcon` glyphs and compact system control states. Chromium-derived layout and interaction geometry does not determine their product icon size, semantic color, or hover shape.

Double-clicking the horizontal track's own blank space with the plain primary button creates a Tab through the same `tabs.tab.create` path as the New Tab control, while creation is available; a single click there does nothing, and a double-click on a Tab, a group, or the New Tab control keeps that target's behavior. A host whose blank track is an operating-system window drag region never receives that double-click.

Hosts that support both orientations pass the controlled `orientation` value and `onOrientationChange`. Right-clicking blank strip space then exposes the localized action for opening or closing vertical tabs; tab, group, and new-tab controls keep their own context menus.

The package API intentionally has no React or browser-vendor subpath. Upstream source attribution and license obligations are recorded in [`NOTICE.reference`](NOTICE.reference).

## Source ownership

```text
src/
├── index.ts
├── styles.ts
└── tab-strip/
    ├── index.ts
    ├── TabStrip.tsx
    ├── components/
    │   └── tab-strip-view/
    │       ├── TabStripView.tsx
    │       └── components/
    │           ├── horizontal-contents/
    │           ├── vertical-contents/
    │           ├── tab-button/
    │           ├── group-header/
    │           ├── drag-ghost/
    │           ├── hover-card/
    │           └── tab-strip-context-menu/
    ├── model/
    ├── layout/
    ├── dragging/
    ├── accessibility/
    ├── contexts/
    ├── hooks/
    ├── constants/
    ├── types/
    └── utils/
```

`TabStrip.tsx` is the narrow public adapter. `TabStripView` composes the prepared horizontal, vertical, overlay, and menu regions. Private components repeat the same ownership structure recursively and have no barrels. The four semantic subsystems are cohesive facets of this complete component capability, not separately consumable source modules, except that `tab-strip/model/index.ts` exposes the React-free `TabModel` and its init and snapshot types as `@orchbee/tab-strip/model`. Other selected public contracts are re-exported only through `tab-strip/index.ts` and the package root.

## Correctness

Unit tests use Node by default. Tests that render components or Hooks opt into jsdom with
`// @vitest-environment jsdom`; pure model, layout, and policy suites do not load a DOM environment.

The general `TabModel` state machine has three complementary layers:

- deterministic regressions preserve each known minimal counterexample;
- bounded state-space exploration enumerates every modeled selection, close, pin, group, and keyboard-movement transition through depth three from a canonical grouped seed (15,497 transitions and 952 distinct states);
- fixed-seed property tests exercise 200 reproducible action sequences of up to 100 transitions.

An independent test invariant checker validates tab and group identity, the pinned prefix, contiguous group membership, active and selected identities, the selection anchor, and recent-group references after every transition. The production model asserts the same structural boundaries so invalid restored state and invalid intermediate mutations fail at their source.

Drag restoration prepares copied membership and selection before validating the complete candidate.
A structural refusal leaves the live model, snapshot, revision, and future mutations intact. It keeps
the existing filtering of deleted tabs and stale selection, and appends current-only tabs; a candidate
whose pinned order or group topology conflicts with later changes is still refused, without inventing
a new merge order. The arrangement Action rejects duplicate tab or group IDs at input and reports an
invalid candidate as `blocked` before commit.

Tab creation validates its complete insertion candidate before changing live state or consuming an
automatic ID. The synchronous will-add hooks still run before insertion and may restore a drag;
creation revalidates against their resulting state, without rolling back those independently committed
changes. Local creation and move-to-group Actions check destination liveness at discovery and execution.
The creation Action reports an invalid candidate as `blocked` and records commit only after insertion.

Drag movement and direct group placement validate copied arrangements before replacing live authority.

Committed model notifications capture one listener cohort, skip listeners removed before their turn,
and isolate observer exceptions. The synchronous arrangement handler records its commit immediately
after restoration returns; cancellation or registration retirement during publication cannot relabel
that committed result. The [model regressions](src/tab-strip/model/__tests__/tab-model.unit.test.ts)
and [Action traces](src/tab-strip/hooks/__tests__/useTabStripActions.unit.test.ts) exercise rejection,
continued model use, observer faults, retirement, and nested duplicate execution.

The shared [Tab configuration corpus](../../schemas/workbench/v1/tab-configuration-conformance-v1.json) compares GUI `TabModel` transitions with the Rust canonical authority for pinning, group creation, assignment, removal, metadata and group movement. Creating a new group from the middle member of a surviving group inserts after that source block, preserving contiguous membership without changing the active Tab.

The context menu consumes `getTabCloseTargets` and `getGroupCloseTabIds`, and Workbench's Close Tabs after the active Tab consumes the trailing set. Their [close-target corpus](../../schemas/workbench/v1/tab-close-targets-conformance-v1.json) also runs against the native Client: target-in-selection controls the command set, relative Close excludes pinned/protected members, trailing begins after the rightmost selected Tab, and group Close includes collapsed members. This compares target policy; lifecycle sequencing and empty-group cleanup remain with each admitted host owner.

Horizontal group drag carries a Lean proof for contiguous-block movement, generated implementation/property tests and deterministic browser traces. See the [correctness evidence](formal/README.md) for mapping, reproducible commands and limits. Local drag and grouping workflows have no separate TLA model.

```sh
pnpm --filter @orchbee/tab-strip typecheck
pnpm --filter @orchbee/tab-strip test
pnpm --filter @orchbee/tab-strip verify:correctness
pnpm --filter @orchbee/tab-strip test:exports
```

Pixel-exact animation midpoint witnesses control one shared monotonic clock for pickup, frame callbacks and pointer retargeting. They retain exact geometry assertions, including an unchanged offset after retargeting that shows the start time held; constructing a midpoint from a live fractional timestamp can cross an integer-pixel rounding boundary.
