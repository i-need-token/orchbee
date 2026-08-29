import type { WorkbenchSharedDocument, WorkbenchSharedPanelNode } from '../document';
import type {
	WorkbenchDirectoryListResult,
	WorkbenchLocationRef,
	WorkbenchProjectListResult,
	WorkbenchTerminalListItem,
	WorkbenchTerminalListResult,
	WorkbenchWorkspaceListResult,
} from './semantic-command-contracts';

export function projectWorkbenchProjectList(
	document: WorkbenchSharedDocument,
): WorkbenchProjectListResult {
	return {
		items: document.projectWorkspace.projects.map((project) => ({
			name: project.name,
			projectId: project.id,
		})),
		revision: document.revision,
	};
}

export function projectWorkbenchDirectoryList(
	document: WorkbenchSharedDocument,
	projectId: string,
): WorkbenchDirectoryListResult {
	const project = document.projectWorkspace.projects.find(
		(candidate) => candidate.id === projectId,
	);
	if (project === undefined) return { kind: 'rejected', reason: 'project-not-found' };
	return {
		items: project.directories.map((directory) => ({
			available: directory.available,
			directoryId: directory.id,
			kind: directory.kind,
			name: directory.name,
			path: directory.path,
			primary: directory.id === project.primaryDirectoryId,
			source: directory.source,
		})),
		revision: document.revision,
	};
}

export function projectWorkbenchWorkspaceList(
	document: WorkbenchSharedDocument,
	projectId: string,
): WorkbenchWorkspaceListResult {
	const project = document.projectWorkspace.projects.find(
		(candidate) => candidate.id === projectId,
	);
	if (project === undefined) return { kind: 'rejected', reason: 'project-not-found' };
	return {
		items: project.workspaces.map((workspace) => ({
			name: workspace.name,
			primaryDirectoryId: workspace.primaryDirectoryId,
			workspaceId: workspace.id,
		})),
		revision: document.revision,
	};
}

export function projectWorkbenchTerminalList(
	document: WorkbenchSharedDocument,
	location: WorkbenchLocationRef,
): WorkbenchTerminalListResult {
	const resolved = resolveLocation(document, location);
	if (resolved.kind === 'rejected') return resolved;
	const matches = document.terminalContexts.filter(
		(context) => context.terminalContextId === resolved.terminalContextId,
	);
	if (matches.length !== 1) {
		throw new Error('A validated Workbench location must resolve to one terminal context.');
	}
	const context = matches[0];
	if (context === undefined) {
		throw new Error('A validated Workbench location lost its terminal context.');
	}
	const tabs = new Map(context.tabCollection.tabs.map((tab) => [tab.id, tab]));
	const items: WorkbenchTerminalListItem[] = [];
	for (const panelTab of context.panelWorkspace.tabs) {
		const tab = tabs.get(panelTab.tabId);
		if (tab === undefined) {
			throw new Error('A validated Workbench Panel workspace must resolve its Tab.');
		}
		collectTerminalItems(
			panelTab.root,
			{
				groupId: tab.group,
				pinned: tab.pinned,
				tabCustomTitle: tab.customTitle,
				tabFallbackTitle: tab.title,
				tabId: tab.id,
				terminalContextId: context.terminalContextId,
			},
			resolved.directoryPaths,
			items,
		);
	}
	return { items, revision: document.revision };
}

function resolveLocation(
	document: WorkbenchSharedDocument,
	location: WorkbenchLocationRef,
):
	| {
			readonly directoryPaths: ReadonlyMap<string, string>;
			readonly kind: 'resolved';
			readonly terminalContextId: string;
	  }
	| { readonly kind: 'rejected'; readonly reason: 'project-not-found' | 'workspace-not-found' } {
	if (location.kind === 'standalone') {
		return {
			directoryPaths: new Map(),
			kind: 'resolved',
			terminalContextId: document.projectWorkspace.standaloneTerminalContextId,
		};
	}
	const project = document.projectWorkspace.projects.find(
		(candidate) => candidate.id === location.projectId,
	);
	if (project === undefined) return { kind: 'rejected', reason: 'project-not-found' };
	const workspace = project.workspaces.find((candidate) => candidate.id === location.workspaceId);
	if (workspace === undefined) return { kind: 'rejected', reason: 'workspace-not-found' };
	return {
		directoryPaths: new Map(project.directories.map((directory) => [directory.path, directory.id])),
		kind: 'resolved',
		terminalContextId: workspace.terminalContextId,
	};
}

function collectTerminalItems(
	node: WorkbenchSharedPanelNode,
	tab: Pick<
		WorkbenchTerminalListItem,
		'groupId' | 'pinned' | 'tabCustomTitle' | 'tabFallbackTitle' | 'tabId' | 'terminalContextId'
	>,
	directoryPaths: ReadonlyMap<string, string>,
	items: WorkbenchTerminalListItem[],
): void {
	if (node.kind === 'split') {
		for (const child of node.children) {
			collectTerminalItems(child.node, tab, directoryPaths, items);
		}
		return;
	}
	for (const [index, presentation] of [
		node.primaryPresentation,
		...node.childPresentations,
	].entries()) {
		items.push({
			...tab,
			intent: { ...presentation.intent },
			launchCwd: presentation.launchCwd,
			launchDirectoryId:
				presentation.launchCwd === null
					? null
					: (directoryPaths.get(presentation.launchCwd) ?? null),
			panelId: node.id,
			presentationId: presentation.presentationId,
			role: index === 0 ? 'primary' : 'child',
			terminalTitle: presentation.title,
		});
	}
}
