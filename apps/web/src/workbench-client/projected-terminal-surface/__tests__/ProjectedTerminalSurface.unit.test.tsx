import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { terminalPresentationId, type WorkbenchTerminalPresentation } from '@orchbee/workbench';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

import { ProjectedTerminalSurface } from '../ProjectedTerminalSurface';
import { useProjectedTerminalSurface } from '../hooks/useProjectedTerminalSurface';

vi.mock('../hooks/useProjectedTerminalSurface', () => ({
	useProjectedTerminalSurface: vi.fn<typeof useProjectedTerminalSurface>(),
}));

const presentation: WorkbenchTerminalPresentation = {
	currentCwd: '/workspace',
	launchCwd: '/workspace',
	lifecycle: { kind: 'running' },
	presentationId: terminalPresentationId('presentation-failed'),
	title: 'Terminal',
};

describe('ProjectedTerminalSurface', () => {
	it('renders an acquisition failure through the visible terminal status overlay', () => {
		vi.mocked(useProjectedTerminalSurface).mockReturnValue({
			approvePaste: () => undefined,
			cancelPaste: () => undefined,
			failureMessage: 'The terminal connection failed.',
			pasteReview: null,
			projectionState: 'failed',
			root: { current: null },
			stopped: false,
			startNewSession: () => undefined,
		});

		const markup = renderToStaticMarkup(
			createElement(
				OrchbeeI18nProvider,
				{ locale: 'en-US' },
				createElement(ProjectedTerminalSurface, { presentation }),
			),
		);

		expect(markup).toContain('data-projected-terminal-status="true"');
		expect(markup).toContain('role="alert"');
		expect(markup).toContain('The terminal connection failed.');
	});
});
