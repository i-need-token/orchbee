import { describe, expect, it } from 'vitest';

import { terminalPresentationId } from '../../../../../terminal-presentation';
import type { TabPanelWorkspace } from '../panel-workspace-state';
import { projectTerminalTabPresentationLayers } from '../projectTerminalTabPresentationLayers';
import type { TerminalTabPresentationHandoffState } from '../transitionTerminalTabPresentationHandoff';

function tab(tabId: string): TabPanelWorkspace {
	return {
		focusedPanelId: `${tabId}-panel`,
		maximizedPanelId: null,
		panelFocusHistory: [`${tabId}-panel`],
		root: {
			activeChildPresentationId: null,
			childPresentations: [],
			id: `${tabId}-panel`,
			kind: 'panel',
			primaryPresentation: {
				closeActivity: 'idle',
				currentCwd: null,
				launchCwd: null,
				lifecycle: { kind: 'preview' },
				naturalTitle: null,
				presentationId: terminalPresentationId(`${tabId}-presentation`),
				title: tabId,
			},
		},
		tabId,
	};
}

describe('projectTerminalTabPresentationLayers', () => {
	it('orders an incoming target below the retained displayed Tab', () => {
		const tabs = [tab('tab-a'), tab('tab-b')];
		const state: TerminalTabPresentationHandoffState = {
			activationReadyPresentationIds: [],
			displayReadyPresentationIds: [],
			displayedTabId: 'tab-a',
			epoch: 1,
			expectedPresentationIds: ['tab-b-presentation'],
			kind: 'preparing',
			targetTabId: 'tab-b',
		};

		expect(projectTerminalTabPresentationLayers(tabs, state)).toMatchObject([
			{ epoch: 1, mode: 'incoming', tab: { tabId: 'tab-b' } },
			{ epoch: 1, mode: 'retained', tab: { tabId: 'tab-a' } },
		]);
	});

	it('projects only the inert target after its display barrier commits', () => {
		const state: TerminalTabPresentationHandoffState = {
			activationReadyPresentationIds: [],
			displayedTabId: 'tab-b',
			epoch: 2,
			expectedPresentationIds: ['tab-b-presentation'],
			kind: 'activating',
		};

		expect(projectTerminalTabPresentationLayers([tab('tab-b')], state)).toMatchObject([
			{ epoch: 2, mode: 'incoming', tab: { tabId: 'tab-b' } },
		]);
	});
});
