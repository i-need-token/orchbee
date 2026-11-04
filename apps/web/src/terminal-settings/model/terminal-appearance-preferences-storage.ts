import { z } from 'zod';

import {
	DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
	MAXIMUM_TERMINAL_FONT_SIZE,
	MINIMUM_TERMINAL_FONT_SIZE,
	TERMINAL_FONT_WEIGHT_OPTIONS,
	type TerminalAppearancePreferences,
} from './terminal-appearance-preferences';

const HOST_APPEARANCE_STORAGE_KEY = 'orchbee:web:terminal-host-appearance:v1';

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

const terminalFontWeightSchema = z.union(
	TERMINAL_FONT_WEIGHT_OPTIONS.map((weight) => z.literal(weight)),
);
const terminalAppearancePreferencesSchema = z.strictObject({
	applicationTheme: z.enum(['light', 'dark', 'system']),
	backgroundOpacityPercent: z.number().int().min(10).max(100),
	customBackground: hexColorSchema,
	customForeground: hexColorSchema,
	fontFamily: z.string().trim().min(1).max(256),
	fontSize: z.number().int().min(MINIMUM_TERMINAL_FONT_SIZE).max(MAXIMUM_TERMINAL_FONT_SIZE),
	terminalDisplayTheme: z.enum(['host', 'light', 'dark', 'custom']),
	fontLigatures: z.boolean(),
	fontWeight: terminalFontWeightSchema,
	fontWeightBold: terminalFontWeightSchema,
	terminalPadding: z.number().int().min(0).max(32),
});

const hostAppearanceSchema = terminalAppearancePreferencesSchema.omit({
	applicationTheme: true,
	terminalDisplayTheme: true,
	customForeground: true,
	customBackground: true,
});

export function readHostAppearancePreferences(
	storage: TerminalAppearanceStorage,
): TerminalAppearanceStorageReadResult {
	let serialized: string | null;
	try {
		serialized = storage.getItem(HOST_APPEARANCE_STORAGE_KEY);
	} catch {
		return defaultReadResult('read-failed');
	}
	if (serialized === null) return defaultReadResult(null);
	try {
		const parsed = hostAppearanceSchema.safeParse(JSON.parse(serialized));
		return parsed.success
			? { issue: null, preferences: { ...DEFAULT_TERMINAL_APPEARANCE_PREFERENCES, ...parsed.data } }
			: defaultReadResult('invalid-record');
	} catch {
		return defaultReadResult('invalid-record');
	}
}

export function writeHostAppearancePreferences(
	storage: TerminalAppearanceStorage,
	preferences: TerminalAppearancePreferences,
): boolean {
	try {
		const {
			applicationTheme: _applicationTheme,
			terminalDisplayTheme: _terminalDisplayTheme,
			customForeground: _customForeground,
			customBackground: _customBackground,
			...host
		} = preferences;
		storage.setItem(HOST_APPEARANCE_STORAGE_KEY, JSON.stringify(hostAppearanceSchema.parse(host)));
		return true;
	} catch {
		return false;
	}
}

export function normalizeTerminalAppearancePreferences(
	input: unknown,
): TerminalAppearancePreferences | null {
	const result = terminalAppearancePreferencesSchema.safeParse(input);
	return result.success ? Object.freeze(result.data) : null;
}

function defaultReadResult(
	issue: TerminalAppearanceStorageReadResult['issue'],
): TerminalAppearanceStorageReadResult {
	return { issue, preferences: DEFAULT_TERMINAL_APPEARANCE_PREFERENCES };
}
