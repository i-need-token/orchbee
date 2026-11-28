export const enUSFileBarMessages = {
	'fileBar.actions.hide': 'Hide File Bar',
	'fileBar.actions.show': 'Show File Bar',
	'fileBar.actions.showExplorer': 'Show Explorer',
	'fileBar.actions.showSourceControl': 'Show Source Control',
	'fileBar.actions.toggle': 'Toggle File Bar',
	'fileBar.badge.labelled': '{label} - {badge}',
	'fileBar.badge.overThousands': '{count}K+',
	'fileBar.badge.thousands': '{count}K',
	'fileBar.label': 'File Bar',
	'fileBar.loadFailed': 'Couldn’t load the File Bar.',
	'fileBar.tree.compactItem': '{name}, compact, {parents}',
	'fileBar.views.explorer': 'Explorer',
	'fileBar.views.sourceControl': 'Source Control',
	'fileBar.viewSwitcher': 'Active View Switcher',
} as const;

export type FileBarMessageId = keyof typeof enUSFileBarMessages;
