import { describe, expect, it, vi } from 'vitest';

import { DEFAULT_TERMINAL_FEEDBACK_PREFERENCES } from '../terminal-feedback-preferences';
import {
	normalizeTerminalFeedbackPreferences,
	readTerminalFeedbackPreferences,
	writeTerminalFeedbackPreferences,
} from '../terminal-feedback-preferences-storage';

describe('terminal feedback preference storage', () => {
	it('accepts only the strict supported record', () => {
		expect(
			normalizeTerminalFeedbackPreferences({
				agentNotificationMode: 'hidden',
				soundEnabled: false,
				completionReminderMinimumSeconds: 12,
				completionReminderMode: 'unfocused',
			}),
		).toEqual({
			agentNotificationMode: 'hidden',
			soundEnabled: false,
			completionReminderMinimumSeconds: 12,
			completionReminderMode: 'unfocused',
		});
		expect(
			normalizeTerminalFeedbackPreferences({
				agentNotificationMode: 'never',
				completionReminderMinimumSeconds: 0,
				completionReminderMode: 'always',
			}),
		).toBeNull();
		expect(
			normalizeTerminalFeedbackPreferences({
				agentNotificationMode: 'never',
				completionReminderMinimumSeconds: 5,
				completionReminderMode: 'never',
				extra: true,
			}),
		).toBeNull();
	});

	it('distinguishes missing, invalid, and unreadable records', () => {
		expect(
			readTerminalFeedbackPreferences({ getItem: () => null, setItem: () => undefined }),
		).toEqual({
			issue: null,
			preferences: DEFAULT_TERMINAL_FEEDBACK_PREFERENCES,
		});
		expect(
			readTerminalFeedbackPreferences({
				getItem: () => '{"completionReminderMode":"always"}',
				setItem: () => undefined,
			}),
		).toEqual({
			issue: 'invalid-record',
			preferences: DEFAULT_TERMINAL_FEEDBACK_PREFERENCES,
		});
		expect(
			readTerminalFeedbackPreferences({
				getItem: () => {
					throw new Error('unavailable');
				},
				setItem: () => undefined,
			}),
		).toEqual({
			issue: 'read-failed',
			preferences: DEFAULT_TERMINAL_FEEDBACK_PREFERENCES,
		});
	});

	it('writes the versioned record and reports storage failures', () => {
		const setItem = vi.fn<(key: string, value: string) => void>();
		expect(
			writeTerminalFeedbackPreferences(
				{ getItem: () => null, setItem },
				{
					agentNotificationMode: 'hidden',
					completionReminderMinimumSeconds: 12,
					completionReminderMode: 'always',
					soundEnabled: false,
				},
			),
		).toBe(true);
		expect(setItem).toHaveBeenCalledWith(
			'orchbee:web:terminal-feedback:v3',
			'{"agentNotificationMode":"hidden","completionReminderMinimumSeconds":12,"completionReminderMode":"always","soundEnabled":false}',
		);

		expect(
			writeTerminalFeedbackPreferences(
				{
					getItem: () => null,
					setItem: () => {
						throw new Error('unavailable');
					},
				},
				DEFAULT_TERMINAL_FEEDBACK_PREFERENCES,
			),
		).toBe(false);
	});

	it('migrates the legacy record with Agent notifications off', () => {
		const legacy = JSON.stringify({
			completionReminderMinimumSeconds: 12,
			completionReminderMode: 'unfocused',
		});
		expect(
			readTerminalFeedbackPreferences({
				getItem: (key) => (key === 'orchbee:web:terminal-feedback:v1' ? legacy : null),
				setItem: () => undefined,
			}),
		).toEqual({
			issue: null,
			preferences: {
				agentNotificationMode: 'never',
				soundEnabled: false,
				completionReminderMinimumSeconds: 12,
				completionReminderMode: 'unfocused',
			},
		});
	});
});
