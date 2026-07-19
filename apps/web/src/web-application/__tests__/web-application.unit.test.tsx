import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { createInMemorySessionService } from '../../sessions';
import { WebApplication } from '../web-application';

describe('WebApplication', () => {
	it('renders the application-owned session adapter through the Workbench', () => {
		const markup = renderToStaticMarkup(
			createElement(WebApplication, {
				locale: 'en-US',
				sessions: createInMemorySessionService('en-US'),
			}),
		);

		expect(markup).toContain('Local shell');
		expect(markup).toContain('Agent output');
	});

	it('composes the Simplified Chinese catalog across the app and Workbench', () => {
		const markup = renderToStaticMarkup(
			createElement(WebApplication, {
				locale: 'zh-CN',
				sessions: createInMemorySessionService('zh-CN'),
			}),
		);

		expect(markup).toContain('本地 Shell');
		expect(markup).toContain('Agent 输出');
		expect(markup).toContain('本地运行时');
	});
});
