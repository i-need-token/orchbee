import { describe, expect, it, vi } from 'vitest';

import {
	readKeyboardShortcutOverrides,
	writeKeyboardShortcutOverrides,
} from '../keyboard-shortcut-storage';

const VERSION_ONE_KEY = 'orchbee:web:keyboard-shortcuts:v1';
const VERSION_TWO_KEY = 'orchbee:web:keyboard-shortcuts:v2';

describe('keyboard shortcut storage', () => {
	it('restores one strict, sorted, canonical v2 override record', () => {
		const storage = storageWith({
			[VERSION_TWO_KEY]: JSON.stringify({
				overrides: [
					{
						chord: { code: 'KeyG', modifiers: ['mod', 'shift'] },
						shortcutId: 'workbench.search.all.open',
					},
				],
				version: 2,
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
		['unknown record field', JSON.stringify({ overrides: [], unexpected: true, version: 2 })],
		[
			'unknown shortcut ID',
			JSON.stringify({
				overrides: [{ chord: { code: 'KeyG', modifiers: ['mod'] }, shortcutId: 'unknown' }],
				version: 2,
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
				version: 2,
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
				version: 2,
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
				version: 2,
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
				version: 2,
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
				version: 2,
			}),
		],
	] as const)('rejects the whole v2 record for %s', (_case, serialized) => {
		expect(
			readKeyboardShortcutOverrides(storageWith({ [VERSION_TWO_KEY]: serialized }), 'standard'),
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

	it('never consults v1 when an invalid v2 record exists', () => {
		const getItem = vi.fn<(key: string) => string | null>((key) =>
			key === VERSION_TWO_KEY ? '{' : JSON.stringify({ overrides: [], version: 1 }),
		);

		expect(readKeyboardShortcutOverrides({ getItem, setItem: () => undefined }, 'apple')).toEqual({
			issue: 'invalid-record',
			migration: null,
			overrides: [],
		});
		expect(getItem).toHaveBeenCalledTimes(1);
		expect(getItem).toHaveBeenCalledWith(VERSION_TWO_KEY);
	});

	it('reports read failures and writes one stable complete v2 record', () => {
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
			VERSION_TWO_KEY,
			'{"overrides":[{"chord":{"code":"KeyG","modifiers":["mod","shift"]},"shortcutId":"workbench.search.all.open"}],"version":2}',
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
