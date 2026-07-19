import { useIntl } from 'react-intl';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { OrchbeeI18nProvider } from '../OrchbeeI18nProvider';

function LocaleProbe() {
	return useIntl().locale;
}

describe('OrchbeeI18nProvider', () => {
	it('provides the selected product locale', () => {
		const markup = renderToStaticMarkup(
			createElement(OrchbeeI18nProvider, { locale: 'zh-CN' }, createElement(LocaleProbe)),
		);

		expect(markup).toContain('zh-CN');
	});
});
