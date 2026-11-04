import {
	createWorkbenchSharedDocument,
	terminalPresentationId,
	workbenchSharedDocumentRevision,
	type WorkbenchSharedPanelNode,
} from '@orchbee/workbench-client-protocol';
import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import {
	createDefaultWorkbenchClientView,
	createWorkbenchClientView,
	parseWorkbenchClientView,
	reconcileWorkbenchClientView,
	type WorkbenchClientView,
} from '../workbench-client-view';

function emptySharedDocument() {
	return createWorkbenchSharedDocument({
		projectWorkspace: {
			nextResourceScopeSequence: 1,
			resourceScopes: [],
			workspaces: [],
			nextProjectSequence: 1,
			nextTerminalContextSequence: 2,
			nextWorkspaceSequence: 1,
			projects: [],
			standaloneTerminalContextId: 'terminal-context-1',
		},
		revision: workbenchSharedDocumentRevision('0'),
		terminalContexts: [
			{
				nextTabSequence: 1,
				panelWorkspace: {
					nextPanelSequence: 1,
					nextPanelSlotSequence: 1,
					nextSplitSequence: 1,
					nextTerminalSequence: 1,
					tabs: [],
				},
				tabCollection: { groups: [], tabs: [] },
				terminalContextId: 'terminal-context-1',
			},
		],
	});
}

function inputWithTabs(
	panelCounts: readonly number[] = [1],
): Parameters<typeof createWorkbenchSharedDocument>[0] {
	const terminalContextId = 'terminal-context-1';
	const tabs = panelCounts.map((_, index) => ({
		customTitle: null,
		group: null,
		id: `${terminalContextId}-tab-${index + 1}`,
		pinned: false,
		title: `Terminal ${index + 1}`,
	}));
	return {
		projectWorkspace: {
			nextResourceScopeSequence: 1,
			resourceScopes: [],
			workspaces: [],
			nextProjectSequence: 1,
			nextTerminalContextSequence: 2,
			nextWorkspaceSequence: 1,
			projects: [],
			standaloneTerminalContextId: terminalContextId,
		},
		revision: workbenchSharedDocumentRevision('0'),
		terminalContexts: [
			{
				nextTabSequence: tabs.length + 1,
				panelWorkspace: {
					nextPanelSequence: panelCounts.reduce((total, count) => total + count, 0) + 1,
					nextPanelSlotSequence: panelCounts.reduce(
						(total, count) => total + (count === 1 ? 0 : count),
						1,
					),
					nextSplitSequence: panelCounts.filter((count) => count > 1).length + 1,
					nextTerminalSequence: tabs.length + 1,
					tabs: panelCounts.map((count, index) => ({
						root: panelRoot(panelCounts, index, count),
						tabId: tabs[index]?.id ?? 'missing-tab',
					})),
				},
				tabCollection: { groups: [], tabs },
				terminalContextId,
			},
		],
	};
}

function panelOffset(panelCounts: readonly number[], tabIndex: number): number {
	return panelCounts.slice(0, tabIndex).reduce((total, count) => total + count, 1);
}

function panelRoot(
	panelCounts: readonly number[],
	tabIndex: number,
	panelCount: number,
): WorkbenchSharedPanelNode {
	const firstPanel = panelOffset(panelCounts, tabIndex);
	return nestedPanelRoot(firstPanel, panelCount, tabIndex, 0);
}

function nestedPanelRoot(
	firstPanel: number,
	panelCount: number,
	tabIndex: number,
	depth: number,
): WorkbenchSharedPanelNode {
	if (panelCount === 1) return panel(firstPanel);
	return {
		children: [
			{
				id: `panel-slot-${tabIndex + 1}-${depth + 1}-1`,
				node: panel(firstPanel),
			},
			{
				id: `panel-slot-${tabIndex + 1}-${depth + 1}-2`,
				node: nestedPanelRoot(firstPanel + 1, panelCount - 1, tabIndex, depth + 1),
			},
		],
		id: `split-${tabIndex + 1}-${depth + 1}`,
		kind: 'split',
		orientation: (tabIndex + depth) % 2 === 0 ? 'horizontal' : 'vertical',
	};
}

function panel(sequence: number): WorkbenchSharedPanelNode {
	return {
		childPresentations: [],
		id: `panel-${sequence}`,
		kind: 'panel',
		primaryPresentation: {
			intent: { kind: 'provision' },
			presentationId: terminalPresentationId(`presentation-${sequence}`),
			title: `Terminal ${sequence}`,
		},
	};
}

