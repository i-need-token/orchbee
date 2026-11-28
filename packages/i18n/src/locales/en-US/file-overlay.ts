export const enUSFileOverlayMessages = {
	'fileOverlay.actions.close': 'Close',
	'fileOverlay.actions.closeEditor': 'Close Editor',
	'fileOverlay.actions.expand': 'Expand',
	'fileOverlay.actions.keepEditor': 'Keep Editor',
	'fileOverlay.actions.restore': 'Restore',
	'fileOverlay.actions.retry': 'Try Again',
	'fileOverlay.actions.showEditor': 'Show Editor',
	'fileOverlay.actions.toggleFull': 'Expand / Restore File Overlay',
	'fileOverlay.actions.unpin': 'Unpin',
	'fileOverlay.actions.unpinEditor': 'Unpin Editor',
	'fileOverlay.label': 'File Overlay',
	'fileOverlay.loadFailed': 'Couldn’t load the file overlay.',
	'fileOverlay.tabs.label': 'Open Editors',
	'fileOverlay.tabs.pinnedName': '{name}, pinned',
	'fileOverlay.tabs.previewName': '{name}, preview',
	'fileOverlay.tabs.previewPath': '{path} (preview)',
} as const;

export type FileOverlayMessageId = keyof typeof enUSFileOverlayMessages;
