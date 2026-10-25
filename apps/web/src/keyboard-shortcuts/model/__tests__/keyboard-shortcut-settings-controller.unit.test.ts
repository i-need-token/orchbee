import { describe, expect, it, vi } from 'vitest';

import {
	KeyboardShortcutSettingsController,
	keyboardShortcutBindingSource,
} from '../keyboard-shortcut-settings-controller';

const CURRENT_STORAGE_KEY = 'orchbee:web:keyboard-shortcut-overrides:v1';

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
		expect(records.at(-1)).toBe('{"overrides":[],"version":1}');
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

	it('commits before storage and drains a reentrant edit without losing either binding', () => {
		let controller: KeyboardShortcutSettingsController | undefined;
		let stored = '';
		let depth = 0;
		let maximumDepth = 0;
		let observedCode: string | undefined;
		let nestedResult: unknown;
		let writes = 0;
		controller = new KeyboardShortcutSettingsController({
			platform: 'standard',
			storage: {
				getItem: () => null,
				setItem: (_key, value) => {
					depth += 1;
					maximumDepth = Math.max(maximumDepth, depth);
					writes += 1;
					if (writes === 1) {
						observedCode = controller?.current()['workbench.search.all.open'].code;
						nestedResult = controller?.setShortcut('terminal.search.open', {
							code: 'KeyU',
							modifiers: ['mod', 'alt', 'shift'],
						});
					}
					stored = value;
					depth -= 1;
				},
			},
		});

		expect(
			controller.setShortcut('workbench.search.all.open', {
				code: 'KeyY',
				modifiers: ['mod', 'shift'],
			}),
		).toEqual({ kind: 'applied-and-saved' });
		expect(observedCode).toBe('KeyY');
		expect(nestedResult).toEqual({ kind: 'applied-session-only' });
		expect(maximumDepth).toBe(1);
		expect(writes).toBe(2);
		expect(controller.current()['terminal.search.open'].code).toBe('KeyU');
		expect(controller.current()['workbench.search.all.open'].code).toBe('KeyY');
		expect(JSON.parse(stored)).toEqual({
			overrides: [
				{
					chord: { code: 'KeyU', modifiers: ['mod', 'alt', 'shift'] },
					shortcutId: 'terminal.search.open',
				},
				{
					chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
					shortcutId: 'workbench.search.all.open',
				},
			],
			version: 1,
		});
	});

	it('keeps a newer write failure visible after the older storage call succeeds', () => {
		let controller: KeyboardShortcutSettingsController | undefined;
		let writes = 0;
		controller = new KeyboardShortcutSettingsController({
			platform: 'standard',
			storage: {
				getItem: () => null,
				setItem: () => {
					writes += 1;
					if (writes > 1) throw new Error('Latest preference write failed.');
					controller?.resetShortcut('workbench.search.all.open');
				},
			},
		});

		expect(
			controller.setShortcut('workbench.search.all.open', {
				code: 'KeyY',
				modifiers: ['mod', 'shift'],
			}),
		).toEqual({ kind: 'applied-session-only' });
		expect(writes).toBe(2);
		expect(controller.getSnapshot()).toMatchObject({
			overrides: [],
			persistenceIssue: 'write-failed',
		});
	});

	it('does not publish a storage completion after disposal', () => {
		let controller: KeyboardShortcutSettingsController | undefined;
		let retiredSnapshot: unknown;
		controller = new KeyboardShortcutSettingsController({
			platform: 'standard',
			storage: {
				getItem: () => null,
				setItem: () => {
					controller?.dispose();
					retiredSnapshot = controller?.getSnapshot();
					throw new Error('Storage failed after retirement.');
				},
			},
		});
		const listener = vi.fn<() => void>();
		controller.subscribe(listener);

		expect(
			controller.setShortcut('workbench.search.all.open', {
				code: 'KeyY',
				modifiers: ['mod', 'shift'],
			}),
		).toEqual({ kind: 'disposed' });
		expect(controller.getSnapshot()).toBe(retiredSnapshot);
		expect(controller.current()['workbench.search.all.open'].code).toBe('KeyY');
		expect(listener).not.toHaveBeenCalled();
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
