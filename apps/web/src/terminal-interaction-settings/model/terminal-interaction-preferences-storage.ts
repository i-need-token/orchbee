import { z } from 'zod';
import type { UserPreferencesPatch } from '@orchbee/workbench-client-protocol';

import {
	DEFAULT_TERMINAL_INTERACTION_PREFERENCES,
	type TerminalInteractionPreferences,
} from './terminal-interaction-preferences';

const TERMINAL_INTERACTION_STORAGE_KEY = 'orchbee:web:terminal-interaction:v2';
const LEGACY_TERMINAL_INTERACTION_STORAGE_KEY = 'orchbee:web:terminal-interaction:v1';

export type TerminalInteractionPersistenceIssue = 'invalid-record' | 'read-failed' | 'write-failed';

export interface TerminalInteractionStorage {
	getItem(key: string): string | null;
	setItem(key: string, value: string): void;
}

const terminalInteractionPreferencesSchema = z.strictObject({
	copyOnSelect: z.boolean(),
	trimTrailingBlankRowsForAgents: z.boolean(),
});
const legacyTerminalInteractionPreferencesSchema = z.strictObject({ copyOnSelect: z.boolean() });

export function readLegacyInteractionPreferencePatch(
	storage: TerminalInteractionStorage,
): Pick<UserPreferencesPatch, 'copyOnSelect' | 'trimTrailingBlankRowsForAgents'> | null {
	try {
		const current = storage.getItem(TERMINAL_INTERACTION_STORAGE_KEY);
		const legacy = storage.getItem(LEGACY_TERMINAL_INTERACTION_STORAGE_KEY);
		if (current === null && legacy === null) return {};
		const restored = readTerminalInteractionPreferences(storage);
		if (restored.issue !== null) return null;
		return current === null
			? { copyOnSelect: restored.preferences.copyOnSelect }
			: restored.preferences;
	} catch {
		return null;
	}
}

export function normalizeTerminalInteractionPreferences(
	input: unknown,
): TerminalInteractionPreferences | null {
	const result = terminalInteractionPreferencesSchema.safeParse(input);
	return result.success ? Object.freeze(result.data) : null;
}

export function readTerminalInteractionPreferences(storage: TerminalInteractionStorage): {
	readonly issue: Exclude<TerminalInteractionPersistenceIssue, 'write-failed'> | null;
	readonly preferences: TerminalInteractionPreferences;
} {
	let serialized: string | null;
	try {
		serialized = storage.getItem(TERMINAL_INTERACTION_STORAGE_KEY);
	} catch {
		return { issue: 'read-failed', preferences: DEFAULT_TERMINAL_INTERACTION_PREFERENCES };
	}
	if (serialized !== null) {
		try {
			const result = terminalInteractionPreferencesSchema.safeParse(JSON.parse(serialized));
			return result.success
				? { issue: null, preferences: Object.freeze(result.data) }
				: { issue: 'invalid-record', preferences: DEFAULT_TERMINAL_INTERACTION_PREFERENCES };
		} catch {
			return { issue: 'invalid-record', preferences: DEFAULT_TERMINAL_INTERACTION_PREFERENCES };
		}
	}

	try {
		serialized = storage.getItem(LEGACY_TERMINAL_INTERACTION_STORAGE_KEY);
	} catch {
		return { issue: 'read-failed', preferences: DEFAULT_TERMINAL_INTERACTION_PREFERENCES };
	}
	if (serialized === null) {
		return { issue: null, preferences: DEFAULT_TERMINAL_INTERACTION_PREFERENCES };
	}
	try {
		const result = legacyTerminalInteractionPreferencesSchema.safeParse(JSON.parse(serialized));
		return result.success
			? {
					issue: null,
					preferences: Object.freeze({
						...result.data,
						trimTrailingBlankRowsForAgents: true,
					}),
				}
			: { issue: 'invalid-record', preferences: DEFAULT_TERMINAL_INTERACTION_PREFERENCES };
	} catch {
		return { issue: 'invalid-record', preferences: DEFAULT_TERMINAL_INTERACTION_PREFERENCES };
	}
}

export function writeTerminalInteractionPreferences(
	storage: TerminalInteractionStorage,
	preferences: TerminalInteractionPreferences,
): boolean {
	try {
		storage.setItem(TERMINAL_INTERACTION_STORAGE_KEY, JSON.stringify(preferences));
		return true;
	} catch {
		return false;
	}
}
