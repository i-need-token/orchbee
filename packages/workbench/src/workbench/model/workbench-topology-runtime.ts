import type { TabModelInit, TabModelSnapshot } from '@orchbee/tab-strip';

import type { ProjectWorkspaceState, WorkbenchDirectory } from '../../project-workspace';
import type { TerminalPresentationLifecycle } from '../../terminal-presentation';
import type {
	PanelLayoutNode,
	PanelWorkspaceState,
	TerminalPresentation,
} from '../components/workbench-panel-workspace/model/panel-workspace-state';
import { validatePanelWorkspaceState } from '../components/workbench-panel-workspace/model/panel-workspace-state';
import {
	createWorkbenchTopologyDocument,
	parseWorkbenchTopologyDocument,
	type WorkbenchTopologyDocument,
	type WorkbenchTopologyPanelNode,
	type WorkbenchTopologyPresentation,
	type WorkbenchTopologyPresentationIntent,
	type WorkbenchTopologyRevision,
} from '../topology';

interface WorkbenchTopologyRuntimeContextSnapshot {
	readonly nextTabSequence: number;
	readonly panelWorkspace: PanelWorkspaceState;
	readonly tabCollection: TabModelSnapshot;
	readonly terminalContextId: string;
}

export interface WorkbenchTopologyRuntimeSnapshot {
	readonly projectWorkspace: ProjectWorkspaceState;
	readonly terminalContexts: readonly WorkbenchTopologyRuntimeContextSnapshot[];
}

interface HydratedWorkbenchTopologyTerminalContext {
	readonly nextTabSequence: number;
	readonly panelWorkspace: PanelWorkspaceState;
	readonly tabCollection: TabModelInit;
	readonly terminalContextId: string;
}

export interface HydratedWorkbenchTopology {
	readonly projectWorkspace: ProjectWorkspaceState;
	readonly revision: WorkbenchTopologyRevision;
	readonly terminalContexts: readonly HydratedWorkbenchTopologyTerminalContext[];
}

export function projectWorkbenchTopology(
	snapshot: WorkbenchTopologyRuntimeSnapshot,
	revision: WorkbenchTopologyRevision,
): WorkbenchTopologyDocument {
	return createWorkbenchTopologyDocument({
		projectWorkspace: projectProjectWorkspace(snapshot.projectWorkspace),
		revision,
		terminalContexts: snapshot.terminalContexts
			.map((terminalContext) => ({
				nextTabSequence: terminalContext.nextTabSequence,
				panelWorkspace: projectPanelWorkspace(terminalContext.panelWorkspace),
				tabCollection: projectTabCollection(terminalContext.tabCollection),
				terminalContextId: terminalContext.terminalContextId,
			}))
			.sort((left, right) => left.terminalContextId.localeCompare(right.terminalContextId)),
	});
}

export function hydrateWorkbenchTopology(value: unknown): HydratedWorkbenchTopology {
	const document = parseWorkbenchTopologyDocument(value);
	return {
		projectWorkspace: document.projectWorkspace,
		revision: document.revision,
		terminalContexts: document.terminalContexts.map((terminalContext) => {
			const panelWorkspace = hydratePanelWorkspace(terminalContext.panelWorkspace);
			const invariantErrors = validatePanelWorkspaceState(panelWorkspace);
			if (invariantErrors.length > 0) {
				throw new Error(`Hydrated Panel workspace is invalid: ${invariantErrors.join('; ')}`);
			}
			return {
				nextTabSequence: terminalContext.nextTabSequence,
				panelWorkspace,
				tabCollection: {
					activeTabId: terminalContext.tabCollection.activeTabId,
					groups: terminalContext.tabCollection.groups,
					mostRecentGroupId: terminalContext.tabCollection.mostRecentGroupId,
					selectedTabIds:
						terminalContext.tabCollection.activeTabId === null
							? []
							: [terminalContext.tabCollection.activeTabId],
					tabs: terminalContext.tabCollection.tabs,
				},
				terminalContextId: terminalContext.terminalContextId,
			};
		}),
	};
}

function projectTabCollection(snapshot: TabModelSnapshot) {
	return {
		activeTabId: snapshot.activeTabId,
		groups: snapshot.groups.map((group) => ({
			collapsed: group.collapsed,
			color: group.color,
			id: group.id,
			title: group.title,
		})),
		mostRecentGroupId: snapshot.mostRecentGroupId,
		tabs: snapshot.tabs.map((tab) => ({
			group: tab.group,
			id: tab.id,
			pinned: tab.pinned,
			title: tab.title,
		})),
	};
}

