import { describe, expect, it } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import {
	findPanel,
	initializePanelWorkspace,
	transitionPanelWorkspace,
	type PanelSplit,
	type PanelWorkspaceState,
} from '../panel-workspace-state';

describe('panel keyboard swap transition', () => {
	it('commits an exact leaf exchange and keeps the moved Panel focused', () => {
		const state = splitRight(initialState());
		const beforeTab = requireTab(state);
		const source = findPanel(beforeTab.root, 'panel-2');
		const target = findPanel(beforeTab.root, 'panel-1');
		const sequences = {
			panel: state.nextPanelSequence,
			slot: state.nextPanelSlotSequence,
			split: state.nextSplitSequence,
			terminal: state.nextTerminalSequence,
		};

		const swapped = transitionPanelWorkspace(state, {
			panelId: 'panel-2',
			tabId: 'tab-1',
			targetPanelId: 'panel-1',
			type: 'panels-swapped',
		});

		expect(swapped).toMatchObject({ outcome: 'applied', rejection: null, commands: [] });
		const swappedTab = requireTab(swapped.state);
		const root = requireSplit(swappedTab.root);
		expect(root.children.map((child) => child.node.id)).toEqual(['panel-2', 'panel-1']);
		expect(root.children[0]?.node).toBe(source);
		expect(root.children[1]?.node).toBe(target);
		expect(swappedTab.focusedPanelId).toBe('panel-2');
		expect(swappedTab.panelFocusHistory.at(-1)).toBe('panel-2');
		expect({
			panel: swapped.state.nextPanelSequence,
			slot: swapped.state.nextPanelSlotSequence,
			split: swapped.state.nextSplitSequence,
			terminal: swapped.state.nextTerminalSequence,
		}).toEqual(sequences);
	});

	it('treats the same Panel as a no-op and rejects stale targets', () => {
		const state = splitRight(initialState());

		expect(
			transitionPanelWorkspace(state, {
				panelId: 'panel-1',
				tabId: 'tab-1',
				targetPanelId: 'panel-1',
				type: 'panels-swapped',
			}),
		).toMatchObject({ outcome: 'unchanged', rejection: null, state });
		expect(
			transitionPanelWorkspace(state, {
				panelId: 'missing-panel',
				tabId: 'tab-1',
				targetPanelId: 'panel-1',
				type: 'panels-swapped',
			}),
		).toMatchObject({ outcome: 'rejected', rejection: 'panel-not-found', state });
		expect(
			transitionPanelWorkspace(state, {
				panelId: 'panel-1',
				tabId: 'tab-1',
				targetPanelId: 'missing-panel',
				type: 'panels-swapped',
			}),
		).toMatchObject({ outcome: 'rejected', rejection: 'panel-not-found', state });
	});

	it('rejects a keyboard swap while the Tab is maximized', () => {
		const state = splitRight(initialState());
		const maximized = transitionPanelWorkspace(state, {
			panelId: 'panel-2',
			tabId: 'tab-1',
			type: 'panel-maximization-toggled',
		}).state;

		expect(
			transitionPanelWorkspace(maximized, {
				panelId: 'panel-2',
				tabId: 'tab-1',
				targetPanelId: 'panel-1',
				type: 'panels-swapped',
			}),
		).toMatchObject({ outcome: 'rejected', rejection: 'panel-maximized', state: maximized });
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

function splitRight(state: PanelWorkspaceState): PanelWorkspaceState {
	return transitionPanelWorkspace(state, {
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
}

function requireTab(state: PanelWorkspaceState) {
	const tab = state.tabs[0];
	if (tab === undefined) throw new Error('Expected the test Tab.');
	return tab;
}

function requireSplit(node: PanelWorkspaceState['tabs'][number]['root']): PanelSplit {
	if (node.kind !== 'split') throw new Error('Expected a Split root.');
	return node;
}