function parsedViewJson(view: WorkbenchClientView): WorkbenchClientView {
	return parseWorkbenchClientView(JSON.parse(JSON.stringify(view)));
}

describe('Workbench Client View', () => {
	it('creates and parses a deterministic disposable default', () => {
		const view = createDefaultWorkbenchClientView(emptySharedDocument());

		expect(parseWorkbenchClientView(JSON.parse(JSON.stringify(view)))).toEqual(view);
		expect(view).toEqual({
			activeLocation: { kind: 'standalone' },
			expandedProjectIds: [],
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
					terminalContextId: 'terminal-context-1',
				},
			],
			version: 1,
		});
	});

	it('drops a well-formed stale context and rejects unknown fields', () => {
		const document = emptySharedDocument();
		const stale = createWorkbenchClientView({
			activeLocation: { kind: 'standalone' },
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

		expect(reconcileWorkbenchClientView(document, stale)).toEqual(
			createDefaultWorkbenchClientView(document),
		);
		expect(() => parseWorkbenchClientView({ ...stale, attachmentId: 'forbidden' })).toThrow(
			/attachmentId/u,
		);
	});

	it('derives a complete default Client View without changing the shared document', () => {
		const document = createWorkbenchSharedDocument(inputWithTabs([1, 3]));
		const before = JSON.stringify(document);
		const view = createDefaultWorkbenchClientView(document);

		expect(parsedViewJson(view)).toEqual(view);
		expect(view.activeLocation).toEqual({ kind: 'standalone' });
		expect(view.terminalContexts[0]?.tabCollection.activeTabId).toBe('terminal-context-1-tab-1');
		expect(view.terminalContexts[0]?.panelWorkspace.tabs[1]?.splits[0]?.weights).toEqual([
			{ slotId: 'panel-slot-2-1-1', weight: 50 },
			{ slotId: 'panel-slot-2-1-2', weight: 50 },
		]);
		expect(JSON.stringify(document)).toBe(before);
	});

	it('keeps two Client Views independent over one shared document', () => {
		const document = createWorkbenchSharedDocument(inputWithTabs([1, 1]));
		const first = createDefaultWorkbenchClientView(document);
		const firstContext = first.terminalContexts[0];
		if (firstContext === undefined) throw new Error('Expected one terminal context.');
		const second = createWorkbenchClientView({
			...first,
			terminalContexts: [
				{
					...firstContext,
					tabCollection: {
						...firstContext.tabCollection,
						activeTabId: 'terminal-context-1-tab-2',
						selectedTabIds: ['terminal-context-1-tab-2'],
						selectionAnchorId: 'terminal-context-1-tab-2',
					},
				},
			],
		});

		expect(reconcileWorkbenchClientView(document, first)).toEqual(first);
		expect(reconcileWorkbenchClientView(document, second)).toEqual(second);
		expect(first.terminalContexts[0]?.tabCollection.activeTabId).toBe('terminal-context-1-tab-1');
		expect(second.terminalContexts[0]?.tabCollection.activeTabId).toBe('terminal-context-1-tab-2');
	});

	it('deterministically removes stale Client View references and is idempotent', () => {
		const document = createWorkbenchSharedDocument(inputWithTabs([1, 2]));
		const defaultView = createDefaultWorkbenchClientView(document);
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

		const reconciled = reconcileWorkbenchClientView(document, stale);
		expect(reconciled.expandedProjectIds).toEqual([]);
		expect(reconciled.terminalContexts[0]?.tabCollection.activeTabId).toBe(
			'terminal-context-1-tab-1',
		);
		expect(reconciled.terminalContexts[0]?.panelWorkspace.tabs[1]?.focusedPanelId).toBe('panel-2');
		expect(reconcileWorkbenchClientView(document, reconciled)).toEqual(reconciled);
	});

	it('reconciles Client Views over generated hierarchies with a stable seed', () => {
		fc.assert(
			fc.property(
				fc.array(fc.integer({ max: 4, min: 1 }), { maxLength: 8, minLength: 1 }),
				(panelCounts) => {
					const document = createWorkbenchSharedDocument(inputWithTabs(panelCounts));
					const view = createDefaultWorkbenchClientView(document);
					expect(parsedViewJson(view)).toEqual(view);
					expect(reconcileWorkbenchClientView(document, view)).toEqual(view);
				},
			),
			{ numRuns: 200, seed: 0x0b_ee_01 },
		);
	});
});
