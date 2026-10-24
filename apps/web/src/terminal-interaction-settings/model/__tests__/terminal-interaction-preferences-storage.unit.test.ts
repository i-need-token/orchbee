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
		expect(
			normalizeTerminalInteractionPreferences({
				copyOnSelect: false,
				trimTrailingBlankRowsForAgents: true,
			}),
		).toEqual({
			copyOnSelect: false,
			trimTrailingBlankRowsForAgents: true,
		});
		expect(normalizeTerminalInteractionPreferences({ copyOnSelect: false })).toBeNull();
		expect(normalizeTerminalInteractionPreferences({})).toBeNull();
		expect(
			normalizeTerminalInteractionPreferences({
				copyOnSelect: true,
				fontLigatures: false,
				trimTrailingBlankRowsForAgents: true,
			}),
		).toBeNull();
	});

	it('restores defaults with a typed issue after invalid data', () => {
		const storage = createStorage({ [EXPECTED_STORAGE_KEY]: '{' });
		expect(readTerminalInteractionPreferences(storage)).toEqual({
			issue: 'invalid-record',
			preferences: DEFAULT_TERMINAL_INTERACTION_PREFERENCES,
		});
	});

	it('rejects an incomplete record without reading another key or writing defaults', () => {
		const storage = createStorage({
			[EXPECTED_STORAGE_KEY]: JSON.stringify({ copyOnSelect: false }),
		});
		expect(readTerminalInteractionPreferences(storage)).toEqual({
			issue: 'invalid-record',
			preferences: DEFAULT_TERMINAL_INTERACTION_PREFERENCES,
		});
		expect(storage.getItem).toHaveBeenCalledOnce();
		expect(storage.setItem).not.toHaveBeenCalled();
	});

	it('writes only the owned versioned key', () => {
		const storage = createStorage({});
		expect(
			writeTerminalInteractionPreferences(storage, {
				copyOnSelect: false,
				trimTrailingBlankRowsForAgents: false,
			}),
		).toBe(true);
		expect(storage.setItem).toHaveBeenCalledWith(
			EXPECTED_STORAGE_KEY,
			JSON.stringify({ copyOnSelect: false, trimTrailingBlankRowsForAgents: false }),
		);
	});
});

function createStorage(records: Readonly<Record<string, string>>): TerminalInteractionStorage & {
	readonly getItem: ReturnType<typeof vi.fn<TerminalInteractionStorage['getItem']>>;
	readonly setItem: ReturnType<typeof vi.fn<TerminalInteractionStorage['setItem']>>;
} {
	return {
		getItem: vi.fn<TerminalInteractionStorage['getItem']>((key) => records[key] ?? null),
		setItem: vi.fn<TerminalInteractionStorage['setItem']>(() => undefined),
	};
}
