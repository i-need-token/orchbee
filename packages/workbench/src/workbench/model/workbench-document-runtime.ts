import type { TabModelInit, TabModelSnapshot } from '@orchbee/tab-strip';
import {
	createWorkbenchSharedDocument,
	parseWorkbenchSharedDocument,
	type TerminalPresentationLifecycle,
	type WorkbenchSharedDocument,
	type WorkbenchSharedDocumentRevision,
	type WorkbenchSharedPanelNode,
	type WorkbenchSharedPresentation,
	type WorkbenchSharedPresentationIntent,
} from '@orchbee/workbench-client-protocol';

import type { ProjectWorkspaceState, WorkbenchDirectory } from '../../project-workspace';
import type {
	PanelLayoutNode,
	PanelWorkspaceState,
	TerminalPresentation,
} from '../components/workbench-panel-workspace/model/panel-workspace-state';
import { validatePanelWorkspaceState } from '../components/workbench-panel-workspace/model/panel-workspace-state';
import {
	createWorkbenchClientView,
	parseWorkbenchClientView,
	reconcileWorkbenchClientView,
	type WorkbenchClientView,
} from '../document';
import type { WorkbenchTabTitlePolicy } from '../components/workbench-panel-workspace/model/PanelWorkspaceStore';

interface WorkbenchDocumentRuntimeContextSnapshot {
	readonly nextTabSequence: number;
	readonly panelWorkspace: PanelWorkspaceState;
	readonly tabCollection: TabModelSnapshot;
	readonly tabTitlePolicies: readonly WorkbenchTabTitlePolicy[];
	readonly terminalContextId: string;
}

interface WorkbenchDocumentRuntimeSnapshot {
	readonly projectWorkspace: ProjectWorkspaceState;
	readonly terminalContexts: readonly WorkbenchDocumentRuntimeContextSnapshot[];
}

interface HydratedWorkbenchDocumentTerminalContext {
	readonly nextTabSequence: number;
	readonly panelWorkspace: PanelWorkspaceState;
	readonly tabCollection: TabModelInit;
	readonly tabTitlePolicies: readonly WorkbenchTabTitlePolicy[];
	readonly terminalContextId: string;
}

interface HydratedWorkbenchDocuments {
	readonly clientView: WorkbenchClientView;
	readonly projectWorkspace: ProjectWorkspaceState;
	readonly revision: WorkbenchSharedDocumentRevision;
	readonly terminalContexts: readonly HydratedWorkbenchDocumentTerminalContext[];
}

export function projectWorkbenchSharedDocument(
	snapshot: WorkbenchDocumentRuntimeSnapshot,
	revision: WorkbenchSharedDocumentRevision,
): WorkbenchSharedDocument {
	return createWorkbenchSharedDocument({
		projectWorkspace: projectSharedProjectWorkspace(snapshot.projectWorkspace),
		revision,
		terminalContexts: snapshot.terminalContexts
			.map((terminalContext) => ({
				nextTabSequence: terminalContext.nextTabSequence,
				panelWorkspace: projectSharedPanelWorkspace(terminalContext.panelWorkspace),
				tabCollection: projectSharedTabCollection(
					terminalContext.tabCollection,
					terminalContext.tabTitlePolicies,
				),
				terminalContextId: terminalContext.terminalContextId,
			}))
			.sort((left, right) => compareIdentifiers(left.terminalContextId, right.terminalContextId)),
	});
}

export function projectWorkbenchClientView(
	snapshot: WorkbenchDocumentRuntimeSnapshot,
): WorkbenchClientView {
	return createWorkbenchClientView({
		activeLocation: { ...snapshot.projectWorkspace.activeLocation },
		expandedProjectIds: [...snapshot.projectWorkspace.expandedProjectIds],
		terminalContexts: snapshot.terminalContexts
			.map((terminalContext) => ({
				panelWorkspace: projectPanelWorkspaceView(terminalContext.panelWorkspace),
				tabCollection: projectTabCollectionView(terminalContext.tabCollection),
				terminalContextId: terminalContext.terminalContextId,
			}))
			.sort((left, right) => compareIdentifiers(left.terminalContextId, right.terminalContextId)),
	});
}

