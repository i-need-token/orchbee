import type {
	TerminalInteractionConfiguration,
	TerminalInteractionConfigurationSource,
} from '@orchbee/terminal';
import { DEFAULT_USER_PREFERENCES } from '@orchbee/workbench-client-protocol';

import type { UserPreferencesControl } from '../../user-preferences';

import { PreferenceWriteQueue } from '../../PreferenceWriteQueue';

import {
	DEFAULT_TERMINAL_INTERACTION_PREFERENCES,
	terminalInteractionConfigurationFromPreferences,
	type TerminalInteractionPreferences,
	type TerminalTrailingBlankRowsPreferenceSource,
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
	| { readonly kind: 'pending' }
	| { readonly kind: 'unchanged' };

type ConfigurationListener = (configuration: TerminalInteractionConfiguration) => void;
type SnapshotListener = () => void;

export class TerminalInteractionSettingsController implements TerminalInteractionConfigurationSource {
	readonly #configurationListeners = new Set<ConfigurationListener>();
	readonly #persistence = new PreferenceWriteQueue();
	readonly #snapshotListeners = new Set<SnapshotListener>();
	readonly #storage: TerminalInteractionStorage;
	readonly #userPreferences: UserPreferencesControl | undefined;
	readonly #removeUserPreferences: (() => void) | undefined;
	#configuration: TerminalInteractionConfiguration;
	#disposed = false;
	#pendingConfigurations: TerminalInteractionConfiguration[] = [];
	#publishingConfiguration = false;
	#revision = 0;
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

	constructor(storage: TerminalInteractionStorage, userPreferences?: UserPreferencesControl) {
		this.#storage = storage;
		this.#userPreferences = userPreferences;
		const restored =
			userPreferences === undefined
				? readTerminalInteractionPreferences(storage)
				: { issue: null, preferences: interactionPreferences(userPreferences) };
		this.#configuration = terminalInteractionConfigurationFromPreferences(restored.preferences);
		this.#snapshot = snapshot(restored.issue, restored.preferences);
		this.#removeUserPreferences = userPreferences?.subscribe(() => this.#syncUserPreferences());
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
		if (this.#userPreferences !== undefined) {
			void this.#userPreferences.update(patch);
			return { kind: 'pending' };
		}
		const preferencesChanged =
			candidate.copyOnSelect !== this.#snapshot.preferences.copyOnSelect ||
			candidate.trimTrailingBlankRowsForAgents !==
				this.#snapshot.preferences.trimTrailingBlankRowsForAgents;
		const configurationChanged = candidate.copyOnSelect !== this.#snapshot.preferences.copyOnSelect;
		if (!preferencesChanged && this.#snapshot.persistenceIssue === null) {
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
		return this.#snapshot.persistenceIssue === null && !this.#persistence.pending
			? { kind: 'applied-and-saved' }
			: { kind: 'applied-session-only' };
	}

	reset(): TerminalInteractionSettingsUpdateResult {
		if (this.#userPreferences !== undefined) {
			void this.#userPreferences.reset(['copyOnSelect', 'trimTrailingBlankRowsForAgents']);
			return { kind: 'pending' };
		}
		return this.update(DEFAULT_TERMINAL_INTERACTION_PREFERENCES);
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#removeUserPreferences?.();
		this.#revision += 1;
		this.#pendingConfigurations = [];
		this.#persistence.dispose();
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

	#syncUserPreferences(): void {
		if (this.#userPreferences === undefined || this.#disposed) return;
		const preferences = interactionPreferences(this.#userPreferences);
		const before = this.#snapshot.preferences;
		if (
			before.copyOnSelect === preferences.copyOnSelect &&
			before.trimTrailingBlankRowsForAgents === preferences.trimTrailingBlankRowsForAgents
		)
			return;
		this.#snapshot = snapshot(null, preferences);
		if (before.copyOnSelect !== preferences.copyOnSelect) {
			this.#configuration = terminalInteractionConfigurationFromPreferences(preferences);
			this.#emitConfiguration(this.#configuration);
		}
		this.#emitSnapshot();
	}

	#persist(preferences: TerminalInteractionPreferences, revision: number): void {
		this.#persistence.write(() => {
			const saved = writeTerminalInteractionPreferences(this.#storage, preferences);
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
	persistenceIssue: TerminalInteractionPersistenceIssue | null,
	preferences: TerminalInteractionPreferences,
): TerminalInteractionSettingsSnapshot {
	return Object.freeze({ persistenceIssue, preferences });
}

function interactionPreferences(control: UserPreferencesControl): TerminalInteractionPreferences {
	const values = control.getSnapshot().snapshot?.values ?? DEFAULT_USER_PREFERENCES;
	return {
		copyOnSelect: values.copyOnSelect,
		trimTrailingBlankRowsForAgents: values.trimTrailingBlankRowsForAgents,
	};
}
