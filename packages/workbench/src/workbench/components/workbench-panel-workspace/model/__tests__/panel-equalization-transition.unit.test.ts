import { describe, expect, it } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import {
	collectPanelLeaves,
	initializePanelWorkspace,
	transitionPanelWorkspace,
	type PanelSplit,
	type PanelWorkspaceState,
} from '../panel-workspace-state';

describe('panel equalization transition', () => {
	it('commits proportional whole-Tab weights without changing focus or identity', () => {
		const state = threeHorizontalPanels();
		const tabBefore = requireTab(state);
		const panelsBefore = collectPanelLeaves(tabBefore.root);

		const result = transitionPanelWorkspace(state, {
			panelId: 'panel-1',
			tabId: 'tab-1',
			type: 'panels-equalized',
		});

		expect(result).toMatchObject({ outcome: 'applied', rejection: null });
		expect(result.commands).toEqual([]);
		const tabAfter = requireTab(result.state);
		const root = requireSplit(tabAfter.root);
		const nested = requireSplit(root.children[1]?.node);
		expect(root.children[0]?.weight).toBeCloseTo(100 / 3, 12);
		expect(root.children[1]?.weight).toBeCloseTo(200 / 3, 12);
		expect(nested.children.map((child) => child.weight)).toEqual([50, 50]);
		expect(tabAfter.focusedPanelId).toBe(tabBefore.focusedPanelId);
		expect(tabAfter.panelFocusHistory).toBe(tabBefore.panelFocusHistory);
		expect({
			panel: result.state.nextPanelSequence,
			slot: result.state.nextPanelSlotSequence,
			split: result.state.nextSplitSequence,
			terminal: result.state.nextTerminalSequence,
		}).toEqual({
			panel: state.nextPanelSequence,
			slot: state.nextPanelSlotSequence,
			split: state.nextSplitSequence,
			terminal: state.nextTerminalSequence,
		});
		for (const [index, panel] of panelsBefore.entries()) {
			expect(collectPanelLeaves(tabAfter.root)[index]).toBe(panel);
		}
	});

	it('is unchanged after the proportional layout has already committed', () => {
		const state = threeHorizontalPanels();
		const first = transitionPanelWorkspace(state, {
			panelId: 'panel-3',
			tabId: 'tab-1',
			type: 'panels-equalized',
		});

		expect(
			transitionPanelWorkspace(first.state, {
				panelId: 'panel-2',
				tabId: 'tab-1',
				type: 'panels-equalized',
			}),
		).toMatchObject({ outcome: 'unchanged', rejection: null, state: first.state });
	});

	it('rejects singleton, maximized, and stale exact targets without changing state', () => {
		const singleton = initialState();
		const splitState = threeHorizontalPanels();
		const maximized = transitionPanelWorkspace(splitState, {
			panelId: 'panel-3',
			tabId: 'tab-1',
			type: 'panel-maximization-toggled',
		}).state;

		expect(
			transitionPanelWorkspace(singleton, {
				panelId: 'panel-1',
				tabId: 'tab-1',
				type: 'panels-equalized',
			}),
		).toMatchObject({
			outcome: 'rejected',
			rejection: 'requires-multiple-panels',
			state: singleton,
		});
		expect(
			transitionPanelWorkspace(maximized, {
				panelId: 'panel-3',
				tabId: 'tab-1',
				type: 'panels-equalized',
			}),
		).toMatchObject({ outcome: 'rejected', rejection: 'panel-maximized', state: maximized });
		expect(
			transitionPanelWorkspace(splitState, {
				panelId: 'missing-panel',
				tabId: 'tab-1',
				type: 'panels-equalized',
			}),
		).toMatchObject({ outcome: 'rejected', rejection: 'panel-not-found', state: splitState });
	});
});

function initialState(): PanelWorkspaceState {
	return initializePanelWorkspace([
		{
			id: 'tab-1',
			presentation: {
				kind: 'preview',
				presentationId: terminalPresentationId('terminal-1'),
			},
			title: 'Terminal',
		},
	]).state;
}

function threeHorizontalPanels(): PanelWorkspaceState {
	let state = transitionPanelWorkspace(initialState(), {
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
	state = transitionPanelWorkspace(state, {
		direction: 'right',
		newTerminalTitle: 'Terminal 3',
		panelId: 'panel-2',
		presentation: {
			kind: 'preview',
			presentationId: terminalPresentationId('terminal-3'),
		},
		tabId: 'tab-1',
		type: 'panel-split',
	}).state;
	return state;
}

function requireTab(state: PanelWorkspaceState) {
	const tab = state.tabs[0];
	if (tab === undefined) throw new Error('Expected the test Tab.');
	return tab;
}

function requireSplit(node: PanelSplit['children'][number]['node'] | undefined): PanelSplit {
	if (node === undefined || node.kind !== 'split') throw new Error('Expected a Split node.');
	return node;
}