export function hydrateWorkbenchDocuments(
	sharedValue: unknown,
	clientViewValue?: unknown,
): HydratedWorkbenchDocuments {
	const document = parseWorkbenchSharedDocument(sharedValue);
	const candidate = parseOptionalClientView(clientViewValue);
	const clientView = reconcileWorkbenchClientView(document, candidate);
	const contextViews = new Map(
		clientView.terminalContexts.map((context) => [context.terminalContextId, context]),
	);
	return {
		clientView,
		projectWorkspace: {
			...document.projectWorkspace,
			activeLocation: { ...clientView.activeLocation },
			expandedProjectIds: [...clientView.expandedProjectIds],
		},
		revision: document.revision,
		terminalContexts: document.terminalContexts.map((terminalContext) => {
			const contextView = requireDefined(contextViews.get(terminalContext.terminalContextId));
			const panelWorkspace = hydratePanelWorkspace(
				terminalContext.panelWorkspace,
				contextView.panelWorkspace,
			);
			const invariantErrors = validatePanelWorkspaceState(panelWorkspace);
			if (invariantErrors.length > 0) {
				throw new Error(`Hydrated Panel workspace is invalid: ${invariantErrors.join('; ')}`);
			}
			const collapsedGroupIds = new Set(contextView.tabCollection.collapsedGroupIds);
			return {
				nextTabSequence: terminalContext.nextTabSequence,
				panelWorkspace,
				tabCollection: {
					activeTabId: contextView.tabCollection.activeTabId,
					groups: terminalContext.tabCollection.groups.map((group) => ({
						...group,
						collapsed: collapsedGroupIds.has(group.id),
					})),
					mostRecentGroupId: contextView.tabCollection.mostRecentGroupId,
					selectedTabIds: contextView.tabCollection.selectedTabIds,
					selectionAnchorId: contextView.tabCollection.selectionAnchorId,
					tabs: terminalContext.tabCollection.tabs.map((tab) => ({
						...tab,
						title: tab.customTitle ?? tab.title,
					})),
				},
				tabTitlePolicies: terminalContext.tabCollection.tabs.map((tab) => ({
					customTitle: tab.customTitle,
					fallbackTitle: tab.title,
					tabId: tab.id,
				})),
				terminalContextId: terminalContext.terminalContextId,
			};
		}),
	};
}

function parseOptionalClientView(value: unknown): WorkbenchClientView | undefined {
	return value === undefined ? undefined : parseWorkbenchClientView(value);
}

function projectSharedTabCollection(
	snapshot: TabModelSnapshot,
	tabTitlePolicies: readonly WorkbenchTabTitlePolicy[],
) {
	const policies = new Map(tabTitlePolicies.map((policy) => [policy.tabId, policy]));
	return {
		groups: snapshot.groups.map((group) => ({
			color: group.color,
			id: group.id,
			title: group.title,
		})),
		tabs: snapshot.tabs.map((tab) => ({
			customTitle: requireDefined(policies.get(tab.id)).customTitle,
			group: tab.group,
			id: tab.id,
			pinned: tab.pinned,
			title: requireDefined(policies.get(tab.id)).fallbackTitle,
		})),
	};
}

function projectTabCollectionView(snapshot: TabModelSnapshot) {
	return {
		activeTabId: snapshot.activeTabId,
		collapsedGroupIds: snapshot.groups.filter((group) => group.collapsed).map((group) => group.id),
		mostRecentGroupId: snapshot.mostRecentGroupId,
		selectedTabIds: snapshot.tabs
			.filter((tab) => snapshot.selectedTabIds.has(tab.id))
			.map((tab) => tab.id),
		selectionAnchorId: snapshot.selectionAnchorId,
	};
}

