import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import type { WorkbenchSession } from '../../sessions';
import { TerminalPane } from '../TerminalPane';

const session: WorkbenchSession = {
	id: 'session-1',
	title: 'Local shell',
	status: 'running',
	terminal: {
		subscribeOutput: () => () => undefined,
		write: () => undefined,
		resize: () => undefined,
	},
};

describe('TerminalPane', () => {
	it('renders session identity while the terminal starts', () => {
		const markup = renderToStaticMarkup(
			createElement(
				OrchbeeI18nProvider,
				{ locale: 'en-US' },
				createElement(TerminalPane, {
					session,
					active: true,
					onFocus: () => undefined,
				}),
			),
		);

		expect(markup).toContain('Local shell');
		expect(markup).toContain('Running');
		expect(markup).toContain('data-status="running"');
		expect(markup).toContain('Starting terminal');
	});
});
