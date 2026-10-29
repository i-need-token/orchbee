// @vitest-environment jsdom

import { ActionRuntime, type ActionScope } from '@orchbee/actions';
import { cleanup, render } from '@testing-library/react';
import { createElement, type ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import { PANEL_ACTION_IDS } from '../../actions/panel-action-contracts';
import {
	transitionPanelWorkspace,
	type PanelWorkspaceState,
	type PanelWorkspaceTransition,
	type TabPanelWorkspace,
} from '../../model/panel-workspace-state';
import { useWorkbenchPanelActions } from '../useWorkbenchPanelActions';

const activeTab: TabPanelWorkspace = {
	focusedPanelId: 'panel-1',
	maximizedPanelId: null,
	panelFocusHistory: ['panel-1'],
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
	readonly closeDisplayedTerminal?: () => PanelWorkspaceTransition;
	readonly createChildTerminal: (panelId: string) => PanelWorkspaceTransition | null;
	readonly equalizePanels?: (panelId: string) => PanelWorkspaceTransition;
	readonly panelCount?: number;
	readonly parentScope: ActionScope;
	readonly runtime: ActionRuntime;
	readonly swapPanels?: (panelId: string, targetPanelId: string) => PanelWorkspaceTransition;
	readonly startPanelVisualSelection?: (panelId: string, mode: 'focus' | 'swap') => boolean;
}

function Harness({
	activeTab: activeTabOverride = activeTab,
	captureScope,
	closeDisplayedTerminal = () => appliedTransition,
	createChildTerminal,
	equalizePanels = () => appliedTransition,
	panelCount = 1,
	parentScope,
	runtime,
	startPanelVisualSelection = () => true,
	swapPanels = () => appliedTransition,
}: HarnessProps): ReactNode {
	const actions = useWorkbenchPanelActions({
		activeTab: activeTabOverride,
		operations: {
			activateChildTerminal: () => appliedTransition,
			canStartBroadcastInput: () => true,
			closeDisplayedTerminal,
			createChildTerminal,
			equalizePanels,
			focusPanel: () => appliedTransition,
			movePanel: () => appliedTransition,
			resizePanel: () => appliedTransition,
			showPrimaryTerminal: () => appliedTransition,
			splitPanel: () => ({ panelId: 'panel-2', transition: appliedTransition }),
			startPanelResize: () => true,
			startBroadcastInput: () => true,
			startPanelVisualSelection,
			swapPanels,
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
						canStartBroadcastInput: () => true,
						closeDisplayedTerminal,
						createChildTerminal: () => appliedTransition,
						equalizePanels: () => appliedTransition,
						focusPanel: () => appliedTransition,
						movePanel: () => appliedTransition,
						resizePanel: () => appliedTransition,
						showPrimaryTerminal: () => appliedTransition,
						splitPanel: () => ({ panelId: 'panel-2', transition: appliedTransition }),
						startPanelResize: () => true,
						startBroadcastInput: () => true,
						startPanelVisualSelection: () => true,
						swapPanels: () => appliedTransition,
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

	it('uses an immediate unknown metadata observation for close Action confirmation', async () => {
		if (activeTab.root.kind !== 'panel') throw new Error('Expected a panel fixture');
		const presentationId = activeTab.root.primaryPresentation.presentationId;
		const idleTab: TabPanelWorkspace = {
			...activeTab,
			root: {
				...activeTab.root,
				primaryPresentation: {
					...activeTab.root.primaryPresentation,
					closeActivity: 'idle',
				},
			},
		};
		const state: PanelWorkspaceState = { ...appliedTransition.state, tabs: [idleTab] };
		const observation = transitionPanelWorkspace(state, {
			closeActivity: 'unknown',
			naturalTitle: null,
			presentationId,
			tabId: idleTab.tabId,
			type: 'terminal-metadata-changed',
		});
		const observedTab = observation.state.tabs[0];
		expect(observation.outcome).toBe('applied');
		expect(observedTab?.root).toMatchObject({
			primaryPresentation: { closeActivity: 'unknown' },
		});
		if (observedTab === undefined) throw new Error('Expected the observed Tab state');

		const confirm = vi.fn<() => boolean>(() => true);
		const runtime = new ActionRuntime({ confirm });
		const parentScope = runtime.createScope('workbench');
		const closeDisplayedTerminal = vi.fn<() => PanelWorkspaceTransition>(() => appliedTransition);
		let scope: ActionScope | undefined;
		render(
			createElement(Harness, {
				activeTab: observedTab,
				captureScope: (candidate) => {
					scope = candidate;
				},
				closeDisplayedTerminal,
				createChildTerminal: () => appliedTransition,
				panelCount: 2,
				parentScope,
				runtime,
			}),
		);
		if (scope === undefined) throw new Error('Expected the panel action scope to be captured');

		const execution = runtime.execute(
			PANEL_ACTION_IDS.close,
			{ childPresentationId: null, panelId: 'panel-1', tabId: 'tab-1' },
			{ scope, source: 'button' },
		);

		await expect(execution.completion).resolves.toEqual({ kind: 'completed' });
		expect(confirm).toHaveBeenCalledOnce();
		expect(closeDisplayedTerminal).toHaveBeenCalledOnce();
	});

	it('swaps two exact live Panels through the lower Panel Action', async () => {
		const splitState = transitionPanelWorkspace(appliedTransition.state, {
			direction: 'right',
			newTerminalTitle: 'Terminal 2',
			panelId: 'panel-1',
			presentation: {
				kind: 'preview',
				presentationId: terminalPresentationId('terminal-2'),
			},
			tabId: 'tab-1',
			type: 'panel-split',
		}).state;
		const splitTab = splitState.tabs[0];
		if (splitTab === undefined) throw new Error('Expected a split Tab fixture');
		const runtime = new ActionRuntime();
		const parentScope = runtime.createScope('workbench');
		const swapPanels = vi.fn<(panelId: string, targetPanelId: string) => PanelWorkspaceTransition>(
			() => appliedTransition,
		);
		let scope: ActionScope | undefined;
		render(
			createElement(Harness, {
				activeTab: splitTab,
				captureScope: (candidate) => {
					scope = candidate;
				},
				createChildTerminal: () => appliedTransition,
				panelCount: 2,
				parentScope,
				runtime,
				swapPanels,
			}),
		);
		if (scope === undefined) throw new Error('Expected the panel action scope to be captured');

		const execution = runtime.execute(
			PANEL_ACTION_IDS.swap,
			{ panelId: 'panel-1', targetPanelId: 'panel-2' },
			{ scope, source: 'keyboard' },
		);

		await expect(execution.completion).resolves.toEqual({ kind: 'completed' });
		expect(swapPanels).toHaveBeenCalledWith('panel-1', 'panel-2');
	});

	it('starts visual selection for an exact source when at least three Panels are visible', async () => {
		const runtime = new ActionRuntime();
		const parentScope = runtime.createScope('workbench');
		const startPanelVisualSelection = vi.fn<(panelId: string, mode: 'focus' | 'swap') => boolean>(
			() => true,
		);
		let scope: ActionScope | undefined;
		render(
			createElement(Harness, {
				captureScope: (candidate) => {
					scope = candidate;
				},
				createChildTerminal: () => appliedTransition,
				panelCount: 3,
				parentScope,
				runtime,
				startPanelVisualSelection,
			}),
		);
		if (scope === undefined) throw new Error('Expected the panel action scope to be captured');

		await expect(
			runtime.execute(
				PANEL_ACTION_IDS.startVisualSelection,
				{ mode: 'focus', panelId: 'panel-1' },
				{ scope, source: 'keyboard' },
			).completion,
		).resolves.toEqual({ kind: 'completed' });
		expect(startPanelVisualSelection).toHaveBeenCalledWith('panel-1', 'focus');
	});

	it('equalizes one exact live Tab target and preserves an idempotent unchanged result', async () => {
		const runtime = new ActionRuntime();
		const parentScope = runtime.createScope('workbench');
		const unchangedTransition: PanelWorkspaceTransition = {
			...appliedTransition,
			outcome: 'unchanged',
		};
		const equalizePanels = vi
			.fn<(panelId: string) => PanelWorkspaceTransition>()
			.mockReturnValueOnce(appliedTransition)
			.mockReturnValueOnce(unchangedTransition);
		let scope: ActionScope | undefined;
		render(
			createElement(Harness, {
				captureScope: (candidate) => {
					scope = candidate;
				},
				createChildTerminal: () => appliedTransition,
				equalizePanels,
				panelCount: 2,
				parentScope,
				runtime,
			}),
		);
		if (scope === undefined) throw new Error('Expected the panel action scope to be captured');

		const first = runtime.execute(
			PANEL_ACTION_IDS.equalize,
			{ panelId: 'panel-1' },
			{ scope, source: 'keyboard' },
		);
		const second = runtime.execute(
			PANEL_ACTION_IDS.equalize,
			{ panelId: 'panel-1' },
			{ scope, source: 'menu' },
		);

		await expect(first.completion).resolves.toEqual({ kind: 'completed' });
		await expect(second.completion).resolves.toEqual({ kind: 'unchanged' });
		expect(equalizePanels).toHaveBeenNthCalledWith(1, 'panel-1');
		expect(equalizePanels).toHaveBeenNthCalledWith(2, 'panel-1');
	});

	it.each([
		{ label: 'singleton', panelCount: 1, tab: activeTab },
		{
			label: 'maximized',
			panelCount: 2,
			tab: { ...activeTab, maximizedPanelId: 'panel-1' },
		},
	])('keeps Panel equalization disabled for a $label Tab', async ({ panelCount, tab }) => {
		const runtime = new ActionRuntime();
		const parentScope = runtime.createScope('workbench');
		const equalizePanels = vi.fn<(panelId: string) => PanelWorkspaceTransition>(
			() => appliedTransition,
		);
		let scope: ActionScope | undefined;
		render(
			createElement(Harness, {
				activeTab: tab,
				captureScope: (candidate) => {
					scope = candidate;
				},
				createChildTerminal: () => appliedTransition,
				equalizePanels,
				panelCount,
				parentScope,
				runtime,
			}),
		);
		if (scope === undefined) throw new Error('Expected the panel action scope to be captured');

		await expect(
			runtime.execute(
				PANEL_ACTION_IDS.equalize,
				{ panelId: 'panel-1' },
				{ scope, source: 'keyboard' },
			).completion,
		).resolves.toMatchObject({ code: 'action-unavailable', kind: 'blocked' });
		expect(equalizePanels).not.toHaveBeenCalled();
	});

	it('registers panel semantics, calls the domain writer, and disposes with its owner', async () => {
		const runtime = new ActionRuntime();
		const parentScope = runtime.createScope('workbench');
		const createChildTerminal = vi.fn<(panelId: string) => PanelWorkspaceTransition>(
			() => appliedTransition,
		);
		const swapPanels = vi.fn<(panelId: string, targetPanelId: string) => PanelWorkspaceTransition>(
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
				swapPanels,
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
							[PANEL_ACTION_IDS.equalize]: { panelId: 'panel-1' },
							[PANEL_ACTION_IDS.focus]: { panelId: 'panel-1' },
							[PANEL_ACTION_IDS.move]: {
								direction: 'right',
								panelId: 'panel-1',
								targetPanelId: 'panel-1',
							},
							[PANEL_ACTION_IDS.resize]: {
								adjustment: 'wider',
								multiplier: 1,
								panelId: 'panel-1',
							},
							[PANEL_ACTION_IDS.showPrimaryTerminal]: { panelId: 'panel-1' },
							[PANEL_ACTION_IDS.split]: {
								direction: 'right',
								panelId: 'panel-1',
							},
							[PANEL_ACTION_IDS.startBroadcastInput]: { panelId: 'panel-1' },
							[PANEL_ACTION_IDS.startResize]: { panelId: 'panel-1' },
							[PANEL_ACTION_IDS.startVisualSelection]: {
								mode: 'focus',
								panelId: 'panel-1',
							},
							[PANEL_ACTION_IDS.swap]: {
								panelId: 'panel-1',
								targetPanelId: 'panel-2',
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

		const swapExecution = runtime.execute(
			PANEL_ACTION_IDS.swap,
			{ panelId: 'panel-1', targetPanelId: 'panel-2' },
			{ scope, source: 'keyboard' },
		);
		await expect(swapExecution.completion).resolves.toMatchObject({
			code: 'action-unavailable',
			kind: 'blocked',
		});
		expect(swapPanels).not.toHaveBeenCalled();
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
