import type { TabMoveDirection } from '../accessibility/tab-strip-accessibility';
import type { TabData, TabGroupColor, TabGroupData } from '../model/tab-model';

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
	readonly closeVerticalTabs: string;
	readonly collapseTabStrip: string;
	readonly duplicate: string;
	readonly expandTabStrip: string;
	readonly expandTabStripOnHover: string;
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
	readonly openVerticalTabs: string;
	readonly pin: string;
	readonly pinSearchTabs: string;
	readonly previewUnavailable: string;
	readonly removeFromGroup: string;
	readonly searchTabs: string;
	readonly searchTabsMenu: string;
	readonly setTabTitle: string;
	readonly tabCrashed: string;
	readonly tabGroups: string;
	readonly tabMoveCanceled: string;
	readonly tabOptions: string;
	readonly tabStripLabel: string;
	readonly tabStripMenu: string;
	readonly ungroup: string;
	readonly unnamedGroup: string;
	readonly unpin: string;
	readonly unpinSearchTabs: string;
	readonly verticalTabStripMenu: string;
}
