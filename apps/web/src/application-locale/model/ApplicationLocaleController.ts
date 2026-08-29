import {
	getOrchbeeTextDirection,
	ORCHBEE_LOCALES,
	resolveOrchbeeLocale,
	type OrchbeeLocale,
} from '@orchbee/i18n';

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
}

export type ApplicationLocaleSelectionResult =
	| { readonly kind: 'applied-and-saved' }
	| { readonly kind: 'applied-session-only' }
	| { readonly kind: 'disposed' }
	| { readonly kind: 'invalid' }
	| { readonly kind: 'unchanged' };

export class ApplicationLocaleController implements ApplicationLocaleSource {
	readonly #documentElement: ApplicationLocaleDocumentElement;
	readonly #listeners = new Set<() => void>();
	readonly #storage: ApplicationLocaleStorage;
	#disposed = false;
	#emitting = false;
	#pendingEmission = false;
	#revision = 0;
	#snapshot: ApplicationLocaleSnapshot;

	constructor({ documentElement, preferredLocales, storage }: ApplicationLocaleControllerOptions) {
		this.#documentElement = documentElement;
		this.#storage = storage;
		const restored = readApplicationLocalePreference(storage);
		const effectiveLocale = restored.preference ?? resolveOrchbeeLocale(preferredLocales);
		this.#snapshot = createSnapshot(effectiveLocale, restored.issue, restored.preference);
		this.#applyDocumentMetadata(effectiveLocale);
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
		if (!isOrchbeeLocale(locale)) return { kind: 'invalid' };
		if (this.#snapshot.preference === locale && this.#snapshot.persistenceIssue === null) {
			return { kind: 'unchanged' };
		}

		const revision = ++this.#revision;
		this.#snapshot = createSnapshot(locale, this.#snapshot.persistenceIssue, locale);
		this.#applyDocumentMetadata(locale);
		this.#emit();
		if (this.#disposed) return { kind: 'disposed' };
		if (this.#revision !== revision) return this.#currentResult();

		const saved = writeApplicationLocalePreference(this.#storage, locale);
		if (this.#disposed) return { kind: 'disposed' };
		if (this.#revision !== revision) return this.#currentResult();
		this.#snapshot = createSnapshot(locale, saved ? null : 'write-failed', locale);
		this.#emit();
		return this.#currentResult();
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#revision += 1;
		this.#pendingEmission = false;
		this.#listeners.clear();
	}

	#applyDocumentMetadata(locale: OrchbeeLocale): void {
		this.#documentElement.lang = locale;
		this.#documentElement.dir = getOrchbeeTextDirection(locale);
	}

	#currentResult(): ApplicationLocaleSelectionResult {
		return this.#snapshot.persistenceIssue === null
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
