import {
	terminalPresentationId,
	terminalPresentationOperationId,
} from '../../../terminal-presentation';
import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import {
	WorkbenchTopologyDocumentSchema,
	WorkbenchTopologyRevisionSchema,
	createWorkbenchTopologyDocument,
	parseWorkbenchTopologyDocument,
	workbenchTopologyRevision,
	type WorkbenchTopologyDocument,
	type WorkbenchTopologyDocumentInput,
	type WorkbenchTopologyPanelNode,
} from '../workbench-topology';

function inputWithTabs(panelCounts: readonly number[] = [1]): WorkbenchTopologyDocumentInput {
	const terminalContextId = 'terminal-context-1';
	const tabs = panelCounts.map((_, index) => ({
		group: null,
		id: `${terminalContextId}-tab-${index + 1}`,
		pinned: false,
		title: `Terminal ${index + 1}`,
	}));
	return {
		projectWorkspace: {
			activeLocation: { kind: 'standalone' },
			expandedProjectIds: [],
			nextProjectSequence: 1,
			nextTerminalContextSequence: 2,
			nextWorkspaceSequence: 1,
			projects: [],
			standaloneTerminalContextId: terminalContextId,
		},
		revision: workbenchTopologyRevision('0'),
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
						focusedPanelId: `panel-${panelOffset(panelCounts, index)}`,
						maximizedPanelId: null,
						root: panelRoot(panelCounts, index, count),
						tabId: tabs[index]?.id ?? 'missing-tab',
					})),
				},
				tabCollection: {
					activeTabId: tabs[0]?.id ?? null,
					groups: [],
					mostRecentGroupId: null,
					tabs,
				},
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
): WorkbenchTopologyPanelNode {
	const firstPanel = panelOffset(panelCounts, tabIndex);
	return nestedPanelRoot(firstPanel, panelCount, tabIndex, 0);
}

function nestedPanelRoot(
	firstPanel: number,
	panelCount: number,
	tabIndex: number,
	depth: number,
): WorkbenchTopologyPanelNode {
	if (panelCount === 1) return panel(firstPanel);
	return {
		children: [
			{
				id: `panel-slot-${tabIndex + 1}-${depth + 1}-1`,
				node: panel(firstPanel),
				weight: 50,
			},
			{
				id: `panel-slot-${tabIndex + 1}-${depth + 1}-2`,
				node: nestedPanelRoot(firstPanel + 1, panelCount - 1, tabIndex, depth + 1),
				weight: 50,
			},
		],
		id: `split-${tabIndex + 1}-${depth + 1}`,
		kind: 'split',
		orientation: (tabIndex + depth) % 2 === 0 ? 'horizontal' : 'vertical',
	};
}

function panel(sequence: number): WorkbenchTopologyPanelNode {
	return {
		activeChildPresentationId: null,
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

function parsedJson(document: WorkbenchTopologyDocument): WorkbenchTopologyDocument {
	return parseWorkbenchTopologyDocument(JSON.parse(JSON.stringify(document)));
}

describe('workbench topology', () => {
	it('creates a canonical v1 document and round-trips only durable product topology', () => {
		const document = createWorkbenchTopologyDocument(inputWithTabs([1, 3]));

		expect(parsedJson(document)).toEqual(document);
		expect(JSON.stringify(document)).not.toContain('currentCwd');
		expect(JSON.stringify(document)).not.toContain('terminalId');
		expect(document).toMatchObject({ revision: '0', version: 1 });
	});

	it('rejects topology with missing contexts or unexpected runtime authority', () => {
		const document = createWorkbenchTopologyDocument(inputWithTabs());

		expect(
			WorkbenchTopologyDocumentSchema.safeParse({ ...document, terminalContexts: [] }).success,
		).toBe(false);
		expect(
			WorkbenchTopologyDocumentSchema.safeParse({ ...document, terminalId: 'host-terminal' })
				.success,
		).toBe(false);
	});

	it('rejects invalid Tab and Panel relationships without repairing them', () => {
		const document = createWorkbenchTopologyDocument(inputWithTabs([1, 2]));
		const [terminalContext] = document.terminalContexts;
		if (terminalContext === undefined) throw new Error('Expected one terminal context.');

		expect(
			WorkbenchTopologyDocumentSchema.safeParse({
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
	});

	it('rejects noncanonical revisions', () => {
		expect(WorkbenchTopologyRevisionSchema.safeParse('01').success).toBe(false);
		expect(WorkbenchTopologyRevisionSchema.safeParse('-1').success).toBe(false);
	});

	it('round-trips generated ordered Tab and Panel hierarchies with a stable seed', () => {
		fc.assert(
			fc.property(
				fc.array(fc.integer({ max: 4, min: 1 }), { maxLength: 8, minLength: 1 }),
				(panelCounts) => {
					const document = createWorkbenchTopologyDocument(inputWithTabs(panelCounts));
					expect(parsedJson(document)).toEqual(document);
				},
			),
			{ numRuns: 200, seed: 0x0b_ee_01 },
		);
	});
});
