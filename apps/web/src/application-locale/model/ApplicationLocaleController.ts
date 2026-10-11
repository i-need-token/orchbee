import {
	getOrchbeeTextDirection,
	ORCHBEE_LOCALES,
	resolveOrchbeeLocale,
	type OrchbeeLocale,
} from '@orchbee/i18n';
import { DEFAULT_USER_PREFERENCES } from '@orchbee/workbench-client-protocol';

import type { UserPreferencesControl } from '../../user-preferences';

import { PreferenceWriteQueue } from '../../PreferenceWriteQueue';

import {
	readApplicationLocalePreference,
	writeApplicationLocalePreference,
	type ApplicationLocalePersistenceIssue,
	type ApplicationLocaleStorage,
} from './application-locale-preference-storage';

export interface ApplicationLocaleDocumentElement {
	dir: string;
	lang: string;
}

export interface ApplicationLocaleSnapshot {
	readonly effectiveLocale: OrchbeeLocale;
	readonly persistenceIssue: ApplicationLocalePersistenceIssue | null;
	readonly preference: OrchbeeLocale | null;
}

export interface ApplicationLocaleSource {
	current(): OrchbeeLocale;
	getSnapshot(): ApplicationLocaleSnapshot;
	subscribe(listener: () => void): () => void;
}

export interface ApplicationLocaleControllerOptions {
	readonly documentElement: ApplicationLocaleDocumentElement;
	readonly preferredLocales: readonly string[];
	readonly storage: ApplicationLocaleStorage;
	readonly userPreferences?: UserPreferencesControl;
}

export type ApplicationLocaleSelectionResult =
	| { readonly kind: 'applied-and-saved' }
	| { readonly kind: 'applied-session-only' }
	| { readonly kind: 'disposed' }
	| { readonly kind: 'invalid' }
	| { readonly kind: 'pending' }
	| { readonly kind: 'unchanged' };

export class ApplicationLocaleController implements ApplicationLocaleSource {
	readonly #documentElement: ApplicationLocaleDocumentElement;
	readonly #listeners = new Set<() => void>();
	readonly #persistence = new PreferenceWriteQueue();
	readonly #storage: ApplicationLocaleStorage;
	readonly #userPreferences: UserPreferencesControl | undefined;
	readonly #preferredLocales: readonly string[];
	readonly #removeUserPreferences: (() => void) | undefined;
	#disposed = false;
	#emitting = false;
	#pendingEmission = false;
	#revision = 0;
	#snapshot: ApplicationLocaleSnapshot;

	constructor({
		documentElement,
		preferredLocales,
		storage,
		userPreferences,
	}: ApplicationLocaleControllerOptions) {
		this.#documentElement = documentElement;
		this.#storage = storage;
		this.#userPreferences = userPreferences;
		this.#preferredLocales = preferredLocales;
		const restored =
			userPreferences === undefined
				? readApplicationLocalePreference(storage)
				: { issue: null, preference: null };
		const effectiveLocale = restored.preference ?? resolveOrchbeeLocale(preferredLocales);
		this.#snapshot = createSnapshot(effectiveLocale, restored.issue, restored.preference);
		this.#applyDocumentMetadata(effectiveLocale);
		if (userPreferences !== undefined) {
			this.#syncUserPreferences();
			this.#removeUserPreferences = userPreferences.subscribe(() => this.#syncUserPreferences());
		}
	}

	current = (): OrchbeeLocale => this.#snapshot.effectiveLocale;

	getSnapshot = (): ApplicationLocaleSnapshot => this.#snapshot;

	subscribe = (listener: () => void): (() => void) => {
		if (this.#disposed) return () => undefined;
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};

	select(locale: unknown): ApplicationLocaleSelectionResult {
		if (this.#disposed) return { kind: 'disposed' };
		if (locale !== 'system' && !isOrchbeeLocale(locale)) return { kind: 'invalid' };
		if (this.#userPreferences !== undefined) {
			void this.#userPreferences.update({ locale });
			return { kind: 'pending' };
		}
		const preference = locale === 'system' ? null : locale;
		const effectiveLocale = preference ?? resolveOrchbeeLocale(this.#preferredLocales);
		if (this.#snapshot.preference === preference && this.#snapshot.persistenceIssue === null) {
			return { kind: 'unchanged' };
		}

		const revision = ++this.#revision;
		this.#snapshot = createSnapshot(effectiveLocale, this.#snapshot.persistenceIssue, preference);
		this.#applyDocumentMetadata(effectiveLocale);
		this.#emit();
		if (this.#disposed) return { kind: 'disposed' };
		if (this.#revision !== revision) return this.#currentResult();

		this.#persistence.write(() => {
			const saved = writeApplicationLocalePreference(this.#storage, preference);
			if (this.#disposed || this.#revision !== revision) return;
			this.#snapshot = createSnapshot(effectiveLocale, saved ? null : 'write-failed', preference);
			this.#emit();
		});
		if (this.#disposed) return { kind: 'disposed' };
		return this.#currentResult();
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#removeUserPreferences?.();
		this.#revision += 1;
		this.#persistence.dispose();
		this.#pendingEmission = false;
		this.#listeners.clear();
	}

	#applyDocumentMetadata(locale: OrchbeeLocale): void {
		this.#documentElement.lang = locale;
		this.#documentElement.dir = getOrchbeeTextDirection(locale);
	}

	#syncUserPreferences(): void {
		const locale =
			this.#userPreferences?.getSnapshot().snapshot?.values.locale ??
			DEFAULT_USER_PREFERENCES.locale;
		const preference = locale === 'system' ? null : locale;
		const effectiveLocale = preference ?? resolveOrchbeeLocale(this.#preferredLocales);
		if (
			this.#snapshot.preference === preference &&
			this.#snapshot.effectiveLocale === effectiveLocale
		)
			return;
		this.#snapshot = createSnapshot(effectiveLocale, null, preference);
		this.#applyDocumentMetadata(effectiveLocale);
		this.#emit();
	}

	#currentResult(): ApplicationLocaleSelectionResult {
		return this.#snapshot.persistenceIssue === null && !this.#persistence.pending
			? { kind: 'applied-and-saved' }
			: { kind: 'applied-session-only' };
	}

	#emit(): void {
		if (this.#emitting) {
			this.#pendingEmission = true;
			return;
		}
		this.#emitting = true;
		try {
			do {
				this.#pendingEmission = false;
				for (const listener of new Set(this.#listeners)) {
					if (this.#disposed) break;
					try {
						listener();
					} catch {
						// External-store listeners are isolated from each other.
					}
				}
			} while (this.#pendingEmission && !this.#disposed);
		} finally {
			this.#emitting = false;
		}
	}
}

function createSnapshot(
	effectiveLocale: OrchbeeLocale,
	persistenceIssue: ApplicationLocalePersistenceIssue | null,
	preference: OrchbeeLocale | null,
): ApplicationLocaleSnapshot {
	return Object.freeze({ effectiveLocale, persistenceIssue, preference });
}

function isOrchbeeLocale(locale: unknown): locale is OrchbeeLocale {
	return typeof locale === 'string' && ORCHBEE_LOCALES.some((candidate) => candidate === locale);
}
