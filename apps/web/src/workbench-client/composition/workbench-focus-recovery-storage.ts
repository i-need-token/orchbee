import {
	createDefaultWorkbenchClientView,
	createWorkbenchClientView,
	type WorkbenchClientView,
	type WorkbenchPublication,
	type WorkbenchPublishedTab,
} from '../../workbench';
import { z } from 'zod';
import {
	TerminalPresentationIdSchema,
	type TerminalPresentationId,
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

/**
 * Opens the Workspace, Tab and Panel that show the terminal this window last focused, while the
 * publication still holds it.
 */
export function createFocusRecoveredWorkbenchClientView(
	publication: WorkbenchPublication,
	presentationId: TerminalPresentationId | null,
): WorkbenchClientView | undefined {
	if (presentationId === null) return undefined;
	const location = findPresentationLocation(publication, presentationId);
	if (location === null) return undefined;
	const workspace = publication.workspaces.find(
		(candidate) => candidate.terminalContextId === location.terminalContextId,
	);
	if (workspace === undefined) return undefined;
	const defaultView = createDefaultWorkbenchClientView(publication);
	return createWorkbenchClientView({
		activeLocation: { kind: 'workspace', workspaceId: workspace.id },
		expandedProjectIds:
			workspace.projectId === null
				? defaultView.expandedProjectIds
				: [...new Set([...defaultView.expandedProjectIds, workspace.projectId])],
		recentsExpanded: defaultView.recentsExpanded,
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

function findPresentationLocation(
	publication: WorkbenchPublication,
	presentationId: TerminalPresentationId,
): WorkbenchPresentationLocation | null {
	for (const terminalContext of publication.terminalContexts) {
		for (const tab of terminalContext.tabs) {
			const panelLocation = findPresentationInPanelTree(tab.root, presentationId);
			if (panelLocation === null) continue;
			return {
				...panelLocation,
				tabId: tab.id,
				terminalContextId: terminalContext.terminalContextId,
			};
		}
	}
	return null;
}

function findPresentationInPanelTree(
	node: WorkbenchPublishedTab['root'],
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
