import { ActionRuntime } from '@orchbee/actions';
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
	closeActivity: 'unknown',
	currentCwd: '/workspace',
	launchCwd: '/workspace',
	lifecycle: { kind: 'running' },
	naturalTitle: null,
	presentationId: terminalPresentationId('presentation-failed'),
	title: 'Terminal',
};

describe('ProjectedTerminalSurface', () => {
	it('renders an acquisition failure through the visible terminal status overlay', () => {
		vi.mocked(useProjectedTerminalSurface).mockReturnValue({
			approveClipboardWrite: () => undefined,
			approvePaste: () => undefined,
			cancelClipboardWrite: () => undefined,
			cancelPaste: () => undefined,
			clipboardWriteRequest: null,
			failureMessage: 'The terminal connection failed.',
			handleFileDragOver: () => undefined,
			handleFileDrop: () => undefined,
			pasteReview: null,
			projectionState: 'failed',
			recoveryAvailable: false,
			retryProjection: () => undefined,
			root: { current: null },
			stopped: false,
			startNewSession: () => undefined,
		});
		const actionRuntime = new ActionRuntime();

		const markup = renderToStaticMarkup(
			createElement(
				OrchbeeI18nProvider,
				{ locale: 'en-US' },
				createElement(ProjectedTerminalSurface, {
					actionRuntime,
					actionScope: actionRuntime.createScope('test.terminal'),
					admitBellEffect: () => true,
					bindActionContextProvider: () => ({ dispose: () => undefined }),
					presentation,
				}),
			),
		);

		expect(markup).toContain('data-projected-terminal-status="true"');
		expect(markup).toContain('data-projected-terminal-close-activity="unknown"');
		expect(markup).toContain('role="alert"');
		expect(markup).toContain('The terminal connection failed.');
	});
});
