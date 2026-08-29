import type {
	WorkbenchSharedDocument,
	WorkbenchSharedPanelNode,
	WorkbenchSharedTerminalContext,
} from '../document';
import type {
	WorkbenchDirectoryListResult,
	WorkbenchLocationRef,
	WorkbenchPanelListItem,
	WorkbenchPanelListResult,
	WorkbenchProjectListResult,
	WorkbenchTabListResult,
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
			placement: { ...directory.placement },
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

export function projectWorkbenchTabList(
	document: WorkbenchSharedDocument,
	location: WorkbenchLocationRef,
): WorkbenchTabListResult {
	const resolved = resolveLocation(document, location);
	if (resolved.kind === 'rejected') return resolved;
	const panelTabs = new Map(resolved.context.panelWorkspace.tabs.map((tab) => [tab.tabId, tab]));
	return {
		items: resolved.context.tabCollection.tabs.map((tab) => {
			const panelTab = panelTabs.get(tab.id);
			if (panelTab === undefined) {
				throw new Error('A validated Workbench Tab must resolve its Panel workspace.');
			}
			const summary = summarizePanelNode(panelTab.root);
			return {
				groupId: tab.group,
				panelCount: summary.panelCount,
				pinned: tab.pinned,
				tabCustomTitle: tab.customTitle,
				tabFallbackTitle: tab.title,
				tabId: tab.id,
				terminalContextId: resolved.context.terminalContextId,
				terminalCount: summary.terminalCount,
			};
		}),
		revision: document.revision,
	};
}

export function projectWorkbenchPanelList(
	document: WorkbenchSharedDocument,
	location: WorkbenchLocationRef,
	tabId: string,
): WorkbenchPanelListResult {
	const resolved = resolveLocation(document, location);
	if (resolved.kind === 'rejected') return resolved;
	const tab = resolved.context.panelWorkspace.tabs.find((candidate) => candidate.tabId === tabId);
	if (tab === undefined) return { kind: 'rejected', reason: 'tab-not-found' };
	const items: WorkbenchPanelListItem[] = [];
	collectPanelItems(tab.root, resolved.context.terminalContextId, tabId, items);
	return { items, revision: document.revision };
}

export function projectWorkbenchTerminalList(
	document: WorkbenchSharedDocument,
	location: WorkbenchLocationRef,
): WorkbenchTerminalListResult {
	const resolved = resolveLocation(document, location);
	if (resolved.kind === 'rejected') return resolved;
	const context = resolved.context;
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
			readonly context: WorkbenchSharedTerminalContext;
			readonly directoryPaths: ReadonlyMap<string, string>;
			readonly kind: 'resolved';
	  }
	| { readonly kind: 'rejected'; readonly reason: 'project-not-found' | 'workspace-not-found' } {
	if (location.kind === 'standalone') {
		return {
			context: findTerminalContext(document, document.projectWorkspace.standaloneTerminalContextId),
			directoryPaths: new Map(),
			kind: 'resolved',
		};
	}
	const project = document.projectWorkspace.projects.find(
		(candidate) => candidate.id === location.projectId,
	);
	if (project === undefined) return { kind: 'rejected', reason: 'project-not-found' };
	const workspace = project.workspaces.find((candidate) => candidate.id === location.workspaceId);
	if (workspace === undefined) return { kind: 'rejected', reason: 'workspace-not-found' };
	return {
		context: findTerminalContext(document, workspace.terminalContextId),
		directoryPaths: new Map(project.directories.map((directory) => [directory.path, directory.id])),
		kind: 'resolved',
	};
}

function findTerminalContext(
	document: WorkbenchSharedDocument,
	terminalContextId: string,
): WorkbenchSharedTerminalContext {
	const matches = document.terminalContexts.filter(
		(context) => context.terminalContextId === terminalContextId,
	);
	if (matches.length !== 1) {
		throw new Error('A validated Workbench location must resolve to one terminal context.');
	}
	const context = matches[0];
	if (context === undefined) {
		throw new Error('A validated Workbench location lost its terminal context.');
	}
	return context;
}

function summarizePanelNode(node: WorkbenchSharedPanelNode): {
	readonly panelCount: number;
	readonly terminalCount: number;
} {
	if (node.kind === 'panel') {
		return { panelCount: 1, terminalCount: 1 + node.childPresentations.length };
	}
	return node.children.reduce(
		(summary, child) => {
			const childSummary = summarizePanelNode(child.node);
			return {
				panelCount: summary.panelCount + childSummary.panelCount,
				terminalCount: summary.terminalCount + childSummary.terminalCount,
			};
		},
		{ panelCount: 0, terminalCount: 0 },
	);
}

function collectPanelItems(
	node: WorkbenchSharedPanelNode,
	terminalContextId: string,
	tabId: string,
	items: WorkbenchPanelListItem[],
): void {
	if (node.kind === 'split') {
		for (const child of node.children) {
			collectPanelItems(child.node, terminalContextId, tabId, items);
		}
		return;
	}
	items.push({
		panelId: node.id,
		position: items.length + 1,
		tabId,
		terminalContextId,
		terminalCount: 1 + node.childPresentations.length,
	});
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
