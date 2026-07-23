import {
	getAccessibleTabLabel,
	getGroupAccessibleName,
	getGroupTooltipText,
	getTabGroupMembershipAnnouncement,
	getTabMoveAnnouncement,
	type TabMoveDirection,
} from '../../accessibility';
import type { TabData, TabGroupColor, TabGroupData } from '../../model';

export type TabStripMoveDirection = 'down' | 'left' | 'right' | 'up';

export interface TabStripMessages {
	readonly addTabToGroup: string;
	readonly addTabsToGroup: string;
	readonly close: string;
	readonly closeGroup: string;
	readonly closeOtherTabs: string;
	readonly closeTab: string;
	readonly closeTabsBelow: string;
	readonly closeTabsToRight: string;
	readonly collapseTabStrip: string;
	readonly duplicate: string;
	readonly expandTabStrip: string;
	readonly expandTabStripOnHover: string;
	readonly focusGroup: string;
	readonly formatClosedTabAnnouncement: (title: string) => string;
	readonly formatClosedTabsAnnouncement: (count: number) => string;
	readonly formatDragCompleteAnnouncement: (
		label: string,
		position: number,
		count: number,
	) => string;
	readonly formatGroupAccessibleName: (
		group: Readonly<TabGroupData>,
		tabs: readonly Pick<TabData, 'title'>[],
	) => string;
	readonly formatGroupMovedAnnouncement: (direction: TabStripMoveDirection) => string;
	readonly formatGroupTooltip: (
		group: Pick<TabGroupData, 'title'>,
		tabs: readonly Pick<TabData, 'title'>[],
	) => string;
	readonly formatNewTabInGroup: (groupTitle: string, tabCount: number) => string;
	readonly formatTabAccessibleLabel: (
		tab: Readonly<TabData>,
		group: Readonly<TabGroupData> | null,
	) => string;
	readonly formatTabGroupMembershipAnnouncement: (
		action: 'added' | 'removed',
		group: Readonly<TabGroupData>,
		tabsBeforeChange: readonly Pick<TabData, 'title'>[],
	) => string;
	readonly formatTabMoveAnnouncement: (
		direction: TabMoveDirection,
		orientation: 'horizontal' | 'vertical',
	) => string;
	readonly groupColor: string;
	readonly groupColorNames: Readonly<Record<TabGroupColor, string>>;
	readonly groupMoveCanceled: string;
	readonly groupName: string;
	readonly groupNamePlaceholder: string;
	readonly groupOptions: string;
	readonly moveGroupToNewWindow: string;
	readonly moveTabToNewWindow: string;
	readonly moveTabsToNewWindow: string;
	readonly newGroup: string;
	readonly moreOptions: string;
	readonly newTab: string;
	readonly newTabBelow: string;
	readonly newTabInGroup: string;
	readonly newTabInNoGroups: string;
	readonly newTabMenu: string;
	readonly newTabTitle: string;
	readonly newTabToRight: string;
	readonly pin: string;
	readonly pinSearchTabs: string;
	readonly previewUnavailable: string;
	readonly removeFromGroup: string;
	readonly searchTabs: string;
	readonly searchTabsMenu: string;
	readonly showAllTabs: string;
	readonly tabCrashed: string;
	readonly tabGroups: string;
	readonly tabMoveCanceled: string;
	readonly tabOptions: string;
	readonly tabStripLabel: string;
	readonly ungroup: string;
	readonly unfocusGroup: string;
	readonly unnamedGroup: string;
	readonly unpin: string;
	readonly unpinSearchTabs: string;
	readonly verticalTabStripMenu: string;
}

export const DEFAULT_TAB_STRIP_MESSAGES: TabStripMessages = {
	addTabToGroup: 'Add tab to group',
	addTabsToGroup: 'Add tabs to group',
	close: 'Close',
	closeGroup: 'Close group',
	closeOtherTabs: 'Close other tabs',
	closeTab: 'Close',
	closeTabsBelow: 'Close tabs below',
	closeTabsToRight: 'Close tabs to the right',
	collapseTabStrip: 'Collapse tab strip',
	duplicate: 'Duplicate',
	expandTabStrip: 'Expand tab strip',
	expandTabStripOnHover: 'Expand tab strip on hover',
	focusGroup: 'Focus group',
	formatClosedTabAnnouncement: (title) => `Closed ${title}`,
	formatClosedTabsAnnouncement: (count) => `Closed ${count} tabs`,
	formatDragCompleteAnnouncement: (label, position, count) =>
		`${label} moved to position ${position} of ${count}`,
	formatGroupAccessibleName: getGroupAccessibleName,
	formatGroupMovedAnnouncement: (direction) => `Group moved ${direction}`,
	formatGroupTooltip: getGroupTooltipText,
	formatNewTabInGroup: (groupTitle, tabCount) =>
		groupTitle
			? `New tab in ${groupTitle}`
			: `New tab in ${tabCount} ${tabCount === 1 ? 'tab' : 'tabs'}`,
	formatTabAccessibleLabel: (tab, group) => getAccessibleTabLabel(tab, { group }),
	formatTabGroupMembershipAnnouncement: getTabGroupMembershipAnnouncement,
	formatTabMoveAnnouncement: getTabMoveAnnouncement,
	groupColor: 'Group color',
	groupColorNames: {
		blue: 'blue',
		cyan: 'cyan',
		green: 'green',
		grey: 'grey',
		orange: 'orange',
		pink: 'pink',
		purple: 'purple',
		red: 'red',
		yellow: 'yellow',
	},
	groupMoveCanceled: 'Group move canceled',
	groupName: 'Group name',
	groupNamePlaceholder: 'Name this group',
	groupOptions: 'Group options',
	moveGroupToNewWindow: 'Move group to new window',
	moreOptions: 'More options',
	moveTabToNewWindow: 'Move tab to new window',
	moveTabsToNewWindow: 'Move tabs to new window',
	newGroup: 'New group',
	newTab: 'New tab',
	newTabBelow: 'New tab below',
	newTabInGroup: 'New tab in group',
	newTabInNoGroups: 'New tab in group (no groups)',
	newTabMenu: 'New tab menu',
	newTabTitle: 'New Tab',
	newTabToRight: 'New tab to the right',
	pin: 'Pin',
	pinSearchTabs: 'Pin Search tabs',
	previewUnavailable: 'Preview unavailable',
	removeFromGroup: 'Remove from group',
	searchTabs: 'Search tabs',
	searchTabsMenu: 'Search tabs menu',
	showAllTabs: 'Show all tabs',
	tabCrashed: 'This tab has crashed',
	tabGroups: 'Tab groups',
	tabMoveCanceled: 'Tab move canceled',
	tabOptions: 'Tab options',
	tabStripLabel: 'Tabs',
	ungroup: 'Ungroup',
	unfocusGroup: 'Unfocus group',
	unnamedGroup: 'Unnamed group',
	unpin: 'Unpin',
	unpinSearchTabs: 'Unpin Search tabs',
	verticalTabStripMenu: 'Vertical tab strip menu',
};
