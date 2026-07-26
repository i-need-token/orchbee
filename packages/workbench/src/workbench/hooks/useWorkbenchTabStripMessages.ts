import type { TabData, TabStripMessages } from '@orchbee/tab-strip';
import { useOrchbeeIntl, workbenchTabMessages } from '@orchbee/i18n';
import * as React from 'react';

const GROUP_ACCESSIBLE_TAB_LIMIT = 5;
const GROUP_CONTENT_TITLE_MAX_LENGTH = 30;
const GROUP_ACCESSIBLE_TITLE_MAX_LENGTH = 47;

export function useWorkbenchTabStripMessages(): TabStripMessages {
	const intl = useOrchbeeIntl();

	return React.useMemo(() => {
		const truncate = (value: string, maximumLength: number) => {
			const segments = Array.from(value);
			if (segments.length <= maximumLength) return value;
			return `${segments
				.slice(0, Math.max(0, maximumLength - 1))
				.join('')
				.trimEnd()}…`;
		};
		const getTabState = (tab: Readonly<TabData>) => {
			if (tab.crashed) return 'crashed';
			if (tab.networkState === 'error') return 'networkError';
			if (tab.permissionRequested) return 'permission';
			if (tab.discarded) return 'discarded';
			return 'normal';
		};
		const format = intl.formatMessage;
		const newTabTitle = format(workbenchTabMessages.newTabTitle);
		const unnamedGroup = format(workbenchTabMessages.unnamedGroup);
		const formatGroupContent = (tabs: readonly Pick<TabData, 'title'>[]) =>
			format(workbenchTabMessages.groupContent, {
				firstTitle: truncate(tabs[0]?.title || newTabTitle, GROUP_CONTENT_TITLE_MAX_LENGTH),
				remaining: Math.max(0, tabs.length - 1),
			});
		const formatMove = (
			direction: Parameters<TabStripMessages['formatTabMoveAnnouncement']>[0],
			orientation: Parameters<TabStripMessages['formatTabMoveAnnouncement']>[1],
		) => {
			if (direction === 'first') return format(workbenchTabMessages.movedFirstAnnouncement);
			if (direction === 'last') return format(workbenchTabMessages.movedLastAnnouncement);
			if (orientation === 'vertical') {
				return format(
					direction === 'previous'
						? workbenchTabMessages.movedUpAnnouncement
						: workbenchTabMessages.movedDownAnnouncement,
				);
			}
			return format(
				direction === 'previous'
					? workbenchTabMessages.movedLeftAnnouncement
					: workbenchTabMessages.movedRightAnnouncement,
			);
		};

		return {
			addTabToGroup: format(workbenchTabMessages.addTabToGroup),
			addTabsToGroup: format(workbenchTabMessages.addTabsToGroup),
			close: format(workbenchTabMessages.close),
			closeGroup: format(workbenchTabMessages.closeGroup),
			closeOtherTabs: format(workbenchTabMessages.closeOtherTabs),
			closeTab: format(workbenchTabMessages.close),
			closeTabsBelow: format(workbenchTabMessages.closeTabsBelow),
			closeTabsToRight: format(workbenchTabMessages.closeTabsToRight),
			closeVerticalTabs: format(workbenchTabMessages.closeVerticalTabs),
			collapseTabStrip: format(workbenchTabMessages.collapseTabStrip),
			duplicate: format(workbenchTabMessages.duplicate),
			expandTabStrip: format(workbenchTabMessages.expandTabStrip),
			expandTabStripOnHover: format(workbenchTabMessages.expandTabStripOnHover),
			formatClosedTabAnnouncement: (title) =>
				format(workbenchTabMessages.closedAnnouncement, { title }),
			formatClosedTabsAnnouncement: (count) =>
				format(workbenchTabMessages.closedTabsAnnouncement, { count }),
			formatDragCompleteAnnouncement: (label, position, count) =>
				format(workbenchTabMessages.dragCompleteAnnouncement, { label, position, count }),
			formatGroupAccessibleName: (group, tabs) => {
				const tabTitles = tabs
					.slice(0, GROUP_ACCESSIBLE_TAB_LIMIT)
					.map((tab) =>
						format(workbenchTabMessages.groupTabTitleAccessibility, {
							title: truncate(tab.title || newTabTitle, GROUP_ACCESSIBLE_TITLE_MAX_LENGTH),
						}),
					)
					.join('');
				const overflowCount = Math.max(0, tabs.length - GROUP_ACCESSIBLE_TAB_LIMIT);
				return format(workbenchTabMessages.groupNameAccessibility, {
					activity:
						group.collapsed && Boolean(group.attention) && tabs.length > 0 ? 'true' : 'false',
					groupTitle: group.title || unnamedGroup,
					overflow:
						overflowCount === 0
							? ''
							: format(workbenchTabMessages.groupOverflowAccessibility, {
									count: overflowCount,
								}),
					state: group.collapsed ? 'collapsed' : 'expanded',
					tabCount: tabs.length,
					tabTitles,
				});
			},
			formatGroupMovedAnnouncement: (direction) =>
				format(workbenchTabMessages.groupMoved, { direction }),
			formatGroupTooltip: (group, tabs) =>
				format(workbenchTabMessages.groupTooltip, {
					contents: formatGroupContent(tabs),
					groupTitle: group.title,
					hasTitle: group.title ? 'true' : 'false',
				}),
			formatNewTabInGroup: (groupTitle, tabCount) =>
				groupTitle
					? format(workbenchTabMessages.newTabInNamedGroup, { groupTitle })
					: format(workbenchTabMessages.newTabInUnnamedGroup, { tabCount }),
			formatTabAccessibleLabel: (tab, group) => {
				let groupKind = 'none';
				if (tab.group !== null) groupKind = group?.title ? 'named' : 'unnamed';
				return format(workbenchTabMessages.tabNameAccessibility, {
					groupKind,
					groupTitle: group?.title ?? '',
					pinned: tab.pinned ? 'true' : 'false',
					state: getTabState(tab),
					title: tab.title || newTabTitle,
				});
			},
			formatTabGroupMembershipAnnouncement: (action, group, tabsBeforeChange) =>
				format(workbenchTabMessages.groupMembershipAnnouncement, {
					action,
					contents: formatGroupContent(tabsBeforeChange),
					groupKind: group.title ? 'named' : 'unnamed',
					groupTitle: group.title,
				}),
			formatTabMoveAnnouncement: formatMove,
			groupColor: format(workbenchTabMessages.groupColor),
			groupColorNames: {
				blue: format(workbenchTabMessages.groupColorBlue),
				cyan: format(workbenchTabMessages.groupColorCyan),
				green: format(workbenchTabMessages.groupColorGreen),
				grey: format(workbenchTabMessages.groupColorGrey),
				orange: format(workbenchTabMessages.groupColorOrange),
				pink: format(workbenchTabMessages.groupColorPink),
				purple: format(workbenchTabMessages.groupColorPurple),
				red: format(workbenchTabMessages.groupColorRed),
				yellow: format(workbenchTabMessages.groupColorYellow),
			},
			groupMoveCanceled: format(workbenchTabMessages.groupMoveCanceled),
			groupName: format(workbenchTabMessages.groupName),
			groupNamePlaceholder: format(workbenchTabMessages.groupNamePlaceholder),
			groupOptions: format(workbenchTabMessages.groupOptions),
			moveGroupToNewWindow: format(workbenchTabMessages.moveGroupToNewWindow),
			moveTabToNewWindow: format(workbenchTabMessages.moveTabToNewWindow),
			moveTabsToNewWindow: format(workbenchTabMessages.moveTabsToNewWindow),
			moreOptions: format(workbenchTabMessages.moreOptions),
			newGroup: format(workbenchTabMessages.newGroup),
			newTab: format(workbenchTabMessages.newTab),
			newTabBelow: format(workbenchTabMessages.newTabBelow),
			newTabInGroup: format(workbenchTabMessages.newTabInGroup),
			newTabInNoGroups: format(workbenchTabMessages.newTabInNoGroups),
			newTabMenu: format(workbenchTabMessages.newTabMenu),
			newTabTitle,
			newTabToRight: format(workbenchTabMessages.newTabToRight),
			openVerticalTabs: format(workbenchTabMessages.openVerticalTabs),
			pin: format(workbenchTabMessages.pin),
			pinSearchTabs: format(workbenchTabMessages.pinSearchTabs),
			previewUnavailable: format(workbenchTabMessages.previewUnavailable),
			removeFromGroup: format(workbenchTabMessages.removeFromGroup),
			searchTabs: format(workbenchTabMessages.searchTabs),
			searchTabsMenu: format(workbenchTabMessages.searchTabsMenu),
			tabCrashed: format(workbenchTabMessages.tabCrashed),
			tabGroups: format(workbenchTabMessages.tabGroups),
			tabMoveCanceled: format(workbenchTabMessages.tabMoveCanceled),
			tabOptions: format(workbenchTabMessages.tabOptions),
			tabStripLabel: format(workbenchTabMessages.tabStripLabel),
			tabStripMenu: format(workbenchTabMessages.tabStripMenu),
			ungroup: format(workbenchTabMessages.ungroup),
			unnamedGroup,
			unpin: format(workbenchTabMessages.unpin),
			unpinSearchTabs: format(workbenchTabMessages.unpinSearchTabs),
			verticalTabStripMenu: format(workbenchTabMessages.verticalTabStripMenu),
		};
	}, [intl]);
}
