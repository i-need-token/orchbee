// @vitest-environment jsdom

import { ActionRuntime, type ActionResult, type ActionScope } from '@orchbee/actions';
import { cleanup, render } from '@testing-library/react';
import { createElement, type ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import type { WorkbenchSharedPanelTerminalCreate } from '../../../../actions/createSharedPanelTerminal';
import type { WorkbenchAuthority } from '../../../../types/workbench-authority';
import { PANEL_ACTION_IDS } from '../../actions/panel-action-contracts';
import {
	transitionPanelWorkspace,
	type PanelLeaf,
	type PanelWorkspaceState,
	type PanelWorkspaceTransition,
	type TabPanelWorkspace,
} from '../../model/panel-workspace-state';
import { useWorkbenchPanelActions } from '../useWorkbenchPanelActions';
import { inertWorkbenchAuthority } from '../../../../../__tests__/inertWorkbenchAuthority';

type AuthorityChange = WorkbenchAuthority['change'];
type AuthorityClose = WorkbenchAuthority['close'];

function panelLeaf(sequence: number): PanelLeaf {
	return {
		activeChildPresentationId: null,
		childPresentations: [],
		id: `panel-${sequence}`,
		kind: 'panel',
		primaryPresentation: {
			closeActivity: 'unknown',
			currentCwd: null,
			launchCwd: null,
			lifecycle: { kind: 'running' },
			naturalTitle: null,
			presentationId: terminalPresentationId(`terminal-${sequence}`),
			title: `Terminal ${sequence}`,
		},
	};
}

const activeTab: TabPanelWorkspace = {
	focusedPanelId: 'panel-1',
	maximizedPanelId: null,
	panelFocusHistory: ['panel-1'],
	root: panelLeaf(1),
	tabId: 'tab-1',
};

const splitTab: TabPanelWorkspace = {
	...activeTab,
	root: {
		children: [
			{ id: 'slot-1', node: panelLeaf(1), weight: 50 },
			{ id: 'slot-2', node: panelLeaf(2), weight: 50 },
		],
		id: 'split-1',
		kind: 'split',
		orientation: 'horizontal',
	},
};

const appliedTransition: PanelWorkspaceTransition = {
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

function unexpected(): Promise<never> {
	return Promise.reject(new Error('The test did not expect this authority call.'));
}

interface HarnessProps {
	readonly activeTab?: TabPanelWorkspace;
	readonly authority?: WorkbenchAuthority;
	readonly captureScope: (scope: ActionScope) => void;
	readonly createPanelTerminal?: WorkbenchSharedPanelTerminalCreate;
	readonly focusPanel?: (panelId: string) => PanelWorkspaceTransition;
	readonly panelCount?: number;
	readonly parentScope: ActionScope;
	readonly runtime: ActionRuntime;
	readonly startPanelVisualSelection?: (panelId: string, mode: 'focus' | 'swap') => boolean;
}

function Harness({
	activeTab: activeTabOverride = activeTab,
	authority = inertWorkbenchAuthority(),
	captureScope,
	createPanelTerminal = unexpected,
	focusPanel = () => appliedTransition,
	panelCount = 1,
	parentScope,
	runtime,
	startPanelVisualSelection = () => true,
}: HarnessProps): ReactNode {
	const actions = useWorkbenchPanelActions({
		activeTab: activeTabOverride,
		authority,
		createPanelTerminal,
		operations: {
			activateChildTerminal: () => appliedTransition,
			canStartBroadcastInput: () => true,
			focusPanel,
			showPrimaryTerminal: () => appliedTransition,
			startPanelResize: () => true,
			startBroadcastInput: () => true,
			startPanelVisualSelection,
			togglePanelMaximization: () => appliedTransition,
		},
		panelCount,
		parentScope,
		runtime,
		terminalContextId: 'terminal-context-1',
	});
	captureScope(actions.scope);
	return null;
}

function renderHarness(props: Omit<HarnessProps, 'captureScope'>): ActionScope {
	let scope: ActionScope | undefined;
	render(
		createElement(Harness, {
			...props,
			captureScope: (candidate) => {
				scope = candidate;
			},
		}),
	);
	if (scope === undefined) throw new Error('Expected the panel action scope to be captured');
	return scope;
}

const PANEL_CLOSURE = {
	panelId: 'panel-1',
	tabId: 'tab-1',
	terminalContextId: 'terminal-context-1',
	type: 'panel-close',
} as const;

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
			const close = vi.fn<AuthorityClose>(() => Promise.resolve({ kind: 'completed' }));
			const candidateTab: TabPanelWorkspace = {
				...activeTab,
				root: {
					...panelLeaf(1),
					primaryPresentation: { ...panelLeaf(1).primaryPresentation, closeActivity },
				},
			};
			const scope = renderHarness({
				activeTab: candidateTab,
				authority: inertWorkbenchAuthority({ close }),
				panelCount: 2,
				parentScope: runtime.createScope('workbench'),
				runtime,
			});

			const execution = runtime.execute(
				PANEL_ACTION_IDS.close,
				{ childPresentationId: null, panelId: 'panel-1', tabId: 'tab-1' },
				{ scope, source: 'button' },
			);

			await expect(execution.completion).resolves.toEqual({ kind: 'completed' });
			expect(confirm).toHaveBeenCalledTimes(confirmationCount);
			expect(close).toHaveBeenCalledExactlyOnceWith(expect.anything(), PANEL_CLOSURE);
		},
	);

	it('uses an immediate unknown metadata observation for close Action confirmation', async () => {
		const presentationId = panelLeaf(1).primaryPresentation.presentationId;
		const idleTab: TabPanelWorkspace = {
			...activeTab,
			root: {
				...panelLeaf(1),
				primaryPresentation: { ...panelLeaf(1).primaryPresentation, closeActivity: 'idle' },
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
		const close = vi.fn<AuthorityClose>(() => Promise.resolve({ kind: 'completed' }));
		const scope = renderHarness({
			activeTab: observedTab,
			authority: inertWorkbenchAuthority({ close }),
			panelCount: 2,
			parentScope: runtime.createScope('workbench'),
			runtime,
		});

		const execution = runtime.execute(
			PANEL_ACTION_IDS.close,
			{ childPresentationId: null, panelId: 'panel-1', tabId: 'tab-1' },
			{ scope, source: 'button' },
		);

		await expect(execution.completion).resolves.toEqual({ kind: 'completed' });
		expect(confirm).toHaveBeenCalledOnce();
		expect(close).toHaveBeenCalledOnce();
	});

	it('swaps two exact live Panels through the authority and focuses the moved Panel', async () => {
		const runtime = new ActionRuntime();
		const change = vi.fn<AuthorityChange>(() => Promise.resolve({ kind: 'completed' }));
		const focusPanel = vi.fn<(panelId: string) => PanelWorkspaceTransition>(
			() => appliedTransition,
		);
		const scope = renderHarness({
			activeTab: splitTab,
			authority: inertWorkbenchAuthority({ change }),
			focusPanel,
			panelCount: 2,
			parentScope: runtime.createScope('workbench'),
			runtime,
		});

		const execution = runtime.execute(
			PANEL_ACTION_IDS.swap,
			{ panelId: 'panel-1', targetPanelId: 'panel-2' },
			{ scope, source: 'keyboard' },
		);

		await expect(execution.completion).resolves.toEqual({ kind: 'completed' });
		expect(focusPanel).toHaveBeenCalledWith('panel-1');
		expect(change).toHaveBeenCalledExactlyOnceWith(expect.anything(), {
			panelId: 'panel-1',
			tabId: 'tab-1',
			targetPanelId: 'panel-2',
			terminalContextId: 'terminal-context-1',
			type: 'panels-swapped',
		});
	});

	it('starts visual selection for an exact source when at least three Panels are visible', async () => {
		const runtime = new ActionRuntime();
		const startPanelVisualSelection = vi.fn<(panelId: string, mode: 'focus' | 'swap') => boolean>(
			() => true,
		);
		const scope = renderHarness({
			panelCount: 3,
			parentScope: runtime.createScope('workbench'),
			runtime,
			startPanelVisualSelection,
		});

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
		const change = vi
			.fn<AuthorityChange>()
			.mockResolvedValueOnce({ kind: 'completed' })
			.mockResolvedValueOnce({ kind: 'unchanged' });
		const scope = renderHarness({
			authority: inertWorkbenchAuthority({ change }),
			panelCount: 2,
			parentScope: runtime.createScope('workbench'),
			runtime,
		});

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
		const equalization = {
			tabId: 'tab-1',
			terminalContextId: 'terminal-context-1',
			type: 'panels-equalized',
		};
		expect(change).toHaveBeenNthCalledWith(1, expect.anything(), equalization);
		expect(change).toHaveBeenNthCalledWith(2, expect.anything(), equalization);
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
		const change = vi.fn<AuthorityChange>(() => Promise.resolve({ kind: 'completed' }));
		const scope = renderHarness({
			activeTab: tab,
			authority: inertWorkbenchAuthority({ change }),
			panelCount,
			parentScope: runtime.createScope('workbench'),
			runtime,
		});

		await expect(
			runtime.execute(
				PANEL_ACTION_IDS.equalize,
				{ panelId: 'panel-1' },
				{ scope, source: 'keyboard' },
			).completion,
		).resolves.toMatchObject({ code: 'action-unavailable', kind: 'blocked' });
		expect(change).not.toHaveBeenCalled();
	});

	it('registers panel semantics, asks for a new terminal, and disposes with its owner', async () => {
		const runtime = new ActionRuntime();
		const parentScope = runtime.createScope('workbench');
		const createPanelTerminal = vi.fn<WorkbenchSharedPanelTerminalCreate>(
			(): Promise<ActionResult<string>> => Promise.resolve({ kind: 'completed', value: 'panel-1' }),
		);
		const change = vi.fn<AuthorityChange>(() => Promise.resolve({ kind: 'completed' }));
		let scope: ActionScope | undefined;
		const rendered = render(
			createElement(Harness, {
				authority: inertWorkbenchAuthority({ change }),
				captureScope: (candidate) => {
					scope = candidate;
				},
				createPanelTerminal,
				parentScope,
				runtime,
			}),
		);
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
		expect(createPanelTerminal).toHaveBeenCalledExactlyOnceWith(expect.anything(), {
			cwd: undefined,
			panelId: 'panel-1',
			tabId: 'tab-1',
			terminalContextId: 'terminal-context-1',
			type: 'child-terminal',
		});

		const swapExecution = runtime.execute(
			PANEL_ACTION_IDS.swap,
			{ panelId: 'panel-1', targetPanelId: 'panel-2' },
			{ scope, source: 'keyboard' },
		);
		await expect(swapExecution.completion).resolves.toMatchObject({
			code: 'action-unavailable',
			kind: 'blocked',
		});
		expect(change).not.toHaveBeenCalled();
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
