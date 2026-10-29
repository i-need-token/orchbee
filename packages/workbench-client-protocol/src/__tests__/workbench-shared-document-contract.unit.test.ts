import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import {
	terminalPresentationId,
	terminalPresentationOperationId,
} from '../terminal-presentation-contract';
import {
	WorkbenchSharedDocumentRevisionSchema,
	WorkbenchSharedDocumentSchema,
	createWorkbenchSharedDocument,
	parseWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
	type WorkbenchSharedDocument,
	type WorkbenchSharedPanelNode,
} from '../workbench-shared-document-contract';

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

describe('Workbench shared document', () => {
	it('round-trips only shared product structure and terminal lifecycle intent', () => {
		const document = createWorkbenchSharedDocument(inputWithTabs([1, 3]));
		const serialized = JSON.stringify(document);
		const serializedWithoutCustomTitles = JSON.stringify(document, (key, value) =>
			key === 'customTitle' ? undefined : value,
		);

		expect(parsedDocumentJson(document)).toEqual(document);
		expect(() => parseWorkbenchSharedDocument(JSON.parse(serializedWithoutCustomTitles))).toThrow(
			/customTitle/u,
		);
		expect(serialized).not.toContain('activeTabId');
		expect(serialized).not.toContain('activeLocation');
		expect(serialized).not.toContain('focusedPanelId');
		expect(serialized).not.toContain('weight');
		expect(serialized).not.toContain('currentCwd');
		expect(serialized).not.toContain('terminalId');
		expect(document).toMatchObject({ revision: '0', version: 1 });
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

	it('accepts a same-axis binary split as meaningful close ancestry', () => {
		const input = inputWithTabs([3]);
		const terminalContext = input.terminalContexts[0];
		const workspaceTab = terminalContext?.panelWorkspace.tabs[0];
		const root = workspaceTab?.root;
		if (terminalContext === undefined || workspaceTab === undefined || root?.kind !== 'split') {
			throw new Error('Expected a three-Panel shared-document fixture.');
		}
		const nestedChild = root.children[1];
		if (nestedChild?.node.kind !== 'split') {
			throw new Error('Expected a nested split fixture.');
		}
		const sameAxisRoot: WorkbenchSharedPanelNode = {
			...root,
			children: [
				root.children[0] ?? nestedChild,
				{
					...nestedChild,
					node: { ...nestedChild.node, orientation: root.orientation },
				},
			],
		};

		expect(
			WorkbenchSharedDocumentSchema.safeParse({
				...input,
				version: 1,
				terminalContexts: [
					{
						...terminalContext,
						panelWorkspace: {
							...terminalContext.panelWorkspace,
							tabs: [{ ...workspaceTab, root: sameAxisRoot }],
						},
					},
				],
			}).success,
		).toBe(true);
	});

	it('rejects an n-ary Split from the binary shared topology', () => {
		const input = inputWithTabs([2]);
		const terminalContext = input.terminalContexts[0];
		const workspaceTab = terminalContext?.panelWorkspace.tabs[0];
		const root = workspaceTab?.root;
		if (terminalContext === undefined || workspaceTab === undefined || root?.kind !== 'split') {
			throw new Error('Expected a two-Panel shared-document fixture.');
		}

		expect(
			WorkbenchSharedDocumentSchema.safeParse({
				...input,
				version: 1,
				terminalContexts: [
					{
						...terminalContext,
						panelWorkspace: {
							...terminalContext.panelWorkspace,
							tabs: [
								{
									...workspaceTab,
									root: {
										...root,
										children: [...root.children, { id: 'panel-slot-extra', node: panel(999) }],
									},
								},
							],
						},
					},
				],
			}).success,
		).toBe(false);
	});

	it('round-trips generated hierarchies with a stable seed', () => {
		fc.assert(
			fc.property(
				fc.array(fc.integer({ max: 4, min: 1 }), { maxLength: 8, minLength: 1 }),
				(panelCounts) => {
					const document = createWorkbenchSharedDocument(inputWithTabs(panelCounts));
					expect(parsedDocumentJson(document)).toEqual(document);
				},
			),
			{ numRuns: 200, seed: 0x0b_ee_01 },
		);
	});
});
