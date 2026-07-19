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
				tabs: [
					{ id: 'orchbee', label: '~/github/orchbee' },
					{ id: 'tabs', label: 'workbench/tabs' },
				],
			}),
		);

		expect(markup).toContain('data-slot="workbench-left-panel"');
		expect(markup).toContain('data-slot="workbench-right-panel"');
		expect(markup).toContain('flex-basis:320px');
		expect(markup).toContain('role="tablist"');
		expect(markup).toContain('~/github/orchbee');
		expect(markup).toContain('aria-label="Close ~/github/orchbee"');
		expect(markup).toContain('aria-label="New Tab"');
		expect(markup.match(/draggable="true"/gu)).toHaveLength(2);
		expect(markup).not.toContain('Local shell');
		expect(markup).not.toContain('本地 Shell');
	});
});
