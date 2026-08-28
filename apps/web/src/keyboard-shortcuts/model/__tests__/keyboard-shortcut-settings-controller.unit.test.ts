import { describe, expect, it, vi } from 'vitest';

import {
	KeyboardShortcutSettingsController,
	keyboardShortcutBindingSource,
} from '../keyboard-shortcut-settings-controller';

const CURRENT_STORAGE_KEY = 'orchbee:web:keyboard-shortcuts:v15';
const CURRENT_OVERRIDE_RECORD =
	'{"overrides":[{"chord":{"code":"KeyY","modifiers":["mod","shift"]},"shortcutId":"workbench.search.all.open"}],"version":15}';

describe('KeyboardShortcutSettingsController', () => {
	it('publishes one immutable effective snapshot and persists a custom binding', () => {
		const setItem = vi.fn<(key: string, value: string) => void>();
		const controller = new KeyboardShortcutSettingsController({
			platform: 'standard',
			storage: { getItem: () => null, setItem },
		});
		const listener = vi.fn<() => void>();
		controller.subscribe(listener);

		expect(
			controller.setShortcut('workbench.search.all.open', {
				code: 'KeyY',
				modifiers: ['mod', 'shift'],
			}),
		).toEqual({ kind: 'applied-and-saved' });
		expect(controller.current()['workbench.search.all.open']).toEqual({
			code: 'KeyY',
			modifiers: ['mod', 'shift'],
		});
		expect(
			keyboardShortcutBindingSource(controller.getSnapshot(), 'workbench.search.all.open'),
		).toBe('custom');
		expect(listener).toHaveBeenCalledOnce();
		expect(setItem).toHaveBeenCalledOnce();
		expect(setItem.mock.calls[0]?.[0]).toBe(CURRENT_STORAGE_KEY);
	});

	it('rejects a global cross-scope conflict without changing, writing, or publishing', () => {
		const setItem = vi.fn<(key: string, value: string) => void>();
		const controller = new KeyboardShortcutSettingsController({
			platform: 'standard',
			storage: { getItem: () => null, setItem },
		});
		const before = controller.getSnapshot();
		const listener = vi.fn<() => void>();
		controller.subscribe(listener);

		expect(
			controller.setShortcut('workbench.search.all.open', {
				code: 'KeyF',
				modifiers: ['mod', 'shift'],
			}),
		).toMatchObject({
			conflict: {
				conflictingShortcutId: 'terminal.search.open',
				shortcutId: 'workbench.search.all.open',
			},
			kind: 'conflict',
		});
		expect(controller.getSnapshot()).toBe(before);
		expect(setItem).not.toHaveBeenCalled();
		expect(listener).not.toHaveBeenCalled();
	});

	it('restores one custom item to its platform default and removes the override', () => {
		const records: string[] = [];
		const controller = new KeyboardShortcutSettingsController({
			platform: 'apple',
			storage: {
				getItem: () => null,
				setItem: (_key, value) => records.push(value),
			},
		});
		controller.setShortcut('terminal.search.open', {
			code: 'KeyG',
			modifiers: ['mod'],
		});

		expect(controller.resetShortcut('terminal.search.open')).toEqual({
			kind: 'applied-and-saved',
		});
		expect(controller.current()['terminal.search.open']).toEqual({
			code: 'KeyF',
			modifiers: ['mod'],
		});
		expect(controller.getSnapshot().overrides).toEqual([]);
		expect(records.at(-1)).toBe('{"overrides":[],"version":15}');
		expect(controller.resetShortcut('terminal.search.open')).toEqual({ kind: 'unchanged' });
	});

	it('keeps a failed write active for the session and retries on the next legal update', () => {
		let writes = 0;
		const controller = new KeyboardShortcutSettingsController({
			platform: 'standard',
			storage: {
				getItem: () => null,
				setItem: () => {
					writes += 1;
					if (writes === 1) throw new Error('quota');
				},
			},
		});
		const chord = { code: 'KeyY', modifiers: ['mod', 'shift'] as const };

		expect(controller.setShortcut('workbench.search.all.open', chord)).toEqual({
			kind: 'applied-session-only',
		});
		expect(controller.getSnapshot().persistenceIssue).toBe('write-failed');
		expect(controller.setShortcut('workbench.search.all.open', chord)).toEqual({
			kind: 'applied-and-saved',
		});
		expect(controller.getSnapshot().persistenceIssue).toBeNull();
		expect(writes).toBe(2);
	});

	it.each([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14] as const)(
		'writes a canonical v15 migration while retaining the v%s rollback record',
		(version) => {
			const legacy = JSON.stringify({
				overrides: [
					{
						chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
						shortcutId: 'workbench.search.all.open',
					},
				],
				version,
			});
			const sourceKey = `orchbee:web:keyboard-shortcuts:v${version}`;
			const setItem = vi.fn<(key: string, value: string) => void>();
			const getItem = vi.fn<(key: string) => string | null>((key) =>
				key === sourceKey ? legacy : null,
			);

			const controller = new KeyboardShortcutSettingsController({
				platform: 'standard',
				storage: { getItem, setItem },
			});

			expect(controller.getSnapshot().persistenceIssue).toBeNull();
			expect(controller.getSnapshot().overrides).toHaveLength(1);
			expect(setItem).toHaveBeenCalledWith(CURRENT_STORAGE_KEY, CURRENT_OVERRIDE_RECORD);
			expect(getItem).toHaveBeenCalledWith(sourceKey);
			expect(setItem).not.toHaveBeenCalledWith(sourceKey, expect.anything());
		},
	);

	it('falls back after an invalid stored record and stops writes and notifications after disposal', () => {
		const setItem = vi.fn<(key: string, value: string) => void>();
		const controller = new KeyboardShortcutSettingsController({
			platform: 'apple',
			storage: { getItem: () => '{', setItem },
		});
		expect(controller.getSnapshot().persistenceIssue).toBe('invalid-record');
		expect(controller.current()['workbench.search.all.open']).toEqual({
			code: 'KeyF',
			modifiers: ['mod', 'shift'],
		});
		const listener = vi.fn<() => void>();
		controller.subscribe(listener);
		controller.dispose();

		expect(
			controller.setShortcut('workbench.search.all.open', {
				code: 'KeyG',
				modifiers: ['mod'],
			}),
		).toEqual({ kind: 'disposed' });
		expect(setItem).not.toHaveBeenCalled();
		expect(listener).not.toHaveBeenCalled();
	});
});
