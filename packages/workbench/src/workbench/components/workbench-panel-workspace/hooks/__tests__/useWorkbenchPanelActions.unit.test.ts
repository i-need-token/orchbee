// @vitest-environment jsdom

import { ActionRuntime, type ActionScope } from '@orchbee/actions';
import { cleanup, render } from '@testing-library/react';
import { createElement, type ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { terminalPresentationId } from '../../../../../terminal-presentation';
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
		activeChildPresentationId: null,
		childPresentations: [],
		id: 'panel-1',
		kind: 'panel',
		primaryPresentation: {
			closeActivity: 'unknown',
			currentCwd: null,
			launchCwd: null,
			lifecycle: { kind: 'running' },
			naturalTitle: null,
			presentationId: terminalPresentationId('terminal-1'),
			title: 'Terminal 1',
		},
	},
	tabId: 'tab-1',
};

const appliedTransition: PanelWorkspaceTransition = {
	commands: [],
	outcome: 'applied',
	rejection: null,
	state: {
		nextPanelSequence: 2,
		nextPanelSlotSequence: 2,
		nextSplitSequence: 1,
		nextTerminalSequence: 2,
		pendingRemovals: [],
		tabs: [activeTab],
	},
};

interface HarnessProps {
	readonly activeTab?: TabPanelWorkspace;
	readonly captureScope: (scope: ActionScope) => void;
	readonly createChildTerminal: (panelId: string) => PanelWorkspaceTransition | null;
	readonly panelCount?: number;
	readonly parentScope: ActionScope;
	readonly runtime: ActionRuntime;
}

function Harness({
	activeTab: activeTabOverride = activeTab,
	captureScope,
	createChildTerminal,
	panelCount = 1,
	parentScope,
	runtime,
}: HarnessProps): ReactNode {
	const actions = useWorkbenchPanelActions({
		activeTab: activeTabOverride,
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
		panelCount,
		parentScope,
		runtime,
	});
	captureScope(actions.scope);
	return null;
}

afterEach(() => cleanup());

describe('useWorkbenchPanelActions', () => {
	it.each([
		{ closeActivity: 'idle' as const, confirmationCount: 0 },
		{ closeActivity: 'active' as const, confirmationCount: 1 },
		{ closeActivity: 'unknown' as const, confirmationCount: 1 },
	])(
		'requests $confirmationCount confirmations when a running terminal is $closeActivity',
		async ({ closeActivity, confirmationCount }) => {
			const confirm = vi.fn<() => boolean>(() => true);
			const runtime = new ActionRuntime({ confirm });
			const parentScope = runtime.createScope('workbench');
			const closeDisplayedTerminal = vi.fn<() => PanelWorkspaceTransition>(() => appliedTransition);
			let scope: ActionScope | undefined;
			if (activeTab.root.kind !== 'panel') throw new Error('Expected a panel fixture');
			const candidateTab: TabPanelWorkspace = {
				...activeTab,
				root: {
					...activeTab.root,
					primaryPresentation: {
						...activeTab.root.primaryPresentation,
						closeActivity,
					},
				},
			};
			function CloseHarness(): ReactNode {
				const actions = useWorkbenchPanelActions({
					activeTab: candidateTab,
					operations: {
						activateChildTerminal: () => appliedTransition,
						closeDisplayedTerminal,
						createChildTerminal: () => appliedTransition,
						focusPanel: () => appliedTransition,
						movePanel: () => appliedTransition,
						showPrimaryTerminal: () => appliedTransition,
						splitPanel: () => ({ panelId: 'panel-2', transition: appliedTransition }),
						togglePanelMaximization: () => appliedTransition,
					},
					panelCount: 2,
					parentScope,
					runtime,
				});
				scope = actions.scope;
				return null;
			}
			render(createElement(CloseHarness));
			if (scope === undefined) throw new Error('Expected the panel action scope to be captured');

			const execution = runtime.execute(
				PANEL_ACTION_IDS.close,
				{ childPresentationId: null, panelId: 'panel-1', tabId: 'tab-1' },
				{ scope, source: 'button' },
			);

			await expect(execution.completion).resolves.toEqual({ kind: 'completed' });
			expect(confirm).toHaveBeenCalledTimes(confirmationCount);
			expect(closeDisplayedTerminal).toHaveBeenCalledOnce();
		},
	);

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
								childPresentationId: 'terminal-2',
								panelId: 'panel-1',
							},
							[PANEL_ACTION_IDS.close]: {
								childPresentationId: null,
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
