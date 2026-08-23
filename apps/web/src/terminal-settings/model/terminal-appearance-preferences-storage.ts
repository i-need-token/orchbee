import { z } from 'zod';

import {
	DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
	TERMINAL_FONT_WEIGHT_OPTIONS,
	type TerminalAppearancePreferences,
} from './terminal-appearance-preferences';

const TERMINAL_APPEARANCE_STORAGE_KEY = 'orchbee:web:terminal-appearance:v2';
const LEGACY_TERMINAL_APPEARANCE_STORAGE_KEY = 'orchbee:web:terminal-appearance:v1';

export type TerminalAppearancePersistenceIssue = 'invalid-record' | 'read-failed' | 'write-failed';

export interface TerminalAppearanceStorage {
	getItem(key: string): string | null;
	setItem(key: string, value: string): void;
}

export interface TerminalAppearanceStorageReadResult {
	readonly issue: Exclude<TerminalAppearancePersistenceIssue, 'write-failed'> | null;
	readonly preferences: TerminalAppearancePreferences;
}

const hexColorSchema = z
	.string()
	.regex(/^#[\da-fA-F]{6}$/)
	.transform((value) => `#${value.slice(1).toLowerCase()}` as const);

const legacyTerminalAppearancePreferencesSchema = z.strictObject({
	applicationTheme: z.enum(['light', 'dark']),
	backgroundOpacityPercent: z.number().int().min(10).max(100),
	customBackground: hexColorSchema,
	customForeground: hexColorSchema,
	fontFamily: z.string().trim().min(1).max(256),
	fontSize: z.number().int().min(8).max(48),
	terminalDisplayTheme: z.enum(['host', 'light', 'dark', 'custom']),
});

const terminalFontWeightSchema = z.union(
	TERMINAL_FONT_WEIGHT_OPTIONS.map((weight) => z.literal(weight)),
);

const terminalAppearancePreferencesSchema = legacyTerminalAppearancePreferencesSchema.extend({
	fontLigatures: z.boolean(),
	fontWeight: terminalFontWeightSchema,
	fontWeightBold: terminalFontWeightSchema,
	terminalPadding: z.number().int().min(0).max(32),
});

export function normalizeTerminalAppearancePreferences(
	input: unknown,
): TerminalAppearancePreferences | null {
	const result = terminalAppearancePreferencesSchema.safeParse(input);
	return result.success ? Object.freeze(result.data) : null;
}

export function readTerminalAppearancePreferences(
	storage: TerminalAppearanceStorage,
): TerminalAppearanceStorageReadResult {
	let serialized: string | null;
	try {
		serialized = storage.getItem(TERMINAL_APPEARANCE_STORAGE_KEY);
	} catch {
		return defaultReadResult('read-failed');
	}
	if (serialized === null) return readLegacyTerminalAppearancePreferences(storage);
	try {
		const result = terminalAppearancePreferencesSchema.safeParse(JSON.parse(serialized));
		return result.success
			? { issue: null, preferences: Object.freeze(result.data) }
			: defaultReadResult('invalid-record');
	} catch {
		return defaultReadResult('invalid-record');
	}
}

function readLegacyTerminalAppearancePreferences(
	storage: TerminalAppearanceStorage,
): TerminalAppearanceStorageReadResult {
	let serialized: string | null;
	try {
		serialized = storage.getItem(LEGACY_TERMINAL_APPEARANCE_STORAGE_KEY);
	} catch {
		return defaultReadResult('read-failed');
	}
	if (serialized === null) return defaultReadResult(null);
	try {
		const result = legacyTerminalAppearancePreferencesSchema.safeParse(JSON.parse(serialized));
		if (!result.success) return defaultReadResult('invalid-record');
		return {
			issue: null,
			preferences: Object.freeze({
				...result.data,
				fontLigatures: DEFAULT_TERMINAL_APPEARANCE_PREFERENCES.fontLigatures,
				fontWeight: DEFAULT_TERMINAL_APPEARANCE_PREFERENCES.fontWeight,
				fontWeightBold: DEFAULT_TERMINAL_APPEARANCE_PREFERENCES.fontWeightBold,
				terminalPadding: DEFAULT_TERMINAL_APPEARANCE_PREFERENCES.terminalPadding,
			}),
		};
	} catch {
		return defaultReadResult('invalid-record');
	}
}

export function writeTerminalAppearancePreferences(
	storage: TerminalAppearanceStorage,
	preferences: TerminalAppearancePreferences,
): boolean {
	try {
		storage.setItem(TERMINAL_APPEARANCE_STORAGE_KEY, JSON.stringify(preferences));
		return true;
	} catch {
		return false;
	}
}

function defaultReadResult(
	issue: TerminalAppearanceStorageReadResult['issue'],
): TerminalAppearanceStorageReadResult {
	return { issue, preferences: DEFAULT_TERMINAL_APPEARANCE_PREFERENCES };
}
