import { z } from 'zod';
import type { UserPreferencesPatch } from '@orchbee/workbench-client-protocol';

import {
	DEFAULT_TERMINAL_FEEDBACK_PREFERENCES,
	MAXIMUM_COMPLETION_REMINDER_SECONDS,
	MINIMUM_COMPLETION_REMINDER_SECONDS,
	type TerminalFeedbackPreferences,
} from './terminal-feedback-preferences';

const TERMINAL_FEEDBACK_STORAGE_KEY = 'orchbee:web:terminal-feedback:v2';
const CURRENT_TERMINAL_FEEDBACK_STORAGE_KEY = 'orchbee:web:terminal-feedback:v3';
const LEGACY_TERMINAL_FEEDBACK_STORAGE_KEY = 'orchbee:web:terminal-feedback:v1';

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

const legacyTerminalFeedbackPreferencesSchema = terminalFeedbackPreferencesSchema.omit({
	agentNotificationMode: true,
	soundEnabled: true,
});
const previousTerminalFeedbackPreferencesSchema = terminalFeedbackPreferencesSchema.omit({
	soundEnabled: true,
});

export function readLegacyFeedbackPreferencePatch(
	storage: TerminalFeedbackStorage,
): Pick<
	UserPreferencesPatch,
	| 'agentNotificationMode'
	| 'completionReminderMode'
	| 'completionReminderMinimumSeconds'
	| 'soundEnabled'
> | null {
	try {
		const current = storage.getItem(CURRENT_TERMINAL_FEEDBACK_STORAGE_KEY);
		const previous = storage.getItem(TERMINAL_FEEDBACK_STORAGE_KEY);
		const legacy = storage.getItem(LEGACY_TERMINAL_FEEDBACK_STORAGE_KEY);
		if (current === null && previous === null && legacy === null) return {};
		const restored = readTerminalFeedbackPreferences(storage);
		if (restored.issue !== null) return null;
		const {
			agentNotificationMode,
			completionReminderMode,
			completionReminderMinimumSeconds,
			soundEnabled,
		} = restored.preferences;
		return {
			completionReminderMode,
			completionReminderMinimumSeconds,
			...(current !== null || previous !== null ? { agentNotificationMode } : {}),
			...(current !== null ? { soundEnabled } : {}),
		};
	} catch {
		return null;
	}
}

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
	let legacy: string | null = null;
	let previous = false;
	try {
		serialized = storage.getItem(CURRENT_TERMINAL_FEEDBACK_STORAGE_KEY);
		if (serialized === null) {
			serialized = storage.getItem(TERMINAL_FEEDBACK_STORAGE_KEY);
			previous = true;
		}
		if (serialized === null) legacy = storage.getItem(LEGACY_TERMINAL_FEEDBACK_STORAGE_KEY);
	} catch {
		return { issue: 'read-failed', preferences: DEFAULT_TERMINAL_FEEDBACK_PREFERENCES };
	}
	if (serialized !== null) {
		try {
			const storedFeedbackSchema = previous
				? previousTerminalFeedbackPreferencesSchema
				: terminalFeedbackPreferencesSchema;
			const result = storedFeedbackSchema.safeParse(JSON.parse(serialized));
			return result.success
				? { issue: null, preferences: Object.freeze({ soundEnabled: false, ...result.data }) }
				: { issue: 'invalid-record', preferences: DEFAULT_TERMINAL_FEEDBACK_PREFERENCES };
		} catch {
			return { issue: 'invalid-record', preferences: DEFAULT_TERMINAL_FEEDBACK_PREFERENCES };
		}
	}
	if (legacy === null) return { issue: null, preferences: DEFAULT_TERMINAL_FEEDBACK_PREFERENCES };
	try {
		const result = legacyTerminalFeedbackPreferencesSchema.safeParse(JSON.parse(legacy));
		return result.success
			? {
					issue: null,
					preferences: Object.freeze({
						soundEnabled: false,
						agentNotificationMode: 'never' as const,
						...result.data,
					}),
				}
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
		storage.setItem(CURRENT_TERMINAL_FEEDBACK_STORAGE_KEY, JSON.stringify(preferences));
		return true;
	} catch {
		return false;
	}
}
