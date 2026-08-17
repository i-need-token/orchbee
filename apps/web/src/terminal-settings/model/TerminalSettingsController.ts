import type { TerminalPresentationConfiguration } from '@orchbee/terminal';

import {
	DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
	terminalAppearancePreferencesEqual,
	terminalDerivedPreferencesEqual,
	terminalPresentationConfigurationFromPreferences,
	type TerminalAppearancePreferences,
	type TerminalAppearancePresentationSource,
} from './terminal-appearance-preferences';
import {
	normalizeTerminalAppearancePreferences,
	readTerminalAppearancePreferences,
	writeTerminalAppearancePreferences,
	type TerminalAppearancePersistenceIssue,
	type TerminalAppearanceStorage,
} from './terminal-appearance-preferences-storage';

export interface TerminalSettingsSnapshot {
	readonly open: boolean;
	readonly persistenceIssue: TerminalAppearancePersistenceIssue | null;
	readonly preferences: TerminalAppearancePreferences;
}

export type TerminalSettingsUpdateResult =
	| { readonly kind: 'applied-and-saved' }
	| { readonly kind: 'applied-session-only' }
	| { readonly kind: 'disposed' }
	| { readonly kind: 'invalid' }
	| { readonly kind: 'unchanged' };

export type TerminalSettingsVisibilityResult =
	| { readonly kind: 'changed' }
	| { readonly kind: 'disposed' }
	| { readonly kind: 'unchanged' };

export interface TerminalSettingsControllerOptions {
	readonly storage: TerminalAppearanceStorage;
	readonly themeTarget: Pick<HTMLElement, 'dataset'>;
}

type SnapshotListener = () => void;
type PresentationListener = (configuration: TerminalPresentationConfiguration) => void;

interface PendingPersistence {
	readonly preferences: TerminalAppearancePreferences;
	readonly revision: number;
}

export class TerminalSettingsController implements TerminalAppearancePresentationSource {
	readonly #presentationListeners = new Set<PresentationListener>();
	readonly #snapshotListeners = new Set<SnapshotListener>();
	readonly #storage: TerminalAppearanceStorage;
	readonly #themeTarget: Pick<HTMLElement, 'dataset'>;
	#disposed = false;
	#pendingPersistence: PendingPersistence | null = null;
	#pendingPresentations: TerminalPresentationConfiguration[] = [];
	#presentationConfiguration: TerminalPresentationConfiguration;
	#publishingPresentation = false;
	#persisting = false;
	#revision = 0;
	#snapshot: TerminalSettingsSnapshot;

