import { describe, expect, it } from 'vitest';

import {
	DEFAULT_ORCHBEE_LOCALE,
	getOrchbeeTextDirection,
	ORCHBEE_LOCALES,
	resolveOrchbeeLocale,
} from '../locale';

describe('resolveOrchbeeLocale', () => {
	it('resolves exact locales and supported language variants', () => {
		expect(resolveOrchbeeLocale(['en-US'])).toBe('en-US');
		expect(resolveOrchbeeLocale(['en-GB'])).toBe('en-US');
		expect(resolveOrchbeeLocale(['zh'])).toBe('zh-CN');
		expect(resolveOrchbeeLocale(['zh-Hans-SG'])).toBe('zh-CN');
	});

	it('continues through ordered preferences until it finds a supported locale', () => {
		expect(resolveOrchbeeLocale(['fr-FR', 'zh-CN', 'en-US'])).toBe('zh-CN');
	});

	it('does not map Traditional Chinese to Simplified Chinese', () => {
		expect(resolveOrchbeeLocale(['zh-Hant'])).toBe(DEFAULT_ORCHBEE_LOCALE);
		expect(resolveOrchbeeLocale(['zh-TW'])).toBe(DEFAULT_ORCHBEE_LOCALE);
		expect(resolveOrchbeeLocale(['zh-HK'])).toBe(DEFAULT_ORCHBEE_LOCALE);
	});

	it('falls back for invalid, unsupported, and missing preferences', () => {
		expect(resolveOrchbeeLocale(['not_a_locale'])).toBe(DEFAULT_ORCHBEE_LOCALE);
		expect(resolveOrchbeeLocale(['fr-FR'])).toBe(DEFAULT_ORCHBEE_LOCALE);
		expect(resolveOrchbeeLocale([])).toBe(DEFAULT_ORCHBEE_LOCALE);
	});
});

describe('getOrchbeeTextDirection', () => {
	it('defines a direction for every shipped locale', () => {
		for (const locale of ORCHBEE_LOCALES) {
			expect(getOrchbeeTextDirection(locale)).toBe('ltr');
		}
	});
});
