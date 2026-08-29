import { afterEach, describe, expect, it, vi } from 'vitest';

import {
	createBrowserTerminalNotificationCapability,
	UNSUPPORTED_TERMINAL_NOTIFICATION_CAPABILITY,
} from '../browser-terminal-notification-capability';

afterEach(() => vi.unstubAllGlobals());

describe('createBrowserTerminalNotificationCapability', () => {
	it('is inert when the browser does not expose notifications', async () => {
		vi.stubGlobal('Notification', undefined);
		const capability = createBrowserTerminalNotificationCapability({
			focus: vi.fn<() => void>(),
		});

		expect(capability).toBe(UNSUPPORTED_TERMINAL_NOTIFICATION_CAPABILITY);
		expect(capability.permission()).toBe('unsupported');
		await expect(capability.requestPermission()).resolves.toBe('unsupported');
		expect(capability.show({ body: 'safe body', tag: 'terminal', title: 'safe title' })).toBe(
			false,
		);
	});

	it('normalizes permission and focuses the owning window only from a notification click', async () => {
		const focus = vi.fn<() => void>();
		class NotificationProbe {
			static permission: NotificationPermission = 'default';
			static readonly requestPermission = vi.fn<() => Promise<'granted'>>(async () => 'granted');
			static latest: NotificationProbe | undefined;
			readonly addEventListener = vi.fn<
				(type: string, listener: () => void, options?: AddEventListenerOptions | boolean) => void
			>((_type, listener, options) => {
				this.click = listener;
				this.options = options;
			});
			click: (() => void) | undefined;
			readonly notificationOptions: NotificationOptions | undefined;
			options: AddEventListenerOptions | boolean | undefined;
			readonly title: string;

			constructor(title: string, notificationOptions?: NotificationOptions) {
				this.notificationOptions = notificationOptions;
				this.title = title;
				NotificationProbe.latest = this;
			}
		}
		vi.stubGlobal('Notification', NotificationProbe);
		const capability = createBrowserTerminalNotificationCapability({ focus });

		expect(capability.permission()).toBe('prompt');
		await expect(capability.requestPermission()).resolves.toBe('granted');
		expect(NotificationProbe.requestPermission).toHaveBeenCalledOnce();
		expect(capability.show({ body: 'safe body', tag: 'terminal:1', title: 'safe title' })).toBe(
			false,
		);

		NotificationProbe.permission = 'granted';
		const onActivate = vi.fn<() => void>();
		expect(
			capability.show({ body: 'safe body', onActivate, tag: 'terminal:1', title: 'safe title' }),
		).toBe(true);
		expect(NotificationProbe.latest).toMatchObject({
			notificationOptions: { body: 'safe body', silent: true, tag: 'terminal:1' },
			title: 'safe title',
		});
		expect(NotificationProbe.latest?.options).toEqual({ once: true });
		expect(focus).not.toHaveBeenCalled();
		NotificationProbe.latest?.click?.();
		expect(focus).toHaveBeenCalledOnce();
		expect(onActivate).toHaveBeenCalledOnce();
	});
});
