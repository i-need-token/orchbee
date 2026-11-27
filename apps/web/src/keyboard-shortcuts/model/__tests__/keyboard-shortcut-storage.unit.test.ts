import { describe, expect, it, vi } from 'vitest';

import {
	readKeyboardShortcutOverrides,
	readKeyboardShortcutSteps,
	writeKeyboardShortcutOverrides,
	writeKeyboardShortcutSteps,
} from '../keyboard-shortcut-storage';

const STORAGE_KEY = 'orchbee:web:keyboard-shortcut-overrides:v1';
const STEPS_STORAGE_KEY = 'orchbee:web:keyboard-shortcut-overrides:v2';
const V1_RECORD = JSON.stringify({
	overrides: [
		{
			chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
			shortcutId: 'workbench.search.all.open',
		},
	],
	version: 1,
});
const WINDOW_OVERRIDE = {
	chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
	shortcutId: 'workbench.search.all.open',
} as const;
const FILE_BAR_SEQUENCE = [
	{ code: 'KeyK', modifiers: ['mod'] },
	{ code: 'KeyB', modifiers: [] },
] as const;

describe('keyboard shortcut storage', () => {
	it('restores one strict, sorted, canonical v1 override record', () => {
		const storage = storageWith({
			[STORAGE_KEY]: JSON.stringify({
				overrides: [
					{
						chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
						shortcutId: 'workbench.search.all.open',
					},
				],
				version: 1,
			}),
		});

		expect(readKeyboardShortcutOverrides(storage, 'standard')).toEqual({
			issue: null,
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
		['unsupported version', JSON.stringify({ overrides: [], version: 2 })],
		['unknown record field', JSON.stringify({ overrides: [], unexpected: true, version: 1 })],
		[
			'unknown shortcut ID',
			JSON.stringify({
				overrides: [{ chord: { code: 'KeyG', modifiers: ['mod'] }, shortcutId: 'unknown' }],
				version: 1,
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
				version: 1,
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
				version: 1,
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
				version: 1,
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
				version: 1,
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
				version: 1,
			}),
		],
	] as const)('rejects the whole v1 record for %s', (_case, serialized) => {
		expect(
			readKeyboardShortcutOverrides(storageWith({ [STORAGE_KEY]: serialized }), 'standard'),
		).toEqual({ issue: 'invalid-record', overrides: [] });
	});

	it('never consults older records when an invalid v1 record exists', () => {
		const getItem = vi.fn<(key: string) => string | null>((key) =>
			key === STORAGE_KEY ? '{' : JSON.stringify({ overrides: [], version: 2 }),
		);

		expect(readKeyboardShortcutOverrides({ getItem, setItem: () => undefined }, 'apple')).toEqual({
			issue: 'invalid-record',
			overrides: [],
		});
		expect(getItem).toHaveBeenCalledTimes(1);
		expect(getItem).toHaveBeenCalledWith(STORAGE_KEY);
	});

	it('reports read failures and writes one stable complete v1 record', () => {
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
		).toEqual({ issue: 'read-failed', overrides: [] });

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
			STORAGE_KEY,
			'{"overrides":[{"chord":{"code":"KeyY","modifiers":["mod","shift"]},"shortcutId":"workbench.search.all.open"}],"version":1}',
		);
	});
});

describe('keyboard shortcut steps storage', () => {
	it('keeps a release on v1 and reads v1 as one-step bindings until a v2 record exists', () => {
		const v2Record = JSON.stringify({
			overrides: [
				{ shortcutId: 'workbench.file-bar.toggle.in-file-bar', steps: FILE_BAR_SEQUENCE },
			],
			version: 2,
		});
		const getItem = vi.fn<(key: string) => string | null>((key) =>
			key === STORAGE_KEY ? V1_RECORD : v2Record,
		);
		expect(
			readKeyboardShortcutOverrides({ getItem, setItem: () => undefined }, 'standard'),
		).toEqual({ issue: null, overrides: [WINDOW_OVERRIDE] });
		expect(getItem.mock.calls).toEqual([[STORAGE_KEY]]);

		const setItem = vi.fn<(key: string, value: string) => void>();
		expect(
			readKeyboardShortcutSteps(
				{ getItem: (key) => (key === STORAGE_KEY ? V1_RECORD : null), setItem },
				'standard',
			),
		).toEqual({ issue: null, overrides: [WINDOW_OVERRIDE], scopedOverrides: [] });
		expect(setItem).not.toHaveBeenCalled();

		expect(
			readKeyboardShortcutSteps(
				storageWith({ [STEPS_STORAGE_KEY]: v2Record, [STORAGE_KEY]: V1_RECORD }),
				'standard',
			),
		).toEqual({
			issue: null,
			overrides: [],
			scopedOverrides: [
				{ sequence: FILE_BAR_SEQUENCE, shortcutId: 'workbench.file-bar.toggle.in-file-bar' },
			],
		});
	});

	it('writes every override as steps, then the window overrides alone as v1', () => {
		const setItem = vi.fn<(key: string, value: string) => void>();
		const scopedOverrides = [
			{ sequence: FILE_BAR_SEQUENCE, shortcutId: 'workbench.file-bar.toggle.in-file-bar' },
		] as const;
		expect(
			writeKeyboardShortcutSteps(
				{ getItem: () => null, setItem },
				[WINDOW_OVERRIDE],
				scopedOverrides,
			),
		).toBe(true);
		expect(setItem.mock.calls.map(([key]) => key)).toEqual([STEPS_STORAGE_KEY, STORAGE_KEY]);
		expect(setItem.mock.calls[1]?.[1]).toBe(V1_RECORD.replace(/\s/gu, ''));
		const written = new Map(setItem.mock.calls.map(([key, value]) => [key, value]));
		expect(readKeyboardShortcutSteps(storageWith(Object.fromEntries(written)), 'standard')).toEqual(
			{
				issue: null,
				overrides: [WINDOW_OVERRIDE],
				scopedOverrides,
			},
		);
	});

	it('restores defaults from a v2 record no key press could follow', () => {
		const invalidRecords = [
			// The window scope admits no sequence.
			[{ shortcutId: 'workbench.file-bar.toggle', steps: FILE_BAR_SEQUENCE }],
			// Inside the File Bar a lone Mod+K always starts a sequence.
			[{ shortcutId: 'workbench.file-bar.toggle.in-file-bar', steps: [FILE_BAR_SEQUENCE[0]] }],
			// A window shortcut has one step.
			[{ shortcutId: 'workbench.search.all.open', steps: FILE_BAR_SEQUENCE }],
			// A default is never stored.
			[
				{
					shortcutId: 'workbench.file-bar.toggle.in-file-bar',
					steps: [{ code: 'KeyB', modifiers: ['mod'] }],
				},
			],
			// The window scope holds Search actions' keys already.
			[
				{
					shortcutId: 'workbench.file-bar.toggle',
					steps: [{ code: 'KeyP', modifiers: ['mod', 'shift'] }],
				},
			],
			[
				{ shortcutId: 'workbench.file-bar.toggle.in-file-bar', steps: FILE_BAR_SEQUENCE },
				{
					shortcutId: 'workbench.file-bar.toggle',
					steps: [{ code: 'KeyJ', modifiers: ['mod', 'alt'] }],
				},
			],
		];
		for (const overrides of invalidRecords) {
			expect(
				readKeyboardShortcutSteps(
					storageWith({ [STEPS_STORAGE_KEY]: JSON.stringify({ overrides, version: 2 }) }),
					'standard',
				),
			).toEqual({ issue: 'invalid-record', overrides: [], scopedOverrides: [] });
		}
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
