import { describe, expect, it, vi } from 'vitest';

import { memoryUserPreferences } from '../../../__tests__/memoryUserPreferences';
import {
	TerminalFeedbackSettingsController,
	type TerminalNotificationCapability,
} from '../TerminalFeedbackSettingsController';

const NOW = 1_786_947_200_000;

function completion(
	sequence: bigint,
	durationMilliseconds = 5_000n,
	completedAtMilliseconds = BigInt(NOW),
) {
	return { completedAtMilliseconds, durationMilliseconds, exitCode: 0, sequence };
}

function notificationCapability(
	permission: 'denied' | 'granted' | 'prompt' | 'unsupported' = 'granted',
): TerminalNotificationCapability & {
	readonly requestPermission: ReturnType<
		typeof vi.fn<TerminalNotificationCapability['requestPermission']>
	>;
	readonly show: ReturnType<typeof vi.fn<TerminalNotificationCapability['show']>>;
} {
	return {
		permission: () => permission,
		requestPermission: vi.fn<TerminalNotificationCapability['requestPermission']>(() =>
			Promise.resolve('granted'),
		),
		show: vi.fn<TerminalNotificationCapability['show']>(() => true),
	};
}

describe('TerminalFeedbackSettingsController', () => {
	it('starts off and never asks for notification permission from terminal output', () => {
		const notifications = notificationCapability('prompt');
		const controller = new TerminalFeedbackSettingsController({
			clock: () => NOW,
			notificationCapability: notifications,
			userPreferences: memoryUserPreferences(),
		});

		expect(controller.getSnapshot()).toEqual({
			notificationPermission: 'prompt',
			preferences: {
				soundEnabled: false,
				agentNotificationMode: 'never',
				completionReminderMinimumSeconds: 5,
				completionReminderMode: 'never',
			},
		});
		expect(
			controller.observeCompletion({
				completion: completion(1n),
				focused: false,
				notification: { body: 'safe', tag: 'terminal', title: 'completed' },
				terminalIdentity: 'terminal-a',
			}),
		).toEqual({ kind: 'ignored', reason: 'turned-off' });
		expect(notifications.requestPermission).not.toHaveBeenCalled();
		expect(notifications.show).not.toHaveBeenCalled();
	});

	it('deduplicates completion sequences and rate limits only the system effect', async () => {
		const notifications = notificationCapability();
		const controller = new TerminalFeedbackSettingsController({
			clock: () => NOW,
			notificationCapability: notifications,
			userPreferences: memoryUserPreferences(),
		});
		expect(controller.update({ completionReminderMode: 'always' })).toEqual({ kind: 'pending' });
		await Promise.resolve();
		const observation = {
			completion: completion(1n),
			focused: true,
			notification: { body: 'safe', tag: 'terminal', title: 'completed' },
			terminalIdentity: 'terminal-a',
		};
		expect(controller.observeCompletion(observation)).toEqual({
			kind: 'remind',
			notification: 'shown',
		});
		expect(controller.observeCompletion(observation)).toEqual({
			kind: 'ignored',
			reason: 'duplicate',
		});
		expect(controller.observeCompletion({ ...observation, completion: completion(2n) })).toEqual({
			kind: 'remind',
			notification: 'rate-limited',
		});
		expect(
			controller.observeCompletion({
				...observation,
				completion: completion(1n),
				terminalIdentity: 'terminal-b',
			}),
		).toEqual({ kind: 'remind', notification: 'shown' });
		expect(notifications.show).toHaveBeenCalledTimes(2);
	});

	it('enforces focus, duration, freshness, and future-time policy before effects', () => {
		const notifications = notificationCapability();
		const controller = new TerminalFeedbackSettingsController({
			clock: () => NOW,
			notificationCapability: notifications,
			userPreferences: memoryUserPreferences({ completionReminderMode: 'unfocused' }),
		});
		const base = {
			focused: false,
			notification: { body: 'safe', tag: 'terminal', title: 'completed' },
		};
		expect(
			controller.observeCompletion({
				...base,
				completion: completion(1n),
				focused: true,
				terminalIdentity: 'focused',
			}),
		).toEqual({ kind: 'ignored', reason: 'focused' });
		expect(
			controller.observeCompletion({
				...base,
				completion: completion(1n, 4_999n),
				terminalIdentity: 'short',
			}),
		).toEqual({ kind: 'ignored', reason: 'short' });
		expect(
			controller.observeCompletion({
				...base,
				completion: completion(1n, 5_000n, BigInt(NOW - 60_001)),
				terminalIdentity: 'stale',
			}),
		).toEqual({ kind: 'ignored', reason: 'stale' });
		expect(
			controller.observeCompletion({
				...base,
				completion: completion(1n, 5_000n, BigInt(NOW + 5_001)),
				terminalIdentity: 'future',
			}),
		).toEqual({ kind: 'ignored', reason: 'future' });
		expect(notifications.show).not.toHaveBeenCalled();
	});

	it('requests permission only explicitly and ignores a late result after disposal', async () => {
		let resolvePermission: ((permission: 'granted') => void) | undefined;
		const notifications = notificationCapability('prompt');
		notifications.requestPermission.mockImplementation(
			() =>
				new Promise((resolve) => {
					resolvePermission = resolve;
				}),
		);
		const controller = new TerminalFeedbackSettingsController({
			notificationCapability: notifications,
			userPreferences: memoryUserPreferences(),
		});
		const request = controller.requestNotificationPermission();
		expect(notifications.requestPermission).toHaveBeenCalledOnce();
		controller.dispose();
		resolvePermission?.('granted');
		expect(await request).toBe('prompt');
		expect(controller.getSnapshot().notificationPermission).toBe('prompt');
	});

	it('notifies once per exact Agent state without requesting permission', async () => {
		const notifications = notificationCapability('granted');
		const controller = new TerminalFeedbackSettingsController({
			notificationCapability: notifications,
			userPreferences: memoryUserPreferences(),
		});
		const notification = {
			key: 'presentation-1:binding-1:3',
			notification: {
				body: 'Workspace · Tab',
				onActivate: vi.fn<() => void>(),
				tag: 'orchbee-agent:presentation-1',
				title: 'Codex finished',
			},
		};

		expect(controller.observeAgentNotification(notification)).toEqual({
			kind: 'ignored',
			reason: 'turned-off',
		});
		controller.update({ agentNotificationMode: 'hidden' });
		await Promise.resolve();
		expect(controller.observeAgentNotification({ ...notification, key: 'next' })).toEqual({
			kind: 'notify',
			notification: 'shown',
		});
		expect(controller.observeAgentNotification({ ...notification, key: 'next' })).toEqual({
			kind: 'ignored',
			reason: 'duplicate',
		});
		expect(notifications.show).toHaveBeenCalledOnce();
		expect(notifications.show).toHaveBeenCalledWith({ ...notification.notification, silent: true });
		expect(notifications.requestPermission).not.toHaveBeenCalled();
	});

	it('never implicitly asks for Agent notification permission', () => {
		const notifications = notificationCapability('prompt');
		const controller = new TerminalFeedbackSettingsController({
			notificationCapability: notifications,
			userPreferences: memoryUserPreferences({ agentNotificationMode: 'hidden' }),
		});

		expect(
			controller.observeAgentNotification({
				key: 'presentation-1:binding-1:4',
				notification: {
					body: 'Workspace · Tab',
					onActivate: () => undefined,
					tag: 'orchbee-agent:presentation-1',
					title: 'Codex needs attention',
				},
			}),
		).toEqual({ kind: 'notify', notification: 'unavailable' });
		expect(notifications.requestPermission).not.toHaveBeenCalled();
		expect(notifications.show).not.toHaveBeenCalled();
	});
});
