import { projectWorkbenchTabsState, readWorkbenchTabStripModel } from './workbench-tab-strip-model';
import type {
	WorkbenchTab,
	WorkbenchTabGroup,
	WorkbenchTabsAction,
	WorkbenchTabsState,
} from './workbench-tabs-state';

function findLastGroupIndex(tabs: readonly WorkbenchTab[], groupId: string): number {
	for (let index = tabs.length - 1; index >= 0; index -= 1) {
		if (tabs[index]?.groupId === groupId) return index;
	}
	return -1;
}

function pruneTabGroups(
	tabs: readonly WorkbenchTab[],
	tabGroups: readonly WorkbenchTabGroup[],
): readonly WorkbenchTabGroup[] {
	const populatedGroupIds = new Set(
		tabs.flatMap((tab) => (tab.groupId === undefined ? [] : [tab.groupId])),
	);
	return tabGroups.filter((group) => populatedGroupIds.has(group.id));
}

function withoutGroup(tab: WorkbenchTab): WorkbenchTab {
	if (tab.groupId === undefined) return tab;
	const { groupId: _groupId, ...ungroupedTab } = tab;
	return ungroupedTab;
}

function withoutPinning(tab: WorkbenchTab): WorkbenchTab {
	if (tab.isPinned !== true) return tab;
	const { isPinned: _isPinned, ...unpinnedTab } = tab;
	return unpinnedTab;
}

function assignGroup(tab: WorkbenchTab, groupId: string): WorkbenchTab {
	return { ...withoutGroup(withoutPinning(tab)), groupId };
}

function expandTabGroup(
	tabGroups: readonly WorkbenchTabGroup[],
	groupId: string | undefined,
): readonly WorkbenchTabGroup[] {
	if (groupId === undefined) return tabGroups;
	const groupIndex = tabGroups.findIndex((group) => group.id === groupId);
	const group = tabGroups[groupIndex];
	if (group === undefined || group.isCollapsed !== true) return tabGroups;
	const nextTabGroups = [...tabGroups];
	nextTabGroups[groupIndex] = { ...group, isCollapsed: false };
	return nextTabGroups;
}

function isInsertionInsideGroup(tabs: readonly WorkbenchTab[], targetIndex: number): boolean {
	const leftGroupId = tabs[targetIndex - 1]?.groupId;
	const rightGroupId = tabs[targetIndex]?.groupId;
	return leftGroupId !== undefined && leftGroupId === rightGroupId;
}

function findNearestVisibleTabId(
	referenceTabs: readonly WorkbenchTab[],
	nextState: WorkbenchTabsState,
	referenceIndex: number,
): string | undefined {
	const survivingTabsById = new Map(nextState.tabs.map((tab) => [tab.id, tab]));
	const groupsById = new Map(nextState.tabGroups.map((group) => [group.id, group]));
	const isVisibleSurvivor = (tab: WorkbenchTab | undefined): tab is WorkbenchTab => {
		if (tab === undefined) return false;
		const survivingTab = survivingTabsById.get(tab.id);
		if (survivingTab === undefined) return false;
		return (
			survivingTab.groupId === undefined ||
			groupsById.get(survivingTab.groupId)?.isCollapsed !== true
		);
	};

	for (let index = referenceIndex; index < referenceTabs.length; index += 1) {
		const tab = referenceTabs[index];
		if (isVisibleSurvivor(tab)) return tab.id;
	}
	for (let index = referenceIndex - 1; index >= 0; index -= 1) {
		const tab = referenceTabs[index];
		if (isVisibleSurvivor(tab)) return tab.id;
	}
	return undefined;
}

class WorkbenchTabsReducer {
	readonly #state: WorkbenchTabsState;

	public constructor(state: WorkbenchTabsState) {
		readWorkbenchTabStripModel(state);
		this.#state = state;
	}

