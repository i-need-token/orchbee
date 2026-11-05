import { describe, expect, it, vi } from 'vitest';

import { DEFAULT_TERMINAL_APPEARANCE_PREFERENCES } from '../terminal-appearance-preferences';
import {
	normalizeTerminalAppearancePreferences,
	readHostAppearancePreferences,
	writeHostAppearancePreferences,
	type TerminalAppearanceStorage,
} from '../terminal-appearance-preferences-storage';

const EXPECTED_STORAGE_KEY = 'orchbee:web:terminal-host-appearance:v1';

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

	it('uses the complete default when a record is malformed or storage cannot be read', () => {
		const malformed = createStorage('{');
		const throwing = createStorage(null);
		throwing.getItem.mockImplementation(() => {
			throw new Error('injected read failure');
		});

		expect(readHostAppearancePreferences(malformed)).toEqual({
			issue: 'invalid-record',
			preferences: DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
		});
		expect(readHostAppearancePreferences(throwing)).toEqual({
			issue: 'read-failed',
			preferences: DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
		});
	});

	it('reads a valid host record and writes only the host leaves once', () => {
		const {
			applicationTheme: _applicationTheme,
			darkTheme: _darkTheme,
			lightTheme: _lightTheme,
			terminalDisplayTheme: _terminalDisplayTheme,
			customForeground: _customForeground,
			customBackground: _customBackground,
			...host
		} = DEFAULT_TERMINAL_APPEARANCE_PREFERENCES;
		const storage = createStorage(JSON.stringify({ ...host, fontSize: 16 }));

		expect(readHostAppearancePreferences(storage)).toMatchObject({
			issue: null,
			preferences: { fontSize: 16 },
		});
		expect(writeHostAppearancePreferences(storage, DEFAULT_TERMINAL_APPEARANCE_PREFERENCES)).toBe(
			true,
		);
		expect(storage.setItem).toHaveBeenCalledOnce();
		expect(storage.setItem).toHaveBeenCalledWith(EXPECTED_STORAGE_KEY, expect.any(String));
		expect(JSON.parse(storage.setItem.mock.calls[0]?.[1] ?? '')).toEqual(host);
	});

	it('reports a failed write without throwing the user-visible preference transition away', () => {
		const storage = createStorage(null);
		storage.setItem.mockImplementation(() => {
			throw new Error('injected write failure');
		});

		expect(writeHostAppearancePreferences(storage, DEFAULT_TERMINAL_APPEARANCE_PREFERENCES)).toBe(
			false,
		);
	});
});

function createStorage(serialized: string | null): StorageFixture {
	return {
		getItem: vi.fn<TerminalAppearanceStorage['getItem']>(() => serialized),
		setItem: vi.fn<TerminalAppearanceStorage['setItem']>(() => undefined),
	};
}
