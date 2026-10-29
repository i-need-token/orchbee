import {
	createDefaultWorkbenchClientView,
	createWorkbenchClientView,
	type WorkbenchClientView,
} from '@orchbee/workbench/document';
import { z } from 'zod';
import {
	TerminalPresentationIdSchema,
	type TerminalPresentationId,
	type WorkbenchSharedDocument,
	type WorkbenchSharedPanelNode,
} from '@orchbee/workbench-client-protocol';

const WORKBENCH_FOCUS_RECOVERY_STORAGE_KEY = 'orchbee:web:terminal-focus-recovery:v1';

const workbenchFocusRecoveryRecordSchema = z.strictObject({
	presentationId: TerminalPresentationIdSchema,
});

export type WorkbenchFocusRecoveryStorage = Pick<Storage, 'getItem' | 'removeItem' | 'setItem'>;

interface WorkbenchPresentationLocation {
	readonly activeChildPresentationId: TerminalPresentationId | null;
	readonly panelId: string;
	readonly tabId: string;
	readonly terminalContextId: string;
}

export function readWorkbenchFocusRecovery(
	storage: WorkbenchFocusRecoveryStorage,
): TerminalPresentationId | null {
	try {
		const value = storage.getItem(WORKBENCH_FOCUS_RECOVERY_STORAGE_KEY);
		if (value === null) return null;
		const result = workbenchFocusRecoveryRecordSchema.safeParse(JSON.parse(value));
		return result.success ? result.data.presentationId : null;
	} catch {
		return null;
	}
}

export function writeWorkbenchFocusRecovery(
	storage: WorkbenchFocusRecoveryStorage,
	presentationId: TerminalPresentationId | null,
): boolean {
	try {
		if (presentationId === null) {
			storage.removeItem(WORKBENCH_FOCUS_RECOVERY_STORAGE_KEY);
		} else {
			storage.setItem(WORKBENCH_FOCUS_RECOVERY_STORAGE_KEY, JSON.stringify({ presentationId }));
		}
		return true;
	} catch {
		return false;
	}
}

export function createFocusRecoveredWorkbenchClientView(
	document: WorkbenchSharedDocument,
	presentationId: TerminalPresentationId | null,
): WorkbenchClientView | undefined {
	if (presentationId === null) return undefined;
	const location = findPresentationLocation(document, presentationId);
	if (location === null) return undefined;
	const defaultView = createDefaultWorkbenchClientView(document);
	const workspace = document.projectWorkspace.workspaces.find(
		(candidate) => candidate.terminalContextId === location.terminalContextId,
	);
	return createWorkbenchClientView({
		activeLocation:
			workspace === undefined
				? { kind: 'standalone' }
				: { kind: 'workspace', workspaceId: workspace.id },
		expandedProjectIds:
			workspace?.projectId === null || workspace === undefined
				? defaultView.expandedProjectIds
				: [...new Set([...defaultView.expandedProjectIds, workspace.projectId])],
		terminalContexts: defaultView.terminalContexts.map((terminalContext) => {
			if (terminalContext.terminalContextId !== location.terminalContextId) {
				return terminalContext;
			}
			return {
				...terminalContext,
				tabCollection: {
					...terminalContext.tabCollection,
					activeTabId: location.tabId,
					selectedTabIds: [location.tabId],
					selectionAnchorId: location.tabId,
				},
				panelWorkspace: {
					tabs: terminalContext.panelWorkspace.tabs.map((tab) => {
						if (tab.tabId !== location.tabId) return tab;
						return {
							...tab,
							focusedPanelId: location.panelId,
							panels: tab.panels.map((panel) =>
								panel.panelId === location.panelId
									? {
											...panel,
											activeChildPresentationId: location.activeChildPresentationId,
										}
									: panel,
							),
						};
					}),
				},
			};
		}),
	});
}

export function resolveFocusedWorkbenchPresentationId(
	document: WorkbenchSharedDocument,
	view: WorkbenchClientView,
): TerminalPresentationId | null {
	const terminalContextId = resolveActiveTerminalContextId(document, view);
	if (terminalContextId === null) return null;
	const terminalContext = document.terminalContexts.find(
		(candidate) => candidate.terminalContextId === terminalContextId,
	);
	const terminalContextView = view.terminalContexts.find(
		(candidate) => candidate.terminalContextId === terminalContextId,
	);
	const activeTabId = terminalContextView?.tabCollection.activeTabId;
	if (
		terminalContext === undefined ||
		terminalContextView === undefined ||
		activeTabId === null ||
		activeTabId === undefined
	) {
		return null;
	}
	const tab = terminalContext.panelWorkspace.tabs.find(
		(candidate) => candidate.tabId === activeTabId,
	);
	const tabView = terminalContextView.panelWorkspace.tabs.find(
		(candidate) => candidate.tabId === activeTabId,
	);
	if (tab === undefined || tabView === undefined) return null;
	const panel = findPanel(tab.root, tabView.focusedPanelId);
	const panelView = tabView.panels.find(
		(candidate) => candidate.panelId === tabView.focusedPanelId,
	);
	if (panel === null || panelView === undefined) return null;
	const activeChildPresentationId = panelView.activeChildPresentationId;
	if (
		activeChildPresentationId !== null &&
		panel.childPresentations.some(
			(presentation) => presentation.presentationId === activeChildPresentationId,
		)
	) {
		return activeChildPresentationId;
	}
	return panel.primaryPresentation.presentationId;
}

function findPresentationLocation(
	document: WorkbenchSharedDocument,
	presentationId: TerminalPresentationId,
): WorkbenchPresentationLocation | null {
	for (const terminalContext of document.terminalContexts) {
		for (const tab of terminalContext.panelWorkspace.tabs) {
			const panelLocation = findPresentationInPanelTree(tab.root, presentationId);
			if (panelLocation === null) continue;
			return {
				...panelLocation,
				tabId: tab.tabId,
				terminalContextId: terminalContext.terminalContextId,
			};
		}
	}
	return null;
}

function findPresentationInPanelTree(
	node: WorkbenchSharedPanelNode,
	presentationId: TerminalPresentationId,
): Pick<WorkbenchPresentationLocation, 'activeChildPresentationId' | 'panelId'> | null {
	if (node.kind === 'panel') {
		if (node.primaryPresentation.presentationId === presentationId) {
			return { activeChildPresentationId: null, panelId: node.id };
		}
		return node.childPresentations.some(
			(presentation) => presentation.presentationId === presentationId,
		)
			? { activeChildPresentationId: presentationId, panelId: node.id }
			: null;
	}
	for (const child of node.children) {
		const location = findPresentationInPanelTree(child.node, presentationId);
		if (location !== null) return location;
	}
	return null;
}

function findPanel(
	node: WorkbenchSharedPanelNode,
	panelId: string,
): Extract<WorkbenchSharedPanelNode, { kind: 'panel' }> | null {
	if (node.kind === 'panel') return node.id === panelId ? node : null;
	for (const child of node.children) {
		const panel = findPanel(child.node, panelId);
		if (panel !== null) return panel;
	}
	return null;
}

function resolveActiveTerminalContextId(
	document: WorkbenchSharedDocument,
	view: WorkbenchClientView,
): string | null {
	const activeLocation = view.activeLocation;
	if (activeLocation.kind === 'standalone') {
		return document.projectWorkspace.standaloneTerminalContextId;
	}
	return (
		document.projectWorkspace.workspaces.find(
			(candidate) => candidate.id === activeLocation.workspaceId,
		)?.terminalContextId ?? null
	);
}
