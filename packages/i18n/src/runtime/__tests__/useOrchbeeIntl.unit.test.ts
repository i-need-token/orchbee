import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { OrchbeeI18nProvider } from '../../orchbee-i18n-provider';
import { useOrchbeeIntl } from '../useOrchbeeIntl';

function IntlHookHarness() {
	return useOrchbeeIntl().locale;
}

describe('useOrchbeeIntl', () => {
	it('returns the selected Orchbee locale context', () => {
		const markup = renderToStaticMarkup(
			createElement(OrchbeeI18nProvider, { locale: 'zh-CN' }, createElement(IntlHookHarness)),
		);

		expect(markup).toContain('zh-CN');
	});
});
