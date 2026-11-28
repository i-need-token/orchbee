export const enUSFileExplorerMessages = {
	'fileExplorer.actions.collapse': 'Collapse Folders in Explorer',
	'fileExplorer.actions.refresh': 'Refresh Explorer',
	'fileExplorer.decorations.symbolicLink': 'Symbolic Link',
	'fileExplorer.decorations.unknownType': 'Unknown File Type',
	'fileExplorer.empty.noDirectory': 'No directory configured.',
	'fileExplorer.root.unresolved': 'Unable to resolve workspace folder ({reason})',
	'fileExplorer.root.unresolvedReason.disconnected': 'the machine is disconnected',
	'fileExplorer.root.unresolvedReason.failed': 'an unexpected error occurred',
	'fileExplorer.root.unresolvedReason.notDirectory': 'it is not a folder',
	'fileExplorer.root.unresolvedReason.notFound': 'it does not exist',
	'fileExplorer.root.unresolvedReason.permissionDenied': 'permission denied',
	'fileExplorer.root.unresolvedReason.timeout': 'the folder took too long to respond',
	'fileExplorer.root.unresolvedReason.unavailable': 'files on this machine are unavailable',
	'fileExplorer.root.unresolvedReason.unsupportedLocation':
		'this kind of location is not supported',
	'fileExplorer.section.label': 'Explorer Section: {name}',
	'fileExplorer.tree.label': 'Files Explorer',
} as const;

export type FileExplorerMessageId = keyof typeof enUSFileExplorerMessages;
