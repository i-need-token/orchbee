import type { ActionInputValidation, ActionSignature } from '@orchbee/actions';

import type { TabDragRestoreState, TabGroupColor } from '../model/tab-model';
import type { TabCloseSource } from '../layout/tab-closing-mode';

export const TAB_STRIP_ACTION_IDS = {
	closeTabs: 'tabs.tab.close',
	createGroup: 'tabs.group.create',
	createGroupWithTab: 'tabs.group.create-with-tab',
	createTab: 'tabs.tab.create',
	duplicateTabs: 'tabs.tab.duplicate',
	moveGroup: 'tabs.group.move',
	moveGroupToWindow: 'tabs.group.move-to-window',
	moveTab: 'tabs.tab.move',
	moveTabsToGroup: 'tabs.tab.move-to-group',
	applyArrangement: 'tabs.layout.apply-arrangement',
	moveTabsToWindow: 'tabs.tab.move-to-window',
	openTabSearch: 'tabs.search.open',
	selectTab: 'tabs.tab.select',
	setGroupCollapsed: 'tabs.group.collapsed.set',
	setTabSearchPinned: 'tabs.search.pinned.set',
	setTabsPinned: 'tabs.tab.pinned.set',
	setVerticalCollapsed: 'tabs.strip.collapsed.set',
	setVerticalExpandOnHover: 'tabs.strip.expand-on-hover.set',
	ungroup: 'tabs.group.ungroup',
	updateGroup: 'tabs.group.update',
} as const;

export interface TabCreateInput {
	readonly groupId: string | null;
	readonly index?: number | undefined;
	readonly title: string;
}

export interface TabIdsInput {
	readonly tabIds: readonly string[];
}

interface TabIdInput {
	readonly tabId: string;
}

export interface TabSelectionInput extends TabIdInput {
	readonly mode: 'add-range' | 'extend' | 'replace' | 'toggle';
}

export interface TabMoveInput extends TabIdInput {
	readonly direction: 'first' | 'last' | 'next' | 'previous';
}

export interface TabCloseInput extends TabIdsInput {
	readonly closeSource: TabCloseSource;
}

export interface TabGroupIdInput {
	readonly groupId: string;
}

type TabGroupCreateInput = TabIdsInput;

export interface TabGroupCollapsedInput extends TabGroupIdInput {
	readonly collapsed: boolean;
}

export interface TabGroupShiftInput extends TabGroupIdInput {
	readonly direction: -1 | 1;
}

export interface TabGroupMoveInput extends TabIdsInput {
	readonly groupId: string | null;
}

export interface TabPinnedInput extends TabIdsInput {
	readonly pinned: boolean;
}

export interface TabGroupUpdateInput extends TabGroupIdInput {
	readonly color?: TabGroupColor | undefined;
	readonly title?: string | undefined;
}

export interface BooleanValueInput {
	readonly value: boolean;
}

export interface TabArrangementInput {
	readonly state: TabDragRestoreState;
}

export interface TabStripActionCatalog {
	readonly [TAB_STRIP_ACTION_IDS.applyArrangement]: ActionSignature<TabArrangementInput>;
	readonly [TAB_STRIP_ACTION_IDS.closeTabs]: ActionSignature<TabCloseInput>;
	readonly [TAB_STRIP_ACTION_IDS.createGroup]: ActionSignature<TabGroupCreateInput>;
	readonly [TAB_STRIP_ACTION_IDS.createGroupWithTab]: ActionSignature<void>;
	readonly [TAB_STRIP_ACTION_IDS.createTab]: ActionSignature<TabCreateInput>;
	readonly [TAB_STRIP_ACTION_IDS.duplicateTabs]: ActionSignature<TabIdsInput>;
	readonly [TAB_STRIP_ACTION_IDS.moveGroup]: ActionSignature<TabGroupShiftInput>;
	readonly [TAB_STRIP_ACTION_IDS.moveGroupToWindow]: ActionSignature<TabGroupIdInput>;
	readonly [TAB_STRIP_ACTION_IDS.moveTab]: ActionSignature<TabMoveInput>;
	readonly [TAB_STRIP_ACTION_IDS.moveTabsToGroup]: ActionSignature<TabGroupMoveInput>;
	readonly [TAB_STRIP_ACTION_IDS.moveTabsToWindow]: ActionSignature<TabIdsInput>;
	readonly [TAB_STRIP_ACTION_IDS.openTabSearch]: ActionSignature<void>;
	readonly [TAB_STRIP_ACTION_IDS.selectTab]: ActionSignature<TabSelectionInput>;
	readonly [TAB_STRIP_ACTION_IDS.setGroupCollapsed]: ActionSignature<TabGroupCollapsedInput>;
	readonly [TAB_STRIP_ACTION_IDS.setTabSearchPinned]: ActionSignature<BooleanValueInput>;
	readonly [TAB_STRIP_ACTION_IDS.setTabsPinned]: ActionSignature<TabPinnedInput>;
	readonly [TAB_STRIP_ACTION_IDS.setVerticalCollapsed]: ActionSignature<BooleanValueInput>;
	readonly [TAB_STRIP_ACTION_IDS.setVerticalExpandOnHover]: ActionSignature<BooleanValueInput>;
	readonly [TAB_STRIP_ACTION_IDS.ungroup]: ActionSignature<TabGroupIdInput>;
	readonly [TAB_STRIP_ACTION_IDS.updateGroup]: ActionSignature<TabGroupUpdateInput>;
}

