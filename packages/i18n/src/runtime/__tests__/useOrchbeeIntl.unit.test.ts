import { createElement } from 'react';
import { FormattedMessage } from 'react-intl';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { workbenchMessages } from '../../descriptors';
import { OrchbeeI18nProvider } from '../../orchbee-i18n-provider';
import { useOrchbeeIntl } from '../useOrchbeeIntl';

function IntlHookHarness() {
	const intl = useOrchbeeIntl();
	return createElement(FormattedMessage, {
		...workbenchMessages.sessionCount,
		values: { sessionCount: intl.locale === 'zh-CN' ? 2 : 1 },
	});
}

describe('useOrchbeeIntl', () => {
	it('returns the selected Orchbee locale context', () => {
		const markup = renderToStaticMarkup(
			createElement(OrchbeeI18nProvider, { locale: 'zh-CN' }, createElement(IntlHookHarness)),
		);

		expect(markup).toContain('2 个会话');
	});
});
