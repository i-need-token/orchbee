import { describe, expect, it, vi } from 'vitest';

import { DEFAULT_TERMINAL_APPEARANCE_PREFERENCES } from '../terminal-appearance-preferences';
import {
	normalizeTerminalAppearancePreferences,
	readTerminalAppearancePreferences,
	writeTerminalAppearancePreferences,
	type TerminalAppearanceStorage,
} from '../terminal-appearance-preferences-storage';

const EXPECTED_STORAGE_KEY = 'orchbee:web:terminal-appearance:v1';

interface StorageFixture extends TerminalAppearanceStorage {
	readonly getItem: ReturnType<typeof vi.fn<TerminalAppearanceStorage['getItem']>>;
	readonly setItem: ReturnType<typeof vi.fn<TerminalAppearanceStorage['setItem']>>;
}

describe('terminal appearance preference storage', () => {
	it('normalizes a complete strict record and rejects incomplete or extra data', () => {
		expect(
			normalizeTerminalAppearancePreferences({
				...DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
				customBackground: '#ABCDEF',
				fontFamily: '  Example Mono  ',
			}),
		).toEqual({
			...DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
			customBackground: '#abcdef',
			fontFamily: 'Example Mono',
		});
		expect(normalizeTerminalAppearancePreferences({ fontSize: 13 })).toBeNull();
		expect(
			normalizeTerminalAppearancePreferences({
				...DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
				unexpected: true,
			}),
		).toBeNull();
		expect(
			normalizeTerminalAppearancePreferences({
				...DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
				fontWeight: 450,
			}),
		).toBeNull();
		expect(
			normalizeTerminalAppearancePreferences({
				...DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
				terminalPadding: 33,
			}),
		).toBeNull();
	});

	it('rejects the old incomplete appearance record without rewriting it', () => {
		const storage = createStorage(
			JSON.stringify({
				applicationTheme: 'dark',
				backgroundOpacityPercent: 80,
				customBackground: '#abcdef',
				customForeground: '#123456',
				fontFamily: 'Example Mono',
				fontSize: 16,
				terminalDisplayTheme: 'custom',
			}),
		);
		expect(readTerminalAppearancePreferences(storage)).toEqual({
			issue: 'invalid-record',
			preferences: DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
		});
		expect(storage.getItem).toHaveBeenCalledOnce();
		expect(storage.getItem).toHaveBeenCalledWith(EXPECTED_STORAGE_KEY);
		expect(storage.setItem).not.toHaveBeenCalled();
	});

	it('uses the complete default when a record is malformed or storage cannot be read', () => {
		const malformed = createStorage('{');
		const throwing = createStorage(null);
		throwing.getItem.mockImplementation(() => {
			throw new Error('injected read failure');
		});

		expect(readTerminalAppearancePreferences(malformed)).toEqual({
			issue: 'invalid-record',
			preferences: DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
		});
		expect(readTerminalAppearancePreferences(throwing)).toEqual({
			issue: 'read-failed',
			preferences: DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
		});
	});

	it('reads a valid persisted record and writes its full current replacement once', () => {
		const storage = createStorage(
			JSON.stringify({
				...DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
				applicationTheme: 'dark',
			}),
		);

		expect(readTerminalAppearancePreferences(storage)).toMatchObject({
			issue: null,
			preferences: { applicationTheme: 'dark' },
		});
		expect(
			writeTerminalAppearancePreferences(storage, DEFAULT_TERMINAL_APPEARANCE_PREFERENCES),
		).toBe(true);
		expect(storage.setItem).toHaveBeenCalledOnce();
		expect(storage.setItem).toHaveBeenCalledWith(
			EXPECTED_STORAGE_KEY,
			JSON.stringify(DEFAULT_TERMINAL_APPEARANCE_PREFERENCES),
		);
	});

	it('reports a failed write without throwing the user-visible preference transition away', () => {
		const storage = createStorage(null);
		storage.setItem.mockImplementation(() => {
			throw new Error('injected write failure');
		});

		expect(
			writeTerminalAppearancePreferences(storage, DEFAULT_TERMINAL_APPEARANCE_PREFERENCES),
		).toBe(false);
	});
});

function createStorage(
	serialized: string | null,
	overrides: Readonly<Record<string, string | null>> = {},
): StorageFixture {
	return {
		getItem: vi.fn<TerminalAppearanceStorage['getItem']>((key) => overrides[key] ?? serialized),
		setItem: vi.fn<TerminalAppearanceStorage['setItem']>(() => undefined),
	};
}
