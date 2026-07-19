import { FormattedMessage } from 'react-intl';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { workbenchMessages } from '../../descriptors';
import { OrchbeeI18nProvider } from '../OrchbeeI18nProvider';

describe('OrchbeeI18nProvider', () => {
	it('renders the selected product catalog', () => {
		const markup = renderToStaticMarkup(
			createElement(
				OrchbeeI18nProvider,
				{ locale: 'zh-CN' },
				createElement(FormattedMessage, workbenchMessages.newTerminal),
			),
		);

		expect(markup).toContain('新建终端');
	});
});
