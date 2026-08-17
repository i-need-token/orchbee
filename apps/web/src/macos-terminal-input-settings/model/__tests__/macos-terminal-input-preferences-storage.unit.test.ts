import { describe, expect, it, vi } from 'vitest';

import {
	normalizeMacosTerminalInputPreferences,
	readMacosTerminalInputPreferences,
	writeMacosTerminalInputPreferences,
} from '../macos-terminal-input-preferences-storage';

describe('macOS terminal input preference storage', () => {
	it('restores only one strict supported Option-as-Alt value', () => {
		const storage = {
			getItem: vi.fn<() => string | null>(() => JSON.stringify({ optionAsAlt: 'right' })),
			setItem: vi.fn<(key: string, value: string) => void>(),
		};
		expect(readMacosTerminalInputPreferences(storage)).toEqual({
			issue: null,
			preferences: { optionAsAlt: 'right' },
		});
		expect(normalizeMacosTerminalInputPreferences({ extra: true, optionAsAlt: 'left' })).toBeNull();
		expect(normalizeMacosTerminalInputPreferences({ optionAsAlt: 'sometimes' })).toBeNull();
	});

	it('falls back to off for malformed or unavailable records', () => {
		expect(
			readMacosTerminalInputPreferences({
				getItem: () => '{',
				setItem: () => undefined,
			}),
		).toEqual({ issue: 'invalid-record', preferences: { optionAsAlt: 'off' } });
		expect(
			readMacosTerminalInputPreferences({
				getItem: () => {
					throw new Error('unavailable');
				},
				setItem: () => undefined,
			}),
		).toEqual({ issue: 'read-failed', preferences: { optionAsAlt: 'off' } });
	});

	it('reports persistence failure without throwing', () => {
		expect(
			writeMacosTerminalInputPreferences(
				{
					getItem: () => null,
					setItem: () => {
						throw new Error('full');
					},
				},
				{ optionAsAlt: 'both' },
			),
		).toBe(false);
	});
});
