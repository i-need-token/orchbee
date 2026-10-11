import { z } from 'zod';

const AppearanceSchema = z.enum(['system', 'light', 'dark']);
const PreferenceSchema = z.strictObject({ appearance: AppearanceSchema });
const APPEARANCE_KEY = 'orchbee:website:appearance:v1';

export type Appearance = z.infer<typeof AppearanceSchema>;
export interface PreferenceStorage {
	getItem(key: string): string | null;
	setItem(key: string, value: string): void;
}

export function parseAppearance(value: unknown): Appearance {
	const parsed = AppearanceSchema.safeParse(value);
	return parsed.success ? parsed.data : 'system';
}

export function readAppearance(storage: PreferenceStorage): Appearance {
	try {
		const raw = storage.getItem(APPEARANCE_KEY);
		if (raw === null) return 'system';
		const result = PreferenceSchema.safeParse(JSON.parse(raw));
		return result.success ? result.data.appearance : 'system';
	} catch {
		return 'system';
	}
}

export function writeAppearance(storage: PreferenceStorage, appearance: Appearance): void {
	try {
		storage.setItem(APPEARANCE_KEY, JSON.stringify({ appearance }));
	} catch {
		// A blocked local preference must not prevent this tab from changing appearance.
	}
}
