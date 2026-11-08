export const enUSDesktopWindowMessages = {
	'desktop.window.action.toggleFullScreen': 'Enter / Exit Full Screen',
	'desktop.window.action.toggleMaximized': 'Maximize / Restore Window',
	'desktop.window.actionUnavailable': 'No desktop window is available.',
	'desktop.window.category': 'Window',
	'desktop.window.dataFolderLayoutNewer':
		"{root} was last used by Orchbee {version}, which changed how the folder is laid out. This version can't open it; use Orchbee {version} or later.",
	'desktop.window.dataFolderLayoutNewerUnnamed':
		"{root} was last used by a newer version of Orchbee, which changed how the folder is laid out. This version can't open it; use that version or later.",
	'desktop.window.dataFolderRefused': "Orchbee can't open its data folder",
	'desktop.window.dataFolderUnreadable':
		"Orchbee can't read {record}, so it won't open {root}. Restore the file, or move the folder away to start again.",
	'desktop.window.placementNewer':
		"The window placement was saved by a newer version of Orchbee, so this version won't change it. The window opens at its default size and doesn't remember where you put it.",
	'desktop.window.placementNotUpdated':
		"Orchbee couldn't update the window placement from an earlier version and left it unchanged. The window opens at its default size and doesn't remember where you put it.",
	'desktop.window.recordNoticeClose': 'Close',
	'desktop.window.maximizedUnavailableInFullScreen':
		'Exit full screen before changing the maximized state.',
} as const;

export type DesktopWindowMessageId = keyof typeof enUSDesktopWindowMessages;
