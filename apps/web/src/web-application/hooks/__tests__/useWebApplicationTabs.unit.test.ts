import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { useWebApplicationTabs } from '../useWebApplicationTabs';

function TabsProbe({ locale }: { readonly locale: 'en-US' | 'zh-CN' }) {
	const { tabModel } = useWebApplicationTabs(locale);
	return tabModel
		.getSnapshot()
		.tabs.map((tab) => tab.title)
		.join('|');
}

describe('useWebApplicationTabs', () => {
	it('creates the initial English terminal model', () => {
		const markup = renderToStaticMarkup(createElement(TabsProbe, { locale: 'en-US' }));

		expect(markup).toBe('Terminal 1|Terminal 2|Terminal 3|Terminal 4');
	});

	it('creates the initial Simplified Chinese terminal model', () => {
		const markup = renderToStaticMarkup(createElement(TabsProbe, { locale: 'zh-CN' }));

		expect(markup).toBe('终端 1|终端 2|终端 3|终端 4');
	});
});
