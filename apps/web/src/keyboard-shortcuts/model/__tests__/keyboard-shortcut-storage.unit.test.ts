import { describe, expect, it, vi } from 'vitest';

import {
	readKeyboardShortcutOverrides,
	writeKeyboardShortcutOverrides,
} from '../keyboard-shortcut-storage';

const STORAGE_KEY = 'orchbee:web:keyboard-shortcut-overrides:v1';

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

function storageWith(records: Readonly<Record<string, string>>): {
	readonly getItem: (key: string) => string | null;
	readonly setItem: (key: string, value: string) => void;
} {
	return {
		getItem: (key) => records[key] ?? null,
		setItem: () => undefined,
	};
}
