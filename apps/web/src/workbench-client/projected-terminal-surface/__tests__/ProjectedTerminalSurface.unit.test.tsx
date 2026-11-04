import { ActionRuntime } from '@orchbee/actions';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import type { WorkbenchTerminalPresentation } from '../../../workbench';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import { ProjectedTerminalSurface } from '../ProjectedTerminalSurface';
import { useProjectedTerminalSurface } from '../hooks/useProjectedTerminalSurface';
import type { ProjectedTerminalSurfaceView } from '../types/projected-terminal-surface-view';

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

function surfaceView(
	overrides: Partial<ProjectedTerminalSurfaceView>,
): ProjectedTerminalSurfaceView {
	return {
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
		stopped: false,
		startNewSession: () => undefined,
		takeControl: () => undefined,
		terminalOutcome: null,
		toggleMobileKeyboard: () => false,
		toggleMobileModifier: () => undefined,
		...overrides,
	};
}

function renderSurface(
	surfacePresentation: WorkbenchTerminalPresentation,
	options: { readonly autoFocus: boolean; readonly focusRequest: number },
): string {
	const actionRuntime = new ActionRuntime();
	return renderToStaticMarkup(
		createElement(
			OrchbeeI18nProvider,
			{ locale: 'en-US' },
			createElement(ProjectedTerminalSurface, {
				actionRuntime,
				actionScope: actionRuntime.createScope('test.terminal'),
				admitBellEffect: () => true,
				autoFocus: options.autoFocus,
				bindActionContextProvider: () => ({ dispose: () => undefined }),
				bindBroadcastInputSink: () => ({ dispose: () => undefined }),
				closeTerminal: () => undefined,
				createTab: () => undefined,
				focusRequest: options.focusRequest,
				onPresentationReady: () => undefined,
				presentation: surfacePresentation,
				presentationMode: 'active',
			}),
		),
	);
}

describe('ProjectedTerminalSurface', () => {
	it('renders an acquisition failure through the visible terminal status overlay', () => {
		vi.mocked(useProjectedTerminalSurface).mockReturnValue(
			surfaceView({ failureMessage: 'Disconnected', projectionState: 'failed' }),
		);

		const markup = renderSurface(presentation, { autoFocus: true, focusRequest: 3 });

		expect(markup).toContain('data-projected-terminal-status="true"');
		expect(markup).toContain('data-projected-terminal-close-activity="unknown"');
		expect(markup).toContain('role="alert"');
		expect(markup).toContain('Disconnected');
		expect(markup).toContain('data-slot="projected-terminal-mobile-input-bar"');
		expect(useProjectedTerminalSurface).toHaveBeenCalledWith(
			presentation,
			expect.objectContaining({ autoFocus: true, focusRequest: 3 }),
		);
	});

	it('names an exited terminal beside New Tab and drops its input controls', () => {
		vi.mocked(useProjectedTerminalSurface).mockReturnValue(
			surfaceView({ terminalOutcome: 'exited' }),
		);

		const markup = renderSurface(presentation, { autoFocus: false, focusRequest: 0 });

		expect(markup).toContain('data-slot="projected-terminal-ended"');
		expect(markup).toContain('role="status"');
		expect(markup).toContain('Terminal exited');
		expect(markup).toContain('data-slot="projected-terminal-new-tab"');
		expect(markup).not.toContain('data-slot="projected-terminal-mobile-input-bar"');
	});

	it('keeps the archived terminal surface visible behind a stopped-session action bar', () => {
		vi.mocked(useProjectedTerminalSurface).mockReturnValue(surfaceView({ stopped: true }));

		const markup = renderSurface(
			{ ...presentation, lifecycle: { kind: 'stopped' } },
			{ autoFocus: false, focusRequest: 0 },
		);

		expect(markup).toContain('data-slot="projected-terminal-stopped"');
		expect(markup).toContain('Terminal session stopped');
		expect(markup).toContain('Start New Session');
		expect(markup).not.toContain('data-projected-terminal-status="true"');
		expect(markup).not.toContain('data-slot="projected-terminal-mobile-input-bar"');
	});
});
