import { isTerminalOptionAsAltMode } from '@orchbee/terminal';
import { z } from 'zod';

import {
	DEFAULT_MACOS_TERMINAL_INPUT_PREFERENCES,
	type MacosTerminalInputPreferences,
} from './macos-terminal-input-preferences';

const MACOS_TERMINAL_INPUT_STORAGE_KEY = 'orchbee:web:macos-terminal-input:v1';

export type MacosTerminalInputPersistenceIssue = 'invalid-record' | 'read-failed' | 'write-failed';

export interface MacosTerminalInputStorage {
	getItem(key: string): string | null;
	setItem(key: string, value: string): void;
}

const macosTerminalInputPreferencesSchema = z.strictObject({
	optionAsAlt: z.unknown().refine(isTerminalOptionAsAltMode),
});

export function normalizeMacosTerminalInputPreferences(
	input: unknown,
): MacosTerminalInputPreferences | null {
	const result = macosTerminalInputPreferencesSchema.safeParse(input);
	return result.success ? Object.freeze(result.data) : null;
}

export function readMacosTerminalInputPreferences(storage: MacosTerminalInputStorage): {
	readonly issue: Exclude<MacosTerminalInputPersistenceIssue, 'write-failed'> | null;
	readonly preferences: MacosTerminalInputPreferences;
} {
	let serialized: string | null;
	try {
		serialized = storage.getItem(MACOS_TERMINAL_INPUT_STORAGE_KEY);
	} catch {
		return { issue: 'read-failed', preferences: DEFAULT_MACOS_TERMINAL_INPUT_PREFERENCES };
	}
	if (serialized === null) {
		return { issue: null, preferences: DEFAULT_MACOS_TERMINAL_INPUT_PREFERENCES };
	}
	try {
		const result = macosTerminalInputPreferencesSchema.safeParse(JSON.parse(serialized));
		return !result.success
			? { issue: 'invalid-record', preferences: DEFAULT_MACOS_TERMINAL_INPUT_PREFERENCES }
			: { issue: null, preferences: Object.freeze(result.data) };
	} catch {
		return { issue: 'invalid-record', preferences: DEFAULT_MACOS_TERMINAL_INPUT_PREFERENCES };
	}
}

export function writeMacosTerminalInputPreferences(
	storage: MacosTerminalInputStorage,
	preferences: MacosTerminalInputPreferences,
): boolean {
	try {
		storage.setItem(MACOS_TERMINAL_INPUT_STORAGE_KEY, JSON.stringify(preferences));
		return true;
	} catch {
		return false;
	}
}
