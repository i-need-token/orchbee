import { ORCHBEE_LOCALES, type OrchbeeLocale } from '@orchbee/i18n';
import { z } from 'zod';

const APPLICATION_LOCALE_STORAGE_KEY = 'orchbee:web:application-locale:v1';

export type ApplicationLocalePersistenceIssue = 'invalid-record' | 'read-failed' | 'write-failed';

export interface ApplicationLocaleStorage {
	getItem(key: string): string | null;
	setItem(key: string, value: string): void;
}

const applicationLocalePreferenceSchema = z.strictObject({ locale: z.enum(ORCHBEE_LOCALES) });

export function readApplicationLocalePreference(storage: ApplicationLocaleStorage): {
	readonly issue: Exclude<ApplicationLocalePersistenceIssue, 'write-failed'> | null;
	readonly preference: OrchbeeLocale | null;
} {
	let serialized: string | null;
	try {
		serialized = storage.getItem(APPLICATION_LOCALE_STORAGE_KEY);
	} catch {
		return { issue: 'read-failed', preference: null };
	}
	if (serialized === null) return { issue: null, preference: null };

	try {
		const result = applicationLocalePreferenceSchema.safeParse(JSON.parse(serialized));
		return result.success
			? { issue: null, preference: result.data.locale }
			: { issue: 'invalid-record', preference: null };
	} catch {
		return { issue: 'invalid-record', preference: null };
	}
}

export function writeApplicationLocalePreference(
	storage: ApplicationLocaleStorage,
	locale: OrchbeeLocale,
): boolean {
	try {
		storage.setItem(APPLICATION_LOCALE_STORAGE_KEY, JSON.stringify({ locale }));
		return true;
	} catch {
		return false;
	}
}
