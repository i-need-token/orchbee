import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { useWebApplicationProjectWorkspace } from '../useWebApplicationProjectWorkspace';

function TabsProbe({ locale }: { readonly locale: 'en-US' | 'zh-CN' }) {
	const runtime = useWebApplicationProjectWorkspace(locale);
	return runtime
		.getActiveTerminalContext()
		.tabModel.getSnapshot()
		.tabs.map((tab) => tab.title)
		.join('|');
}

function ProjectProbe({ locale }: { readonly locale: 'en-US' | 'zh-CN' }) {
	const runtime = useWebApplicationProjectWorkspace(locale);
	const snapshot = runtime.getSnapshot();
	return `${snapshot.activeLocation.kind}|${snapshot.projects.length}`;
}

describe('useWebApplicationProjectWorkspace', () => {
	it('creates the initial English terminal model', () => {
		const markup = renderToStaticMarkup(createElement(TabsProbe, { locale: 'en-US' }));

		expect(markup).toBe('Terminal 1');
	});

	it('creates the initial Simplified Chinese terminal model', () => {
		const markup = renderToStaticMarkup(createElement(TabsProbe, { locale: 'zh-CN' }));

		expect(markup).toBe('终端 1');
	});

	it('starts both locales without a synthetic Project', () => {
		expect(renderToStaticMarkup(createElement(ProjectProbe, { locale: 'en-US' }))).toBe(
			'standalone|0',
		);
		expect(renderToStaticMarkup(createElement(ProjectProbe, { locale: 'zh-CN' }))).toBe(
			'standalone|0',
		);
	});
});
