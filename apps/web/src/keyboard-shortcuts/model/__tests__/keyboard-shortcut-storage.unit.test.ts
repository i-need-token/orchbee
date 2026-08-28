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
const VERSION_NINE_KEY = 'orchbee:web:keyboard-shortcuts:v9';
const VERSION_TEN_KEY = 'orchbee:web:keyboard-shortcuts:v10';
const VERSION_ELEVEN_KEY = 'orchbee:web:keyboard-shortcuts:v11';
const VERSION_TWELVE_KEY = 'orchbee:web:keyboard-shortcuts:v12';
const VERSION_THIRTEEN_KEY = 'orchbee:web:keyboard-shortcuts:v13';
const VERSION_FOURTEEN_KEY = 'orchbee:web:keyboard-shortcuts:v14';
const VERSION_FIFTEEN_KEY = 'orchbee:web:keyboard-shortcuts:v15';
const VERSION_SIXTEEN_KEY = 'orchbee:web:keyboard-shortcuts:v16';

describe('keyboard shortcut storage', () => {
	it('restores one strict, sorted, canonical v16 override record', () => {
		const storage = storageWith({
			[VERSION_SIXTEEN_KEY]: JSON.stringify({
				overrides: [
					{
						chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
						shortcutId: 'workbench.search.all.open',
					},
				],
				version: 16,
			}),
		});

		expect(readKeyboardShortcutOverrides(storage, 'standard')).toEqual({
			issue: null,
			migration: null,
			overrides: [
				{
					chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
		});
	});

	it.each([
		['malformed JSON', '{'],
		['unknown record field', JSON.stringify({ overrides: [], unexpected: true, version: 16 })],
		[
			'unknown shortcut ID',
			JSON.stringify({
				overrides: [{ chord: { code: 'KeyG', modifiers: ['mod'] }, shortcutId: 'unknown' }],
				version: 16,
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
				version: 16,
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
				version: 16,
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
				version: 16,
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
				version: 16,
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
				version: 16,
			}),
		],
	] as const)('rejects the whole v16 record for %s', (_case, serialized) => {
		expect(
			readKeyboardShortcutOverrides(storageWith({ [VERSION_SIXTEEN_KEY]: serialized }), 'standard'),
		).toEqual({ issue: 'invalid-record', migration: null, overrides: [] });
	});

	it('migrates a valid v15 override into the Panel-equalization catalog', () => {
		const versionFifteen = JSON.stringify({
			overrides: [
				{
					chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 15,
		});

		expect(
			readKeyboardShortcutOverrides(
				storageWith({ [VERSION_FIFTEEN_KEY]: versionFifteen }),
				'standard',
			),
		).toEqual({
			issue: null,
			migration: 'v15',
			overrides: [
				{
					chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
		});
	});

	it('rejects a v15 override that conflicts with the new Panel-equalization default', () => {
		const versionFifteen = JSON.stringify({
			overrides: [
				{
					chord: { code: 'Equal', modifiers: ['alt', 'mod'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 15,
		});

		expect(
			readKeyboardShortcutOverrides(
				storageWith({ [VERSION_FIFTEEN_KEY]: versionFifteen }),
				'standard',
			),
		).toEqual({ issue: 'invalid-record', migration: null, overrides: [] });
	});

	it('migrates a valid v14 override into the directional Panel-movement catalog', () => {
		const versionFourteen = JSON.stringify({
			overrides: [
				{
					chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 14,
		});

		expect(
			readKeyboardShortcutOverrides(
				storageWith({ [VERSION_FOURTEEN_KEY]: versionFourteen }),
				'standard',
			),
		).toEqual({
			issue: null,
			migration: 'v14',
			overrides: [
				{
					chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
		});
	});

	it('rejects a v14 override that conflicts with a new directional Panel-movement default', () => {
		const versionFourteen = JSON.stringify({
			overrides: [
				{
					chord: { code: 'ArrowLeft', modifiers: ['mod', 'alt', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 14,
		});

		expect(
			readKeyboardShortcutOverrides(
				storageWith({ [VERSION_FOURTEEN_KEY]: versionFourteen }),
				'standard',
			),
		).toEqual({ issue: 'invalid-record', migration: null, overrides: [] });
	});

	it('migrates a valid v13 override into the panel-resize catalog', () => {
		const versionThirteen = JSON.stringify({
			overrides: [
				{
					chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 13,
		});

		expect(
			readKeyboardShortcutOverrides(
				storageWith({ [VERSION_THIRTEEN_KEY]: versionThirteen }),
				'standard',
			),
		).toEqual({
			issue: null,
			migration: 'v13',
			overrides: [
				{
					chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
		});
	});

	it('rejects a v13 override that conflicts with the new panel-resize default', () => {
		const versionThirteen = JSON.stringify({
			overrides: [
				{
					chord: { code: 'KeyR', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 13,
		});

		expect(
			readKeyboardShortcutOverrides(
				storageWith({ [VERSION_THIRTEEN_KEY]: versionThirteen }),
				'standard',
			),
		).toEqual({ issue: 'invalid-record', migration: null, overrides: [] });
	});

	it('migrates a valid v12 override into the Tab-title catalog', () => {
		const versionTwelve = JSON.stringify({
			overrides: [
				{
					chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 12,
		});

		expect(
			readKeyboardShortcutOverrides(
				storageWith({ [VERSION_TWELVE_KEY]: versionTwelve }),
				'standard',
			),
		).toEqual({
			issue: null,
			migration: 'v12',
			overrides: [
				{
					chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
		});
	});

	it('rejects a v12 override that conflicts with the new Tab-title default', () => {
		const versionTwelve = JSON.stringify({
			overrides: [
				{
					chord: { code: 'KeyT', modifiers: ['mod', 'alt', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 12,
		});

		expect(
			readKeyboardShortcutOverrides(
				storageWith({ [VERSION_TWELVE_KEY]: versionTwelve }),
				'standard',
			),
		).toEqual({ issue: 'invalid-record', migration: null, overrides: [] });
	});

	it('migrates a valid v11 override into the active-Tab-movement catalog', () => {
		const versionEleven = JSON.stringify({
			overrides: [
				{
					chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 11,
		});

		expect(
			readKeyboardShortcutOverrides(
				storageWith({ [VERSION_ELEVEN_KEY]: versionEleven }),
				'standard',
			),
		).toEqual({
			issue: null,
			migration: 'v11',
			overrides: [
				{
					chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
		});
	});

	it('rejects a v11 override that conflicts with a new active-Tab-movement default', () => {
		const versionEleven = JSON.stringify({
			overrides: [
				{
					chord: { code: 'Comma', modifiers: ['control', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 11,
		});

		expect(
			readKeyboardShortcutOverrides(storageWith({ [VERSION_ELEVEN_KEY]: versionEleven }), 'apple'),
		).toEqual({ issue: 'invalid-record', migration: null, overrides: [] });
	});

	it('migrates a valid v10 override into the scrollback-viewer catalog', () => {
		const versionTen = JSON.stringify({
			overrides: [
				{
					chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 10,
		});

		expect(
			readKeyboardShortcutOverrides(storageWith({ [VERSION_TEN_KEY]: versionTen }), 'standard'),
		).toEqual({
			issue: null,
			migration: 'v10',
			overrides: [
				{
					chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
		});
	});

	it('rejects a v10 override that conflicts with the new scrollback-viewer default', () => {
		const versionTen = JSON.stringify({
			overrides: [
				{
					chord: { code: 'KeyH', modifiers: ['control', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 10,
		});

		expect(
			readKeyboardShortcutOverrides(storageWith({ [VERSION_TEN_KEY]: versionTen }), 'apple'),
		).toEqual({ issue: 'invalid-record', migration: null, overrides: [] });
	});

	it('migrates a valid v9 override into the command-output catalog', () => {
		const versionNine = JSON.stringify({
			overrides: [
				{
					chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 9,
		});

		expect(
			readKeyboardShortcutOverrides(storageWith({ [VERSION_NINE_KEY]: versionNine }), 'standard'),
		).toEqual({
			issue: null,
			migration: 'v9',
			overrides: [
				{
					chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
		});
	});

	it('rejects a v9 override that conflicts with the new command-output default', () => {
		const versionNine = JSON.stringify({
			overrides: [
				{
					chord: { code: 'KeyG', modifiers: ['control', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 9,
		});

		expect(
			readKeyboardShortcutOverrides(storageWith({ [VERSION_NINE_KEY]: versionNine }), 'apple'),
		).toEqual({ issue: 'invalid-record', migration: null, overrides: [] });
	});

	it('migrates a valid v8 override into the terminal-hints catalog', () => {
		const versionEight = JSON.stringify({
			overrides: [
				{
					chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 8,
		});

		expect(
			readKeyboardShortcutOverrides(storageWith({ [VERSION_EIGHT_KEY]: versionEight }), 'standard'),
		).toEqual({
			issue: null,
			migration: 'v8',
			overrides: [
				{
					chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
		});
	});

	it('rejects a v8 override that conflicts with the new terminal-hints default', () => {
		const versionEight = JSON.stringify({
			overrides: [
				{
					chord: { code: 'KeyO', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 8,
		});

		expect(
			readKeyboardShortcutOverrides(storageWith({ [VERSION_EIGHT_KEY]: versionEight }), 'standard'),
		).toEqual({ issue: 'invalid-record', migration: null, overrides: [] });
	});

	it('migrates a valid v7 override into the terminal font-size catalog', () => {
		const versionSeven = JSON.stringify({
			overrides: [
				{
					chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
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
					chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
		});
	});

	it('migrates a valid v6 override into the keyboard history catalog', () => {
		const versionSix = JSON.stringify({
			overrides: [
				{
					chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
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
					chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
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
					chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
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
					chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
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
					chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
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
					chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
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
					chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
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
					chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
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
					chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
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
					chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
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
					chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
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
					chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
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

	it('never consults older records when an invalid v16 record exists', () => {
		const getItem = vi.fn<(key: string) => string | null>((key) =>
			key === VERSION_SIXTEEN_KEY ? '{' : JSON.stringify({ overrides: [], version: 15 }),
		);

		expect(readKeyboardShortcutOverrides({ getItem, setItem: () => undefined }, 'apple')).toEqual({
			issue: 'invalid-record',
			migration: null,
			overrides: [],
		});
		expect(getItem).toHaveBeenCalledTimes(1);
		expect(getItem).toHaveBeenCalledWith(VERSION_SIXTEEN_KEY);
	});

	it('never consults v11 when an invalid v12 migration source exists', () => {
		const getItem = vi.fn<(key: string) => string | null>((key) => {
			if (key === VERSION_SIXTEEN_KEY) return null;
			if (key === VERSION_FIFTEEN_KEY) return null;
			if (key === VERSION_FOURTEEN_KEY) return null;
			if (key === VERSION_THIRTEEN_KEY) return null;
			if (key === VERSION_TWELVE_KEY) return '{';
			return JSON.stringify({ overrides: [], version: 11 });
		});

		expect(readKeyboardShortcutOverrides({ getItem, setItem: () => undefined }, 'apple')).toEqual({
			issue: 'invalid-record',
			migration: null,
			overrides: [],
		});
		expect(getItem).toHaveBeenCalledTimes(5);
		expect(getItem).not.toHaveBeenCalledWith(VERSION_ELEVEN_KEY);
	});

	it('never consults v8 when an invalid v9 migration source exists', () => {
		const getItem = vi.fn<(key: string) => string | null>((key) => {
			if (key === VERSION_SIXTEEN_KEY) return null;
			if (key === VERSION_FIFTEEN_KEY) return null;
			if (key === VERSION_FOURTEEN_KEY) return null;
			if (key === VERSION_THIRTEEN_KEY) return null;
			if (key === VERSION_TWELVE_KEY) return null;
			if (key === VERSION_ELEVEN_KEY) return null;
			if (key === VERSION_TEN_KEY) return null;
			if (key === VERSION_NINE_KEY) return '{';
			return JSON.stringify({ overrides: [], version: 8 });
		});

		expect(readKeyboardShortcutOverrides({ getItem, setItem: () => undefined }, 'apple')).toEqual({
			issue: 'invalid-record',
			migration: null,
			overrides: [],
		});
		expect(getItem).toHaveBeenCalledTimes(8);
		expect(getItem).not.toHaveBeenCalledWith(VERSION_EIGHT_KEY);
	});

	it('never consults v5 when an invalid v6 migration source exists', () => {
		const getItem = vi.fn<(key: string) => string | null>((key) => {
			if (key === VERSION_SIXTEEN_KEY) return null;
			if (key === VERSION_FIFTEEN_KEY) return null;
			if (key === VERSION_FOURTEEN_KEY) return null;
			if (key === VERSION_THIRTEEN_KEY) return null;
			if (key === VERSION_TWELVE_KEY) return null;
			if (key === VERSION_ELEVEN_KEY) return null;
			if (key === VERSION_TEN_KEY) return null;
			if (key === VERSION_NINE_KEY) return null;
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
		expect(getItem).toHaveBeenCalledTimes(11);
		expect(getItem).not.toHaveBeenCalledWith(VERSION_FIVE_KEY);
	});

	it('never consults v4 when an invalid v5 migration source exists', () => {
		const getItem = vi.fn<(key: string) => string | null>((key) => {
			if (key === VERSION_SIXTEEN_KEY) return null;
			if (key === VERSION_FIFTEEN_KEY) return null;
			if (key === VERSION_FOURTEEN_KEY) return null;
			if (key === VERSION_THIRTEEN_KEY) return null;
			if (key === VERSION_TWELVE_KEY) return null;
			if (key === VERSION_ELEVEN_KEY) return null;
			if (key === VERSION_TEN_KEY) return null;
			if (key === VERSION_NINE_KEY) return null;
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
		expect(getItem).toHaveBeenCalledTimes(12);
		expect(getItem).not.toHaveBeenCalledWith(VERSION_FOUR_KEY);
	});

	it('never consults v3 when an invalid v4 migration source exists', () => {
		const getItem = vi.fn<(key: string) => string | null>((key) => {
			if (key === VERSION_SIXTEEN_KEY) return null;
			if (key === VERSION_FIFTEEN_KEY) return null;
			if (key === VERSION_FOURTEEN_KEY) return null;
			if (key === VERSION_THIRTEEN_KEY) return null;
			if (key === VERSION_TWELVE_KEY) return null;
			if (key === VERSION_ELEVEN_KEY) return null;
			if (key === VERSION_TEN_KEY) return null;
			if (key === VERSION_NINE_KEY) return null;
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
		expect(getItem).toHaveBeenCalledTimes(13);
		expect(getItem).not.toHaveBeenCalledWith(VERSION_THREE_KEY);
	});

	it('never consults v2 when an invalid v3 migration source exists', () => {
		const getItem = vi.fn<(key: string) => string | null>((key) => {
			if (key === VERSION_SIXTEEN_KEY) return null;
			if (key === VERSION_FIFTEEN_KEY) return null;
			if (key === VERSION_FOURTEEN_KEY) return null;
			if (key === VERSION_THIRTEEN_KEY) return null;
			if (key === VERSION_TWELVE_KEY) return null;
			if (key === VERSION_ELEVEN_KEY) return null;
			if (key === VERSION_TEN_KEY) return null;
			if (key === VERSION_NINE_KEY) return null;
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
		expect(getItem).toHaveBeenCalledTimes(14);
		expect(getItem).not.toHaveBeenCalledWith(VERSION_TWO_KEY);
	});

	it('never consults v1 when an invalid v2 migration source exists', () => {
		const getItem = vi.fn<(key: string) => string | null>((key) => {
			if (key === VERSION_SIXTEEN_KEY) return null;
			if (key === VERSION_FIFTEEN_KEY) return null;
			if (key === VERSION_FOURTEEN_KEY) return null;
			if (key === VERSION_THIRTEEN_KEY) return null;
			if (key === VERSION_TWELVE_KEY) return null;
			if (key === VERSION_ELEVEN_KEY) return null;
			if (key === VERSION_TEN_KEY) return null;
			if (key === VERSION_NINE_KEY) return null;
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
		expect(getItem).toHaveBeenCalledTimes(15);
		expect(getItem).not.toHaveBeenCalledWith(VERSION_ONE_KEY);
	});

	it('reports read failures and writes one stable complete v16 record', () => {
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
					chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			]),
		).toBe(true);
		expect(setItem).toHaveBeenCalledWith(
			VERSION_SIXTEEN_KEY,
			'{"overrides":[{"chord":{"code":"KeyY","modifiers":["mod","shift"]},"shortcutId":"workbench.search.all.open"}],"version":16}',
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
