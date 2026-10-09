# @orchbee/tab-strip

Complete reusable React tab-strip capability. The package combines the public component, its domain model, layout rules, accessibility projections, drag interactions, and recursively owned private component tree behind one package entry. It is an Rslib library, not a standalone site.

## Scope

The package owns tab and group transitions, selection invariants, pinned and grouped ordering, horizontal and vertical layout, drag geometry, accessibility projections, hover cards, menus, and deterministic React event integration. Consumers own application composition, terminal sessions, navigation, persistence, product localization, theme token values, and platform or cross-window host implementations.

## Entry points

```ts
import type { ActionRuntime, ActionScope } from '@orchbee/actions';
import { TabModel, TabStrip, TabStripOrientation, type TabStripMessages } from '@orchbee/tab-strip';
import '@orchbee/ui/styles.css';
import '@orchbee/tab-strip/styles.css';
```

- `@orchbee/tab-strip` exposes the component plus the Action identifiers, close and selection inputs, and boundary validators needed to host it.
- `@orchbee/ui/styles.css` provides the shared primitive styles and design-token registry used by the context menu.
- `@orchbee/tab-strip/styles.css` provides the component's CSS Module output for built consumers.

Product hosts may provide the complete `TabStripMessages` contract to localize visible controls, menus, accessibility names, and live-region announcements. The built-in English messages remain a reference fallback for package tests and non-product fixtures; Orchbee product surfaces supply the contract from `@orchbee/i18n`.

Product compositions pass their mounted `ActionRuntime` and parent `ActionScope`. The tab strip registers one child scope whose actions own selection, creation, close, grouping, pinning, preference, and committed movement semantics. Buttons, keyboard interactions, and context menus invoke those same actions. A standalone fixture may omit the runtime; the component then owns and disposes an isolated fallback runtime that is not visible to the application command surface.

Keyboard focus uses one manual-activation roving composite across visible Tabs and group headers. Orientation-relative arrows wrap without changing selection, Home and End reach the boundaries, Enter and Space activate the focused item, and Shift+F10 or Menu opens that exact item's context menu. The New Tab control follows the composite in sequential focus order. Structural changes retain the same visible focus identity where possible, project a hidden active Tab through its collapsed group header, and otherwise reconcile to a nearby live item. A host may increment `focusRequest` to enter the composite at its current roving item; repeated values are no-ops.

A product composition may provide `shortcutLabels` for New Tab, Set Tab Title, and contextual Close. It may also provide `onSetTabTitle`, which receives exactly the invoked Tab ID while the product owner retains title policy, editing, persistence, and focus restoration. Context menus render only non-empty, platform-formatted current bindings in the shared trailing shortcut slot; Tab Strip does not invent bindings for commands that have none.

A host may provide an `identityIcons` map keyed by Tab identity. Each value is a stable, decorative React icon that remains independent from transient status; it takes precedence over a status fallback icon, `faviconUrl`, and the default favicon without changing the accessible Tab name. A separate `statusAdornments` map may supply a host-owned fallback icon, localized accessible label, and one semantic visual kind (`active`, `attention`, `success`, `error`, `degraded`, or `idle`). Tab Strip composes the status indicator and accessible label independently from the identity icon without mutating `TabData`, overloading loading/crash/attention fields, or learning the host's business domain. The host remains responsible for identity selection, aggregation, localization, status lifetime, and meaning.

Local composition closes known Tab IDs synchronously through `TabModel`. A terminal-owning composition instead supplies `onCloseTabs`. It may also supply `getCloseTabsConfirmation`, which lets the owner derive protection from the complete resources affected by the validated Tab set while Tab Strip keeps confirmation inside the same `tabs.tab.close` Action. The Action delegates its validated ID set and execution context without mutating `TabModel`; the owner marks the commit point and removes Tabs only after its terminal-close orchestration succeeds. A cancelled confirmation, pending close, or partially failed close therefore cannot make Tab Strip delete topology early.