	#build(
		tabs: readonly WorkbenchTab[],
		tabGroups: readonly WorkbenchTabGroup[] = this.#state.tabGroups,
		activeTab: { readonly value: string | undefined } | undefined = undefined,
	): WorkbenchTabsState {
		const candidate = {
			activeTabId: activeTab === undefined ? this.#state.activeTabId : activeTab.value,
			tabGroups,
			tabs,
		};
		return projectWorkbenchTabsState(readWorkbenchTabStripModel(candidate));
	}

	#closeTabs(tabIds: readonly string[]): WorkbenchTabsState {
		const closingIds = new Set(tabIds);
		if (!this.#state.tabs.some((tab) => closingIds.has(tab.id))) return this.#state;

		const tabs = this.#state.tabs.filter((tab) => !closingIds.has(tab.id));
		const tabGroups = pruneTabGroups(tabs, this.#state.tabGroups);
		let activeTabId = this.#state.activeTabId;
		if (activeTabId !== undefined && closingIds.has(activeTabId)) {
			const activeTabIndex = this.#state.tabs.findIndex((tab) => tab.id === activeTabId);
			const nextState = this.#build(tabs, tabGroups, { value: tabs[0]?.id });
			activeTabId = findNearestVisibleTabId(this.#state.tabs, nextState, activeTabIndex);
		}
		return this.#build(tabs, tabGroups, { value: activeTabId });
	}

	#moveTab(tabId: string, targetIndex: number, groupId: string | undefined): WorkbenchTabsState {
		const sourceIndex = this.#state.tabs.findIndex((tab) => tab.id === tabId);
		const sourceTab = this.#state.tabs[sourceIndex];
		if (
			sourceTab === undefined ||
			!Number.isInteger(targetIndex) ||
			targetIndex < 0 ||
			targetIndex >= this.#state.tabs.length
		) {
			return this.#state;
		}
		if (groupId !== undefined && !this.#state.tabGroups.some((group) => group.id === groupId)) {
			return this.#state;
		}
		const remainingTabs = this.#state.tabs.filter((tab) => tab.id !== tabId);
		const remainingPinnedCount = remainingTabs.filter((tab) => tab.isPinned === true).length;
		if (
			sourceTab.isPinned === true
				? targetIndex > remainingPinnedCount
				: targetIndex < remainingPinnedCount
		)
			return this.#state;
		const nextGroupId = sourceTab.isPinned === true ? undefined : groupId;
		if (sourceIndex === targetIndex && sourceTab.groupId === nextGroupId) return this.#state;
		if (nextGroupId === undefined && isInsertionInsideGroup(remainingTabs, targetIndex)) {
			return this.#state;
		}
		if (nextGroupId !== undefined) {
			const firstGroupIndex = remainingTabs.findIndex((tab) => tab.groupId === nextGroupId);
			const lastGroupIndex = findLastGroupIndex(remainingTabs, nextGroupId);
			if (
				firstGroupIndex === -1 ||
				targetIndex < firstGroupIndex ||
				targetIndex > lastGroupIndex + 1
			)
				return this.#state;
		}

		const tabs = [...remainingTabs];
		const removedTab = sourceTab;
		const movedTab =
			removedTab.groupId === nextGroupId ? removedTab : { ...removedTab, groupId: nextGroupId };
		tabs.splice(targetIndex, 0, movedTab);
		const tabGroups = expandTabGroup(pruneTabGroups(tabs, this.#state.tabGroups), nextGroupId);
		return this.#build(tabs, tabGroups);
	}

	public reduce(action: WorkbenchTabsAction): WorkbenchTabsState {
		switch (action.kind) {
			case 'activate-tab': {
				const tab = this.#state.tabs.find((candidate) => candidate.id === action.tabId);
				if (tab === undefined || tab.id === this.#state.activeTabId) return this.#state;
				return { ...this.#state, activeTabId: action.tabId };
			}
			case 'assign-tab-group': {
				const sourceIndex = this.#state.tabs.findIndex((tab) => tab.id === action.tabId);
				const sourceTab = this.#state.tabs[sourceIndex];
				if (sourceTab === undefined || sourceTab.groupId === action.groupId) {
					return this.#state;
				}
				if (
					action.groupId !== undefined &&
					!this.#state.tabGroups.some((group) => group.id === action.groupId)
				) {
					return this.#state;
				}
				const approachesTargetGroupFromStart =
					action.groupId !== undefined &&
					sourceIndex < this.#state.tabs.findIndex((tab) => tab.groupId === action.groupId);
				const tabs = this.#state.tabs.filter((tab) => tab.id !== sourceTab.id);
				const unpinnedTab = withoutPinning(sourceTab);
				const updatedTab =
					action.groupId === undefined
						? withoutGroup(unpinnedTab)
						: { ...withoutGroup(unpinnedTab), groupId: action.groupId };
				let targetIndex = tabs.length;
				if (action.groupId !== undefined) {
					const firstGroupIndex = tabs.findIndex((tab) => tab.groupId === action.groupId);
					const lastGroupIndex = findLastGroupIndex(tabs, action.groupId);
					targetIndex = approachesTargetGroupFromStart ? firstGroupIndex : lastGroupIndex + 1;
				} else if (sourceTab.groupId !== undefined) {
					const remainingGroupTabs = tabs.filter((tab) => tab.groupId === sourceTab.groupId);
					const firstGroupIndex = tabs.findIndex((tab) => tab.groupId === sourceTab.groupId);
					const memberIndex = this.#state.tabs
						.filter((tab) => tab.groupId === sourceTab.groupId)
						.findIndex((tab) => tab.id === sourceTab.id);
					if (remainingGroupTabs.length === 0) targetIndex = sourceIndex;
					else if (memberIndex * 2 < remainingGroupTabs.length + 1) targetIndex = firstGroupIndex;
					else targetIndex = findLastGroupIndex(tabs, sourceTab.groupId) + 1;
				}
				tabs.splice(targetIndex, 0, updatedTab);
				const tabGroups = expandTabGroup(
					pruneTabGroups(tabs, this.#state.tabGroups),
					action.groupId,
				);
				return this.#build(tabs, tabGroups);
			}
			case 'close-tab-group': {
				const tabIds = this.#state.tabs
					.filter((tab) => tab.groupId === action.groupId)
					.map((tab) => tab.id);
				return this.#closeTabs(tabIds);
			}
			case 'close-tabs':
				return this.#closeTabs(action.tabIds);
			case 'create-tab-group': {
				if (
					action.tabIds.length === 0 ||
					this.#state.tabGroups.some((group) => group.id === action.group.id)
				) {
					return this.#state;
				}
				const memberIds = new Set(action.tabIds);
				const memberTabs = this.#state.tabs.filter((tab) => memberIds.has(tab.id));
				if (memberTabs.length !== memberIds.size) return this.#state;
				const firstMemberIndex = this.#state.tabs.findIndex((tab) => memberIds.has(tab.id));
				const groupedMembers = memberTabs.map((tab) => assignGroup(tab, action.group.id));
				const tabs = this.#state.tabs.filter((tab) => !memberIds.has(tab.id));
				const pinnedCount = tabs.filter((tab) => tab.isPinned === true).length;
				const targetIndex = Math.max(firstMemberIndex, pinnedCount);
				tabs.splice(targetIndex, 0, ...groupedMembers);
				const tabGroups = [...pruneTabGroups(tabs, this.#state.tabGroups), action.group];
				return this.#build(tabs, tabGroups);
			}
			case 'insert-tab': {
				if (this.#state.tabs.some((tab) => tab.id === action.tab.id)) return this.#state;
				if (
					action.groupId !== undefined &&
					!this.#state.tabGroups.some((group) => group.id === action.groupId)
				) {
					return this.#state;
				}
				let insertedTab = withoutGroup(action.tab);
				let targetIndex =
					action.tab.isPinned === true
						? this.#state.tabs.filter((tab) => tab.isPinned === true).length
						: this.#state.tabs.length;
				if (action.groupId !== undefined) {
					insertedTab = assignGroup(action.tab, action.groupId);
					targetIndex = findLastGroupIndex(this.#state.tabs, action.groupId) + 1;
				}
				const tabs = [...this.#state.tabs];
				tabs.splice(targetIndex, 0, insertedTab);
				const tabGroups = this.#state.tabGroups.map((group) =>
					group.id === action.groupId && group.isCollapsed === true
						? { ...group, isCollapsed: false }
						: group,
				);
				return this.#build(tabs, tabGroups, { value: insertedTab.id });
			}
			case 'move-tab':
				return this.#moveTab(action.tabId, action.targetIndex, action.groupId);
			case 'move-tab-group': {
				const groupTabs = this.#state.tabs.filter((tab) => tab.groupId === action.groupId);
				const pinnedCount = this.#state.tabs.filter((tab) => tab.isPinned === true).length;
				const maximumTargetIndex = this.#state.tabs.length - groupTabs.length;
				const sourceIndex = this.#state.tabs.findIndex((tab) => tab.groupId === action.groupId);
				if (
					groupTabs.length === 0 ||
					!Number.isInteger(action.targetIndex) ||
					action.targetIndex < pinnedCount ||
					action.targetIndex > maximumTargetIndex ||
					action.targetIndex === sourceIndex
				) {
					return this.#state;
				}
				const tabs = this.#state.tabs.filter((tab) => tab.groupId !== action.groupId);
				if (isInsertionInsideGroup(tabs, action.targetIndex)) return this.#state;
				tabs.splice(action.targetIndex, 0, ...groupTabs);
				return this.#build(tabs);
			}
			case 'rename-tab': {
				const label = action.label.trim();
				const tabIndex = this.#state.tabs.findIndex((tab) => tab.id === action.tabId);
				const tab = this.#state.tabs[tabIndex];
				if (tab === undefined || label.length === 0 || tab.label === label) return this.#state;
				const tabs = [...this.#state.tabs];
				tabs[tabIndex] = { ...tab, label };
				return this.#build(tabs);
			}
			case 'set-tab-color': {
				const tabIndex = this.#state.tabs.findIndex((tab) => tab.id === action.tabId);
				const tab = this.#state.tabs[tabIndex];
				if (tab === undefined || (tab.color ?? 'default') === action.color) return this.#state;
				const tabs = [...this.#state.tabs];
				tabs[tabIndex] = { ...tab, color: action.color };
				return this.#build(tabs);
			}
			case 'set-tab-group-collapsed': {
				const groupIndex = this.#state.tabGroups.findIndex((group) => group.id === action.groupId);
				const group = this.#state.tabGroups[groupIndex];
				if (group === undefined || group.isCollapsed === action.isCollapsed) return this.#state;
				const tabGroups = [...this.#state.tabGroups];
				tabGroups[groupIndex] = { ...group, isCollapsed: action.isCollapsed };
				return this.#build(this.#state.tabs, tabGroups);
			}
			case 'set-tab-pinned': {
				const tabIndex = this.#state.tabs.findIndex((tab) => tab.id === action.tabId);
				const tab = this.#state.tabs[tabIndex];
				if (tab === undefined || (tab.isPinned === true) === action.isPinned) return this.#state;
				const tabs = this.#state.tabs.filter((candidate) => candidate.id !== action.tabId);
				const pinnedCount = tabs.filter((candidate) => candidate.isPinned === true).length;
				const updatedTab = action.isPinned
					? { ...withoutGroup(tab), isPinned: true }
					: withoutPinning(withoutGroup(tab));
				tabs.splice(pinnedCount, 0, updatedTab);
				return this.#build(tabs, pruneTabGroups(tabs, this.#state.tabGroups));
			}
			case 'ungroup-tab-group': {
				if (!this.#state.tabGroups.some((group) => group.id === action.groupId)) return this.#state;
				const tabs = this.#state.tabs.map((tab) =>
					tab.groupId === action.groupId ? { ...tab, groupId: undefined } : tab,
				);
				const tabGroups = this.#state.tabGroups.filter((group) => group.id !== action.groupId);
				return this.#build(tabs, tabGroups);
			}
			case 'update-tab-group': {
				const groupIndex = this.#state.tabGroups.findIndex((group) => group.id === action.groupId);
				const group = this.#state.tabGroups[groupIndex];
				const label = action.update.label?.trim();
				if (group === undefined || label === '') return this.#state;
				const updatedGroup = {
					...group,
					color: action.update.color ?? group.color,
					label: label ?? group.label,
				};
				if (updatedGroup.color === group.color && updatedGroup.label === group.label)
					return this.#state;
				const tabGroups = [...this.#state.tabGroups];
				tabGroups[groupIndex] = updatedGroup;
				return this.#build(this.#state.tabs, tabGroups);
			}
		}
	}
}

export function reduceWorkbenchTabs(
	state: WorkbenchTabsState,
	action: WorkbenchTabsAction,
): WorkbenchTabsState {
	return new WorkbenchTabsReducer(state).reduce(action);
}
