import { terminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import { describe, expect, it } from 'vitest';

import {
	initializePanelWorkspace,
	transitionPanelWorkspace,
	type PanelSplit,
	type PanelWorkspaceState,
} from '../panel-workspace-state';

describe('panel keyboard resize transition', () => {
	it('commits the exact target adjustment through the canonical workspace state', () => {
		const state = splitRight(initialState());

		const resized = transitionPanelWorkspace(state, {
			adjustment: 'wider',
			multiplier: 1,
			panelId: 'panel-2',
			tabId: 'tab-1',
			type: 'panel-size-adjusted',
		});

		expect(resized).toMatchObject({ outcome: 'applied', rejection: null });
		expect(requireRootSplit(resized.state).children.map((child) => child.weight)).toEqual([45, 55]);
		expect(resized.commands).toEqual([]);
	});

	it('keeps missing axes and reached boundaries as intentional no-ops', () => {
		const state = splitRight(initialState());
		const narrower = transitionPanelWorkspace(state, {
			adjustment: 'narrower',
			multiplier: 2,
			panelId: 'panel-2',
			tabId: 'tab-1',
			type: 'panel-size-adjusted',
		});
		const shorter = transitionPanelWorkspace(state, {
			adjustment: 'shorter',
			multiplier: 1,
			panelId: 'panel-2',
			tabId: 'tab-1',
			type: 'panel-size-adjusted',
		});

		expect(narrower.outcome).toBe('applied');
		expect(shorter).toMatchObject({ outcome: 'unchanged', rejection: null, state });
	});

	it('rejects stale or maximized targets without mutating canonical weights', () => {
		const state = splitRight(initialState());
		const maximized = transitionPanelWorkspace(state, {
			panelId: 'panel-2',
			tabId: 'tab-1',
			type: 'panel-maximization-toggled',
		}).state;

		expect(
			transitionPanelWorkspace(state, {
				adjustment: 'wider',
				multiplier: 1,
				panelId: 'missing-panel',
				tabId: 'tab-1',
				type: 'panel-size-adjusted',
			}),
		).toMatchObject({ outcome: 'rejected', rejection: 'panel-not-found', state });
		expect(
			transitionPanelWorkspace(maximized, {
				adjustment: 'wider',
				multiplier: 1,
				panelId: 'panel-2',
				tabId: 'tab-1',
				type: 'panel-size-adjusted',
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

function requireRootSplit(state: PanelWorkspaceState): PanelSplit {
	const root = state.tabs[0]?.root;
	if (root?.kind !== 'split') throw new Error('Expected a split root.');
	return root;
}