A composition whose URL is authoritative may supply `onSelectTab`. The `tabs.tab.select` Action delegates its validated `TabSelectionInput` and execution context before any local `TabModel` write; the public `tabSelectionInput` validator lets a composing Action prepare that same boundary without importing private source. The owner commits navigation, then applies the selection when that route is observed. Omitting the callback preserves the package-owned local selection transition. This port does not move routing, history, or URL parsing into Tab Strip.

Pointer movement and drag geometry remain local gesture state. Model changes shown during a drag are reversible previews; accepting a local drop rolls the preview back and invokes `tabs.layout.apply-arrangement` to commit the final arrangement through the Action lifecycle.

Horizontal hosts may provide `horizontalLeadingActions` for controls that must share the tab-strip surface. The tab track consumes only the remaining width, so hosted controls never overlap the first tab or require a separately painted sibling region. Horizontal layout uses a `36px` strip, item, and drag boundary; the `28px` new-tab control and `20px` group chips and separators are vertically centered within that row. Vertical geometry remains independent.

New Tab and Close use the shared `ProductIcon` glyphs and compact system control states. Chromium-derived layout and interaction geometry does not determine their product icon size, semantic color, or hover shape.

Hosts that support both orientations pass the controlled `orientation` value and `onOrientationChange`. Right-clicking blank strip space then exposes the localized action for opening or closing vertical tabs; tab, group, and new-tab controls keep their own context menus.

The package API intentionally has no React or browser-vendor subpath. Desktop compositions import the cross-window contracts from the package root: `CrossWindowTabDragRuntime` owns model handoff, cancel restoration, attached-model liveness, and destination layout sequencing; `DomTabDragRegistry` adapts live bounds, z-order, liveness, and compatibility decisions supplied by the host. `TabStrip` accepts a `createCrossWindowDragRuntime` factory, while native window creation, movement, capture, move loops, and source-window disposal remain host responsibilities. The browser application does not synthesize those operating-system capabilities. Upstream source attribution and license obligations are recorded in [`NOTICE.reference`](NOTICE.reference).

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

`TabStrip.tsx` is the narrow public adapter. `TabStripView` composes the prepared horizontal, vertical, overlay, and menu regions. Private components repeat the same ownership structure recursively and have no barrels. The four semantic subsystems are cohesive facets of this complete component capability, not separately consumable source modules; selected public contracts are re-exported only through `tab-strip/index.ts` and the package root.

## Correctness

The general `TabModel` state machine has three complementary layers:

- deterministic regressions preserve each known minimal counterexample;
- bounded state-space exploration enumerates every modeled selection, close, pin, group, and keyboard-movement transition through depth three from a canonical grouped seed (15,497 transitions and 952 distinct states);
- fixed-seed property tests exercise 200 reproducible action sequences of up to 100 transitions.

An independent test invariant checker validates tab and group identity, the pinned prefix, contiguous group membership, active and selected identities, the selection anchor, and recent-group references after every transition. The production model asserts the same structural boundaries so invalid restored state and invalid intermediate mutations fail at their source.

The shared [Tab configuration corpus](../../schemas/workbench/v1/tab-configuration-conformance-v1.json) compares GUI `TabModel` transitions with the Rust canonical authority for pinning, group creation, assignment, removal, metadata and group movement. Creating a new group from the middle member of a surviving group inserts after that source block, preserving contiguous membership without changing the active Tab.

Horizontal group drag additionally carries a TLA+ model, a Lean proof for contiguous-block movement, focused property tests, and deterministic browser traces. See the [`horizontal group-drag contract`](docs/horizontal-group-drag.md) and [`formal/README.md`](formal/README.md).

```sh
pnpm --filter @orchbee/tab-strip typecheck
pnpm --filter @orchbee/tab-strip test
pnpm --filter @orchbee/tab-strip verify:correctness
pnpm --filter @orchbee/tab-strip test:exports
```

Pixel-exact animation midpoint witnesses control one shared monotonic clock for pickup, frame callbacks and pointer retargeting. They retain exact geometry and unchanged-start-time assertions; constructing a midpoint from a live fractional timestamp can cross an integer-pixel rounding boundary.
