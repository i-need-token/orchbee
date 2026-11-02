import type {
	DesktopTerminalDefaultSettingsFailureCode,
	DesktopTerminalDefaultSettingsSnapshot,
} from '@orchbee/desktop-client-protocol';
import type { TerminalConfiguration } from '@orchbee/terminal-protocol';
import {
	DEFAULT_USER_PREFERENCES,
	type UserPreferencesSnapshot,
} from '@orchbee/workbench-client-protocol';

import type {
	DesktopTerminalDefaultSettingsClientOperationResult,
	DesktopTerminalDefaultSettingsClientSnapshot,
	DesktopTerminalDefaultSettingsControl,
} from '../../terminal-default-settings-control';
import type { UserPreferencesControl } from '../../user-preferences';

/**
 * Scrollback is one of the Session's shared preferences. The Worker gives it to each terminal it
 * launches after the save, so nothing here reconfigures a running terminal.
 */
export class ServerTerminalDefaultSettingsClient implements DesktopTerminalDefaultSettingsControl {
	readonly #listeners = new Set<() => void>();
	readonly #preferences: UserPreferencesControl;
	readonly #stopObserving: () => void;
	#disposed = false;
	#failureCode: DesktopTerminalDefaultSettingsFailureCode | null = null;
	#observed: UserPreferencesSnapshot | null;
	#pending = false;
	#snapshot: DesktopTerminalDefaultSettingsClientSnapshot;

	constructor(preferences: UserPreferencesControl) {
		this.#preferences = preferences;
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

	getSnapshot = (): DesktopTerminalDefaultSettingsClientSnapshot => this.#snapshot;

	subscribe = (listener: () => void): (() => void) => {
		if (this.#disposed) return () => undefined;
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};

	async update(
		configuration: TerminalConfiguration,
		preferencesSource?: string,
	): Promise<DesktopTerminalDefaultSettingsClientOperationResult> {
		const observed = this.#observed;
		if (this.#disposed || observed === null) return { kind: 'unavailable' };
		if (this.#pending) {
			return { code: 'busy', kind: 'failed', snapshot: settingsSnapshot(this.#snapshot) };
		}
		this.#pending = true;
		this.#failureCode = null;
		this.#present();
		const response = await this.#preferences.update(
			{ scrollbackBytes: configuration.scrollbackBytes },
			preferencesSource ?? observed.source,
		);
		this.#pending = false;
		if (response.kind === 'snapshot') {
			this.#observed = response.snapshot;
		} else if (response.kind === 'outcome-unknown') {
			this.#failureCode = 'outcome-unknown';
		} else if (response.reason === 'busy' || response.reason === 'source-changed') {
			this.#failureCode = response.reason;
		} else {
			this.#failureCode =
				response.reason === 'unavailable' ? 'control-unavailable' : 'write-failed';
		}
		this.#present();
		const snapshot = settingsSnapshot(this.#snapshot);
		return this.#failureCode === null
			? { kind: 'completed', snapshot }
			: { code: this.#failureCode, kind: 'failed', snapshot };
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#stopObserving();
		this.#listeners.clear();
	}

	#project(): DesktopTerminalDefaultSettingsClientSnapshot {
		const observed = this.#observed;
		return {
			configuration: {
				scrollbackBytes:
					observed?.values.scrollbackBytes ?? DEFAULT_USER_PREFERENCES.scrollbackBytes,
			},
			failureCode: this.#failureCode ?? (observed === null ? 'control-unavailable' : null),
			pending: this.#pending,
			persistenceIssue: null,
			...(observed === null ? {} : { preferencesSource: observed.source }),
			source: 'session',
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
				// A presentation observer cannot strand a save or its answer.
			}
		}
	}
}

function settingsSnapshot(
	snapshot: DesktopTerminalDefaultSettingsClientSnapshot,
): DesktopTerminalDefaultSettingsSnapshot {
	return {
		configuration: snapshot.configuration,
		persistenceIssue: snapshot.persistenceIssue,
		...(snapshot.preferencesSource === undefined
			? {}
			: { preferencesSource: snapshot.preferencesSource }),
		source: snapshot.source,
	};
}
