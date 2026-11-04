// @vitest-environment jsdom

import { ActionRuntime, defineAction, noActionInput } from '@orchbee/actions';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import * as React from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import { TERMINAL_ACTION_IDS } from '../../../../../../../../../actions/terminal-action-contracts';
import type { WorkbenchTerminalSurfaceProps } from '../../../../../../../../../types/workbench-terminal-surface';
import type { TerminalPresentation } from '../../../../../../../model/panel-workspace-state';
import type { WorkbenchPanelWorkspaceView } from '../../../../../../../types/panel-workspace-view';
import type { TerminalTabPresentationLayer } from '../../../../../../../types/terminal-tab-presentation-layer';
import { WorkbenchTerminalViewport } from '../WorkbenchTerminalViewport';

const clearTerminal = vi.fn<() => void>();
const terminal: TerminalPresentation = {
	closeActivity: 'unknown',
	currentCwd: null,
	launchCwd: null,
	lifecycle: { kind: 'preview' },
	naturalTitle: null,
	presentationId: terminalPresentationId('terminal-1'),
	title: 'Terminal 1',
};
const layer: TerminalTabPresentationLayer = {
	epoch: 0,
	mode: 'active',
	reportPresentationReady: () => undefined,
};

function TestTerminalSurface({
	actionRuntime,
	actionScope,
	bindActionContextProvider,
}: WorkbenchTerminalSurfaceProps): React.ReactNode {
	React.useEffect(() => {
		const actionRegistration = actionRuntime.register(
			actionScope,
			defineAction({
				category: {
					id: 'terminal',
					title: { defaultMessage: 'Terminal', id: 'test.terminal.category' },
				},
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'test-terminal-clear',
					mode: 'foreground',
				},
				getAvailability: () => ({ kind: 'enabled' }),
				handler: {
					execute: (context) => {
						clearTerminal();
						context.markCommitted();
						return { kind: 'completed' };
					},
				},
				id: TERMINAL_ACTION_IDS.clear,
				input: noActionInput,
				title: { defaultMessage: 'Clear terminal', id: 'test.terminal.clear' },
			}),
		);
		const contextRegistration = bindActionContextProvider({
			capture: () => ({
				focus: { terminal: true },
				kind: 'product',
				target: { id: 'attachment-1', type: 'terminal-attachment' },
			}),
			executePrepared: (invocation) => actionRuntime.executePrepared(invocation),
		});
		return () => {
			contextRegistration.dispose();
			actionRegistration.dispose();
		};
	}, [actionRuntime, actionScope, bindActionContextProvider]);

	return <textarea aria-label='Terminal input' />;
}

function createView(actionRuntime: ActionRuntime): WorkbenchPanelWorkspaceView {
	return {
		actionRuntime,
		actionScope: actionRuntime.createScope('test.workbench'),
		admitTerminalBellEffect: () => true,
		activeTab: { focusedPanelId: 'panel-1' },
		bindBroadcastInputSink: () => ({ dispose: () => undefined }),
		broadcastInput: { kind: 'idle' },
		canStartBroadcastInput: () => false,
		closeActiveTerminal: () => undefined,
		createChildTerminal: () => undefined,
		createTerminalTab: () => undefined,
		maximizedPanelId: null,
		messages: {
			broadcastStart: 'Broadcast Input',
			close: 'Close',
			maximize: 'Maximize',
			newChildTerminal: 'New child terminal',
			restore: 'Restore',
			splitBelow: 'Split below',
			splitRight: 'Split right',
		},
		panelCount: 1,
		shortcutLabelForAction: (actionId: string) =>
			actionId === TERMINAL_ACTION_IDS.clear ? '⌘K' : undefined,
		splitPanel: () => null,
		startBroadcastInput: () => undefined,
		togglePanelMaximization: () => undefined,
	} as unknown as WorkbenchPanelWorkspaceView;
}

afterEach(() => {
	cleanup();
	clearTerminal.mockReset();
});

describe('WorkbenchTerminalViewport', () => {
	it('does not mount an incoming terminal surface before its ancestor layout settles', () => {
		const actionRuntime = new ActionRuntime();
		const view = createView(actionRuntime);
		const incomingLayer: TerminalTabPresentationLayer = {
			...layer,
			mode: 'incoming',
		};
		const renderViewport = (layoutSettled: boolean) => (
			<OrchbeeI18nProvider locale='en-US'>
				<WorkbenchTerminalViewport
					layer={incomingLayer}
					layoutSettled={layoutSettled}
					panelId='panel-1'
					terminal={terminal}
					terminalAutoFocusAllowed
					terminalFocusRequest={null}
					terminalRole='primary'
					terminalSurface={TestTerminalSurface}
					view={view}
				/>
			</OrchbeeI18nProvider>
		);
		const rendered = render(renderViewport(false));

		expect(screen.queryByRole('textbox', { name: 'Terminal input' })).toBeNull();
		rendered.rerender(renderViewport(true));
		expect(screen.getByRole('textbox', { name: 'Terminal input' })).toBeDefined();

		rendered.unmount();
		actionRuntime.dispose();
	});

	it('restores the same terminal input focus after Clear closes the product context menu', async () => {
		const actionRuntime = new ActionRuntime();
		const rendered = render(
			<OrchbeeI18nProvider locale='en-US'>
				<WorkbenchTerminalViewport
					layer={layer}
					layoutSettled
					panelId='panel-1'
					terminal={terminal}
					terminalAutoFocusAllowed
					terminalFocusRequest={null}
					terminalRole='primary'
					terminalSurface={TestTerminalSurface}
					view={createView(actionRuntime)}
				/>
			</OrchbeeI18nProvider>,
		);
		const input = screen.getByRole('textbox', { name: 'Terminal input' });
		input.focus();
		expect(input.ownerDocument.activeElement).toBe(input);

		fireEvent.contextMenu(input, { clientX: 20, clientY: 20 });
		const clearItem = await screen.findByRole('menuitem', { name: /Clear terminal/u });
		expect(clearItem.textContent).toContain('⌘K');
		clearItem.focus();
		expect(input.ownerDocument.activeElement).not.toBe(input);

		fireEvent.click(clearItem);

		await waitFor(() => expect(input.ownerDocument.activeElement).toBe(input));
		expect(clearTerminal).toHaveBeenCalledOnce();
		rendered.unmount();
		actionRuntime.dispose();
	});
});
