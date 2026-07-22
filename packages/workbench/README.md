# @orchbee/workbench

Shared product-semantic React shell for Orchbee applications.

The current tab-state, pointer-interaction, geometry, and verification contract lives in
[`docs/tabs.md`](docs/tabs.md).

## Scope

The package currently owns one full-height, horizontally resizable two-panel shell. The left sidebar defaults to `250px` and is constrained to `200px`–`400px`; the right panel fills the remaining width. Its neutral divider always reserves one pixel. The divider uses the default border color at rest and the same stronger neutral border color during hover and active resizing without changing geometry. A persistent root-level icon control and the `Mod+B` shortcut hide or restore the sidebar. The control keeps the same DOM node and fixed screen position across visibility changes; while the sidebar is hidden, the tab bar reserves matching leading space beneath it so the first tab never overlaps the control. The localized sidebar chrome presents only the `New Terminal` and `Search` controls above a `Project` section heading, plus a bottom-anchored `Settings` control that stays separate from project content. Each control uses the `30px` navigation size, adds a neutral hover and keyboard-focus surface, and reveals its platform-formatted shortcut at the trailing edge without shifting the label. Every right-panel tab body remains intentionally empty so product content can be introduced explicitly, one responsibility at a time. It composes generic primitives from `@orchbee/ui` without owning terminal, search, settings, project, session, or status lifecycles.

The right panel accepts a prepared workspace-tab display model. Its tab bar is `35px` high: a `34px` content area plus a `1px` bottom border. A subtle one-pixel separator is painted on each visual tab or group segment without creating a DOM sibling or reserving layout width. Hovering, activating, or dragging a tab suppresses the separators on both sides so surface emphasis remains stronger than internal boundaries. Unselected tabs use the brighter base surface, hover adds a lighter surface without changing the label color, and the active tab uses the darker selected surface. Each tab is `200px` wide when space permits and never exceeds that maximum; when the available width is insufficient, every tab shrinks by the same share and its label truncates. Standard-width unpinned tabs keep their localized close action visible. Compact non-active tabs hide it until hover or close-control focus, while the active action remains available. An icon-only tab centers its optional semantic icon and lets the close action cover that same center without shifting the icon or neighboring layout. Mouse close presses preserve every surviving tab's measured width while the pointer remains inside the tab cluster, allowing repeated closes at one screen position; leaving the cluster restores flexible distribution, while `Mod+W` and context-menu closes reflow immediately. A pinned tab replaces that trailing close action with a persistent pin action; invoking it reports an unpin request and restores the close-action state when the caller updates the model. Pinned tabs form a stable leading group, and reorder resolution rejects every move across the boundary so pinned and unpinned tabs can only reorder within their own groups. Unpinned tabs may reference a caller-owned group; group members render contiguously after a centered `24px` header control whose complete label background carries the group color. An expanded header shares the same uninterrupted bottom color line as its member tabs; collapsing removes both the member triggers and the header line. A primary click reports the opposite collapsed state, while right-clicking opens the same Workbench context-menu surface used by tabs for creating, renaming through the controlled dialog, recoloring, closing, ungrouping, deleting, and creating a tab inside the group. A primary outside press dismisses either Workbench context menu and immediately invokes an underlying tab, tab action, group header, or new-tab control, so switching tabs never requires a second click; secondary outside presses never synthesize a primary activation and therefore cannot toggle group disclosure while reopening its context menu. Collapsed groups keep their member models and content while hiding the member tab triggers. Moving an unpinned tab onto a group header or grouped member previews that target group and immediately extends the group-color line across both the dragged overlay and its slightly overlapping provisional layout slot, so the expanded group line remains continuous before release; moving it onto an ungrouped member removes both line segments after the group-boundary hysteresis is crossed. Pinning a grouped tab removes its group membership before moving it into the leading pinned region. Right-clicking an enabled tab opens a localized context menu for closing the target, other tabs, tabs to its right, or all tabs; pinning or unpinning; grouping; renaming through a controlled dialog; and selecting the default or one of nine named colors. Empty group metadata is excluded from tab assignment choices. Bulk-close targets are reported as one ordered identifier array, unavailable target sets are disabled, and the caller remains authoritative for every resulting state change. Pinned, grouped, collapsed, colored, and optional semantic-icon display state is prepared on the tab and group models and rendered without changing tab identity. A localized new-tab action follows the tab list rather than occupying it. The tab list clips overflow and never becomes a horizontal scrolling region. When tab movement is enabled, only a primary pointer down whose DOM target is physically contained by the tab or group item can arm a reorder, so events bubbling from a portaled context menu are excluded. Each move must confirm that the primary button remains pressed; otherwise the candidate or active drag is canceled. After a ten-pixel Euclidean threshold, Workbench captures the pointer, immediately reports the dragged tab as active, keeps its selected surface while adding the raised drag shadow, and updates a transient local order from the canonical strip projection. Active-surface changes are immediate during reordering so the previously active tab never leaves a fading background behind. The fast design-token motion lets displaced tabs and group headers slide into their new provisional positions without a separate insertion indicator. Pointer cancellation restores the prepared order; pointer release reports one semantic move containing the final index and group, then retains that final preview across any number of frames until the caller's authoritative topology acknowledges it or changes to a different topology that explicitly rejects it. Tab action controls do not initiate a reorder, reduced-motion preferences make displacement immediate, and the caller remains authoritative for the committed tab model.

