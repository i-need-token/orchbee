import { describe, expect, it } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import type { TabPanelWorkspace } from '../panel-workspace-state';
import { projectTerminalTabPresentationRevision } from '../projectTerminalTabPresentationRevision';
import { resolveTerminalTabPresentationDisplayedTab } from '../resolveTerminalTabPresentationDisplayedTab';
import type { TerminalTabPresentationHandoffState } from '../transitionTerminalTabPresentationHandoff';

function tab(presentationId: string): TabPanelWorkspace {
	return {
		focusedPanelId: 'panel-a',
		maximizedPanelId: null,
		panelFocusHistory: ['panel-a'],
		root: {
			activeChildPresentationId: null,
			childPresentations: [],
			id: 'panel-a',
			kind: 'panel',
			primaryPresentation: {
				closeActivity: 'idle',
				currentCwd: null,
				lifecycle: { kind: 'running' },
				naturalTitle: null,
				presentationId: terminalPresentationId(presentationId),
				title: presentationId,
			},
		},
		tabId: 'tab-a',
	};
}

describe('resolveTerminalTabPresentationDisplayedTab', () => {
	it('returns the active snapshot when its exact revision is displayed', () => {
		const activeTab = tab('active');
		const revision = projectTerminalTabPresentationRevision('scope-a', activeTab);
		const state: TerminalTabPresentationHandoffState = {
			displayedRevision: revision,
			displayedTabId: activeTab.tabId,
			epoch: 0,
			kind: 'settled',
		};

		expect(
			resolveTerminalTabPresentationDisplayedTab({
				activeTab,
				currentDisplayedTab: null,
				presentationScopeId: 'scope-a',
				state,
				tabs: [activeTab],
				targetRevision: revision,
			}),
		).toBe(activeTab);
	});

	it('retains a detached same-Tab snapshot while a newer revision prepares', () => {
		const displayedTab = tab('displayed');
		const activeTab = tab('target');
		const displayedRevision = projectTerminalTabPresentationRevision('scope-a', displayedTab);
		const targetRevision = projectTerminalTabPresentationRevision('scope-a', activeTab);
		if (displayedRevision === null || targetRevision === null) {
			throw new Error('Expected concrete layout revisions.');
		}
		const state: TerminalTabPresentationHandoffState = {
			activationReadyPresentationIds: [],
			displayedRevision,
			displayedTabId: displayedTab.tabId,
			displayReadyPresentationIds: [],
			epoch: 1,
			expectedPresentationIds: ['target'],
			kind: 'preparing',
			targetRevision,
			targetTabId: activeTab.tabId,
		};

		expect(
			resolveTerminalTabPresentationDisplayedTab({
				activeTab,
				currentDisplayedTab: displayedTab,
				presentationScopeId: 'scope-a',
				state,
				tabs: [activeTab],
				targetRevision,
			}),
		).toBe(displayedTab);
	});

	it('returns no snapshot when the handoff has no displayed revision', () => {
		const activeTab = tab('active');
		expect(
			resolveTerminalTabPresentationDisplayedTab({
				activeTab,
				currentDisplayedTab: activeTab,
				presentationScopeId: 'scope-a',
				state: { displayedRevision: null, displayedTabId: null, epoch: 0, kind: 'settled' },
				tabs: [activeTab],
				targetRevision: null,
			}),
		).toBeNull();
	});
});
