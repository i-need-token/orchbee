import { defineMessages } from 'react-intl';

import { enUSWorkbenchMessages } from '../locales';

export const workbenchTabMessages = defineMessages({
	actionCategoryTabs: {
		id: 'workbench.actions.categories.tabs',
		defaultMessage: enUSWorkbenchMessages['workbench.actions.categories.tabs'],
	},
	close: {
		id: 'workbench.tabs.actions.close',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.close'],
	},
	closeActiveTab: {
		id: 'workbench.tabs.actions.closeActive',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.closeActive'],
		description:
			'Search Action that closes the active Tab, the counterpart of Close Tab… in the TUI global action menu.',
	},
	closeTabsBelow: {
		id: 'workbench.tabs.actions.closeBelow',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.closeBelow'],
	},
	closeTabsToRight: {
		id: 'workbench.tabs.actions.closeToRight',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.closeToRight'],
	},
	commandPromptTitle: {
		id: 'workbench.tabs.titles.commandPrompt',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.titles.commandPrompt'],
		description: 'The name Windows gives cmd in this language, naming a terminal that runs it.',
	},
	duplicate: {
		id: 'workbench.tabs.actions.duplicate',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.duplicate'],
	},
	duplicateActiveTab: {
		id: 'workbench.tabs.actions.duplicateActive',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.duplicateActive'],
	},
	selectTab: {
		id: 'workbench.tabs.actions.select',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.select'],
	},
	selectNextTab: {
		id: 'workbench.tabs.actions.selectNext',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.selectNext'],
	},
	selectPreviousTab: {
		id: 'workbench.tabs.actions.selectPrevious',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.selectPrevious'],
	},
	moveTab: {
		id: 'workbench.tabs.actions.move',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.move'],
	},
	moveActiveTabBackward: {
		id: 'workbench.tabs.actions.moveBackward',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.moveBackward'],
	},
	moveActiveTabForward: {
		id: 'workbench.tabs.actions.moveForward',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.moveForward'],
	},
	renameTab: {
		id: 'workbench.tabs.actions.rename',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.rename'],
	},
	moveGroup: {
		id: 'workbench.tabs.actions.moveGroup',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.moveGroup'],
	},
	setGroupCollapsed: {
		id: 'workbench.tabs.actions.setGroupCollapsed',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.setGroupCollapsed'],
	},
	groupColor: {
		id: 'workbench.tabs.groups.color',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.groups.color'],
	},
	groupColorBlue: {
		id: 'workbench.tabs.groups.colors.blue',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.groups.colors.blue'],
	},
	groupColorCyan: {
		id: 'workbench.tabs.groups.colors.cyan',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.groups.colors.cyan'],
	},
	groupColorGreen: {
		id: 'workbench.tabs.groups.colors.green',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.groups.colors.green'],
	},
	groupColorGrey: {
		id: 'workbench.tabs.groups.colors.grey',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.groups.colors.grey'],
	},
	groupColorOrange: {
		id: 'workbench.tabs.groups.colors.orange',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.groups.colors.orange'],
	},
	groupColorPink: {
		id: 'workbench.tabs.groups.colors.pink',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.groups.colors.pink'],
	},
	groupColorPurple: {
		id: 'workbench.tabs.groups.colors.purple',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.groups.colors.purple'],
	},
	groupColorRed: {
		id: 'workbench.tabs.groups.colors.red',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.groups.colors.red'],
	},
	groupColorYellow: {
		id: 'workbench.tabs.groups.colors.yellow',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.groups.colors.yellow'],
	},
	groupMoveCanceled: {
		id: 'workbench.tabs.announcements.groupMoveCanceled',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.announcements.groupMoveCanceled'],
	},
	groupMoved: {
		id: 'workbench.tabs.announcements.groupMoved',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.announcements.groupMoved'],
	},
	groupNameCreateTitle: {
		id: 'workbench.tabs.groupName.createTitle',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.groupName.createTitle'],
	},
	groupNameHelp: {
		id: 'workbench.tabs.groupName.help',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.groupName.help'],
	},
	groupNameRenameTitle: {
		id: 'workbench.tabs.groupName.renameTitle',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.groupName.renameTitle'],
	},
	groupOptions: {
		id: 'workbench.tabs.menus.group',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.menus.group'],
	},
	moveTabsToGroup: {
		id: 'workbench.tabs.actions.moveToGroup',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.moveToGroup'],
	},
	moreOptions: {
		id: 'workbench.tabs.actions.more',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.more'],
	},
	newGroup: {
		id: 'workbench.tabs.actions.newGroup',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.newGroup'],
	},
	newGroupWithTab: {
		id: 'workbench.tabs.actions.newGroupWithTab',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.newGroupWithTab'],
	},
	newTab: {
		id: 'workbench.tabs.actions.new',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.new'],
	},
	newTabCurrentDirectory: {
		id: 'workbench.tabs.actions.newCurrentDirectory',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.newCurrentDirectory'],
	},
	newTabBelow: {
		id: 'workbench.tabs.actions.newBelow',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.newBelow'],
	},
	newTabInGroup: {
		id: 'workbench.tabs.actions.newInGroup',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.newInGroup'],
	},
	newTabInRecentGroup: {
		id: 'workbench.tabs.actions.newInRecentGroup',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.newInRecentGroup'],
		description:
			'Action that opens a Tab in the active Tab group, or else the group used most recently.',
	},
	newTabMenu: {
		id: 'workbench.tabs.menus.new',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.menus.new'],
	},
	newTabProfileCustomExecutable: {
		id: 'workbench.tabs.profiles.customExecutable',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.profiles.customExecutable'],
		description:
			'New Tab menu row for a saved launch profile without a name that runs a chosen program; matches the name Settings lists.',
	},
	newTabProfileDefault: {
		id: 'workbench.tabs.profiles.default',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.profiles.default'],
		description:
			'Trailing mark on the New Tab menu row for the launch profile a click on New Tab already uses.',
	},
	newTabProfileSystemShell: {
		id: 'workbench.tabs.profiles.systemShell',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.profiles.systemShell'],
		description:
			"New Tab menu row for a saved launch profile without a name that runs the system's shell; matches the name Settings lists.",
	},
	newTabAgentsHeading: {
		id: 'workbench.tabs.agents.heading',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.agents.heading'],
		description:
			'Heading of the New Tab menu section whose rows start an Agent in a new Tab; not a command.',
	},
	newTabMoreAgents: {
		id: 'workbench.tabs.agents.more',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.agents.more'],
		description:
			'Last row of the New Tab menu’s Agents section, which opens Start Agent to choose any Agent for a new Tab.',
	},
	loadingTabTitle: {
		id: 'workbench.tabs.titles.loading',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.titles.loading'],
	},
	newTabTitle: {
		id: 'workbench.tabs.titles.new',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.titles.new'],
	},
	newTabToRight: {
		id: 'workbench.tabs.actions.newToRight',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.newToRight'],
	},
	pin: {
		id: 'workbench.tabs.actions.pin',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.pin'],
	},
	positionTitle: {
		id: 'workbench.tabs.titles.position',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.titles.position'],
		description:
			'Names a Tab by its one-based place in the Tab Strip when nothing it runs gives a name.',
	},
	previewUnavailable: {
		id: 'workbench.tabs.states.previewUnavailable',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.states.previewUnavailable'],
	},
	tabGroups: {
		id: 'workbench.tabs.groups.collection',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.groups.collection'],
	},
	tabMoveCanceled: {
		id: 'workbench.tabs.announcements.tabMoveCanceled',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.announcements.tabMoveCanceled'],
	},
	tabMenuAddToGroup: {
		id: 'workbench.tabs.menu.addToGroup',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.menu.addToGroup'],
		description: 'A count of 1 means the Tab the menu opened on; a larger one is its selected set.',
	},
	tabMenuClose: {
		id: 'workbench.tabs.menu.close',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.menu.close'],
		description: 'A count of 1 means the Tab the menu opened on; a larger one is its selected set.',
	},
	tabMenuCloseOthers: {
		id: 'workbench.tabs.menu.closeOthers',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.menu.closeOthers'],
	},
	tabMenuDuplicate: {
		id: 'workbench.tabs.menu.duplicate',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.menu.duplicate'],
		description: 'A count of 1 means the Tab the menu opened on; a larger one is its selected set.',
	},
	tabMenuNewGroup: {
		id: 'workbench.tabs.menu.newGroup',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.menu.newGroup'],
		description: 'Asks for the new group’s name before creating it.',
	},
	tabMenuPin: {
		id: 'workbench.tabs.menu.pin',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.menu.pin'],
		description: 'A count of 1 means the Tab the menu opened on; a larger one is its selected set.',
	},
	tabMenuRemoveFromGroup: {
		id: 'workbench.tabs.menu.removeFromGroup',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.menu.removeFromGroup'],
		description: 'A count of 1 means the Tab the menu opened on; a larger one is its selected set.',
	},
	tabMenuRename: {
		id: 'workbench.tabs.menu.rename',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.menu.rename'],
		description: 'Renames the Tab or group the menu opened on, which the menu does not repeat.',
	},
	tabMenuUngroup: {
		id: 'workbench.tabs.menu.ungroup',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.menu.ungroup'],
	},
	tabMenuUnpin: {
		id: 'workbench.tabs.menu.unpin',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.menu.unpin'],
		description: 'A count of 1 means the Tab the menu opened on; a larger one is its selected set.',
	},
	tabOptions: {
		id: 'workbench.tabs.menus.tab',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.menus.tab'],
	},
	tabStripLabel: {
		id: 'workbench.tabs.label',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.label'],
	},
	terminalTitle: {
		id: 'workbench.tabs.titles.terminal',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.titles.terminal'],
	},
	ungroup: {
		id: 'workbench.tabs.actions.ungroup',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.ungroup'],
	},
	unavailableMissing: {
		id: 'workbench.tabs.unavailable.missing',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.unavailable.missing'],
		description:
			'Reason a Tab action is disabled: none of the Tabs it names remains; count is how many it names.',
	},
	unavailableNoneSelected: {
		id: 'workbench.tabs.unavailable.noneSelected',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.unavailable.noneSelected'],
		description: 'Reason an action on the active Tab is disabled: the Workspace has no active Tab.',
	},
	unavailableOnlyOne: {
		id: 'workbench.tabs.unavailable.onlyOne',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.unavailable.onlyOne'],
		description: 'Reason Select Next or Previous Tab is disabled: the Workspace has a single Tab.',
	},
	unavailableOnlyPinned: {
		id: 'workbench.tabs.unavailable.onlyPinned',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.unavailable.onlyPinned'],
		description:
			'Reason moving the active pinned Tab is disabled: it is the only pinned Tab, and a Tab moves only among its kind.',
	},
	unavailableOnlyUnpinned: {
		id: 'workbench.tabs.unavailable.onlyUnpinned',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.unavailable.onlyUnpinned'],
		description:
			'Reason moving the active unpinned Tab is disabled: it is the only unpinned Tab, and a Tab moves only among its kind.',
	},
	unavailablePositionEmpty: {
		id: 'workbench.tabs.unavailable.positionEmpty',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.unavailable.positionEmpty'],
		description: 'Reason selecting a Tab by position is disabled: the Workspace has fewer Tabs.',
	},
	unavailableAmongPinned: {
		id: 'workbench.tabs.unavailable.amongPinned',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.unavailable.amongPinned'],
		description:
			'Reason creating a Tab at a position is disabled: the position lies among pinned Tabs, and a new Tab is never pinned.',
	},
	unavailableNoMatchingTabs: {
		id: 'workbench.tabs.unavailable.noMatchingTabs',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.unavailable.noMatchingTabs'],
		description:
			'Reason a close action is disabled: no Tab falls in its set. Matches the TUI reason for the same condition.',
	},
	unavailableGroupMissing: {
		id: 'workbench.tabs.unavailable.groupMissing',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.unavailable.groupMissing'],
		description: 'Reason a Tab group action is disabled: the group it names is gone.',
	},
	unavailableNoRecentGroup: {
		id: 'workbench.tabs.unavailable.noRecentGroup',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.unavailable.noRecentGroup'],
		description:
			'Reason New Tab in recent group is disabled: no group exists. Matches the TUI reason for the same condition.',
	},
	unnamedGroup: {
		id: 'workbench.tabs.groups.unnamed',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.groups.unnamed'],
	},
	updateGroup: {
		id: 'workbench.tabs.actions.updateGroup',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.updateGroup'],
	},
	closedAnnouncement: {
		id: 'workbench.tabs.announcements.closed',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.announcements.closed'],
	},
	closedTabsAnnouncement: {
		id: 'workbench.tabs.announcements.closedMany',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.announcements.closedMany'],
	},
	dragCompleteAnnouncement: {
		id: 'workbench.tabs.announcements.dragComplete',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.announcements.dragComplete'],
	},
	groupContent: {
		id: 'workbench.tabs.accessibility.groupContent',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.accessibility.groupContent'],
	},
	groupMembershipAnnouncement: {
		id: 'workbench.tabs.announcements.groupMembership',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.announcements.groupMembership'],
	},
	groupNameAccessibility: {
		id: 'workbench.tabs.accessibility.groupName',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.accessibility.groupName'],
	},
	groupOverflowAccessibility: {
		id: 'workbench.tabs.accessibility.groupOverflow',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.accessibility.groupOverflow'],
	},
	groupTabTitleAccessibility: {
		id: 'workbench.tabs.accessibility.groupTabTitle',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.accessibility.groupTabTitle'],
	},
	groupTooltip: {
		id: 'workbench.tabs.accessibility.groupTooltip',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.accessibility.groupTooltip'],
	},
	movedDownAnnouncement: {
		id: 'workbench.tabs.announcements.movedDown',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.announcements.movedDown'],
	},
	movedFirstAnnouncement: {
		id: 'workbench.tabs.announcements.movedFirst',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.announcements.movedFirst'],
	},
	movedLastAnnouncement: {
		id: 'workbench.tabs.announcements.movedLast',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.announcements.movedLast'],
	},
	movedLeftAnnouncement: {
		id: 'workbench.tabs.announcements.movedLeft',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.announcements.movedLeft'],
	},
	movedRightAnnouncement: {
		id: 'workbench.tabs.announcements.movedRight',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.announcements.movedRight'],
	},
	movedUpAnnouncement: {
		id: 'workbench.tabs.announcements.movedUp',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.announcements.movedUp'],
	},
	tabNameAccessibility: {
		id: 'workbench.tabs.accessibility.tabName',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.accessibility.tabName'],
	},
});
