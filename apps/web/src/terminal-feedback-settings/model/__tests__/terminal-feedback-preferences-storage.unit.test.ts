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
				completionReminderMinimumSeconds: 12,
				completionReminderMode: 'unfocused',
			}),
		).toEqual({
			completionReminderMinimumSeconds: 12,
			completionReminderMode: 'unfocused',
		});
		expect(
			normalizeTerminalFeedbackPreferences({
				completionReminderMinimumSeconds: 0,
				completionReminderMode: 'always',
			}),
		).toBeNull();
		expect(
			normalizeTerminalFeedbackPreferences({
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
					completionReminderMinimumSeconds: 12,
					completionReminderMode: 'always',
				},
			),
		).toBe(true);
		expect(setItem).toHaveBeenCalledWith(
			'orchbee:web:terminal-feedback:v1',
			'{"completionReminderMinimumSeconds":12,"completionReminderMode":"always"}',
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
});
