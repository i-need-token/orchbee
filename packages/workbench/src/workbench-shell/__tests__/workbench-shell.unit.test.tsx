import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { WorkbenchShell } from '../workbench-shell';

describe('WorkbenchShell', () => {
	it('renders the shared product regions', () => {
		const markup = renderToStaticMarkup(
			createElement(
				OrchbeeI18nProvider,
				{ locale: 'en-US' },
				createElement(WorkbenchShell, {
					sidebar: 'Session navigation',
					toolbar: 'Workbench actions',
					tabs: 'Session tabs',
					content: 'Terminal panes',
					status: 'Connected',
				}),
			),
		);

		expect(markup).toContain('Orchbee');
		expect(markup).toContain('Workbench navigation');
		expect(markup).toContain('Session navigation');
		expect(markup).toContain('Terminal panes');
		expect(markup).toContain('Connected');
	});
});
