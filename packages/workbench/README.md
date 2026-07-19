# @orchbee/workbench

Shared product-semantic React shell for Orchbee applications.

## Scope

The package currently owns one full-height, horizontally resizable two-panel shell. The left sidebar defaults to `250px` and is constrained to `200px`–`400px`; the right panel fills the remaining width. Its neutral divider always reserves one pixel. The divider uses the default border color at rest and the same stronger neutral border color during hover and active resizing without changing geometry. A persistent root-level icon control and the `Mod+B` shortcut hide or restore the sidebar. The control keeps the same DOM node and fixed screen position across visibility changes; while the sidebar is hidden, the tab bar reserves matching leading space beneath it so the first tab never overlaps the control. The left region and every right-panel tab body remain intentionally empty so product content can be introduced explicitly, one responsibility at a time. It composes generic primitives from `@orchbee/ui` without assigning navigation, terminal, session, or status behavior to either region.

The right panel accepts a prepared workspace-tab display model. Its tab bar is `35px` high: a `34px` content area plus a `1px` bottom border. Independent one-pixel divider elements sit between tabs and after the final tab; tab surfaces do not draw separator borders. The leading insertion divider is positioned over the list edge and never reserves permanent layout space. Unselected tabs use the brighter base surface, hover adds a lighter surface without changing the label color, and the active tab uses the darker selected surface. Each tab is `200px` wide when space permits and never exceeds that maximum; when the available width is insufficient, every tab shrinks by the same share and its label truncates. Hovering a tab reveals its localized close action; keyboard focus reveals that action only when the close control itself receives focus. A localized new-tab action follows the final divider rather than occupying the semantic tab list. The tab list clips overflow and never becomes a horizontal scrolling region. When tab movement is enabled, dragging uses a borderless label preview while the final one-pixel insertion boundary uses a non-layout shadow on both sides to appear three pixels wide. Crossing the midpoint of a hovered tab switches between its leading and trailing divider. Divider geometry never changes, and only the final drop commits one semantic move. Tab action controls do not initiate a drag. The caller owns tab identity, labels, authoritative selection, ordering, creation, closure, and the semantic effect of those actions.

Workbench owns the validation and versioned storage record for its device-local panel width. The consuming application owns the platform storage implementation and injects it at composition time. Application startup, routing roots, authentication, deployment, other persistence, platform integrations, and product capabilities remain outside this package. The machine-readable scope contract lives in [`project.json`](project.json).

## Use

Import the UI compatibility stylesheet once before the Workbench stylesheet at the application composition boundary:

```ts
import '@orchbee/ui/styles.css';
import '@orchbee/workbench/styles.css';
```

The consuming application selects the light or dark theme at that same boundary. Workbench uses the surrounding design-token theme for its two panel surfaces and divider.

Render the shell at the application composition boundary:

```tsx
import { Workbench, type WorkbenchLayoutStorage, type WorkbenchTab } from '@orchbee/workbench';

export function ProductInterface({
	layoutStorage,
	tabs,
	activeTabId,
	onActiveTabChange,
	onTabClose,
	onTabCreate,
	onTabMove,
	shortcutPlatform,
}: {
	readonly layoutStorage: WorkbenchLayoutStorage;
	readonly tabs: readonly WorkbenchTab[];
	readonly activeTabId: string;
	readonly onActiveTabChange: (tabId: string) => void;
	readonly onTabClose: (tabId: string) => void;
	readonly onTabCreate: () => void;
	readonly onTabMove: (tabId: string, targetIndex: number) => void;
	readonly shortcutPlatform: 'apple' | 'standard';
}) {
	return (
		<Workbench
			activeTabId={activeTabId}
			layoutStorage={layoutStorage}
			onActiveTabChange={onActiveTabChange}
			onTabClose={onTabClose}
			onTabCreate={onTabCreate}
			onTabMove={onTabMove}
			shortcutPlatform={shortcutPlatform}
			tabs={tabs}
		/>
	);
}
```

When storage is provided, Workbench restores `orchbee:workbench:left-panel-width:v1` and `orchbee:workbench:left-panel-visibility:v1`. It validates the complete JSON records, saves width only after a completed user resize, and saves visibility only after the button or shortcut invokes the semantic toggle action. Missing, malformed, unavailable, or out-of-range storage falls back to a visible `250px` sidebar without blocking rendering. The repository-wide ownership and key contract lives in [`docs/frontend/browser-storage.md`](../../docs/frontend/browser-storage.md).

The consuming application passes the validated shortcut platform: `apple` maps `Mod+B` to `⌘ B`, while `standard` maps it to `Ctrl B`. Workbench matches the exact chord, ignores repeated and composing keydown events, and exposes the same localized action through its icon button and delayed tooltip. Repository-wide shortcut and tooltip rules live in [`docs/frontend/keyboard-shortcuts.md`](../../docs/frontend/keyboard-shortcuts.md) and [`docs/frontend/tooltips.md`](../../docs/frontend/tooltips.md).

Workbench does not currently expose panel-content slots. A tab is a prepared identifier and label only; selecting or closing one reports its identifier to the caller, the new-tab action reports an unparameterized creation request, and a completed drag reports the moved identifier plus its final zero-based index. The caller applies that move to its authoritative tab array; Workbench never mutates or mirrors the order. Every tab body remains empty. Each product region will be added through an explicit package API when its ownership and interaction contract are defined.

## Development

```sh
pnpm --filter @orchbee/workbench... build
pnpm --filter @orchbee/workbench typecheck
pnpm --filter @orchbee/workbench test
pnpm --filter @orchbee/workbench test:exports
```