function projectSharedProjectWorkspace(state: ProjectWorkspaceState) {
	return {
		nextProjectSequence: state.nextProjectSequence,
		nextResourceScopeSequence: state.nextResourceScopeSequence,
		nextTerminalContextSequence: state.nextTerminalContextSequence,
		nextWorkspaceSequence: state.nextWorkspaceSequence,
		projects: state.projects.map((project) => ({ ...project })),
		workspaces: state.workspaces.map((workspace) => ({ ...workspace })),
		resourceScopes: state.resourceScopes.map((project) => ({
			directories: project.directories.map(projectDirectory),
			id: project.id,
			launchConfigurations: project.launchConfigurations.map((configuration) => ({
				...configuration,
			})),
			name: project.name,
			primaryDirectoryId: project.primaryDirectoryId,
		})),
		standaloneTerminalContextId: state.standaloneTerminalContextId,
	};
}

function projectDirectory(directory: WorkbenchDirectory) {
	if (directory.kind === 'worktree') {
		return { ...directory, git: { ...directory.git } };
	}
	return {
		...directory,
		git: directory.git === null ? null : { ...directory.git },
	};
}

function projectSharedPanelWorkspace(state: PanelWorkspaceState) {
	return {
		nextPanelSequence: state.nextPanelSequence,
		nextPanelSlotSequence: state.nextPanelSlotSequence,
		nextSplitSequence: state.nextSplitSequence,
		nextTerminalSequence: state.nextTerminalSequence,
		pendingRemovals: state.pendingRemovals.map((removal) => ({
			...removal,
			presentationIds: [...removal.presentationIds],
		})),
		tabs: state.tabs.map((tab) => ({
			root: projectSharedPanelNode(tab.root),
			tabId: tab.tabId,
		})),
	};
}

function projectPanelWorkspaceView(state: PanelWorkspaceState) {
	return {
		tabs: state.tabs.map((tab) => {
			const panels: Array<{
				activeChildPresentationId: TerminalPresentation['presentationId'] | null;
				panelId: string;
			}> = [];
			const splits: Array<{
				splitId: string;
				weights: Array<{ slotId: string; weight: number }>;
			}> = [];
			projectPanelNodeView(tab.root, panels, splits);
			return {
				focusedPanelId: tab.focusedPanelId,
				maximizedPanelId: tab.maximizedPanelId,
				panels,
				splits,
				tabId: tab.tabId,
			};
		}),
	};
}

function projectSharedPanelNode(node: PanelLayoutNode): WorkbenchSharedPanelNode {
	if (node.kind === 'panel') {
		return {
			childPresentations: node.childPresentations.map(projectPresentation),
			id: node.id,
			kind: node.kind,
			primaryPresentation: projectPresentation(node.primaryPresentation),
		};
	}
	return {
		children: node.children.map((child) => ({
			id: child.id,
			node: projectSharedPanelNode(child.node),
		})),
		id: node.id,
		kind: node.kind,
		orientation: node.orientation,
	};
}

function projectPanelNodeView(
	node: PanelLayoutNode,
	panels: Array<{
		activeChildPresentationId: TerminalPresentation['presentationId'] | null;
		panelId: string;
	}>,
	splits: Array<{
		splitId: string;
		weights: Array<{ slotId: string; weight: number }>;
	}>,
): void {
	if (node.kind === 'panel') {
		panels.push({ activeChildPresentationId: node.activeChildPresentationId, panelId: node.id });
		return;
	}
	splits.push({
		splitId: node.id,
		weights: node.children.map((child) => ({ slotId: child.id, weight: child.weight })),
	});
	for (const child of node.children) projectPanelNodeView(child.node, panels, splits);
}

function projectPresentation(presentation: TerminalPresentation): WorkbenchSharedPresentation {
	return {
		intent: intentForLifecycle(presentation.lifecycle),
		launchCwd: presentation.launchCwd,
		presentationId: presentation.presentationId,
		title: presentation.title,
	};
}

function intentForLifecycle(
	lifecycle: TerminalPresentationLifecycle,
): WorkbenchSharedPresentationIntent {
	switch (lifecycle.kind) {
		case 'preview':
			throw new Error('Preview terminal presentations cannot enter a shared document.');
		case 'provisioning':
		case 'provision-failed':
			return { kind: 'provision', operationId: lifecycle.operationId };
		case 'running':
		case 'restoring':
		case 'reconciling':
		case 'restore-failed':
		case 'exited':
		case 'failed':
			return { kind: 'retain' };
		case 'stopping':
		case 'stop-failed':
			return { kind: 'stop', operationId: lifecycle.operationId };
		case 'stopped':
			return { kind: 'stopped' };
		case 'closing':
		case 'close-failed':
		case 'closed':
			return {
				kind: 'close',
				operationId: lifecycle.operationId,
				removalId: lifecycle.removalId,
			};
	}
}