function projectProjectWorkspace(state: ProjectWorkspaceState) {
	return {
		activeLocation: { ...state.activeLocation },
		expandedProjectIds: [...state.expandedProjectIds],
		nextProjectSequence: state.nextProjectSequence,
		nextTerminalContextSequence: state.nextTerminalContextSequence,
		nextWorkspaceSequence: state.nextWorkspaceSequence,
		projects: state.projects.map((project) => ({
			directories: project.directories.map(projectDirectory),
			id: project.id,
			launchConfigurations: project.launchConfigurations.map((configuration) => ({
				...configuration,
			})),
			name: project.name,
			primaryDirectoryId: project.primaryDirectoryId,
			workspaces: project.workspaces.map((workspace) => ({ ...workspace })),
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

function projectPanelWorkspace(state: PanelWorkspaceState) {
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
			focusedPanelId: tab.focusedPanelId,
			maximizedPanelId: tab.maximizedPanelId,
			root: projectPanelNode(tab.root),
			tabId: tab.tabId,
		})),
	};
}

function projectPanelNode(node: PanelLayoutNode): WorkbenchTopologyPanelNode {
	if (node.kind === 'panel') {
		return {
			activeChildPresentationId: node.activeChildPresentationId,
			childPresentations: node.childPresentations.map(projectPresentation),
			id: node.id,
			kind: node.kind,
			primaryPresentation: projectPresentation(node.primaryPresentation),
		};
	}
	return {
		children: node.children.map((child) => ({
			id: child.id,
			node: projectPanelNode(child.node),
			weight: child.weight,
		})),
		id: node.id,
		kind: node.kind,
		orientation: node.orientation,
	};
}

function projectPresentation(presentation: TerminalPresentation): WorkbenchTopologyPresentation {
	return {
		intent: intentForLifecycle(presentation.lifecycle),
		launchCwd: presentation.launchCwd,
		presentationId: presentation.presentationId,
		title: presentation.title,
	};
}

function intentForLifecycle(
	lifecycle: TerminalPresentationLifecycle,
): WorkbenchTopologyPresentationIntent {
	switch (lifecycle.kind) {
		case 'preview':
			throw new Error('Preview terminal presentations cannot enter durable topology.');
		case 'provisioning':
		case 'provision-failed':
			return { kind: 'provision', operationId: lifecycle.operationId };
		case 'running':
		case 'restoring':
		case 'exited':
			return { kind: 'retain' };
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
	state: WorkbenchTopologyDocument['terminalContexts'][number]['panelWorkspace'],
): PanelWorkspaceState {
	return {
		nextPanelSequence: state.nextPanelSequence,
		nextPanelSlotSequence: state.nextPanelSlotSequence,
		nextSplitSequence: state.nextSplitSequence,
		nextTerminalSequence: state.nextTerminalSequence,
		pendingRemovals: state.pendingRemovals.map((removal) => ({ ...removal })),
		tabs: state.tabs.map((tab) => ({
			focusedPanelId: tab.focusedPanelId,
			maximizedPanelId: tab.maximizedPanelId,
			root: hydratePanelNode(tab.root),
			tabId: tab.tabId,
		})),
	};
}

function hydratePanelNode(node: WorkbenchTopologyPanelNode): PanelLayoutNode {
	if (node.kind === 'panel') {
		return {
			activeChildPresentationId: node.activeChildPresentationId,
			childPresentations: node.childPresentations.map(hydratePresentation),
			id: node.id,
			kind: node.kind,
			primaryPresentation: hydratePresentation(node.primaryPresentation),
		};
	}
	return {
		children: node.children.map((child) => ({
			id: child.id,
			node: hydratePanelNode(child.node),
			weight: child.weight,
		})),
		id: node.id,
		kind: node.kind,
		orientation: node.orientation,
	};
}

function hydratePresentation(presentation: WorkbenchTopologyPresentation): TerminalPresentation {
	return {
		currentCwd: null,
		launchCwd: presentation.launchCwd,
		lifecycle: lifecycleForIntent(presentation.intent),
		presentationId: presentation.presentationId,
		title: presentation.title,
	};
}

function lifecycleForIntent(
	intent: WorkbenchTopologyPresentationIntent,
): TerminalPresentationLifecycle {
	switch (intent.kind) {
		case 'provision':
			return { kind: 'provisioning', operationId: intent.operationId };
		case 'retain':
			return { kind: 'restoring' };
		case 'close':
			return {
				kind: 'closing',
				operationId: intent.operationId,
				removalId: intent.removalId,
			};
	}
}
