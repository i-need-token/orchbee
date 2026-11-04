import { z } from 'zod';
import {
	TerminalPresentationIdSchema,
	type WorkbenchSharedDocument,
	type WorkbenchSharedPanelNode,
} from '@orchbee/workbench-client-protocol';

const identifierSchema = z.string().min(1);
const nullableIdentifierSchema = identifierSchema.nullable();
const weightSchema = z.number().finite().nonnegative();

const activeLocationSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('standalone') }),
	z.strictObject({
		kind: z.literal('workspace'),
		workspaceId: identifierSchema,
	}),
]);

const tabCollectionViewSchema = z.strictObject({
	activeTabId: nullableIdentifierSchema,
	collapsedGroupIds: z.array(identifierSchema),
	mostRecentGroupId: nullableIdentifierSchema,
	selectedTabIds: z.array(identifierSchema),
	selectionAnchorId: nullableIdentifierSchema,
});

const panelViewSchema = z.strictObject({
	activeChildPresentationId: TerminalPresentationIdSchema.nullable(),
	panelId: identifierSchema,
});

const splitWeightSchema = z.strictObject({
	slotId: identifierSchema,
	weight: weightSchema,
});

const splitViewSchema = z.strictObject({
	splitId: identifierSchema,
	weights: z.array(splitWeightSchema),
});

const tabPanelViewSchema = z.strictObject({
	focusedPanelId: identifierSchema,
	maximizedPanelId: nullableIdentifierSchema,
	panels: z.array(panelViewSchema),
	splits: z.array(splitViewSchema),
	tabId: identifierSchema,
});

const panelWorkspaceViewSchema = z.strictObject({
	tabs: z.array(tabPanelViewSchema),
});

const terminalContextViewSchema = z.strictObject({
	panelWorkspace: panelWorkspaceViewSchema,
	tabCollection: tabCollectionViewSchema,
	terminalContextId: identifierSchema,
});

const workbenchClientViewBaseSchema = z.strictObject({
	activeLocation: activeLocationSchema,
	expandedProjectIds: z.array(identifierSchema),
	recentsExpanded: z.boolean(),
	terminalContexts: z.array(terminalContextViewSchema),
	version: z.literal(1),
});

const WorkbenchClientViewSchema = workbenchClientViewBaseSchema.superRefine((view, context) => {
	for (const issue of validateClientView(view)) {
		context.addIssue({ code: 'custom', message: issue });
	}
});

export type WorkbenchClientView = z.infer<typeof WorkbenchClientViewSchema>;
type WorkbenchClientViewInput = Omit<WorkbenchClientView, 'version'>;

export function createWorkbenchClientView(input: WorkbenchClientViewInput): WorkbenchClientView {
	return WorkbenchClientViewSchema.parse({ ...input, version: 1 });
}

export function parseWorkbenchClientView(value: unknown): WorkbenchClientView {
	return WorkbenchClientViewSchema.parse(value);
}

export function createDefaultWorkbenchClientView(
	document: WorkbenchSharedDocument,
): WorkbenchClientView {
	return reconcileWorkbenchClientView(document, undefined);
}

export function reconcileWorkbenchClientView(
	document: WorkbenchSharedDocument,
	candidate: WorkbenchClientView | undefined,
): WorkbenchClientView {
	const activeLocation = reconcileActiveLocation(
		document.projectWorkspace.workspaces,
		candidate?.activeLocation,
	);
	const expandedCandidates = new Set(candidate?.expandedProjectIds ?? []);
	const candidateContexts = new Map(
		(candidate?.terminalContexts ?? []).map((context) => [context.terminalContextId, context]),
	);
	return createWorkbenchClientView({
		activeLocation,
		expandedProjectIds: document.projectWorkspace.projects
			.filter((project) => expandedCandidates.has(project.id))
			.map((project) => project.id),
		recentsExpanded: candidate?.recentsExpanded ?? true,
		terminalContexts: document.terminalContexts.map((terminalContext) =>
			reconcileTerminalContextView(
				terminalContext,
				candidateContexts.get(terminalContext.terminalContextId),
			),
		),
	});
}

function reconcileActiveLocation(
	workspaces: WorkbenchSharedDocument['projectWorkspace']['workspaces'],
	candidate: WorkbenchClientView['activeLocation'] | undefined,
): WorkbenchClientView['activeLocation'] {
	return candidate?.kind === 'workspace' &&
		workspaces.some((workspace) => workspace.id === candidate.workspaceId)
		? { ...candidate }
		: { kind: 'standalone' };
}

