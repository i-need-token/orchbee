// @vitest-environment jsdom

import type { TerminalPresentationConfiguration } from '@orchbee/terminal';
import { describe, expect, it, vi } from 'vitest';

import { TerminalSettingsController } from '../TerminalSettingsController';
import { DEFAULT_TERMINAL_APPEARANCE_PREFERENCES } from '../terminal-appearance-preferences';
import type { TerminalAppearanceStorage } from '../terminal-appearance-preferences-storage';
import type {
	TerminalThemePresentationCommit,
	TerminalThemePresentationParticipant,
	TerminalThemePresentationPreparation,
} from '../isTerminalThemePresentationSource';

interface StorageFixture extends TerminalAppearanceStorage {
	readonly getItem: ReturnType<typeof vi.fn<TerminalAppearanceStorage['getItem']>>;
	readonly setItem: ReturnType<typeof vi.fn<TerminalAppearanceStorage['setItem']>>;
}

describe('TerminalSettingsController', () => {
	it('starts from the product default without writing a missing record', () => {
		const storage = createStorage(null);
		const target = document.createElement('div');
		const controller = new TerminalSettingsController({ storage, themeTarget: target });

		expect(controller.getSnapshot()).toEqual({
			open: false,
			persistenceIssue: null,
			preferences: DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
		});
		expect(controller.current()).toMatchObject({
			allowTransparency: false,
			backgroundOpacity: 1,
			defaultColorOverride: null,
			fontFamily: '"Orchbee JetBrains Mono", "Orchbee Symbols Nerd Font", monospace',
			fontSize: 13,
		});
		expect(target.dataset['theme']).toBe('light');
		expect(storage.setItem).not.toHaveBeenCalled();
	});

	it('restores and normalizes one complete valid record', () => {
		const storage = createStorage(
			JSON.stringify({
				applicationTheme: 'dark',
				backgroundOpacityPercent: 65,
				customBackground: '#ABCDEF',
				customForeground: '#0123AB',
				fontFamily: '  Example Mono, monospace  ',
				fontSize: 19,
				terminalDisplayTheme: 'custom',
			}),
		);
		const target = document.createElement('div');
		const controller = new TerminalSettingsController({ storage, themeTarget: target });

		expect(controller.getSnapshot().preferences).toEqual({
			applicationTheme: 'dark',
			backgroundOpacityPercent: 65,
			customBackground: '#abcdef',
			customForeground: '#0123ab',
			fontFamily: 'Example Mono, monospace',
			fontSize: 19,
			terminalDisplayTheme: 'custom',
		});
		expect(controller.current()).toMatchObject({
			allowTransparency: true,
			backgroundOpacity: 0.65,
			defaultColorOverride: null,
			fontFamily: 'Example Mono, monospace',
			fontSize: 19,
		});
		expect(target.dataset['theme']).toBe('dark');
	});

	it.each([
		['malformed JSON', '{'],
		[
			'unknown fields',
			JSON.stringify({ ...DEFAULT_TERMINAL_APPEARANCE_PREFERENCES, unknown: true }),
		],
		['an old partial shape', JSON.stringify({ applicationTheme: 'dark', fontFamily: 'monospace' })],
		[
			'an out-of-range value',
			JSON.stringify({ ...DEFAULT_TERMINAL_APPEARANCE_PREFERENCES, fontSize: 49 }),
		],
	])('falls back completely for %s', (_name, serialized) => {
		const controller = new TerminalSettingsController({
			storage: createStorage(serialized),
			themeTarget: document.createElement('div'),
		});

		expect(controller.getSnapshot()).toMatchObject({
			persistenceIssue: 'invalid-record',
			preferences: DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
		});
	});

	it('falls back completely when storage cannot be read', () => {
		const storage = createStorage(null);
		storage.getItem.mockImplementation(() => {
			throw new Error('injected read failure');
		});
		const controller = new TerminalSettingsController({
			storage,
			themeTarget: document.createElement('div'),
		});

		expect(controller.getSnapshot()).toMatchObject({
			persistenceIssue: 'read-failed',
			preferences: DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
		});
	});

	it('keeps application theme isolated from terminal presentation publication', () => {
		const storage = createStorage(null);
		const target = document.createElement('div');
		const controller = new TerminalSettingsController({ storage, themeTarget: target });
		const presentationListener =
			vi.fn<(configuration: TerminalPresentationConfiguration) => void>();
		controller.subscribe(presentationListener);

		expect(controller.update({ applicationTheme: 'dark' })).toEqual({
			kind: 'applied-and-saved',
		});
		expect(target.dataset['theme']).toBe('dark');
		expect(presentationListener).not.toHaveBeenCalled();
		expect(storage.setItem).toHaveBeenCalledOnce();
	});

	it('publishes one complete presentation for one terminal-derived change', () => {
		const storage = createStorage(null);
		const target = document.createElement('div');
		const controller = new TerminalSettingsController({ storage, themeTarget: target });
		const throwingListener = vi.fn<(configuration: TerminalPresentationConfiguration) => void>(
			() => {
				throw new Error('injected subscriber failure');
			},
		);
		const presentationListener =
			vi.fn<(configuration: TerminalPresentationConfiguration) => void>();
		controller.subscribe(throwingListener);
		controller.subscribe(presentationListener);

		expect(
			controller.update({
				backgroundOpacityPercent: 75,
				terminalDisplayTheme: 'dark',
			}),
		).toEqual({ kind: 'applied-and-saved' });
		expect(presentationListener).toHaveBeenCalledOnce();
		expect(presentationListener).toHaveBeenCalledWith(controller.current());
		expect(controller.current()).toMatchObject({
			allowTransparency: true,
			backgroundOpacity: 0.75,
			defaultColorOverride: null,
		});
		expect(target.dataset['theme']).toBe('light');
	});

	it('changes the app surface only after every terminal can commit in the same task', async () => {
		const target = document.createElement('div');
		const controller = new TerminalSettingsController({
			storage: createStorage(null),
			themeTarget: target,
		});
		const first = deferred<TerminalThemePresentationPreparation>();
		const second = deferred<TerminalThemePresentationPreparation>();
		const commitObservations: string[] = [];
		const firstCommit = themeCommit(() =>
			commitObservations.push(`first:${target.dataset['theme']}`),
		);
		const secondCommit = themeCommit(() =>
			commitObservations.push(`second:${target.dataset['theme']}`),
		);
		const firstParticipant = participantForDark(first.promise);
		const secondParticipant = participantForDark(second.promise);
		controller.registerThemeParticipant(firstParticipant);
		controller.registerThemeParticipant(secondParticipant);
		await Promise.resolve();

		expect(controller.update({ applicationTheme: 'dark' })).toEqual({
			kind: 'applied-and-saved',
		});
		expect(target.dataset['theme']).toBe('light');
		first.resolve({ commit: firstCommit.commit, kind: 'prepared' });
		await Promise.resolve();
		expect(target.dataset['theme']).toBe('light');
		second.resolve({ commit: secondCommit.commit, kind: 'prepared' });
		await vi.waitFor(() => expect(target.dataset['theme']).toBe('dark'));

		expect(commitObservations).toEqual(['first:light', 'second:light']);
		expect(firstCommit.commit.commit).toHaveBeenCalledOnce();
		expect(secondCommit.commit.commit).toHaveBeenCalledOnce();
	});

	it('aborts prepared terminals and preserves the app surface when one renderer fails', async () => {
		const target = document.createElement('div');
		const controller = new TerminalSettingsController({
			storage: createStorage(null),
			themeTarget: target,
		});
		const prepared = themeCommit();
		controller.registerThemeParticipant(
			participantForDark(Promise.resolve({ commit: prepared.commit, kind: 'prepared' })),
		);
		controller.registerThemeParticipant(
			participantForDark(
				Promise.resolve({
					code: 'renderer-preparation-failed',
					kind: 'failed',
					retryable: true,
				}),
			),
		);
		await Promise.resolve();

		controller.update({ applicationTheme: 'dark' });
		await vi.waitFor(() => expect(prepared.commit.abort).toHaveBeenCalledOnce());
		expect(prepared.commit.commit).not.toHaveBeenCalled();
		expect(target.dataset['theme']).toBe('light');
	});

	it('queues a re-entrant presentation update after the current delivery and persists only the latest state', () => {
		const storage = createStorage(null);
		const controller = new TerminalSettingsController({
			storage,
			themeTarget: document.createElement('div'),
		});
		let reentered = false;
		const reentrantDeliveries: number[] = [];
		const observingDeliveries: number[] = [];
		const reentrantListener = vi.fn<(configuration: TerminalPresentationConfiguration) => void>(
			() => {
				reentrantDeliveries.push(controller.current().fontSize);
				if (reentered) return;
				reentered = true;
				expect(controller.getSnapshot().preferences.fontSize).toBe(14);
				expect(controller.update({ fontSize: 15 })).toEqual({ kind: 'applied-and-saved' });
			},
		);
		const observingListener = vi.fn<(configuration: TerminalPresentationConfiguration) => void>(
			(configuration) => {
				observingDeliveries.push(configuration.fontSize);
			},
		);
		controller.subscribe(reentrantListener);
		controller.subscribe(observingListener);

		expect(controller.update({ fontSize: 14 })).toEqual({ kind: 'applied-and-saved' });
		expect(reentrantDeliveries).toEqual([14, 15]);
		expect(observingDeliveries).toEqual([14, 15]);
		expect(controller.getSnapshot().preferences.fontSize).toBe(15);
		expect(controller.current().fontSize).toBe(15);
		expect(storage.setItem).toHaveBeenCalledOnce();
		const serialized = storage.setItem.mock.calls[0]?.[1];
		if (serialized === undefined) throw new Error('The latest preferences were not written.');
		expect(JSON.parse(serialized)).toMatchObject({ fontSize: 15 });
	});

	it('drains a storage-reentrant update after the older storage call returns', () => {
		let controller: TerminalSettingsController | null = null;
		let stored = '';
		let reentered = false;
		const storage: StorageFixture = {
			getItem: vi.fn<TerminalAppearanceStorage['getItem']>(() => null),
			setItem: vi.fn<TerminalAppearanceStorage['setItem']>((_key, value) => {
				if (!reentered) {
					reentered = true;
					if (controller === null) throw new Error('Controller was not initialized.');
					controller.update({ fontSize: 15 });
				}
				stored = value;
			}),
		};
		controller = new TerminalSettingsController({
			storage,
			themeTarget: document.createElement('div'),
		});

		expect(controller.update({ fontSize: 14 })).toEqual({ kind: 'applied-and-saved' });
		expect(storage.setItem).toHaveBeenCalledTimes(2);
		expect(controller.getSnapshot().preferences.fontSize).toBe(15);
		expect(controller.current().fontSize).toBe(15);
		expect(JSON.parse(stored)).toMatchObject({ fontSize: 15 });
	});

	it('stops the current publication and persistence when a listener disposes the controller', () => {
		const storage = createStorage(null);
		const controller = new TerminalSettingsController({
			storage,
			themeTarget: document.createElement('div'),
		});
		const secondListener = vi.fn<(configuration: TerminalPresentationConfiguration) => void>();
		controller.subscribe(() => controller.dispose());
		controller.subscribe(secondListener);

		expect(controller.update({ fontSize: 14 })).toEqual({ kind: 'disposed' });
		expect(secondListener).not.toHaveBeenCalled();
		expect(storage.setItem).not.toHaveBeenCalled();
	});

	it('retains custom colors outside Custom mode without a presentation publication', () => {
		const controller = new TerminalSettingsController({
			storage: createStorage(null),
			themeTarget: document.createElement('div'),
		});
		const presentationListener =
			vi.fn<(configuration: TerminalPresentationConfiguration) => void>();
		controller.subscribe(presentationListener);

		expect(controller.update({ customForeground: '#abcdef' })).toEqual({
			kind: 'applied-and-saved',
		});
		expect(presentationListener).not.toHaveBeenCalled();
		expect(controller.current().defaultColorOverride).toBeNull();
	});

	it('rejects an invalid candidate without changing any external state', () => {
		const storage = createStorage(null);
		const target = document.createElement('div');
		const controller = new TerminalSettingsController({ storage, themeTarget: target });
		const initialSnapshot = controller.getSnapshot();
		const initialPresentation = controller.current();
		const presentationListener =
			vi.fn<(configuration: TerminalPresentationConfiguration) => void>();
		const snapshotListener = vi.fn<() => void>();
		controller.subscribe(presentationListener);
		controller.subscribeSnapshot(snapshotListener);

		expect(controller.update({ fontSize: 49 })).toEqual({ kind: 'invalid' });
		expect(controller.getSnapshot()).toBe(initialSnapshot);
		expect(controller.current()).toBe(initialPresentation);
		expect(target.dataset['theme']).toBe('light');
		expect(storage.setItem).not.toHaveBeenCalled();
		expect(presentationListener).not.toHaveBeenCalled();
		expect(snapshotListener).not.toHaveBeenCalled();
	});

	it('keeps a valid update for the session after a write failure and clears the issue on retry', () => {
		const storage = createStorage(null);
		storage.setItem.mockImplementationOnce(() => {
			throw new Error('injected write failure');
		});
		const controller = new TerminalSettingsController({
			storage,
			themeTarget: document.createElement('div'),
		});
		const presentationListener =
			vi.fn<(configuration: TerminalPresentationConfiguration) => void>();
		controller.subscribe(presentationListener);

		expect(controller.update({ fontSize: 14 })).toEqual({ kind: 'applied-session-only' });
		expect(controller.getSnapshot()).toMatchObject({
			persistenceIssue: 'write-failed',
			preferences: { fontSize: 14 },
		});
		expect(controller.current().fontSize).toBe(14);
		expect(presentationListener).toHaveBeenCalledOnce();

		expect(controller.update({ applicationTheme: 'dark' })).toEqual({
			kind: 'applied-and-saved',
		});
		expect(controller.getSnapshot().persistenceIssue).toBeNull();
		expect(storage.setItem).toHaveBeenCalledTimes(2);
		expect(presentationListener).toHaveBeenCalledOnce();
	});

	it('resets changed values, repairs an invalid default, and no-ops when healthy', () => {
		const changedStorage = createStorage(
			JSON.stringify({
				...DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
				applicationTheme: 'dark',
				fontSize: 18,
			}),
		);
		const changed = new TerminalSettingsController({
			storage: changedStorage,
			themeTarget: document.createElement('div'),
		});
		const changedPresentation = vi.fn<(configuration: TerminalPresentationConfiguration) => void>();
		changed.subscribe(changedPresentation);
		expect(changed.reset()).toEqual({ kind: 'applied-and-saved' });
		expect(changed.getSnapshot().preferences).toEqual(DEFAULT_TERMINAL_APPEARANCE_PREFERENCES);
		expect(changedStorage.setItem).toHaveBeenCalledOnce();
		expect(changedPresentation).toHaveBeenCalledOnce();

		const repairStorage = createStorage('{');
		const repair = new TerminalSettingsController({
			storage: repairStorage,
			themeTarget: document.createElement('div'),
		});
		const repairPresentation = vi.fn<(configuration: TerminalPresentationConfiguration) => void>();
		repair.subscribe(repairPresentation);
		expect(repair.reset()).toEqual({ kind: 'applied-and-saved' });
		expect(repairStorage.setItem).toHaveBeenCalledOnce();
		expect(repair.getSnapshot().persistenceIssue).toBeNull();
		expect(repairPresentation).not.toHaveBeenCalled();

		const healthyStorage = createStorage(null);
		const healthy = new TerminalSettingsController({
			storage: healthyStorage,
			themeTarget: document.createElement('div'),
		});
		expect(healthy.reset()).toEqual({ kind: 'unchanged' });
		expect(healthyStorage.setItem).not.toHaveBeenCalled();
	});

	it('opens and closes idempotently without publishing a terminal configuration', () => {
		const controller = new TerminalSettingsController({
			storage: createStorage(null),
			themeTarget: document.createElement('div'),
		});
		const snapshotListener = vi.fn<() => void>();
		const presentationListener =
			vi.fn<(configuration: TerminalPresentationConfiguration) => void>();
		controller.subscribeSnapshot(snapshotListener);
		controller.subscribe(presentationListener);

		expect(controller.open()).toEqual({ kind: 'changed' });
		expect(controller.open()).toEqual({ kind: 'unchanged' });
		expect(controller.getSnapshot().open).toBe(true);
		expect(controller.close()).toEqual({ kind: 'changed' });
		expect(controller.close()).toEqual({ kind: 'unchanged' });
		expect(snapshotListener).toHaveBeenCalledTimes(2);
		expect(presentationListener).not.toHaveBeenCalled();
	});

	it('rejects late updates and stops notifying after disposal', () => {
		const storage = createStorage(null);
		const target = document.createElement('div');
		const controller = new TerminalSettingsController({ storage, themeTarget: target });
		const snapshotListener = vi.fn<() => void>();
		const presentationListener =
			vi.fn<(configuration: TerminalPresentationConfiguration) => void>();
		controller.subscribeSnapshot(snapshotListener);
		controller.subscribe(presentationListener);
		controller.dispose();

		expect(controller.update({ fontSize: 14 })).toEqual({ kind: 'disposed' });
		expect(controller.open()).toEqual({ kind: 'disposed' });
		expect(controller.close()).toEqual({ kind: 'disposed' });
		expect(storage.setItem).not.toHaveBeenCalled();
		expect(target.dataset['theme']).toBe('light');
		expect(snapshotListener).not.toHaveBeenCalled();
		expect(presentationListener).not.toHaveBeenCalled();
	});
});