const TAB_GROUP_COLORS = new Set<string>([
	'blue',
	'cyan',
	'green',
	'grey',
	'orange',
	'pink',
	'purple',
	'red',
	'yellow',
]);

function isTabGroupColor(input: unknown): input is TabGroupColor {
	return typeof input === 'string' && TAB_GROUP_COLORS.has(input);
}

function isRecord(input: unknown): input is Readonly<Record<string, unknown>> {
	return typeof input === 'object' && input !== null && !Array.isArray(input);
}

function isArrangementTab(input: unknown): input is Readonly<{ id: string; group: string | null }> {
	return (
		isRecord(input) &&
		typeof input['id'] === 'string' &&
		(input['group'] === null || typeof input['group'] === 'string')
	);
}

function isArrangementGroup(input: unknown): input is Readonly<{
	attention?: boolean | undefined;
	collapsed: boolean;
	color: TabGroupColor;
	id: string;
	title: string;
}> {
	return (
		isRecord(input) &&
		typeof input['id'] === 'string' &&
		typeof input['title'] === 'string' &&
		isTabGroupColor(input['color']) &&
		typeof input['collapsed'] === 'boolean' &&
		(input['attention'] === undefined || typeof input['attention'] === 'boolean')
	);
}

function parseTabIds(input: unknown): readonly string[] | null {
	if (
		!isRecord(input) ||
		!Array.isArray(input['tabIds']) ||
		!input['tabIds'].every((id) => typeof id === 'string')
	) {
		return null;
	}
	return Object.freeze([...input['tabIds']]);
}

export const tabCreateInput: ActionInputValidation<TabCreateInput> = {
	name: 'TabCreateInput',
	parse: (input) =>
		isRecord(input) &&
		(input['groupId'] === null || typeof input['groupId'] === 'string') &&
		(input['index'] === undefined ||
			(typeof input['index'] === 'number' &&
				Number.isInteger(input['index']) &&
				input['index'] >= 0)) &&
		typeof input['title'] === 'string'
			? {
					success: true,
					data: {
						groupId: input['groupId'],
						...(input['index'] === undefined ? {} : { index: input['index'] }),
						title: input['title'],
					},
				}
			: { success: false, code: 'expected-tab-create-input' },
};

export const tabIdsInput: ActionInputValidation<TabIdsInput> = {
	name: 'TabIdsInput',
	parse: (input) => {
		const tabIds = parseTabIds(input);
		return tabIds === null
			? { success: false, code: 'expected-tab-ids' }
			: { success: true, data: { tabIds } };
	},
};

export const tabSelectionInput: ActionInputValidation<TabSelectionInput> = {
	name: 'TabSelectionInput',
	parse: (input) =>
		isRecord(input) &&
		typeof input['tabId'] === 'string' &&
		(input['mode'] === 'add-range' ||
			input['mode'] === 'extend' ||
			input['mode'] === 'replace' ||
			input['mode'] === 'toggle')
			? {
					success: true,
					data: { mode: input['mode'], tabId: input['tabId'] },
				}
			: { success: false, code: 'expected-tab-selection-input' },
};

export const tabMoveInput: ActionInputValidation<TabMoveInput> = {
	name: 'TabMoveInput',
	parse: (input) =>
		isRecord(input) &&
		typeof input['tabId'] === 'string' &&
		(input['direction'] === 'first' ||
			input['direction'] === 'last' ||
			input['direction'] === 'next' ||
			input['direction'] === 'previous')
			? {
					success: true,
					data: { direction: input['direction'], tabId: input['tabId'] },
				}
			: { success: false, code: 'expected-tab-move-input' },
};

export const tabCloseInput: ActionInputValidation<TabCloseInput> = {
	name: 'TabCloseInput',
	parse: (input) => {
		const tabIds = parseTabIds(input);
		return tabIds !== null &&
			isRecord(input) &&
			(input['closeSource'] === 'mouse' ||
				input['closeSource'] === 'touch' ||
				input['closeSource'] === 'non-ui')
			? {
					success: true,
					data: { closeSource: input['closeSource'], tabIds },
				}
			: { success: false, code: 'expected-tab-close-input' };
	},
};

export const tabGroupIdInput: ActionInputValidation<TabGroupIdInput> = {
	name: 'TabGroupIdInput',
	parse: (input) =>
		isRecord(input) && typeof input['groupId'] === 'string'
			? { success: true, data: { groupId: input['groupId'] } }
			: { success: false, code: 'expected-tab-group-id' },
};

