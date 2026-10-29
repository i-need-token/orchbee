import { describe, expect, it } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import {
	initializePanelWorkspace,
	transitionPanelWorkspace,
	type PanelLayoutNode,
	type PanelWorkspaceState,
} from '../panel-workspace-state';
import {
	resolveDirectionalPanelFocus,
	type PanelFocusDirection,
} from '../resolveDirectionalPanelFocus';

describe('resolveDirectionalPanelFocus', () => {
	it('traverses a four-Panel grid by visual direction without wrapping at an outer edge', () => {
		const state = fourPanelGrid();
		const root = state.tabs[0]?.root;
		if (root === undefined) throw new Error('Expected the test Tab.');

		expect(resolve(root, 'panel-4', 'up')).toBe('panel-1');
		expect(resolve(root, 'panel-1', 'right')).toBe('panel-2');
		expect(resolve(root, 'panel-2', 'down')).toBe('panel-3');
		expect(resolve(root, 'panel-3', 'left')).toBe('panel-4');
		expect(resolve(root, 'panel-1', 'up')).toBeNull();
		expect(resolve(root, 'panel-1', 'left')).toBeNull();
		expect(resolve(root, 'panel-3', 'right')).toBeNull();
		expect(resolve(root, 'panel-4', 'down')).toBeNull();
	});

	it('uses perpendicular center distance and then visual order for multiple adjacent candidates', () => {
		const equal = threePanelLayout();
		const equalRoot = equal.tabs[0]?.root;
		if (equalRoot === undefined) throw new Error('Expected the test Tab.');
		expect(resolve(equalRoot, 'panel-1', 'right')).toBe('panel-2');

		if (equalRoot.kind !== 'split') throw new Error('Expected the root Split.');
		const right = equalRoot.children[1]?.node;
		if (right?.kind !== 'split') throw new Error('Expected the nested right Split.');
		const resized = transitionPanelWorkspace(equal, {
			leadingWeight: 25,
			separatorIndex: 0,
			splitId: right.id,
			tabId: 'tab-1',
			type: 'adjacent-panels-resized',
		}).state;
		const resizedRoot = resized.tabs[0]?.root;
		if (resizedRoot === undefined) throw new Error('Expected the resized test Tab.');

		expect(resolve(resizedRoot, 'panel-1', 'right')).toBe('panel-3');
	});

	it('returns null for a missing source and a singleton layout', () => {
		const state = initialState();
		const root = state.tabs[0]?.root;
		if (root === undefined) throw new Error('Expected the test Tab.');

		expect(resolve(root, 'missing-panel', 'right')).toBeNull();
		expect(resolve(root, 'panel-1', 'right')).toBeNull();
	});
});

function resolve(
	root: PanelLayoutNode,
	panelId: string,
	direction: PanelFocusDirection,
): string | null {
	return resolveDirectionalPanelFocus(root, panelId, direction);
}

function initialState(): PanelWorkspaceState {
	return initializePanelWorkspace([
		{
			id: 'tab-1',
			presentation: {
				kind: 'preview',
				presentationId: terminalPresentationId('terminal-1'),
			},
			title: 'Terminal 1',
		},
	]).state;
}

function split(
	state: PanelWorkspaceState,
	panelId: string,
	direction: 'below' | 'right',
	terminalSequence: number,
): PanelWorkspaceState {
	return transitionPanelWorkspace(state, {
		direction,
		newTerminalTitle: `Terminal ${terminalSequence}`,
		panelId,
		presentation: {
			kind: 'preview',
			presentationId: terminalPresentationId(`terminal-${terminalSequence}`),
		},
		tabId: 'tab-1',
		type: 'panel-split',
	}).state;
}

function threePanelLayout(): PanelWorkspaceState {
	return split(split(initialState(), 'panel-1', 'right', 2), 'panel-2', 'below', 3);
}

function fourPanelGrid(): PanelWorkspaceState {
	return split(threePanelLayout(), 'panel-1', 'below', 4);
}