function reconcileTerminalContextView(
	terminalContext: WorkbenchSharedDocument['terminalContexts'][number],
	candidate: WorkbenchClientView['terminalContexts'][number] | undefined,
): WorkbenchClientView['terminalContexts'][number] {
	const tabIds = new Set(terminalContext.tabCollection.tabs.map((tab) => tab.id));
	const groupIds = new Set(terminalContext.tabCollection.groups.map((group) => group.id));
	const activeTabId =
		candidate?.tabCollection.activeTabId !== null &&
		candidate?.tabCollection.activeTabId !== undefined &&
		tabIds.has(candidate.tabCollection.activeTabId)
			? candidate.tabCollection.activeTabId
			: (terminalContext.tabCollection.tabs[0]?.id ?? null);
	const selectedCandidates = new Set(candidate?.tabCollection.selectedTabIds ?? []);
	const selectedTabIds = terminalContext.tabCollection.tabs
		.filter((tab) => selectedCandidates.has(tab.id))
		.map((tab) => tab.id);
	if (activeTabId !== null && !selectedTabIds.includes(activeTabId))
		selectedTabIds.push(activeTabId);
	const activeGroupId =
		activeTabId === null
			? null
			: (terminalContext.tabCollection.tabs.find((tab) => tab.id === activeTabId)?.group ?? null);
	const mostRecentGroupId =
		candidate?.tabCollection.mostRecentGroupId !== null &&
		candidate?.tabCollection.mostRecentGroupId !== undefined &&
		groupIds.has(candidate.tabCollection.mostRecentGroupId)
			? candidate.tabCollection.mostRecentGroupId
			: (activeGroupId ?? terminalContext.tabCollection.groups.at(-1)?.id ?? null);
	const selectionAnchorId =
		candidate?.tabCollection.selectionAnchorId !== null &&
		candidate?.tabCollection.selectionAnchorId !== undefined &&
		tabIds.has(candidate.tabCollection.selectionAnchorId)
			? candidate.tabCollection.selectionAnchorId
			: activeTabId;
	const collapsedCandidates = new Set(candidate?.tabCollection.collapsedGroupIds ?? []);
	const candidateTabs = new Map(
		(candidate?.panelWorkspace.tabs ?? []).map((tab) => [tab.tabId, tab]),
	);
	return {
		panelWorkspace: {
			tabs: terminalContext.panelWorkspace.tabs.map((tab) =>
				reconcileTabPanelView(tab, candidateTabs.get(tab.tabId)),
			),
		},
		tabCollection: {
			activeTabId,
			collapsedGroupIds: terminalContext.tabCollection.groups
				.filter((group) => collapsedCandidates.has(group.id))
				.map((group) => group.id),
			mostRecentGroupId,
			selectedTabIds,
			selectionAnchorId,
		},
		terminalContextId: terminalContext.terminalContextId,
	};
}

function reconcileTabPanelView(
	tab: WorkbenchSharedDocument['terminalContexts'][number]['panelWorkspace']['tabs'][number],
	candidate:
		| WorkbenchClientView['terminalContexts'][number]['panelWorkspace']['tabs'][number]
		| undefined,
): WorkbenchClientView['terminalContexts'][number]['panelWorkspace']['tabs'][number] {
	const panels: Extract<WorkbenchSharedPanelNode, { kind: 'panel' }>[] = [];
	const splits: Extract<WorkbenchSharedPanelNode, { kind: 'split' }>[] = [];
	visitPanelTree(tab.root, panels, splits);
	const panelIds = new Set(panels.map((panel) => panel.id));
	const candidatePanels = new Map(candidate?.panels.map((panel) => [panel.panelId, panel]) ?? []);
	const candidateSplits = new Map(candidate?.splits.map((split) => [split.splitId, split]) ?? []);
	const firstPanelId = panels[0]?.id;
	if (firstPanelId === undefined) throw new Error(`Tab ${tab.tabId} has no Panel.`);
	return {
		focusedPanelId:
			candidate !== undefined && panelIds.has(candidate.focusedPanelId)
				? candidate.focusedPanelId
				: firstPanelId,
		maximizedPanelId:
			candidate?.maximizedPanelId !== null &&
			candidate?.maximizedPanelId !== undefined &&
			panelIds.has(candidate.maximizedPanelId)
				? candidate.maximizedPanelId
				: null,
		panels: panels.map((node) => {
			const candidatePanel = candidatePanels.get(node.id);
			const childIds = new Set(
				node.childPresentations.map((presentation) => presentation.presentationId),
			);
			return {
				activeChildPresentationId:
					candidatePanel?.activeChildPresentationId !== null &&
					candidatePanel?.activeChildPresentationId !== undefined &&
					childIds.has(candidatePanel.activeChildPresentationId)
						? candidatePanel.activeChildPresentationId
						: null,
				panelId: node.id,
			};
		}),
		splits: splits.map((split) => ({
			splitId: split.id,
			weights: reconcileSplitWeights(split, candidateSplits.get(split.id)),
		})),
		tabId: tab.tabId,
	};
}

