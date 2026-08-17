export const enUSDesktopWindowMessages = {
	'desktop.window.action.toggleFullScreen': 'Enter / Exit Full Screen',
	'desktop.window.action.toggleMaximized': 'Maximize / Restore Window',
	'desktop.window.actionUnavailable': 'No desktop window is available.',
	'desktop.window.category': 'Window',
	'desktop.window.maximizedUnavailableInFullScreen':
		'Exit full screen before changing the maximized state.',
} as const;

export type DesktopWindowMessageId = keyof typeof enUSDesktopWindowMessages;
