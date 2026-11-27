export const enUSFileBarMessages = {
	'fileBar.actions.toggle': 'Toggle File Bar',
	'fileBar.label': 'File Bar',
	'fileBar.loadFailed': 'Couldn’t load the File Bar.',
} as const;

export type FileBarMessageId = keyof typeof enUSFileBarMessages;
