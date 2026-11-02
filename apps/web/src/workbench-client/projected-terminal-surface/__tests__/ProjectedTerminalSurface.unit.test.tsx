import { ActionRuntime } from '@orchbee/actions';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import type { WorkbenchTerminalPresentation } from '@orchbee/workbench';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

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
			closeTerminal: () => undefined,
			controlPresentation: 'none',
			createTab: () => undefined,
			failureMessage: 'The terminal connection failed.',
			handleFileDragOver: () => undefined,
			handleFileDrop: () => undefined,
			imagePreview: null,
			launchFailure: null,
			launchFailureRoot: { current: null },
			liveCropped: false,
			pasteReview: null,
			mobileInputState: {
				alt: false,
				control: false,
				focused: false,
				readiness: { kind: 'unavailable' },
			},
			projectionState: 'failed',
			recoveryAvailable: false,
			retryProjection: () => undefined,
			root: { current: null },
			sendMobileKey: () => false,
			stopped: false,
			startNewSession: () => undefined,
			takeControl: () => undefined,
			toggleMobileKeyboard: () => false,
			toggleMobileModifier: () => undefined,
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
					bindBroadcastInputSink: () => ({ dispose: () => undefined }),
					closeTerminal: () => undefined,
					createTab: () => undefined,
					focusRequest: 3,
					onPresentationReady: () => undefined,
					presentation,
					presentationMode: 'active',
				}),
			),
		);

		expect(markup).toContain('data-projected-terminal-status="true"');
		expect(markup).toContain('data-projected-terminal-close-activity="unknown"');
		expect(markup).toContain('role="alert"');
		expect(markup).toContain('The terminal connection failed.');
		expect(markup).toContain('data-slot="projected-terminal-mobile-input-bar"');
		expect(useProjectedTerminalSurface).toHaveBeenCalledWith(
			presentation,
			expect.objectContaining({ autoFocus: true, focusRequest: 3 }),
		);
	});

	it('keeps the archived terminal surface visible behind a stopped-session action bar', () => {
		vi.mocked(useProjectedTerminalSurface).mockReturnValue({
			approvePaste: () => undefined,
			cancelPaste: () => undefined,
			closeTerminal: () => undefined,
			controlPresentation: 'none',
			createTab: () => undefined,
			failureMessage: null,
			handleFileDragOver: () => undefined,
			handleFileDrop: () => undefined,
			imagePreview: null,
			launchFailure: null,
			launchFailureRoot: { current: null },
			liveCropped: false,
			pasteReview: null,
			mobileInputState: {
				alt: false,
				control: false,
				focused: false,
				readiness: { kind: 'unavailable' },
			},
			projectionState: 'requested',
			recoveryAvailable: false,
			retryProjection: () => undefined,
			root: { current: null },
			sendMobileKey: () => false,
			stopped: true,
			startNewSession: () => undefined,
			takeControl: () => undefined,
			toggleMobileKeyboard: () => false,
			toggleMobileModifier: () => undefined,
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
					bindBroadcastInputSink: () => ({ dispose: () => undefined }),
					closeTerminal: () => undefined,
					createTab: () => undefined,
					focusRequest: 0,
					onPresentationReady: () => undefined,
					presentation: stoppedPresentation,
					presentationMode: 'active',
				}),
			),
		);

		expect(markup).toContain('data-slot="projected-terminal-stopped"');
		expect(markup).toContain('Terminal session stopped');
		expect(markup).toContain('Start New Session');
		expect(markup).not.toContain('data-projected-terminal-status="true"');
		expect(markup).not.toContain('data-slot="projected-terminal-mobile-input-bar"');
	});
});
