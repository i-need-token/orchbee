import { z } from 'zod';

import {
	DEFAULT_TERMINAL_FEEDBACK_PREFERENCES,
	MAXIMUM_COMPLETION_REMINDER_SECONDS,
	MINIMUM_COMPLETION_REMINDER_SECONDS,
	type TerminalFeedbackPreferences,
} from './terminal-feedback-preferences';

const TERMINAL_FEEDBACK_STORAGE_KEY = 'orchbee:web:terminal-feedback:v1';

export type TerminalFeedbackPersistenceIssue = 'invalid-record' | 'read-failed' | 'write-failed';

export interface TerminalFeedbackStorage {
	getItem(key: string): string | null;
	setItem(key: string, value: string): void;
}

const terminalFeedbackPreferencesSchema = z.strictObject({
	soundEnabled: z.boolean(),
	agentNotificationMode: z.enum(['never', 'hidden']),
	completionReminderMinimumSeconds: z
		.number()
		.int()
		.min(MINIMUM_COMPLETION_REMINDER_SECONDS)
		.max(MAXIMUM_COMPLETION_REMINDER_SECONDS),
	completionReminderMode: z.enum(['never', 'unfocused', 'always']),
});

export function normalizeTerminalFeedbackPreferences(
	input: unknown,
): TerminalFeedbackPreferences | null {
	const result = terminalFeedbackPreferencesSchema.safeParse(input);
	return result.success ? Object.freeze(result.data) : null;
}

export function readTerminalFeedbackPreferences(storage: TerminalFeedbackStorage): {
	readonly issue: Exclude<TerminalFeedbackPersistenceIssue, 'write-failed'> | null;
	readonly preferences: TerminalFeedbackPreferences;
} {
	let serialized: string | null;
	try {
		serialized = storage.getItem(TERMINAL_FEEDBACK_STORAGE_KEY);
	} catch {
		return { issue: 'read-failed', preferences: DEFAULT_TERMINAL_FEEDBACK_PREFERENCES };
	}
	if (serialized === null)
		return { issue: null, preferences: DEFAULT_TERMINAL_FEEDBACK_PREFERENCES };
	try {
		const result = terminalFeedbackPreferencesSchema.safeParse(JSON.parse(serialized));
		return result.success
			? { issue: null, preferences: Object.freeze(result.data) }
			: { issue: 'invalid-record', preferences: DEFAULT_TERMINAL_FEEDBACK_PREFERENCES };
	} catch {
		return { issue: 'invalid-record', preferences: DEFAULT_TERMINAL_FEEDBACK_PREFERENCES };
	}
}

export function writeTerminalFeedbackPreferences(
	storage: TerminalFeedbackStorage,
	preferences: TerminalFeedbackPreferences,
): boolean {
	try {
		storage.setItem(TERMINAL_FEEDBACK_STORAGE_KEY, JSON.stringify(preferences));
		return true;
	} catch {
		return false;
	}
}
