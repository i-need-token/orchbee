import type { WorkbenchPublication } from '@orchbee/workbench';
import { deriveProjectNameFromPath } from '@orchbee/workbench/project-workspace';
import {
	createWorkbenchSharedDocument,
	terminalPresentationOperationId,
	workbenchSharedDocumentRevision,
	type TerminalPresentationLifecycle,
	type WorkbenchPanelNode,
	type WorkbenchPresentation,
	type WorkbenchSharedPanelNode,
	type WorkbenchSharedPresentation,
	type WorkbenchSnapshot,
	type WorkbenchTerminalContext,
} from '@orchbee/workbench-client-protocol';

import { serverResourceScopeId } from './serverResourceScopeId';

type Navigation = WorkbenchSnapshot['navigation'];
type Directory = Navigation['directories'][number];
type Workspace = Navigation['workspaces'][number];
type Tab = WorkbenchTerminalContext['tabs'][number];

interface WritablePublishedFacts {
	readonly presentations: Map<
		string,
		{ readonly customTitle: string | null; readonly lifecycle: TerminalPresentationLifecycle }
	>;
	readonly splitWeights: Map<string, readonly [first: number, second: number]>;
}

/** The React Workbench needs one context when no Workspace is open; a Server ID is never this. */
const NO_WORKSPACE_TERMINAL_CONTEXT_ID = 'no-workspace';

/** The Server owns every identity, so a local sequence is never consumed. */
const UNUSED_SEQUENCE = 1;

const EVEN_SPLIT_RATIO = 0.5;

/** This Client's name for a member with no terminal, beside the Server's launch failure reasons. */
const UNBOUND_TERMINAL_FAILURE_CODE = 'terminal-unbound';

/**
 * Projects what the Server published onto the model the React Workbench reads. Every Workspace gets
 * its own one-directory resource scope, because the Server has no scopes and a Workspace it holds
 * without a directory still needs one here. `defaultDirectory` only names that placeholder, which is
 * unavailable, so the Workbench opens no Workspace in it and says the Workspace has no directory.
 */
export function projectServerSnapshot(
	snapshot: WorkbenchSnapshot,
	defaultDirectory: string,
): WorkbenchPublication {
	const { navigation } = snapshot;
	const directories = new Map(
		navigation.directories.map((directory) => [directory.directoryId, directory]),
	);
	const workspaces = inDisplayOrder(
		navigation.workspaces,
		navigation.workspaceOrder,
		(workspace) => workspace.workspaceId,
	).map((workspace) => ({
		directory: projectWorkspaceDirectory(workspace, directories, defaultDirectory),
		scopeId: serverResourceScopeId(workspace.workspaceId),
		workspace,
	}));
	const facts: WritablePublishedFacts = { presentations: new Map(), splitWeights: new Map() };
	return {
		contextFingerprints: new Map(
			snapshot.terminalContexts.map((context) => [
				context.terminalContextId,
				JSON.stringify(context),
			]),
		),
		document: createWorkbenchSharedDocument({
			projectWorkspace: {
				nextProjectSequence: UNUSED_SEQUENCE,
				nextResourceScopeSequence: UNUSED_SEQUENCE,
				nextTerminalContextSequence: UNUSED_SEQUENCE,
				nextWorkspaceSequence: UNUSED_SEQUENCE,
				projects: inDisplayOrder(
					navigation.projects,
					navigation.projectOrder,
					(project) => project.projectId,
				).map((project) => ({ id: project.projectId, name: project.label })),
				resourceScopes: workspaces.map(({ directory, scopeId, workspace }) => ({
					directories: [directory],
					id: scopeId,
					launchConfigurations: [],
					name: workspace.label,
					primaryDirectoryId: directory.id,
				})),
				standaloneTerminalContextId: NO_WORKSPACE_TERMINAL_CONTEXT_ID,
				workspaces: workspaces.map(({ directory, scopeId, workspace }) => ({
					id: workspace.workspaceId,
					launchConfigurationId: null,
					name: workspace.label,
					primaryDirectoryId: directory.id,
					projectId: workspace.projectId,
					resourceScopeId: scopeId,
					terminalContextId: workspace.terminalContextId,
				})),
			},
			revision: workbenchSharedDocumentRevision(snapshot.revision),
			terminalContexts: [
				{
					nextTabSequence: UNUSED_SEQUENCE,
					panelWorkspace: {
						nextPanelSequence: UNUSED_SEQUENCE,
						nextPanelSlotSequence: UNUSED_SEQUENCE,
						nextSplitSequence: UNUSED_SEQUENCE,
						nextTerminalSequence: UNUSED_SEQUENCE,
						pendingRemovals: [],
						tabs: [],
					},
					tabCollection: { groups: [], tabs: [] },
					terminalContextId: NO_WORKSPACE_TERMINAL_CONTEXT_ID,
				},
				...snapshot.terminalContexts.map((context) => projectTerminalContext(context, facts)),
			],
		}),
		presentations: facts.presentations,
		splitWeights: facts.splitWeights,
	};
}

