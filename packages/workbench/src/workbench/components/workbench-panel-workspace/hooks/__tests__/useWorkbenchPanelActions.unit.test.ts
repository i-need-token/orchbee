// @vitest-environment jsdom

import { ActionRuntime, type ActionScope } from '@orchbee/actions';
import { cleanup, render } from '@testing-library/react';
import { createElement, type ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { PANEL_ACTION_IDS } from '../../actions/panel-action-contracts';
import type {
	PanelWorkspaceTransition,
	TabPanelWorkspace,
} from '../../model/panel-workspace-state';
import { useWorkbenchPanelActions } from '../useWorkbenchPanelActions';

const activeTab: TabPanelWorkspace = {
	focusedPanelId: 'panel-1',
	maximizedPanelId: null,
	root: {
		activeChildTerminalId: null,
		childTerminals: [],
		id: 'panel-1',
		kind: 'panel',
		primaryTerminal: {
			cwd: null,
			id: 'terminal-1',
			phase: 'running',
			title: 'Terminal 1',
		},
	},
	tabId: 'tab-1',
};

const appliedTransition: PanelWorkspaceTransition = {
	outcome: 'applied',
	rejection: null,
	state: {
		nextPanelSequence: 2,
		nextPanelSlotSequence: 2,
		nextSplitSequence: 1,
		nextTerminalSequence: 2,
		tabs: [activeTab],
	},
};

interface HarnessProps {
	readonly captureScope: (scope: ActionScope) => void;
	readonly createChildTerminal: (panelId: string) => PanelWorkspaceTransition | null;
	readonly parentScope: ActionScope;
	readonly runtime: ActionRuntime;
}

function Harness({
	captureScope,
	createChildTerminal,
	parentScope,
	runtime,
}: HarnessProps): ReactNode {
	const actions = useWorkbenchPanelActions({
		activeTab,
		operations: {
			activateChildTerminal: () => appliedTransition,
			closeDisplayedTerminal: () => appliedTransition,
			createChildTerminal,
			focusPanel: () => appliedTransition,
			movePanel: () => appliedTransition,
			showPrimaryTerminal: () => appliedTransition,
			splitPanel: () => ({ panelId: 'panel-2', transition: appliedTransition }),
			togglePanelMaximization: () => appliedTransition,
		},
		panelCount: 1,
		parentScope,
		runtime,
	});
	captureScope(actions.scope);
	return null;
}

afterEach(() => cleanup());

describe('useWorkbenchPanelActions', () => {
	it('registers panel semantics, calls the domain writer, and disposes with its owner', async () => {
		const runtime = new ActionRuntime();
		const parentScope = runtime.createScope('workbench');
		const createChildTerminal = vi.fn<(panelId: string) => PanelWorkspaceTransition>(
			() => appliedTransition,
		);
		let scope: ActionScope | undefined;
		const rendered = render(
			createElement(Harness, {
				captureScope: (candidate) => {
					scope = candidate;
				},
				createChildTerminal,
				parentScope,
				runtime,
			}),
		);
		expect(scope).toBeDefined();
		if (scope === undefined) {
			throw new Error('Expected the panel action scope to be captured');
		}

		expect(
			new Set(
				runtime
					.list(
						{ scope, source: 'command-surface' },
						{
							[PANEL_ACTION_IDS.activateChildTerminal]: {
								childTerminalId: 'terminal-2',
								panelId: 'panel-1',
							},
							[PANEL_ACTION_IDS.close]: {
								childTerminalId: null,
								panelId: 'panel-1',
								tabId: 'tab-1',
							},
							[PANEL_ACTION_IDS.createChildTerminal]: { panelId: 'panel-1' },
							[PANEL_ACTION_IDS.focus]: { panelId: 'panel-1' },
							[PANEL_ACTION_IDS.move]: {
								direction: 'right',
								panelId: 'panel-1',
								targetPanelId: 'panel-1',
							},
							[PANEL_ACTION_IDS.showPrimaryTerminal]: { panelId: 'panel-1' },
							[PANEL_ACTION_IDS.split]: {
								direction: 'right',
								panelId: 'panel-1',
							},
							[PANEL_ACTION_IDS.toggleMaximization]: { panelId: 'panel-1' },
						},
					)
					.map((action) => action.descriptor.id),
			),
		).toEqual(new Set(Object.values(PANEL_ACTION_IDS)));

		const execution = runtime.execute(
			PANEL_ACTION_IDS.createChildTerminal,
			{ panelId: 'panel-1' },
			{ scope, source: 'context-menu' },
		);

		await expect(execution.completion).resolves.toEqual({ kind: 'completed' });
		expect(createChildTerminal).toHaveBeenCalledWith('panel-1');
		rendered.unmount();
		expect(
			runtime
				.execute(
					PANEL_ACTION_IDS.createChildTerminal,
					{ panelId: 'panel-1' },
					{ scope, source: 'context-menu' },
				)
				.getSnapshot(),
		).toMatchObject({ state: 'unsupported' });
	});
});
