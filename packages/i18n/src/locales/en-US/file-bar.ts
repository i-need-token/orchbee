export const enUSFileBarMessages = {
	'fileBar.actions.hide': 'Hide File Bar',
	'fileBar.actions.show': 'Show File Bar',
	'fileBar.actions.showExplorer': 'Show Explorer',
	'fileBar.actions.showSourceControl': 'Show Source Control',
	'fileBar.actions.toggle': 'Toggle File Bar',
	'fileBar.label': 'File Bar',
	'fileBar.loadFailed': 'Couldn’t load the File Bar.',
	'fileBar.views.explorer': 'Explorer',
	'fileBar.views.sourceControl': 'Source Control',
	'fileBar.viewSwitcher': 'Active View Switcher',
} as const;

export type FileBarMessageId = keyof typeof enUSFileBarMessages;
