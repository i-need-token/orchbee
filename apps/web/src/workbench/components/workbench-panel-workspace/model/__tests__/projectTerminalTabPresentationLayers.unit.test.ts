import { describe, expect, it } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

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
				currentCwd: null,
				lifecycle: { kind: 'running' },
				naturalTitle: null,
				presentationId: terminalPresentationId(`${tabId}-presentation`),
				title: tabId,
			},
		},
		tabId,
	};
}

describe('projectTerminalTabPresentationLayers', () => {
	it('orders an incoming revision below the retained displayed revision', () => {
		const displayed = tab('tab-a');
		const target = tab('tab-b');
		const state: TerminalTabPresentationHandoffState = {
			activationReadyPresentationIds: [],
			displayedRevision: 'scope:tab-a:layout-1',
			displayedTabId: 'tab-a',
			displayReadyPresentationIds: [],
			epoch: 1,
			expectedPresentationIds: ['tab-b-presentation'],
			kind: 'preparing',
			targetRevision: 'scope:tab-b:layout-1',
			targetTabId: 'tab-b',
		};

		expect(projectTerminalTabPresentationLayers(target, state, displayed)).toMatchObject([
			{
				epoch: 1,
				mode: 'incoming',
				revision: 'scope:tab-b:layout-1',
				tab: { tabId: 'tab-b' },
			},
			{
				epoch: 1,
				mode: 'retained',
				revision: 'scope:tab-a:layout-1',
				tab: { tabId: 'tab-a' },
			},
		]);
	});

	it('keeps distinct layers when one Tab replaces its layout revision', () => {
		const displayed = tab('tab-a');
		const target = { ...displayed, maximizedPanelId: displayed.root.id };
		const state: TerminalTabPresentationHandoffState = {
			activationReadyPresentationIds: [],
			displayedRevision: 'scope:tab-a:layout-1',
			displayedTabId: 'tab-a',
			displayReadyPresentationIds: [],
			epoch: 2,
			expectedPresentationIds: ['tab-a-presentation'],
			kind: 'preparing',
			targetRevision: 'scope:tab-a:layout-2',
			targetTabId: 'tab-a',
		};

		expect(projectTerminalTabPresentationLayers(target, state, displayed)).toMatchObject([
			{ mode: 'incoming', revision: 'scope:tab-a:layout-2' },
			{ mode: 'retained', revision: 'scope:tab-a:layout-1' },
		]);
	});

	it('reuses one keyed layer when returning to the frozen displayed revision', () => {
		const target = tab('tab-a');
		const state: TerminalTabPresentationHandoffState = {
			activationReadyPresentationIds: [],
			displayedRevision: 'scope:tab-a:layout-1',
			displayedTabId: 'tab-a',
			displayReadyPresentationIds: [],
			epoch: 3,
			expectedPresentationIds: ['tab-a-presentation'],
			kind: 'preparing',
			targetRevision: 'scope:tab-a:layout-1',
			targetTabId: 'tab-a',
		};

		expect(projectTerminalTabPresentationLayers(target, state, target)).toMatchObject([
			{ mode: 'incoming', revision: 'scope:tab-a:layout-1' },
		]);
	});

	it('projects only the inert target after its display barrier commits', () => {
		const state: TerminalTabPresentationHandoffState = {
			activationReadyPresentationIds: [],
			displayedRevision: 'scope:tab-b:layout-1',
			displayedTabId: 'tab-b',
			displayReadyPresentationIds: ['tab-b-presentation'],
			epoch: 2,
			expectedPresentationIds: ['tab-b-presentation'],
			kind: 'activating',
			targetRevision: 'scope:tab-b:layout-1',
			targetTabId: 'tab-b',
		};

		expect(projectTerminalTabPresentationLayers(tab('tab-b'), state, tab('tab-b'))).toMatchObject([
			{ epoch: 2, mode: 'incoming', revision: 'scope:tab-b:layout-1' },
		]);
	});
});