	constructor({ storage, themeTarget }: TerminalSettingsControllerOptions) {
		this.#storage = storage;
		this.#themeTarget = themeTarget;
		const restored = readTerminalAppearancePreferences(storage);
		this.#presentationConfiguration = terminalPresentationConfigurationFromPreferences(
			restored.preferences,
		);
		this.#snapshot = snapshot(false, restored.issue, restored.preferences);
		this.#applyApplicationTheme(restored.preferences.applicationTheme);
	}

	current = (): TerminalPresentationConfiguration => this.#presentationConfiguration;

	getSnapshot = (): TerminalSettingsSnapshot => this.#snapshot;

	subscribe = (listener: PresentationListener): (() => void) => {
		if (this.#disposed) return () => undefined;
		this.#presentationListeners.add(listener);
		return () => this.#presentationListeners.delete(listener);
	};

	subscribeSnapshot = (listener: SnapshotListener): (() => void) => {
		if (this.#disposed) return () => undefined;
		this.#snapshotListeners.add(listener);
		return () => this.#snapshotListeners.delete(listener);
	};

	open(): TerminalSettingsVisibilityResult {
		if (this.#disposed) return { kind: 'disposed' };
		if (this.#snapshot.open) return { kind: 'unchanged' };
		this.#snapshot = snapshot(true, this.#snapshot.persistenceIssue, this.#snapshot.preferences);
		this.#emitSnapshot();
		return { kind: 'changed' };
	}

	close(): TerminalSettingsVisibilityResult {
		if (this.#disposed) return { kind: 'disposed' };
		if (!this.#snapshot.open) return { kind: 'unchanged' };
		this.#snapshot = snapshot(false, this.#snapshot.persistenceIssue, this.#snapshot.preferences);
		this.#emitSnapshot();
		return { kind: 'changed' };
	}

	update(patch: Partial<TerminalAppearancePreferences>): TerminalSettingsUpdateResult {
		if (this.#disposed) return { kind: 'disposed' };
		const currentPreferences = this.#snapshot.preferences;
		const candidate = normalizeTerminalAppearancePreferences({ ...currentPreferences, ...patch });
		if (candidate === null) return { kind: 'invalid' };

		const preferencesChanged = !terminalAppearancePreferencesEqual(currentPreferences, candidate);
		if (!preferencesChanged && this.#snapshot.persistenceIssue === null) {
			return { kind: 'unchanged' };
		}

		const terminalChanged = !terminalDerivedPreferencesEqual(currentPreferences, candidate);
		const revision = this.#commitCandidate(candidate, terminalChanged);
		if (currentPreferences.applicationTheme !== candidate.applicationTheme) {
			this.#applyApplicationTheme(candidate.applicationTheme);
		}
		if (terminalChanged) {
			this.#emitPresentation(this.#presentationConfiguration);
		}
		if (this.#disposed) return { kind: 'disposed' };
		if (this.#revision === revision) this.#persist(candidate, revision);
		if (this.#disposed) return { kind: 'disposed' };
		return this.#currentUpdateResult();
	}

	reset(): TerminalSettingsUpdateResult {
		return this.update(DEFAULT_TERMINAL_APPEARANCE_PREFERENCES);
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#revision += 1;
		this.#pendingPersistence = null;
		this.#pendingPresentations = [];
		this.#presentationListeners.clear();
		this.#snapshotListeners.clear();
	}

	#applyApplicationTheme(theme: TerminalAppearancePreferences['applicationTheme']): void {
		this.#themeTarget.dataset['theme'] = theme;
	}

	#commitCandidate(preferences: TerminalAppearancePreferences, terminalChanged: boolean): number {
		this.#revision += 1;
		this.#snapshot = snapshot(this.#snapshot.open, this.#snapshot.persistenceIssue, preferences);
		if (terminalChanged) {
			this.#presentationConfiguration =
				terminalPresentationConfigurationFromPreferences(preferences);
		}
		return this.#revision;
	}

	#currentUpdateResult(): TerminalSettingsUpdateResult {
		return this.#snapshot.persistenceIssue === null
			? { kind: 'applied-and-saved' }
			: { kind: 'applied-session-only' };
	}

	#emitPresentation(configuration: TerminalPresentationConfiguration): void {
		if (this.#publishingPresentation) {
			this.#pendingPresentations.push(configuration);
			return;
		}
		this.#publishingPresentation = true;
		let next: TerminalPresentationConfiguration | undefined = configuration;
		try {
			while (next !== undefined && !this.#disposed) {
				const listeners = new Set(this.#presentationListeners);
				for (const listener of listeners) {
					if (this.#disposed) break;
					try {
						listener(next);
					} catch {
						// One presentation consumer cannot prevent the remaining stable subscribers.
					}
				}
				next = this.#pendingPresentations.shift();
			}
		} finally {
			this.#publishingPresentation = false;
		}
	}

	#persist(preferences: TerminalAppearancePreferences, revision: number): void {
		this.#pendingPersistence = { preferences, revision };
		if (this.#persisting) return;
		this.#persisting = true;
		try {
			while (this.#pendingPersistence !== null && !this.#disposed) {
				const pending = this.#pendingPersistence;
				this.#pendingPersistence = null;
				const saved = writeTerminalAppearancePreferences(this.#storage, pending.preferences);
				if (this.#disposed || this.#revision !== pending.revision) continue;
				this.#snapshot = snapshot(
					this.#snapshot.open,
					saved ? null : 'write-failed',
					pending.preferences,
				);
				this.#emitSnapshot();
			}
		} finally {
			this.#persisting = false;
		}
	}

	#emitSnapshot(): void {
		const listeners = new Set(this.#snapshotListeners);
		for (const listener of listeners) {
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
	open: boolean,
	persistenceIssue: TerminalAppearancePersistenceIssue | null,
	preferences: TerminalAppearancePreferences,
): TerminalSettingsSnapshot {
	return Object.freeze({ open, persistenceIssue, preferences });
}
