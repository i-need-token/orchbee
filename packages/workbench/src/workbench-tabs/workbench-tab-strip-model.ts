import type {
	WorkbenchTab,
	WorkbenchTabGroup,
	WorkbenchTabsState,
	WorkbenchTabsStateInput,
} from './workbench-tabs-state';

interface WorkbenchTabStripSelection {
	readonly activeTabId: string | undefined;
	readonly anchorTabId: string | undefined;
	readonly selectedTabIds: readonly string[];
}

interface WorkbenchTabStripTabNode {
	readonly kind: 'tab';
	readonly tab: WorkbenchTab;
}

interface WorkbenchTabStripGroupNode {
	readonly group: WorkbenchTabGroup;
	readonly kind: 'group';
	readonly tabs: readonly WorkbenchTab[];
}

type WorkbenchTabStripNode = WorkbenchTabStripGroupNode | WorkbenchTabStripTabNode;

export interface WorkbenchTabStripModel {
	readonly groups: readonly WorkbenchTabGroup[];
	readonly items: readonly WorkbenchTabStripNode[];
	readonly pinnedTabs: readonly WorkbenchTab[];
	readonly selection: WorkbenchTabStripSelection;
}

function assertUniqueIdentifiers(values: readonly { readonly id: string }[], owner: string): void {
	const identifiers = new Set<string>();
	for (const value of values) {
		if (identifiers.has(value.id)) throw new Error(`Duplicate ${owner} identifier: ${value.id}`);
		identifiers.add(value.id);
	}
}

function withoutGroup(tab: WorkbenchTab): WorkbenchTab {
	if (tab.groupId === undefined) return tab;
	const { groupId: _groupId, ...ungroupedTab } = tab;
	return ungroupedTab;
}

function asPinned(tab: WorkbenchTab): WorkbenchTab {
	const ungroupedTab = withoutGroup(tab);
	return ungroupedTab.isPinned === true ? ungroupedTab : { ...ungroupedTab, isPinned: true };
}

function asUnpinned(tab: WorkbenchTab): WorkbenchTab {
	if (tab.isPinned !== true) return tab;
	const { isPinned: _isPinned, ...unpinnedTab } = tab;
	return unpinnedTab;
}

function withGroup(tab: WorkbenchTab, groupId: string): WorkbenchTab {
	const unpinnedTab = asUnpinned(tab);
	return unpinnedTab.groupId === groupId ? unpinnedTab : { ...unpinnedTab, groupId };
}

function getAllTabs(model: WorkbenchTabStripModel): readonly WorkbenchTab[] {
	return [
		...model.pinnedTabs,
		...model.items.flatMap((item) => (item.kind === 'tab' ? [item.tab] : item.tabs)),
	];
}

function getDefaultActiveTabId(
	model: Omit<WorkbenchTabStripModel, 'selection'>,
): string | undefined {
	const firstPinnedTab = model.pinnedTabs[0];
	if (firstPinnedTab !== undefined) return firstPinnedTab.id;
	for (const item of model.items) {
		if (item.kind === 'tab') return item.tab.id;
		if (item.group.isCollapsed !== true && item.tabs[0] !== undefined) return item.tabs[0].id;
	}
	for (const item of model.items) {
		if (item.kind === 'group' && item.tabs[0] !== undefined) return item.tabs[0].id;
	}
	return undefined;
}

function createSelection(
	model: Omit<WorkbenchTabStripModel, 'selection'>,
	requestedActiveTabId: string | undefined,
): WorkbenchTabStripSelection {
	const tabs = getAllTabs({
		...model,
		selection: { activeTabId: undefined, anchorTabId: undefined, selectedTabIds: [] },
	});
	const activeTabId = tabs.some((tab) => tab.id === requestedActiveTabId)
		? requestedActiveTabId
		: getDefaultActiveTabId(model);
	return {
		activeTabId,
		anchorTabId: activeTabId,
		selectedTabIds: activeTabId === undefined ? [] : [activeTabId],
	};
}

export function createWorkbenchTabStripModel(
	input: WorkbenchTabsStateInput,
): WorkbenchTabStripModel {
	const suppliedGroups = input.tabGroups ?? [];
	assertUniqueIdentifiers(input.tabs, 'tab');
	assertUniqueIdentifiers(suppliedGroups, 'tab group');

	const groupsById = new Map(suppliedGroups.map((group) => [group.id, group]));
	const pinnedTabs = input.tabs.filter((tab) => tab.isPinned === true).map(asPinned);
	const unpinnedTabs = input.tabs.filter((tab) => tab.isPinned !== true).map(asUnpinned);
	const items: WorkbenchTabStripNode[] = [];
	const visitedGroupIds = new Set<string>();

	for (const tab of unpinnedTabs) {
		const group = tab.groupId === undefined ? undefined : groupsById.get(tab.groupId);
		if (group === undefined) {
			items.push({ kind: 'tab', tab: withoutGroup(tab) });
			continue;
		}
		if (visitedGroupIds.has(group.id)) continue;
		visitedGroupIds.add(group.id);
		const tabs = unpinnedTabs
			.filter((candidate) => candidate.groupId === group.id)
			.map((candidate) => withGroup(candidate, group.id));
		if (tabs.length > 0) items.push({ group, kind: 'group', tabs });
	}

	const groups = suppliedGroups.filter((group) => visitedGroupIds.has(group.id));
	const modelWithoutSelection = { groups, items, pinnedTabs };
	return {
		...modelWithoutSelection,
		selection: createSelection(modelWithoutSelection, input.activeTabId),
	};
}

export function projectWorkbenchTabsState(model: WorkbenchTabStripModel): WorkbenchTabsState {
	return {
		activeTabId: model.selection.activeTabId,
		tabGroups: model.groups,
		tabs: getAllTabs(model),
	};
}

export function readWorkbenchTabStripModel(state: WorkbenchTabsState): WorkbenchTabStripModel {
	const model = createWorkbenchTabStripModel(state);
	const projectedState = projectWorkbenchTabsState(model);
	const projectedTabSignature = projectedState.tabs.map(
		(tab) => `${tab.id}:${tab.isPinned === true ? 'pinned' : 'open'}:${tab.groupId ?? ''}`,
	);
	const suppliedTabSignature = state.tabs.map(
		(tab) => `${tab.id}:${tab.isPinned === true ? 'pinned' : 'open'}:${tab.groupId ?? ''}`,
	);
	const projectedGroupSignature = projectedState.tabGroups.map((group) => group.id);
	const suppliedGroupSignature = state.tabGroups.map((group) => group.id);
	if (
		projectedTabSignature.join('|') !== suppliedTabSignature.join('|') ||
		projectedGroupSignature.join('|') !== suppliedGroupSignature.join('|') ||
		projectedState.activeTabId !== state.activeTabId
	) {
		throw new Error(
			`WorkbenchTabsState violates the tab strip topology invariants: ${JSON.stringify({
				projectedActiveTabId: projectedState.activeTabId,
				projectedGroupSignature,
				projectedTabSignature,
				suppliedActiveTabId: state.activeTabId,
				suppliedGroupSignature,
				suppliedTabSignature,
			})}`,
		);
	}
	return model;
}
