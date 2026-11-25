import type { TabData, TabGroupData, TabModelSnapshot } from '../../../../../model/tab-model';
import type { TabGroupId } from '../../../../../model/tab-types';

/**
 * What one Tab button reads from the whole model. Each value is a primitive or a record whose
 * identity the model keeps while it is unchanged, so a memoized Tab button skips renders caused by
 * the rest of the strip: a selection change, for example, reaches only the Tabs it changes and
 * their separator neighbours. A neighbour is described by the values its separator reads, so an
 * edit to a neighbour's title does not reach this Tab.
 */
export interface TabButtonContext {
	readonly active: boolean;
	readonly selected: boolean;
	readonly group: Readonly<TabGroupData> | null;
	readonly tabPositionIndex: number;
	readonly tabCount: number;
	readonly hoveredByStrip: boolean;
	readonly hasPreviousTab: boolean;
	readonly previousGroup: TabGroupId | null;
	readonly previousActive: boolean;
	readonly previousSelected: boolean;
	readonly previousTabHovered: boolean;
	readonly hasNextTab: boolean;
	readonly nextGroup: TabGroupId | null;
	readonly nextActive: boolean;
	readonly nextSelected: boolean;
	readonly nextTabHovered: boolean;
}

interface SnapshotIndex {
	readonly tabPositions: ReadonlyMap<string, number>;
	readonly groups: ReadonlyMap<TabGroupId, Readonly<TabGroupData>>;
}

/** A snapshot never changes, so each one is indexed once for all of its Tab buttons. */
const snapshotIndexes = new WeakMap<TabModelSnapshot, SnapshotIndex>();

function indexSnapshot(snapshot: TabModelSnapshot): SnapshotIndex {
	const cached = snapshotIndexes.get(snapshot);
	if (cached !== undefined) return cached;
	const index: SnapshotIndex = {
		tabPositions: new Map(snapshot.tabs.map((tab, position) => [tab.id, position])),
		groups: new Map(snapshot.groups.map((group) => [group.id, group])),
	};
	snapshotIndexes.set(snapshot, index);
	return index;
}

export function tabButtonContext(
	tab: Readonly<TabData>,
	snapshot: TabModelSnapshot,
	hoveredTabId: string | null,
): TabButtonContext {
	const { tabPositions, groups } = indexSnapshot(snapshot);
	const tabPositionIndex = tabPositions.get(tab.id) ?? -1;
	const previousTab = snapshot.tabs[tabPositionIndex - 1];
	const nextTab = snapshot.tabs[tabPositionIndex + 1];
	return {
		active: tab.id === snapshot.activeTabId,
		selected: snapshot.selectedTabIds.has(tab.id),
		group: tab.group === null ? null : (groups.get(tab.group) ?? null),
		tabPositionIndex,
		tabCount: snapshot.tabs.length,
		hoveredByStrip: hoveredTabId === tab.id,
		hasPreviousTab: previousTab !== undefined,
		previousGroup: previousTab?.group ?? null,
		previousActive: previousTab !== undefined && previousTab.id === snapshot.activeTabId,
		previousSelected: previousTab !== undefined && snapshot.selectedTabIds.has(previousTab.id),
		previousTabHovered: previousTab !== undefined && hoveredTabId === previousTab.id,
		hasNextTab: nextTab !== undefined,
		nextGroup: nextTab?.group ?? null,
		nextActive: nextTab !== undefined && nextTab.id === snapshot.activeTabId,
		nextSelected: nextTab !== undefined && snapshot.selectedTabIds.has(nextTab.id),
		nextTabHovered: nextTab !== undefined && hoveredTabId === nextTab.id,
	};
}
