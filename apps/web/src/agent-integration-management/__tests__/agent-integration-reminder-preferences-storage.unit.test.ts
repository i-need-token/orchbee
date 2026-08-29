import { describe, expect, it, vi } from 'vitest';

import {
	readAgentIntegrationReminderPreferences,
	writeAgentIntegrationReminderPreferences,
	type AgentIntegrationReminderStorage,
} from '../agent-integration-reminder-preferences-storage';

const EXPECTED_STORAGE_KEY = 'orchbee:web:agent-integration-reminders:v1';
const DEFAULT_PREFERENCES = { dismissedIntegrationIds: [] } as const;

describe('Agent integration reminder preference storage', () => {
	it('restores a bounded valid record in canonical integration order', () => {
		const storage = {
			getItem: vi.fn<() => string>(() =>
				JSON.stringify({ dismissedIntegrationIds: ['omp', 'codex'] }),
			),
			setItem: vi.fn<(key: string, value: string) => void>(),
		};

		expect(readAgentIntegrationReminderPreferences(storage)).toEqual({
			issue: null,
			preferences: { dismissedIntegrationIds: ['codex', 'omp'] },
		});
		expect(storage.getItem).toHaveBeenCalledWith(EXPECTED_STORAGE_KEY);
	});

	it('falls back for missing, malformed, duplicate, and unavailable records', () => {
		expect(
			readAgentIntegrationReminderPreferences({
				getItem: () => null,
				setItem: () => undefined,
			}),
		).toEqual({ issue: null, preferences: DEFAULT_PREFERENCES });
		for (const serialized of [
			'{',
			JSON.stringify({ dismissedIntegrationIds: ['codex', 'codex'] }),
			JSON.stringify({ dismissedIntegrationIds: ['unknown-agent'] }),
			JSON.stringify({ dismissedIntegrationIds: [], extra: true }),
		]) {
			expect(
				readAgentIntegrationReminderPreferences({
					getItem: () => serialized,
					setItem: () => undefined,
				}),
			).toEqual({
				issue: 'invalid-record',
				preferences: DEFAULT_PREFERENCES,
			});
		}
		expect(
			readAgentIntegrationReminderPreferences({
				getItem: () => {
					throw new Error('unavailable');
				},
				setItem: () => undefined,
			}),
		).toEqual({
			issue: 'read-failed',
			preferences: DEFAULT_PREFERENCES,
		});
	});

	it('writes only the dismissed IDs and contains write failures', () => {
		const storage = {
			getItem: vi.fn<() => null>(() => null),
			setItem: vi.fn<(key: string, value: string) => void>(),
		};
		expect(
			writeAgentIntegrationReminderPreferences(storage, {
				dismissedIntegrationIds: ['codex'],
			}),
		).toBe(true);
		expect(storage.setItem).toHaveBeenCalledWith(
			EXPECTED_STORAGE_KEY,
			JSON.stringify({ dismissedIntegrationIds: ['codex'] }),
		);
		const unavailableStorage: AgentIntegrationReminderStorage = {
			getItem: () => null,
			setItem: () => {
				throw new Error('quota');
			},
		};
		expect(
			writeAgentIntegrationReminderPreferences(unavailableStorage, {
				dismissedIntegrationIds: ['codex'],
			}),
		).toBe(false);
	});
});
