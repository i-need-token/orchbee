import type {
	TerminalDefaultSettingsClientSnapshot,
	TerminalDefaultSettingsControl,
} from '../../terminal-default-settings-control';
import { readSetting, type UserSettingsControl } from '../../user-settings';

/**
 * Scrollback is `terminal.scrollback_bytes` in the settings file. Settings edits it through the
 * file, and the Worker gives it to each terminal it launches after the edit, so nothing here writes
 * or reconfigures a running terminal.
 */
export class ServerTerminalDefaultSettingsClient implements TerminalDefaultSettingsControl {
	readonly #listeners = new Set<() => void>();
	readonly #settings: UserSettingsControl;
	readonly #stopObserving: () => void;
	#disposed = false;
	#snapshot: TerminalDefaultSettingsClientSnapshot;

	constructor(settings: UserSettingsControl) {
		this.#settings = settings;
		this.#snapshot = this.#project();
		this.#stopObserving = settings.subscribe(() => {
			const next = this.#project();
			if (
				next.failureCode === this.#snapshot.failureCode &&
				next.configuration.scrollbackBytes === this.#snapshot.configuration.scrollbackBytes
			) {
				return;
			}
			this.#snapshot = next;
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
		const { snapshot } = this.#settings.getSnapshot();
		return {
			configuration: { scrollbackBytes: readSetting(snapshot, 'terminal.scrollback_bytes') },
			failureCode: snapshot === null ? 'control-unavailable' : null,
		};
	}

	#present(): void {
		if (this.#disposed) return;
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