Workbench owns the validation and versioned storage record for its device-local panel width. The consuming application owns the platform storage implementation and injects it at composition time. Application startup, routing roots, authentication, deployment, other persistence, platform integrations, and product capabilities remain outside this package. The machine-readable scope contract lives in [`project.json`](project.json).

Dragging a group header moves its header and all members as one contiguous block in either expanded or collapsed state. Crossing the drag threshold preserves an active member of that group; when the active tab is outside the group, Workbench reports the group's first tab as active before movement begins, while the header itself never becomes a tab selection target. Dragged tab and group surfaces use an opaque overlay base beneath their selected or colored treatment so underlying labels cannot show through. Every enabled final action in a tab or group context menu dismisses the complete menu tree immediately after invocation; submenu triggers remain open only while choosing their final action.

## Use

Import the UI compatibility stylesheet once before the Workbench stylesheet at the application composition boundary:

```ts
import '@orchbee/ui/styles.css';
import '@orchbee/workbench/styles.css';
```

The consuming application selects the light or dark theme at that same boundary. Workbench uses the surrounding design-token theme for its two panel surfaces and divider.

Render the shell at the application composition boundary:

```tsx
import {
	Workbench,
	type WorkbenchLayoutStorage,
	type WorkbenchTab,
	type WorkbenchTabColor,
	type WorkbenchTabGroup,
	type WorkbenchTabGroupUpdate,
} from '@orchbee/workbench';

export function ProductInterface({
	layoutStorage,
	tabs,
	activeTabId,
	onActiveTabChange,
	onTabClose,
	onTabColorChange,
	onTabCreate,
	onTabGroupAssign,
	onTabGroupChange,
	onTabGroupClose,
	onTabGroupCollapseChange,
	onTabGroupCreate,
	onTabGroupDelete,
	onTabGroupMove,
	onTabGroupUngroup,
	onTabMove,
	onTabPinChange,
	onTabRename,
	onTabsClose,
	onSearch,
	onSettings,
	shortcutPlatform,
	tabGroups,
}: {
	readonly layoutStorage: WorkbenchLayoutStorage;
	readonly tabs: readonly WorkbenchTab[];
	readonly activeTabId: string;
	readonly onActiveTabChange: (tabId: string) => void;
	readonly onTabClose: (tabId: string) => void;
	readonly onTabColorChange: (tabId: string, color: WorkbenchTabColor) => void;
	readonly onTabCreate: (groupId?: string) => void;
	readonly onTabGroupAssign: (tabId: string, groupId: string | undefined) => void;
	readonly onTabGroupChange: (groupId: string, update: WorkbenchTabGroupUpdate) => void;
	readonly onTabGroupClose: (groupId: string) => void;
	readonly onTabGroupCollapseChange: (groupId: string, isCollapsed: boolean) => void;
	readonly onTabGroupCreate: (tabId: string) => void;
	readonly onTabGroupDelete: (groupId: string) => void;
	readonly onTabGroupMove: (groupId: string, targetIndex: number) => void;
	readonly onTabGroupUngroup: (groupId: string) => void;
	readonly onTabMove: (tabId: string, targetIndex: number, groupId: string | undefined) => void;
	readonly onTabPinChange: (tabId: string, isPinned: boolean) => void;
	readonly onTabRename: (tabId: string, label: string) => void;
	readonly onTabsClose: (tabIds: readonly string[]) => void;
	readonly onSearch: () => void;
	readonly onSettings: () => void;
	readonly shortcutPlatform: 'apple' | 'standard';
	readonly tabGroups: readonly WorkbenchTabGroup[];
}) {
	return (
		<Workbench
			activeTabId={activeTabId}
			layoutStorage={layoutStorage}
			onActiveTabChange={onActiveTabChange}
			onTabClose={onTabClose}
			onTabColorChange={onTabColorChange}
			onTabCreate={onTabCreate}
			onTabGroupAssign={onTabGroupAssign}
			onTabGroupChange={onTabGroupChange}
			onTabGroupClose={onTabGroupClose}
			onTabGroupCollapseChange={onTabGroupCollapseChange}
			onTabGroupCreate={onTabGroupCreate}
			onTabGroupDelete={onTabGroupDelete}
			onTabGroupMove={onTabGroupMove}
			onTabGroupUngroup={onTabGroupUngroup}
			onTabMove={onTabMove}
			onTabPinChange={onTabPinChange}
			onTabRename={onTabRename}
			onTabsClose={onTabsClose}
			onSearch={onSearch}
			onSettings={onSettings}
			shortcutPlatform={shortcutPlatform}
			tabGroups={tabGroups}
			tabs={tabs}
		/>
	);
}
```