function createStorage(serialized: string | null): StorageFixture {
	return {
		getItem: vi.fn<TerminalAppearanceStorage['getItem']>(() => serialized),
		setItem: vi.fn<TerminalAppearanceStorage['setItem']>(() => undefined),
	};
}

function deferred<T>(): {
	readonly promise: Promise<T>;
	readonly resolve: (value: T) => void;
} {
	let resolve: ((value: T) => void) | undefined;
	const promise = new Promise<T>((settle) => {
		resolve = settle;
	});
	if (resolve === undefined) throw new Error('Deferred result was not initialized synchronously.');
	return { promise, resolve };
}

function participantForDark(
	darkPreparation: Promise<TerminalThemePresentationPreparation>,
): TerminalThemePresentationParticipant {
	return {
		prepareThemePresentation: (preferences) =>
			preferences.applicationTheme === 'dark'
				? darkPreparation
				: Promise.resolve({ kind: 'cancelled' }),
	};
}

function themeCommit(onCommit: () => void = () => undefined): {
	readonly commit: TerminalThemePresentationCommit & {
		readonly abort: ReturnType<typeof vi.fn<TerminalThemePresentationCommit['abort']>>;
		readonly commit: ReturnType<typeof vi.fn<TerminalThemePresentationCommit['commit']>>;
	};
} {
	return {
		commit: {
			abort: vi.fn<TerminalThemePresentationCommit['abort']>(),
			canCommit: () => true,
			commit: vi.fn<TerminalThemePresentationCommit['commit']>(onCommit),
		},
	};
}
