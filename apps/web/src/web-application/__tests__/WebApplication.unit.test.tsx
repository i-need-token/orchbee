import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { WebApplication } from '../WebApplication';

describe('WebApplication', () => {
	it('renders the Workbench with the initial terminal tabs', () => {
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
		expect(markup).toContain('role="tablist"');
		expect(markup).toContain('Terminal 1');
		expect(markup).not.toContain('Terminal 4');
		expect(markup).toContain('aria-label="Create Project"');
		expect(markup).not.toContain('data-slot="workbench-sidebar-project"');
		expect(markup).not.toContain('data-slot="workbench-sidebar-workspace"');
		expect(markup).not.toContain('Local shell');
		expect(markup).not.toContain('本地 Shell');
	});
});
