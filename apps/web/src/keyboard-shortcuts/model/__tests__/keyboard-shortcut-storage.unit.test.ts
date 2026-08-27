import { describe, expect, it, vi } from 'vitest';

import {
	readKeyboardShortcutOverrides,
	writeKeyboardShortcutOverrides,
} from '../keyboard-shortcut-storage';

const VERSION_ONE_KEY = 'orchbee:web:keyboard-shortcuts:v1';
const VERSION_TWO_KEY = 'orchbee:web:keyboard-shortcuts:v2';
const VERSION_THREE_KEY = 'orchbee:web:keyboard-shortcuts:v3';
const VERSION_FOUR_KEY = 'orchbee:web:keyboard-shortcuts:v4';
const VERSION_FIVE_KEY = 'orchbee:web:keyboard-shortcuts:v5';
const VERSION_SIX_KEY = 'orchbee:web:keyboard-shortcuts:v6';
const VERSION_SEVEN_KEY = 'orchbee:web:keyboard-shortcuts:v7';
const VERSION_EIGHT_KEY = 'orchbee:web:keyboard-shortcuts:v8';

describe('keyboard shortcut storage', () => {
	it('restores one strict, sorted, canonical v8 override record', () => {
		const storage = storageWith({
			[VERSION_EIGHT_KEY]: JSON.stringify({
				overrides: [
					{
						chord: { code: 'KeyG', modifiers: ['mod', 'shift'] },
						shortcutId: 'workbench.search.all.open',
					},
				],
				version: 8,
			}),
		});

		expect(readKeyboardShortcutOverrides(storage, 'standard')).toEqual({
			issue: null,
			migration: null,
			overrides: [
				{
					chord: { code: 'KeyG', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
		});
	});

	it.each([
		['malformed JSON', '{'],
		['unknown record field', JSON.stringify({ overrides: [], unexpected: true, version: 8 })],
		[
			'unknown shortcut ID',
			JSON.stringify({
				overrides: [{ chord: { code: 'KeyG', modifiers: ['mod'] }, shortcutId: 'unknown' }],
				version: 8,
			}),
		],
		[
			'duplicate shortcut ID',
			JSON.stringify({
				overrides: [
					{
						chord: { code: 'KeyG', modifiers: ['mod'] },
						shortcutId: 'workbench.search.all.open',
					},
					{
						chord: { code: 'KeyH', modifiers: ['mod'] },
						shortcutId: 'workbench.search.all.open',
					},
				],
				version: 8,
			}),
		],
		[
			'unsorted overrides',
			JSON.stringify({
				overrides: [
					{
						chord: { code: 'KeyG', modifiers: ['mod'] },
						shortcutId: 'workbench.settings.open',
					},
					{
						chord: { code: 'KeyH', modifiers: ['mod'] },
						shortcutId: 'terminal.search.open',
					},
				],
				version: 8,
			}),
		],
		[
			'noncanonical modifier alias',
			JSON.stringify({
				overrides: [
					{
						chord: { code: 'KeyG', modifiers: ['control'] },
						shortcutId: 'workbench.search.all.open',
					},
				],
				version: 8,
			}),
		],
		[
			'default stored as custom',
			JSON.stringify({
				overrides: [
					{
						chord: { code: 'KeyK', modifiers: ['mod', 'shift'] },
						shortcutId: 'workbench.search.all.open',
					},
				],
				version: 8,
			}),
		],
		[
			'effective conflict',
			JSON.stringify({
				overrides: [
					{
						chord: { code: 'KeyF', modifiers: ['mod', 'shift'] },
						shortcutId: 'workbench.search.all.open',
					},
				],
				version: 8,
			}),
		],
	] as const)('rejects the whole v8 record for %s', (_case, serialized) => {
		expect(
			readKeyboardShortcutOverrides(storageWith({ [VERSION_EIGHT_KEY]: serialized }), 'standard'),
		).toEqual({ issue: 'invalid-record', migration: null, overrides: [] });
	});

	it('migrates a valid v7 override into the terminal font-size catalog', () => {
		const versionSeven = JSON.stringify({
			overrides: [
				{
					chord: { code: 'KeyG', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 7,
		});

		expect(
			readKeyboardShortcutOverrides(storageWith({ [VERSION_SEVEN_KEY]: versionSeven }), 'standard'),
		).toEqual({
			issue: null,
			migration: 'v7',
			overrides: [
				{
					chord: { code: 'KeyG', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
		});
	});

	it('migrates a valid v6 override into the keyboard history catalog', () => {
		const versionSix = JSON.stringify({
			overrides: [
				{
					chord: { code: 'KeyG', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 6,
		});

		expect(
			readKeyboardShortcutOverrides(storageWith({ [VERSION_SIX_KEY]: versionSix }), 'standard'),
		).toEqual({
			issue: null,
			migration: 'v6',
			overrides: [
				{
					chord: { code: 'KeyG', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
		});
	});

	it('rejects a v6 override that conflicts with a new history scroll default', () => {
		const versionSix = JSON.stringify({
			overrides: [
				{
					chord: { code: 'PageUp', modifiers: ['control', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 6,
		});

		expect(
			readKeyboardShortcutOverrides(storageWith({ [VERSION_SIX_KEY]: versionSix }), 'apple'),
		).toEqual({ issue: 'invalid-record', migration: null, overrides: [] });
	});

	it('migrates a valid v5 override into the prompt-navigation catalog', () => {
		const versionFive = JSON.stringify({
			overrides: [
				{
					chord: { code: 'KeyG', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 5,
		});

		expect(
			readKeyboardShortcutOverrides(storageWith({ [VERSION_FIVE_KEY]: versionFive }), 'standard'),
		).toEqual({
			issue: null,
			migration: 'v5',
			overrides: [
				{
					chord: { code: 'KeyG', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
		});
	});

	it('rejects a v5 override that conflicts with a new prompt-navigation default', () => {
		const versionFive = JSON.stringify({
			overrides: [
				{
					chord: { code: 'KeyZ', modifiers: ['control', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 5,
		});

		expect(
			readKeyboardShortcutOverrides(storageWith({ [VERSION_FIVE_KEY]: versionFive }), 'apple'),
		).toEqual({ issue: 'invalid-record', migration: null, overrides: [] });
	});

	it('migrates a valid v4 override into the adjacent Tab catalog', () => {
		const versionFour = JSON.stringify({
			overrides: [
				{
					chord: { code: 'KeyG', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 4,
		});

		expect(
			readKeyboardShortcutOverrides(storageWith({ [VERSION_FOUR_KEY]: versionFour }), 'standard'),
		).toEqual({
			issue: null,
			migration: 'v4',
			overrides: [
				{
					chord: { code: 'KeyG', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
		});
	});

	it('rejects a v4 override that conflicts with a new adjacent Tab default', () => {
		const versionFour = JSON.stringify({
			overrides: [
				{
					chord: { code: 'PageDown', modifiers: ['mod'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 4,
		});

		expect(
			readKeyboardShortcutOverrides(storageWith({ [VERSION_FOUR_KEY]: versionFour }), 'standard'),
		).toEqual({ issue: 'invalid-record', migration: null, overrides: [] });
	});

	it('migrates a valid v3 override into the positional Tab catalog', () => {
		const versionThree = JSON.stringify({
			overrides: [
				{
					chord: { code: 'KeyG', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 3,
		});

		expect(
			readKeyboardShortcutOverrides(storageWith({ [VERSION_THREE_KEY]: versionThree }), 'standard'),
		).toEqual({
			issue: null,
			migration: 'v3',
			overrides: [
				{
					chord: { code: 'KeyG', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
		});
	});

	it('rejects a v3 override that conflicts with a new positional Tab default', () => {
		const versionThree = JSON.stringify({
			overrides: [
				{
					chord: { code: 'Digit2', modifiers: ['mod'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 3,
		});

		expect(
			readKeyboardShortcutOverrides(storageWith({ [VERSION_THREE_KEY]: versionThree }), 'apple'),
		).toEqual({ issue: 'invalid-record', migration: null, overrides: [] });
	});

	it('migrates a valid v2 override into the expanded catalog', () => {
		const versionTwo = JSON.stringify({
			overrides: [
				{
					chord: { code: 'KeyG', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 2,
		});

		expect(
			readKeyboardShortcutOverrides(storageWith({ [VERSION_TWO_KEY]: versionTwo }), 'standard'),
		).toEqual({
			issue: null,
			migration: 'v2',
			overrides: [
				{
					chord: { code: 'KeyG', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
		});
	});

	it('drops a v2 override equal to the new Apple search default', () => {
		const versionTwo = JSON.stringify({
			overrides: [
				{
					chord: { code: 'KeyF', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 2,
		});

		expect(
			readKeyboardShortcutOverrides(storageWith({ [VERSION_TWO_KEY]: versionTwo }), 'apple'),
		).toEqual({ issue: null, migration: 'v2', overrides: [] });
	});

	it('rejects a valid v2 override that conflicts with the added clear default', () => {
		const versionTwo = JSON.stringify({
			overrides: [
				{
					chord: { code: 'KeyG', modifiers: ['mod'] },
					shortcutId: 'workbench.search.all.open',
				},
				{
					chord: { code: 'KeyK', modifiers: ['mod'] },
					shortcutId: 'workbench.settings.open',
				},
			],
			version: 2,
		});

		expect(
			readKeyboardShortcutOverrides(storageWith({ [VERSION_TWO_KEY]: versionTwo }), 'apple'),
		).toEqual({ issue: 'invalid-record', migration: null, overrides: [] });
	});

	it('migrates valid v1 overrides and drops an override equal to the new default', () => {
		const legacy = JSON.stringify({
			overrides: [
				{
					chord: { code: 'KeyG', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
				{
					chord: { code: 'KeyT', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.terminal.create',
				},
			],
			version: 1,
		});

		expect(
			readKeyboardShortcutOverrides(storageWith({ [VERSION_ONE_KEY]: legacy }), 'standard'),
		).toEqual({
			issue: null,
			migration: 'v1',
			overrides: [
				{
					chord: { code: 'KeyG', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
		});
	});

	it('rejects a valid legacy override that conflicts with an added v2 default', () => {
		const legacy = JSON.stringify({
			overrides: [
				{
					chord: { code: 'KeyD', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 1,
		});

		expect(
			readKeyboardShortcutOverrides(storageWith({ [VERSION_ONE_KEY]: legacy }), 'standard'),
		).toEqual({ issue: 'invalid-record', migration: null, overrides: [] });
	});

	it('never consults older records when an invalid v8 record exists', () => {
		const getItem = vi.fn<(key: string) => string | null>((key) =>
			key === VERSION_EIGHT_KEY ? '{' : JSON.stringify({ overrides: [], version: 7 }),
		);

		expect(readKeyboardShortcutOverrides({ getItem, setItem: () => undefined }, 'apple')).toEqual({
			issue: 'invalid-record',
			migration: null,
			overrides: [],
		});
		expect(getItem).toHaveBeenCalledTimes(1);
		expect(getItem).toHaveBeenCalledWith(VERSION_EIGHT_KEY);
	});

	it('never consults v5 when an invalid v6 migration source exists', () => {
		const getItem = vi.fn<(key: string) => string | null>((key) => {
			if (key === VERSION_EIGHT_KEY) return null;
			if (key === VERSION_SEVEN_KEY) return null;
			if (key === VERSION_SIX_KEY) return '{';
			return JSON.stringify({ overrides: [], version: 5 });
		});

		expect(readKeyboardShortcutOverrides({ getItem, setItem: () => undefined }, 'apple')).toEqual({
			issue: 'invalid-record',
			migration: null,
			overrides: [],
		});
		expect(getItem).toHaveBeenCalledTimes(3);
		expect(getItem).not.toHaveBeenCalledWith(VERSION_FIVE_KEY);
	});

	it('never consults v4 when an invalid v5 migration source exists', () => {
		const getItem = vi.fn<(key: string) => string | null>((key) => {
			if (key === VERSION_EIGHT_KEY) return null;
			if (key === VERSION_SEVEN_KEY) return null;
			if (key === VERSION_SIX_KEY) return null;
			if (key === VERSION_FIVE_KEY) return '{';
			return JSON.stringify({ overrides: [], version: 4 });
		});

		expect(readKeyboardShortcutOverrides({ getItem, setItem: () => undefined }, 'apple')).toEqual({
			issue: 'invalid-record',
			migration: null,
			overrides: [],
		});
		expect(getItem).toHaveBeenCalledTimes(4);
		expect(getItem).not.toHaveBeenCalledWith(VERSION_FOUR_KEY);
	});

	it('never consults v3 when an invalid v4 migration source exists', () => {
		const getItem = vi.fn<(key: string) => string | null>((key) => {
			if (key === VERSION_EIGHT_KEY) return null;
			if (key === VERSION_SEVEN_KEY) return null;
			if (key === VERSION_SIX_KEY) return null;
			if (key === VERSION_FIVE_KEY) return null;
			if (key === VERSION_FOUR_KEY) return '{';
			return JSON.stringify({ overrides: [], version: 3 });
		});

		expect(readKeyboardShortcutOverrides({ getItem, setItem: () => undefined }, 'apple')).toEqual({
			issue: 'invalid-record',
			migration: null,
			overrides: [],
		});
		expect(getItem).toHaveBeenCalledTimes(5);
		expect(getItem).not.toHaveBeenCalledWith(VERSION_THREE_KEY);
	});

	it('never consults v2 when an invalid v3 migration source exists', () => {
		const getItem = vi.fn<(key: string) => string | null>((key) => {
			if (key === VERSION_EIGHT_KEY) return null;
			if (key === VERSION_SEVEN_KEY) return null;
			if (key === VERSION_SIX_KEY) return null;
			if (key === VERSION_FIVE_KEY) return null;
			if (key === VERSION_FOUR_KEY) return null;
			if (key === VERSION_THREE_KEY) return '{';
			return JSON.stringify({ overrides: [], version: 2 });
		});

		expect(readKeyboardShortcutOverrides({ getItem, setItem: () => undefined }, 'apple')).toEqual({
			issue: 'invalid-record',
			migration: null,
			overrides: [],
		});
		expect(getItem).toHaveBeenCalledTimes(6);
		expect(getItem).not.toHaveBeenCalledWith(VERSION_TWO_KEY);
	});

	it('never consults v1 when an invalid v2 migration source exists', () => {
		const getItem = vi.fn<(key: string) => string | null>((key) => {
			if (key === VERSION_EIGHT_KEY) return null;
			if (key === VERSION_SEVEN_KEY) return null;
			if (key === VERSION_SIX_KEY) return null;
			if (key === VERSION_FIVE_KEY) return null;
			if (key === VERSION_FOUR_KEY) return null;
			if (key === VERSION_THREE_KEY) return null;
			if (key === VERSION_TWO_KEY) return '{';
			return JSON.stringify({ overrides: [], version: 1 });
		});

		expect(readKeyboardShortcutOverrides({ getItem, setItem: () => undefined }, 'apple')).toEqual({
			issue: 'invalid-record',
			migration: null,
			overrides: [],
		});
		expect(getItem).toHaveBeenCalledTimes(7);
		expect(getItem).not.toHaveBeenCalledWith(VERSION_ONE_KEY);
	});

	it('reports read failures and writes one stable complete v8 record', () => {
		expect(
			readKeyboardShortcutOverrides(
				{
					getItem: () => {
						throw new Error('unavailable');
					},
					setItem: () => undefined,
				},
				'apple',
			),
		).toEqual({ issue: 'read-failed', migration: null, overrides: [] });

		const setItem = vi.fn<(key: string, value: string) => void>();
		expect(
			writeKeyboardShortcutOverrides({ getItem: () => null, setItem }, [
				{
					chord: { code: 'KeyG', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			]),
		).toBe(true);
		expect(setItem).toHaveBeenCalledWith(
			VERSION_EIGHT_KEY,
			'{"overrides":[{"chord":{"code":"KeyG","modifiers":["mod","shift"]},"shortcutId":"workbench.search.all.open"}],"version":8}',
		);
	});
});

function storageWith(records: Readonly<Record<string, string>>): {
	readonly getItem: (key: string) => string | null;
	readonly setItem: (key: string, value: string) => void;
} {
	return {
		getItem: (key) => records[key] ?? null,
		setItem: () => undefined,
	};
}
