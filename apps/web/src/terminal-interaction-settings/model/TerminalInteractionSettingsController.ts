import type {
	TerminalInteractionConfiguration,
	TerminalInteractionConfigurationSource,
} from '@orchbee/terminal';
import type { SettingChange } from '@orchbee/workbench-client-protocol';

import { readSetting, type UserSettingsControl } from '../../user-settings';

import {
	terminalInteractionConfigurationFromPreferences,
	type TerminalInteractionPreferences,
	type TerminalTrailingBlankRowsPreferenceSource,
} from './terminal-interaction-preferences';

export interface TerminalInteractionSettingsSnapshot {
	readonly preferences: TerminalInteractionPreferences;
}

export type TerminalInteractionSettingsUpdateResult =
	| { readonly kind: 'disposed' }
	| { readonly kind: 'pending' };

type ConfigurationListener = (configuration: TerminalInteractionConfiguration) => void;
type SnapshotListener = () => void;

/**
 * `terminal.copy_on_select` and `terminal.trim_trailing_blank_rows_for_agents` in the settings file
 * own both values; this publishes the file's values.
 */
export class TerminalInteractionSettingsController implements TerminalInteractionConfigurationSource {
	readonly #configurationListeners = new Set<ConfigurationListener>();
	readonly #snapshotListeners = new Set<SnapshotListener>();
	readonly #userSettings: UserSettingsControl;
	readonly #removeUserSettings: () => void;
	#configuration: TerminalInteractionConfiguration;
	#disposed = false;
	#pendingConfigurations: TerminalInteractionConfiguration[] = [];
	#publishingConfiguration = false;
	#snapshot: TerminalInteractionSettingsSnapshot;
	readonly trailingBlankRowsPreferenceSource: TerminalTrailingBlankRowsPreferenceSource = {
		current: () => this.#snapshot.preferences.trimTrailingBlankRowsForAgents,
		subscribe: (listener) => {
			let previous = this.#snapshot.preferences.trimTrailingBlankRowsForAgents;
			return this.subscribeSnapshot(() => {
				const next = this.#snapshot.preferences.trimTrailingBlankRowsForAgents;
				if (next === previous) return;
				previous = next;
				listener(next);
			});
		},
	};

	constructor(userSettings: UserSettingsControl) {
		this.#userSettings = userSettings;
		const preferences = interactionPreferences(userSettings);
		this.#configuration = terminalInteractionConfigurationFromPreferences(preferences);
		this.#snapshot = snapshot(preferences);
		this.#removeUserSettings = userSettings.subscribe(() => this.#syncUserSettings());
	}

	current = (): TerminalInteractionConfiguration => this.#configuration;

	getSnapshot = (): TerminalInteractionSettingsSnapshot => this.#snapshot;

	subscribe = (listener: ConfigurationListener): (() => void) => {
		if (this.#disposed) return () => undefined;
		this.#configurationListeners.add(listener);
		return () => this.#configurationListeners.delete(listener);
	};

	subscribeSnapshot = (listener: SnapshotListener): (() => void) => {
		if (this.#disposed) return () => undefined;
		this.#snapshotListeners.add(listener);
		return () => this.#snapshotListeners.delete(listener);
	};

	update(patch: Partial<TerminalInteractionPreferences>): TerminalInteractionSettingsUpdateResult {
		if (this.#disposed) return { kind: 'disposed' };
		const changes: SettingChange[] = [];
		if (patch.copyOnSelect !== undefined) {
			changes.push({ key: 'terminal.copy_on_select', value: patch.copyOnSelect });
		}
		if (patch.trimTrailingBlankRowsForAgents !== undefined) {
			changes.push({
				key: 'terminal.trim_trailing_blank_rows_for_agents',
				value: patch.trimTrailingBlankRowsForAgents,
			});
		}
		void this.#userSettings.edit(changes);
		return { kind: 'pending' };
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#removeUserSettings();
		this.#pendingConfigurations = [];
		this.#configurationListeners.clear();
		this.#snapshotListeners.clear();
	}

	#emitConfiguration(configuration: TerminalInteractionConfiguration): void {
		if (this.#publishingConfiguration) {
			this.#pendingConfigurations.push(configuration);
			return;
		}
		this.#publishingConfiguration = true;
		let next: TerminalInteractionConfiguration | undefined = configuration;
		try {
			while (next !== undefined && !this.#disposed) {
				for (const listener of new Set(this.#configurationListeners)) {
					if (this.#disposed) break;
					try {
						listener(next);
					} catch {
						// One projected terminal cannot prevent the remaining stable consumers.
					}
				}
				next = this.#pendingConfigurations.shift();
			}
		} finally {
			this.#publishingConfiguration = false;
		}
	}

	#syncUserSettings(): void {
		if (this.#disposed) return;
		const preferences = interactionPreferences(this.#userSettings);
		const before = this.#snapshot.preferences;
		if (
			before.copyOnSelect === preferences.copyOnSelect &&
			before.trimTrailingBlankRowsForAgents === preferences.trimTrailingBlankRowsForAgents
		)
			return;
		this.#snapshot = snapshot(preferences);
		if (before.copyOnSelect !== preferences.copyOnSelect) {
			this.#configuration = terminalInteractionConfigurationFromPreferences(preferences);
			this.#emitConfiguration(this.#configuration);
		}
		this.#emitSnapshot();
	}

	#emitSnapshot(): void {
		for (const listener of new Set(this.#snapshotListeners)) {
			if (this.#disposed) break;
			try {
				listener();
			} catch {
				// External-store listeners are isolated from each other.
			}
		}
	}
}

function snapshot(
	preferences: TerminalInteractionPreferences,
): TerminalInteractionSettingsSnapshot {
	return Object.freeze({ preferences });
}

function interactionPreferences(control: UserSettingsControl): TerminalInteractionPreferences {
	const settings = control.getSnapshot().snapshot;
	return {
		copyOnSelect: readSetting(settings, 'terminal.copy_on_select'),
		trimTrailingBlankRowsForAgents: readSetting(
			settings,
			'terminal.trim_trailing_blank_rows_for_agents',
		),
	};
}
