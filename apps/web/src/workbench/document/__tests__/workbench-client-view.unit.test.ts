import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import {
	fixturePanelId,
	fixtureTabId,
	fixtureWorkspaceId,
	oneWorkspaceSnapshot,
	snapshotPanel,
	snapshotPublication,
	snapshotSplit,
	snapshotTab,
	workbenchSnapshot,
	type FixtureRecord,
} from '../../../__tests__/server-workbench-snapshot';
import {
	createDefaultWorkbenchClientView,
	createWorkbenchClientView,
	reconcileWorkbenchClientView,
} from '../workbench-client-view';

/** Panels `firstPanel` onward, each split from the rest; Splits are numbered like the Panels. */
function nestedPanels(firstPanel: number, panelCount: number): FixtureRecord {
	if (panelCount === 1) return snapshotPanel(firstPanel);
	return snapshotSplit(
		firstPanel,
		firstPanel % 2 === 0 ? 'horizontal' : 'vertical',
		snapshotPanel(firstPanel),
		nestedPanels(firstPanel + 1, panelCount - 1),
	);
}

/** Workspace 1 with one Tab per count, each holding that many Panels numbered after the last. */
function publicationWithPanels(panelCounts: readonly number[]) {
	return snapshotPublication(
		oneWorkspaceSnapshot(
			1,
			panelCounts.map((count, index) => {
				const before = panelCounts.slice(0, index).reduce((total, each) => total + each, 0);
				return snapshotTab(index + 1, nestedPanels(before + 1, count));
			}),
		),
	);
}

describe('Workbench Client View', () => {
	it('opens No Workspace by default while the Server publishes none', () => {
		expect(
			createDefaultWorkbenchClientView(
				snapshotPublication(workbenchSnapshot(1, { workspaces: [] }, [])),
			),
		).toEqual({
			activeLocation: { kind: 'no-workspace' },
			expandedProjectIds: [],
			recentsExpanded: true,
			terminalContexts: [],
			version: 1,
		});
	});

	it('drops a well-formed stale context', () => {
		const publication = publicationWithPanels([1]);
		const stale = createWorkbenchClientView({
			activeLocation: { kind: 'workspace', workspaceId: 'missing-workspace' },
			expandedProjectIds: ['missing-project'],
			recentsExpanded: true,
			terminalContexts: [
				{
					panelWorkspace: { tabs: [] },
					tabCollection: {
						activeTabId: null,
						collapsedGroupIds: [],
						mostRecentGroupId: null,
						selectedTabIds: [],
						selectionAnchorId: null,
					},
					terminalContextId: 'missing-context',
				},
			],
		});

		expect(reconcileWorkbenchClientView(publication, stale)).toEqual(
			createDefaultWorkbenchClientView(publication),
		);
	});

	it('derives a complete default Client View without changing the publication', () => {
		const publication = publicationWithPanels([1, 3]);
		const before = JSON.stringify(publication);
		const view = createDefaultWorkbenchClientView(publication);

		expect(view.activeLocation).toEqual({ kind: 'workspace', workspaceId: fixtureWorkspaceId(1) });
		expect(view.terminalContexts[0]?.tabCollection.activeTabId).toBe(fixtureTabId(1));
		expect(view.terminalContexts[0]?.panelWorkspace.tabs[1]?.focusedPanelId).toBe(
			fixturePanelId(2),
		);
		expect(JSON.stringify(publication)).toBe(before);
	});

	it('keeps two Client Views independent over one publication', () => {
		const publication = publicationWithPanels([1, 1]);
		const first = createDefaultWorkbenchClientView(publication);
		const firstContext = first.terminalContexts[0];
		if (firstContext === undefined) throw new Error('Expected one terminal context.');
		const second = createWorkbenchClientView({
			...first,
			terminalContexts: [
				{
					...firstContext,
					tabCollection: {
						...firstContext.tabCollection,
						activeTabId: fixtureTabId(2),
						selectedTabIds: [fixtureTabId(2)],
						selectionAnchorId: fixtureTabId(2),
					},
				},
			],
		});

		expect(reconcileWorkbenchClientView(publication, first)).toEqual(first);
		expect(reconcileWorkbenchClientView(publication, second)).toEqual(second);
		expect(first.terminalContexts[0]?.tabCollection.activeTabId).toBe(fixtureTabId(1));
		expect(second.terminalContexts[0]?.tabCollection.activeTabId).toBe(fixtureTabId(2));
	});

	it('deterministically removes stale Client View references and is idempotent', () => {
		const publication = publicationWithPanels([1, 2]);
		const defaultView = createDefaultWorkbenchClientView(publication);
		const context = defaultView.terminalContexts[0];
		const tab = context?.panelWorkspace.tabs[1];
		if (context === undefined || tab === undefined)
			throw new Error('Expected Client View fixtures.');
		const stale = createWorkbenchClientView({
			...defaultView,
			expandedProjectIds: ['missing-project'],
			terminalContexts: [
				{
					...context,
					panelWorkspace: {
						tabs: context.panelWorkspace.tabs.map((candidate) =>
							candidate.tabId === tab.tabId
								? {
										...candidate,
										focusedPanelId: 'missing-panel',
										maximizedPanelId: 'missing-panel',
									}
								: candidate,
						),
					},
					tabCollection: {
						...context.tabCollection,
						activeTabId: 'missing-tab',
						selectedTabIds: ['missing-tab'],
						selectionAnchorId: 'missing-tab',
					},
				},
			],
		});

		const reconciled = reconcileWorkbenchClientView(publication, stale);
		expect(reconciled.expandedProjectIds).toEqual([]);
		expect(reconciled.terminalContexts[0]?.tabCollection.activeTabId).toBe(fixtureTabId(1));
		expect(reconciled.terminalContexts[0]?.panelWorkspace.tabs[1]?.focusedPanelId).toBe(
			fixturePanelId(2),
		);
		expect(reconcileWorkbenchClientView(publication, reconciled)).toEqual(reconciled);
	});

	it('reconciles Client Views over generated hierarchies with a stable seed', () => {
		fc.assert(
			fc.property(
				fc.array(fc.integer({ max: 4, min: 1 }), { maxLength: 8, minLength: 1 }),
				(panelCounts) => {
					const publication = publicationWithPanels(panelCounts);
					const view = createDefaultWorkbenchClientView(publication);
					expect(reconcileWorkbenchClientView(publication, view)).toEqual(view);
				},
			),
			{ numRuns: 200, seed: 0x0b_ee_01 },
		);
	});
});
