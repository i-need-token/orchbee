import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { RouteUnavailable } from '../RouteUnavailable';

function render(locale: OrchbeeLocale): string {
	return renderToStaticMarkup(
		createElement(OrchbeeI18nProvider, { locale }, createElement(RouteUnavailable)),
	);
}

describe('RouteUnavailable', () => {
	it('ships localized route failure copy in both product locales', () => {
		expect(render('en-US')).toContain('This page isn’t available');
		expect(render('zh-CN')).toContain('此页面不可用');
	});
});
