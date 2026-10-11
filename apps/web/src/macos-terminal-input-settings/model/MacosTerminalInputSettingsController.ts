import type { TerminalInputConfiguration } from '@orchbee/terminal';

import type { TerminalInputConfigurationSource } from '../../terminal-input-configuration';
import { PreferenceWriteQueue } from '../../PreferenceWriteQueue';

import {
	DEFAULT_MACOS_TERMINAL_INPUT_PREFERENCES,
	terminalInputConfigurationFromMacosPreferences,
	type MacosTerminalInputPreferences,
} from './macos-terminal-input-preferences';
import {
	normalizeMacosTerminalInputPreferences,
	readMacosTerminalInputPreferences,
	writeMacosTerminalInputPreferences,
	type MacosTerminalInputPersistenceIssue,
	type MacosTerminalInputStorage,
} from './macos-terminal-input-preferences-storage';

export interface MacosTerminalInputSettingsSnapshot {
	readonly persistenceIssue: MacosTerminalInputPersistenceIssue | null;
	readonly preferences: MacosTerminalInputPreferences;
}

export type MacosTerminalInputSettingsUpdateResult =
	| { readonly kind: 'applied-and-saved' }
	| { readonly kind: 'applied-session-only' }
	| { readonly kind: 'disposed' }
	| { readonly kind: 'invalid' }
	| { readonly kind: 'unchanged' };

type ConfigurationListener = (configuration: TerminalInputConfiguration) => void;
type SnapshotListener = () => void;

export class MacosTerminalInputSettingsController implements TerminalInputConfigurationSource {
	readonly #configurationListeners = new Set<ConfigurationListener>();
	readonly #persistence = new PreferenceWriteQueue();
	readonly #snapshotListeners = new Set<SnapshotListener>();
	readonly #storage: MacosTerminalInputStorage;
	#configuration: TerminalInputConfiguration;
	#disposed = false;
	#pendingConfigurations: TerminalInputConfiguration[] = [];
	#publishingConfiguration = false;
	#revision = 0;
	#snapshot: MacosTerminalInputSettingsSnapshot;

	constructor(storage: MacosTerminalInputStorage) {
		this.#storage = storage;
		const restored = readMacosTerminalInputPreferences(storage);
		this.#configuration = terminalInputConfigurationFromMacosPreferences(restored.preferences);
		this.#snapshot = snapshot(restored.issue, restored.preferences);
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
		const candidate = normalizeMacosTerminalInputPreferences({
			...this.#snapshot.preferences,
			...patch,
		});
		if (candidate === null) return { kind: 'invalid' };
		const configurationChanged = candidate.optionAsAlt !== this.#snapshot.preferences.optionAsAlt;
		if (!configurationChanged && this.#snapshot.persistenceIssue === null) {
			return { kind: 'unchanged' };
		}

		const revision = ++this.#revision;
		this.#snapshot = snapshot(this.#snapshot.persistenceIssue, candidate);
		if (configurationChanged) {
			this.#configuration = terminalInputConfigurationFromMacosPreferences(candidate);
			this.#emitConfiguration(this.#configuration);
		}
		if (this.#disposed) return { kind: 'disposed' };
		if (this.#revision === revision) this.#persist(candidate, revision);
		if (this.#disposed) return { kind: 'disposed' };
		return this.#snapshot.persistenceIssue === null && !this.#persistence.pending
			? { kind: 'applied-and-saved' }
			: { kind: 'applied-session-only' };
	}

	reset(): MacosTerminalInputSettingsUpdateResult {
		return this.update(DEFAULT_MACOS_TERMINAL_INPUT_PREFERENCES);
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#revision += 1;
		this.#pendingConfigurations = [];
		this.#persistence.dispose();
		this.#configurationListeners.clear();
		this.#snapshotListeners.clear();
	}

	#emitConfiguration(configuration: TerminalInputConfiguration): void {
		if (this.#publishingConfiguration) {
			this.#pendingConfigurations.push(configuration);
			return;
		}
		this.#publishingConfiguration = true;
		let next: TerminalInputConfiguration | undefined = configuration;
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

	#persist(preferences: MacosTerminalInputPreferences, revision: number): void {
		this.#persistence.write(() => {
			const saved = writeMacosTerminalInputPreferences(this.#storage, preferences);
			if (this.#disposed || this.#revision !== revision) return;
			this.#snapshot = snapshot(saved ? null : 'write-failed', preferences);
			this.#emitSnapshot();
		});
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
	persistenceIssue: MacosTerminalInputPersistenceIssue | null,
	preferences: MacosTerminalInputPreferences,
): MacosTerminalInputSettingsSnapshot {
	return Object.freeze({ persistenceIssue, preferences });
}
