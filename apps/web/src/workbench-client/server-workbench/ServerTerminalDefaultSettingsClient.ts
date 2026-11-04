import {
	DEFAULT_USER_PREFERENCES,
	type UserPreferencesSnapshot,
} from '@orchbee/workbench-client-protocol';

import type {
	TerminalDefaultSettingsClientSnapshot,
	TerminalDefaultSettingsControl,
} from '../../terminal-default-settings-control';
import type { UserPreferencesControl } from '../../user-preferences';

/**
 * Scrollback is one of the Session's shared preferences. Settings saves it through those preferences,
 * and the Worker gives it to each terminal it launches after the save, so nothing here writes or
 * reconfigures a running terminal.
 */
export class ServerTerminalDefaultSettingsClient implements TerminalDefaultSettingsControl {
	readonly #listeners = new Set<() => void>();
	readonly #stopObserving: () => void;
	#disposed = false;
	#observed: UserPreferencesSnapshot | null;
	#snapshot: TerminalDefaultSettingsClientSnapshot;

	constructor(preferences: UserPreferencesControl) {
		this.#observed = preferences.getSnapshot().snapshot;
		this.#snapshot = this.#project();
		this.#stopObserving = preferences.subscribe(() => {
			const observed = preferences.getSnapshot().snapshot;
			// A source names its record's content, so an unchanged one changes nothing shown here.
			if (observed?.source === this.#observed?.source) return;
			this.#observed = observed;
			this.#present();
		});
	}

	getSnapshot = (): TerminalDefaultSettingsClientSnapshot => this.#snapshot;

	subscribe = (listener: () => void): (() => void) => {
		if (this.#disposed) return () => undefined;
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#stopObserving();
		this.#listeners.clear();
	}

	#project(): TerminalDefaultSettingsClientSnapshot {
		const observed = this.#observed;
		return {
			configuration: {
				scrollbackBytes:
					observed?.values.scrollbackBytes ?? DEFAULT_USER_PREFERENCES.scrollbackBytes,
			},
			failureCode: observed === null ? 'control-unavailable' : null,
		};
	}

	#present(): void {
		if (this.#disposed) return;
		this.#snapshot = this.#project();
		for (const listener of new Set(this.#listeners)) {
			if (!this.#listeners.has(listener)) continue;
			try {
				listener();
			} catch {
				// A presentation observer cannot strand another observer.
			}
		}
	}
}
