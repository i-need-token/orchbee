import { defineMessages } from 'react-intl';

import { enUSWorkbenchMessages } from '../locales';

export const workbenchMessages = defineMessages({
	addTabToGroup: {
		id: 'workbench.tabs.contextMenu.actions.addToGroup',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.contextMenu.actions.addToGroup'],
		description: 'Context-menu submenu that assigns an unpinned Workbench tab to a group.',
	},
	addTabToNewGroup: {
		id: 'workbench.tabs.contextMenu.actions.addToNewGroup',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.contextMenu.actions.addToNewGroup'],
		description: 'Context-menu action that creates a group for an unpinned Workbench tab.',
	},
	addTab: {
		id: 'workbench.tabs.actions.add',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.add'],
		description: 'Action that creates a new Workbench tab.',
	},
	closeTab: {
		id: 'workbench.tabs.actions.close',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.close'],
		description: 'Action that closes a named Workbench tab.',
	},
	closeAllTabs: {
		id: 'workbench.tabs.contextMenu.actions.closeAll',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.contextMenu.actions.closeAll'],
		description: 'Context-menu action that closes every Workbench tab.',
	},
	closeCurrentTab: {
		id: 'workbench.tabs.contextMenu.actions.close',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.contextMenu.actions.close'],
		description: 'Context-menu action that closes its Workbench tab.',
	},
	closeOtherTabs: {
		id: 'workbench.tabs.contextMenu.actions.closeOthers',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.contextMenu.actions.closeOthers'],
		description: 'Context-menu action that closes every Workbench tab except its target.',
	},
	closeTabsToRight: {
		id: 'workbench.tabs.contextMenu.actions.closeRight',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.contextMenu.actions.closeRight'],
		description: 'Context-menu action that closes Workbench tabs to the right of its target.',
	},
	colorBlue: {
		id: 'workbench.tabs.contextMenu.colors.blue',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.contextMenu.colors.blue'],
		description: 'Accessible name for the blue Workbench tab color.',
	},
	colorCyan: {
		id: 'workbench.tabs.contextMenu.colors.cyan',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.contextMenu.colors.cyan'],
		description: 'Accessible name for the cyan Workbench tab color.',
	},
	colorDefault: {
		id: 'workbench.tabs.contextMenu.colors.default',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.contextMenu.colors.default'],
		description: 'Accessible name for the default Workbench tab color.',
	},
	colorGray: {
		id: 'workbench.tabs.contextMenu.colors.gray',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.contextMenu.colors.gray'],
		description: 'Accessible name for the gray Workbench tab color.',
	},
	colorGreen: {
		id: 'workbench.tabs.contextMenu.colors.green',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.contextMenu.colors.green'],
		description: 'Accessible name for the green Workbench tab color.',
	},
	colorOrange: {
		id: 'workbench.tabs.contextMenu.colors.orange',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.contextMenu.colors.orange'],
		description: 'Accessible name for the orange Workbench tab color.',
	},
	colorPink: {
		id: 'workbench.tabs.contextMenu.colors.pink',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.contextMenu.colors.pink'],
		description: 'Accessible name for the pink Workbench tab color.',
	},
	colorPurple: {
		id: 'workbench.tabs.contextMenu.colors.purple',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.contextMenu.colors.purple'],
		description: 'Accessible name for the purple Workbench tab color.',
	},
	colorRed: {
		id: 'workbench.tabs.contextMenu.colors.red',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.contextMenu.colors.red'],
		description: 'Accessible name for the red Workbench tab color.',
	},
	colorYellow: {
		id: 'workbench.tabs.contextMenu.colors.yellow',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.contextMenu.colors.yellow'],
		description: 'Accessible name for the yellow Workbench tab color.',
	},
	closeTabGroup: {
		id: 'workbench.tabs.groups.actions.close',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.groups.actions.close'],
		description: 'Action that closes every tab in a Workbench tab group.',
	},
	collapseTabGroup: {
		id: 'workbench.tabs.groups.actions.collapse',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.groups.actions.collapse'],
		description: 'Accessible name for collapsing a named Workbench tab group.',
	},
	deleteTabGroup: {
		id: 'workbench.tabs.groups.actions.delete',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.groups.actions.delete'],
		description: 'Action that closes a Workbench tab group and deletes its metadata.',
	},
	expandTabGroup: {
		id: 'workbench.tabs.groups.actions.expand',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.groups.actions.expand'],
		description: 'Accessible name for expanding a named Workbench tab group.',
	},
	newTabInGroup: {
		id: 'workbench.tabs.groups.actions.newTab',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.groups.actions.newTab'],
		description: 'Action that creates a new tab inside a Workbench tab group.',
	},
	newTerminal: {
		id: 'workbench.sidebar.items.newTerminal',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.items.newTerminal'],
		description: 'Label for the new-terminal entry in the Workbench sidebar.',
	},
	project: {
		id: 'workbench.sidebar.sections.project',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.sections.project'],
		description: 'Heading for the project section in the Workbench sidebar.',
	},
	pinTab: {
		id: 'workbench.tabs.contextMenu.actions.pin',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.contextMenu.actions.pin'],
		description: 'Context-menu action that pins a Workbench tab.',
	},
	renameTab: {
		id: 'workbench.tabs.contextMenu.actions.rename',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.contextMenu.actions.rename'],
		description: 'Context-menu action that opens the Workbench tab rename dialog.',
	},
	renameTabGroup: {
		id: 'workbench.tabs.groups.actions.rename',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.groups.actions.rename'],
		description: 'Context-menu action that opens the Workbench tab-group rename dialog.',
	},
	renameTabGroupDescription: {
		id: 'workbench.tabs.groups.rename.description',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.groups.rename.description'],
		description: 'Instruction shown while renaming a named Workbench tab group.',
	},
	renameTabGroupTitle: {
		id: 'workbench.tabs.groups.rename.title',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.groups.rename.title'],
		description: 'Title of the Workbench tab-group rename dialog.',
	},
	renameTabCancel: {
		id: 'workbench.tabs.rename.actions.cancel',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.rename.actions.cancel'],
		description: 'Action that cancels renaming a Workbench tab.',
	},
	renameTabConfirm: {
		id: 'workbench.tabs.rename.actions.confirm',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.rename.actions.confirm'],
		description: 'Action that confirms a new Workbench tab name.',
	},
	renameTabDescription: {
		id: 'workbench.tabs.rename.description',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.rename.description'],
		description: 'Instruction shown while renaming a named Workbench tab.',
	},
	renameTabInputLabel: {
		id: 'workbench.tabs.rename.inputLabel',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.rename.inputLabel'],
		description: 'Accessible label for the Workbench tab name input.',
	},
	renameTabTitle: {
		id: 'workbench.tabs.rename.title',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.rename.title'],
		description: 'Title of the Workbench tab rename dialog.',
	},
	removeTabFromGroup: {
		id: 'workbench.tabs.contextMenu.actions.removeFromGroup',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.contextMenu.actions.removeFromGroup'],
		description: 'Context-menu action that removes a Workbench tab from its group.',
	},
	search: {
		id: 'workbench.sidebar.items.search',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.items.search'],
		description: 'Label for the search entry in the Workbench sidebar.',
	},
	settings: {
		id: 'workbench.sidebar.items.settings',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.items.settings'],
		description: 'Label for the settings entry in the Workbench sidebar footer.',
	},
	tabColor: {
		id: 'workbench.tabs.contextMenu.tabColor',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.contextMenu.tabColor'],
		description: 'Heading for the Workbench tab color choices.',
	},
	tabGroupColor: {
		id: 'workbench.tabs.groups.color',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.groups.color'],
		description: 'Accessible label for Workbench tab-group color choices.',
	},
	tabGroupDefaultLabel: {
		id: 'workbench.tabs.groups.defaultLabel',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.groups.defaultLabel'],
		description: 'Default label for a newly created Workbench tab group.',
	},
	tabGroupName: {
		id: 'workbench.tabs.groups.name',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.groups.name'],
		description: 'Accessible label for the Workbench tab-group name input.',
	},
	hideSidebar: {
		id: 'workbench.sidebar.actions.hide',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.actions.hide'],
		description: 'Action that hides the Workbench sidebar.',
	},
	showSidebar: {
		id: 'workbench.sidebar.actions.show',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.actions.show'],
		description: 'Action that shows the Workbench sidebar.',
	},
	sidebarLabel: {
		id: 'workbench.sidebar.label',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.label'],
		description: 'Accessible label for the Workbench navigation sidebar.',
	},
	tabsLabel: {
		id: 'workbench.tabs.label',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.label'],
		description: 'Accessible label for the Workbench tab list.',
	},
	untitledTab: {
		id: 'workbench.tabs.untitled',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.untitled'],
		description: 'Default label for a newly created Workbench tab.',
	},
	ungroupTabs: {
		id: 'workbench.tabs.groups.actions.ungroup',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.groups.actions.ungroup'],
		description: 'Action that removes grouping from every tab in a Workbench tab group.',
	},
	unpinTab: {
		id: 'workbench.tabs.contextMenu.actions.unpin',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.contextMenu.actions.unpin'],
		description: 'Context-menu action that unpins a Workbench tab.',
	},
});
