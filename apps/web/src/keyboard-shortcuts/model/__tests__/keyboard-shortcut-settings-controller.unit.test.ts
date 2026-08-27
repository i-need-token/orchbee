import { describe, expect, it, vi } from 'vitest';

import {
	KeyboardShortcutSettingsController,
	keyboardShortcutBindingSource,
} from '../keyboard-shortcut-settings-controller';

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
				code: 'KeyG',
				modifiers: ['mod', 'shift'],
			}),
		).toEqual({ kind: 'applied-and-saved' });
		expect(controller.current()['workbench.search.all.open']).toEqual({
			code: 'KeyG',
			modifiers: ['mod', 'shift'],
		});
		expect(
			keyboardShortcutBindingSource(controller.getSnapshot(), 'workbench.search.all.open'),
		).toBe('custom');
		expect(listener).toHaveBeenCalledOnce();
		expect(setItem).toHaveBeenCalledOnce();
		expect(setItem.mock.calls[0]?.[0]).toBe('orchbee:web:keyboard-shortcuts:v7');
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
		expect(records.at(-1)).toBe('{"overrides":[],"version":7}');
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
		const chord = { code: 'KeyG', modifiers: ['mod', 'shift'] as const };

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

	it('writes a canonical v7 migration while retaining the v1 rollback record', () => {
		const versionOne = JSON.stringify({
			overrides: [
				{
					chord: { code: 'KeyG', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 1,
		});
		const setItem = vi.fn<(key: string, value: string) => void>();
		const getItem = vi.fn<(key: string) => string | null>((key) =>
			key === 'orchbee:web:keyboard-shortcuts:v1' ? versionOne : null,
		);

		const controller = new KeyboardShortcutSettingsController({
			platform: 'standard',
			storage: { getItem, setItem },
		});

		expect(controller.getSnapshot().persistenceIssue).toBeNull();
		expect(controller.getSnapshot().overrides).toHaveLength(1);
		expect(setItem).toHaveBeenCalledWith(
			'orchbee:web:keyboard-shortcuts:v7',
			'{"overrides":[{"chord":{"code":"KeyG","modifiers":["mod","shift"]},"shortcutId":"workbench.search.all.open"}],"version":7}',
		);
		expect(getItem).toHaveBeenCalledWith('orchbee:web:keyboard-shortcuts:v1');
		expect(setItem).not.toHaveBeenCalledWith(
			'orchbee:web:keyboard-shortcuts:v1',
			expect.anything(),
		);
	});

	it('writes a canonical v7 migration while retaining the v2 rollback record', () => {
		const versionTwo = JSON.stringify({
			overrides: [
				{
					chord: { code: 'KeyG', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 2,
		});
		const setItem = vi.fn<(key: string, value: string) => void>();
		const getItem = vi.fn<(key: string) => string | null>((key) =>
			key === 'orchbee:web:keyboard-shortcuts:v2' ? versionTwo : null,
		);

		const controller = new KeyboardShortcutSettingsController({
			platform: 'standard',
			storage: { getItem, setItem },
		});

		expect(controller.getSnapshot().persistenceIssue).toBeNull();
		expect(controller.getSnapshot().overrides).toHaveLength(1);
		expect(setItem).toHaveBeenCalledWith(
			'orchbee:web:keyboard-shortcuts:v7',
			'{"overrides":[{"chord":{"code":"KeyG","modifiers":["mod","shift"]},"shortcutId":"workbench.search.all.open"}],"version":7}',
		);
		expect(getItem).toHaveBeenCalledWith('orchbee:web:keyboard-shortcuts:v2');
		expect(setItem).not.toHaveBeenCalledWith(
			'orchbee:web:keyboard-shortcuts:v2',
			expect.anything(),
		);
	});

	it('writes a canonical v7 migration while retaining the v3 rollback record', () => {
		const versionThree = JSON.stringify({
			overrides: [
				{
					chord: { code: 'KeyG', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 3,
		});
		const setItem = vi.fn<(key: string, value: string) => void>();
		const getItem = vi.fn<(key: string) => string | null>((key) =>
			key === 'orchbee:web:keyboard-shortcuts:v3' ? versionThree : null,
		);

		const controller = new KeyboardShortcutSettingsController({
			platform: 'standard',
			storage: { getItem, setItem },
		});

		expect(controller.getSnapshot().persistenceIssue).toBeNull();
		expect(controller.getSnapshot().overrides).toHaveLength(1);
		expect(setItem).toHaveBeenCalledWith(
			'orchbee:web:keyboard-shortcuts:v7',
			'{"overrides":[{"chord":{"code":"KeyG","modifiers":["mod","shift"]},"shortcutId":"workbench.search.all.open"}],"version":7}',
		);
		expect(getItem).toHaveBeenCalledWith('orchbee:web:keyboard-shortcuts:v3');
		expect(setItem).not.toHaveBeenCalledWith(
			'orchbee:web:keyboard-shortcuts:v3',
			expect.anything(),
		);
	});

	it('writes a canonical v7 migration while retaining the v4 rollback record', () => {
		const versionFour = JSON.stringify({
			overrides: [
				{
					chord: { code: 'KeyG', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 4,
		});
		const setItem = vi.fn<(key: string, value: string) => void>();
		const getItem = vi.fn<(key: string) => string | null>((key) =>
			key === 'orchbee:web:keyboard-shortcuts:v4' ? versionFour : null,
		);

		const controller = new KeyboardShortcutSettingsController({
			platform: 'standard',
			storage: { getItem, setItem },
		});

		expect(controller.getSnapshot().persistenceIssue).toBeNull();
		expect(controller.getSnapshot().overrides).toHaveLength(1);
		expect(setItem).toHaveBeenCalledWith(
			'orchbee:web:keyboard-shortcuts:v7',
			'{"overrides":[{"chord":{"code":"KeyG","modifiers":["mod","shift"]},"shortcutId":"workbench.search.all.open"}],"version":7}',
		);
		expect(getItem).toHaveBeenCalledWith('orchbee:web:keyboard-shortcuts:v4');
		expect(setItem).not.toHaveBeenCalledWith(
			'orchbee:web:keyboard-shortcuts:v4',
			expect.anything(),
		);
	});

	it('writes a canonical v7 migration while retaining the v5 rollback record', () => {
		const versionFive = JSON.stringify({
			overrides: [
				{
					chord: { code: 'KeyG', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 5,
		});
		const setItem = vi.fn<(key: string, value: string) => void>();
		const getItem = vi.fn<(key: string) => string | null>((key) =>
			key === 'orchbee:web:keyboard-shortcuts:v5' ? versionFive : null,
		);

		const controller = new KeyboardShortcutSettingsController({
			platform: 'standard',
			storage: { getItem, setItem },
		});

		expect(controller.getSnapshot().persistenceIssue).toBeNull();
		expect(controller.getSnapshot().overrides).toHaveLength(1);
		expect(setItem).toHaveBeenCalledWith(
			'orchbee:web:keyboard-shortcuts:v7',
			'{"overrides":[{"chord":{"code":"KeyG","modifiers":["mod","shift"]},"shortcutId":"workbench.search.all.open"}],"version":7}',
		);
		expect(getItem).toHaveBeenCalledWith('orchbee:web:keyboard-shortcuts:v5');
		expect(setItem).not.toHaveBeenCalledWith(
			'orchbee:web:keyboard-shortcuts:v5',
			expect.anything(),
		);
	});

	it('writes a canonical v7 migration while retaining the v6 rollback record', () => {
		const versionSix = JSON.stringify({
			overrides: [
				{
					chord: { code: 'KeyG', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 6,
		});
		const setItem = vi.fn<(key: string, value: string) => void>();
		const getItem = vi.fn<(key: string) => string | null>((key) =>
			key === 'orchbee:web:keyboard-shortcuts:v6' ? versionSix : null,
		);

		const controller = new KeyboardShortcutSettingsController({
			platform: 'standard',
			storage: { getItem, setItem },
		});

		expect(controller.getSnapshot().persistenceIssue).toBeNull();
		expect(controller.getSnapshot().overrides).toHaveLength(1);
		expect(setItem).toHaveBeenCalledWith(
			'orchbee:web:keyboard-shortcuts:v7',
			'{"overrides":[{"chord":{"code":"KeyG","modifiers":["mod","shift"]},"shortcutId":"workbench.search.all.open"}],"version":7}',
		);
		expect(getItem).toHaveBeenCalledWith('orchbee:web:keyboard-shortcuts:v6');
		expect(setItem).not.toHaveBeenCalledWith(
			'orchbee:web:keyboard-shortcuts:v6',
			expect.anything(),
		);
	});

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
