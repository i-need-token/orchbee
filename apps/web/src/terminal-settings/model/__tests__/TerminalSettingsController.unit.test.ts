// @vitest-environment jsdom

import {
	orchbeeTerminalColorTheme,
	type TerminalPresentationConfiguration,
} from '@orchbee/terminal';
import { describe, expect, it, vi } from 'vitest';

import {
	colorThemesWithFixtures,
	fixtureDarkTheme,
	fixtureLightTheme,
} from '../../../__tests__/color-theme-fixtures';
import { memoryUserSettings } from '../../../__tests__/memoryUserSettings';
import { TerminalSettingsController } from '../TerminalSettingsController';
import { DEFAULT_TERMINAL_APPEARANCE_PREFERENCES } from '../terminal-appearance-preferences';
import type {
	TerminalThemePresentationCommit,
	TerminalThemePresentationParticipant,
	TerminalThemePresentationPreparation,
} from '../isTerminalThemePresentationSource';

describe('TerminalSettingsController', () => {
	it('starts from the product default without writing a missing record', () => {
		const settings = memoryUserSettings();
		const edit = vi.spyOn(settings, 'edit');
		const target = document.createElement('div');
		const controller = new TerminalSettingsController({
			themeTarget: target,
			userSettings: settings,
		});

		expect(controller.getSnapshot()).toEqual({
			landing: null,
			open: false,
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
		expect(edit).not.toHaveBeenCalled();
	});

	it('reads the appearance from the settings file', () => {
		const target = document.createElement('div');
		const controller = new TerminalSettingsController({
			themeTarget: target,
			userSettings: memoryUserSettings({
				values: {
					'appearance.custom_background': '#abcdef',
					'appearance.custom_foreground': '#0123ab',
					'appearance.mode': 'dark',
					'appearance.terminal_colors': 'custom',
					'terminal.background_opacity': 65,
					'terminal.font_family': '  Example Mono, monospace  ',
					'terminal.font_size': 19,
				},
			}),
		});

		expect(controller.getSnapshot().preferences).toEqual({
			...DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
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

	it('keeps application theme isolated from terminal presentation publication', async () => {
		const target = document.createElement('div');
		const controller = new TerminalSettingsController({
			themeTarget: target,
			userSettings: memoryUserSettings(),
		});
		const presentationListener =
			vi.fn<(configuration: TerminalPresentationConfiguration) => void>();
		controller.subscribe(presentationListener);

		expect(controller.update({ applicationTheme: 'dark' })).toEqual({ kind: 'pending' });
		await Promise.resolve();
		expect(target.dataset['theme']).toBe('dark');
		expect(presentationListener).not.toHaveBeenCalled();
	});

	it('marks only a named theme of the active slot on the theme boundary', async () => {
		const target = document.createElement('div');
		const controller = new TerminalSettingsController({
			colorThemes: colorThemesWithFixtures,
			themeTarget: target,
			userSettings: memoryUserSettings({
				values: { 'appearance.dark_theme': fixtureDarkTheme.id, 'appearance.mode': 'light' },
			}),
		});
		const participant = vi.fn<TerminalThemePresentationParticipant['prepareThemePresentation']>(
			async () => ({ commit: themeCommit().commit, kind: 'prepared' }),
		);
		controller.registerThemeParticipant({ prepareThemePresentation: participant });
		// The light slot holds the Orchbee theme: no named theme attribute.
		await vi.waitFor(() => expect({ ...target.dataset }).toEqual({ theme: 'light' }));

		controller.update({ applicationTheme: 'dark' });
		await vi.waitFor(() =>
			expect({ ...target.dataset }).toEqual({ colorTheme: fixtureDarkTheme.id, theme: 'dark' }),
		);
		expect(participant).toHaveBeenLastCalledWith(
			expect.objectContaining({ applicationTheme: 'dark', colorTheme: fixtureDarkTheme }),
		);

		// An id this build does not ship falls back to the Orchbee theme of the slot's polarity, and
		// so does a theme of the other polarity stored in the slot.
		controller.update({ darkTheme: 'not-shipped' });
		await vi.waitFor(() => expect({ ...target.dataset }).toEqual({ theme: 'dark' }));
		expect(participant).toHaveBeenLastCalledWith(
			expect.objectContaining({ colorTheme: orchbeeTerminalColorTheme('dark') }),
		);
		controller.update({ darkTheme: fixtureDarkTheme.id });
		await vi.waitFor(() => expect(target.dataset['colorTheme']).toBe(fixtureDarkTheme.id));
		controller.update({ darkTheme: fixtureLightTheme.id });
		await vi.waitFor(() => expect({ ...target.dataset }).toEqual({ theme: 'dark' }));
		expect(participant).toHaveBeenLastCalledWith(
			expect.objectContaining({ colorTheme: orchbeeTerminalColorTheme('dark') }),
		);
		controller.dispose();
	});

	it('previews a theme at its polarity without saving, and cancelling or closing restores', () => {
		const target = document.createElement('div');
		const preferences = memoryUserSettings({ values: { 'appearance.mode': 'light' } });
		const update = vi.spyOn(preferences, 'edit');
		const controller = new TerminalSettingsController({
			colorThemes: colorThemesWithFixtures,
			themeTarget: target,
			userSettings: preferences,
		});
		controller.open();

		controller.previewColorTheme({ polarity: 'dark', themeId: fixtureDarkTheme.id });
		expect({ ...target.dataset }).toEqual({ colorTheme: fixtureDarkTheme.id, theme: 'dark' });
		controller.previewColorTheme(null);
		expect({ ...target.dataset }).toEqual({ theme: 'light' });

		controller.previewColorTheme({ polarity: 'light', themeId: fixtureLightTheme.id });
		expect({ ...target.dataset }).toEqual({ colorTheme: fixtureLightTheme.id, theme: 'light' });
		controller.close();
		expect({ ...target.dataset }).toEqual({ theme: 'light' });

		expect(update).not.toHaveBeenCalled();
		expect(controller.getSnapshot().preferences).toMatchObject({
			applicationTheme: 'light',
			darkTheme: 'orchbee-dark',
			lightTheme: 'orchbee-light',
		});
		controller.dispose();
	});

	it('commits a pick to its polarity slot, switching only a fixed Appearance of the other polarity', async () => {
		const target = document.createElement('div');
		const preferences = memoryUserSettings({ values: { 'appearance.mode': 'light' } });
		const controller = new TerminalSettingsController({
			colorThemes: colorThemesWithFixtures,
			themeTarget: target,
			userSettings: preferences,
		});

		controller.selectColorTheme('dark', fixtureDarkTheme.id);
		// The pick shows at once and stays shown through the save.
		expect({ ...target.dataset }).toEqual({ colorTheme: fixtureDarkTheme.id, theme: 'dark' });
		await vi.waitFor(() =>
			expect(preferences.getSnapshot().snapshot?.values).toMatchObject({
				'appearance.dark_theme': fixtureDarkTheme.id,
				'appearance.mode': 'dark',
			}),
		);
		expect({ ...target.dataset }).toEqual({ colorTheme: fixtureDarkTheme.id, theme: 'dark' });

		// Under System, a pick for the inactive polarity waits for the system to match.
		controller.update({ applicationTheme: 'system' });
		await vi.waitFor(() => expect(target.dataset['theme']).toBe('light'));
		controller.selectColorTheme('dark', orchbeeTerminalColorTheme('dark').id);
		await vi.waitFor(() =>
			expect(preferences.getSnapshot().snapshot?.values).toMatchObject({
				'appearance.dark_theme': 'orchbee-dark',
				'appearance.mode': 'system',
			}),
		);
		await vi.waitFor(() => expect({ ...target.dataset }).toEqual({ theme: 'light' }));
		controller.dispose();
	});

	it('restores the committed theme when the Session refuses a pick', async () => {
		const target = document.createElement('div');
		const preferences = memoryUserSettings({ values: { 'appearance.mode': 'dark' } });
		vi.spyOn(preferences, 'edit').mockResolvedValue({
			kind: 'refused',
			refusal: { outcome: 'refused', reason: 'unavailable' },
		});
		const controller = new TerminalSettingsController({
			colorThemes: colorThemesWithFixtures,
			themeTarget: target,
			userSettings: preferences,
		});

		controller.selectColorTheme('dark', fixtureDarkTheme.id);
		expect(target.dataset['colorTheme']).toBe(fixtureDarkTheme.id);
		await vi.waitFor(() => expect({ ...target.dataset }).toEqual({ theme: 'dark' }));
		expect(controller.getSnapshot().preferences.darkTheme).toBe('orchbee-dark');
		controller.dispose();
	});

	it('publishes one complete presentation for one terminal-derived change', async () => {
		const target = document.createElement('div');
		const controller = new TerminalSettingsController({
			themeTarget: target,
			userSettings: memoryUserSettings(),
		});
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
				terminalDisplayTheme: 'host',
			}),
		).toEqual({ kind: 'pending' });
		await Promise.resolve();
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
			themeTarget: target,
			userSettings: memoryUserSettings(),
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

		expect(controller.update({ applicationTheme: 'dark' })).toEqual({ kind: 'pending' });
		await Promise.resolve();
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
			themeTarget: target,
			userSettings: memoryUserSettings(),
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

	it('stops the current publication when a listener disposes the controller', async () => {
		const controller = new TerminalSettingsController({
			themeTarget: document.createElement('div'),
			userSettings: memoryUserSettings(),
		});
		const secondListener = vi.fn<(configuration: TerminalPresentationConfiguration) => void>();
		controller.subscribe(() => controller.dispose());
		controller.subscribe(secondListener);

		controller.update({ fontSize: 14 });
		await Promise.resolve();
		expect(secondListener).not.toHaveBeenCalled();
	});

	it('retains custom colors outside Custom mode without a presentation publication', async () => {
		const controller = new TerminalSettingsController({
			themeTarget: document.createElement('div'),
			userSettings: memoryUserSettings(),
		});
		const presentationListener =
			vi.fn<(configuration: TerminalPresentationConfiguration) => void>();
		controller.subscribe(presentationListener);

		expect(controller.update({ customForeground: '#abcdef' })).toEqual({ kind: 'pending' });
		await Promise.resolve();
		expect(controller.getSnapshot().preferences.customForeground).toBe('#abcdef');
		expect(presentationListener).not.toHaveBeenCalled();
		expect(controller.current().defaultColorOverride).toBeNull();
	});

	it('rejects an invalid candidate without changing any external state', () => {
		const settings = memoryUserSettings();
		const edit = vi.spyOn(settings, 'edit');
		const target = document.createElement('div');
		const controller = new TerminalSettingsController({
			themeTarget: target,
			userSettings: settings,
		});
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
		expect(edit).not.toHaveBeenCalled();
		expect(presentationListener).not.toHaveBeenCalled();
		expect(snapshotListener).not.toHaveBeenCalled();
	});

	it('resets the terminal leaves in one edit', async () => {
		const settings = memoryUserSettings({ values: { 'terminal.font_size': 18 } });
		const restore = vi.spyOn(settings, 'restore');
		const changed = new TerminalSettingsController({
			themeTarget: document.createElement('div'),
			userSettings: settings,
		});
		const changedPresentation = vi.fn<(configuration: TerminalPresentationConfiguration) => void>();
		changed.subscribe(changedPresentation);
		changed.resetTerminalAppearance();
		await Promise.resolve();
		expect(changed.getSnapshot().preferences).toEqual(DEFAULT_TERMINAL_APPEARANCE_PREFERENCES);
		expect(restore).toHaveBeenCalledOnce();
		expect(changedPresentation).toHaveBeenCalledOnce();
	});

	it('changes only the font size by the product step and clamps at both bounds', async () => {
		const controller = new TerminalSettingsController({
			themeTarget: document.createElement('div'),
			userSettings: memoryUserSettings({ values: { 'terminal.font_size': 47 } }),
		});

		controller.changeFontSize('increase');
		await Promise.resolve();
		expect(controller.getSnapshot().preferences.fontSize).toBe(48);
		expect(controller.changeFontSize('increase')).toEqual({ kind: 'unchanged' });
		controller.changeFontSize('decrease');
		await Promise.resolve();
		expect(controller.getSnapshot().preferences.fontSize).toBe(46);
		controller.changeFontSize('reset');
		await Promise.resolve();
		expect(controller.getSnapshot().preferences).toEqual(DEFAULT_TERMINAL_APPEARANCE_PREFERENCES);
	});

	it('opens and closes idempotently without publishing a terminal configuration', () => {
		const controller = new TerminalSettingsController({
			themeTarget: document.createElement('div'),
			userSettings: memoryUserSettings(),
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
		const settings = memoryUserSettings();
		const edit = vi.spyOn(settings, 'edit');
		const target = document.createElement('div');
		const controller = new TerminalSettingsController({
			themeTarget: target,
			userSettings: settings,
		});
		const snapshotListener = vi.fn<() => void>();
		const presentationListener =
			vi.fn<(configuration: TerminalPresentationConfiguration) => void>();
		controller.subscribeSnapshot(snapshotListener);
		controller.subscribe(presentationListener);
		controller.dispose();

		expect(controller.update({ fontSize: 14 })).toEqual({ kind: 'disposed' });
		expect(controller.changeFontSize('increase')).toEqual({ kind: 'disposed' });
		expect(controller.open()).toEqual({ kind: 'disposed' });
		expect(controller.close()).toEqual({ kind: 'disposed' });
		expect(edit).not.toHaveBeenCalled();
		expect(target.dataset['theme']).toBe('light');
		expect(snapshotListener).not.toHaveBeenCalled();
		expect(presentationListener).not.toHaveBeenCalled();
	});
});

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
