import { z } from 'zod';
import type { UserPreferencesPatch } from '@orchbee/workbench-client-protocol';

import {
	DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
	MAXIMUM_TERMINAL_FONT_SIZE,
	MINIMUM_TERMINAL_FONT_SIZE,
	TERMINAL_FONT_WEIGHT_OPTIONS,
	type TerminalAppearancePreferences,
} from './terminal-appearance-preferences';

const TERMINAL_APPEARANCE_STORAGE_KEY = 'orchbee:web:terminal-appearance:v2';
const CURRENT_TERMINAL_APPEARANCE_STORAGE_KEY = 'orchbee:web:terminal-appearance:v3';
const HOST_APPEARANCE_STORAGE_KEY = 'orchbee:web:terminal-host-appearance:v1';
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
	fontSize: z.number().int().min(MINIMUM_TERMINAL_FONT_SIZE).max(MAXIMUM_TERMINAL_FONT_SIZE),
	terminalDisplayTheme: z.enum(['host', 'light', 'dark', 'custom']),
});

const terminalFontWeightSchema = z.union(
	TERMINAL_FONT_WEIGHT_OPTIONS.map((weight) => z.literal(weight)),
);

const previousTerminalAppearancePreferencesSchema =
	legacyTerminalAppearancePreferencesSchema.extend({
		fontLigatures: z.boolean(),
		fontWeight: terminalFontWeightSchema,
		fontWeightBold: terminalFontWeightSchema,
		terminalPadding: z.number().int().min(0).max(32),
	});

const terminalAppearancePreferencesSchema = previousTerminalAppearancePreferencesSchema.extend({
	applicationTheme: z.enum(['light', 'dark', 'system']),
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
	try {
		const serialized = storage.getItem(HOST_APPEARANCE_STORAGE_KEY);
		if (serialized === null) return readTerminalAppearancePreferences(storage);
		const parsed = hostAppearanceSchema.safeParse(JSON.parse(serialized));
		return parsed.success
			? { issue: null, preferences: { ...DEFAULT_TERMINAL_APPEARANCE_PREFERENCES, ...parsed.data } }
			: defaultReadResult('invalid-record');
	} catch {
		return defaultReadResult('read-failed');
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

export function readLegacyAppearancePreferencePatch(
	storage: TerminalAppearanceStorage,
): Pick<
	UserPreferencesPatch,
	'applicationTheme' | 'terminalDisplayTheme' | 'customForeground' | 'customBackground'
> | null {
	try {
		if (
			storage.getItem(CURRENT_TERMINAL_APPEARANCE_STORAGE_KEY) === null &&
			storage.getItem(TERMINAL_APPEARANCE_STORAGE_KEY) === null &&
			storage.getItem(LEGACY_TERMINAL_APPEARANCE_STORAGE_KEY) === null
		)
			return {};
		const restored = readTerminalAppearancePreferences(storage);
		if (restored.issue !== null) return null;
		const { applicationTheme, terminalDisplayTheme, customForeground, customBackground } =
			restored.preferences;
		return { applicationTheme, terminalDisplayTheme, customForeground, customBackground };
	} catch {
		return null;
	}
}

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
	let previous = false;
	try {
		serialized = storage.getItem(CURRENT_TERMINAL_APPEARANCE_STORAGE_KEY);
		if (serialized === null) {
			serialized = storage.getItem(TERMINAL_APPEARANCE_STORAGE_KEY);
			previous = true;
		}
	} catch {
		return defaultReadResult('read-failed');
	}
	if (serialized === null) return readLegacyTerminalAppearancePreferences(storage);
	try {
		const storedAppearanceSchema = previous
			? previousTerminalAppearancePreferencesSchema
			: terminalAppearancePreferencesSchema;
		const result = storedAppearanceSchema.safeParse(JSON.parse(serialized));
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
		storage.setItem(CURRENT_TERMINAL_APPEARANCE_STORAGE_KEY, JSON.stringify(preferences));
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
