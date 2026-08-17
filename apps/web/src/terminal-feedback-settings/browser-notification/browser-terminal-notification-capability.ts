import type { TerminalNotificationCapability, TerminalNotificationPermission } from '../model';

export function createBrowserTerminalNotificationCapability(
	targetWindow: Pick<Window, 'focus'>,
): TerminalNotificationCapability {
	const notificationConstructor = typeof Notification === 'undefined' ? undefined : Notification;
	if (notificationConstructor === undefined) return UNSUPPORTED_TERMINAL_NOTIFICATION_CAPABILITY;
	return {
		permission: () => normalizePermission(notificationConstructor.permission),
		requestPermission: async () =>
			normalizePermission(await notificationConstructor.requestPermission()),
		show: ({ body, tag, title }) => {
			if (notificationConstructor.permission !== 'granted') return false;
			const notification = new notificationConstructor(title, { body, silent: true, tag });
			notification.addEventListener('click', () => targetWindow.focus(), { once: true });
			return true;
		},
	};
}

export const UNSUPPORTED_TERMINAL_NOTIFICATION_CAPABILITY: TerminalNotificationCapability =
	Object.freeze({
		permission: () => 'unsupported',
		requestPermission: (): Promise<TerminalNotificationPermission> =>
			Promise.resolve('unsupported'),
		show: () => false,
	});

function normalizePermission(permission: NotificationPermission): TerminalNotificationPermission {
	switch (permission) {
		case 'denied':
		case 'granted':
			return permission;
		case 'default':
			return 'prompt';
	}
}
