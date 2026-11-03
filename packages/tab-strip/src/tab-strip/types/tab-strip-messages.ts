import type { TabMoveDirection } from '../accessibility/tab-strip-accessibility';
import type { TabData, TabGroupColor, TabGroupData } from '../model/tab-model';

export type TabStripMoveDirection = 'down' | 'left' | 'right' | 'up';

/**
 * Object-menu rows that take a count name the selected set when it is larger than one, and otherwise
 * act on the Tab the menu opened on, which they do not repeat.
 */
export interface TabStripMessages {
	readonly close: string;
	readonly closeTab: string;
	readonly closeVerticalTabs: string;
	readonly collapseTabStrip: string;
	readonly expandTabStrip: string;
	readonly expandTabStripOnHover: string;
	readonly formatAddTabsToGroup: (count: number) => string;
	readonly formatClosedTabAnnouncement: (title: string) => string;
	readonly formatClosedTabsAnnouncement: (count: number) => string;
	readonly formatCloseOtherTabs: (count: number) => string;
	readonly formatCloseTabs: (count: number) => string;
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
	readonly formatPinTabs: (count: number) => string;
	readonly formatRemoveTabsFromGroup: (count: number) => string;
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
	/** Ungrouping keeps every Tab the group holds. */
	readonly formatUngroup: (tabCount: number) => string;
	readonly formatUnpinTabs: (count: number) => string;
	readonly groupColor: string;
	readonly groupColorNames: Readonly<Record<TabGroupColor, string>>;
	readonly groupMoveCanceled: string;
	readonly groupOptions: string;
	readonly moveGroupToNewWindow: string;
	readonly moveTabToNewWindow: string;
	readonly moveTabsToNewWindow: string;
	/** Asks for a name before creating the group. */
	readonly newGroup: string;
	readonly moreOptions: string;
	readonly newTab: string;
	readonly newTabInGroup: string;
	readonly newTabMenu: string;
	/** Optional for older message providers; product hosts supply a localized loading title. */
	readonly loadingTabTitle?: string;
	readonly newTabTitle: string;
	readonly openVerticalTabs: string;
	readonly pinSearchTabs: string;
	readonly previewUnavailable: string;
	readonly rename: string;
	/** Continues the Tab's close that stopped; it acts on the Tab the menu opened on alone. */
	readonly resumeTabClose: string;
	readonly searchTabs: string;
	readonly searchTabsMenu: string;
	readonly tabCrashed: string;
	readonly tabGroups: string;
	readonly tabMoveCanceled: string;
	readonly tabOptions: string;
	readonly tabStripLabel: string;
	readonly tabStripMenu: string;
	readonly unnamedGroup: string;
	readonly unpinSearchTabs: string;
	readonly verticalTabStripMenu: string;
}
