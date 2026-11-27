export const enUSFileOverlayMessages = {
	'fileOverlay.actions.expand': 'Expand',
	'fileOverlay.actions.restore': 'Restore',
	'fileOverlay.actions.retry': 'Try Again',
	'fileOverlay.actions.toggleFull': 'Expand / Restore File Overlay',
	'fileOverlay.label': 'File Overlay',
	'fileOverlay.loadFailed': 'Couldn’t load the file overlay.',
} as const;

export type FileOverlayMessageId = keyof typeof enUSFileOverlayMessages;
