import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { WebApplication } from '../WebApplication';

describe('WebApplication', () => {
	it('renders the empty Workbench panels', () => {
		const markup = renderToStaticMarkup(
			createElement(WebApplication, {
				layoutStorage: {
					getItem: () => '{"width":320}',
					setItem: () => undefined,
				},
				locale: 'en-US',
				shortcutPlatform: 'apple',
			}),
		);

		expect(markup).toContain('data-slot="workbench-left-panel"');
		expect(markup).toContain('data-slot="workbench-right-panel"');
		expect(markup).toContain('flex-basis:320px');
		expect(markup).not.toContain('role="tablist"');
		expect(markup).not.toContain('Local shell');
		expect(markup).not.toContain('本地 Shell');
	});
});
