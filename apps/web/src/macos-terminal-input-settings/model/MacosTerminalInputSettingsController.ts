import { isTerminalOptionAsAltMode, type TerminalInputConfiguration } from '@orchbee/terminal';

import type { TerminalInputConfigurationSource } from '../../terminal-input-configuration';
import { readSetting, settingChange, type UserSettingsControl } from '../../user-settings';

import {
	DEFAULT_MACOS_TERMINAL_INPUT_PREFERENCES,
	terminalInputConfigurationFromMacosPreferences,
	type MacosTerminalInputPreferences,
} from './macos-terminal-input-preferences';

const OPTION_AS_ALT_KEY = 'terminal.macos_option_as_alt';

export interface MacosTerminalInputSettingsSnapshot {
	readonly preferences: MacosTerminalInputPreferences;
}

/** An accepted change is `pending` until the settings file answers with it. */
export type MacosTerminalInputSettingsUpdateResult =
	| { readonly kind: 'disposed' }
	| { readonly kind: 'invalid' }
	| { readonly kind: 'pending' }
	| { readonly kind: 'unchanged' };

type ConfigurationListener = (configuration: TerminalInputConfiguration) => void;
type SnapshotListener = () => void;

/** Option as Alt is `terminal.macos_option_as_alt` in the settings file of the Server's machine. */
export class MacosTerminalInputSettingsController implements TerminalInputConfigurationSource {
	readonly #configurationListeners = new Set<ConfigurationListener>();
	readonly #settings: UserSettingsControl;
	readonly #snapshotListeners = new Set<SnapshotListener>();
	readonly #stopObserving: () => void;
	#configuration: TerminalInputConfiguration;
	#disposed = false;
	#snapshot: MacosTerminalInputSettingsSnapshot;

	constructor(settings: UserSettingsControl) {
		this.#settings = settings;
		const preferences = this.#read();
		this.#configuration = terminalInputConfigurationFromMacosPreferences(preferences);
		this.#snapshot = Object.freeze({ preferences });
		this.#stopObserving = settings.subscribe(() => this.#sync());
	}

	current = (): TerminalInputConfiguration => this.#configuration;

	getSnapshot = (): MacosTerminalInputSettingsSnapshot => this.#snapshot;

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

	update(patch: Partial<MacosTerminalInputPreferences>): MacosTerminalInputSettingsUpdateResult {
		if (this.#disposed) return { kind: 'disposed' };
		const optionAsAlt = patch.optionAsAlt ?? this.#snapshot.preferences.optionAsAlt;
		if (!isTerminalOptionAsAltMode(optionAsAlt)) return { kind: 'invalid' };
		if (optionAsAlt === this.#snapshot.preferences.optionAsAlt) return { kind: 'unchanged' };
		void this.#settings.edit([settingChange(OPTION_AS_ALT_KEY, optionAsAlt)]);
		return { kind: 'pending' };
	}

	reset(): MacosTerminalInputSettingsUpdateResult {
		if (this.#disposed) return { kind: 'disposed' };
		void this.#settings.restore([OPTION_AS_ALT_KEY]);
		return { kind: 'pending' };
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#stopObserving();
		this.#configurationListeners.clear();
		this.#snapshotListeners.clear();
	}

	#read(): MacosTerminalInputPreferences {
		const optionAsAlt = readSetting(this.#settings.getSnapshot().snapshot, OPTION_AS_ALT_KEY);
		return isTerminalOptionAsAltMode(optionAsAlt)
			? Object.freeze({ optionAsAlt })
			: DEFAULT_MACOS_TERMINAL_INPUT_PREFERENCES;
	}

	#sync(): void {
		if (this.#disposed) return;
		const preferences = this.#read();
		if (preferences.optionAsAlt === this.#snapshot.preferences.optionAsAlt) return;
		this.#snapshot = Object.freeze({ preferences });
		this.#configuration = terminalInputConfigurationFromMacosPreferences(preferences);
		for (const listener of new Set(this.#configurationListeners)) {
			if (this.#disposed) break;
			try {
				listener(this.#configuration);
			} catch {
				// One projected terminal cannot prevent the remaining stable consumers.
			}
		}
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