export const tabGroupCollapsedInput: ActionInputValidation<TabGroupCollapsedInput> = {
	name: 'TabGroupCollapsedInput',
	parse: (input) =>
		isRecord(input) &&
		typeof input['groupId'] === 'string' &&
		typeof input['collapsed'] === 'boolean'
			? {
					success: true,
					data: { collapsed: input['collapsed'], groupId: input['groupId'] },
				}
			: { success: false, code: 'expected-tab-group-collapsed-input' },
};

export const tabGroupShiftInput: ActionInputValidation<TabGroupShiftInput> = {
	name: 'TabGroupShiftInput',
	parse: (input) =>
		isRecord(input) &&
		typeof input['groupId'] === 'string' &&
		(input['direction'] === -1 || input['direction'] === 1)
			? {
					success: true,
					data: { direction: input['direction'], groupId: input['groupId'] },
				}
			: { success: false, code: 'expected-tab-group-shift-input' },
};

export const tabGroupMoveInput: ActionInputValidation<TabGroupMoveInput> = {
	name: 'TabGroupMoveInput',
	parse: (input) => {
		const tabIds = parseTabIds(input);
		return tabIds !== null &&
			isRecord(input) &&
			(input['groupId'] === null || typeof input['groupId'] === 'string')
			? { success: true, data: { groupId: input['groupId'], tabIds } }
			: { success: false, code: 'expected-tab-group-move' };
	},
};

export const tabPinnedInput: ActionInputValidation<TabPinnedInput> = {
	name: 'TabPinnedInput',
	parse: (input) => {
		const tabIds = parseTabIds(input);
		return tabIds !== null && isRecord(input) && typeof input['pinned'] === 'boolean'
			? { success: true, data: { pinned: input['pinned'], tabIds } }
			: { success: false, code: 'expected-tab-pinned-input' };
	},
};

export const tabGroupUpdateInput: ActionInputValidation<TabGroupUpdateInput> = {
	name: 'TabGroupUpdateInput',
	parse: (input) =>
		isRecord(input) &&
		typeof input['groupId'] === 'string' &&
		(input['title'] === undefined || typeof input['title'] === 'string') &&
		(input['color'] === undefined || isTabGroupColor(input['color'])) &&
		(input['title'] !== undefined || input['color'] !== undefined)
			? {
					success: true,
					data: {
						groupId: input['groupId'],
						...(isTabGroupColor(input['color']) ? { color: input['color'] } : {}),
						...(typeof input['title'] === 'string' ? { title: input['title'] } : {}),
					},
				}
			: { success: false, code: 'expected-tab-group-update' },
};

export const booleanValueInput: ActionInputValidation<BooleanValueInput> = {
	name: 'BooleanValueInput',
	parse: (input) =>
		isRecord(input) && typeof input['value'] === 'boolean'
			? { success: true, data: { value: input['value'] } }
			: { success: false, code: 'expected-boolean-value-input' },
};

export const tabArrangementInput: ActionInputValidation<TabArrangementInput> = {
	name: 'TabArrangementInput',
	parse: (input) => {
		if (!isRecord(input) || !isRecord(input['state'])) {
			return { success: false, code: 'expected-tab-arrangement-input' };
		}
		const state = input['state'];
		const tabs = state['tabs'];
		const groups = state['groups'];
		const selectedTabIds = state['selectedTabIds'];
		if (
			!Array.isArray(tabs) ||
			!tabs.every(isArrangementTab) ||
			new Set(tabs.map((tab) => tab.id)).size !== tabs.length ||
			!Array.isArray(groups) ||
			!groups.every(isArrangementGroup) ||
			new Set(groups.map((group) => group.id)).size !== groups.length ||
			!Array.isArray(selectedTabIds) ||
			!selectedTabIds.every((id) => typeof id === 'string') ||
			!(state['activeTabId'] === null || typeof state['activeTabId'] === 'string') ||
			!(state['selectionAnchorId'] === null || typeof state['selectionAnchorId'] === 'string') ||
			!(state['mostRecentGroupId'] === null || typeof state['mostRecentGroupId'] === 'string')
		) {
			return { success: false, code: 'expected-tab-arrangement-input' };
		}
		return {
			success: true,
			data: {
				state: {
					activeTabId: state['activeTabId'],
					groups: groups.map((group) => ({
						collapsed: group.collapsed,
						color: group.color,
						id: group.id,
						title: group.title,
						...(group.attention === undefined ? {} : { attention: group.attention }),
					})),
					mostRecentGroupId: state['mostRecentGroupId'],
					selectedTabIds: Object.freeze([...selectedTabIds]),
					selectionAnchorId: state['selectionAnchorId'],
					tabs: tabs.map((tab) => ({
						group: tab.group,
						id: tab.id,
					})),
				},
			},
		};
	},
};