When storage is provided, Workbench restores `orchbee:workbench:left-panel-width:v1` and `orchbee:workbench:left-panel-visibility:v1`. It validates the complete JSON records, saves width only after a completed user resize, and saves visibility only after the button or shortcut invokes the semantic toggle action. Missing, malformed, unavailable, or out-of-range storage falls back to a visible `250px` sidebar without blocking rendering. The repository-wide ownership and key contract lives in [`docs/frontend/browser-storage.md`](../../docs/frontend/browser-storage.md).

The consuming application passes the validated shortcut platform. Workbench owns `Mod+B` for sidebar visibility, `Mod+W` for closing the active tab when both its identifier and `onTabClose` are available, `Mod+N` for the available new-tab action shown as `New Terminal`, `Mod+K` for an available `onSearch` action, and `Mod+,` for an available `onSettings` action. Apple platforms display `⌘B`, `⌘W`, `⌘N`, `⌘K`, and `⌘,`; standard platforms display `Ctrl B`, `Ctrl W`, `Ctrl N`, `Ctrl K`, and `Ctrl+,`. Workbench matches exact chords, ignores repeated and composing keydown events, and prevents browser defaults only when the matching action is available. The close shortcut appears beside the context-menu action, sidebar controls reveal their shortcuts on hover or keyboard focus, and the visibility action remains available through its delayed tooltip. Repository-wide shortcut and tooltip rules live in [`docs/frontend/keyboard-shortcuts.md`](../../docs/frontend/keyboard-shortcuts.md) and [`docs/frontend/tooltips.md`](../../docs/frontend/tooltips.md).

Workbench does not currently expose panel-content slots. A tab supplies a prepared identifier and label plus optional `icon`, `isPinned`, `groupId`, and `color` display state; the current semantic-icon vocabulary contains `terminal`. Each referenced group supplies a stable identifier, label, color, and optional `isCollapsed` state. Selecting, closing, pinning, grouping, collapsing, renaming, or recoloring reports the corresponding semantic values to the caller; bulk close reports one ordered identifier array; the sidebar `New Terminal` control and tab-bar new-tab action report an ungrouped creation request while a group action supplies its identifier; `Search` and `Settings` report through their optional callbacks; and a completed pointer reorder reports the moved identifier, its final zero-based index, and the target group identifier or `undefined`. `createWorkbenchTabsState` establishes the canonical pinned-first, contiguous-group model, and `reduceWorkbenchTabs` applies every accepted state action through the same invariant-preserving operation used by the Web preview. Missing sidebar callbacks remain visible for layout continuity, carry `aria-disabled`, and are excluded from global shortcut handling. Missing tab capabilities remain disabled, or the context menu is omitted when no menu action is available. The caller applies every committed change to its authoritative tab and group arrays and removes group metadata after its final tab leaves. Collapsing a group hides its member triggers without changing `activeTabId`; expanding it restores the active member trigger. Workbench never mutates caller-owned arrays; it owns only the transient reorder projection between the drag threshold and authoritative acknowledgement, rejection, or cancellation. Every tab body remains empty. Each product workflow will be added through an explicit package API when its ownership and interaction contract are defined.

A completed group reorder reports the group identifier and its first tab's final zero-based index. The `move-tab-group` reducer action applies that committed move atomically while preserving member order and rejecting movement into the pinned region.

## Development

```sh
pnpm --filter @orchbee/workbench... build
pnpm --filter @orchbee/workbench typecheck
pnpm --filter @orchbee/workbench test
pnpm --filter @orchbee/workbench test:exports
```
