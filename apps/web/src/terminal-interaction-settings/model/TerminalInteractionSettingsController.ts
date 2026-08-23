import type {
	TerminalInteractionConfiguration,
	TerminalInteractionConfigurationSource,
} from '@orchbee/terminal';

import {
	DEFAULT_TERMINAL_INTERACTION_PREFERENCES,
	terminalInteractionConfigurationFromPreferences,
	type TerminalInteractionPreferences,
} from './terminal-interaction-preferences';
import {
	normalizeTerminalInteractionPreferences,
	readTerminalInteractionPreferences,
	writeTerminalInteractionPreferences,
	type TerminalInteractionPersistenceIssue,
	type TerminalInteractionStorage,
} from './terminal-interaction-preferences-storage';

export interface TerminalInteractionSettingsSnapshot {
	readonly persistenceIssue: TerminalInteractionPersistenceIssue | null;
	readonly preferences: TerminalInteractionPreferences;
}

export type TerminalInteractionSettingsUpdateResult =
	| { readonly kind: 'applied-and-saved' }
	| { readonly kind: 'applied-session-only' }
	| { readonly kind: 'disposed' }
	| { readonly kind: 'invalid' }
	| { readonly kind: 'unchanged' };

type ConfigurationListener = (configuration: TerminalInteractionConfiguration) => void;
type SnapshotListener = () => void;

interface PendingPersistence {
	readonly preferences: TerminalInteractionPreferences;
	readonly revision: number;
}

export class TerminalInteractionSettingsController implements TerminalInteractionConfigurationSource {
	readonly #configurationListeners = new Set<ConfigurationListener>();
	readonly #snapshotListeners = new Set<SnapshotListener>();
	readonly #storage: TerminalInteractionStorage;
	#configuration: TerminalInteractionConfiguration;
	#disposed = false;
	#pendingConfigurations: TerminalInteractionConfiguration[] = [];
	#pendingPersistence: PendingPersistence | null = null;
	#persisting = false;
	#publishingConfiguration = false;
	#revision = 0;
	#snapshot: TerminalInteractionSettingsSnapshot;

	constructor(storage: TerminalInteractionStorage) {
		this.#storage = storage;
		const restored = readTerminalInteractionPreferences(storage);
		this.#configuration = terminalInteractionConfigurationFromPreferences(restored.preferences);
		this.#snapshot = snapshot(restored.issue, restored.preferences);
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
		const candidate = normalizeTerminalInteractionPreferences({
			...this.#snapshot.preferences,
			...patch,
		});
		if (candidate === null) return { kind: 'invalid' };
		const configurationChanged = candidate.copyOnSelect !== this.#snapshot.preferences.copyOnSelect;
		if (!configurationChanged && this.#snapshot.persistenceIssue === null) {
			return { kind: 'unchanged' };
		}

		const revision = ++this.#revision;
		this.#snapshot = snapshot(this.#snapshot.persistenceIssue, candidate);
		if (configurationChanged) {
			this.#configuration = terminalInteractionConfigurationFromPreferences(candidate);
			this.#emitConfiguration(this.#configuration);
		}
		if (this.#disposed) return { kind: 'disposed' };
		if (this.#revision === revision) this.#persist(candidate, revision);
		if (this.#disposed) return { kind: 'disposed' };
		return this.#snapshot.persistenceIssue === null
			? { kind: 'applied-and-saved' }
			: { kind: 'applied-session-only' };
	}

	reset(): TerminalInteractionSettingsUpdateResult {
		return this.update(DEFAULT_TERMINAL_INTERACTION_PREFERENCES);
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#revision += 1;
		this.#pendingConfigurations = [];
		this.#pendingPersistence = null;
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

	#persist(preferences: TerminalInteractionPreferences, revision: number): void {
		this.#pendingPersistence = { preferences, revision };
		if (this.#persisting) return;
		this.#persisting = true;
		try {
			while (this.#pendingPersistence !== null && !this.#disposed) {
				const pending = this.#pendingPersistence;
				this.#pendingPersistence = null;
				const saved = writeTerminalInteractionPreferences(this.#storage, pending.preferences);
				if (this.#disposed || this.#revision !== pending.revision) continue;
				this.#snapshot = snapshot(saved ? null : 'write-failed', pending.preferences);
				this.#emitSnapshot();
			}
		} finally {
			this.#persisting = false;
		}
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
	persistenceIssue: TerminalInteractionPersistenceIssue | null,
	preferences: TerminalInteractionPreferences,
): TerminalInteractionSettingsSnapshot {
	return Object.freeze({ persistenceIssue, preferences });
}
