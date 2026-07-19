import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { useWebApplicationTabs } from '../useWebApplicationTabs';

function WebApplicationTabsHarness() {
	const workspace = useWebApplicationTabs('zh-CN', [
		{ id: 'first', label: 'First' },
		{ id: 'second', label: 'Second' },
	]);
	return createElement(
		'div',
		{ 'data-active-tab-id': workspace.activeTabId },
		workspace.tabs.map((tab) => tab.label).join(','),
	);
}

describe('useWebApplicationTabs', () => {
	it('prepares the first supplied tab as the initial active tab', () => {
		const markup = renderToStaticMarkup(createElement(WebApplicationTabsHarness));

		expect(markup).toContain('data-active-tab-id="first"');
		expect(markup).toContain('First,Second');
	});
});
