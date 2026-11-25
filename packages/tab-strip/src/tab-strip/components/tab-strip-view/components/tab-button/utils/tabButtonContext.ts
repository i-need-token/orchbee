import type { TabData, TabGroupData, TabModelSnapshot } from '../../../../../model/tab-model';

/**
 * What one Tab button reads from the whole model. Each value is a primitive or a record whose
 * identity the model keeps while it is unchanged, so a memoized Tab button skips renders caused by
 * the rest of the strip: a selection change, for example, reaches only the Tabs it changes and
 * their separator neighbours.
 */
export interface TabButtonContext {
	readonly active: boolean;
	readonly selected: boolean;
	readonly group: Readonly<TabGroupData> | null;
	readonly tabPositionIndex: number;
	readonly tabCount: number;
	readonly hoveredByStrip: boolean;
	readonly previousTab: Readonly<TabData> | undefined;
	readonly previousActive: boolean;
	readonly previousSelected: boolean;
	readonly previousTabHovered: boolean;
	readonly nextTab: Readonly<TabData> | undefined;
	readonly nextActive: boolean;
	readonly nextSelected: boolean;
	readonly nextTabHovered: boolean;
}

export function tabButtonContext(
	tab: Readonly<TabData>,
	snapshot: TabModelSnapshot,
	hoveredTabId: string | null,
): TabButtonContext {
	const tabPositionIndex = snapshot.tabs.findIndex((candidate) => candidate.id === tab.id);
	const previousTab = snapshot.tabs[tabPositionIndex - 1];
	const nextTab = snapshot.tabs[tabPositionIndex + 1];
	return {
		active: tab.id === snapshot.activeTabId,
		selected: snapshot.selectedTabIds.has(tab.id),
		group:
			tab.group === null
				? null
				: (snapshot.groups.find((candidate) => candidate.id === tab.group) ?? null),
		tabPositionIndex,
		tabCount: snapshot.tabs.length,
		hoveredByStrip: hoveredTabId === tab.id,
		previousTab,
		previousActive: previousTab?.id === snapshot.activeTabId,
		previousSelected: previousTab !== undefined && snapshot.selectedTabIds.has(previousTab.id),
		previousTabHovered: previousTab !== undefined && hoveredTabId === previousTab.id,
		nextTab,
		nextActive: nextTab?.id === snapshot.activeTabId,
		nextSelected: nextTab !== undefined && snapshot.selectedTabIds.has(nextTab.id),
		nextTabHovered: nextTab !== undefined && hoveredTabId === nextTab.id,
	};
}
