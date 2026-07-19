export const enUSWorkbenchMessages = {
	'workbench.sidebar.actions.hide': 'Hide Sidebar',
	'workbench.sidebar.actions.show': 'Show Sidebar',
	'workbench.tabs.actions.add': 'New Tab',
	'workbench.tabs.actions.close': 'Close {tabLabel}',
	'workbench.tabs.label': 'Workbench tabs',
	'workbench.tabs.untitled': 'New Tab {tabNumber}',
} as const;

export type WorkbenchMessageId = keyof typeof enUSWorkbenchMessages;
