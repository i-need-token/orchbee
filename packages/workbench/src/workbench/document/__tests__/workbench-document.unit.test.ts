import {
	terminalPresentationId,
	terminalPresentationOperationId,
} from '../../../terminal-presentation';
import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import {
	WorkbenchSharedDocumentRevisionSchema,
	WorkbenchSharedDocumentSchema,
	createDefaultWorkbenchClientView,
	createWorkbenchClientView,
	createWorkbenchSharedDocument,
	parseWorkbenchClientView,
	parseWorkbenchSharedDocument,
	reconcileWorkbenchClientView,
	workbenchSharedDocumentRevision,
	type WorkbenchClientView,
	type WorkbenchSharedDocument,
	type WorkbenchSharedDocumentInput,
	type WorkbenchSharedPanelNode,
} from '..';

function inputWithTabs(panelCounts: readonly number[] = [1]): WorkbenchSharedDocumentInput {
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
					pendingRemovals: [],
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
			intent: {
				kind: 'provision',
				operationId: terminalPresentationOperationId(`operation-${sequence}`),
			},
			launchCwd: `/workspace/${sequence}`,
			presentationId: terminalPresentationId(`presentation-${sequence}`),
			title: `Terminal ${sequence}`,
		},
	};
}

function parsedDocumentJson(document: WorkbenchSharedDocument): WorkbenchSharedDocument {
	return parseWorkbenchSharedDocument(JSON.parse(JSON.stringify(document)));
}

function parsedViewJson(view: WorkbenchClientView): WorkbenchClientView {
	return parseWorkbenchClientView(JSON.parse(JSON.stringify(view)));
}

describe('Workbench shared document and Client View', () => {
	it('round-trips only shared product structure and terminal lifecycle intent', () => {
		const document = createWorkbenchSharedDocument(inputWithTabs([1, 3]));
		const serialized = JSON.stringify(document);
		const serializedWithoutCustomTitles = JSON.stringify(document, (key, value) =>
			key === 'customTitle' ? undefined : value,
		);

		expect(parsedDocumentJson(document)).toEqual(document);
		expect(
			parseWorkbenchSharedDocument(JSON.parse(serializedWithoutCustomTitles)).terminalContexts[0]
				?.tabCollection.tabs[0]?.customTitle,
		).toBeNull();
		expect(serialized).not.toContain('activeTabId');
		expect(serialized).not.toContain('activeLocation');
		expect(serialized).not.toContain('focusedPanelId');
		expect(serialized).not.toContain('weight');
		expect(serialized).not.toContain('currentCwd');
		expect(serialized).not.toContain('terminalId');
		expect(document).toMatchObject({ revision: '0', version: 1 });
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

	it('rejects invalid shared relationships, runtime authority, and revisions', () => {
		const document = createWorkbenchSharedDocument(inputWithTabs([1, 2]));
		const [terminalContext] = document.terminalContexts;
		if (terminalContext === undefined) throw new Error('Expected one terminal context.');

		expect(
			WorkbenchSharedDocumentSchema.safeParse({ ...document, terminalContexts: [] }).success,
		).toBe(false);
		expect(
			WorkbenchSharedDocumentSchema.safeParse({ ...document, terminalId: 'host-terminal' }).success,
		).toBe(false);
		expect(
			WorkbenchSharedDocumentSchema.safeParse({
				...document,
				terminalContexts: [
					{
						...terminalContext,
						panelWorkspace: {
							...terminalContext.panelWorkspace,
							tabs: [...terminalContext.panelWorkspace.tabs].reverse(),
						},
					},
				],
			}).success,
		).toBe(false);
		expect(WorkbenchSharedDocumentRevisionSchema.safeParse('01').success).toBe(false);
		expect(WorkbenchSharedDocumentRevisionSchema.safeParse('-1').success).toBe(false);
	});

	it('round-trips generated hierarchies and reconciles Client Views with a stable seed', () => {
		fc.assert(
			fc.property(
				fc.array(fc.integer({ max: 4, min: 1 }), { maxLength: 8, minLength: 1 }),
				(panelCounts) => {
					const document = createWorkbenchSharedDocument(inputWithTabs(panelCounts));
					const view = createDefaultWorkbenchClientView(document);
					expect(parsedDocumentJson(document)).toEqual(document);
					expect(parsedViewJson(view)).toEqual(view);
					expect(reconcileWorkbenchClientView(document, view)).toEqual(view);
				},
			),
			{ numRuns: 200, seed: 0x0b_ee_01 },
		);
	});
});
