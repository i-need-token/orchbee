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
			approvePaste: () => undefined,
			cancelPaste: () => undefined,
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
					autoFocus: true,
					bindActionContextProvider: () => ({ dispose: () => undefined }),
					presentation,
				}),
			),
		);

		expect(markup).toContain('data-projected-terminal-status="true"');
		expect(markup).toContain('data-projected-terminal-close-activity="unknown"');
		expect(markup).toContain('role="alert"');
		expect(markup).toContain('The terminal connection failed.');
		expect(useProjectedTerminalSurface).toHaveBeenCalledWith(
			presentation,
			expect.objectContaining({ autoFocus: true }),
		);
	});

	it('keeps the archived terminal surface visible behind a stopped-session action bar', () => {
		vi.mocked(useProjectedTerminalSurface).mockReturnValue({
			approvePaste: () => undefined,
			cancelPaste: () => undefined,
			failureMessage: null,
			handleFileDragOver: () => undefined,
			handleFileDrop: () => undefined,
			pasteReview: null,
			projectionState: 'requested',
			recoveryAvailable: false,
			retryProjection: () => undefined,
			root: { current: null },
			stopped: true,
			startNewSession: () => undefined,
		});
		const actionRuntime = new ActionRuntime();
		const stoppedPresentation: WorkbenchTerminalPresentation = {
			...presentation,
			lifecycle: { kind: 'stopped' },
		};

		const markup = renderToStaticMarkup(
			createElement(
				OrchbeeI18nProvider,
				{ locale: 'en-US' },
				createElement(ProjectedTerminalSurface, {
					actionRuntime,
					actionScope: actionRuntime.createScope('test.stopped-terminal'),
					admitBellEffect: () => true,
					autoFocus: false,
					bindActionContextProvider: () => ({ dispose: () => undefined }),
					presentation: stoppedPresentation,
				}),
			),
		);

		expect(markup).toContain('data-slot="projected-terminal-stopped"');
		expect(markup).toContain('Terminal session stopped');
		expect(markup).toContain('Start New Session');
		expect(markup).not.toContain('data-projected-terminal-status="true"');
	});
});