/** An order is either absent or a complete permutation; identity order is the display order then. */
function inDisplayOrder<Item>(
	items: readonly Item[],
	order: readonly string[] | undefined,
	identityOf: (item: Item) => string,
): readonly Item[] {
	if (order === undefined || order.length === 0) return items;
	const positions = new Map(order.map((identity, position) => [identity, position]));
	const positionOf = (item: Item): number => positions.get(identityOf(item)) ?? order.length;
	return [...items].sort((left, right) => positionOf(left) - positionOf(right));
}

function projectWorkspaceDirectory(
	workspace: Workspace,
	directories: ReadonlyMap<string, Directory>,
	defaultDirectory: string,
) {
	if (workspace.primaryDirectoryId === null) {
		return projectDirectory(
			`no-directory:${workspace.workspaceId}`,
			workspace.terminalDirectory ?? defaultDirectory,
			false,
		);
	}
	const directory = directories.get(workspace.primaryDirectoryId);
	if (directory === undefined) {
		throw new Error(`Workspace ${workspace.workspaceId} names a missing directory.`);
	}
	return projectDirectory(directory.directoryId, directory.path, true);
}

function projectDirectory(id: string, path: string, available: boolean) {
	return {
		available,
		git: null,
		id,
		kind: 'folder' as const,
		name: deriveProjectNameFromPath(path),
		path,
		placement: { kind: 'local' as const },
		source: 'selected' as const,
	};
}

function projectTerminalContext(context: WorkbenchTerminalContext, facts: WritablePublishedFacts) {
	return {
		nextTabSequence: UNUSED_SEQUENCE,
		panelWorkspace: {
			nextPanelSequence: UNUSED_SEQUENCE,
			nextPanelSlotSequence: UNUSED_SEQUENCE,
			nextSplitSequence: UNUSED_SEQUENCE,
			nextTerminalSequence: UNUSED_SEQUENCE,
			pendingRemovals: [],
			tabs: context.tabs.map((tab) => ({
				root: projectPanelNode(tab.root, facts),
				tabId: tab.tabId,
			})),
		},
		tabCollection: {
			groups: context.groups.map((group) => ({
				color: group.color,
				id: group.groupId,
				title: group.label,
			})),
			tabs: context.tabs.map((tab, position) => ({
				customTitle: customTabTitle(tab),
				group: tab.groupId,
				id: tab.tabId,
				pinned: tab.pinned,
				title: tab.automaticLabel === true ? `${position + 1}` : tab.label,
			})),
		},
		terminalContextId: context.terminalContextId,
	};
}

/** A Tab without a chosen name follows its focused terminal, so a blank label chooses nothing. */
function customTabTitle(tab: Tab): string | null {
	if (tab.automaticLabel === true) return null;
	const title = tab.label.trim();
	return title.length === 0 ? null : title;
}

function projectPanelNode(
	node: WorkbenchPanelNode,
	facts: WritablePublishedFacts,
): WorkbenchSharedPanelNode {
	if (node.kind === 'panel') {
		return {
			childPresentations: node.childPresentations.map((presentation) =>
				projectPresentation(presentation, facts),
			),
			id: node.panelId,
			kind: 'panel',
			primaryPresentation: projectPresentation(node.primaryPresentation, facts),
		};
	}
	const firstWeight = (node.ratio ?? EVEN_SPLIT_RATIO) * 100;
	facts.splitWeights.set(node.splitId, [firstWeight, 100 - firstWeight]);
	return {
		children: node.children.map((slot) => ({
			id: slot.slotId,
			node: projectPanelNode(slot.node, facts),
		})),
		id: node.splitId,
		kind: 'split',
		orientation: node.orientation,
	};
}

function projectPresentation(
	presentation: WorkbenchPresentation,
	facts: WritablePublishedFacts,
): WorkbenchSharedPresentation {
	const { intent, lifecycle } = projectTerminalState(presentation);
	facts.presentations.set(presentation.presentationId, {
		customTitle: presentation.customLabel,
		lifecycle,
	});
	return {
		intent,
		launchCwd: null,
		presentationId: presentation.presentationId,
		title: presentation.label,
	};
}

/**
 * A surface attaches only to a `running` terminal's binding, and reports for itself once that
 * terminal ends. A failed launch keeps its binding, yet every Client leaves it unattached, and a
 * member with no binding never launches by itself, since the Server binds one only when a command or
 * a restore starts its terminal. Neither is `running`, nor `failed`, which a surface would try to
 * attach to, nor `provisioning`, which would claim a launch is under way. Nothing retries either: a
 * person creates another terminal or closes this one.
 */
function projectTerminalState(presentation: WorkbenchPresentation): {
	readonly intent: WorkbenchSharedPresentation['intent'];
	readonly lifecycle: TerminalPresentationLifecycle;
} {
	if (presentation.binding !== null && presentation.launchFailure === undefined) {
		return { intent: { kind: 'retain' }, lifecycle: { kind: 'running' } };
	}
	const operationId = terminalPresentationOperationId(
		`server-launch:${presentation.presentationId}`,
	);
	return {
		intent: { kind: 'provision', operationId },
		lifecycle: {
			failure: {
				code: presentation.launchFailure?.reason ?? UNBOUND_TERMINAL_FAILURE_CODE,
				retryable: false,
			},
			kind: 'provision-failed',
			operationId,
		},
	};
}
