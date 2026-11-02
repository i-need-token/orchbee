import { describe, expect, it, vi } from 'vitest';
import {
	AgentEventIdSchema,
	AgentRequestIdSchema,
	type AgentSemanticEvent,
	AgentSessionIdSchema,
	AgentTurnIdSchema,
} from '@orchbee/agent';

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

function storage(initial: string | null = null): {
	readonly values: Map<string, string>;
	getItem(key: string): string | null;
	setItem(key: string, value: string): void;
} {
	const values = new Map<string, string>();
	if (initial !== null) values.set('orchbee:web:terminal-feedback:v1', initial);
	return {
		values,
		getItem: (key) => values.get(key) ?? null,
		setItem: (key, value) => values.set(key, value),
	};
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
	it('coalesces storage-reentrant updates without losing complete session preferences', () => {
		let stored = '';
		let writes = 0;
		let nestedResult: unknown;
		const controller = new TerminalFeedbackSettingsController({
			notificationCapability: notificationCapability(),
			storage: {
				getItem: () => null,
				setItem: (_key, value) => {
					writes += 1;
					if (writes === 1) {
						nestedResult = controller.update({ completionReminderMinimumSeconds: 15 });
						controller.update({ agentNotificationMode: 'hidden' });
					}
					stored = value;
				},
			},
		});

		expect(controller.update({ completionReminderMode: 'always' })).toEqual({
			kind: 'applied-and-saved',
		});
		expect(controller.getSnapshot().preferences).toEqual({
			agentNotificationMode: 'hidden',
			completionReminderMinimumSeconds: 15,
			completionReminderMode: 'always',
			soundEnabled: false,
		});
		expect(nestedResult).toEqual({ kind: 'applied-session-only' });
		expect(JSON.parse(stored)).toMatchObject(controller.getSnapshot().preferences);
		expect(writes).toBe(2);
	});

	it('drops a queued preference write when storage retires the controller', () => {
		let writes = 0;
		let retiredSnapshot: unknown;
		const controller = new TerminalFeedbackSettingsController({
			notificationCapability: notificationCapability(),
			storage: {
				getItem: () => null,
				setItem: () => {
					writes += 1;
					if (writes > 1) return;
					controller.update({ completionReminderMinimumSeconds: 15 });
					controller.dispose();
					retiredSnapshot = controller.getSnapshot();
				},
			},
		});
		const listener = vi.fn<() => void>();
		controller.subscribe(listener);

		expect(controller.update({ completionReminderMode: 'always' })).toEqual({ kind: 'disposed' });
		expect(writes).toBe(1);
		expect(controller.getSnapshot()).toBe(retiredSnapshot);
		expect(listener).not.toHaveBeenCalled();
	});

	it('starts off and never asks for notification permission from terminal output', () => {
		const notifications = notificationCapability('prompt');
		const controller = new TerminalFeedbackSettingsController({
			clock: () => NOW,
			notificationCapability: notifications,
			storage: storage(),
		});

		expect(controller.getSnapshot()).toEqual({
			notificationPermission: 'prompt',
			persistenceIssue: null,
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

	it('deduplicates completion sequences and rate limits only the system effect', () => {
		const notifications = notificationCapability();
		const controller = new TerminalFeedbackSettingsController({
			clock: () => NOW,
			notificationCapability: notifications,
			storage: storage(),
		});
		expect(controller.update({ completionReminderMode: 'always' })).toEqual({
			kind: 'applied-and-saved',
		});
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
			storage: storage(),
		});
		controller.update({ completionReminderMode: 'unfocused' });
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
			storage: storage(),
		});
		const request = controller.requestNotificationPermission();
		expect(notifications.requestPermission).toHaveBeenCalledOnce();
		controller.dispose();
		resolvePermission?.('granted');
		expect(await request).toBe('prompt');
		expect(controller.getSnapshot().notificationPermission).toBe('prompt');
	});

	it('notifies once for a trusted hidden Agent event without requesting permission', () => {
		const notifications = notificationCapability('granted');
		const controller = new TerminalFeedbackSettingsController({
			notificationCapability: notifications,
			storage: storage(),
		});
		controller.update({ agentNotificationMode: 'hidden' });
		const event: AgentSemanticEvent = {
			eventId: AgentEventIdSchema.parse('agent-event-1'),
			request: {
				actionPath: 'navigate-only',
				kind: 'question',
				requestId: AgentRequestIdSchema.parse('agent-request-1'),
				turnId: null,
			},
			sessionId: AgentSessionIdSchema.parse('agent-session-1'),
			type: 'attention-requested',
		};
		const onActivate = vi.fn<() => void>();
		const observation = {
			event,
			notification: {
				body: 'safe body',
				onActivate,
				tag: 'agent:event-1',
				title: 'safe title',
			},
			visible: false,
		};

		expect(controller.observeAgentEvent(observation)).toEqual({
			kind: 'notify',
			notification: 'shown',
		});
		expect(controller.observeAgentEvent(observation)).toEqual({
			kind: 'ignored',
			reason: 'duplicate',
		});
		expect(notifications.show).toHaveBeenCalledOnce();
		expect(notifications.show).toHaveBeenCalledWith({ ...observation.notification, silent: true });
		expect(notifications.requestPermission).not.toHaveBeenCalled();
	});

	it('suppresses visible Agent events and never implicitly asks for permission', () => {
		const notifications = notificationCapability('prompt');
		const controller = new TerminalFeedbackSettingsController({
			notificationCapability: notifications,
			storage: storage(),
		});
		controller.update({ agentNotificationMode: 'hidden' });
		const event: AgentSemanticEvent = {
			eventId: AgentEventIdSchema.parse('agent-event-visible'),
			outcome: 'succeeded',
			sessionId: AgentSessionIdSchema.parse('agent-session-visible'),
			turnId: AgentTurnIdSchema.parse('agent-turn-visible'),
			type: 'turn-outcome',
		};
		const notification = {
			body: 'safe',
			onActivate: () => undefined,
			tag: 'agent:visible',
			title: 'completed',
		};

		expect(controller.observeAgentEvent({ event, notification, visible: true })).toEqual({
			kind: 'ignored',
			reason: 'visible',
		});
		expect(notifications.requestPermission).not.toHaveBeenCalled();
		expect(notifications.show).not.toHaveBeenCalled();
	});

	it('rejects invalid saved data and keeps valid updates session-only after a write failure', () => {
		const brokenStorage = storage('{"completionReminderMode":"always","extra":true}');
		brokenStorage.setItem = () => {
			throw new Error('unavailable');
		};
		const controller = new TerminalFeedbackSettingsController({
			notificationCapability: notificationCapability('unsupported'),
			storage: brokenStorage,
		});
		expect(controller.getSnapshot().persistenceIssue).toBe('invalid-record');
		expect(controller.update({ completionReminderMode: 'always' })).toEqual({
			kind: 'applied-session-only',
		});
		expect(controller.getSnapshot().persistenceIssue).toBe('write-failed');
		expect(controller.update({ completionReminderMinimumSeconds: 0 })).toEqual({
			kind: 'invalid',
		});
	});
});
