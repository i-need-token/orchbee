export const enUSFileExplorerMessages = {
	'fileExplorer.actions.collapse': 'Collapse Folders in Explorer',
	'fileExplorer.actions.refresh': 'Refresh Explorer',
	'fileExplorer.decorations.containsEmphasizedItems': 'Contains emphasized items',
	'fileExplorer.decorations.symbolicLink': 'Symbolic Link',
	'fileExplorer.decorations.unknownType': 'Unknown File Type',
	'fileExplorer.empty.noDirectory': 'No directory configured.',
	'fileExplorer.folder.unreadable': 'Unable to read the folder {name} ({reason})',
	'fileExplorer.progress.label': 'Loading folders',
	'fileExplorer.readFailure.disconnected': 'the machine is disconnected',
	'fileExplorer.readFailure.failed': 'an unexpected error occurred',
	'fileExplorer.readFailure.notDirectory': 'it is not a folder',
	'fileExplorer.readFailure.notFound': 'it does not exist',
	'fileExplorer.readFailure.permissionDenied': 'permission denied',
	'fileExplorer.readFailure.timeout': 'the folder took too long to respond',
	'fileExplorer.readFailure.unavailable': 'files on this machine are unavailable',
	'fileExplorer.readFailure.unsupportedLocation': 'this kind of location is not supported',
	'fileExplorer.root.unresolved': 'Unable to resolve workspace folder ({reason})',
	'fileExplorer.section.label': 'Explorer Section: {name}',
	'fileExplorer.tree.label': 'Files Explorer',
} as const;

export type FileExplorerMessageId = keyof typeof enUSFileExplorerMessages;