function visitPanelTree(
	node: WorkbenchSharedPanelNode,
	panels: Extract<WorkbenchSharedPanelNode, { kind: 'panel' }>[],
	splits: Extract<WorkbenchSharedPanelNode, { kind: 'split' }>[],
): void {
	if (node.kind === 'panel') {
		panels.push(node);
		return;
	}
	splits.push(node);
	for (const child of node.children) visitPanelTree(child.node, panels, splits);
}

function reconcileSplitWeights(
	split: Extract<WorkbenchSharedPanelNode, { kind: 'split' }>,
	candidate:
		| WorkbenchClientView['terminalContexts'][number]['panelWorkspace']['tabs'][number]['splits'][number]
		| undefined,
): WorkbenchClientView['terminalContexts'][number]['panelWorkspace']['tabs'][number]['splits'][number]['weights'] {
	const candidateWeights = new Map(
		candidate?.weights.map((weight) => [weight.slotId, weight.weight]) ?? [],
	);
	if (
		candidateWeights.size === split.children.length &&
		split.children.every((child) => candidateWeights.has(child.id))
	) {
		return split.children.map((child) => ({
			slotId: child.id,
			weight: requireWeight(candidateWeights.get(child.id)),
		}));
	}
	const weight = 100 / split.children.length;
	return split.children.map((child) => ({ slotId: child.id, weight }));
}

function requireWeight(weight: number | undefined): number {
	if (weight === undefined) throw new Error('Missing reconciled split weight.');
	return weight;
}

function validateClientView(
	view: Pick<
		z.infer<typeof workbenchClientViewBaseSchema>,
		'expandedProjectIds' | 'terminalContexts'
	>,
): readonly string[] {
	const issues: string[] = [];
	validateUnique(view.expandedProjectIds, 'expanded Project', issues);
	let previousContextId: string | undefined;
	for (const terminalContext of view.terminalContexts) {
		if (
			previousContextId !== undefined &&
			compareIdentifiers(previousContextId, terminalContext.terminalContextId) >= 0
		) {
			issues.push('Client terminal contexts must use unique canonical identity order.');
		}
		previousContextId = terminalContext.terminalContextId;
		validateUnique(terminalContext.tabCollection.collapsedGroupIds, 'collapsed Tab group', issues);
		validateUnique(terminalContext.tabCollection.selectedTabIds, 'selected Tab', issues);
		if (
			terminalContext.tabCollection.activeTabId !== null &&
			!terminalContext.tabCollection.selectedTabIds.includes(
				terminalContext.tabCollection.activeTabId,
			)
		) {
			issues.push('The active client Tab must be selected.');
		}
		if (
			terminalContext.tabCollection.activeTabId === null &&
			terminalContext.tabCollection.selectedTabIds.length > 0
		) {
			issues.push('A client without an active Tab cannot retain selected Tabs.');
		}
		const tabIds = terminalContext.panelWorkspace.tabs.map((tab) => tab.tabId);
		validateUnique(tabIds, 'client Panel-workspace Tab', issues);
		for (const tab of terminalContext.panelWorkspace.tabs) {
			validateUnique(
				tab.panels.map((panel) => panel.panelId),
				'client Panel',
				issues,
			);
			validateUnique(
				tab.splits.map((split) => split.splitId),
				'client Split',
				issues,
			);
			for (const split of tab.splits) {
				validateUnique(
					split.weights.map((weight) => weight.slotId),
					'client Panel slot',
					issues,
				);
				const total = split.weights.reduce((sum, weight) => sum + weight.weight, 0);
				if (split.weights.length !== 2 || Math.abs(total - 100) > 0.000_001) {
					issues.push(`Client Split ${split.splitId} has invalid weights.`);
				}
			}
		}
	}
	return issues;
}

function validateUnique(values: readonly string[], description: string, issues: string[]): void {
	const seen = new Set<string>();
	for (const value of values) {
		if (seen.has(value)) issues.push(`Duplicate ${description}: ${value}.`);
		seen.add(value);
	}
}

function compareIdentifiers(left: string, right: string): number {
	if (left < right) return -1;
	return left === right ? 0 : 1;
}