function hydratePanelWorkspace(
	state: WorkbenchSharedDocument['terminalContexts'][number]['panelWorkspace'],
	view: WorkbenchClientView['terminalContexts'][number]['panelWorkspace'],
): PanelWorkspaceState {
	const tabViews = new Map(view.tabs.map((tab) => [tab.tabId, tab]));
	return {
		nextPanelSequence: state.nextPanelSequence,
		nextPanelSlotSequence: state.nextPanelSlotSequence,
		nextSplitSequence: state.nextSplitSequence,
		nextTerminalSequence: state.nextTerminalSequence,
		pendingRemovals: state.pendingRemovals.map((removal) => ({ ...removal })),
		tabs: state.tabs.map((tab) => {
			const tabView = requireDefined(tabViews.get(tab.tabId));
			const panelViews = new Map(tabView.panels.map((panel) => [panel.panelId, panel]));
			const splitViews = new Map(tabView.splits.map((split) => [split.splitId, split]));
			return {
				focusedPanelId: tabView.focusedPanelId,
				maximizedPanelId: tabView.maximizedPanelId,
				panelFocusHistory: [tabView.focusedPanelId],
				root: hydratePanelNode(tab.root, panelViews, splitViews),
				tabId: tab.tabId,
			};
		}),
	};
}

function hydratePanelNode(
	node: WorkbenchSharedPanelNode,
	panelViews: ReadonlyMap<
		string,
		WorkbenchClientView['terminalContexts'][number]['panelWorkspace']['tabs'][number]['panels'][number]
	>,
	splitViews: ReadonlyMap<
		string,
		WorkbenchClientView['terminalContexts'][number]['panelWorkspace']['tabs'][number]['splits'][number]
	>,
): PanelLayoutNode {
	if (node.kind === 'panel') {
		return {
			activeChildPresentationId: requireDefined(panelViews.get(node.id)).activeChildPresentationId,
			childPresentations: node.childPresentations.map(hydratePresentation),
			id: node.id,
			kind: node.kind,
			primaryPresentation: hydratePresentation(node.primaryPresentation),
		};
	}
	const weights = new Map(
		requireDefined(splitViews.get(node.id)).weights.map((weight) => [weight.slotId, weight.weight]),
	);
	return {
		children: node.children.map((child) => ({
			id: child.id,
			node: hydratePanelNode(child.node, panelViews, splitViews),
			weight: requireDefined(weights.get(child.id)),
		})),
		id: node.id,
		kind: node.kind,
		orientation: node.orientation,
	};
}

function hydratePresentation(presentation: WorkbenchSharedPresentation): TerminalPresentation {
	return {
		closeActivity: 'unknown',
		currentCwd: null,
		launchCwd: presentation.launchCwd,
		lifecycle: lifecycleForIntent(presentation.intent),
		naturalTitle: null,
		presentationId: presentation.presentationId,
		title: presentation.title,
	};
}

function lifecycleForIntent(
	intent: WorkbenchSharedPresentationIntent,
): TerminalPresentationLifecycle {
	switch (intent.kind) {
		case 'provision':
			return { kind: 'provisioning', operationId: intent.operationId };
		case 'retain':
			return { kind: 'restoring' };
		case 'stop':
			return { kind: 'stopping', operationId: intent.operationId };
		case 'stopped':
			return { kind: 'stopped' };
		case 'close':
			return {
				kind: 'closing',
				operationId: intent.operationId,
				removalId: intent.removalId,
			};
	}
}

function compareIdentifiers(left: string, right: string): number {
	if (left < right) return -1;
	return left === right ? 0 : 1;
}

function requireDefined<Value>(value: Value | undefined): Value {
	if (value === undefined) throw new Error('Expected a reconciled Workbench value.');
	return value;
}
