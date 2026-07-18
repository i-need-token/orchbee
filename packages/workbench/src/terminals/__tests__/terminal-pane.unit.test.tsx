import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import type { WorkbenchSession } from '../../sessions';
import { TerminalPane } from '../terminal-pane';

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
			createElement(TerminalPane, { session, active: true, onFocus: () => undefined }),
		);

		expect(markup).toContain('Local shell');
		expect(markup).toContain('running');
		expect(markup).toContain('data-status="running"');
		expect(markup).toContain('Starting terminal');
	});
});
