export const enUSWorkbenchMessages = {
	'workbench.sidebar.label': 'Workbench navigation',
	'workbench.sidebar.actions.hide': 'Hide Sidebar',
	'workbench.sidebar.actions.show': 'Show Sidebar',
	'workbench.sidebar.items.newTerminal': 'New Terminal',
	'workbench.sidebar.items.search': 'Search',
	'workbench.sidebar.items.settings': 'Settings',
	'workbench.sidebar.sections.project': 'Project',
} as const;

export type WorkbenchMessageId = keyof typeof enUSWorkbenchMessages;
