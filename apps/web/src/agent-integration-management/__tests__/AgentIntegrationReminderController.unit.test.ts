import { describe, expect, it, vi } from 'vitest';

import { AgentIntegrationReminderController } from '../AgentIntegrationReminderController';

describe('AgentIntegrationReminderController', () => {
	it('offers at most one non-dismissed integration per application session', () => {
		const controller = new AgentIntegrationReminderController({
			getItem: () => null,
			setItem: () => undefined,
		});
		const listener = vi.fn<() => void>();
		controller.subscribe(listener);

		expect(controller.offer('codex')).toBe(true);
		expect(controller.getSnapshot().activeIntegrationId).toBe('codex');
		controller.hide('codex');
		expect(controller.getSnapshot().activeIntegrationId).toBeNull();
		expect(controller.offer('grok')).toBe(false);
		expect(listener).toHaveBeenCalledTimes(2);

		controller.dispose();
	});

	it('persists dismissal and restores it only after an explicit successful caller action', () => {
		let serialized: string | null = null;
		const storage = {
			getItem: () => serialized,
			setItem: (_key: string, value: string) => {
				serialized = value;
			},
		};
		const controller = new AgentIntegrationReminderController(storage);
		controller.offer('codex');

		expect(controller.dismiss('codex')).toEqual({ kind: 'applied-and-saved' });
		expect(controller.getSnapshot()).toMatchObject({
			activeIntegrationId: null,
			dismissedIntegrationIds: ['codex'],
			persistenceIssue: null,
		});
		expect(new AgentIntegrationReminderController(storage).offer('codex')).toBe(false);

		expect(controller.restore('codex')).toEqual({ kind: 'applied-and-saved' });
		expect(controller.getSnapshot().dismissedIntegrationIds).toEqual([]);
		expect(JSON.parse(serialized ?? '')).toEqual({ dismissedIntegrationIds: [] });
	});

	it('keeps the in-memory preference when persistence is unavailable', () => {
		const controller = new AgentIntegrationReminderController({
			getItem: () => null,
			setItem: () => {
				throw new Error('quota');
			},
		});

		expect(controller.dismiss('pi')).toEqual({ kind: 'applied-session-only' });
		expect(controller.getSnapshot()).toMatchObject({
			dismissedIntegrationIds: ['pi'],
			persistenceIssue: 'write-failed',
		});
	});
});
