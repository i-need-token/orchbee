export const enUSRemoteDirectoryBrowserMessages = {
	'remoteDirectoryBrowser.actions.cancel': 'Cancel',
	'remoteDirectoryBrowser.actions.editPath': 'Edit folder path',
	'remoteDirectoryBrowser.actions.go': 'Go',
	'remoteDirectoryBrowser.actions.showHidden': 'Show hidden folders',
	'remoteDirectoryBrowser.actions.useFolder': 'Use This Folder',
	'remoteDirectoryBrowser.empty': 'This folder has no accessible subfolders.',
	'remoteDirectoryBrowser.empty.hidden':
		'This folder has no visible subfolders. Show hidden folders to see more.',
	'remoteDirectoryBrowser.failure.busy':
		'The remote source is busy. Wait a moment, then try this path again.',
	'remoteDirectoryBrowser.failure.invalidPath': 'Enter an absolute folder path.',
	'remoteDirectoryBrowser.failure.notDirectory': 'That path is not a folder.',
	'remoteDirectoryBrowser.failure.notFound': 'That folder could not be found.',
	'remoteDirectoryBrowser.failure.permission': 'You do not have permission to open that folder.',
	'remoteDirectoryBrowser.failure.timedOut': 'The remote source took too long to open that folder.',
	'remoteDirectoryBrowser.failure.unavailable':
		'The folder could not be loaded. Check the connection and try again.',
	'remoteDirectoryBrowser.filter.noMatches':
		'No listed folders match this path. You can still go to the complete path.',
	'remoteDirectoryBrowser.filter.partial':
		'Select a matching folder below, or finish entering its complete path.',
	'remoteDirectoryBrowser.listLabel': 'Folders',
	'remoteDirectoryBrowser.loading': 'Loading folder…',
	'remoteDirectoryBrowser.locationLabel': 'Current remote location',
	'remoteDirectoryBrowser.path': 'Folder path',
	'remoteDirectoryBrowser.truncated':
		'Some folders are not shown. Enter an absolute path to open another folder.',
} as const;

export type RemoteDirectoryBrowserMessageId = keyof typeof enUSRemoteDirectoryBrowserMessages;
