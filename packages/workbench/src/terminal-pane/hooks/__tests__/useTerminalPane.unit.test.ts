import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import type { WorkbenchSession } from '../../../sessions';
import { useTerminalPane } from '../useTerminalPane';

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

function TerminalPaneHookHarness() {
	const terminalPane = useTerminalPane(session);
	return createElement('output', { 'data-pane-state': terminalPane.paneState });
}

describe('useTerminalPane', () => {
	it('prepares the terminal surface in its starting state', () => {
		const markup = renderToStaticMarkup(createElement(TerminalPaneHookHarness));

		expect(markup).toContain('data-pane-state="starting"');
	});
});
