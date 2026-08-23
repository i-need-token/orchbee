import { describe, expect, it, vi } from 'vitest';

import { DEFAULT_TERMINAL_INTERACTION_PREFERENCES } from '../terminal-interaction-preferences';
import {
	normalizeTerminalInteractionPreferences,
	readTerminalInteractionPreferences,
	writeTerminalInteractionPreferences,
	type TerminalInteractionStorage,
} from '../terminal-interaction-preferences-storage';

const EXPECTED_STORAGE_KEY = 'orchbee:web:terminal-interaction:v1';

describe('terminal interaction preference storage', () => {
	it('accepts only one complete strict record', () => {
		expect(normalizeTerminalInteractionPreferences({ copyOnSelect: false })).toEqual({
			copyOnSelect: false,
		});
		expect(normalizeTerminalInteractionPreferences({})).toBeNull();
		expect(
			normalizeTerminalInteractionPreferences({ copyOnSelect: true, fontLigatures: false }),
		).toBeNull();
	});

	it('restores defaults with a typed issue after invalid data', () => {
		const storage = createStorage('{');
		expect(readTerminalInteractionPreferences(storage)).toEqual({
			issue: 'invalid-record',
			preferences: DEFAULT_TERMINAL_INTERACTION_PREFERENCES,
		});
	});

	it('writes only the owned versioned key', () => {
		const storage = createStorage(null);
		expect(writeTerminalInteractionPreferences(storage, { copyOnSelect: false })).toBe(true);
		expect(storage.setItem).toHaveBeenCalledWith(
			EXPECTED_STORAGE_KEY,
			JSON.stringify({ copyOnSelect: false }),
		);
	});
});

function createStorage(serialized: string | null): TerminalInteractionStorage & {
	readonly setItem: ReturnType<typeof vi.fn<TerminalInteractionStorage['setItem']>>;
} {
	return {
		getItem: vi.fn<TerminalInteractionStorage['getItem']>(() => serialized),
		setItem: vi.fn<TerminalInteractionStorage['setItem']>(() => undefined),
	};
}
