import { ORCHBEE_LOCALES, type OrchbeeLocale } from '@orchbee/i18n';
import type { UserPreferencesPatch } from '@orchbee/workbench-client-protocol';
import { z } from 'zod';

const APPLICATION_LOCALE_STORAGE_KEY = 'orchbee:web:application-locale:v1';
const CURRENT_APPLICATION_LOCALE_STORAGE_KEY = 'orchbee:web:application-locale:v2';

export type ApplicationLocalePersistenceIssue = 'invalid-record' | 'read-failed' | 'write-failed';

export interface ApplicationLocaleStorage {
	getItem(key: string): string | null;
	setItem(key: string, value: string): void;
}

const applicationLocalePreferenceSchema = z.strictObject({
	locale: z.enum(ORCHBEE_LOCALES).nullable(),
});
const legacyApplicationLocalePreferenceSchema = z.strictObject({ locale: z.enum(ORCHBEE_LOCALES) });

export function readLegacyLocalePreferencePatch(
	storage: ApplicationLocaleStorage,
): Pick<UserPreferencesPatch, 'locale'> | null {
	try {
		if (
			storage.getItem(CURRENT_APPLICATION_LOCALE_STORAGE_KEY) === null &&
			storage.getItem(APPLICATION_LOCALE_STORAGE_KEY) === null
		)
			return {};
		const restored = readApplicationLocalePreference(storage);
		return restored.issue === null ? { locale: restored.preference ?? ('system' as const) } : null;
	} catch {
		return null;
	}
}

export function readApplicationLocalePreference(storage: ApplicationLocaleStorage): {
	readonly issue: Exclude<ApplicationLocalePersistenceIssue, 'write-failed'> | null;
	readonly preference: OrchbeeLocale | null;
} {
	let serialized: string | null;
	let legacy = false;
	try {
		serialized = storage.getItem(CURRENT_APPLICATION_LOCALE_STORAGE_KEY);
		if (serialized === null) {
			serialized = storage.getItem(APPLICATION_LOCALE_STORAGE_KEY);
			legacy = true;
		}
	} catch {
		return { issue: 'read-failed', preference: null };
	}
	if (serialized === null) return { issue: null, preference: null };

	try {
		const storedLocaleSchema = legacy
			? legacyApplicationLocalePreferenceSchema
			: applicationLocalePreferenceSchema;
		const result = storedLocaleSchema.safeParse(JSON.parse(serialized));
		return result.success
			? { issue: null, preference: result.data.locale }
			: { issue: 'invalid-record', preference: null };
	} catch {
		return { issue: 'invalid-record', preference: null };
	}
}

export function writeApplicationLocalePreference(
	storage: ApplicationLocaleStorage,
	locale: OrchbeeLocale | null,
): boolean {
	try {
		storage.setItem(CURRENT_APPLICATION_LOCALE_STORAGE_KEY, JSON.stringify({ locale }));
		return true;
	} catch {
		return false;
	}
}
