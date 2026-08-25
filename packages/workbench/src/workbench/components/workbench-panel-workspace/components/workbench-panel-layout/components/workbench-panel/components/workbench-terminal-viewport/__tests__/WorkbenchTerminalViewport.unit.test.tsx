// @vitest-environment jsdom

import { ActionRuntime, defineAction, noActionInput } from '@orchbee/actions';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import * as React from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { terminalPresentationId } from '../../../../../../../../../../terminal-presentation';
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
		closeActiveTerminal: () => undefined,
		createChildTerminal: () => undefined,
		createTerminalTab: () => undefined,
		maximizedPanelId: null,
		messages: {
			close: 'Close',
			maximize: 'Maximize Panel',
			minimize: 'Minimize Panel',
			newChildTerminal: 'New child terminal',
			newTab: 'New tab',
			split: 'Split panel',
			splitAbove: 'Split above',
			splitBelow: 'Split below',
			splitLeft: 'Split left',
			splitRight: 'Split right',
		},
		panelCount: 1,
		shortcutLabelForAction: (actionId: string) =>
			actionId === TERMINAL_ACTION_IDS.clear ? '⌘K' : undefined,
		splitPanel: () => null,
		togglePanelMaximization: () => undefined,
	} as unknown as WorkbenchPanelWorkspaceView;
}

afterEach(() => {
	cleanup();
	clearTerminal.mockReset();
});

describe('WorkbenchTerminalViewport', () => {
	it('restores the same terminal input focus after Clear closes the product context menu', async () => {
		const actionRuntime = new ActionRuntime();
		const rendered = render(
			<OrchbeeI18nProvider locale='en-US'>
				<WorkbenchTerminalViewport
					layer={layer}
					layoutSettled
					panelId='panel-1'
					terminal={terminal}
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
