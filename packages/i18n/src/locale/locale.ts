export const ORCHBEE_LOCALES = ['en-US', 'zh-CN'] as const;
export const DEFAULT_ORCHBEE_LOCALE = 'en-US';

export type OrchbeeLocale = (typeof ORCHBEE_LOCALES)[number];
export type OrchbeeTextDirection = 'ltr' | 'rtl';

const SIMPLIFIED_CHINESE_REGIONS = new Set(['CN', 'SG']);
const TRADITIONAL_CHINESE_REGIONS = new Set(['HK', 'MO', 'TW']);
const ORCHBEE_TEXT_DIRECTIONS = {
	'en-US': 'ltr',
	'zh-CN': 'ltr',
} satisfies Readonly<Record<OrchbeeLocale, OrchbeeTextDirection>>;

function normalizeLocale(locale: string): OrchbeeLocale | undefined {
	let parsedLocale: Intl.Locale;
	try {
		parsedLocale = new Intl.Locale(locale);
	} catch {
		return undefined;
	}

	if (parsedLocale.language === 'en') return 'en-US';
	if (parsedLocale.language !== 'zh') return undefined;
	if (parsedLocale.script === 'Hant') return undefined;
	if (parsedLocale.script === 'Hans') return 'zh-CN';
	if (parsedLocale.region !== undefined && TRADITIONAL_CHINESE_REGIONS.has(parsedLocale.region)) {
		return undefined;
	}
	if (parsedLocale.region === undefined || SIMPLIFIED_CHINESE_REGIONS.has(parsedLocale.region)) {
		return 'zh-CN';
	}
	return undefined;
}

export function resolveOrchbeeLocale(preferredLocales: readonly string[]): OrchbeeLocale {
	for (const preferredLocale of preferredLocales) {
		const locale = normalizeLocale(preferredLocale);
		if (locale !== undefined) return locale;
	}
	return DEFAULT_ORCHBEE_LOCALE;
}

export function getOrchbeeTextDirection(locale: OrchbeeLocale): OrchbeeTextDirection {
	return ORCHBEE_TEXT_DIRECTIONS[locale];
}
