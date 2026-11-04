import type { TabModelSnapshot } from '@orchbee/tab-strip/model';
import { z } from 'zod';
import { TerminalPresentationIdSchema } from '@orchbee/workbench-client-protocol';

import type { ProjectWorkspaceState, WorkbenchWorkspace } from '../../project-workspace';
import {
	collectPanelLeaves,
	type PanelWorkspaceState,
} from '../components/workbench-panel-workspace/model/panel-workspace-state';
import type {
	WorkbenchPublication,
	WorkbenchPublishedTab,
	WorkbenchPublishedTerminalContext,
} from '../model/workbench-publication';

const identifierSchema = z.string().min(1);
const nullableIdentifierSchema = identifierSchema.nullable();

const activeLocationSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('no-workspace') }),
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

const tabPanelViewSchema = z.strictObject({
	focusedPanelId: identifierSchema,
	maximizedPanelId: nullableIdentifierSchema,
	panels: z.array(panelViewSchema),
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
type TerminalContextView = WorkbenchClientView['terminalContexts'][number];
/** This Client's view of one Tab's Panels: which one has focus or fills the Tab, and what each shows. */
export type WorkbenchClientTabPanelView = TerminalContextView['panelWorkspace']['tabs'][number];
type TabPanelView = WorkbenchClientTabPanelView;
/** Views of Tabs that return to a publication, by terminal context and then by Tab. */
type ReturningTabPanelViews = ReadonlyMap<string, ReadonlyMap<string, TabPanelView>>;
const NO_RETURNING_TAB_PANEL_VIEWS: ReturningTabPanelViews = new Map();

/** What this Client's stores hold, from which its view is read. */
interface WorkbenchClientViewSource {
	readonly projectWorkspace: ProjectWorkspaceState;
	readonly terminalContexts: readonly {
		readonly panelWorkspace: PanelWorkspaceState;
		readonly tabCollection: TabModelSnapshot;
		readonly terminalContextId: string;
	}[];
}

export function createWorkbenchClientView(input: WorkbenchClientViewInput): WorkbenchClientView {
	return WorkbenchClientViewSchema.parse({ ...input, version: 1 });
}

export function createDefaultWorkbenchClientView(
	publication: WorkbenchPublication,
): WorkbenchClientView {
	return reconcileWorkbenchClientView(publication, undefined);
}

/** Reads what this Client chose out of its stores, which a publication then reconciles. */
export function projectWorkbenchClientView(source: WorkbenchClientViewSource): WorkbenchClientView {
	return createWorkbenchClientView({
		activeLocation: { ...source.projectWorkspace.activeLocation },
		expandedProjectIds: [...source.projectWorkspace.expandedProjectIds],
		recentsExpanded: source.projectWorkspace.recentsExpanded,
		terminalContexts: source.terminalContexts
			.map(({ panelWorkspace, tabCollection, terminalContextId }) => ({
				panelWorkspace: {
					tabs: panelWorkspace.tabs.map((tab) => ({
						focusedPanelId: tab.focusedPanelId,
						maximizedPanelId: tab.maximizedPanelId,
						panels: collectPanelLeaves(tab.root).map((panel) => ({
							activeChildPresentationId: panel.activeChildPresentationId,
							panelId: panel.id,
						})),
						tabId: tab.tabId,
					})),
				},
				tabCollection: {
					activeTabId: tabCollection.activeTabId,
					collapsedGroupIds: tabCollection.groups
						.filter((group) => group.collapsed)
						.map((group) => group.id),
					mostRecentGroupId: tabCollection.mostRecentGroupId,
					selectedTabIds: tabCollection.tabs
						.filter((tab) => tabCollection.selectedTabIds.has(tab.id))
						.map((tab) => tab.id),
					selectionAnchorId: tabCollection.selectionAnchorId,
				},
				terminalContextId,
			}))
			.sort((left, right) => compareIdentifiers(left.terminalContextId, right.terminalContextId)),
	});
}

/**
 * Keeps every choice of `candidate` that still names something `publication` holds. A Tab that
 * `candidate` lacks takes its view from `returning`, where this Client kept it while an earlier
 * publication withheld the Tab.
 */
export function reconcileWorkbenchClientView(
	publication: WorkbenchPublication,
	candidate: WorkbenchClientView | undefined,
	returning: ReturningTabPanelViews = NO_RETURNING_TAB_PANEL_VIEWS,
): WorkbenchClientView {
	const expandedCandidates = new Set(candidate?.expandedProjectIds ?? []);
	const candidateContexts = new Map(
		(candidate?.terminalContexts ?? []).map((context) => [context.terminalContextId, context]),
	);
	return createWorkbenchClientView({
		activeLocation: reconcileActiveLocation(publication, candidate?.activeLocation),
		expandedProjectIds: publication.projects
			.filter((project) => expandedCandidates.has(project.id))
			.map((project) => project.id),
		recentsExpanded: candidate?.recentsExpanded ?? true,
		terminalContexts: publication.terminalContexts.map((terminalContext) =>
			reconcileTerminalContextView(
				terminalContext,
				candidateContexts.get(terminalContext.terminalContextId),
				returning.get(terminalContext.terminalContextId),
			),
		),
	});
}

/** A Workspace that is gone gives way to the first one the sidebar lists, and the last one to none. */
function reconcileActiveLocation(
	publication: WorkbenchPublication,
	candidate: WorkbenchClientView['activeLocation'] | undefined,
): WorkbenchClientView['activeLocation'] {
	if (
		candidate?.kind === 'workspace' &&
		publication.workspaces.some((workspace) => workspace.id === candidate.workspaceId)
	) {
		return { ...candidate };
	}
	const workspace = firstWorkspaceInDisplayOrder(publication);
	return workspace === undefined
		? { kind: 'no-workspace' }
		: { kind: 'workspace', workspaceId: workspace.id };
}

/** The sidebar lists each Project's Workspaces in Project order, then Recents. */
function firstWorkspaceInDisplayOrder(
	publication: WorkbenchPublication,
): WorkbenchWorkspace | undefined {
	for (const project of publication.projects) {
		const workspace = publication.workspaces.find(
			(candidate) => candidate.projectId === project.id,
		);
		if (workspace !== undefined) return workspace;
	}
	return publication.workspaces.find((workspace) => workspace.projectId === null);
}

/**
 * A Tab that is gone gives way to the next one after it that remains, else the one before it, the
 * survivor its close itself selects, as in the TUI; without either, to the first Tab.
 */
function survivingActiveTabId(
	terminalContext: WorkbenchPublishedTerminalContext,
	candidate: TerminalContextView | undefined,
	tabIds: ReadonlySet<string>,
): string | null {
	const activeTabId = candidate?.tabCollection.activeTabId ?? null;
	if (activeTabId !== null && tabIds.has(activeTabId)) return activeTabId;
	const previousOrder = candidate?.panelWorkspace.tabs.map((tab) => tab.tabId) ?? [];
	const position = activeTabId === null ? -1 : previousOrder.indexOf(activeTabId);
	if (position >= 0) {
		const next = previousOrder.slice(position + 1).find((tabId) => tabIds.has(tabId));
		if (next !== undefined) return next;
		for (let index = position - 1; index >= 0; index -= 1) {
			const previous = previousOrder[index];
			if (previous !== undefined && tabIds.has(previous)) return previous;
		}
	}
	return terminalContext.tabs[0]?.id ?? null;
}

function reconcileTerminalContextView(
	terminalContext: WorkbenchPublishedTerminalContext,
	candidate: TerminalContextView | undefined,
	returning: ReadonlyMap<string, TabPanelView> | undefined,
): TerminalContextView {
	const tabIds = new Set(terminalContext.tabs.map((tab) => tab.id));
	const groupIds = new Set(terminalContext.groups.map((group) => group.id));
	const activeTabId = survivingActiveTabId(terminalContext, candidate, tabIds);
	const selectedCandidates = new Set(candidate?.tabCollection.selectedTabIds ?? []);
	const selectedTabIds = terminalContext.tabs
		.filter((tab) => selectedCandidates.has(tab.id))
		.map((tab) => tab.id);
	if (activeTabId !== null && !selectedTabIds.includes(activeTabId))
		selectedTabIds.push(activeTabId);
	const activeGroupId =
		activeTabId === null
			? null
			: (terminalContext.tabs.find((tab) => tab.id === activeTabId)?.group ?? null);
	const mostRecentGroupId =
		candidate?.tabCollection.mostRecentGroupId !== null &&
		candidate?.tabCollection.mostRecentGroupId !== undefined &&
		groupIds.has(candidate.tabCollection.mostRecentGroupId)
			? candidate.tabCollection.mostRecentGroupId
			: (activeGroupId ?? terminalContext.groups.at(-1)?.id ?? null);
	const selectionAnchorId =
		candidate?.tabCollection.selectionAnchorId !== null &&
		candidate?.tabCollection.selectionAnchorId !== undefined &&
		tabIds.has(candidate.tabCollection.selectionAnchorId)
			? candidate.tabCollection.selectionAnchorId
			: activeTabId;
	const collapsedCandidates = new Set(candidate?.tabCollection.collapsedGroupIds ?? []);
	const candidateTabs = new Map([
		...(returning ?? []),
		...(candidate?.panelWorkspace.tabs ?? []).map((tab): [string, TabPanelView] => [
			tab.tabId,
			tab,
		]),
	]);
	return {
		panelWorkspace: {
			tabs: terminalContext.tabs.map((tab) =>
				reconcileTabPanelView(tab, candidateTabs.get(tab.id)),
			),
		},
		tabCollection: {
			activeTabId,
			collapsedGroupIds: terminalContext.groups
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
	tab: WorkbenchPublishedTab,
	candidate: TabPanelView | undefined,
): TabPanelView {
	const panels = collectPanelLeaves(tab.root);
	const panelIds = new Set(panels.map((panel) => panel.id));
	const candidatePanels = new Map(candidate?.panels.map((panel) => [panel.panelId, panel]) ?? []);
	const firstPanelId = panels[0]?.id;
	if (firstPanelId === undefined) throw new Error(`Tab ${tab.id} has no Panel.`);
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
		tabId: tab.id,
	};
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
