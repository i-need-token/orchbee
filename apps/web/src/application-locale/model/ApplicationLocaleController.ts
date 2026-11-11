import {
	getOrchbeeTextDirection,
	ORCHBEE_LOCALES,
	resolveOrchbeeLocale,
	type OrchbeeLocale,
} from '@orchbee/i18n';
import { readSetting, type UserSettingsControl } from '../../user-settings';

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
	readonly userSettings: UserSettingsControl;
}

export type ApplicationLocaleSelectionResult =
	| { readonly kind: 'disposed' }
	| { readonly kind: 'invalid' }
	| { readonly kind: 'pending' };

/** `general.language` in the settings file owns the language; this applies the file's value. */
export class ApplicationLocaleController implements ApplicationLocaleSource {
	readonly #documentElement: ApplicationLocaleDocumentElement;
	readonly #listeners = new Set<() => void>();
	readonly #userSettings: UserSettingsControl;
	readonly #preferredLocales: readonly string[];
	readonly #removeUserSettings: () => void;
	#disposed = false;
	#emitting = false;
	#pendingEmission = false;
	#snapshot: ApplicationLocaleSnapshot;

	constructor({
		documentElement,
		preferredLocales,
		userSettings,
	}: ApplicationLocaleControllerOptions) {
		this.#documentElement = documentElement;
		this.#userSettings = userSettings;
		this.#preferredLocales = preferredLocales;
		const effectiveLocale = resolveOrchbeeLocale(preferredLocales);
		this.#snapshot = createSnapshot(effectiveLocale, null);
		this.#applyDocumentMetadata(effectiveLocale);
		this.#syncUserSettings();
		this.#removeUserSettings = userSettings.subscribe(() => this.#syncUserSettings());
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
		void this.#userSettings.set('general.language', locale);
		return { kind: 'pending' };
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#removeUserSettings();
		this.#pendingEmission = false;
		this.#listeners.clear();
	}

	#applyDocumentMetadata(locale: OrchbeeLocale): void {
		this.#documentElement.lang = locale;
		this.#documentElement.dir = getOrchbeeTextDirection(locale);
	}

	#syncUserSettings(): void {
		const locale = readSetting(this.#userSettings.getSnapshot().snapshot, 'general.language');
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
