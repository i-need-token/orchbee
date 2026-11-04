import {
	getOrchbeeTextDirection,
	ORCHBEE_LOCALES,
	resolveOrchbeeLocale,
	type OrchbeeLocale,
} from '@orchbee/i18n';
import { DEFAULT_USER_PREFERENCES } from '@orchbee/workbench-client-protocol';

import type { UserPreferencesControl } from '../../user-preferences';

export interface ApplicationLocaleDocumentElement {
	dir: string;
	lang: string;
}

export interface ApplicationLocaleSnapshot {
	readonly effectiveLocale: OrchbeeLocale;
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
	readonly userPreferences: UserPreferencesControl;
}

export type ApplicationLocaleSelectionResult =
	| { readonly kind: 'disposed' }
	| { readonly kind: 'invalid' }
	| { readonly kind: 'pending' };

/** The Session's shared preferences own the language; this applies their confirmed value. */
export class ApplicationLocaleController implements ApplicationLocaleSource {
	readonly #documentElement: ApplicationLocaleDocumentElement;
	readonly #listeners = new Set<() => void>();
	readonly #userPreferences: UserPreferencesControl;
	readonly #preferredLocales: readonly string[];
	readonly #removeUserPreferences: () => void;
	#disposed = false;
	#emitting = false;
	#pendingEmission = false;
	#snapshot: ApplicationLocaleSnapshot;

	constructor({
		documentElement,
		preferredLocales,
		userPreferences,
	}: ApplicationLocaleControllerOptions) {
		this.#documentElement = documentElement;
		this.#userPreferences = userPreferences;
		this.#preferredLocales = preferredLocales;
		const effectiveLocale = resolveOrchbeeLocale(preferredLocales);
		this.#snapshot = createSnapshot(effectiveLocale, null);
		this.#applyDocumentMetadata(effectiveLocale);
		this.#syncUserPreferences();
		this.#removeUserPreferences = userPreferences.subscribe(() => this.#syncUserPreferences());
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
		void this.#userPreferences.update({ locale });
		return { kind: 'pending' };
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#removeUserPreferences();
		this.#pendingEmission = false;
		this.#listeners.clear();
	}

	#applyDocumentMetadata(locale: OrchbeeLocale): void {
		this.#documentElement.lang = locale;
		this.#documentElement.dir = getOrchbeeTextDirection(locale);
	}

	#syncUserPreferences(): void {
		const locale =
			this.#userPreferences.getSnapshot().snapshot?.values.locale ??
			DEFAULT_USER_PREFERENCES.locale;
		const preference = locale === 'system' ? null : locale;
		const effectiveLocale = preference ?? resolveOrchbeeLocale(this.#preferredLocales);
		if (
			this.#snapshot.preference === preference &&
			this.#snapshot.effectiveLocale === effectiveLocale
		)
			return;
		this.#snapshot = createSnapshot(effectiveLocale, preference);
		this.#applyDocumentMetadata(effectiveLocale);
		this.#emit();
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
	preference: OrchbeeLocale | null,
): ApplicationLocaleSnapshot {
	return Object.freeze({ effectiveLocale, preference });
}

function isOrchbeeLocale(locale: unknown): locale is OrchbeeLocale {
	return typeof locale === 'string' && ORCHBEE_LOCALES.some((candidate) => candidate === locale);
}
